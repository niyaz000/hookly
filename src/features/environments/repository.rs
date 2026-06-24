use sqlx::{PgPool, QueryBuilder};
use tracing::debug;
use uuid::Uuid;

use crate::common::{types::RequestContext, NanoId};
use crate::error::AppError;

use super::models::{Environment, EnvironmentStatus};

const SELECT_COLS: &str = "
    id, public_id, tenant_id, name, status, tags,
    version, created_by, updated_by, created_at, updated_at
";

#[derive(Clone)]
pub struct EnvironmentRepository {
    pool: PgPool,
}

impl EnvironmentRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn resolve_tenant(&self, public_id: &str) -> Result<Option<Uuid>, AppError> {
        let id = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM tenants WHERE public_id = $1 AND deleted_at IS NULL",
        )
        .bind(public_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(id)
    }

    pub async fn create(
        &self,
        tenant_id: Uuid,
        name: String,
        tags: serde_json::Value,
        ctx: RequestContext,
    ) -> Result<Environment, AppError> {
        let id = Uuid::now_v7();
        let public_id = format!("env_{}", NanoId::new());

        debug!(public_id = %public_id, tenant_id = %tenant_id, name = %name, "inserting environment");

        let env = sqlx::query_as::<_, Environment>(&format!(
            r#"
            INSERT INTO environments (
                id, public_id, tenant_id, name, tags,
                request_id, version, created_by, updated_by, created_at, updated_at
            ) VALUES (
                $1, $2, $3, $4, $5,
                $6, 0, $7, $7, NOW(), NOW()
            )
            RETURNING {SELECT_COLS}
            "#
        ))
        .bind(id)
        .bind(&public_id)
        .bind(tenant_id)
        .bind(&name)
        .bind(&tags)
        .bind(ctx.request_id)
        .bind(ctx.created_by)
        .fetch_one(&self.pool)
        .await?;

        Ok(env)
    }

    pub async fn get_by_public_id(&self, public_id: &str) -> Result<Option<Environment>, AppError> {
        debug!(public_id = %public_id, "querying environment by public_id");

        let env = sqlx::query_as::<_, Environment>(&format!(
            "SELECT {SELECT_COLS} FROM environments WHERE public_id = $1"
        ))
        .bind(public_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(env)
    }

    pub async fn list(
        &self,
        tenant_id: Uuid,
        status: Option<EnvironmentStatus>,
        limit: i64,
        cursor: Option<String>,
        tags: Option<serde_json::Value>,
    ) -> Result<(Vec<Environment>, Option<String>), AppError> {
        debug!(tenant_id = %tenant_id, limit = limit, "listing environments");

        let cols = SELECT_COLS;
        let mut qb = QueryBuilder::<sqlx::Postgres>::new(format!(
            "SELECT {cols} FROM environments WHERE tenant_id = "
        ));
        qb.push_bind(tenant_id);

        if let Some(st) = status {
            qb.push(" AND status = ").push_bind(st);
        }
        if let Some(tags_val) = tags {
            qb.push(" AND tags @> ").push_bind(tags_val);
        }
        if let Some(ref cursor_id) = cursor {
            qb.push(" AND public_id > ").push_bind(cursor_id.clone());
        }

        qb.push(" ORDER BY public_id ASC LIMIT ").push_bind(limit + 1);

        let mut envs: Vec<Environment> = qb
            .build_query_as::<Environment>()
            .fetch_all(&self.pool)
            .await?;

        let next_cursor = if envs.len() as i64 > limit {
            envs.pop().map(|e| e.public_id)
        } else {
            None
        };

        Ok((envs, next_cursor))
    }

    pub async fn update_tags(
        &self,
        public_id: &str,
        tags: serde_json::Value,
        ctx: RequestContext,
    ) -> Result<Option<Environment>, AppError> {
        debug!(public_id = %public_id, "updating environment tags");

        let env = sqlx::query_as::<_, Environment>(&format!(
            r#"
            UPDATE environments SET
                tags       = $1,
                updated_by = $2,
                request_id = $3,
                version    = version + 1,
                updated_at = NOW()
            WHERE public_id = $4
            RETURNING {SELECT_COLS}
            "#
        ))
        .bind(&tags)
        .bind(ctx.created_by)
        .bind(ctx.request_id)
        .bind(public_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(env)
    }

    pub async fn set_status(
        &self,
        public_id: &str,
        status: EnvironmentStatus,
        ctx: RequestContext,
    ) -> Result<Option<Environment>, AppError> {
        debug!(public_id = %public_id, "updating environment status");

        let env = sqlx::query_as::<_, Environment>(&format!(
            r#"
            UPDATE environments SET
                status     = $1,
                updated_by = $2,
                request_id = $3,
                version    = version + 1,
                updated_at = NOW()
            WHERE public_id = $4
            RETURNING {SELECT_COLS}
            "#
        ))
        .bind(status)
        .bind(ctx.created_by)
        .bind(ctx.request_id)
        .bind(public_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(env)
    }
}
