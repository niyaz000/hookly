use chrono::Utc;
use sqlx::types::Json;
use sqlx::PgPool;
use tracing::debug;
use uuid::Uuid;

use crate::common::types::RequestContext;
use crate::common::NanoId;
use crate::error::AppError;
use crate::features::applications::models::{
    Application, CreateApplicationRequest, GetApplicationResponse,
};

pub struct ApplicationRepository {
    pool: PgPool,
}

impl ApplicationRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(
        &self,
        req: CreateApplicationRequest,
        ctx: RequestContext,
    ) -> Result<Application, AppError> {
        let id = Uuid::new_v4();
        let public_id = format!("app_{}", NanoId::new());
        let now = Utc::now();

        debug!(public_id = %public_id, "inserting application");
        let application = sqlx::query_as::<_, Application>(
            r#"
            INSERT INTO applications (
                id, organization_id, tenant_id, public_id,
                name, description, tags,
                created_at, updated_at,
                request_id, version, created_by, updated_by, deleted_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
            RETURNING
                id, public_id, organization_id, tenant_id,
                name, description, tags, state,
                created_by, updated_by, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(req.organization_id)
        .bind(req.tenant_id)
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
        let application = sqlx::query_as::<_, Application>(
            r#"
            SELECT id, public_id, organization_id, tenant_id,
                   name, description, tags, state,
                   created_by, updated_by, created_at, updated_at
            FROM applications
            WHERE public_id = $1
            "#,
        )
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
        let application = sqlx::query_as::<_, Application>(
            r#"
            UPDATE applications
            SET deleted_at = NULL,
                state      = 'ACTIVE',
                updated_by = $2,
                request_id = $3,
                updated_at = NOW()
            WHERE public_id = $1
            RETURNING
                id, public_id, organization_id, tenant_id,
                name, description, tags, state,
                created_by, updated_by, created_at, updated_at
            "#,
        )
        .bind(public_id)
        .bind(ctx.created_by)
        .bind(ctx.request_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(application.map(GetApplicationResponse::from))
    }
}
