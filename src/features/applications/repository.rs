use chrono::Utc;
use sqlx::PgPool;
use sqlx::types::Json;
use uuid::Uuid;

use crate::common::NanoId;
use crate::common::types::RequestContext;
use crate::error::AppError;
use crate::features::applications::models::{Application, CreateApplicationRequest};

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

        let application = sqlx::query_as::<_, Application>(
            r#"
            INSERT INTO applications (
                id,
                organization_id,
                tenant_id,
                public_id,
                name,
                description,
                tags,
                created_at,
                updated_at,
                request_id,
                version,
                created_by,
                updated_by,
                deleted_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
            RETURNING id, public_id, organization_id, tenant_id, name, description, tags, created_at, updated_at
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
}
