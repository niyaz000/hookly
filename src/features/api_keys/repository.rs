use chrono::{DateTime, Utc};
use sqlx::{PgPool, QueryBuilder};
use tracing::debug;
use uuid::Uuid;

use crate::common::{types::RequestContext, NanoId};
use crate::error::AppError;

use super::models::{
    ApiKey, ApiKeySettings, ApiKeyStatus, InsertAuditParams,
    UpdateApiKeySettingsRequest, UpsertApiKeySettingsRequest,
};

const SELECT_API_KEY_COLS: &str = "
    id, public_id, organization_id, tenant_id, user_id,
    name, description, key_hash, key_encrypted, key_prefix,
    environment_id, status, expires_at, last_used_at,
    version, created_by, updated_by, created_at, updated_at, deleted_at
";

const SELECT_SETTINGS_COLS: &str = "
    id, public_id, organization_id, tenant_id,
    max_keys_per_user, key_length, default_ttl_seconds, allow_view_later,
    version, created_by, updated_by, created_at, updated_at
";

pub struct ApiKeyRepository {
    pool: PgPool,
}

impl ApiKeyRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(
        &self,
        organization_id: Uuid,
        tenant_id: Uuid,
        user_id: Uuid,
        name: String,
        description: Option<String>,
        key_hash: String,
        key_encrypted: Option<String>,
        key_prefix: String,
        environment_id: String,
        expires_at: Option<DateTime<Utc>>,
        ctx: RequestContext,
    ) -> Result<ApiKey, AppError> {
        let id = Uuid::now_v7();
        let public_id = format!("key_{}", NanoId::new());

        debug!(
            public_id = %public_id,
            tenant_id = %tenant_id,
            user_id = %user_id,
            environment_id = %environment_id,
            "inserting api key"
        );

        let key = sqlx::query_as::<_, ApiKey>(&format!(
            r#"
            INSERT INTO api_keys (
                id, public_id, organization_id, tenant_id, user_id,
                name, description, key_hash, key_encrypted, key_prefix,
                environment_id, expires_at,
                request_id, version, created_by, updated_by, created_at, updated_at
            ) VALUES (
                $1, $2, $3, $4, $5,
                $6, $7, $8, $9, $10,
                $11, $12,
                $13, 0, $14, $14, NOW(), NOW()
            )
            RETURNING {SELECT_API_KEY_COLS}
            "#
        ))
        .bind(id)
        .bind(&public_id)
        .bind(organization_id)
        .bind(tenant_id)
        .bind(user_id)
        .bind(&name)
        .bind(&description)
        .bind(&key_hash)
        .bind(&key_encrypted)
        .bind(&key_prefix)
        .bind(&environment_id)
        .bind(expires_at)
        .bind(ctx.request_id)
        .bind(ctx.created_by)
        .fetch_one(&self.pool)
        .await?;

        Ok(key)
    }

    pub async fn get_by_public_id(&self, public_id: &str) -> Result<Option<ApiKey>, AppError> {
        debug!(public_id = %public_id, "querying api key by public_id");

        let key = sqlx::query_as::<_, ApiKey>(&format!(
            "SELECT {SELECT_API_KEY_COLS} FROM api_keys WHERE public_id = $1 AND deleted_at IS NULL"
        ))
        .bind(public_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(key)
    }

    pub async fn get_by_hash(&self, key_hash: &str) -> Result<Option<ApiKey>, AppError> {
        debug!("querying api key by hash");

        let key = sqlx::query_as::<_, ApiKey>(&format!(
            "SELECT {SELECT_API_KEY_COLS} FROM api_keys WHERE key_hash = $1"
        ))
        .bind(key_hash)
        .fetch_optional(&self.pool)
        .await?;

        Ok(key)
    }

    pub async fn list(
        &self,
        tenant_id: Uuid,
        user_id: Option<Uuid>,
        environment_id: Option<String>,
        status: Option<ApiKeyStatus>,
        limit: i64,
        cursor: Option<Uuid>,
        tags: Option<serde_json::Value>,
    ) -> Result<(Vec<ApiKey>, Option<Uuid>), AppError> {
        debug!(
            tenant_id = %tenant_id,
            limit = limit,
            "listing api keys"
        );

        let cols = SELECT_API_KEY_COLS;
        let mut qb = QueryBuilder::<sqlx::Postgres>::new(format!(
            "SELECT {cols} FROM api_keys WHERE tenant_id = "
        ));
        qb.push_bind(tenant_id);
        qb.push(" AND deleted_at IS NULL");

        if let Some(uid) = user_id {
            qb.push(" AND user_id = ").push_bind(uid);
        }
        if let Some(env_id) = environment_id {
            qb.push(" AND environment_id = ").push_bind(env_id);
        }
        if let Some(st) = status {
            qb.push(" AND status = ").push_bind(st);
        }
        if let Some(tags_val) = tags {
            qb.push(" AND tags @> ").push_bind(tags_val);
        }
        if let Some(cursor_id) = cursor {
            qb.push(" AND id > ").push_bind(cursor_id);
        }

        qb.push(" ORDER BY id ASC LIMIT ").push_bind(limit + 1);

        let mut keys: Vec<ApiKey> = qb
            .build_query_as::<ApiKey>()
            .fetch_all(&self.pool)
            .await?;

        let next_cursor = if keys.len() as i64 > limit {
            keys.pop().map(|k| k.id)
        } else {
            None
        };

        Ok((keys, next_cursor))
    }

    pub async fn update(
        &self,
        public_id: &str,
        description: Option<String>,
        expires_at: Option<DateTime<Utc>>,
        ctx: RequestContext,
    ) -> Result<Option<ApiKey>, AppError> {
        debug!(public_id = %public_id, "updating api key");

        let key = sqlx::query_as::<_, ApiKey>(&format!(
            r#"
            UPDATE api_keys SET
                description = COALESCE($1, description),
                expires_at  = COALESCE($2, expires_at),
                updated_by  = $3,
                request_id  = $4,
                version     = version + 1,
                updated_at  = NOW()
            WHERE public_id = $5 AND deleted_at IS NULL
            RETURNING {SELECT_API_KEY_COLS}
            "#
        ))
        .bind(description)
        .bind(expires_at)
        .bind(ctx.created_by)
        .bind(ctx.request_id)
        .bind(public_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(key)
    }

    pub async fn delete(&self, public_id: &str, ctx: RequestContext) -> Result<bool, AppError> {
        debug!(public_id = %public_id, "soft deleting api key");

        let result = sqlx::query(
            r#"
            UPDATE api_keys SET
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

    pub async fn count_active_for_user(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
    ) -> Result<i64, AppError> {
        debug!(
            tenant_id = %tenant_id,
            user_id = %user_id,
            "counting active api keys for user"
        );

        let count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*) FROM api_keys
            WHERE tenant_id = $1
              AND user_id   = $2
              AND deleted_at IS NULL
              AND status    = 'active'
            "#,
        )
        .bind(tenant_id)
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(count)
    }

    pub async fn get_settings_by_tenant(
        &self,
        tenant_id: Uuid,
    ) -> Result<Option<ApiKeySettings>, AppError> {
        debug!(tenant_id = %tenant_id, "querying api key settings by tenant");

        let settings = sqlx::query_as::<_, ApiKeySettings>(&format!(
            "SELECT {SELECT_SETTINGS_COLS} FROM api_key_settings WHERE tenant_id = $1"
        ))
        .bind(tenant_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(settings)
    }

    pub async fn get_settings_by_public_id(
        &self,
        public_id: &str,
    ) -> Result<Option<ApiKeySettings>, AppError> {
        debug!(public_id = %public_id, "querying api key settings by public_id");

        let settings = sqlx::query_as::<_, ApiKeySettings>(&format!(
            "SELECT {SELECT_SETTINGS_COLS} FROM api_key_settings WHERE public_id = $1"
        ))
        .bind(public_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(settings)
    }

    pub async fn upsert_settings(
        &self,
        req: &UpsertApiKeySettingsRequest,
        ctx: RequestContext,
    ) -> Result<ApiKeySettings, AppError> {
        let id = Uuid::now_v7();
        let public_id = format!("aks_{}", NanoId::new());

        debug!(
            organization_id = %req.organization_id,
            tenant_id = %req.tenant_id,
            "upserting api key settings"
        );

        let settings = sqlx::query_as::<_, ApiKeySettings>(&format!(
            r#"
            INSERT INTO api_key_settings (
                id, public_id, organization_id, tenant_id,
                max_keys_per_user, key_length, default_ttl_seconds, allow_view_later,
                request_id, version, created_by, updated_by, created_at, updated_at
            ) VALUES (
                $1, $2, $3, $4,
                $5, $6, $7, $8,
                $9, 0, $10, $10, NOW(), NOW()
            )
            ON CONFLICT (organization_id, tenant_id) DO UPDATE SET
                max_keys_per_user   = EXCLUDED.max_keys_per_user,
                key_length          = EXCLUDED.key_length,
                default_ttl_seconds = EXCLUDED.default_ttl_seconds,
                allow_view_later    = EXCLUDED.allow_view_later,
                request_id          = EXCLUDED.request_id,
                version             = api_key_settings.version + 1,
                updated_by          = EXCLUDED.updated_by,
                updated_at          = NOW()
            RETURNING {SELECT_SETTINGS_COLS}
            "#
        ))
        .bind(id)
        .bind(&public_id)
        .bind(req.organization_id)
        .bind(req.tenant_id)
        .bind(req.max_keys_per_user)
        .bind(req.key_length)
        .bind(req.default_ttl_seconds)
        .bind(req.allow_view_later)
        .bind(ctx.request_id)
        .bind(ctx.created_by)
        .fetch_one(&self.pool)
        .await?;

        Ok(settings)
    }

    pub async fn update_settings_by_public_id(
        &self,
        public_id: &str,
        req: &UpdateApiKeySettingsRequest,
        ctx: RequestContext,
    ) -> Result<Option<ApiKeySettings>, AppError> {
        debug!(public_id = %public_id, "updating api key settings");

        let settings = sqlx::query_as::<_, ApiKeySettings>(&format!(
            r#"
            UPDATE api_key_settings SET
                max_keys_per_user   = $1,
                key_length          = $2,
                default_ttl_seconds = $3,
                allow_view_later    = $4,
                request_id          = $5,
                version             = version + 1,
                updated_by          = $6,
                updated_at          = NOW()
            WHERE public_id = $7
            RETURNING {SELECT_SETTINGS_COLS}
            "#
        ))
        .bind(req.max_keys_per_user)
        .bind(req.key_length)
        .bind(req.default_ttl_seconds)
        .bind(req.allow_view_later)
        .bind(ctx.request_id)
        .bind(ctx.created_by)
        .bind(public_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(settings)
    }

    pub async fn insert_audit(&self, params: InsertAuditParams) -> Result<(), AppError> {
        debug!(
            api_key_public_id = %params.api_key_public_id,
            action = %params.action,
            "inserting api key audit record"
        );

        sqlx::query(
            r#"
            INSERT INTO api_key_audits (
                id, api_key_id, api_key_public_id,
                organization_id, tenant_id, user_id,
                action, actor_id, request_id, changes, created_at
            ) VALUES (
                gen_random_uuid(), $1, $2,
                $3, $4, $5,
                $6, $7, $8, $9, NOW()
            )
            "#,
        )
        .bind(params.api_key_id)
        .bind(&params.api_key_public_id)
        .bind(params.organization_id)
        .bind(params.tenant_id)
        .bind(params.user_id)
        .bind(params.action)
        .bind(params.actor_id)
        .bind(params.request_id)
        .bind(params.changes)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}
