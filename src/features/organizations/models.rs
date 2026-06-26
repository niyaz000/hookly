use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use uuid::Uuid;
use validator::Validate;

use crate::common::validators::{validate_not_blank, validate_slug};
use crate::error::AppError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "organization_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum OrganizationStatus {
    Active,
    Suspended,
    Inactive,
}

#[derive(sqlx::FromRow, Debug, Clone)]
pub struct Organization {
    pub id: Uuid,
    pub public_id: String,
    pub name: String,
    pub slug: String,
    pub status: OrganizationStatus,
    pub owner_email: Option<String>,
    pub plan: String,
    pub stripe_customer_id: Option<String>,
    pub external_id: Option<String>,
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
pub struct CreateOrganizationRequest {
    #[validate(custom(function = "validate_not_blank", message = "name is required"))]
    #[validate(length(min = 5, max = 255, message = "name must be 255 characters or fewer"))]
    pub name: String,
    #[validate(custom(function = "validate_not_blank", message = "slug is required"))]
    #[validate(length(min = 3, max = 64, message = "slug must be 64 characters or fewer"))]
    #[validate(custom(
        function = "validate_slug",
        message = "slug must be lowercase alphanumeric and hyphens, not starting or ending with a hyphen"
    ))]
    pub slug: String,
    #[validate(custom(function = "validate_not_blank", message = "owner_email is required"))]
    #[validate(email(message = "owner_email is not a valid email address"))]
    #[validate(length(max = 64, message = "owner_email must be 64 characters or fewer"))]
    pub owner_email: String,
    #[validate(length(max = 64, message = "external_id must be 64 characters or fewer"))]
    pub external_id: Option<String>,
    pub tags: Option<HashMap<String, String>>,
}

impl CreateOrganizationRequest {
    pub fn validate(&self) -> Result<(), AppError> {
        Validate::validate(self).map_err(AppError::from)
    }
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateOrganizationRequest {
    #[validate(custom(function = "validate_not_blank", message = "name cannot be empty"))]
    #[validate(length(max = 255, message = "name must be 255 characters or fewer"))]
    pub name: Option<String>,
    #[validate(custom(function = "validate_not_blank", message = "slug cannot be empty"))]
    #[validate(length(max = 64, message = "slug must be 64 characters or fewer"))]
    #[validate(custom(
        function = "validate_slug",
        message = "slug must be lowercase alphanumeric and hyphens, not starting or ending with a hyphen"
    ))]
    pub slug: Option<String>,
    #[validate(email(message = "owner_email is not a valid email address"))]
    #[validate(length(max = 64, message = "owner_email must be 64 characters or fewer"))]
    pub owner_email: Option<String>,
    #[validate(length(max = 64, message = "external_id must be 64 characters or fewer"))]
    pub external_id: Option<String>,
    pub tags: Option<HashMap<String, String>>,
}

impl UpdateOrganizationRequest {
    pub fn validate(&self) -> Result<(), AppError> {
        Validate::validate(self).map_err(AppError::from)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OrganizationResponse {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub status: OrganizationStatus,
    pub owner_email: Option<String>,
    pub external_id: Option<String>,
    pub tags: HashMap<String, String>,
    pub created_by: Uuid,
    pub updated_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Organization> for OrganizationResponse {
    fn from(org: Organization) -> Self {
        Self {
            id: org.public_id,
            name: org.name,
            slug: org.slug,
            status: org.status,
            owner_email: org.owner_email,
            external_id: org.external_id,
            tags: org.tags.0,
            created_by: org.created_by,
            updated_by: org.updated_by,
            created_at: org.created_at,
            updated_at: org.updated_at,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ListOrganizationsQuery {
    pub limit: Option<i64>,
    pub cursor: Option<String>,
    pub status: Option<OrganizationStatus>,
    pub tags: Option<HashMap<String, String>>,
}

#[derive(Debug, Serialize)]
pub struct ListOrganizationsResponse {
    pub items: Vec<OrganizationResponse>,
    pub next_cursor: Option<String>,
    pub limit: i64,
}
