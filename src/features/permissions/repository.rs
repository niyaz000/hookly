use sqlx::{QueryBuilder};
use tracing::debug;
use uuid::Uuid;

use crate::common::NanoId;
use crate::error::AppError;

use super::models::{Permission, PermissionType};

const SELECT_COLS: &str = "
    id, public_id, tenant_id, name, description,
    perm_type, resource, action, created_at, updated_at
";

#[derive(Clone)]
pub struct PermissionRepository {
    pool: crate::common::CountingPool,
}

impl PermissionRepository {
    pub fn new(pool: crate::common::CountingPool) -> Self {
        Self { pool }
    }

    pub async fn create(
        &self,
        tenant_id: Uuid,
        name: String,
        description: Option<String>,
        resource: String,
        action: String,
    ) -> Result<Permission, AppError> {
        let id = Uuid::now_v7();
        let public_id = format!("per_{}", NanoId::new());

        debug!(public_id = %public_id, tenant_id = %tenant_id, name = %name, "inserting permission");

        let perm = sqlx::query_as::<_, Permission>(&format!(
            r#"
            INSERT INTO permissions (id, public_id, tenant_id, name, description, perm_type, resource, action)
            VALUES ($1, $2, $3, $4, $5, 'custom', $6, $7)
            RETURNING {SELECT_COLS}
            "#
        ))
        .bind(id)
        .bind(&public_id)
        .bind(tenant_id)
        .bind(&name)
        .bind(&description)
        .bind(&resource)
        .bind(&action)
        .fetch_one(&self.pool)
        .await?;

        Ok(perm)
    }

    pub async fn get_by_public_id(&self, public_id: &str) -> Result<Option<Permission>, AppError> {
        debug!(public_id = %public_id, "querying permission");

        let perm = sqlx::query_as::<_, Permission>(&format!(
            "SELECT {SELECT_COLS} FROM permissions WHERE public_id = $1"
        ))
        .bind(public_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(perm)
    }

    pub async fn list(
        &self,
        tenant_id: Option<Uuid>,
        perm_type: Option<PermissionType>,
        resource: Option<String>,
        limit: i64,
        cursor: Option<String>,
    ) -> Result<(Vec<Permission>, Option<String>), AppError> {
        debug!(tenant_id = ?tenant_id, limit = limit, "listing permissions");

        let cols = SELECT_COLS;
        let mut qb = QueryBuilder::<sqlx::Postgres>::new(format!(
            "SELECT {cols} FROM permissions WHERE 1=1"
        ));

        // System permissions are always visible; custom permissions are filtered by tenant
        match tenant_id {
            Some(tid) => {
                qb.push(" AND (tenant_id = ").push_bind(tid).push(" OR tenant_id IS NULL)");
            }
            None => {
                qb.push(" AND tenant_id IS NULL");
            }
        }

        if let Some(pt) = perm_type {
            qb.push(" AND perm_type = ").push_bind(pt);
        }
        if let Some(res) = resource {
            qb.push(" AND resource = ").push_bind(res);
        }
        if let Some(ref c) = cursor {
            qb.push(" AND public_id > ").push_bind(c.clone());
        }

        qb.push(" ORDER BY public_id ASC LIMIT ").push_bind(limit + 1);

        let mut perms: Vec<Permission> =
            qb.build_query_as::<Permission>().fetch_all(&self.pool).await?;

        let next_cursor = if perms.len() as i64 > limit {
            perms.pop().map(|p| p.public_id)
        } else {
            None
        };

        Ok((perms, next_cursor))
    }

    pub async fn update_description(
        &self,
        public_id: &str,
        description: Option<String>,
    ) -> Result<Option<Permission>, AppError> {
        debug!(public_id = %public_id, "updating permission description");

        let perm = sqlx::query_as::<_, Permission>(&format!(
            r#"
            UPDATE permissions
            SET description = $1, updated_at = NOW()
            WHERE public_id = $2 AND perm_type = 'custom'
            RETURNING {SELECT_COLS}
            "#
        ))
        .bind(&description)
        .bind(public_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(perm)
    }

    pub async fn delete(&self, public_id: &str) -> Result<bool, AppError> {
        debug!(public_id = %public_id, "deleting permission");

        let result =
            sqlx::query("DELETE FROM permissions WHERE public_id = $1 AND perm_type = 'custom'")
                .bind(public_id)
                .execute(&self.pool)
                .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Fetch all system permissions (used for role seeding).
    pub async fn list_system(&self) -> Result<Vec<Permission>, AppError> {
        let perms = sqlx::query_as::<_, Permission>(&format!(
            "SELECT {SELECT_COLS} FROM permissions WHERE tenant_id IS NULL ORDER BY name"
        ))
        .fetch_all(&self.pool)
        .await?;

        Ok(perms)
    }
}
