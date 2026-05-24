use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(sqlx::FromRow, Serialize, Deserialize, Debug, Clone)]
pub struct Application {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Deserialize, Debug)]
pub struct CreateApplicationRequest {
    pub tenant_id: Uuid,
    pub organization_id: Uuid,
    pub name: String,
    pub description: String,
    pub tags: HashMap<String, String>,
}

pub struct CreateApplicationResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub organization_id: Uuid,
    pub name: String,
    pub description: String,
    pub tags: HashMap<String, String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
