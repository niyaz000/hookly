use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use uuid::Uuid;

// --- Schema validation types ---

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SchemaError {
    pub field: String,
    pub message: String,
}

// --- Payload type ---

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "lowercase")]
pub enum PayloadType {
    #[default]
    Json,
    Text,
}

impl PayloadType {
    pub fn as_str(&self) -> &'static str {
        match self {
            PayloadType::Json => "json",
            PayloadType::Text => "text",
        }
    }
}

// --- DB row ---

#[allow(dead_code)]
#[derive(sqlx::FromRow, Debug, Clone)]
pub struct EventRow {
    pub id: Uuid,
    pub public_id: String,
    pub application_id: Uuid,
    pub application_public_id: String,
    pub event_type_id: Uuid,
    pub event_type_public_id: String,
    pub event_type_name: String,
    pub endpoint_id: Option<Uuid>,
    pub endpoint_public_id: Option<String>,
    pub tenant_id: Uuid,
    pub organization_id: Uuid,
    pub payload: Json<serde_json::Value>,
    pub payload_type: String,
    pub idempotency_key: Option<String>,
    pub body_hash: Option<Vec<u8>>,
    pub tags: Json<HashMap<String, String>>,
    pub request_id: Uuid,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub schema_valid: bool,
    pub schema_errors: Json<Vec<SchemaError>>,
}

// --- Request types ---

#[derive(Serialize, Deserialize, Debug)]
pub struct CreateEventRequest {
    pub application_id: String,
    pub event_type_id: String,
    #[serde(default)]
    pub schema_version: Option<String>,
    pub payload: serde_json::Value,
    #[serde(default)]
    pub payload_type: PayloadType,
    #[serde(default)]
    pub tags: HashMap<String, String>,
}

#[derive(Deserialize, Debug, Default)]
pub struct ListQueryParams {
    pub application_id: String,
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(default = "default_limit")]
    pub limit: u32,
    pub event_type_id: Option<String>,
    pub before: Option<DateTime<Utc>>,
    pub after: Option<DateTime<Utc>>,
    pub tags: Option<HashMap<String, String>>,
}

fn default_page() -> u32 {
    1
}
fn default_limit() -> u32 {
    20
}

// --- Bulk request types ---

#[derive(Serialize, Deserialize, Debug)]
pub struct BulkCreateEventItem {
    pub application_id: String,
    pub event_type_id: String,
    #[serde(default)]
    pub schema_version: Option<String>,
    pub payload: serde_json::Value,
    #[serde(default)]
    pub payload_type: PayloadType,
    #[serde(default)]
    pub tags: HashMap<String, String>,
    #[serde(default)]
    pub idempotency_key: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BulkCreateEventRequest {
    pub events: Vec<BulkCreateEventItem>,
}

// --- Bulk response types ---

#[derive(Serialize, Debug)]
pub struct BulkEventError {
    pub code: String,
    pub message: String,
}

#[derive(Serialize, Debug)]
pub struct BulkEventResultItem {
    pub index: usize,
    pub status: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event: Option<EventResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<BulkEventError>,
}

#[derive(Serialize, Debug)]
pub struct BulkCreateResponse {
    pub results: Vec<BulkEventResultItem>,
    pub succeeded: usize,
    pub failed: usize,
}

// --- Response types ---

#[derive(Serialize, Deserialize, Debug)]
pub struct EventResponse {
    pub id: String,
    pub application_id: String,
    pub event_type_id: String,
    pub event_type_name: String,
    pub endpoint_id: Option<String>,
    pub payload: serde_json::Value,
    pub payload_type: String,
    pub idempotency_key: Option<String>,
    pub tags: HashMap<String, String>,
    pub schema_valid: bool,
    pub schema_errors: Vec<SchemaError>,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
}

impl From<EventRow> for EventResponse {
    fn from(row: EventRow) -> Self {
        Self {
            id: row.public_id,
            application_id: row.application_public_id,
            event_type_id: row.event_type_public_id,
            event_type_name: row.event_type_name,
            endpoint_id: row.endpoint_public_id,
            payload: row.payload.0,
            payload_type: row.payload_type,
            idempotency_key: row.idempotency_key,
            tags: row.tags.0,
            schema_valid: row.schema_valid,
            schema_errors: row.schema_errors.0,
            created_by: row.created_by,
            created_at: row.created_at,
        }
    }
}
