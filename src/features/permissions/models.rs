use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{AppError, FieldError};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "permission_type", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum PermissionType {
    System,
    Custom,
}

#[derive(sqlx::FromRow, Debug, Clone)]
pub struct Permission {
    pub id: Uuid,
    pub public_id: String,
    pub tenant_id: Option<Uuid>,
    pub name: String,
    pub description: Option<String>,
    pub perm_type: PermissionType,
    pub resource: String,
    pub action: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ── Request types ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreatePermissionRequest {
    pub tenant_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub resource: String,
    pub action: String,
}

impl CreatePermissionRequest {
    pub fn normalize(mut self) -> Self {
        self.name = self.name.trim().to_lowercase();
        self.description =
            self.description.map(|d| d.trim().to_owned()).filter(|d| !d.is_empty());
        self.resource = self.resource.trim().to_lowercase();
        self.action = self.action.trim().to_lowercase();
        self
    }

    pub fn validate_all(&self) -> Result<(), AppError> {
        let mut errors: Vec<FieldError> = Vec::new();
        if let Some(e) = validate_name_part("resource", &self.resource) {
            errors.push(e);
        }
        if let Some(e) = validate_name_part("action", &self.action) {
            errors.push(e);
        }
        if !errors.is_empty() {
            return Err(AppError::Validation(errors));
        }
        Ok(())
    }
}

fn validate_name_part(field: &'static str, value: &str) -> Option<FieldError> {
    if value.is_empty() {
        return Some(FieldError::new(field, "required", format!("{field} is required")));
    }
    if value.len() > 64 {
        return Some(FieldError::new(
            field,
            "max_length",
            format!("{field} must be 64 characters or fewer"),
        ));
    }
    if !value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '*')
    {
        return Some(FieldError::new(
            field,
            "invalid_format",
            format!("{field} may only contain letters, digits, hyphens, underscores, or *"),
        ));
    }
    None
}

#[derive(Debug, Deserialize)]
pub struct UpdatePermissionRequest {
    pub description: Option<String>,
}

impl UpdatePermissionRequest {
    pub fn normalize(mut self) -> Self {
        self.description =
            self.description.map(|d| d.trim().to_owned()).filter(|d| !d.is_empty());
        self
    }
}

#[derive(Debug, Deserialize)]
pub struct ListPermissionsQuery {
    pub tenant_id: Option<Uuid>,
    pub perm_type: Option<PermissionType>,
    pub resource: Option<String>,
    pub limit: Option<i64>,
    pub cursor: Option<String>,
}

// ── Response types ─────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct PermissionResponse {
    pub id: String,
    pub tenant_id: Option<Uuid>,
    pub name: String,
    pub description: Option<String>,
    #[serde(rename = "type")]
    pub perm_type: PermissionType,
    pub resource: String,
    pub action: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Permission> for PermissionResponse {
    fn from(p: Permission) -> Self {
        Self {
            id: p.public_id,
            tenant_id: p.tenant_id,
            name: p.name,
            description: p.description,
            perm_type: p.perm_type,
            resource: p.resource,
            action: p.action,
            created_at: p.created_at,
            updated_at: p.updated_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ListPermissionsResponse {
    pub items: Vec<PermissionResponse>,
    pub next_cursor: Option<String>,
    pub limit: i64,
}
