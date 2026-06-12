use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::types::Json;
use uuid::Uuid;

#[allow(dead_code)]
#[derive(sqlx::FromRow, Debug, Clone)]
pub struct DeliveryJobRow {
    pub id: Uuid,
    pub public_id: String,
    pub event_id: Uuid,
    pub endpoint_id: Uuid,
    pub organization_id: Uuid,
    pub status: String,
    pub attempt: i32,
    pub enqueued_at: Option<DateTime<Utc>>,
    pub stream_name: String,
    pub created_at: DateTime<Utc>,
}

/// Full job payload fetched by the worker for a single delivery attempt.
#[allow(dead_code)]
#[derive(sqlx::FromRow, Debug, Clone)]
pub struct WorkerJob {
    pub job_id: Uuid,
    pub job_public_id: String,
    pub event_id: Uuid,
    pub endpoint_id: Uuid,
    pub organization_id: Uuid,
    pub attempt: i32,
    pub event_public_id: String,
    pub payload: Json<serde_json::Value>,
    pub tenant_id: Uuid,
    pub endpoint_config: Json<serde_json::Value>,
    pub encrypted_secret: String,
}

#[derive(Serialize, Debug)]
pub struct DeliveryJobResponse {
    pub id: String,
    pub event_id: Uuid,
    pub endpoint_id: Uuid,
    pub organization_id: Uuid,
    pub status: String,
    pub attempt: i32,
    pub enqueued_at: Option<DateTime<Utc>>,
    pub stream_name: String,
    pub created_at: DateTime<Utc>,
}

impl From<DeliveryJobRow> for DeliveryJobResponse {
    fn from(r: DeliveryJobRow) -> Self {
        Self {
            id: r.public_id,
            event_id: r.event_id,
            endpoint_id: r.endpoint_id,
            organization_id: r.organization_id,
            status: r.status,
            attempt: r.attempt,
            enqueued_at: r.enqueued_at,
            stream_name: r.stream_name,
            created_at: r.created_at,
        }
    }
}

/// Minimal projection used by the outbox poller.
#[allow(dead_code)]
#[derive(sqlx::FromRow, Debug)]
pub struct UnqueuedJob {
    pub id: Uuid,
    pub public_id: String,
    pub stream_name: String,
}
