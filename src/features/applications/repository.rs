use chrono::Utc;
use sqlx::types::Json;

use tracing::debug;
use uuid::Uuid;

use crate::common::types::RequestContext;
use crate::error::AppError;
use crate::features::applications::models::{
    Application, CreateApplicationRequest, GetApplicationResponse,
};

const SELECT_JOINED: &str = r#"
    ins.id, ins.public_id,
    ins.organization_id, o.public_id AS organization_public_id,
    ins.tenant_id,       t.public_id AS tenant_public_id,
    ins.environment_id,  e.public_id AS environment_public_id,
    ins.name, ins.description, ins.tags, ins.state,
    ins.created_by, ins.updated_by, ins.created_at, ins.updated_at
"#;

const GET_JOINED: &str = r#"
    a.id, a.public_id,
    a.organization_id, o.public_id AS organization_public_id,
    a.tenant_id,       t.public_id AS tenant_public_id,
    a.environment_id,  e.public_id AS environment_public_id,
    a.name, a.description, a.tags, a.state,
    a.created_by, a.updated_by, a.created_at, a.updated_at
"#;

pub struct ApplicationRepository {
    pool: crate::common::CountingPool,
}

impl ApplicationRepository {
    pub fn new(pool: crate::common::CountingPool) -> Self {
        Self { pool }
    }

    pub async fn resolve_environment(&self, public_id: &str) -> Result<Option<Uuid>, AppError> {
        sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM environments WHERE public_id = $1",
        )
        .bind(public_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::from)
    }

    pub async fn create(
        &self,
        req: CreateApplicationRequest,
        tenant_id: Uuid,
        organization_id: Uuid,
        environment_id: Uuid,
        ctx: RequestContext,
    ) -> Result<Application, AppError> {
        let id = Uuid::new_v4();
        let public_id = Application::new_public_id();
        let now = Utc::now();

        debug!(public_id = %public_id, "inserting application");

        let application = sqlx::query_as::<_, Application>(&format!(
            r#"
            WITH ins AS (
                INSERT INTO applications (
                    id, organization_id, tenant_id, environment_id, public_id,
                    name, description, tags,
                    created_at, updated_at,
                    request_id, version, created_by, updated_by, deleted_at
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
                RETURNING *
            )
            SELECT {SELECT_JOINED}
            FROM ins
            JOIN tenants t ON t.id = ins.tenant_id
            JOIN organizations o ON o.id = ins.organization_id
            LEFT JOIN environments e ON e.id = ins.environment_id
            "#
        ))
        .bind(id)
        .bind(organization_id)
        .bind(tenant_id)
        .bind(environment_id)
        .bind(public_id)
        .bind(req.name)
        .bind(req.description)
        .bind(Json(req.tags))
        .bind(now)
        .bind(now)
        .bind(ctx.request_id)
        .bind(0i32)
        .bind(ctx.created_by)
        .bind(ctx.created_by)
        .bind(None::<chrono::DateTime<Utc>>)
        .fetch_one(&self.pool)
        .await?;

        Ok(application)
    }

    pub async fn get_by_id(
        &self,
        public_id: String,
    ) -> Result<Option<GetApplicationResponse>, AppError> {
        debug!(public_id = %public_id, "querying application");

        let application = sqlx::query_as::<_, Application>(&format!(
            r#"
            SELECT {GET_JOINED}
            FROM applications a
            JOIN tenants t ON t.id = a.tenant_id
            JOIN organizations o ON o.id = a.organization_id
            LEFT JOIN environments e ON e.id = a.environment_id
            WHERE a.public_id = $1
            "#
        ))
        .bind(public_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(application.map(GetApplicationResponse::from))
    }

    pub async fn delete_by_id(
        &self,
        public_id: String,
        ctx: RequestContext,
    ) -> Result<(), AppError> {
        debug!(public_id = %public_id, "soft deleting application");
        sqlx::query(
            r#"
            UPDATE applications
            SET deleted_at = NOW(),
                state      = 'INACTIVE',
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

    pub async fn restore_by_id(
        &self,
        public_id: String,
        ctx: RequestContext,
    ) -> Result<Option<GetApplicationResponse>, AppError> {
        debug!(public_id = %public_id, "restoring application");

        let application = sqlx::query_as::<_, Application>(&format!(
            r#"
            WITH ins AS (
                UPDATE applications
                SET deleted_at = NULL,
                    state      = 'ACTIVE',
                    updated_by = $2,
                    request_id = $3,
                    updated_at = NOW()
                WHERE public_id = $1
                RETURNING *
            )
            SELECT {SELECT_JOINED}
            FROM ins
            JOIN tenants t ON t.id = ins.tenant_id
            JOIN organizations o ON o.id = ins.organization_id
            LEFT JOIN environments e ON e.id = ins.environment_id
            "#
        ))
        .bind(public_id)
        .bind(ctx.created_by)
        .bind(ctx.request_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(application.map(GetApplicationResponse::from))
    }
}
