use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use uuid::Uuid;

#[derive(sqlx::FromRow, Debug, Clone)]
pub struct Application {
    pub id: Uuid,
    pub public_id: String,
    pub organization_id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub description: String,
    pub tags: Json<HashMap<String, String>>,
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

#[derive(Serialize, Debug)]
pub struct CreateApplicationResponse {
    pub id: Uuid,
    pub public_id: String,
    pub tenant_id: Uuid,
    pub organization_id: Uuid,
    pub name: String,
    pub description: String,
    pub tags: HashMap<String, String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Application> for CreateApplicationResponse {
    fn from(app: Application) -> Self {
        Self {
            id: app.id,
            public_id: app.public_id,
            tenant_id: app.tenant_id,
            organization_id: app.organization_id,
            name: app.name,
            description: app.description,
            tags: app.tags.0,
            created_at: app.created_at,
            updated_at: app.updated_at,
        }
    }
}
