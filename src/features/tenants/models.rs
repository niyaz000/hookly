use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use uuid::Uuid;
use validator::Validate;

use crate::common::validators::validate_not_blank;
use crate::error::AppError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "tenant_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum TenantStatus {
    Active,
    Suspended,
    Inactive,
}

#[derive(sqlx::FromRow, Debug, Clone)]
pub struct Tenant {
    pub id: Uuid,
    pub public_id: String,
    pub organization_id: Uuid,
    pub organization_public_id: String,
    pub name: String,
    pub description: Option<String>,
    pub status: TenantStatus,
    pub tags: Json<HashMap<String, String>>,
    pub metadata: Json<HashMap<String, String>>,
    pub settings: Json<HashMap<String, String>>,
    pub created_by: Uuid,
    pub updated_by: Uuid,
    pub version: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateTenantRequest {
    pub organization_id: String,
    #[validate(custom(function = "validate_not_blank", message = "name is required"))]
    #[validate(length(max = 255, message = "name must be 255 characters or fewer"))]
    pub name: String,
    pub description: Option<String>,
    pub tags: Option<HashMap<String, String>>,
    pub metadata: Option<HashMap<String, String>>,
    pub settings: Option<HashMap<String, String>>,
}

impl CreateTenantRequest {
    pub fn validate(&self) -> Result<(), AppError> {
        Validate::validate(self).map_err(AppError::from)
    }
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateTenantRequest {
    #[validate(custom(function = "validate_not_blank", message = "name cannot be empty"))]
    #[validate(length(max = 255, message = "name must be 255 characters or fewer"))]
    pub name: Option<String>,
    pub description: Option<String>,
    pub tags: Option<HashMap<String, String>>,
    pub metadata: Option<HashMap<String, String>>,
    pub settings: Option<HashMap<String, String>>,
}

impl UpdateTenantRequest {
    pub fn validate(&self) -> Result<(), AppError> {
        Validate::validate(self).map_err(AppError::from)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TenantResponse {
    pub id: String,
    pub organization_id: String,
    pub name: String,
    pub description: Option<String>,
    pub status: TenantStatus,
    pub tags: HashMap<String, String>,
    pub created_by: Uuid,
    pub updated_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Tenant> for TenantResponse {
    fn from(t: Tenant) -> Self {
        Self {
            id: t.public_id,
            organization_id: t.organization_public_id,
            name: t.name,
            description: t.description,
            status: t.status,
            tags: t.tags.0,
            created_by: t.created_by,
            updated_by: t.updated_by,
            created_at: t.created_at,
            updated_at: t.updated_at,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ListTenantsQuery {
    pub limit: Option<i64>,
    pub cursor: Option<String>,
    pub status: Option<TenantStatus>,
    pub organization_id: Option<Uuid>,
    pub tags: Option<HashMap<String, String>>,
}

#[derive(Debug, Serialize)]
pub struct ListTenantsResponse {
    pub items: Vec<TenantResponse>,
    pub next_cursor: Option<String>,
    pub limit: i64,
}
