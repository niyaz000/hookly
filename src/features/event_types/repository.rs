use sqlx::types::Json;
use sqlx::PgPool;
use tracing::debug;
use uuid::Uuid;

use crate::common::types::RequestContext;
use crate::common::NanoId;
use crate::error::AppError;
use crate::features::event_types::models::{EventType, ListQueryParams};

const SELECT_COLS: &str = "
    id, public_id, organization_id, tenant_id,
    name, schema_version, description, event_schema,
    archived, version, created_by, updated_by, created_at, updated_at";

pub struct EventTypeRepository {
    pool: PgPool,
}

impl EventTypeRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(
        &self,
        req: crate::features::event_types::models::CreateEventTypeRequest,
        ctx: RequestContext,
    ) -> Result<EventType, AppError> {
        let id = Uuid::new_v4();
        let public_id = format!("evt_{}", NanoId::new());
        let schema_version = req.schema_version.unwrap_or_else(|| "1.0".to_string());

        debug!(public_id = %public_id, "inserting event_type");
        let et = sqlx::query_as::<_, EventType>(&format!(
            r#"
            INSERT INTO event_types (
                id, organization_id, tenant_id, public_id,
                name, schema_version, description, event_schema,
                archived, created_by, updated_by, request_id,
                version, created_at, updated_at, deleted_at
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,FALSE,$9,$9,$10,0,NOW(),NOW(),NULL)
            RETURNING {SELECT_COLS}
            "#
        ))
        .bind(id)
        .bind(req.organization_id)
        .bind(req.tenant_id)
        .bind(public_id)
        .bind(req.name)
        .bind(schema_version)
        .bind(req.description)
        .bind(Json(req.event_schema))
        .bind(ctx.created_by)
        .bind(ctx.request_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(et)
    }

    /// Creates a new version row derived from an existing event type.
    /// Returns None if the source public_id does not exist or is deleted.
    pub async fn create_version(
        &self,
        source_public_id: &str,
        req: crate::features::event_types::models::CreateVersionRequest,
        ctx: RequestContext,
    ) -> Result<Option<EventType>, AppError> {
        let new_public_id = format!("evt_{}", NanoId::new());
        let new_id = Uuid::new_v4();

        debug!(source = %source_public_id, new = %new_public_id, "inserting event_type version");
        let et = sqlx::query_as::<_, EventType>(&format!(
            r#"
            WITH source AS (
                SELECT tenant_id, organization_id, name
                FROM event_types
                WHERE public_id = $1 AND deleted_at IS NULL
            )
            INSERT INTO event_types (
                id, organization_id, tenant_id, public_id,
                name, schema_version, description, event_schema,
                archived, created_by, updated_by, request_id,
                version, created_at, updated_at, deleted_at
            )
            SELECT $2, organization_id, tenant_id, $3,
                   name, $4, $5, $6,
                   FALSE, $7, $7, $8,
                   0, NOW(), NOW(), NULL
            FROM source
            RETURNING {SELECT_COLS}
            "#
        ))
        .bind(source_public_id)
        .bind(new_id)
        .bind(new_public_id)
        .bind(req.schema_version)
        .bind(req.description)
        .bind(Json(req.event_schema))
        .bind(ctx.created_by)
        .bind(ctx.request_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(et)
    }

    pub async fn list(&self, filter: ListQueryParams) -> Result<(Vec<EventType>, i64), AppError> {
        debug!(tenant_id = %filter.tenant_id, "listing event_types");
        let offset = (filter.page - 1) * filter.limit;

        #[derive(sqlx::FromRow)]
        struct Row {
            id: Uuid,
            public_id: String,
            organization_id: Uuid,
            tenant_id: Uuid,
            name: String,
            schema_version: String,
            description: Option<String>,
            event_schema: Json<crate::features::event_types::models::PropertyDef>,
            archived: bool,
            version: i32,
            created_by: Uuid,
            updated_by: Uuid,
            created_at: chrono::DateTime<chrono::Utc>,
            updated_at: chrono::DateTime<chrono::Utc>,
            total_count: i64,
        }

        let rows = sqlx::query_as::<_, Row>(
            r#"
            SELECT
                id, public_id, organization_id, tenant_id,
                name, schema_version, description, event_schema,
                archived, version, created_by, updated_by, created_at, updated_at,
                COUNT(*) OVER() AS total_count
            FROM event_types
            WHERE tenant_id = $1
              AND ($2::text  IS NULL OR name           = $2)
              AND ($3::text  IS NULL OR schema_version = $3)
              AND ($4::bool  IS NULL OR archived       = $4)
              AND deleted_at IS NULL
            ORDER BY created_at DESC
            LIMIT $5 OFFSET $6
            "#,
        )
        .bind(filter.tenant_id)
        .bind(filter.name)
        .bind(filter.schema_version)
        .bind(filter.archived)
        .bind(filter.limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        let total = rows.first().map(|r| r.total_count).unwrap_or(0);
        let items = rows
            .into_iter()
            .map(|r| EventType {
                id: r.id,
                public_id: r.public_id,
                organization_id: r.organization_id,
                tenant_id: r.tenant_id,
                name: r.name,
                schema_version: r.schema_version,
                description: r.description,
                event_schema: r.event_schema,
                archived: r.archived,
                version: r.version,
                created_by: r.created_by,
                updated_by: r.updated_by,
                created_at: r.created_at,
                updated_at: r.updated_at,
            })
            .collect();

        Ok((items, total))
    }

    pub async fn get_by_id(&self, public_id: &str) -> Result<Option<EventType>, AppError> {
        debug!(public_id = %public_id, "querying event_type");
        let et = sqlx::query_as::<_, EventType>(&format!(
            r#"
            SELECT {SELECT_COLS}
            FROM event_types
            WHERE public_id = $1 AND deleted_at IS NULL
            "#
        ))
        .bind(public_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(et)
    }

    /// Returns all versions (same name + tenant) ordered by schema_version ASC.
    pub async fn get_versions(&self, public_id: &str) -> Result<Vec<EventType>, AppError> {
        debug!(public_id = %public_id, "querying event_type versions");
        let items = sqlx::query_as::<_, EventType>(
            r#"
            SELECT id, public_id, organization_id, tenant_id,
                   name, schema_version, description, event_schema,
                   archived, version, created_by, updated_by, created_at, updated_at
            FROM event_types
            WHERE (name, tenant_id) = (
                SELECT name, tenant_id FROM event_types WHERE public_id = $1
            )
            AND deleted_at IS NULL
            ORDER BY schema_version ASC
            "#,
        )
        .bind(public_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(items)
    }

    /// Updates only the description. Uses optimistic locking via `version`.
    /// Returns None when the row doesn't match (not found OR stale version).
    pub async fn update_description(
        &self,
        public_id: &str,
        description: Option<String>,
        version: i32,
        ctx: RequestContext,
    ) -> Result<Option<EventType>, AppError> {
        debug!(public_id = %public_id, "updating event_type description");
        let et = sqlx::query_as::<_, EventType>(&format!(
            r#"
            UPDATE event_types
            SET description = $2,
                version     = version + 1,
                updated_by  = $3,
                request_id  = $4,
                updated_at  = NOW()
            WHERE public_id = $1
              AND version   = $5
              AND deleted_at IS NULL
            RETURNING {SELECT_COLS}
            "#
        ))
        .bind(public_id)
        .bind(description)
        .bind(ctx.created_by)
        .bind(ctx.request_id)
        .bind(version)
        .fetch_optional(&self.pool)
        .await?;

        Ok(et)
    }

    pub async fn delete_by_id(&self, public_id: &str, ctx: RequestContext) -> Result<(), AppError> {
        debug!(public_id = %public_id, "soft deleting event_type");
        sqlx::query(
            r#"
            UPDATE event_types
            SET deleted_at = NOW(),
                updated_by = $2,
                request_id = $3,
                updated_at = NOW()
            WHERE public_id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(public_id)
        .bind(ctx.created_by)
        .bind(ctx.request_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn set_archived(
        &self,
        public_id: &str,
        archived: bool,
        ctx: RequestContext,
    ) -> Result<Option<EventType>, AppError> {
        debug!(public_id = %public_id, archived = %archived, "setting event_type archived");
        let et = sqlx::query_as::<_, EventType>(&format!(
            r#"
            UPDATE event_types
            SET archived   = $2,
                version    = version + 1,
                updated_by = $3,
                request_id = $4,
                updated_at = NOW()
            WHERE public_id = $1 AND deleted_at IS NULL
            RETURNING {SELECT_COLS}
            "#
        ))
        .bind(public_id)
        .bind(archived)
        .bind(ctx.created_by)
        .bind(ctx.request_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(et)
    }
}
