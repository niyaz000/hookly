use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: T,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PaginatedResponse<T> {
    pub items: Vec<T>,
    pub total: i64,
    pub page: i32,
    pub limit: i32,
}

/// Shared metadata for tracing and audit on write operations.
#[derive(Debug, Clone, Copy)]
pub struct RequestContext {
    pub request_id: Uuid,
    pub created_by: Uuid,
}
