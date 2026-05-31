use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use uuid::Uuid;
use validator::Validate;

use crate::common::validators::validate_not_blank;
use crate::error::AppError;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Team {
    pub id: Uuid,
    pub public_id: String,
    pub name: String,
    pub tenant_id: Uuid,
    pub organization_id: Uuid,
    pub description: Option<String>,
    pub tags: Json<HashMap<String, String>>,
    pub metadata: Json<HashMap<String, String>>,
    pub settings: Json<HashMap<String, String>>,
    pub created_by: Uuid,
    pub updated_by: Uuid,
    pub request_id: Uuid,
    pub version: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct TeamMember {
    pub id: Uuid,
    pub public_id: String,
    pub tenant_id: Uuid,
    pub organization_id: Uuid,
    pub team_id: Uuid,
    pub user_id: Uuid,
    pub created_by: Uuid,
    pub updated_by: Uuid,
    pub request_id: Uuid,
    pub version: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateTeamRequest {
    #[validate(custom(function = "validate_not_blank", message = "name is required"))]
    #[validate(length(max = 255, message = "name must be 255 characters or fewer"))]
    pub name: String,
    pub tenant_id: Uuid,
    pub organization_id: Uuid,
    pub description: Option<String>,
    pub tags: Option<HashMap<String, String>>,
    pub metadata: Option<HashMap<String, String>>,
    pub settings: Option<HashMap<String, String>>,
}

impl CreateTeamRequest {
    pub fn validate(&self) -> Result<(), AppError> {
        Validate::validate(self).map_err(AppError::from)
    }
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateTeamRequest {
    #[validate(custom(function = "validate_not_blank", message = "name cannot be empty"))]
    #[validate(length(max = 255, message = "name must be 255 characters or fewer"))]
    pub name: Option<String>,
    pub description: Option<String>,
    pub tags: Option<HashMap<String, String>>,
    pub metadata: Option<HashMap<String, String>>,
    pub settings: Option<HashMap<String, String>>,
}

impl UpdateTeamRequest {
    pub fn validate(&self) -> Result<(), AppError> {
        Validate::validate(self).map_err(AppError::from)
    }
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct AddTeamMembersRequest {
    #[validate(length(min = 1, message = "user_ids must contain at least one entry"))]
    pub user_ids: Vec<Uuid>,
}

impl AddTeamMembersRequest {
    pub fn validate(&self) -> Result<(), AppError> {
        Validate::validate(self).map_err(AppError::from)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TeamResponse {
    pub id: String,
    pub name: String,
    pub tenant_id: Uuid,
    pub organization_id: Uuid,
    pub description: Option<String>,
    pub tags: HashMap<String, String>,
    pub metadata: HashMap<String, String>,
    pub settings: HashMap<String, String>,
    pub created_by: Uuid,
    pub updated_by: Uuid,
    pub request_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Team> for TeamResponse {
    fn from(t: Team) -> Self {
        Self {
            id: t.public_id,
            name: t.name,
            tenant_id: t.tenant_id,
            organization_id: t.organization_id,
            description: t.description,
            tags: t.tags.0,
            metadata: t.metadata.0,
            settings: t.settings.0,
            created_by: t.created_by,
            updated_by: t.updated_by,
            request_id: t.request_id,
            created_at: t.created_at,
            updated_at: t.updated_at,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TeamMemberResponse {
    pub id: String,
    pub tenant_id: Uuid,
    pub organization_id: Uuid,
    pub team_id: Uuid,
    pub user_id: Uuid,
    pub created_by: Uuid,
    pub updated_by: Uuid,
    pub request_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<TeamMember> for TeamMemberResponse {
    fn from(m: TeamMember) -> Self {
        Self {
            id: m.public_id,
            tenant_id: m.tenant_id,
            organization_id: m.organization_id,
            team_id: m.team_id,
            user_id: m.user_id,
            created_by: m.created_by,
            updated_by: m.updated_by,
            request_id: m.request_id,
            created_at: m.created_at,
            updated_at: m.updated_at,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ListTeamsQuery {
    pub limit: Option<i64>,
    pub cursor: Option<String>,
    pub organization_id: Option<Uuid>,
    pub tenant_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
pub struct ListTeamsResponse {
    pub items: Vec<TeamResponse>,
    pub next_cursor: Option<String>,
    pub limit: i64,
}
