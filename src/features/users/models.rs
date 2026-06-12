use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use uuid::Uuid;
use validator::Validate;

use crate::common::validators::validate_not_blank;
use crate::error::AppError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "user_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum UserStatus {
    Active,
    Suspended,
    Inactive,
    Locked,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct User {
    pub id: Uuid,
    pub public_id: String,
    pub organization_id: Uuid,
    pub tenant_id: Uuid,
    pub email: String,
    pub phone: Option<String>,
    pub status: UserStatus,
    pub email_verified_at: Option<DateTime<Utc>>,
    pub phone_verified_at: Option<DateTime<Utc>>,
    pub last_active_at: Option<DateTime<Utc>>,
    pub metadata: Json<HashMap<String, String>>,
    pub tags: Json<HashMap<String, String>>,
    pub settings: Json<HashMap<String, String>>,
    #[allow(dead_code)]
    pub password_hash: Option<String>,
    pub version: i32,
    pub created_by: Uuid,
    pub updated_by: Uuid,
    pub request_id: Uuid,
    pub locked_until: Option<DateTime<Utc>>,
    pub login_count: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateUserRequest {
    pub organization_id: String,
    pub tenant_id: String,
    #[validate(custom(function = "validate_not_blank", message = "email is required"))]
    #[validate(email(message = "email is not a valid email address"))]
    #[validate(length(max = 64, message = "email must be 64 characters or fewer"))]
    pub email: String,
    #[validate(length(max = 13, message = "phone must be 13 characters or fewer"))]
    pub phone: Option<String>,
    pub metadata: Option<HashMap<String, String>>,
    pub tags: Option<HashMap<String, String>>,
    pub settings: Option<HashMap<String, String>>,
    pub password_hash: Option<String>,
}

impl CreateUserRequest {
    pub fn validate(&self) -> Result<(), AppError> {
        Validate::validate(self).map_err(AppError::from)
    }
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateUserRequest {
    #[validate(length(max = 13, message = "phone must be 13 characters or fewer"))]
    pub phone: Option<String>,
    pub metadata: Option<HashMap<String, String>>,
    pub tags: Option<HashMap<String, String>>,
    pub settings: Option<HashMap<String, String>>,
}

impl UpdateUserRequest {
    pub fn validate(&self) -> Result<(), AppError> {
        Validate::validate(self).map_err(AppError::from)
    }
}

#[derive(Debug, Deserialize)]
pub struct LockUserRequest {
    pub locked_until: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserResponse {
    pub id: String,
    pub organization_id: Uuid,
    pub tenant_id: Uuid,
    pub email: String,
    pub phone: Option<String>,
    pub status: UserStatus,
    pub email_verified_at: Option<DateTime<Utc>>,
    pub phone_verified_at: Option<DateTime<Utc>>,
    pub last_active_at: Option<DateTime<Utc>>,
    pub metadata: HashMap<String, String>,
    pub tags: HashMap<String, String>,
    pub settings: HashMap<String, String>,
    pub created_by: Uuid,
    pub updated_by: Uuid,
    pub request_id: Uuid,
    pub locked_until: Option<DateTime<Utc>>,
    pub login_count: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<User> for UserResponse {
    fn from(u: User) -> Self {
        Self {
            id: u.public_id,
            organization_id: u.organization_id,
            tenant_id: u.tenant_id,
            email: u.email,
            phone: u.phone,
            status: u.status,
            email_verified_at: u.email_verified_at,
            phone_verified_at: u.phone_verified_at,
            last_active_at: u.last_active_at,
            metadata: u.metadata.0,
            tags: u.tags.0,
            settings: u.settings.0,
            created_by: u.created_by,
            updated_by: u.updated_by,
            request_id: u.request_id,
            locked_until: u.locked_until,
            login_count: u.login_count,
            created_at: u.created_at,
            updated_at: u.updated_at,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ListUsersQuery {
    pub limit: Option<i64>,
    pub cursor: Option<String>,
    pub status: Option<UserStatus>,
    pub organization_id: Option<String>,
    pub tenant_id: Option<String>,
    pub tags: Option<HashMap<String, String>>,
}

#[derive(Debug, Serialize)]
pub struct ListUsersResponse {
    pub items: Vec<UserResponse>,
    pub next_cursor: Option<String>,
    pub limit: i64,
}
