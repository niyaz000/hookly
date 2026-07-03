use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use uuid::Uuid;

// --- Enum types ---

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EndpointType {
    Http,
}

impl EndpointType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Http => "http",
        }
    }
}

// --- HTTP config shape (used for validation in the service layer) ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpConfig {
    pub url: String,
    #[serde(default = "HttpConfig::default_method")]
    pub method: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
}

impl HttpConfig {
    fn default_method() -> String {
        "POST".to_string()
    }
}

// --- DB row structs ---

#[allow(dead_code)]
#[derive(sqlx::FromRow, Debug, Clone)]
pub struct EndpointRow {
    pub id: Uuid,
    pub public_id: String,
    pub application_id: Uuid,
    pub application_public_id: String,
    pub tenant_id: Uuid,
    pub organization_id: Uuid,
    pub description: Option<String>,
    pub endpoint_type: String,
    pub config: Json<serde_json::Value>,
    pub event_types: Vec<String>,
    pub status: String,
    pub rate_limit_per_minute: Option<i32>,
    pub tags: Json<HashMap<String, String>>,
    pub version: i32,
    pub request_id: Uuid,
    pub created_by: Uuid,
    pub updated_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[allow(dead_code)]
#[derive(sqlx::FromRow, Debug, Clone)]
pub struct SecretRow {
    pub id: Uuid,
    pub public_id: String,
    pub endpoint_id: Uuid,
    pub tenant_id: Uuid,
    pub organization_id: Uuid,
    pub secret: String,
    pub is_active: bool,
    pub expires_at: Option<DateTime<Utc>>,
    pub request_id: Uuid,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
}

// --- Request types ---

#[derive(Serialize, Deserialize, Debug)]
pub struct CreateEndpointRequest {
    pub application_id: String,
    pub description: Option<String>,
    pub endpoint_type: EndpointType,
    pub config: serde_json::Value,
    #[serde(default)]
    pub event_types: Vec<String>,
    pub rate_limit_per_minute: Option<i32>,
    #[serde(default)]
    pub tags: HashMap<String, String>,
}

#[derive(Deserialize, Debug)]
pub struct UpdateEndpointRequest {
    pub description: Option<String>,
    pub config: Option<serde_json::Value>,
    pub event_types: Option<Vec<String>>,
    /// None = don't touch; Some(None) = clear to null; Some(Some(x)) = set to x
    #[serde(default, deserialize_with = "double_option::deserialize")]
    pub rate_limit_per_minute: Option<Option<i32>>,
    pub tags: Option<HashMap<String, String>>,
    pub version: i32,
}

#[derive(Deserialize, Debug, Default)]
pub struct RotateSecretRequest {
    /// Seconds the old secret remains valid after rotation. 0 = immediate deactivation.
    pub expiry_seconds: Option<u32>,
}

#[derive(Deserialize, Debug)]
pub struct ListQueryParams {
    pub application_id: String,
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(default = "default_limit")]
    pub limit: u32,
    pub status: Option<String>,
    pub endpoint_type: Option<String>,
    pub tags: Option<HashMap<String, String>>,
}

fn default_page() -> u32 {
    1
}
fn default_limit() -> u32 {
    20
}

// --- Response types ---

#[derive(Serialize, Deserialize, Debug)]
pub struct EndpointResponse {
    pub id: String,
    pub application_id: String,
    pub description: Option<String>,
    pub endpoint_type: String,
    pub config: serde_json::Value,
    pub event_types: Vec<String>,
    pub status: String,
    pub rate_limit_per_minute: Option<i32>,
    pub tags: HashMap<String, String>,
    pub created_by: Uuid,
    pub updated_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Serialize, Debug)]
pub struct SecretResponse {
    pub id: String,
    pub secret: String,
    pub created_at: DateTime<Utc>,
}

impl From<EndpointRow> for EndpointResponse {
    fn from(row: EndpointRow) -> Self {
        Self {
            id: row.public_id,
            application_id: row.application_public_id,
            description: row.description,
            endpoint_type: row.endpoint_type,
            config: row.config.0,
            event_types: row.event_types,
            status: row.status,
            rate_limit_per_minute: row.rate_limit_per_minute,
            tags: row.tags.0,
            created_by: row.created_by,
            updated_by: row.updated_by,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

// Serde helper: deserializes a field as Option<Option<T>> so that an absent JSON key
// becomes None (no-op) while an explicit JSON null becomes Some(None) (clear the value).
mod double_option {
    use serde::{Deserialize, Deserializer};

    pub fn deserialize<'de, T, D>(d: D) -> Result<Option<Option<T>>, D::Error>
    where
        T: Deserialize<'de>,
        D: Deserializer<'de>,
    {
        Ok(Some(Option::deserialize(d)?))
    }
}

impl EndpointRow {
    pub fn new_public_id() -> String {
        format!("ep_{}", crate::common::NanoId::new())
    }
}

impl SecretRow {
    pub fn new_public_id() -> String {
        format!("sec_{}", crate::common::NanoId::new())
    }
}
