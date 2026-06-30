
use tracing::debug;
use uuid::Uuid;

use crate::error::AppError;

use super::models::{AssignedPermissionRow, AssignedRoleRow, EffectivePermissionRow};

#[derive(Clone)]
pub struct AssignmentRepository {
    pool: crate::common::CountingPool,
}

impl AssignmentRepository {
    pub fn new(pool: crate::common::CountingPool) -> Self {
        Self { pool }
    }

    // ── User roles ─────────────────────────────────────────────────────────────

    pub async fn list_user_roles(
        &self,
        user_public_id: &str,
    ) -> Result<Vec<AssignedRoleRow>, AppError> {
        debug!(user_public_id = %user_public_id, "listing user roles");

        let rows = sqlx::query_as::<_, AssignedRoleRow>(
            r#"
            SELECT r.public_id AS role_public_id, r.name AS role_name,
                   ur.tenant_id, ur.expires_at, ur.created_at
            FROM user_roles ur
            JOIN roles r ON r.id = ur.role_id AND r.deleted_at IS NULL
            WHERE ur.user_public_id = $1
              AND (ur.expires_at IS NULL OR ur.expires_at > NOW())
            ORDER BY r.name
            "#,
        )
        .bind(user_public_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    pub async fn assign_user_roles(
        &self,
        user_public_id: &str,
        role_ids: Vec<(Uuid, String)>, // (internal_id, public_id)
        tenant_id: Uuid,
        expires_at: Option<chrono::DateTime<chrono::Utc>>,
        created_by: Uuid,
    ) -> Result<(Vec<String>, Vec<String>), AppError> {
        let mut assigned = Vec::new();
        let mut already_present = Vec::new();

        for (role_id, role_public_id) in role_ids {
            let result = sqlx::query(
                r#"
                INSERT INTO user_roles (user_public_id, role_id, tenant_id, expires_at, created_by)
                VALUES ($1, $2, $3, $4, $5)
                ON CONFLICT (user_public_id, role_id) DO NOTHING
                "#,
            )
            .bind(user_public_id)
            .bind(role_id)
            .bind(tenant_id)
            .bind(expires_at)
            .bind(created_by)
            .execute(&self.pool)
            .await?;

            if result.rows_affected() > 0 {
                assigned.push(role_public_id);
            } else {
                already_present.push(role_public_id);
            }
        }

        Ok((assigned, already_present))
    }

    pub async fn remove_user_role(
        &self,
        user_public_id: &str,
        role_public_id: &str,
    ) -> Result<bool, AppError> {
        debug!(user_public_id = %user_public_id, role_public_id = %role_public_id, "removing user role");

        let result = sqlx::query(
            r#"
            DELETE FROM user_roles
            WHERE user_public_id = $1
              AND role_id = (SELECT id FROM roles WHERE public_id = $2 AND deleted_at IS NULL)
            "#,
        )
        .bind(user_public_id)
        .bind(role_public_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    // ── User permissions ───────────────────────────────────────────────────────

    pub async fn list_user_permissions(
        &self,
        user_public_id: &str,
    ) -> Result<Vec<AssignedPermissionRow>, AppError> {
        debug!(user_public_id = %user_public_id, "listing user direct permissions");

        let rows = sqlx::query_as::<_, AssignedPermissionRow>(
            r#"
            SELECT p.public_id AS perm_public_id, p.name AS permission_name,
                   p.resource, p.action, up.expires_at, up.created_at
            FROM user_permissions up
            JOIN permissions p ON p.id = up.permission_id
            WHERE up.user_public_id = $1
              AND (up.expires_at IS NULL OR up.expires_at > NOW())
            ORDER BY p.name
            "#,
        )
        .bind(user_public_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    pub async fn assign_user_permissions(
        &self,
        user_public_id: &str,
        permission_ids: Vec<(Uuid, String)>, // (internal_id, public_id)
        tenant_id: Uuid,
        expires_at: Option<chrono::DateTime<chrono::Utc>>,
        created_by: Uuid,
    ) -> Result<(Vec<String>, Vec<String>), AppError> {
        let mut assigned = Vec::new();
        let mut already_present = Vec::new();

        for (perm_id, perm_public_id) in permission_ids {
            let result = sqlx::query(
                r#"
                INSERT INTO user_permissions (user_public_id, permission_id, tenant_id, expires_at, created_by)
                VALUES ($1, $2, $3, $4, $5)
                ON CONFLICT (user_public_id, permission_id) DO NOTHING
                "#,
            )
            .bind(user_public_id)
            .bind(perm_id)
            .bind(tenant_id)
            .bind(expires_at)
            .bind(created_by)
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

    pub async fn remove_user_permission(
        &self,
        user_public_id: &str,
        perm_public_id: &str,
    ) -> Result<bool, AppError> {
        debug!(user_public_id = %user_public_id, perm_public_id = %perm_public_id, "removing user permission");

        let result = sqlx::query(
            r#"
            DELETE FROM user_permissions
            WHERE user_public_id = $1
              AND permission_id = (SELECT id FROM permissions WHERE public_id = $2)
            "#,
        )
        .bind(user_public_id)
        .bind(perm_public_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn get_user_effective_permissions(
        &self,
        user_public_id: &str,
        tenant_id: Uuid,
    ) -> Result<Vec<EffectivePermissionRow>, AppError> {
        debug!(user_public_id = %user_public_id, tenant_id = %tenant_id, "computing effective permissions");

        let rows = sqlx::query_as::<_, EffectivePermissionRow>(
            r#"
            SELECT p.public_id AS perm_public_id, p.name AS perm_name,
                   p.resource, p.action,
                   'role'::TEXT AS source,
                   r.public_id  AS from_role,
                   ur.expires_at
            FROM user_roles ur
            JOIN roles r           ON r.id = ur.role_id AND r.deleted_at IS NULL
            JOIN role_permissions rp ON rp.role_id = ur.role_id
            JOIN permissions p     ON p.id = rp.permission_id
            WHERE ur.user_public_id = $1
              AND ur.tenant_id = $2
              AND (ur.expires_at IS NULL OR ur.expires_at > NOW())

            UNION ALL

            SELECT p.public_id AS perm_public_id, p.name AS perm_name,
                   p.resource, p.action,
                   'direct'::TEXT AS source,
                   NULL::TEXT     AS from_role,
                   up.expires_at
            FROM user_permissions up
            JOIN permissions p ON p.id = up.permission_id
            WHERE up.user_public_id = $1
              AND up.tenant_id = $2
              AND (up.expires_at IS NULL OR up.expires_at > NOW())

            ORDER BY perm_name, source
            "#,
        )
        .bind(user_public_id)
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    // ── API key roles ──────────────────────────────────────────────────────────

    pub async fn list_api_key_roles(
        &self,
        api_key_public_id: &str,
    ) -> Result<Vec<AssignedRoleRow>, AppError> {
        debug!(api_key_public_id = %api_key_public_id, "listing api key roles");

        let rows = sqlx::query_as::<_, AssignedRoleRow>(
            r#"
            SELECT r.public_id AS role_public_id, r.name AS role_name,
                   r.tenant_id, NULL::TIMESTAMPTZ AS expires_at, akr.created_at
            FROM api_key_roles akr
            JOIN roles r ON r.id = akr.role_id AND r.deleted_at IS NULL
            WHERE akr.api_key_public_id = $1
            ORDER BY r.name
            "#,
        )
        .bind(api_key_public_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    pub async fn assign_api_key_roles(
        &self,
        api_key_public_id: &str,
        role_ids: Vec<(Uuid, String)>,
        created_by: Uuid,
    ) -> Result<(Vec<String>, Vec<String>), AppError> {
        let mut assigned = Vec::new();
        let mut already_present = Vec::new();

        for (role_id, role_public_id) in role_ids {
            let result = sqlx::query(
                r#"
                INSERT INTO api_key_roles (api_key_public_id, role_id, created_by)
                VALUES ($1, $2, $3)
                ON CONFLICT (api_key_public_id, role_id) DO NOTHING
                "#,
            )
            .bind(api_key_public_id)
            .bind(role_id)
            .bind(created_by)
            .execute(&self.pool)
            .await?;

            if result.rows_affected() > 0 {
                assigned.push(role_public_id);
            } else {
                already_present.push(role_public_id);
            }
        }

        Ok((assigned, already_present))
    }

    pub async fn remove_api_key_role(
        &self,
        api_key_public_id: &str,
        role_public_id: &str,
    ) -> Result<bool, AppError> {
        let result = sqlx::query(
            r#"
            DELETE FROM api_key_roles
            WHERE api_key_public_id = $1
              AND role_id = (SELECT id FROM roles WHERE public_id = $2 AND deleted_at IS NULL)
            "#,
        )
        .bind(api_key_public_id)
        .bind(role_public_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    // ── API key permissions ────────────────────────────────────────────────────

    pub async fn list_api_key_permissions(
        &self,
        api_key_public_id: &str,
    ) -> Result<Vec<AssignedPermissionRow>, AppError> {
        debug!(api_key_public_id = %api_key_public_id, "listing api key direct permissions");

        let rows = sqlx::query_as::<_, AssignedPermissionRow>(
            r#"
            SELECT p.public_id AS perm_public_id, p.name AS permission_name,
                   p.resource, p.action, akp.expires_at, akp.created_at
            FROM api_key_permissions akp
            JOIN permissions p ON p.id = akp.permission_id
            WHERE akp.api_key_public_id = $1
              AND (akp.expires_at IS NULL OR akp.expires_at > NOW())
            ORDER BY p.name
            "#,
        )
        .bind(api_key_public_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    pub async fn assign_api_key_permissions(
        &self,
        api_key_public_id: &str,
        permission_ids: Vec<(Uuid, String)>,
        expires_at: Option<chrono::DateTime<chrono::Utc>>,
        created_by: Uuid,
    ) -> Result<(Vec<String>, Vec<String>), AppError> {
        let mut assigned = Vec::new();
        let mut already_present = Vec::new();

        for (perm_id, perm_public_id) in permission_ids {
            let result = sqlx::query(
                r#"
                INSERT INTO api_key_permissions (api_key_public_id, permission_id, expires_at, created_by)
                VALUES ($1, $2, $3, $4)
                ON CONFLICT (api_key_public_id, permission_id) DO NOTHING
                "#,
            )
            .bind(api_key_public_id)
            .bind(perm_id)
            .bind(expires_at)
            .bind(created_by)
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

    pub async fn remove_api_key_permission(
        &self,
        api_key_public_id: &str,
        perm_public_id: &str,
    ) -> Result<bool, AppError> {
        let result = sqlx::query(
            r#"
            DELETE FROM api_key_permissions
            WHERE api_key_public_id = $1
              AND permission_id = (SELECT id FROM permissions WHERE public_id = $2)
            "#,
        )
        .bind(api_key_public_id)
        .bind(perm_public_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }
}
