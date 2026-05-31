use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use uuid::Uuid;

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
    pub idempotency_key: Option<String>,
    pub tags: Json<HashMap<String, String>>,
    pub request_id: Uuid,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
}

// --- Request types ---

#[derive(Deserialize, Debug)]
pub struct CreateEventRequest {
    pub application_id: String,
    pub event_type_id: String,
    pub endpoint_id: String,
    pub payload: serde_json::Value,
    pub idempotency_key: Option<String>,
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
    pub endpoint_id: Option<String>,
    pub before: Option<DateTime<Utc>>,
    pub after: Option<DateTime<Utc>>,
}

fn default_page() -> u32 {
    1
}
fn default_limit() -> u32 {
    20
}

// --- Response types ---

#[derive(Serialize, Debug)]
pub struct EventResponse {
    pub id: String,
    pub application_id: String,
    pub event_type_id: String,
    pub event_type_name: String,
    pub endpoint_id: Option<String>,
    pub payload: serde_json::Value,
    pub idempotency_key: Option<String>,
    pub tags: HashMap<String, String>,
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
            idempotency_key: row.idempotency_key,
            tags: row.tags.0,
            created_by: row.created_by,
            created_at: row.created_at,
        }
    }
}
