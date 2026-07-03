use sqlx::{QueryBuilder};
use tracing::debug;
use uuid::Uuid;

use crate::common::{types::RequestContext, NanoId};
use crate::error::AppError;

use super::models::{JwtAlgorithm, JwtKey, JwtKeyStatus, JwtKeyUse};

const SELECT_COLS: &str = "
    id, public_id, tenant_id, application_id, name, key_use, algorithm,
    key_id, status, public_key, private_key_enc, secret_enc,
    expires_at, grace_period_ends_at, rotated_from_id, last_rotated_at,
    version, created_by, updated_by, created_at, updated_at
";

#[derive(Clone)]
pub struct JwtKeyRepository {
    pool: crate::common::CountingPool,
}

impl JwtKeyRepository {
    pub fn new(pool: crate::common::CountingPool) -> Self {
        Self { pool }
    }

    pub async fn create(
        &self,
        tenant_id: Uuid,
        application_id: Option<String>,
        name: String,
        key_use: JwtKeyUse,
        algorithm: JwtAlgorithm,
        public_key: Option<String>,
        private_key_enc: Option<String>,
        secret_enc: Option<String>,
        expires_at: Option<chrono::DateTime<chrono::Utc>>,
        ctx: RequestContext,
    ) -> Result<JwtKey, AppError> {
        let id = Uuid::now_v7();
        let public_id = JwtKey::new_public_id();
        let key_id = format!("kid_{}", NanoId::new());

        debug!(public_id = %public_id, tenant_id = %tenant_id, name = %name, "inserting jwt key");

        let key = sqlx::query_as::<_, JwtKey>(&format!(
            r#"
            INSERT INTO jwt_keys (
                id, public_id, tenant_id, application_id, name,
                key_use, algorithm, key_id, status,
                public_key, private_key_enc, secret_enc,
                expires_at, version, created_by, updated_by
            ) VALUES (
                $1, $2, $3, $4, $5,
                $6, $7, $8, 'active',
                $9, $10, $11,
                $12, 0, $13, $13
            )
            RETURNING {SELECT_COLS}
            "#
        ))
        .bind(id)
        .bind(&public_id)
        .bind(tenant_id)
        .bind(&application_id)
        .bind(&name)
        .bind(&key_use)
        .bind(&algorithm)
        .bind(&key_id)
        .bind(&public_key)
        .bind(&private_key_enc)
        .bind(&secret_enc)
        .bind(expires_at)
        .bind(ctx.created_by)
        .fetch_one(&self.pool)
        .await?;

        Ok(key)
    }

    pub async fn get_by_public_id(&self, public_id: &str) -> Result<Option<JwtKey>, AppError> {
        debug!(public_id = %public_id, "querying jwt key");

        let key = sqlx::query_as::<_, JwtKey>(&format!(
            "SELECT {SELECT_COLS} FROM jwt_keys WHERE public_id = $1 AND deleted_at IS NULL"
        ))
        .bind(public_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(key)
    }

    pub async fn list(
        &self,
        tenant_id: Option<Uuid>,
        application_id: Option<String>,
        key_use: Option<JwtKeyUse>,
        status: Option<JwtKeyStatus>,
        limit: i64,
        cursor: Option<String>,
    ) -> Result<(Vec<JwtKey>, Option<String>), AppError> {
        debug!(tenant_id = ?tenant_id, limit = limit, "listing jwt keys");

        let cols = SELECT_COLS;
        let mut qb = QueryBuilder::<sqlx::Postgres>::new(format!(
            "SELECT {cols} FROM jwt_keys WHERE deleted_at IS NULL"
        ));

        if let Some(tid) = tenant_id {
            qb.push(" AND tenant_id = ").push_bind(tid);
        }
        if let Some(app_id) = application_id {
            qb.push(" AND application_id = ").push_bind(app_id);
        }
        if let Some(ku) = key_use {
            qb.push(" AND key_use = ").push_bind(ku);
        }
        if let Some(st) = status {
            qb.push(" AND status = ").push_bind(st);
        }
        if let Some(ref c) = cursor {
            qb.push(" AND public_id > ").push_bind(c.clone());
        }

        qb.push(" ORDER BY public_id ASC LIMIT ").push_bind(limit + 1);

        let mut keys: Vec<JwtKey> =
            qb.build_query_as::<JwtKey>().fetch_all(&self.pool).await?;

        let next_cursor = if keys.len() as i64 > limit {
            keys.pop().map(|k| k.public_id)
        } else {
            None
        };

        Ok((keys, next_cursor))
    }

    pub async fn update(
        &self,
        public_id: &str,
        name: Option<String>,
        expires_at: Option<chrono::DateTime<chrono::Utc>>,
        ctx: RequestContext,
    ) -> Result<Option<JwtKey>, AppError> {
        debug!(public_id = %public_id, "updating jwt key");

        let key = sqlx::query_as::<_, JwtKey>(&format!(
            r#"
            UPDATE jwt_keys SET
                name       = COALESCE($1, name),
                expires_at = COALESCE($2, expires_at),
                updated_by = $3,
                version    = version + 1,
                updated_at = NOW()
            WHERE public_id = $4 AND deleted_at IS NULL
            RETURNING {SELECT_COLS}
            "#
        ))
        .bind(&name)
        .bind(expires_at)
        .bind(ctx.created_by)
        .bind(public_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(key)
    }

    pub async fn set_status(
        &self,
        public_id: &str,
        status: JwtKeyStatus,
        ctx: RequestContext,
    ) -> Result<Option<JwtKey>, AppError> {
        debug!(public_id = %public_id, "updating jwt key status");

        let key = sqlx::query_as::<_, JwtKey>(&format!(
            r#"
            UPDATE jwt_keys SET
                status     = $1,
                updated_by = $2,
                version    = version + 1,
                updated_at = NOW()
            WHERE public_id = $3 AND deleted_at IS NULL
            RETURNING {SELECT_COLS}
            "#
        ))
        .bind(status)
        .bind(ctx.created_by)
        .bind(public_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(key)
    }

    pub async fn soft_delete(&self, public_id: &str, ctx: RequestContext) -> Result<bool, AppError> {
        debug!(public_id = %public_id, "soft-deleting jwt key");

        let result = sqlx::query(
            "UPDATE jwt_keys SET deleted_at = NOW(), updated_by = $1
             WHERE public_id = $2 AND deleted_at IS NULL",
        )
        .bind(ctx.created_by)
        .bind(public_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Creates the new rotated key and sets grace_period_ends_at on the old key.
    pub async fn rotate(
        &self,
        old_public_id: &str,
        tenant_id: Uuid,
        application_id: Option<String>,
        name: String,
        key_use: JwtKeyUse,
        algorithm: JwtAlgorithm,
        public_key: Option<String>,
        private_key_enc: Option<String>,
        secret_enc: Option<String>,
        expires_at: Option<chrono::DateTime<chrono::Utc>>,
        grace_period_hours: i64,
        ctx: RequestContext,
    ) -> Result<JwtKey, AppError> {
        use chrono::Duration;

        let grace_ends = chrono::Utc::now() + Duration::hours(grace_period_hours);

        // Mark old key with grace period end time and record rotation timestamp
        sqlx::query(
            r#"
            UPDATE jwt_keys SET
                grace_period_ends_at = $1,
                last_rotated_at      = NOW(),
                updated_by           = $2,
                version              = version + 1,
                updated_at           = NOW()
            WHERE public_id = $3 AND deleted_at IS NULL
            "#,
        )
        .bind(grace_ends)
        .bind(ctx.created_by)
        .bind(old_public_id)
        .execute(&self.pool)
        .await?;

        // Create the new key, linking to the old one
        let id = Uuid::now_v7();
        let public_id = JwtKey::new_public_id();
        let key_id = format!("kid_{}", NanoId::new());

        let new_key = sqlx::query_as::<_, JwtKey>(&format!(
            r#"
            INSERT INTO jwt_keys (
                id, public_id, tenant_id, application_id, name,
                key_use, algorithm, key_id, status,
                public_key, private_key_enc, secret_enc,
                expires_at, rotated_from_id, last_rotated_at,
                version, created_by, updated_by
            ) VALUES (
                $1, $2, $3, $4, $5,
                $6, $7, $8, 'active',
                $9, $10, $11,
                $12, $13, NOW(),
                0, $14, $14
            )
            RETURNING {SELECT_COLS}
            "#
        ))
        .bind(id)
        .bind(&public_id)
        .bind(tenant_id)
        .bind(&application_id)
        .bind(&name)
        .bind(&key_use)
        .bind(&algorithm)
        .bind(&key_id)
        .bind(&public_key)
        .bind(&private_key_enc)
        .bind(&secret_enc)
        .bind(expires_at)
        .bind(old_public_id)
        .bind(ctx.created_by)
        .fetch_one(&self.pool)
        .await?;

        Ok(new_key)
    }

    /// Fetches active keys for JWKS endpoint (all `authentication` keys for a tenant
    /// that are active and not yet deleted).
    pub async fn list_active_for_jwks(
        &self,
        tenant_id: Uuid,
    ) -> Result<Vec<JwtKey>, AppError> {
        debug!(tenant_id = %tenant_id, "fetching active keys for JWKS");

        let keys = sqlx::query_as::<_, JwtKey>(&format!(
            r#"
            SELECT {SELECT_COLS} FROM jwt_keys
            WHERE tenant_id = $1
              AND key_use = 'authentication'
              AND status = 'active'
              AND deleted_at IS NULL
              AND (expires_at IS NULL OR expires_at > NOW())
            ORDER BY created_at DESC
            "#
        ))
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(keys)
    }

    /// Disables all keys whose grace period has ended.
    /// Called periodically by the background task.
    pub async fn expire_grace_period_keys(&self) -> Result<u64, AppError> {
        let result = sqlx::query(
            r#"
            UPDATE jwt_keys SET
                status     = 'disabled',
                updated_at = NOW()
            WHERE status = 'active'
              AND grace_period_ends_at IS NOT NULL
              AND grace_period_ends_at < NOW()
              AND deleted_at IS NULL
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }
}
