use sqlx::{PgPool, QueryBuilder};
use tracing::debug;
use uuid::Uuid;

use crate::common::{NanoId, types::RequestContext};
use crate::error::AppError;

use super::models::{PlatformWebhook, PlatformWebhookStatus};

const SELECT_COLS: &str = "
    id, public_id, tenant_id, name, description, url,
    signing_secret_enc, status, metadata,
    created_by, updated_by, created_at, updated_at
";

const MAX_WEBHOOKS_PER_TENANT: i64 = 10;

#[derive(Clone)]
pub struct PlatformWebhookRepository {
    pool: PgPool,
}

impl PlatformWebhookRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn count_active_for_tenant(&self, tenant_id: Uuid) -> Result<i64, AppError> {
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM platform_webhooks
             WHERE tenant_id = $1 AND deleted_at IS NULL AND status != 'disabled'",
        )
        .bind(tenant_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.0)
    }

    pub async fn create(
        &self,
        tenant_id: Uuid,
        name: String,
        description: Option<String>,
        url: String,
        signing_secret_enc: String,
        metadata: serde_json::Value,
        ctx: RequestContext,
    ) -> Result<PlatformWebhook, AppError> {
        let id = Uuid::now_v7();
        let public_id = format!("pwh_{}", NanoId::new());

        debug!(public_id = %public_id, tenant_id = %tenant_id, "inserting platform webhook");

        let webhook = sqlx::query_as::<_, PlatformWebhook>(&format!(
            r#"
            INSERT INTO platform_webhooks (
                id, public_id, tenant_id, name, description,
                url, signing_secret_enc, metadata, created_by, updated_by
            ) VALUES (
                $1, $2, $3, $4, $5,
                $6, $7, $8, $9, $9
            )
            RETURNING {SELECT_COLS}
            "#
        ))
        .bind(id)
        .bind(&public_id)
        .bind(tenant_id)
        .bind(&name)
        .bind(&description)
        .bind(&url)
        .bind(&signing_secret_enc)
        .bind(sqlx::types::Json(metadata))
        .bind(ctx.created_by)
        .fetch_one(&self.pool)
        .await?;

        Ok(webhook)
    }

    pub async fn get_by_public_id(&self, public_id: &str) -> Result<Option<PlatformWebhook>, AppError> {
        debug!(public_id = %public_id, "querying platform webhook");
        let webhook = sqlx::query_as::<_, PlatformWebhook>(&format!(
            "SELECT {SELECT_COLS} FROM platform_webhooks WHERE public_id = $1 AND deleted_at IS NULL"
        ))
        .bind(public_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(webhook)
    }

    pub async fn list(
        &self,
        tenant_id: Option<Uuid>,
        status: Option<PlatformWebhookStatus>,
        limit: i64,
        cursor: Option<String>,
    ) -> Result<(Vec<PlatformWebhook>, Option<String>), AppError> {
        debug!(tenant_id = ?tenant_id, limit = limit, "listing platform webhooks");
        let cols = SELECT_COLS;
        let mut qb = QueryBuilder::<sqlx::Postgres>::new(format!(
            "SELECT {cols} FROM platform_webhooks WHERE deleted_at IS NULL"
        ));
        if let Some(tid) = tenant_id {
            qb.push(" AND tenant_id = ").push_bind(tid);
        }
        if let Some(st) = status {
            qb.push(" AND status = ").push_bind(st);
        }
        if let Some(ref c) = cursor {
            qb.push(" AND public_id > ").push_bind(c.clone());
        }
        qb.push(" ORDER BY public_id ASC LIMIT ").push_bind(limit + 1);

        let mut webhooks: Vec<PlatformWebhook> =
            qb.build_query_as::<PlatformWebhook>().fetch_all(&self.pool).await?;

        let next_cursor = if webhooks.len() as i64 > limit {
            webhooks.pop().map(|w| w.public_id)
        } else {
            None
        };

        Ok((webhooks, next_cursor))
    }

    pub async fn update(
        &self,
        public_id: &str,
        name: Option<String>,
        description: Option<String>,
        url: Option<String>,
        metadata: Option<serde_json::Value>,
        ctx: RequestContext,
    ) -> Result<Option<PlatformWebhook>, AppError> {
        debug!(public_id = %public_id, "updating platform webhook");
        let webhook = sqlx::query_as::<_, PlatformWebhook>(&format!(
            r#"
            UPDATE platform_webhooks SET
                name        = COALESCE($1, name),
                description = COALESCE($2, description),
                url         = COALESCE($3, url),
                metadata    = COALESCE($4, metadata),
                updated_by  = $5,
                updated_at  = NOW()
            WHERE public_id = $6 AND deleted_at IS NULL
            RETURNING {SELECT_COLS}
            "#
        ))
        .bind(&name)
        .bind(&description)
        .bind(&url)
        .bind(metadata.map(sqlx::types::Json))
        .bind(ctx.created_by)
        .bind(public_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(webhook)
    }

    pub async fn set_status(
        &self,
        public_id: &str,
        status: PlatformWebhookStatus,
        ctx: RequestContext,
    ) -> Result<Option<PlatformWebhook>, AppError> {
        debug!(public_id = %public_id, "updating platform webhook status");
        let webhook = sqlx::query_as::<_, PlatformWebhook>(&format!(
            r#"
            UPDATE platform_webhooks SET
                status     = $1,
                updated_by = $2,
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
        Ok(webhook)
    }

    pub async fn rotate_secret(
        &self,
        public_id: &str,
        new_signing_secret_enc: String,
        ctx: RequestContext,
    ) -> Result<Option<PlatformWebhook>, AppError> {
        debug!(public_id = %public_id, "rotating platform webhook signing secret");
        let webhook = sqlx::query_as::<_, PlatformWebhook>(&format!(
            r#"
            UPDATE platform_webhooks SET
                signing_secret_enc = $1,
                updated_by         = $2,
                updated_at         = NOW()
            WHERE public_id = $3 AND deleted_at IS NULL
            RETURNING {SELECT_COLS}
            "#
        ))
        .bind(&new_signing_secret_enc)
        .bind(ctx.created_by)
        .bind(public_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(webhook)
    }

    pub async fn soft_delete(&self, public_id: &str, ctx: RequestContext) -> Result<bool, AppError> {
        debug!(public_id = %public_id, "soft-deleting platform webhook");
        let result = sqlx::query(
            "UPDATE platform_webhooks SET deleted_at = NOW(), updated_by = $1
             WHERE public_id = $2 AND deleted_at IS NULL",
        )
        .bind(ctx.created_by)
        .bind(public_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub fn max_per_tenant() -> i64 {
        MAX_WEBHOOKS_PER_TENANT
    }
}
