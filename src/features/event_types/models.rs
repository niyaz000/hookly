use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use uuid::Uuid;

// ── Schema type system ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FieldType {
    String,
    Number,
    Integer,
    Boolean,
    Null,
    Object,
    Array,
}

/// Valid `format` values for `string` fields.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum StringFormat {
    Date,
    DateTime,
    Uuid,
    Email,
    Uri,
}

/// A single field definition inside an event schema.
/// The same struct is used recursively for nested objects and array items.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropertyDef {
    #[serde(rename = "type")]
    pub field_type: FieldType,

    // string constraints
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<StringFormat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_length: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_length: Option<u32>,

    // number / integer constraints
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimum: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maximum: Option<f64>,

    // object constraints
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<HashMap<String, PropertyDef>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required: Vec<String>,

    // array constraints
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Box<PropertyDef>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_items: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_items: Option<u32>,

    // cross-cutting
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub nullable: bool,
}

// ── DB row ────────────────────────────────────────────────────────────────────

#[derive(sqlx::FromRow, Debug, Clone)]
pub struct EventType {
    pub id: Uuid,
    pub public_id: String,
    pub organization_id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub schema_version: String,
    pub description: Option<String>,
    pub event_schema: Json<PropertyDef>,
    pub archived: bool,
    pub version: i32,
    pub created_by: Uuid,
    pub updated_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ── Request types ─────────────────────────────────────────────────────────────

#[derive(Deserialize, Debug)]
pub struct CreateEventTypeRequest {
    pub tenant_id: Uuid,
    pub organization_id: Uuid,
    pub name: String,
    pub schema_version: Option<String>,
    pub description: Option<String>,
    pub event_schema: PropertyDef,
}

#[derive(Deserialize, Debug)]
pub struct CreateVersionRequest {
    pub schema_version: String,
    pub description: Option<String>,
    pub event_schema: PropertyDef,
}

#[derive(Deserialize, Debug)]
pub struct ListQueryParams {
    pub tenant_id: Uuid,
    pub name: Option<String>,
    pub schema_version: Option<String>,
    pub archived: Option<bool>,
    #[serde(default = "default_page")]
    pub page: i64,
    #[serde(default = "default_limit")]
    pub limit: i64,
}

fn default_page() -> i64 {
    1
}
fn default_limit() -> i64 {
    20
}

#[derive(Deserialize, Debug)]
pub struct UpdateEventTypeRequest {
    pub description: Option<String>,
    pub version: i32,
}

// ── Response types ────────────────────────────────────────────────────────────

#[derive(Serialize, Debug)]
pub struct EventTypeResponse {
    pub id: String,
    pub organization_id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub schema_version: String,
    pub description: Option<String>,
    pub event_schema: PropertyDef,
    pub archived: bool,
    pub created_by: Uuid,
    pub updated_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<EventType> for EventTypeResponse {
    fn from(et: EventType) -> Self {
        Self {
            id: et.public_id,
            organization_id: et.organization_id,
            tenant_id: et.tenant_id,
            name: et.name,
            schema_version: et.schema_version,
            description: et.description,
            event_schema: et.event_schema.0,
            archived: et.archived,
            created_by: et.created_by,
            updated_by: et.updated_by,
            created_at: et.created_at,
            updated_at: et.updated_at,
        }
    }
}

#[derive(Serialize, Debug)]
pub struct EventTypeSchemaResponse {
    pub id: String,
    pub name: String,
    pub schema_version: String,
    pub event_schema: PropertyDef,
}

impl From<EventType> for EventTypeSchemaResponse {
    fn from(et: EventType) -> Self {
        Self {
            id: et.public_id,
            name: et.name,
            schema_version: et.schema_version,
            event_schema: et.event_schema.0,
        }
    }
}
