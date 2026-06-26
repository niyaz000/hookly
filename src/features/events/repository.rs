use std::collections::HashMap;

use chrono::{DateTime, Utc};
use sqlx::{types::Json, PgPool};
use uuid::Uuid;

use crate::common::{nano_id::NanoId, types::RequestContext};
use crate::error::AppError;
use crate::features::events::models::{EventRow, ListQueryParams};

const BASE_SELECT: &str = r#"
    SELECT
        ev.id,
        ev.public_id,
        ev.application_id,
        a.public_id  AS application_public_id,
        ev.event_type_id,
        et.public_id AS event_type_public_id,
        et.name      AS event_type_name,
        ev.endpoint_id,
        ep.public_id AS endpoint_public_id,
        ev.tenant_id,
        ev.organization_id,
        ev.payload,
        ev.payload_type,
        ev.idempotency_key,
        ev.body_hash,
        ev.tags,
        ev.request_id,
        ev.created_by,
        ev.created_at,
        ev.schema_valid,
        ev.schema_errors
    FROM events ev
    JOIN  applications a  ON a.id  = ev.application_id
    JOIN  event_types  et ON et.id = ev.event_type_id
    LEFT JOIN endpoints ep ON ep.id = ev.endpoint_id
"#;

#[derive(sqlx::FromRow, Debug)]
pub struct ApplicationRef {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub organization_id: Uuid,
}

#[derive(sqlx::FromRow, Debug)]
pub struct EventTypeRef {
    pub id: Uuid,
    pub event_schema: sqlx::types::Json<crate::features::event_types::models::PropertyDef>,
    pub schema_version: String,
}

pub struct EventRepository {
    db: PgPool,
}

impl EventRepository {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }

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

    /// Resolves an event type by public_id, scoped to the tenant.
    ///
    /// If `schema_version` is `Some`, finds that specific version within the same name-family
    /// as `public_id`. If `None`, returns the latest version (highest `schema_version`) in
    /// that family. Returns `None` if the referenced type doesn't exist or is archived.
    pub async fn get_event_type(
        &self,
        public_id: &str,
        tenant_id: Uuid,
        schema_version: Option<&str>,
    ) -> Result<Option<EventTypeRef>, AppError> {
        sqlx::query_as::<_, EventTypeRef>(
            "SELECT id, event_schema, schema_version \
             FROM event_types \
             WHERE name = (SELECT name FROM event_types WHERE public_id = $1 AND deleted_at IS NULL) \
               AND tenant_id = $2 \
               AND ($3::text IS NULL OR schema_version = $3) \
               AND archived = FALSE \
               AND deleted_at IS NULL \
             ORDER BY schema_version DESC \
             LIMIT 1",
        )
        .bind(public_id)
        .bind(tenant_id)
        .bind(schema_version)
        .fetch_optional(&self.db)
        .await
        .map_err(AppError::from)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn create(
        &self,
        app: ApplicationRef,
        event_type_id: Uuid,
        payload: &serde_json::Value,
        payload_type: &str,
        tags: &HashMap<String, String>,
        idempotency_key: Option<&str>,
        body_hash: Option<&[u8]>,
        schema_valid: bool,
        schema_errors: &[crate::features::events::models::SchemaError],
        ctx: RequestContext,
    ) -> Result<EventRow, AppError> {
        let public_id = format!("evn_{}", NanoId::new());

        sqlx::query_scalar::<_, Uuid>(
            r#"INSERT INTO events
               (public_id, application_id, event_type_id,
                tenant_id, organization_id,
                payload, payload_type, idempotency_key, body_hash, tags, request_id, created_by,
                schema_valid, schema_errors)
               VALUES ($1, $2, $3, $4, $5, $6::jsonb, $7, $8, $9, $10::jsonb, $11, $12, $13, $14::jsonb)
               RETURNING id"#,
        )
        .bind(&public_id)
        .bind(app.id)
        .bind(event_type_id)
        .bind(app.tenant_id)
        .bind(app.organization_id)
        .bind(Json(payload))
        .bind(payload_type)
        .bind(idempotency_key)
        .bind(body_hash)
        .bind(Json(tags))
        .bind(ctx.request_id)
        .bind(ctx.created_by)
        .bind(schema_valid)
        .bind(Json(schema_errors))
        .fetch_one(&self.db)
        .await?;

        self.get_by_id(&public_id).await?.ok_or_else(|| {
            AppError::Internal("event created but not found on fetch".into())
        })
    }

    /// Looks up an event by idempotency key within the 1-hour TTL window.
    pub async fn find_by_idempotency_key(
        &self,
        application_id: Uuid,
        key: &str,
    ) -> Result<Option<EventRow>, AppError> {
        let sql = format!(
            "{} WHERE ev.application_id = $1 AND ev.idempotency_key = $2 \
             AND ev.created_at > NOW() - INTERVAL '1 hour'",
            BASE_SELECT
        );
        sqlx::query_as::<_, EventRow>(&sql)
            .bind(application_id)
            .bind(key)
            .fetch_optional(&self.db)
            .await
            .map_err(AppError::from)
    }

    pub async fn get_by_id(&self, public_id: &str) -> Result<Option<EventRow>, AppError> {
        let sql = format!("{} WHERE ev.public_id = $1", BASE_SELECT);
        sqlx::query_as::<_, EventRow>(&sql)
            .bind(public_id)
            .fetch_optional(&self.db)
            .await
            .map_err(AppError::from)
    }

    pub async fn list(&self, filter: ListQueryParams) -> Result<(Vec<EventRow>, i64), AppError> {
        #[derive(sqlx::FromRow)]
        struct Row {
            id: Uuid,
            public_id: String,
            application_id: Uuid,
            application_public_id: String,
            event_type_id: Uuid,
            event_type_public_id: String,
            event_type_name: String,
            endpoint_id: Option<Uuid>,
            endpoint_public_id: Option<String>,
            tenant_id: Uuid,
            organization_id: Uuid,
            payload: Json<serde_json::Value>,
            payload_type: String,
            idempotency_key: Option<String>,
            body_hash: Option<Vec<u8>>,
            tags: Json<HashMap<String, String>>,
            request_id: Uuid,
            created_by: Uuid,
            created_at: DateTime<Utc>,
            schema_valid: bool,
            schema_errors: Json<Vec<crate::features::events::models::SchemaError>>,
            total_count: i64,
        }

        let limit = filter.limit.min(100) as i64;
        let offset = (filter.page.saturating_sub(1)) as i64 * limit;

        let rows = sqlx::query_as::<_, Row>(
            r#"SELECT
                ev.id, ev.public_id,
                ev.application_id, a.public_id  AS application_public_id,
                ev.event_type_id,  et.public_id AS event_type_public_id, et.name AS event_type_name,
                ev.endpoint_id,    ep.public_id AS endpoint_public_id,
                ev.tenant_id, ev.organization_id,
                ev.payload, ev.idempotency_key, ev.body_hash, ev.tags,
                ev.request_id, ev.created_by, ev.created_at,
                ev.schema_valid, ev.schema_errors,
                ev.payload_type,
                COUNT(*) OVER() AS total_count
               FROM events ev
               JOIN  applications a  ON a.id  = ev.application_id
               JOIN  event_types  et ON et.id = ev.event_type_id
               LEFT JOIN endpoints ep ON ep.id = ev.endpoint_id
               WHERE ev.application_id = (
                   SELECT id FROM applications WHERE public_id = $1 AND deleted_at IS NULL
               )
                 AND ($2::text        IS NULL OR et.public_id = $2)
                 AND ($3::timestamptz IS NULL OR ev.created_at < $3)
                 AND ($4::timestamptz IS NULL OR ev.created_at > $4)
               ORDER BY ev.created_at DESC
               LIMIT $5 OFFSET $6"#,
        )
        .bind(&filter.application_id)
        .bind(&filter.event_type_id)
        .bind(filter.before)
        .bind(filter.after)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.db)
        .await?;

        let total = rows.first().map(|r| r.total_count).unwrap_or(0);
        let items = rows
            .into_iter()
            .map(|r| EventRow {
                id: r.id,
                public_id: r.public_id,
                application_id: r.application_id,
                application_public_id: r.application_public_id,
                event_type_id: r.event_type_id,
                event_type_public_id: r.event_type_public_id,
                event_type_name: r.event_type_name,
                endpoint_id: r.endpoint_id,
                endpoint_public_id: r.endpoint_public_id,
                tenant_id: r.tenant_id,
                organization_id: r.organization_id,
                payload: r.payload,
                idempotency_key: r.idempotency_key,
                body_hash: r.body_hash,
                tags: r.tags,
                request_id: r.request_id,
                created_by: r.created_by,
                created_at: r.created_at,
                schema_valid: r.schema_valid,
                schema_errors: r.schema_errors,
                payload_type: r.payload_type,
            })
            .collect();

        Ok((items, total))
    }
}
