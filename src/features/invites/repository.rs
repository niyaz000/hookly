use chrono::{DateTime, Utc};
use sqlx::{types::Json, QueryBuilder};
use tracing::debug;
use uuid::Uuid;

use crate::error::AppError;

use super::models::{InviteRow, TenantMemberRow};

const INVITE_SELECT: &str = r#"
    SELECT
        id, public_id, tenant_id, organization_id,
        user_email, role, status, token_hash, tags, metadata,
        created_by, request_id, version,
        created_at, updated_at, deleted_at,
        revoked_at, accepted_at, expires_at
    FROM invites
"#;

pub struct InviteRepository {
    pool: crate::common::CountingPool,
}

impl InviteRepository {
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

    #[allow(clippy::too_many_arguments)]
    pub async fn create(
        &self,
        tenant_id: Uuid,
        organization_id: Uuid,
        user_email: &str,
        role: &str,
        token_hash: &str,
        tags: &serde_json::Value,
        metadata: &serde_json::Value,
        expires_at: DateTime<Utc>,
        created_by: Uuid,
        request_id: Uuid,
    ) -> Result<InviteRow, AppError> {
        let id = Uuid::now_v7();
        let public_id = InviteRow::new_public_id();

        debug!(public_id = %public_id, "inserting invite");

        sqlx::query(
            r#"
            INSERT INTO invites (
                id, public_id, tenant_id, organization_id,
                user_email, role, status, token_hash,
                tags, metadata, expires_at,
                created_by, request_id, version,
                created_at, updated_at
            ) VALUES (
                $1, $2, $3, $4,
                $5, $6, 'sent', $7,
                $8, $9, $10,
                $11, $12, 1,
                NOW(), NOW()
            )
            "#,
        )
        .bind(id)
        .bind(&public_id)
        .bind(tenant_id)
        .bind(organization_id)
        .bind(user_email)
        .bind(role)
        .bind(token_hash)
        .bind(Json(tags))
        .bind(Json(metadata))
        .bind(expires_at)
        .bind(created_by)
        .bind(request_id)
        .execute(&self.pool)
        .await?;

        self.get_by_id(id)
            .await?
            .ok_or_else(|| AppError::Internal("invite created but not found on fetch".into()))
    }

    async fn get_by_id(&self, id: Uuid) -> Result<Option<InviteRow>, AppError> {
        let sql = format!("{} WHERE id = $1", INVITE_SELECT);
        sqlx::query_as::<_, InviteRow>(&sql)
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::from)
    }

    pub async fn get_by_public_id(&self, public_id: &str) -> Result<Option<InviteRow>, AppError> {
        debug!(public_id = %public_id, "querying invite");
        let sql = format!("{} WHERE public_id = $1 AND deleted_at IS NULL", INVITE_SELECT);
        sqlx::query_as::<_, InviteRow>(&sql)
            .bind(public_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::from)
    }

    pub async fn get_by_token_hash(&self, token_hash: &str) -> Result<Option<InviteRow>, AppError> {
        debug!("querying invite by token hash");
        let sql = format!("{} WHERE token_hash = $1 AND deleted_at IS NULL", INVITE_SELECT);
        sqlx::query_as::<_, InviteRow>(&sql)
            .bind(token_hash)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::from)
    }

    pub async fn set_status(
        &self,
        id: Uuid,
        status: &str,
        revoked_at: Option<DateTime<Utc>>,
        accepted_at: Option<DateTime<Utc>>,
    ) -> Result<Option<InviteRow>, AppError> {
        debug!(%id, %status, "updating invite status");

        let updated_id: Option<Uuid> = sqlx::query_scalar(
            r#"
            UPDATE invites SET
                status      = $2,
                revoked_at  = COALESCE($3, revoked_at),
                accepted_at = COALESCE($4, accepted_at),
                version     = version + 1,
                updated_at  = NOW()
            WHERE id = $1 AND deleted_at IS NULL
            RETURNING id
            "#,
        )
        .bind(id)
        .bind(status)
        .bind(revoked_at)
        .bind(accepted_at)
        .fetch_optional(&self.pool)
        .await?;

        match updated_id {
            Some(id) => self.get_by_id(id).await,
            None => Ok(None),
        }
    }

    pub async fn delete(&self, public_id: &str) -> Result<bool, AppError> {
        debug!(public_id = %public_id, "soft deleting invite");

        let result = sqlx::query(
            r#"
            UPDATE invites SET
                deleted_at = NOW(),
                version    = version + 1,
                updated_at = NOW()
            WHERE public_id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(public_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn resend(
        &self,
        public_id: &str,
        new_token_hash: &str,
        new_expires_at: DateTime<Utc>,
        created_by: Uuid,
        request_id: Uuid,
    ) -> Result<Option<InviteRow>, AppError> {
        debug!(public_id = %public_id, "resending invite");

        let mut tx = self.pool.begin().await?;

        let old: Option<(Uuid, Uuid, Uuid, String, String, serde_json::Value, serde_json::Value)> =
            sqlx::query_as(
                r#"
                UPDATE invites SET
                    status     = 'revoked',
                    revoked_at = NOW(),
                    version    = version + 1,
                    updated_at = NOW()
                WHERE public_id = $1
                  AND status IN ('sent', 'failed', 'opened')
                  AND deleted_at IS NULL
                RETURNING id, tenant_id, organization_id, user_email, role, tags, metadata
                "#,
            )
            .bind(public_id)
            .fetch_optional(&mut *tx)
            .await?;

        let Some((_, tenant_id, organization_id, user_email, role, tags, metadata)) = old else {
            tx.rollback().await?;
            return Ok(None);
        };

        let new_id = Uuid::now_v7();
        let new_public_id = InviteRow::new_public_id();

        sqlx::query(
            r#"
            INSERT INTO invites (
                id, public_id, tenant_id, organization_id,
                user_email, role, status, token_hash,
                tags, metadata, expires_at,
                created_by, request_id, version,
                created_at, updated_at
            ) VALUES (
                $1, $2, $3, $4,
                $5, $6, 'sent', $7,
                $8, $9, $10,
                $11, $12, 1,
                NOW(), NOW()
            )
            "#,
        )
        .bind(new_id)
        .bind(&new_public_id)
        .bind(tenant_id)
        .bind(organization_id)
        .bind(&user_email)
        .bind(&role)
        .bind(new_token_hash)
        .bind(Json(&tags))
        .bind(Json(&metadata))
        .bind(new_expires_at)
        .bind(created_by)
        .bind(request_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        self.get_by_id(new_id).await
    }

    pub async fn list(
        &self,
        limit: i64,
        cursor: Option<Uuid>,
        tenant_id: Option<Uuid>,
        organization_id: Option<Uuid>,
        status: Option<&str>,
        user_email: Option<&str>,
    ) -> Result<(Vec<InviteRow>, Option<Uuid>), AppError> {
        debug!(limit = limit, "listing invites");

        let mut qb = QueryBuilder::<sqlx::Postgres>::new(format!(
            "{} WHERE deleted_at IS NULL",
            INVITE_SELECT
        ));

        if let Some(c) = cursor {
            qb.push(" AND id > ").push_bind(c);
        }
        if let Some(tid) = tenant_id {
            qb.push(" AND tenant_id = ").push_bind(tid);
        }
        if let Some(oid) = organization_id {
            qb.push(" AND organization_id = ").push_bind(oid);
        }
        if let Some(s) = status {
            qb.push(" AND status = ").push_bind(s.to_string());
        }
        if let Some(email) = user_email {
            qb.push(" AND user_email = ").push_bind(email.to_string());
        }

        qb.push(" ORDER BY id ASC LIMIT ").push_bind(limit + 1);

        let mut rows: Vec<InviteRow> = qb
            .build_query_as::<InviteRow>()
            .fetch_all(&self.pool)
            .await?;

        let next_cursor = if rows.len() as i64 > limit {
            rows.pop().map(|r| r.id)
        } else {
            None
        };

        Ok((rows, next_cursor))
    }

    pub async fn create_member(
        &self,
        invite_id: Uuid,
        _invite_public_id: &str,
        tenant_id: Uuid,
        organization_id: Uuid,
        user_email: &str,
        role: &str,
    ) -> Result<TenantMemberRow, AppError> {
        let id = Uuid::now_v7();
        let public_id = TenantMemberRow::new_public_id();

        debug!(public_id = %public_id, "inserting tenant member");

        sqlx::query(
            r#"
            INSERT INTO tenant_members (
                id, public_id, tenant_id, organization_id,
                invite_id, user_email, role,
                status, created_at, updated_at
            ) VALUES (
                $1, $2, $3, $4,
                $5, $6, $7,
                'active', NOW(), NOW()
            )
            "#,
        )
        .bind(id)
        .bind(&public_id)
        .bind(tenant_id)
        .bind(organization_id)
        .bind(invite_id)
        .bind(user_email)
        .bind(role)
        .execute(&self.pool)
        .await?;

        sqlx::query_as::<_, TenantMemberRow>(
            r#"
            SELECT
                m.id, m.public_id, m.tenant_id, m.organization_id,
                m.invite_id, i.public_id AS invite_public_id,
                m.user_email, m.user_id, m.role, m.status,
                m.created_at, m.updated_at, m.deleted_at
            FROM tenant_members m
            JOIN invites i ON i.id = m.invite_id
            WHERE m.id = $1
            "#,
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)
    }
}
