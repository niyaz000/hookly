use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use uuid::Uuid;

use crate::error::{AppError, FieldError};

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

#[derive(Debug, Deserialize)]
pub struct CreateTenantRequest {
    pub organization_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub tags: Option<HashMap<String, String>>,
    pub metadata: Option<HashMap<String, String>>,
    pub settings: Option<HashMap<String, String>>,
}

impl CreateTenantRequest {
    pub fn validate(&self) -> Result<(), AppError> {
        let mut errors = Vec::new();

        if self.name.trim().is_empty() {
            errors.push(FieldError::new("name", "required", "name is required"));
        } else if self.name.len() > 255 {
            errors.push(FieldError::new(
                "name",
                "max_length",
                "name must be 255 characters or fewer",
            ));
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(AppError::Validation(errors))
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct UpdateTenantRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub tags: Option<HashMap<String, String>>,
    pub metadata: Option<HashMap<String, String>>,
    pub settings: Option<HashMap<String, String>>,
}

impl UpdateTenantRequest {
    pub fn validate(&self) -> Result<(), AppError> {
        let mut errors = Vec::new();

        if let Some(ref n) = self.name {
            if n.trim().is_empty() {
                errors.push(FieldError::new("name", "required", "name cannot be empty"));
            } else if n.len() > 255 {
                errors.push(FieldError::new(
                    "name",
                    "max_length",
                    "name must be 255 characters or fewer",
                ));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(AppError::Validation(errors))
        }
    }
}

#[derive(Debug, Serialize)]
pub struct TenantResponse {
    pub id: String,
    pub organization_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub status: TenantStatus,
    pub tags: HashMap<String, String>,
    pub metadata: HashMap<String, String>,
    pub settings: HashMap<String, String>,
    pub created_by: Uuid,
    pub updated_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Tenant> for TenantResponse {
    fn from(t: Tenant) -> Self {
        Self {
            id: t.public_id,
            organization_id: t.organization_id,
            name: t.name,
            description: t.description,
            status: t.status,
            tags: t.tags.0,
            metadata: t.metadata.0,
            settings: t.settings.0,
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
}

#[derive(Debug, Serialize)]
pub struct ListTenantsResponse {
    pub items: Vec<TenantResponse>,
    pub next_cursor: Option<String>,
    pub limit: i64,
}
