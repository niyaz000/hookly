use sqlx::{QueryBuilder};
use tracing::debug;
use uuid::Uuid;

use crate::common::types::RequestContext;
use crate::error::AppError;
use crate::features::permissions::models::Permission;

use super::models::{Role, RolePermissionRow};

const SELECT_COLS: &str = "
    id, public_id, tenant_id, name, description, is_system,
    version, created_by, updated_by, created_at, updated_at
";

#[derive(Clone)]
pub struct RoleRepository {
    pool: crate::common::CountingPool,
}

impl RoleRepository {
    pub fn new(pool: crate::common::CountingPool) -> Self {
        Self { pool }
    }

    pub async fn create(
        &self,
        tenant_id: Uuid,
        name: String,
        description: Option<String>,
        is_system: bool,
        ctx: RequestContext,
    ) -> Result<Role, AppError> {
        let id = Uuid::now_v7();
        let public_id = Role::new_public_id();

        debug!(public_id = %public_id, tenant_id = %tenant_id, name = %name, "inserting role");

        let role = sqlx::query_as::<_, Role>(&format!(
            r#"
            INSERT INTO roles (id, public_id, tenant_id, name, description, is_system, version, created_by, updated_by)
            VALUES ($1, $2, $3, $4, $5, $6, 0, $7, $7)
            RETURNING {SELECT_COLS}
            "#
        ))
        .bind(id)
        .bind(&public_id)
        .bind(tenant_id)
        .bind(&name)
        .bind(&description)
        .bind(is_system)
        .bind(ctx.created_by)
        .fetch_one(&self.pool)
        .await?;

        Ok(role)
    }

    pub async fn get_by_public_id(&self, public_id: &str) -> Result<Option<Role>, AppError> {
        debug!(public_id = %public_id, "querying role");

        let role = sqlx::query_as::<_, Role>(&format!(
            "SELECT {SELECT_COLS} FROM roles WHERE public_id = $1 AND deleted_at IS NULL"
        ))
        .bind(public_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(role)
    }

    pub async fn get_id_by_public_id(&self, public_id: &str) -> Result<Option<Uuid>, AppError> {
        let row: Option<(Uuid,)> =
            sqlx::query_as("SELECT id FROM roles WHERE public_id = $1 AND deleted_at IS NULL")
                .bind(public_id)
                .fetch_optional(&self.pool)
                .await?;

        Ok(row.map(|(id,)| id))
    }

    pub async fn list(
        &self,
        tenant_id: Option<Uuid>,
        is_system: Option<bool>,
        limit: i64,
        cursor: Option<String>,
    ) -> Result<(Vec<Role>, Option<String>), AppError> {
        debug!(tenant_id = ?tenant_id, limit = limit, "listing roles");

        let cols = SELECT_COLS;
        let mut qb = QueryBuilder::<sqlx::Postgres>::new(format!(
            "SELECT {cols} FROM roles WHERE deleted_at IS NULL"
        ));

        if let Some(tid) = tenant_id {
            qb.push(" AND tenant_id = ").push_bind(tid);
        }
        if let Some(sys) = is_system {
            qb.push(" AND is_system = ").push_bind(sys);
        }
        if let Some(ref c) = cursor {
            qb.push(" AND public_id > ").push_bind(c.clone());
        }

        qb.push(" ORDER BY public_id ASC LIMIT ")
            .push_bind(limit + 1);

        let mut roles: Vec<Role> = qb.build_query_as::<Role>().fetch_all(&self.pool).await?;

        let next_cursor = if roles.len() as i64 > limit {
            roles.pop().map(|r| r.public_id)
        } else {
            None
        };

        Ok((roles, next_cursor))
    }

    pub async fn update(
        &self,
        public_id: &str,
        name: Option<String>,
        description: Option<String>,
        ctx: RequestContext,
    ) -> Result<Option<Role>, AppError> {
        debug!(public_id = %public_id, "updating role");

        let role = sqlx::query_as::<_, Role>(&format!(
            r#"
            UPDATE roles SET
                name        = COALESCE($1, name),
                description = COALESCE($2, description),
                updated_by  = $3,
                version     = version + 1,
                updated_at  = NOW()
            WHERE public_id = $4 AND deleted_at IS NULL AND is_system = false
            RETURNING {SELECT_COLS}
            "#
        ))
        .bind(&name)
        .bind(&description)
        .bind(ctx.created_by)
        .bind(public_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(role)
    }

    pub async fn delete(&self, public_id: &str, ctx: RequestContext) -> Result<bool, AppError> {
        debug!(public_id = %public_id, "soft-deleting role");

        let result = sqlx::query(
            "UPDATE roles SET deleted_at = NOW(), updated_by = $1
             WHERE public_id = $2 AND deleted_at IS NULL AND is_system = false",
        )
        .bind(ctx.created_by)
        .bind(public_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    // ── Permissions ───────────────────────────────────────────────────────────

    pub async fn list_permissions(
        &self,
        role_public_id: &str,
    ) -> Result<Vec<RolePermissionRow>, AppError> {
        debug!(role_public_id = %role_public_id, "listing role permissions");

        let rows = sqlx::query_as::<_, RolePermissionRow>(
            r#"
            SELECT p.public_id AS permission_public_id, p.name AS permission_name,
                   p.resource, p.action
            FROM role_permissions rp
            JOIN roles r ON r.id = rp.role_id
            JOIN permissions p ON p.id = rp.permission_id
            WHERE r.public_id = $1 AND r.deleted_at IS NULL
            ORDER BY p.name
            "#,
        )
        .bind(role_public_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    /// Assigns a batch of permissions (by their UUIDs) to a role.
    /// Returns (newly_assigned_public_ids, already_present_public_ids).
    pub async fn assign_permissions(
        &self,
        role_id: Uuid,
        permission_ids: Vec<(Uuid, String)>, // (internal_id, public_id)
        ctx: RequestContext,
    ) -> Result<(Vec<String>, Vec<String>), AppError> {
        if permission_ids.is_empty() {
            return Ok((vec![], vec![]));
        }

        let mut assigned = Vec::new();
        let mut already_present = Vec::new();

        for (perm_id, perm_public_id) in permission_ids {
            let result = sqlx::query(
                "INSERT INTO role_permissions (role_id, permission_id, created_by)
                 VALUES ($1, $2, $3)
                 ON CONFLICT DO NOTHING",
            )
            .bind(role_id)
            .bind(perm_id)
            .bind(ctx.created_by)
            .execute(&self.pool)
            .await?;

            if result.rows_affected() > 0 {
                assigned.push(perm_public_id);
            } else {
                already_present.push(perm_public_id);
            }
        }

        Ok((assigned, already_present))
    }

    pub async fn remove_permission(
        &self,
        role_public_id: &str,
        perm_public_id: &str,
    ) -> Result<bool, AppError> {
        debug!(role_public_id = %role_public_id, perm_public_id = %perm_public_id, "removing role permission");

        let result = sqlx::query(
            r#"
            DELETE FROM role_permissions
            WHERE role_id    = (SELECT id FROM roles       WHERE public_id = $1 AND deleted_at IS NULL)
              AND permission_id = (SELECT id FROM permissions WHERE public_id = $2)
            "#,
        )
        .bind(role_public_id)
        .bind(perm_public_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Seed default roles for a newly created tenant.
    /// Creates owner, admin, developer, viewer with appropriate system permissions.
    pub async fn seed_default_roles(
        &self,
        tenant_id: Uuid,
        system_permissions: &[Permission],
        ctx: RequestContext,
    ) -> Result<(), AppError> {
        debug!(tenant_id = %tenant_id, "seeding default roles");

        let all_perm_ids: Vec<(Uuid, &str)> = system_permissions
            .iter()
            .map(|p| (p.id, p.name.as_str()))
            .collect();

        let definitions: &[(&str, &[&str])] = &[
            ("owner", &["*:*"]),
            (
                "admin",
                &[
                    "applications:read",
                    "applications:write",
                    "applications:delete",
                    "endpoints:read",
                    "endpoints:write",
                    "endpoints:delete",
                    "event_types:read",
                    "event_types:write",
                    "event_types:delete",
                    "events:read",
                    "events:send",
                    "events:delete",
                    "schedules:read",
                    "schedules:write",
                    "schedules:delete",
                    "environments:read",
                    "environments:write",
                    "api_keys:read",
                    "api_keys:write",
                    "api_keys:delete",
                    "jwt_keys:read",
                    "jwt_keys:write",
                    "jwt_keys:delete",
                    "jwt_keys:rotate",
                    "users:read",
                    "users:write",
                    "teams:read",
                    "teams:write",
                    "teams:delete",
                    "roles:read",
                    "roles:write",
                    "roles:delete",
                    "permissions:read",
                    "permissions:write",
                    "permissions:delete",
                    "invites:read",
                    "invites:write",
                    "tenant:read",
                ],
            ),
            (
                "developer",
                &[
                    "applications:read",
                    "applications:write",
                    "endpoints:read",
                    "endpoints:write",
                    "event_types:read",
                    "event_types:write",
                    "events:read",
                    "events:send",
                    "schedules:read",
                    "schedules:write",
                    "environments:read",
                    "environments:write",
                    "api_keys:read",
                    "api_keys:write",
                    "jwt_keys:read",
                ],
            ),
            (
                "viewer",
                &[
                    "applications:read",
                    "endpoints:read",
                    "event_types:read",
                    "events:read",
                    "schedules:read",
                    "environments:read",
                    "api_keys:read",
                    "jwt_keys:read",
                    "users:read",
                    "teams:read",
                    "roles:read",
                    "permissions:read",
                    "tenant:read",
                ],
            ),
        ];

        for (role_name, perm_names) in definitions {
            let role = self
                .create(tenant_id, role_name.to_string(), None, true, ctx)
                .await?;

            let matching: Vec<(Uuid, String)> = all_perm_ids
                .iter()
                .filter(|(_, name)| perm_names.contains(name))
                .map(|(id, _)| (*id, String::new()))
                .collect();

            self.assign_permissions(role.id, matching, ctx).await?;
        }

        Ok(())
    }
}
