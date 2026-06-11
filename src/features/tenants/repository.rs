use sqlx::{types::Json, PgPool, QueryBuilder};
use tracing::debug;
use uuid::Uuid;

use crate::{
    common::{types::RequestContext, NanoId},
    error::AppError,
};

use super::models::{CreateTenantRequest, Tenant, TenantStatus, UpdateTenantRequest};

pub struct TenantRepository {
    pool: PgPool,
}

impl TenantRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn resolve_organization(&self, public_id: &str) -> Result<Option<Uuid>, AppError> {
        let id = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM organizations WHERE public_id = $1 AND deleted_at IS NULL",
        )
        .bind(public_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(id)
    }

    pub async fn create(
        &self,
        req: CreateTenantRequest,
        organization_id: Uuid,
        ctx: RequestContext,
    ) -> Result<Tenant, AppError> {
        let id = Uuid::now_v7();
        let public_id = format!("ten_{}", NanoId::generate(20));

        debug!(public_id = %public_id, organization_id = %organization_id, "inserting tenant");

        let tenant = sqlx::query_as::<_, Tenant>(
            r#"
            INSERT INTO tenants (
                id, public_id, organization_id, name, description,
                tags, metadata, settings,
                created_by, updated_by, request_id, version,
                created_at, updated_at
            ) VALUES (
                $1, $2, $3, $4, $5,
                $6, $7, $8,
                $9, $9, $10, 0,
                NOW(), NOW()
            )
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(&public_id)
        .bind(organization_id)
        .bind(&req.name)
        .bind(req.description.as_deref())
        .bind(Json(req.tags.unwrap_or_default()))
        .bind(Json(req.metadata.unwrap_or_default()))
        .bind(Json(req.settings.unwrap_or_default()))
        .bind(ctx.created_by)
        .bind(ctx.request_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(tenant)
    }

    pub async fn get_by_public_id(&self, public_id: &str) -> Result<Option<Tenant>, AppError> {
        debug!(public_id = %public_id, "querying tenant");

        let tenant = sqlx::query_as::<_, Tenant>("SELECT * FROM tenants WHERE public_id = $1")
            .bind(public_id)
            .fetch_optional(&self.pool)
            .await?;

        Ok(tenant)
    }

    pub async fn update(
        &self,
        public_id: &str,
        req: UpdateTenantRequest,
        ctx: RequestContext,
    ) -> Result<Option<Tenant>, AppError> {
        debug!(public_id = %public_id, "updating tenant");

        let tenant = sqlx::query_as::<_, Tenant>(
            r#"
            UPDATE tenants SET
                name        = COALESCE($1, name),
                description = COALESCE($2, description),
                tags        = COALESCE($3, tags),
                metadata    = COALESCE($4, metadata),
                settings    = COALESCE($5, settings),
                updated_by  = $6,
                request_id  = $7,
                version     = version + 1,
                updated_at  = NOW()
            WHERE public_id = $8 AND deleted_at IS NULL
            RETURNING *
            "#,
        )
        .bind(req.name)
        .bind(req.description)
        .bind(req.tags.map(Json))
        .bind(req.metadata.map(Json))
        .bind(req.settings.map(Json))
        .bind(ctx.created_by)
        .bind(ctx.request_id)
        .bind(public_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(tenant)
    }

    pub async fn delete(&self, public_id: &str, ctx: RequestContext) -> Result<bool, AppError> {
        debug!(public_id = %public_id, "soft deleting tenant");

        let result = sqlx::query(
            r#"
            UPDATE tenants SET
                deleted_at = NOW(),
                status     = 'inactive',
                updated_by = $2,
                request_id = $3,
                version    = version + 1,
                updated_at = NOW()
            WHERE public_id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(public_id)
        .bind(ctx.created_by)
        .bind(ctx.request_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn suspend(
        &self,
        public_id: &str,
        ctx: RequestContext,
    ) -> Result<Option<Tenant>, AppError> {
        debug!(public_id = %public_id, "suspending tenant");

        let tenant = sqlx::query_as::<_, Tenant>(
            r#"
            UPDATE tenants SET
                status     = 'suspended',
                updated_by = $2,
                request_id = $3,
                version    = version + 1,
                updated_at = NOW()
            WHERE public_id = $1 AND status = 'active' AND deleted_at IS NULL
            RETURNING *
            "#,
        )
        .bind(public_id)
        .bind(ctx.created_by)
        .bind(ctx.request_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(tenant)
    }

    pub async fn reactivate(
        &self,
        public_id: &str,
        ctx: RequestContext,
    ) -> Result<Option<Tenant>, AppError> {
        debug!(public_id = %public_id, "reactivating tenant");

        let tenant = sqlx::query_as::<_, Tenant>(
            r#"
            UPDATE tenants SET
                status     = 'active',
                updated_by = $2,
                request_id = $3,
                version    = version + 1,
                updated_at = NOW()
            WHERE public_id = $1 AND status = 'suspended' AND deleted_at IS NULL
            RETURNING *
            "#,
        )
        .bind(public_id)
        .bind(ctx.created_by)
        .bind(ctx.request_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(tenant)
    }

    pub async fn list(
        &self,
        limit: i64,
        cursor: Option<Uuid>,
        status: Option<TenantStatus>,
        organization_id: Option<Uuid>,
    ) -> Result<(Vec<Tenant>, Option<Uuid>), AppError> {
        debug!(limit = limit, "listing tenants");

        let mut qb =
            QueryBuilder::<sqlx::Postgres>::new("SELECT * FROM tenants WHERE deleted_at IS NULL");

        if let Some(cursor_id) = cursor {
            qb.push(" AND id > ").push_bind(cursor_id);
        }

        if let Some(s) = status {
            qb.push(" AND status = ").push_bind(s);
        }

        if let Some(org_id) = organization_id {
            qb.push(" AND organization_id = ").push_bind(org_id);
        }

        qb.push(" ORDER BY id ASC LIMIT ").push_bind(limit + 1);

        let mut tenants: Vec<Tenant> = qb.build_query_as::<Tenant>().fetch_all(&self.pool).await?;

        let next_cursor = if tenants.len() as i64 > limit {
            tenants.pop().map(|t| t.id)
        } else {
            None
        };

        Ok((tenants, next_cursor))
    }
}
