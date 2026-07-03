use chrono::{DateTime, Utc};
use sqlx::{types::Json, QueryBuilder};
use tracing::debug;
use uuid::Uuid;

use crate::{
    common::types::RequestContext,
    error::AppError,
};

use super::models::{CreateUserRequest, UpdateUserRequest, User, UserStatus};

// Explicit column list for SELECT with JOIN to get public IDs from parent tables.
const SELECT_COLS: &str = "
    u.id, u.public_id,
    u.organization_id, o.public_id AS organization_public_id,
    u.tenant_id,       t.public_id AS tenant_public_id,
    u.email, u.phone, u.status,
    u.email_verified_at, u.phone_verified_at, u.last_active_at,
    u.metadata, u.tags, u.settings, u.password_hash,
    u.version, u.created_by, u.updated_by, u.request_id,
    u.locked_until, u.login_count,
    u.created_at, u.updated_at, u.deleted_at,
    creator.public_id AS created_by_public_id,
    updater.public_id AS updated_by_public_id
";

// Columns returned by INSERT/UPDATE RETURNING (all from users table, no JOIN).
const RETURNING_COLS: &str = "
    id, public_id, organization_id, tenant_id,
    email, phone, status,
    email_verified_at, phone_verified_at, last_active_at,
    metadata, tags, settings, password_hash,
    version, created_by, updated_by, request_id,
    locked_until, login_count,
    created_at, updated_at, deleted_at
";

pub struct UserRepository {
    pool: crate::common::CountingPool,
}

impl UserRepository {
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

    pub async fn resolve_organization(&self, public_id: &str) -> Result<Option<Uuid>, AppError> {
        sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM organizations WHERE public_id = $1 AND deleted_at IS NULL",
        )
        .bind(public_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::from)
    }

    pub async fn create(
        &self,
        req: CreateUserRequest,
        tenant_id: Uuid,
        organization_id: Uuid,
        ctx: RequestContext,
    ) -> Result<User, AppError> {
        let id = Uuid::now_v7();
        let public_id = User::new_public_id();

        debug!(public_id = %public_id, "inserting user");

        let user = sqlx::query_as::<_, User>(&format!(
            r#"
            WITH ins AS (
                INSERT INTO identity.users (
                    id, public_id, organization_id, tenant_id, email, phone,
                    metadata, tags, settings, password_hash,
                    created_by, updated_by, request_id,
                    version, created_at, updated_at
                ) VALUES (
                    $1, $2, $3, $4, $5, $6,
                    $7, $8, $9, $10,
                    $11, $11, $12,
                    1, NOW(), NOW()
                )
                RETURNING {RETURNING_COLS}
            )
            SELECT {SELECT_COLS}
            FROM ins u
            JOIN organizations o ON o.id = u.organization_id
            JOIN tenants t       ON t.id = u.tenant_id
            LEFT JOIN identity.users creator ON creator.id = u.created_by
            LEFT JOIN identity.users updater ON updater.id = u.updated_by
            "#
        ))
        .bind(id)
        .bind(&public_id)
        .bind(organization_id)
        .bind(tenant_id)
        .bind(&req.email)
        .bind(req.phone.as_deref())
        .bind(Json(req.metadata.unwrap_or_default()))
        .bind(Json(req.tags.unwrap_or_default()))
        .bind(Json(req.settings.unwrap_or_default()))
        .bind(req.password_hash.as_deref())
        .bind(ctx.created_by)
        .bind(ctx.request_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(user)
    }

    pub async fn get_by_public_id(&self, public_id: &str) -> Result<Option<User>, AppError> {
        debug!(public_id = %public_id, "querying user");

        let user = sqlx::query_as::<_, User>(&format!(
            r#"SELECT {SELECT_COLS}
            FROM identity.users u
            JOIN organizations o ON o.id = u.organization_id
            JOIN tenants t       ON t.id = u.tenant_id
            LEFT JOIN identity.users creator ON creator.id = u.created_by
            LEFT JOIN identity.users updater ON updater.id = u.updated_by
            WHERE u.public_id = $1 AND u.deleted_at IS NULL"#
        ))
        .bind(public_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(user)
    }

    pub async fn update(
        &self,
        public_id: &str,
        req: UpdateUserRequest,
        ctx: RequestContext,
    ) -> Result<Option<User>, AppError> {
        debug!(public_id = %public_id, "updating user");

        let user = sqlx::query_as::<_, User>(&format!(
            r#"
            WITH upd AS (
                UPDATE identity.users SET
                    phone      = COALESCE($1, phone),
                    metadata   = COALESCE($2, metadata),
                    tags       = COALESCE($3, tags),
                    settings   = COALESCE($4, settings),
                    updated_by = $5,
                    request_id = $6,
                    version    = version + 1,
                    updated_at = NOW()
                WHERE public_id = $7 AND deleted_at IS NULL
                RETURNING {RETURNING_COLS}
            )
            SELECT {SELECT_COLS}
            FROM upd u
            JOIN organizations o ON o.id = u.organization_id
            JOIN tenants t       ON t.id = u.tenant_id
            LEFT JOIN identity.users creator ON creator.id = u.created_by
            LEFT JOIN identity.users updater ON updater.id = u.updated_by
            "#
        ))
        .bind(req.phone.as_deref())
        .bind(req.metadata.map(Json))
        .bind(req.tags.map(Json))
        .bind(req.settings.map(Json))
        .bind(ctx.created_by)
        .bind(ctx.request_id)
        .bind(public_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(user)
    }

    pub async fn delete(&self, public_id: &str, ctx: RequestContext) -> Result<bool, AppError> {
        debug!(public_id = %public_id, "soft deleting user");

        let result = sqlx::query(
            r#"
            UPDATE identity.users SET
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
    ) -> Result<Option<User>, AppError> {
        debug!(public_id = %public_id, "suspending user");

        let user = sqlx::query_as::<_, User>(&format!(
            r#"
            WITH upd AS (
                UPDATE identity.users SET
                    status     = 'suspended',
                    updated_by = $2,
                    request_id = $3,
                    version    = version + 1,
                    updated_at = NOW()
                WHERE public_id = $1 AND status = 'active' AND deleted_at IS NULL
                RETURNING {RETURNING_COLS}
            )
            SELECT {SELECT_COLS}
            FROM upd u
            JOIN organizations o ON o.id = u.organization_id
            JOIN tenants t       ON t.id = u.tenant_id
            LEFT JOIN identity.users creator ON creator.id = u.created_by
            LEFT JOIN identity.users updater ON updater.id = u.updated_by
            "#
        ))
        .bind(public_id)
        .bind(ctx.created_by)
        .bind(ctx.request_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(user)
    }

    pub async fn reactivate(
        &self,
        public_id: &str,
        ctx: RequestContext,
    ) -> Result<Option<User>, AppError> {
        debug!(public_id = %public_id, "reactivating user");

        let user = sqlx::query_as::<_, User>(&format!(
            r#"
            WITH upd AS (
                UPDATE identity.users SET
                    status     = 'active',
                    updated_by = $2,
                    request_id = $3,
                    version    = version + 1,
                    updated_at = NOW()
                WHERE public_id = $1 AND status = 'suspended' AND deleted_at IS NULL
                RETURNING {RETURNING_COLS}
            )
            SELECT {SELECT_COLS}
            FROM upd u
            JOIN organizations o ON o.id = u.organization_id
            JOIN tenants t       ON t.id = u.tenant_id
            LEFT JOIN identity.users creator ON creator.id = u.created_by
            LEFT JOIN identity.users updater ON updater.id = u.updated_by
            "#
        ))
        .bind(public_id)
        .bind(ctx.created_by)
        .bind(ctx.request_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(user)
    }

    pub async fn lock(
        &self,
        public_id: &str,
        locked_until: DateTime<Utc>,
        ctx: RequestContext,
    ) -> Result<Option<User>, AppError> {
        debug!(public_id = %public_id, "locking user");

        let user = sqlx::query_as::<_, User>(&format!(
            r#"
            WITH upd AS (
                UPDATE identity.users SET
                    status       = 'locked',
                    locked_until = $2,
                    updated_by   = $3,
                    request_id   = $4,
                    version      = version + 1,
                    updated_at   = NOW()
                WHERE public_id = $1 AND status = 'active' AND deleted_at IS NULL
                RETURNING {RETURNING_COLS}
            )
            SELECT {SELECT_COLS}
            FROM upd u
            JOIN organizations o ON o.id = u.organization_id
            JOIN tenants t       ON t.id = u.tenant_id
            LEFT JOIN identity.users creator ON creator.id = u.created_by
            LEFT JOIN identity.users updater ON updater.id = u.updated_by
            "#
        ))
        .bind(public_id)
        .bind(locked_until)
        .bind(ctx.created_by)
        .bind(ctx.request_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(user)
    }

    pub async fn unlock(
        &self,
        public_id: &str,
        ctx: RequestContext,
    ) -> Result<Option<User>, AppError> {
        debug!(public_id = %public_id, "unlocking user");

        let user = sqlx::query_as::<_, User>(&format!(
            r#"
            WITH upd AS (
                UPDATE identity.users SET
                    status       = 'active',
                    locked_until = NULL,
                    updated_by   = $2,
                    request_id   = $3,
                    version      = version + 1,
                    updated_at   = NOW()
                WHERE public_id = $1 AND status = 'locked' AND deleted_at IS NULL
                RETURNING {RETURNING_COLS}
            )
            SELECT {SELECT_COLS}
            FROM upd u
            JOIN organizations o ON o.id = u.organization_id
            JOIN tenants t       ON t.id = u.tenant_id
            LEFT JOIN identity.users creator ON creator.id = u.created_by
            LEFT JOIN identity.users updater ON updater.id = u.updated_by
            "#
        ))
        .bind(public_id)
        .bind(ctx.created_by)
        .bind(ctx.request_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(user)
    }

    pub async fn list(
        &self,
        limit: i64,
        cursor: Option<Uuid>,
        status: Option<UserStatus>,
        organization_id: Option<Uuid>,
        tenant_id: Option<Uuid>,
    ) -> Result<(Vec<User>, Option<Uuid>), AppError> {
        debug!(limit = limit, "listing users");

        let cols = SELECT_COLS;
        let mut qb = QueryBuilder::<sqlx::Postgres>::new(format!(
            r#"SELECT {cols}
            FROM identity.users u
            JOIN organizations o ON o.id = u.organization_id
            JOIN tenants t       ON t.id = u.tenant_id
            LEFT JOIN identity.users creator ON creator.id = u.created_by
            LEFT JOIN identity.users updater ON updater.id = u.updated_by
            WHERE u.deleted_at IS NULL"#
        ));

        if let Some(cursor_id) = cursor {
            qb.push(" AND u.id > ").push_bind(cursor_id);
        }

        if let Some(s) = status {
            qb.push(" AND u.status = ").push_bind(s);
        }

        if let Some(org_id) = organization_id {
            qb.push(" AND u.organization_id = ").push_bind(org_id);
        }

        if let Some(t_id) = tenant_id {
            qb.push(" AND u.tenant_id = ").push_bind(t_id);
        }

        qb.push(" ORDER BY u.id ASC LIMIT ").push_bind(limit + 1);

        let mut users: Vec<User> = qb.build_query_as::<User>().fetch_all(&self.pool).await?;

        let next_cursor = if users.len() as i64 > limit {
            users.pop().map(|u| u.id)
        } else {
            None
        };

        Ok((users, next_cursor))
    }
}
