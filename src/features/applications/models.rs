use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "application_state", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ApplicationState {
    Active,
    Suspended,
    Inactive,
}

#[derive(sqlx::FromRow, Debug, Clone)]
pub struct Application {
    pub id: Uuid,
    pub public_id: String,
    pub organization_id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub description: String,
    pub tags: Json<HashMap<String, String>>,
    pub state: ApplicationState,
    pub created_by: Uuid,
    pub updated_by: Uuid,
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
    pub state: ApplicationState,
    pub created_by: Uuid,
    pub updated_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Serialize, Debug)]
pub struct GetApplicationResponse {
    pub id: Uuid,
    pub public_id: String,
    pub tenant_id: Uuid,
    pub organization_id: Uuid,
    pub name: String,
    pub description: String,
    pub tags: HashMap<String, String>,
    pub state: ApplicationState,
    pub created_by: Uuid,
    pub updated_by: Uuid,
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
            state: app.state,
            created_by: app.created_by,
            updated_by: app.updated_by,
            created_at: app.created_at,
            updated_at: app.updated_at,
        }
    }
}

impl From<Application> for GetApplicationResponse {
    fn from(app: Application) -> Self {
        Self {
            id: app.id,
            public_id: app.public_id,
            tenant_id: app.tenant_id,
            organization_id: app.organization_id,
            name: app.name,
            description: app.description,
            tags: app.tags.0,
            state: app.state,
            created_by: app.created_by,
            updated_by: app.updated_by,
            created_at: app.created_at,
            updated_at: app.updated_at,
        }
    }
}
