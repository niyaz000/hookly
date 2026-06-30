use sqlx::types::Json;

use tracing::debug;
use uuid::Uuid;

use crate::common::types::RequestContext;
use crate::common::NanoId;
use crate::error::AppError;
use crate::features::event_types::models::{EventType, ListQueryParams};

// Column list for CTE-based writes (INSERT/UPDATE … RETURNING * aliased as `cte`).
const CTE_JOINED: &str = r#"
    cte.id, cte.public_id,
    cte.organization_id, o.public_id AS organization_public_id,
    cte.tenant_id,       t.public_id AS tenant_public_id,
    cte.application_id,  a.public_id AS application_public_id,
    cte.name, cte.schema_version, cte.description, cte.event_schema,
    cte.archived, cte.version, cte.created_by, cte.updated_by, cte.created_at, cte.updated_at
"#;

// Column list for direct SELECT queries (table alias `et`).
const ET_JOINED: &str = r#"
    et.id, et.public_id,
    et.organization_id, o.public_id AS organization_public_id,
    et.tenant_id,       t.public_id AS tenant_public_id,
    et.application_id,  a.public_id AS application_public_id,
    et.name, et.schema_version, et.description, et.event_schema,
    et.archived, et.version, et.created_by, et.updated_by, et.created_at, et.updated_at
"#;

const JOINS: &str = r#"
    JOIN tenants t      ON t.id = cte.tenant_id
    JOIN organizations o ON o.id = cte.organization_id
    LEFT JOIN applications a ON a.id = cte.application_id
"#;

const ET_JOINS: &str = r#"
    JOIN tenants t      ON t.id = et.tenant_id
    JOIN organizations o ON o.id = et.organization_id
    LEFT JOIN applications a ON a.id = et.application_id
"#;

pub struct EventTypeRepository {
    pool: crate::common::CountingPool,
}

impl EventTypeRepository {
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

    pub async fn resolve_tenant_with_org(
        &self,
        public_id: &str,
    ) -> Result<Option<(Uuid, Uuid)>, AppError> {
        sqlx::query_as::<_, (Uuid, Uuid)>(
            "SELECT id, organization_id FROM tenants WHERE public_id = $1 AND deleted_at IS NULL",
        )
        .bind(public_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::from)
    }

    pub async fn resolve_application(&self, public_id: &str) -> Result<Option<Uuid>, AppError> {
        sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM applications WHERE public_id = $1 AND deleted_at IS NULL",
        )
        .bind(public_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::from)
    }

    pub async fn create(
        &self,
        req: crate::features::event_types::models::CreateEventTypeRequest,
        tenant_id: Uuid,
        organization_id: Uuid,
        application_id: Uuid,
        ctx: RequestContext,
    ) -> Result<EventType, AppError> {
        let id = Uuid::new_v4();
        let public_id = format!("evt_{}", NanoId::new());
        let schema_version = req.schema_version.unwrap_or_else(|| "1.0".to_string());

        debug!(public_id = %public_id, "inserting event_type");
        let et = sqlx::query_as::<_, EventType>(&format!(
            r#"
            WITH cte AS (
                INSERT INTO event_types (
                    id, organization_id, tenant_id, application_id, public_id,
                    name, schema_version, description, event_schema,
                    archived, created_by, updated_by, request_id,
                    version, created_at, updated_at, deleted_at
                ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,FALSE,$10,$10,$11,0,NOW(),NOW(),NULL)
                RETURNING *
            )
            SELECT {CTE_JOINED}
            FROM cte {JOINS}
            "#
        ))
        .bind(id)
        .bind(organization_id)
        .bind(tenant_id)
        .bind(application_id)
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
                SELECT tenant_id, organization_id, application_id, name
                FROM event_types
                WHERE public_id = $1 AND deleted_at IS NULL
            ),
            cte AS (
                INSERT INTO event_types (
                    id, organization_id, tenant_id, application_id, public_id,
                    name, schema_version, description, event_schema,
                    archived, created_by, updated_by, request_id,
                    version, created_at, updated_at, deleted_at
                )
                SELECT $2, organization_id, tenant_id, application_id, $3,
                       name, $4, $5, $6,
                       FALSE, $7, $7, $8,
                       0, NOW(), NOW(), NULL
                FROM source
                RETURNING *
            )
            SELECT {CTE_JOINED}
            FROM cte {JOINS}
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

    pub async fn list(&self, tenant_id: Uuid, filter: ListQueryParams) -> Result<(Vec<EventType>, i64), AppError> {
        debug!(tenant_id = %tenant_id, "listing event_types");
        let offset = (filter.page - 1) * filter.limit;

        #[derive(sqlx::FromRow)]
        struct Row {
            id: Uuid,
            public_id: String,
            organization_id: Uuid,
            organization_public_id: String,
            tenant_id: Uuid,
            tenant_public_id: String,
            application_id: Option<Uuid>,
            application_public_id: Option<String>,
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
                et.id, et.public_id,
                et.organization_id, o.public_id AS organization_public_id,
                et.tenant_id,       t.public_id AS tenant_public_id,
                et.application_id,  a.public_id AS application_public_id,
                et.name, et.schema_version, et.description, et.event_schema,
                et.archived, et.version, et.created_by, et.updated_by, et.created_at, et.updated_at,
                COUNT(*) OVER() AS total_count
            FROM event_types et
            JOIN tenants t      ON t.id = et.tenant_id
            JOIN organizations o ON o.id = et.organization_id
            LEFT JOIN applications a ON a.id = et.application_id
            WHERE et.tenant_id = $1
              AND ($2::text  IS NULL OR et.name           = $2)
              AND ($3::text  IS NULL OR et.schema_version = $3)
              AND ($4::bool  IS NULL OR et.archived       = $4)
              AND et.deleted_at IS NULL
            ORDER BY et.created_at DESC
            LIMIT $5 OFFSET $6
            "#,
        )
        .bind(tenant_id)
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
                organization_public_id: r.organization_public_id,
                tenant_id: r.tenant_id,
                tenant_public_id: r.tenant_public_id,
                application_id: r.application_id,
                application_public_id: r.application_public_id,
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
            SELECT {ET_JOINED}
            FROM event_types et
            {ET_JOINS}
            WHERE et.public_id = $1 AND et.deleted_at IS NULL
            "#
        ))
        .bind(public_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(et)
    }

    pub async fn get_versions(&self, public_id: &str) -> Result<Vec<EventType>, AppError> {
        debug!(public_id = %public_id, "querying event_type versions");
        let items = sqlx::query_as::<_, EventType>(&format!(
            r#"
            SELECT {ET_JOINED}
            FROM event_types et
            {ET_JOINS}
            WHERE (et.name, et.tenant_id) = (
                SELECT name, tenant_id FROM event_types WHERE public_id = $1
            )
            AND et.deleted_at IS NULL
            ORDER BY et.schema_version ASC
            "#
        ))
        .bind(public_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(items)
    }

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
            WITH cte AS (
                UPDATE event_types
                SET description = $2,
                    version     = version + 1,
                    updated_by  = $3,
                    request_id  = $4,
                    updated_at  = NOW()
                WHERE public_id = $1
                  AND version   = $5
                  AND deleted_at IS NULL
                RETURNING *
            )
            SELECT {CTE_JOINED}
            FROM cte {JOINS}
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
            WITH cte AS (
                UPDATE event_types
                SET archived   = $2,
                    version    = version + 1,
                    updated_by = $3,
                    request_id = $4,
                    updated_at = NOW()
                WHERE public_id = $1 AND deleted_at IS NULL
                RETURNING *
            )
            SELECT {CTE_JOINED}
            FROM cte {JOINS}
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
