use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use uuid::Uuid;

use crate::error::{AppError, FieldError};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "platform_webhook_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum PlatformWebhookStatus {
    Active,
    Suspended,
    Disabled,
}

#[derive(sqlx::FromRow, Debug, Clone)]
pub struct PlatformWebhook {
    pub id: Uuid,
    pub public_id: String,
    pub tenant_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub url: String,
    pub signing_secret_enc: String,
    pub status: PlatformWebhookStatus,
    pub metadata: Json<serde_json::Value>,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ── Request types ──────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreatePlatformWebhookRequest {
    pub tenant_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub url: String,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

impl CreatePlatformWebhookRequest {
    pub fn normalize(mut self) -> Self {
        self.name = self.name.trim().to_owned();
        self.description = self.description.map(|d| d.trim().to_owned()).filter(|d| !d.is_empty());
        self.url = self.url.trim().to_owned();
        self
    }

    pub fn validate_all(&self) -> Result<(), AppError> {
        let mut errors: Vec<FieldError> = Vec::new();

        if self.name.is_empty() {
            errors.push(FieldError::new("name", "required", "name is required"));
        } else if self.name.len() > 128 {
            errors.push(FieldError::new("name", "max_length", "name must be 128 characters or fewer"));
        }

        if self.url.is_empty() {
            errors.push(FieldError::new("url", "required", "url is required"));
        } else if !self.url.starts_with("https://") {
            errors.push(FieldError::new("url", "invalid_format", "url must use HTTPS"));
        } else if self.url.len() > 2048 {
            errors.push(FieldError::new("url", "max_length", "url must be 2048 characters or fewer"));
        }

        if !errors.is_empty() {
            return Err(AppError::Validation(errors));
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
pub struct UpdatePlatformWebhookRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub url: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

impl UpdatePlatformWebhookRequest {
    pub fn normalize(mut self) -> Self {
        self.name = self.name.map(|n| n.trim().to_owned()).filter(|n| !n.is_empty());
        self.description = self.description.map(|d| d.trim().to_owned()).filter(|d| !d.is_empty());
        self.url = self.url.map(|u| u.trim().to_owned()).filter(|u| !u.is_empty());
        self
    }

    pub fn validate_all(&self) -> Result<(), AppError> {
        let mut errors: Vec<FieldError> = Vec::new();

        if let Some(name) = &self.name {
            if name.len() > 128 {
                errors.push(FieldError::new("name", "max_length", "name must be 128 characters or fewer"));
            }
        }

        if let Some(url) = &self.url {
            if !url.starts_with("https://") {
                errors.push(FieldError::new("url", "invalid_format", "url must use HTTPS"));
            } else if url.len() > 2048 {
                errors.push(FieldError::new("url", "max_length", "url must be 2048 characters or fewer"));
            }
        }

        if !errors.is_empty() {
            return Err(AppError::Validation(errors));
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
pub struct ListPlatformWebhooksQuery {
    pub tenant_id: Option<Uuid>,
    pub status: Option<PlatformWebhookStatus>,
    pub limit: Option<i64>,
    pub cursor: Option<String>,
}

// ── Response types ─────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct PlatformWebhookResponse {
    pub id: String,
    pub tenant_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub url: String,
    /// Only present on create and rotate-secret responses.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signing_secret: Option<String>,
    pub status: PlatformWebhookStatus,
    pub metadata: serde_json::Value,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl PlatformWebhookResponse {
    pub fn from_webhook(w: PlatformWebhook) -> Self {
        Self {
            id: w.public_id,
            tenant_id: w.tenant_id,
            name: w.name,
            description: w.description,
            url: w.url,
            signing_secret: None,
            status: w.status,
            metadata: w.metadata.0,
            created_by: w.created_by,
            updated_by: w.updated_by,
            created_at: w.created_at,
            updated_at: w.updated_at,
        }
    }

    pub fn with_signing_secret(mut self, secret: String) -> Self {
        self.signing_secret = Some(secret);
        self
    }
}

#[derive(Debug, Serialize)]
pub struct ListPlatformWebhooksResponse {
    pub items: Vec<PlatformWebhookResponse>,
    pub next_cursor: Option<String>,
    pub limit: i64,
}
