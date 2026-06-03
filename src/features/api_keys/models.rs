use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::error::{AppError, FieldError};
use crate::common::validators::validate_not_blank;

// ── Postgres enum types ──────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "api_key_environment", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum ApiKeyEnvironment {
    Live,
    Test,
    Dev,
    Sandbox,
}

impl ApiKeyEnvironment {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Live => "live",
            Self::Test => "test",
            Self::Dev => "dev",
            Self::Sandbox => "sandbox",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "api_key_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum ApiKeyStatus {
    Active,
    Expired,
}

// ── DB row structs ───────────────────────────────────────────────────────────

#[derive(sqlx::FromRow, Debug, Clone)]
pub struct ApiKey {
    pub id: Uuid,
    pub public_id: String,
    pub organization_id: Uuid,
    pub tenant_id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub key_hash: String,
    pub key_encrypted: Option<String>,
    pub key_prefix: String,
    pub environment: ApiKeyEnvironment,
    pub status: ApiKeyStatus,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub version: i32,
    pub created_by: Uuid,
    pub updated_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(sqlx::FromRow, Debug, Clone)]
pub struct ApiKeySettings {
    pub id: Uuid,
    pub public_id: String,
    pub organization_id: Uuid,
    pub tenant_id: Uuid,
    pub max_keys_per_user: Option<i32>,
    pub key_length: i16,
    pub default_ttl_seconds: Option<i32>,
    pub allow_view_later: bool,
    pub version: i32,
    pub created_by: Uuid,
    pub updated_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ── Request types ────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct CreateApiKeyRequest {
    pub tenant_id: Uuid,
    pub organization_id: Uuid,
    pub user_id: Uuid,
    #[validate(custom(function = "validate_not_blank", message = "name is required"))]
    #[validate(length(max = 64, message = "name must be 64 characters or fewer"))]
    pub name: String,
    #[validate(length(max = 521, message = "description must be 521 characters or fewer"))]
    pub description: Option<String>,
    pub environment: ApiKeyEnvironment,
    pub expires_at: Option<DateTime<Utc>>,
}

impl CreateApiKeyRequest {
    pub fn validate_all(&self) -> Result<(), AppError> {
        Validate::validate(self).map_err(AppError::from)?;
        if let Some(expires_at) = self.expires_at {
            if expires_at <= Utc::now() {
                return Err(AppError::Validation(vec![FieldError::new(
                    "expires_at",
                    "invalid_value",
                    "expires_at must be in the future",
                )]));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
pub struct UpdateApiKeyRequest {
    pub description: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
}

impl UpdateApiKeyRequest {
    pub fn validate_all(&self) -> Result<(), AppError> {
        if self.description.is_none() && self.expires_at.is_none() {
            return Err(AppError::BadRequest(
                "at least one of description or expires_at must be provided".into(),
            ));
        }
        if let Some(ref desc) = self.description {
            if desc.len() > 521 {
                return Err(AppError::Validation(vec![FieldError::new(
                    "description",
                    "max_length",
                    "description must be 521 characters or fewer",
                )]));
            }
        }
        if let Some(expires_at) = self.expires_at {
            if expires_at <= Utc::now() {
                return Err(AppError::Validation(vec![FieldError::new(
                    "expires_at",
                    "invalid_value",
                    "expires_at must be in the future",
                )]));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct UpsertApiKeySettingsRequest {
    pub organization_id: Uuid,
    pub tenant_id: Uuid,
    pub max_keys_per_user: Option<i32>,
    pub key_length: i16,
    pub default_ttl_seconds: Option<i32>,
    pub allow_view_later: bool,
}

impl UpsertApiKeySettingsRequest {
    pub fn validate_all(&self) -> Result<(), AppError> {
        if self.key_length < 16 || self.key_length > 128 {
            return Err(AppError::Validation(vec![FieldError::new(
                "key_length",
                "invalid_value",
                "key_length must be between 16 and 128",
            )]));
        }
        if let Some(max) = self.max_keys_per_user {
            if max < 1 {
                return Err(AppError::Validation(vec![FieldError::new(
                    "max_keys_per_user",
                    "invalid_value",
                    "max_keys_per_user must be at least 1",
                )]));
            }
        }
        if let Some(ttl) = self.default_ttl_seconds {
            if ttl < 1 {
                return Err(AppError::Validation(vec![FieldError::new(
                    "default_ttl_seconds",
                    "invalid_value",
                    "default_ttl_seconds must be at least 1",
                )]));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateApiKeySettingsRequest {
    pub max_keys_per_user: Option<i32>,
    pub key_length: i16,
    pub default_ttl_seconds: Option<i32>,
    pub allow_view_later: bool,
}

impl UpdateApiKeySettingsRequest {
    pub fn validate_all(&self) -> Result<(), AppError> {
        if self.key_length < 16 || self.key_length > 128 {
            return Err(AppError::Validation(vec![FieldError::new(
                "key_length",
                "invalid_value",
                "key_length must be between 16 and 128",
            )]));
        }
        if let Some(max) = self.max_keys_per_user {
            if max < 1 {
                return Err(AppError::Validation(vec![FieldError::new(
                    "max_keys_per_user",
                    "invalid_value",
                    "max_keys_per_user must be at least 1",
                )]));
            }
        }
        if let Some(ttl) = self.default_ttl_seconds {
            if ttl < 1 {
                return Err(AppError::Validation(vec![FieldError::new(
                    "default_ttl_seconds",
                    "invalid_value",
                    "default_ttl_seconds must be at least 1",
                )]));
            }
        }
        Ok(())
    }
}

// ── List query ───────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ListApiKeysQuery {
    pub tenant_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub environment: Option<ApiKeyEnvironment>,
    pub status: Option<ApiKeyStatus>,
    pub limit: Option<i64>,
    pub cursor: Option<Uuid>,
}

// ── Audit helpers ────────────────────────────────────────────────────────────

pub struct InsertAuditParams {
    pub api_key_id: Uuid,
    pub api_key_public_id: String,
    pub organization_id: Uuid,
    pub tenant_id: Uuid,
    pub user_id: Uuid,
    pub action: &'static str,
    pub actor_id: Option<Uuid>,
    pub request_id: Uuid,
    pub changes: Option<serde_json::Value>,
}

// ── Response types ───────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct ApiKeyResponse {
    pub id: String,
    pub organization_id: Uuid,
    pub tenant_id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub environment: ApiKeyEnvironment,
    pub status: ApiKeyStatus,
    /// Display hint: `hkly_<env>_<3-char prefix>...`
    pub key_hint: String,
    /// Plaintext key — present only in the creation response, absent on all other reads.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub version: i32,
    pub created_by: Uuid,
    pub updated_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<ApiKey> for ApiKeyResponse {
    fn from(k: ApiKey) -> Self {
        let key_hint = format!("hkly_{}_{}...", k.environment.as_str(), k.key_prefix);
        Self {
            id: k.public_id,
            organization_id: k.organization_id,
            tenant_id: k.tenant_id,
            user_id: k.user_id,
            name: k.name,
            description: k.description,
            environment: k.environment,
            status: k.status,
            key_hint,
            key: None,
            expires_at: k.expires_at,
            last_used_at: k.last_used_at,
            version: k.version,
            created_by: k.created_by,
            updated_by: k.updated_by,
            created_at: k.created_at,
            updated_at: k.updated_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ListApiKeysResponse {
    pub items: Vec<ApiKeyResponse>,
    pub next_cursor: Option<String>,
    pub limit: i64,
}

#[derive(Debug, Serialize)]
pub struct RevealApiKeyResponse {
    pub id: String,
    pub key: String,
}

#[derive(Debug, Serialize)]
pub struct ApiKeySettingsResponse {
    pub id: String,
    pub organization_id: Uuid,
    pub tenant_id: Uuid,
    pub max_keys_per_user: Option<i32>,
    pub key_length: i16,
    pub default_ttl_seconds: Option<i32>,
    pub allow_view_later: bool,
    pub version: i32,
    pub created_by: Uuid,
    pub updated_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<ApiKeySettings> for ApiKeySettingsResponse {
    fn from(s: ApiKeySettings) -> Self {
        Self {
            id: s.public_id,
            organization_id: s.organization_id,
            tenant_id: s.tenant_id,
            max_keys_per_user: s.max_keys_per_user,
            key_length: s.key_length,
            default_ttl_seconds: s.default_ttl_seconds,
            allow_view_later: s.allow_view_later,
            version: s.version,
            created_by: s.created_by,
            updated_by: s.updated_by,
            created_at: s.created_at,
            updated_at: s.updated_at,
        }
    }
}
