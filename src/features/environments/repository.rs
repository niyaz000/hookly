use sqlx::{QueryBuilder};
use tracing::debug;
use uuid::Uuid;

use crate::common::{types::RequestContext, NanoId};
use crate::error::AppError;

use super::models::{Environment, EnvironmentStatus};

// Columns for SELECT queries that JOIN to get public IDs from parent tables.
const SELECT_COLS: &str = "
    e.id, e.public_id, e.tenant_id,
    t.public_id  AS tenant_public_id,
    o.public_id  AS organization_public_id,
    e.name, e.description, e.status, e.tags,
    e.version, e.created_by, e.updated_by, e.created_at, e.updated_at,
    creator.public_id AS created_by_public_id,
    updater.public_id AS updated_by_public_id
";

// Columns returned by INSERT/UPDATE RETURNING (no JOIN available there).
const RETURNING_COLS: &str = "
    id, public_id, tenant_id, name, description, status, tags,
    version, created_by, updated_by, created_at, updated_at
";

#[derive(Clone)]
pub struct EnvironmentRepository {
    pool: crate::common::CountingPool,
}

impl EnvironmentRepository {
    pub fn new(pool: crate::common::CountingPool) -> Self {
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
        description: Option<String>,
        tags: serde_json::Value,
        ctx: RequestContext,
    ) -> Result<Environment, AppError> {
        let id = Uuid::now_v7();
        let public_id = format!("env_{}", NanoId::new());

        debug!(public_id = %public_id, tenant_id = %tenant_id, name = %name, "inserting environment");

        let env = sqlx::query_as::<_, Environment>(&format!(
            r#"
            WITH ins AS (
                INSERT INTO environments (
                    id, public_id, tenant_id, name, description, tags,
                    request_id, version, created_by, updated_by, created_at, updated_at
                ) VALUES (
                    $1, $2, $3, $4, $5, $6,
                    $7, 0, $8, $8, NOW(), NOW()
                )
                RETURNING {RETURNING_COLS}
            )
            SELECT {SELECT_COLS}
            FROM ins e
            JOIN tenants t       ON t.id = e.tenant_id
            JOIN organizations o ON o.id = t.organization_id
            LEFT JOIN identity.users creator ON creator.id = e.created_by
            LEFT JOIN identity.users updater ON updater.id = e.updated_by
            "#
        ))
        .bind(id)
        .bind(&public_id)
        .bind(tenant_id)
        .bind(&name)
        .bind(description.as_deref())
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
            r#"
            SELECT {SELECT_COLS}
            FROM environments e
            JOIN tenants t       ON t.id = e.tenant_id
            JOIN organizations o ON o.id = t.organization_id
            LEFT JOIN identity.users creator ON creator.id = e.created_by
            LEFT JOIN identity.users updater ON updater.id = e.updated_by
            WHERE e.public_id = $1
            "#
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
            r#"SELECT {cols}
            FROM environments e
            JOIN tenants t       ON t.id = e.tenant_id
            JOIN organizations o ON o.id = t.organization_id
            LEFT JOIN identity.users creator ON creator.id = e.created_by
            LEFT JOIN identity.users updater ON updater.id = e.updated_by
            WHERE e.tenant_id = "#
        ));
        qb.push_bind(tenant_id);

        if let Some(st) = status {
            qb.push(" AND e.status = ").push_bind(st);
        }
        if let Some(tags_val) = tags {
            qb.push(" AND e.tags @> ").push_bind(tags_val);
        }
        if let Some(ref cursor_id) = cursor {
            qb.push(" AND e.public_id > ").push_bind(cursor_id.clone());
        }

        qb.push(" ORDER BY e.public_id ASC LIMIT ").push_bind(limit + 1);

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
            WITH upd AS (
                UPDATE environments SET
                    tags       = $1,
                    updated_by = $2,
                    request_id = $3,
                    version    = version + 1,
                    updated_at = NOW()
                WHERE public_id = $4
                RETURNING {RETURNING_COLS}
            )
            SELECT {SELECT_COLS}
            FROM upd e
            JOIN tenants t       ON t.id = e.tenant_id
            JOIN organizations o ON o.id = t.organization_id
            LEFT JOIN identity.users creator ON creator.id = e.created_by
            LEFT JOIN identity.users updater ON updater.id = e.updated_by
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
            WITH upd AS (
                UPDATE environments SET
                    status     = $1,
                    updated_by = $2,
                    request_id = $3,
                    version    = version + 1,
                    updated_at = NOW()
                WHERE public_id = $4
                RETURNING {RETURNING_COLS}
            )
            SELECT {SELECT_COLS}
            FROM upd e
            JOIN tenants t       ON t.id = e.tenant_id
            JOIN organizations o ON o.id = t.organization_id
            LEFT JOIN identity.users creator ON creator.id = e.created_by
            LEFT JOIN identity.users updater ON updater.id = e.updated_by
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
