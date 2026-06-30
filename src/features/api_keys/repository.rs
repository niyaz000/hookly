use chrono::{DateTime, Utc};
use sqlx::{QueryBuilder};
use tracing::debug;
use uuid::Uuid;

use crate::common::{types::RequestContext, NanoId};
use crate::error::AppError;

use super::models::{
    ApiKey, ApiKeySettings, ApiKeyStatus, InsertAuditParams,
    UpdateApiKeySettingsRequest, UpsertApiKeySettingsRequest,
};

// Columns for SELECT queries (with JOIN aliases for public IDs).
const SELECT_API_KEY_COLS: &str = "
    ak.id, ak.public_id, ak.organization_id, ak.tenant_id, ak.user_id,
    o.public_id AS organization_public_id,
    t.public_id AS tenant_public_id,
    u.public_id AS user_public_id,
    ak.name, ak.description, ak.key_hash, ak.key_encrypted, ak.key_prefix,
    ak.environment_id, ak.status, ak.expires_at, ak.last_used_at,
    ak.version, ak.created_by, ak.updated_by, ak.created_at, ak.updated_at, ak.deleted_at,
    creator.public_id AS created_by_public_id,
    updater.public_id AS updated_by_public_id
";

// Columns returned by INSERT/UPDATE RETURNING (no JOIN available).
const RETURNING_API_KEY_COLS: &str = "
    id, public_id, organization_id, tenant_id, user_id,
    name, description, key_hash, key_encrypted, key_prefix,
    environment_id, status, expires_at, last_used_at,
    version, created_by, updated_by, created_at, updated_at, deleted_at
";

const SELECT_SETTINGS_COLS: &str = "
    s.id, s.public_id, s.organization_id, s.tenant_id,
    o.public_id AS organization_public_id,
    t.public_id AS tenant_public_id,
    s.max_keys_per_user, s.key_length, s.default_ttl_seconds, s.allow_view_later,
    s.version, s.created_by, s.updated_by, s.created_at, s.updated_at
";

const RETURNING_SETTINGS_COLS: &str = "
    id, public_id, organization_id, tenant_id,
    max_keys_per_user, key_length, default_ttl_seconds, allow_view_later,
    version, created_by, updated_by, created_at, updated_at
";

pub struct ApiKeyRepository {
    pool: crate::common::CountingPool,
}

impl ApiKeyRepository {
    pub fn new(pool: crate::common::CountingPool) -> Self {
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
            WITH ins AS (
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
                RETURNING {RETURNING_API_KEY_COLS}
            )
            SELECT {SELECT_API_KEY_COLS}
            FROM ins ak
            JOIN organizations o      ON o.id = ak.organization_id
            JOIN tenants t            ON t.id = ak.tenant_id
            JOIN identity.users u     ON u.id = ak.user_id
            LEFT JOIN identity.users creator ON creator.id = ak.created_by
            LEFT JOIN identity.users updater ON updater.id = ak.updated_by
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
            r#"SELECT {SELECT_API_KEY_COLS}
            FROM api_keys ak
            JOIN organizations o  ON o.id = ak.organization_id
            JOIN tenants t        ON t.id = ak.tenant_id
            JOIN identity.users u ON u.id = ak.user_id
            LEFT JOIN identity.users creator ON creator.id = ak.created_by
            LEFT JOIN identity.users updater ON updater.id = ak.updated_by
            WHERE ak.public_id = $1 AND ak.deleted_at IS NULL"#
        ))
        .bind(public_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(key)
    }

    pub async fn get_by_hash(&self, key_hash: &str) -> Result<Option<ApiKey>, AppError> {
        debug!("querying api key by hash");

        let key = sqlx::query_as::<_, ApiKey>(&format!(
            r#"SELECT {SELECT_API_KEY_COLS}
            FROM api_keys ak
            JOIN organizations o  ON o.id = ak.organization_id
            JOIN tenants t        ON t.id = ak.tenant_id
            JOIN identity.users u ON u.id = ak.user_id
            LEFT JOIN identity.users creator ON creator.id = ak.created_by
            LEFT JOIN identity.users updater ON updater.id = ak.updated_by
            WHERE ak.key_hash = $1"#
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
            r#"SELECT {cols}
            FROM api_keys ak
            JOIN organizations o  ON o.id = ak.organization_id
            JOIN tenants t        ON t.id = ak.tenant_id
            JOIN identity.users u ON u.id = ak.user_id
            LEFT JOIN identity.users creator ON creator.id = ak.created_by
            LEFT JOIN identity.users updater ON updater.id = ak.updated_by
            WHERE ak.tenant_id = "#
        ));
        qb.push_bind(tenant_id);
        qb.push(" AND ak.deleted_at IS NULL");

        if let Some(uid) = user_id {
            qb.push(" AND ak.user_id = ").push_bind(uid);
        }
        if let Some(env_id) = environment_id {
            qb.push(" AND ak.environment_id = ").push_bind(env_id);
        }
        if let Some(st) = status {
            qb.push(" AND ak.status = ").push_bind(st);
        }
        if let Some(tags_val) = tags {
            qb.push(" AND ak.tags @> ").push_bind(tags_val);
        }
        if let Some(cursor_id) = cursor {
            qb.push(" AND ak.id > ").push_bind(cursor_id);
        }

        qb.push(" ORDER BY ak.id ASC LIMIT ").push_bind(limit + 1);

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
            WITH upd AS (
                UPDATE api_keys SET
                    description = COALESCE($1, description),
                    expires_at  = COALESCE($2, expires_at),
                    updated_by  = $3,
                    request_id  = $4,
                    version     = version + 1,
                    updated_at  = NOW()
                WHERE public_id = $5 AND deleted_at IS NULL
                RETURNING {RETURNING_API_KEY_COLS}
            )
            SELECT {SELECT_API_KEY_COLS}
            FROM upd ak
            JOIN organizations o  ON o.id = ak.organization_id
            JOIN tenants t        ON t.id = ak.tenant_id
            JOIN identity.users u ON u.id = ak.user_id
            LEFT JOIN identity.users creator ON creator.id = ak.created_by
            LEFT JOIN identity.users updater ON updater.id = ak.updated_by
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
            r#"SELECT {SELECT_SETTINGS_COLS}
            FROM api_key_settings s
            JOIN organizations o ON o.id = s.organization_id
            JOIN tenants t       ON t.id = s.tenant_id
            WHERE s.tenant_id = $1"#
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
            r#"SELECT {SELECT_SETTINGS_COLS}
            FROM api_key_settings s
            JOIN organizations o ON o.id = s.organization_id
            JOIN tenants t       ON t.id = s.tenant_id
            WHERE s.public_id = $1"#
        ))
        .bind(public_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(settings)
    }

    pub async fn upsert_settings(
        &self,
        organization_id: Uuid,
        tenant_id: Uuid,
        req: &UpsertApiKeySettingsRequest,
        ctx: RequestContext,
    ) -> Result<ApiKeySettings, AppError> {
        let id = Uuid::now_v7();
        let public_id = format!("aks_{}", NanoId::new());

        debug!(
            organization_id = %organization_id,
            tenant_id = %tenant_id,
            "upserting api key settings"
        );

        let settings = sqlx::query_as::<_, ApiKeySettings>(&format!(
            r#"
            WITH ups AS (
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
                RETURNING {RETURNING_SETTINGS_COLS}
            )
            SELECT {SELECT_SETTINGS_COLS}
            FROM ups s
            JOIN organizations o ON o.id = s.organization_id
            JOIN tenants t       ON t.id = s.tenant_id
            "#
        ))
        .bind(id)
        .bind(&public_id)
        .bind(organization_id)
        .bind(tenant_id)
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
            WITH upd AS (
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
                RETURNING {RETURNING_SETTINGS_COLS}
            )
            SELECT {SELECT_SETTINGS_COLS}
            FROM upd s
            JOIN organizations o ON o.id = s.organization_id
            JOIN tenants t       ON t.id = s.tenant_id
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
