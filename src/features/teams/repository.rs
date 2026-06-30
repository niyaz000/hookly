use sqlx::{types::Json, QueryBuilder};
use tracing::debug;
use uuid::Uuid;

use crate::{
    common::{types::RequestContext, NanoId},
    error::AppError,
};

use super::models::{CreateTeamRequest, Team, TeamMember, UpdateTeamRequest};

pub struct TeamRepository {
    pool: crate::common::CountingPool,
}

impl TeamRepository {
    pub fn new(pool: crate::common::CountingPool) -> Self {
        Self { pool }
    }

    pub async fn resolve_tenant(&self, public_id: &str) -> Result<Option<Uuid>, AppError> {
        sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM tenants WHERE public_id = $1 AND deleted_at IS NULL",
        )
        .bind(public_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::from)
    }

    pub async fn resolve_organization(&self, public_id: &str) -> Result<Option<Uuid>, AppError> {
        sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM organizations WHERE public_id = $1 AND deleted_at IS NULL",
        )
        .bind(public_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::from)
    }

    pub async fn resolve_users(&self, public_ids: &[String]) -> Result<Vec<Uuid>, AppError> {
        if public_ids.is_empty() {
            return Ok(vec![]);
        }
        let rows: Vec<(Uuid, String)> = sqlx::query_as(
            "SELECT id, public_id FROM identity.users \
             WHERE public_id = ANY($1) AND deleted_at IS NULL",
        )
        .bind(public_ids)
        .fetch_all(&self.pool)
        .await?;

        let missing: Vec<&str> = public_ids
            .iter()
            .filter(|pid| !rows.iter().any(|(_, found)| found == *pid))
            .map(|s| s.as_str())
            .collect();

        if !missing.is_empty() {
            return Err(AppError::NotFound(format!(
                "Users not found: {}",
                missing.join(", ")
            )));
        }

        Ok(rows.into_iter().map(|(id, _)| id).collect())
    }

    pub async fn create(
        &self,
        req: CreateTeamRequest,
        tenant_id: Uuid,
        organization_id: Uuid,
        ctx: RequestContext,
    ) -> Result<Team, AppError> {
        let id = Uuid::now_v7();
        let public_id = format!("tea_{}", NanoId::generate(20));

        debug!(public_id = %public_id, "inserting team");

        let team = sqlx::query_as::<_, Team>(
            r#"
            INSERT INTO teams (
                id, public_id, name, tenant_id, organization_id, description,
                tags, metadata, settings,
                created_by, updated_by, request_id, version,
                created_at, updated_at
            ) VALUES (
                $1, $2, $3, $4, $5, $6,
                $7, $8, $9,
                $10, $10, $11, 1,
                NOW(), NOW()
            )
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(&public_id)
        .bind(&req.name)
        .bind(tenant_id)
        .bind(organization_id)
        .bind(req.description.as_deref())
        .bind(Json(req.tags.unwrap_or_default()))
        .bind(Json(req.metadata.unwrap_or_default()))
        .bind(Json(req.settings.unwrap_or_default()))
        .bind(ctx.created_by)
        .bind(ctx.request_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(team)
    }

    pub async fn get_by_public_id(&self, public_id: &str) -> Result<Option<Team>, AppError> {
        debug!(public_id = %public_id, "querying team");

        let team = sqlx::query_as::<_, Team>(
            "SELECT * FROM teams WHERE public_id = $1 AND deleted_at IS NULL",
        )
        .bind(public_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(team)
    }

    pub async fn update(
        &self,
        public_id: &str,
        req: UpdateTeamRequest,
        ctx: RequestContext,
    ) -> Result<Option<Team>, AppError> {
        debug!(public_id = %public_id, "updating team");

        let team = sqlx::query_as::<_, Team>(
            r#"
            UPDATE teams SET
                name        = COALESCE($1, name),
                description = COALESCE($2, description),
                tags        = COALESCE($3, tags),
                metadata    = COALESCE($4, metadata),
                settings    = COALESCE($5, settings),
                updated_by  = $6,
                request_id  = $7,
                version     = version + 1,
                updated_at  = NOW()
            WHERE public_id = $8 AND deleted_at IS NULL
            RETURNING *
            "#,
        )
        .bind(req.name)
        .bind(req.description)
        .bind(req.tags.map(Json))
        .bind(req.metadata.map(Json))
        .bind(req.settings.map(Json))
        .bind(ctx.created_by)
        .bind(ctx.request_id)
        .bind(public_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(team)
    }

    pub async fn delete(&self, public_id: &str, ctx: RequestContext) -> Result<bool, AppError> {
        debug!(public_id = %public_id, "soft deleting team");

        let result = sqlx::query(
            r#"
            UPDATE teams SET
                deleted_at = NOW(),
                updated_by = $2,
                request_id = $3,
                version    = version + 1,
                updated_at = NOW()
            WHERE public_id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(public_id)
        .bind(ctx.created_by)
        .bind(ctx.request_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn restore(
        &self,
        public_id: &str,
        ctx: RequestContext,
    ) -> Result<Option<Team>, AppError> {
        debug!(public_id = %public_id, "restoring team");

        let team = sqlx::query_as::<_, Team>(
            r#"
            UPDATE teams SET
                deleted_at = NULL,
                updated_by = $2,
                request_id = $3,
                version    = version + 1,
                updated_at = NOW()
            WHERE public_id = $1
            RETURNING *
            "#,
        )
        .bind(public_id)
        .bind(ctx.created_by)
        .bind(ctx.request_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(team)
    }

    pub async fn list(
        &self,
        limit: i64,
        cursor: Option<Uuid>,
        organization_id: Option<Uuid>,
        tenant_id: Option<Uuid>,
    ) -> Result<(Vec<Team>, Option<Uuid>), AppError> {
        debug!(limit = limit, "listing teams");

        let mut qb =
            QueryBuilder::<sqlx::Postgres>::new("SELECT * FROM teams WHERE deleted_at IS NULL");

        if let Some(cursor_id) = cursor {
            qb.push(" AND id > ").push_bind(cursor_id);
        }

        if let Some(org_id) = organization_id {
            qb.push(" AND organization_id = ").push_bind(org_id);
        }

        if let Some(t_id) = tenant_id {
            qb.push(" AND tenant_id = ").push_bind(t_id);
        }

        qb.push(" ORDER BY id ASC LIMIT ").push_bind(limit + 1);

        let mut teams: Vec<Team> = qb.build_query_as::<Team>().fetch_all(&self.pool).await?;

        let next_cursor = if teams.len() as i64 > limit {
            teams.pop().map(|t| t.id)
        } else {
            None
        };

        Ok((teams, next_cursor))
    }

    pub async fn add_members(
        &self,
        team: &Team,
        user_ids: Vec<Uuid>,
        ctx: RequestContext,
    ) -> Result<Vec<TeamMember>, AppError> {
        debug!(team_id = %team.id, count = user_ids.len(), "adding team members");

        let mut members = Vec::with_capacity(user_ids.len());
        for user_id in user_ids {
            let id = Uuid::now_v7();
            let public_id = format!("mem_{}", NanoId::generate(20));

            let member = sqlx::query_as::<_, TeamMember>(
                r#"
                INSERT INTO team_members (
                    id, public_id, tenant_id, organization_id, team_id, user_id,
                    created_by, updated_by, request_id, version, created_at, updated_at
                ) VALUES (
                    $1, $2, $3, $4, $5, $6,
                    $7, $7, $8, 1, NOW(), NOW()
                )
                ON CONFLICT (team_id, user_id) DO UPDATE SET
                    deleted_at = NULL,
                    updated_by = EXCLUDED.updated_by,
                    request_id = EXCLUDED.request_id,
                    version    = team_members.version + 1,
                    updated_at = NOW()
                RETURNING *
                "#,
            )
            .bind(id)
            .bind(&public_id)
            .bind(team.tenant_id)
            .bind(team.organization_id)
            .bind(team.id)
            .bind(user_id)
            .bind(ctx.created_by)
            .bind(ctx.request_id)
            .fetch_one(&self.pool)
            .await?;

            members.push(member);
        }

        Ok(members)
    }

    pub async fn remove_member(
        &self,
        team_id: Uuid,
        member_public_id: &str,
        ctx: RequestContext,
    ) -> Result<bool, AppError> {
        debug!(team_id = %team_id, member_public_id = %member_public_id, "removing team member");

        let result = sqlx::query(
            r#"
            UPDATE team_members SET
                deleted_at = NOW(),
                updated_by = $3,
                request_id = $4,
                version    = version + 1,
                updated_at = NOW()
            WHERE public_id = $1 AND team_id = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(member_public_id)
        .bind(team_id)
        .bind(ctx.created_by)
        .bind(ctx.request_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }
}
