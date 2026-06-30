use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{AppError, FieldError};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "jwt_key_use", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum JwtKeyUse {
    Authentication,
    WebhookSignature,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "jwt_algorithm", rename_all = "SCREAMING_SNAKE_CASE")]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum JwtAlgorithm {
    #[sqlx(rename = "RS256")]
    RS256,
    #[sqlx(rename = "RS384")]
    RS384,
    #[sqlx(rename = "RS512")]
    RS512,
    #[sqlx(rename = "ES256")]
    ES256,
    #[sqlx(rename = "ES384")]
    ES384,
    #[sqlx(rename = "ES512")]
    ES512,
    #[sqlx(rename = "HS256")]
    HS256,
    #[sqlx(rename = "HS512")]
    HS512,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "jwt_key_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum JwtKeyStatus {
    Active,
    Disabled,
    Expired,
}

#[derive(sqlx::FromRow, Debug, Clone)]
pub struct JwtKey {
    pub id: Uuid,
    pub public_id: String,
    pub tenant_id: Uuid,
    pub application_id: Option<String>,
    pub name: String,
    pub key_use: JwtKeyUse,
    pub algorithm: JwtAlgorithm,
    pub key_id: String,
    pub status: JwtKeyStatus,
    pub public_key: Option<String>,
    pub private_key_enc: Option<String>,
    pub secret_enc: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub grace_period_ends_at: Option<DateTime<Utc>>,
    pub rotated_from_id: Option<String>,
    pub last_rotated_at: Option<DateTime<Utc>>,
    pub version: i32,
    pub created_by: Uuid,
    pub updated_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ── Request types ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateJwtKeyRequest {
    pub application_id: Option<String>,
    pub name: String,
    pub key_use: JwtKeyUse,
    pub algorithm: JwtAlgorithm,
    pub expires_at: Option<DateTime<Utc>>,
}

impl CreateJwtKeyRequest {
    pub fn normalize(mut self) -> Self {
        self.name = self.name.trim().to_owned();
        self
    }

    pub fn validate_all(&self) -> Result<(), AppError> {
        let mut errors: Vec<FieldError> = Vec::new();

        if self.name.is_empty() {
            errors.push(FieldError::new("name", "required", "name is required"));
        } else if self.name.len() > 128 {
            errors.push(FieldError::new("name", "max_length", "name must be 128 characters or fewer"));
        }

        if matches!(self.key_use, JwtKeyUse::WebhookSignature) && self.application_id.is_none() {
            errors.push(FieldError::new(
                "application_id",
                "required",
                "application_id is required for webhook_signature keys",
            ));
        }

        if !errors.is_empty() {
            return Err(AppError::Validation(errors));
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
pub struct UpdateJwtKeyRequest {
    pub name: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
}

impl UpdateJwtKeyRequest {
    pub fn normalize(mut self) -> Self {
        self.name = self.name.map(|n| n.trim().to_owned()).filter(|n| !n.is_empty());
        self
    }
}

#[derive(Debug, Deserialize)]
pub struct RotateJwtKeyRequest {
    /// How many hours the old key stays active after rotation. Default: 24.
    pub grace_period_hours: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct ListJwtKeysQuery {
    pub application_id: Option<String>,
    pub key_use: Option<JwtKeyUse>,
    pub status: Option<JwtKeyStatus>,
    pub limit: Option<i64>,
    pub cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GenerateKeyPairRequest {
    pub algorithm: JwtAlgorithm,
}

// ── Response types ─────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct JwtKeyResponse {
    pub id: String,
    pub tenant_id: Uuid,
    pub application_id: Option<String>,
    pub name: String,
    pub key_use: JwtKeyUse,
    pub algorithm: JwtAlgorithm,
    pub key_id: String,
    pub status: JwtKeyStatus,
    pub public_key: Option<String>,
    /// Only present on initial creation and rotation responses
    #[serde(skip_serializing_if = "Option::is_none")]
    pub private_key: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub grace_period_ends_at: Option<DateTime<Utc>>,
    pub rotated_from_id: Option<String>,
    pub last_rotated_at: Option<DateTime<Utc>>,
    pub created_by: Uuid,
    pub updated_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl JwtKeyResponse {
    pub fn from_key(k: JwtKey) -> Self {
        Self {
            id: k.public_id,
            tenant_id: k.tenant_id,
            application_id: k.application_id,
            name: k.name,
            key_use: k.key_use,
            algorithm: k.algorithm,
            key_id: k.key_id,
            status: k.status,
            public_key: k.public_key,
            private_key: None,
            expires_at: k.expires_at,
            grace_period_ends_at: k.grace_period_ends_at,
            rotated_from_id: k.rotated_from_id,
            last_rotated_at: k.last_rotated_at,
            created_by: k.created_by,
            updated_by: k.updated_by,
            created_at: k.created_at,
            updated_at: k.updated_at,
        }
    }

    pub fn with_private_key(mut self, private_key: String) -> Self {
        self.private_key = Some(private_key);
        self
    }
}

#[derive(Debug, Serialize)]
pub struct ListJwtKeysResponse {
    pub items: Vec<JwtKeyResponse>,
    pub next_cursor: Option<String>,
    pub limit: i64,
}

#[derive(Debug, Serialize)]
pub struct GenerateKeyPairResponse {
    pub algorithm: JwtAlgorithm,
    pub public_key: Option<String>,
    pub private_key: Option<String>,
    pub secret: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct JwksResponse {
    pub keys: Vec<serde_json::Value>,
}
