use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use uuid::Uuid;
use validator::Validate;

use crate::common::validators::{validate_future_date, validate_not_blank};
use crate::error::AppError;

// --- DB row structs ---

#[allow(dead_code)]
#[derive(sqlx::FromRow, Debug, Clone)]
pub struct InviteRow {
    pub id: Uuid,
    pub public_id: String,
    pub tenant_id: Uuid,
    pub organization_id: Uuid,
    pub user_email: String,
    pub role: String,
    pub status: String,
    pub token_hash: String,
    pub tags: Json<serde_json::Value>,
    pub metadata: Json<serde_json::Value>,
    pub created_by: Uuid,
    pub request_id: Uuid,
    pub version: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub accepted_at: Option<DateTime<Utc>>,
    pub expires_at: DateTime<Utc>,
}

#[allow(dead_code)]
#[derive(sqlx::FromRow, Debug, Clone)]
pub struct TenantMemberRow {
    pub id: Uuid,
    pub public_id: String,
    pub tenant_id: Uuid,
    pub organization_id: Uuid,
    pub invite_id: Uuid,
    pub invite_public_id: String,
    pub user_email: String,
    pub user_id: Option<Uuid>,
    pub role: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

// --- Request types ---

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateInviteRequest {
    #[validate(custom(function = "validate_not_blank", message = "user_email is required"))]
    #[validate(email(message = "user_email is not a valid email address"))]
    #[validate(length(max = 255, message = "user_email must be 255 characters or fewer"))]
    pub user_email: String,
    #[validate(custom(function = "validate_not_blank", message = "role is required"))]
    #[validate(length(max = 50, message = "role must be 50 characters or fewer"))]
    pub role: String,
    pub tenant_id: Uuid,
    pub organization_id: Uuid,
    #[validate(custom(function = "validate_future_date", message = "expires_at must be a future date"))]
    pub expires_at: Option<DateTime<Utc>>,
    pub tags: Option<serde_json::Value>,
    pub metadata: Option<serde_json::Value>,
    pub created_by: Uuid,
}

impl CreateInviteRequest {
    pub fn validate(&self) -> Result<(), AppError> {
        Validate::validate(self).map_err(AppError::from)
    }
}

#[derive(Debug, Deserialize)]
pub struct VerifyInviteRequest {
    pub token: String,
}

#[derive(Debug, Deserialize)]
pub struct AcceptInviteRequest {
    pub token: String,
}

#[derive(Debug, Deserialize)]
pub struct ListInvitesQuery {
    pub limit: Option<i64>,
    pub cursor: Option<String>,
    pub tenant_id: Option<Uuid>,
    pub organization_id: Option<Uuid>,
    pub status: Option<String>,
    pub user_email: Option<String>,
}

// --- Response types ---

#[derive(Debug, Serialize, Deserialize)]
pub struct InviteResponse {
    pub id: String,
    pub tenant_id: Uuid,
    pub organization_id: Uuid,
    pub user_email: String,
    pub role: String,
    pub status: String,
    pub tags: serde_json::Value,
    pub created_by: Uuid,
    pub request_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub accepted_at: Option<DateTime<Utc>>,
    pub expires_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
}

impl InviteResponse {
    pub fn from_row(r: InviteRow) -> Self {
        Self {
            id: r.public_id,
            tenant_id: r.tenant_id,
            organization_id: r.organization_id,
            user_email: r.user_email,
            role: r.role,
            status: r.status,
            tags: r.tags.0,
            created_by: r.created_by,
            request_id: r.request_id,
            created_at: r.created_at,
            updated_at: r.updated_at,
            revoked_at: r.revoked_at,
            accepted_at: r.accepted_at,
            expires_at: r.expires_at,
            token: None,
        }
    }

    pub fn with_token(mut self, token: String) -> Self {
        self.token = Some(token);
        self
    }
}

#[derive(Debug, Serialize)]
pub struct InviteVerifyResponse {
    pub id: String,
    pub user_email: String,
    pub role: String,
    pub tenant_id: Uuid,
    pub organization_id: Uuid,
    pub expires_at: DateTime<Utc>,
    pub status: String,
}

impl From<InviteRow> for InviteVerifyResponse {
    fn from(r: InviteRow) -> Self {
        Self {
            id: r.public_id,
            user_email: r.user_email,
            role: r.role,
            tenant_id: r.tenant_id,
            organization_id: r.organization_id,
            expires_at: r.expires_at,
            status: r.status,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct TenantMemberResponse {
    pub id: String,
    pub tenant_id: Uuid,
    pub organization_id: Uuid,
    pub invite_id: String,
    pub user_email: String,
    pub user_id: Option<Uuid>,
    pub role: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

impl From<TenantMemberRow> for TenantMemberResponse {
    fn from(r: TenantMemberRow) -> Self {
        Self {
            id: r.public_id,
            tenant_id: r.tenant_id,
            organization_id: r.organization_id,
            invite_id: r.invite_public_id,
            user_email: r.user_email,
            user_id: r.user_id,
            role: r.role,
            status: r.status,
            created_at: r.created_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ListInvitesResponse {
    pub items: Vec<InviteResponse>,
    pub next_cursor: Option<String>,
    pub limit: i64,
}
