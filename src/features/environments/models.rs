use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use uuid::Uuid;

use crate::error::{AppError, FieldError};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "environment_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum EnvironmentStatus {
    Active,
    Disabled,
}

#[derive(sqlx::FromRow, Debug, Clone)]
pub struct Environment {
    pub id: Uuid,
    pub public_id: String,
    pub tenant_id: Uuid,
    #[sqlx(default)]
    pub tenant_public_id: Option<String>,
    #[sqlx(default)]
    pub organization_public_id: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub status: EnvironmentStatus,
    pub tags: Json<HashMap<String, String>>,
    pub version: i32,
    pub created_by: Uuid,
    pub updated_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[sqlx(default)]
    pub created_by_public_id: Option<String>,
    #[sqlx(default)]
    pub updated_by_public_id: Option<String>,
}

// ── Request types ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateEnvironmentRequest {
    pub name: String,
    pub description: Option<String>,
    pub tags: Option<HashMap<String, String>>,
}

impl CreateEnvironmentRequest {
    pub fn normalize(mut self) -> Self {
        self.name = self.name.trim().to_lowercase();
        self
    }

    pub fn validate_all(&self) -> Result<(), AppError> {
        let name = self.name.trim();
        if name.len() < 3 {
            return Err(AppError::Validation(vec![FieldError::new(
                "name",
                "min_length",
                "name must be at least 3 characters",
            )]));
        }
        if name.len() > 64 {
            return Err(AppError::Validation(vec![FieldError::new(
                "name",
                "max_length",
                "name must be 64 characters or fewer",
            )]));
        }
        if !name.chars().next().map(|c| c.is_ascii_lowercase()).unwrap_or(false) {
            return Err(AppError::Validation(vec![FieldError::new(
                "name",
                "invalid_format",
                "name must start with a lowercase letter",
            )]));
        }
        if !name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-') {
            return Err(AppError::Validation(vec![FieldError::new(
                "name",
                "invalid_format",
                "name may only contain lowercase letters, digits, hyphens, and underscores",
            )]));
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
pub struct UpdateEnvironmentRequest {
    pub tags: Option<HashMap<String, String>>,
}

#[derive(Debug, Deserialize)]
pub struct ListEnvironmentsQuery {
    pub status: Option<EnvironmentStatus>,
    pub limit: Option<i64>,
    pub cursor: Option<String>,
    pub tags: Option<HashMap<String, String>>,
}

// ── Response types ─────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct EnvironmentResponse {
    pub id: String,
    pub organization_id: String,
    pub tenant_id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub status: EnvironmentStatus,
    pub tags: HashMap<String, String>,
    pub created_by: String,
    pub updated_by: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Environment> for EnvironmentResponse {
    fn from(e: Environment) -> Self {
        Self {
            id: e.public_id,
            organization_id: e.organization_public_id.unwrap_or_default(),
            tenant_id: e.tenant_public_id.unwrap_or_else(|| e.tenant_id.to_string()),
            name: e.name,
            description: e.description,
            status: e.status,
            tags: e.tags.0,
            created_by: e.created_by_public_id.unwrap_or_else(|| e.created_by.to_string()),
            updated_by: e.updated_by_public_id.unwrap_or_else(|| e.updated_by.to_string()),
            created_at: e.created_at,
            updated_at: e.updated_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ListEnvironmentsResponse {
    pub items: Vec<EnvironmentResponse>,
    pub next_cursor: Option<String>,
    pub limit: i64,
}
