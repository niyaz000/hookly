use chrono::Utc;
use sqlx::{types::Json, PgPool, QueryBuilder};
use tracing::debug;
use uuid::Uuid;

use crate::{
    common::{types::RequestContext, NanoId},
    error::AppError,
};

use super::models::{
    CreateOrganizationRequest, Organization, OrganizationStatus, UpdateOrganizationRequest,
};

pub struct OrganizationRepository {
    pool: PgPool,
}

impl OrganizationRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(
        &self,
        req: CreateOrganizationRequest,
        ctx: RequestContext,
    ) -> Result<Organization, AppError> {
        let id = Uuid::now_v7();
        let public_id = format!("org_{}", NanoId::generate(20));
        let now = Utc::now();

        let org = sqlx::query_as::<_, Organization>(
            r#"
            INSERT INTO organizations (
                id, public_id, name, slug,
                owner_email, external_id,
                tags,
                created_by, updated_by, request_id, version,
                created_at, updated_at
            ) VALUES (
                $1, $2, $3, $4,
                $5, $6,
                $7,
                $8, $8, $9, $10,
                $11, $11
            )
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(&public_id)
        .bind(&req.name.trim())
        .bind(&&req.slug.to_lowercase().trim())
        .bind(&req.owner_email)
        .bind(req.external_id.as_deref())
        .bind(Json(req.tags.unwrap_or_default()))
        .bind(ctx.created_by)
        .bind(ctx.request_id)
        .bind(0i32)
        .bind(now)
        .fetch_one(&self.pool)
        .await?;

        Ok(org)
    }

    pub async fn get_by_public_id(
        &self,
        public_id: &str,
    ) -> Result<Option<Organization>, AppError> {
        debug!(public_id = %public_id, "querying organization");

        let org =
            sqlx::query_as::<_, Organization>("SELECT * FROM organizations WHERE public_id = $1")
                .bind(public_id)
                .fetch_optional(&self.pool)
                .await?;

        Ok(org)
    }

    pub async fn update(
        &self,
        public_id: &str,
        req: UpdateOrganizationRequest,
        ctx: RequestContext,
    ) -> Result<Option<Organization>, AppError> {
        debug!(public_id = %public_id, "updating organization");

        let org = sqlx::query_as::<_, Organization>(
            r#"
            UPDATE organizations SET
                name        = COALESCE($1, name),
                slug        = COALESCE($2, slug),
                owner_email = COALESCE($3, owner_email),
                external_id = COALESCE($4, external_id),
                tags        = COALESCE($5, tags),
                updated_by  = $6,
                request_id  = $7,
                version     = version + 1,
                updated_at  = NOW()
            WHERE public_id = $8 AND deleted_at IS NULL
            RETURNING *
            "#,
        )
        .bind(req.name)
        .bind(req.slug)
        .bind(req.owner_email)
        .bind(req.external_id)
        .bind(req.tags.map(Json))
        .bind(ctx.created_by)
        .bind(ctx.request_id)
        .bind(public_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(org)
    }

    pub async fn delete(&self, public_id: &str, ctx: RequestContext) -> Result<bool, AppError> {
        debug!(public_id = %public_id, "soft deleting organization");

        let result = sqlx::query(
            r#"
            UPDATE organizations SET
                deleted_at = NOW(),
                status     = 'inactive',
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

    /// Suspend an organization. When `caller_org_id` is Some, the UPDATE is
    /// scoped so only the org matching that internal UUID is affected — enforcing
    /// ownership. Pass None for admin paths that bypass ownership.
    pub async fn suspend(
        &self,
        public_id: &str,
        caller_org_id: Option<Uuid>,
        ctx: RequestContext,
    ) -> Result<Option<Organization>, AppError> {
        debug!(public_id = %public_id, "suspending organization");

        let org = if let Some(org_id) = caller_org_id {
            sqlx::query_as::<_, Organization>(
                r#"
                UPDATE organizations SET
                    status     = 'suspended',
                    updated_by = $2,
                    request_id = $3,
                    version    = version + 1,
                    updated_at = NOW()
                WHERE public_id = $1 AND id = $4 AND status = 'active' AND deleted_at IS NULL
                RETURNING *
                "#,
            )
            .bind(public_id)
            .bind(ctx.created_by)
            .bind(ctx.request_id)
            .bind(org_id)
            .fetch_optional(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, Organization>(
                r#"
                UPDATE organizations SET
                    status     = 'suspended',
                    updated_by = $2,
                    request_id = $3,
                    version    = version + 1,
                    updated_at = NOW()
                WHERE public_id = $1 AND status = 'active' AND deleted_at IS NULL
                RETURNING *
                "#,
            )
            .bind(public_id)
            .bind(ctx.created_by)
            .bind(ctx.request_id)
            .fetch_optional(&self.pool)
            .await?
        };

        Ok(org)
    }

    pub async fn restore(
        &self,
        public_id: &str,
        ctx: RequestContext,
    ) -> Result<Option<Organization>, AppError> {
        debug!(public_id = %public_id, "restoring organization");

        let org = sqlx::query_as::<_, Organization>(
            r#"
            UPDATE organizations SET
                deleted_at = NULL,
                status     = 'active',
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

        Ok(org)
    }

    pub async fn list(
        &self,
        limit: i64,
        cursor: Option<Uuid>,
        status: Option<OrganizationStatus>,
    ) -> Result<(Vec<Organization>, Option<Uuid>), AppError> {
        debug!(limit = limit, "listing organizations");

        let mut qb = QueryBuilder::<sqlx::Postgres>::new(
            "SELECT * FROM organizations WHERE deleted_at IS NULL",
        );

        if let Some(cursor_id) = cursor {
            qb.push(" AND id > ").push_bind(cursor_id);
        }

        if let Some(s) = status {
            qb.push(" AND status = ").push_bind(s);
        }

        qb.push(" ORDER BY id ASC LIMIT ").push_bind(limit + 1);

        let mut orgs: Vec<Organization> = qb
            .build_query_as::<Organization>()
            .fetch_all(&self.pool)
            .await?;

        let next_cursor = if orgs.len() as i64 > limit {
            orgs.pop().map(|o| o.id)
        } else {
            None
        };

        Ok((orgs, next_cursor))
    }
}
