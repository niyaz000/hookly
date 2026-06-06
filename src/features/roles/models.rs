use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{AppError, FieldError};

#[derive(sqlx::FromRow, Debug, Clone)]
pub struct Role {
    pub id: Uuid,
    pub public_id: String,
    pub tenant_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub is_system: bool,
    pub version: i32,
    pub created_by: Uuid,
    pub updated_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ── Request types ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateRoleRequest {
    pub tenant_id: Uuid,
    pub name: String,
    pub description: Option<String>,
}

impl CreateRoleRequest {
    pub fn normalize(mut self) -> Self {
        self.name = self.name.trim().to_owned();
        self.description =
            self.description.map(|d| d.trim().to_owned()).filter(|d| !d.is_empty());
        self
    }

    pub fn validate_all(&self) -> Result<(), AppError> {
        let mut errors: Vec<FieldError> = Vec::new();
        if self.name.is_empty() {
            errors.push(FieldError::new("name", "required", "name is required"));
        } else if self.name.len() > 128 {
            errors.push(FieldError::new(
                "name",
                "max_length",
                "name must be 128 characters or fewer",
            ));
        }
        if !errors.is_empty() {
            return Err(AppError::Validation(errors));
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
pub struct UpdateRoleRequest {
    pub name: Option<String>,
    pub description: Option<String>,
}

impl UpdateRoleRequest {
    pub fn normalize(mut self) -> Self {
        self.name = self.name.map(|n| n.trim().to_owned()).filter(|n| !n.is_empty());
        self.description =
            self.description.map(|d| d.trim().to_owned()).filter(|d| !d.is_empty());
        self
    }
}

#[derive(Debug, Deserialize)]
pub struct ListRolesQuery {
    pub tenant_id: Option<Uuid>,
    pub is_system: Option<bool>,
    pub limit: Option<i64>,
    pub cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AssignPermissionsRequest {
    pub permission_ids: Vec<String>,
}

// ── Response types ─────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct RoleResponse {
    pub id: String,
    pub tenant_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub is_system: bool,
    pub created_by: Uuid,
    pub updated_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Role> for RoleResponse {
    fn from(r: Role) -> Self {
        Self {
            id: r.public_id,
            tenant_id: r.tenant_id,
            name: r.name,
            description: r.description,
            is_system: r.is_system,
            created_by: r.created_by,
            updated_by: r.updated_by,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ListRolesResponse {
    pub items: Vec<RoleResponse>,
    pub next_cursor: Option<String>,
    pub limit: i64,
}

#[derive(Debug, Serialize)]
pub struct AssignPermissionsResponse {
    pub assigned: Vec<String>,
    pub already_present: Vec<String>,
}

#[derive(Debug, sqlx::FromRow, Serialize)]
pub struct RolePermissionRow {
    pub permission_public_id: String,
    pub permission_name: String,
    pub resource: String,
    pub action: String,
}

#[derive(Debug, Serialize)]
pub struct ListRolePermissionsResponse {
    pub role_id: String,
    pub items: Vec<RolePermissionRow>,
}
