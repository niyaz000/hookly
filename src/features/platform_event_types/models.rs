use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(sqlx::FromRow, Debug, Clone)]
pub struct PlatformEventType {
    pub id: Uuid,
    pub public_id: String,
    pub name: String,
    pub description: Option<String>,
    pub resource: String,
    pub action: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct ListPlatformEventTypesQuery {
    pub resource: Option<String>,
    pub limit: Option<i64>,
    pub cursor: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PlatformEventTypeResponse {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub resource: String,
    pub action: String,
    pub created_at: DateTime<Utc>,
}

impl From<PlatformEventType> for PlatformEventTypeResponse {
    fn from(e: PlatformEventType) -> Self {
        Self {
            id: e.public_id,
            name: e.name,
            description: e.description,
            resource: e.resource,
            action: e.action,
            created_at: e.created_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ListPlatformEventTypesResponse {
    pub items: Vec<PlatformEventTypeResponse>,
    pub next_cursor: Option<String>,
    pub limit: i64,
}
