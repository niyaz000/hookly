use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Request types ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct AssignRolesRequest {
    pub role_ids: Vec<String>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct AssignPermissionsRequest {
    pub permission_ids: Vec<String>,
    pub expires_at: Option<DateTime<Utc>>,
}

// ── Response types ─────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct AssignedRole {
    pub role_id: String,
    pub role_name: String,
    pub tenant_id: Uuid,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct AssignedRoleRow {
    pub role_public_id: String,
    pub role_name: String,
    pub tenant_id: Uuid,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

impl From<AssignedRoleRow> for AssignedRole {
    fn from(r: AssignedRoleRow) -> Self {
        Self {
            role_id: r.role_public_id,
            role_name: r.role_name,
            tenant_id: r.tenant_id,
            expires_at: r.expires_at,
            created_at: r.created_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ListAssignedRolesResponse {
    pub items: Vec<AssignedRole>,
}

#[derive(Debug, Serialize)]
pub struct BulkAssignResponse {
    pub assigned: Vec<String>,
    pub already_present: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct AssignedPermission {
    pub permission_id: String,
    pub permission_name: String,
    pub resource: String,
    pub action: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct AssignedPermissionRow {
    pub perm_public_id: String,
    pub permission_name: String,
    pub resource: String,
    pub action: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

impl From<AssignedPermissionRow> for AssignedPermission {
    fn from(r: AssignedPermissionRow) -> Self {
        Self {
            permission_id: r.perm_public_id,
            permission_name: r.permission_name,
            resource: r.resource,
            action: r.action,
            expires_at: r.expires_at,
            created_at: r.created_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ListAssignedPermissionsResponse {
    pub items: Vec<AssignedPermission>,
}

#[derive(Debug, Serialize)]
pub struct EffectivePermission {
    pub permission_id: String,
    pub name: String,
    pub resource: String,
    pub action: String,
    /// "role" or "direct"
    pub source: String,
    /// Only set when source == "role"
    pub from_role: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct EffectivePermissionRow {
    pub perm_public_id: String,
    pub perm_name: String,
    pub resource: String,
    pub action: String,
    pub source: String,
    pub from_role: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
}

impl From<EffectivePermissionRow> for EffectivePermission {
    fn from(r: EffectivePermissionRow) -> Self {
        Self {
            permission_id: r.perm_public_id,
            name: r.perm_name,
            resource: r.resource,
            action: r.action,
            source: r.source,
            from_role: r.from_role,
            expires_at: r.expires_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct EffectivePermissionsResponse {
    pub subject_id: String,
    pub tenant_id: Uuid,
    pub permissions: Vec<EffectivePermission>,
}
