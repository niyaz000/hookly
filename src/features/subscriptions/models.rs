use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(sqlx::FromRow, Debug, Clone)]
pub struct SubscriptionRow {
    pub id: Uuid,
    pub public_id: String,
    pub endpoint_id: Uuid,
    pub endpoint_public_id: String,
    pub event_type_id: Uuid,
    pub event_type_public_id: String,
    pub event_type_name: String,
    pub application_id: Uuid,
    pub application_public_id: String,
    pub tenant_id: Uuid,
    pub organization_id: Uuid,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Deserialize, Debug)]
pub struct CreateSubscriptionRequest {
    pub application_id: String,
    pub endpoint_id: String,
    pub event_type_id: String,
}

#[derive(Deserialize, Debug, Default)]
pub struct ListQueryParams {
    pub application_id: String,
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(default = "default_limit")]
    pub limit: u32,
    pub endpoint_id: Option<String>,
    pub event_type_id: Option<String>,
}

fn default_page() -> u32 {
    1
}
fn default_limit() -> u32 {
    20
}

#[derive(Serialize, Debug)]
pub struct SubscriptionResponse {
    pub id: String,
    pub application_id: String,
    pub endpoint_id: String,
    pub event_type_id: String,
    pub event_type_name: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

impl From<SubscriptionRow> for SubscriptionResponse {
    fn from(row: SubscriptionRow) -> Self {
        Self {
            id: row.public_id,
            application_id: row.application_public_id,
            endpoint_id: row.endpoint_public_id,
            event_type_id: row.event_type_public_id,
            event_type_name: row.event_type_name,
            status: row.status,
            created_at: row.created_at,
        }
    }
}
