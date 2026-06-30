use std::collections::HashMap;

use chrono::{DateTime, Utc};
use sqlx::{types::Json};
use uuid::Uuid;

use crate::common::{nano_id::NanoId, types::RequestContext};
use crate::error::AppError;
use crate::features::endpoints::models::{
    EndpointRow, ListQueryParams, SecretRow, UpdateEndpointRequest,
};

// SELECT fragment used by all single-row and list queries.
const BASE_SELECT: &str = r#"
    SELECT
        e.id,
        e.public_id,
        e.application_id,
        a.public_id AS application_public_id,
        e.tenant_id,
        e.organization_id,
        e.description,
        e.endpoint_type,
        e.config,
        e.event_types,
        e.status,
        e.rate_limit_per_minute,
        e.tags,
        e.version,
        e.request_id,
        e.created_by,
        e.updated_by,
        e.created_at,
        e.updated_at
    FROM endpoints e
    JOIN applications a ON a.id = e.application_id
"#;

// Resolved application identity, used when creating an endpoint.
#[derive(sqlx::FromRow, Debug)]
pub struct ApplicationRef {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub organization_id: Uuid,
}

// Minimal endpoint identity used for meta-only lookups (secret ops).
#[derive(sqlx::FromRow, Debug)]
pub struct EndpointMeta {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub organization_id: Uuid,
}

pub struct EndpointRepository {
    db: crate::common::CountingPool,
}

impl EndpointRepository {
    pub fn new(db: crate::common::CountingPool) -> Self {
        Self { db }
    }

    /// Looks up an application by its public_id for endpoint creation.
    pub async fn get_application(
        &self,
        public_id: &str,
    ) -> Result<Option<ApplicationRef>, AppError> {
        sqlx::query_as::<_, ApplicationRef>(
            "SELECT id, tenant_id, organization_id FROM applications \
             WHERE public_id = $1 AND deleted_at IS NULL",
        )
        .bind(public_id)
        .fetch_optional(&self.db)
        .await
        .map_err(AppError::from)
    }

    /// Returns minimal identity fields for an endpoint (for secret operations).
    pub async fn get_meta(&self, public_id: &str) -> Result<Option<EndpointMeta>, AppError> {
        sqlx::query_as::<_, EndpointMeta>(
            "SELECT id, tenant_id, organization_id FROM endpoints \
             WHERE public_id = $1 AND deleted_at IS NULL",
        )
        .bind(public_id)
        .fetch_optional(&self.db)
        .await
        .map_err(AppError::from)
    }

    /// Creates an endpoint and its initial signing secret in a single transaction.
    #[allow(clippy::too_many_arguments)]
    pub async fn create(
        &self,
        app: ApplicationRef,
        endpoint_type: &str,
        description: Option<&str>,
        config: &serde_json::Value,
        event_types: &[String],
        rate_limit_per_minute: Option<i32>,
        tags: &HashMap<String, String>,
        encrypted_secret: &str,
        ctx: RequestContext,
    ) -> Result<EndpointRow, AppError> {
        let ep_public_id = format!("ep_{}", NanoId::new());
        let sec_public_id = format!("sec_{}", NanoId::new());

        let mut tx = self.db.begin().await?;

        sqlx::query(
            r#"INSERT INTO endpoints
               (public_id, application_id, tenant_id, organization_id,
                description, endpoint_type, config, event_types,
                rate_limit_per_minute, tags, version, request_id, created_by, updated_by)
               VALUES ($1, $2, $3, $4, $5, $6, $7::jsonb, $8, $9, $10::jsonb, 0, $11, $12, $12)"#,
        )
        .bind(&ep_public_id)
        .bind(app.id)
        .bind(app.tenant_id)
        .bind(app.organization_id)
        .bind(description)
        .bind(endpoint_type)
        .bind(Json(config))
        .bind(event_types)
        .bind(rate_limit_per_minute)
        .bind(Json(tags))
        .bind(ctx.request_id)
        .bind(ctx.created_by)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"INSERT INTO endpoint_secrets
               (public_id, endpoint_id, tenant_id, organization_id,
                secret, is_active, request_id, created_by)
               SELECT $1, e.id, $3, $4, $5, TRUE, $6, $7
               FROM endpoints e WHERE e.public_id = $2"#,
        )
        .bind(&sec_public_id)
        .bind(&ep_public_id)
        .bind(app.tenant_id)
        .bind(app.organization_id)
        .bind(encrypted_secret)
        .bind(ctx.request_id)
        .bind(ctx.created_by)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        self.get_by_id(&ep_public_id)
            .await?
            .ok_or_else(|| AppError::Internal("endpoint created but not found on fetch".into()))
    }

    /// Returns a single non-deleted endpoint by its public_id.
    pub async fn get_by_id(&self, public_id: &str) -> Result<Option<EndpointRow>, AppError> {
        let sql = format!(
            "{} WHERE e.public_id = $1 AND e.deleted_at IS NULL",
            BASE_SELECT
        );
        sqlx::query_as::<_, EndpointRow>(&sql)
            .bind(public_id)
            .fetch_optional(&self.db)
            .await
            .map_err(AppError::from)
    }

    /// Returns a paginated list of non-deleted endpoints for an application.
    pub async fn list(&self, filter: ListQueryParams) -> Result<(Vec<EndpointRow>, i64), AppError> {
        #[derive(sqlx::FromRow)]
        struct Row {
            id: Uuid,
            public_id: String,
            application_id: Uuid,
            application_public_id: String,
            tenant_id: Uuid,
            organization_id: Uuid,
            description: Option<String>,
            endpoint_type: String,
            config: Json<serde_json::Value>,
            event_types: Vec<String>,
            status: String,
            rate_limit_per_minute: Option<i32>,
            tags: Json<HashMap<String, String>>,
            version: i32,
            request_id: Uuid,
            created_by: Uuid,
            updated_by: Uuid,
            created_at: DateTime<Utc>,
            updated_at: DateTime<Utc>,
            total_count: i64,
        }

        let limit = filter.limit.min(250) as i64;
        let offset = (filter.page.saturating_sub(1)) as i64 * limit;

        let rows = sqlx::query_as::<_, Row>(
            r#"SELECT
                e.id, e.public_id,
                e.application_id, a.public_id AS application_public_id,
                e.tenant_id, e.organization_id,
                e.description, e.endpoint_type,
                e.config, e.event_types,
                e.status, e.rate_limit_per_minute, e.tags,
                e.version, e.request_id,
                e.created_by, e.updated_by,
                e.created_at, e.updated_at,
                COUNT(*) OVER() AS total_count
               FROM endpoints e
               JOIN applications a ON a.id = e.application_id
               WHERE e.application_id = (
                   SELECT id FROM applications WHERE public_id = $1 AND deleted_at IS NULL
               )
                 AND e.deleted_at IS NULL
                 AND ($2::text IS NULL OR e.status = $2)
                 AND ($3::text IS NULL OR e.endpoint_type = $3)
               ORDER BY e.created_at DESC
               LIMIT $4 OFFSET $5"#,
        )
        .bind(&filter.application_id)
        .bind(&filter.status)
        .bind(&filter.endpoint_type)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.db)
        .await?;

        let total = rows.first().map(|r| r.total_count).unwrap_or(0);
        let endpoints = rows
            .into_iter()
            .map(|r| EndpointRow {
                id: r.id,
                public_id: r.public_id,
                application_id: r.application_id,
                application_public_id: r.application_public_id,
                tenant_id: r.tenant_id,
                organization_id: r.organization_id,
                description: r.description,
                endpoint_type: r.endpoint_type,
                config: r.config,
                event_types: r.event_types,
                status: r.status,
                rate_limit_per_minute: r.rate_limit_per_minute,
                tags: r.tags,
                version: r.version,
                request_id: r.request_id,
                created_by: r.created_by,
                updated_by: r.updated_by,
                created_at: r.created_at,
                updated_at: r.updated_at,
            })
            .collect();

        Ok((endpoints, total))
    }

    /// Partially updates an endpoint with optimistic locking.
    /// Returns None if no row matched (either not found or version mismatch).
    pub async fn update(
        &self,
        public_id: &str,
        req: UpdateEndpointRequest,
        ctx: RequestContext,
    ) -> Result<Option<EndpointRow>, AppError> {
        let update_description = req.description.is_some();
        let description = req.description;

        let update_config = req.config.is_some();
        let config_val = req.config.unwrap_or(serde_json::Value::Null);

        let update_event_types = req.event_types.is_some();
        let event_types = req.event_types.unwrap_or_default();

        let update_rate_limit = req.rate_limit_per_minute.is_some();
        let new_rate_limit = req.rate_limit_per_minute.flatten();

        let update_tags = req.tags.is_some();
        let tags_val = req.tags.unwrap_or_default();

        let id: Option<Uuid> = sqlx::query_scalar(
            r#"UPDATE endpoints SET
                description           = CASE WHEN $1  THEN $2           ELSE description           END,
                config                = CASE WHEN $3  THEN $4::jsonb    ELSE config                END,
                event_types           = CASE WHEN $5  THEN $6::text[]   ELSE event_types           END,
                rate_limit_per_minute = CASE WHEN $7  THEN $8           ELSE rate_limit_per_minute END,
                tags                  = CASE WHEN $9  THEN $10::jsonb   ELSE tags                  END,
                updated_by  = $11,
                request_id  = $12,
                updated_at  = NOW(),
                version     = version + 1
               WHERE public_id = $13
                 AND version   = $14
                 AND deleted_at IS NULL
               RETURNING id"#,
        )
        .bind(update_description)
        .bind(&description)
        .bind(update_config)
        .bind(Json(&config_val))
        .bind(update_event_types)
        .bind(&event_types)
        .bind(update_rate_limit)
        .bind(new_rate_limit)
        .bind(update_tags)
        .bind(Json(&tags_val))
        .bind(ctx.created_by)
        .bind(ctx.request_id)
        .bind(public_id)
        .bind(req.version)
        .fetch_optional(&self.db)
        .await?;

        match id {
            Some(_) => self.get_by_id(public_id).await,
            None => Ok(None),
        }
    }

    /// Soft-deletes an endpoint (idempotent).
    pub async fn delete(&self, public_id: &str, ctx: RequestContext) -> Result<(), AppError> {
        sqlx::query(
            r#"UPDATE endpoints SET
                deleted_at = NOW(),
                updated_by = $1,
                request_id = $2,
                updated_at = NOW(),
                version    = version + 1
               WHERE public_id = $3 AND deleted_at IS NULL"#,
        )
        .bind(ctx.created_by)
        .bind(ctx.request_id)
        .bind(public_id)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    /// Sets endpoint status (active / paused). Returns None if not found.
    pub async fn set_status(
        &self,
        public_id: &str,
        status: &str,
        ctx: RequestContext,
    ) -> Result<Option<EndpointRow>, AppError> {
        let id: Option<Uuid> = sqlx::query_scalar(
            r#"UPDATE endpoints SET
                status     = $1,
                updated_by = $2,
                request_id = $3,
                updated_at = NOW(),
                version    = version + 1
               WHERE public_id = $4 AND deleted_at IS NULL
               RETURNING id"#,
        )
        .bind(status)
        .bind(ctx.created_by)
        .bind(ctx.request_id)
        .bind(public_id)
        .fetch_optional(&self.db)
        .await?;

        match id {
            Some(_) => self.get_by_id(public_id).await,
            None => Ok(None),
        }
    }

    /// Returns the most recently created active (and not expired) signing secret for an endpoint.
    pub async fn get_active_secret(
        &self,
        ep_public_id: &str,
    ) -> Result<Option<SecretRow>, AppError> {
        sqlx::query_as::<_, SecretRow>(
            r#"SELECT
                es.id, es.public_id, es.endpoint_id,
                es.tenant_id, es.organization_id,
                es.secret, es.is_active, es.expires_at,
                es.request_id, es.created_by, es.created_at
               FROM endpoint_secrets es
               JOIN endpoints e ON e.id = es.endpoint_id
               WHERE e.public_id    = $1
                 AND e.deleted_at   IS NULL
                 AND es.is_active   = TRUE
                 AND (es.expires_at IS NULL OR es.expires_at > NOW())
               ORDER BY es.created_at DESC
               LIMIT 1"#,
        )
        .bind(ep_public_id)
        .fetch_optional(&self.db)
        .await
        .map_err(AppError::from)
    }

    /// Rotates the signing secret for an endpoint.
    ///
    /// - `expiry_seconds = 0`: old secret is immediately deactivated.
    /// - `expiry_seconds > 0`: old secret remains active until `NOW() + expiry_seconds`.
    ///
    /// Returns None if the endpoint does not exist.
    pub async fn rotate_secret(
        &self,
        ep_public_id: &str,
        encrypted_secret: &str,
        expiry_seconds: u32,
        ctx: RequestContext,
    ) -> Result<Option<SecretRow>, AppError> {
        let mut tx = self.db.begin().await?;

        let meta = sqlx::query_as::<_, EndpointMeta>(
            "SELECT id, tenant_id, organization_id FROM endpoints \
             WHERE public_id = $1 AND deleted_at IS NULL",
        )
        .bind(ep_public_id)
        .fetch_optional(&mut *tx)
        .await?;

        let meta = match meta {
            Some(m) => m,
            None => {
                tx.rollback().await.ok();
                return Ok(None);
            }
        };

        // Deactivate current active secrets (or set grace-period expiry).
        sqlx::query(
            r#"UPDATE endpoint_secrets SET
                is_active  = CASE WHEN $2 = 0 THEN FALSE ELSE is_active END,
                expires_at = CASE WHEN $2 > 0
                                  THEN NOW() + ($2::bigint * INTERVAL '1 second')
                                  ELSE expires_at END
               WHERE endpoint_id = $1
                 AND is_active = TRUE
                 AND (expires_at IS NULL OR expires_at > NOW())"#,
        )
        .bind(meta.id)
        .bind(expiry_seconds as i64)
        .execute(&mut *tx)
        .await?;

        let sec_public_id = format!("sec_{}", NanoId::new());
        let secret: SecretRow = sqlx::query_as(
            r#"INSERT INTO endpoint_secrets
               (public_id, endpoint_id, tenant_id, organization_id,
                secret, is_active, request_id, created_by)
               VALUES ($1, $2, $3, $4, $5, TRUE, $6, $7)
               RETURNING id, public_id, endpoint_id, tenant_id, organization_id,
                         secret, is_active, expires_at, request_id, created_by, created_at"#,
        )
        .bind(&sec_public_id)
        .bind(meta.id)
        .bind(meta.tenant_id)
        .bind(meta.organization_id)
        .bind(encrypted_secret)
        .bind(ctx.request_id)
        .bind(ctx.created_by)
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(Some(secret))
    }
}
