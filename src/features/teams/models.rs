use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use uuid::Uuid;

use crate::error::{AppError, FieldError};

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

#[derive(Debug, Deserialize)]
pub struct CreateTeamRequest {
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
        let mut errors = Vec::new();

        if self.name.trim().is_empty() {
            errors.push(FieldError::new("name", "required", "name is required"));
        } else if self.name.len() > 255 {
            errors.push(FieldError::new(
                "name",
                "max_length",
                "name must be 255 characters or fewer",
            ));
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(AppError::Validation(errors))
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct UpdateTeamRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub tags: Option<HashMap<String, String>>,
    pub metadata: Option<HashMap<String, String>>,
    pub settings: Option<HashMap<String, String>>,
}

impl UpdateTeamRequest {
    pub fn validate(&self) -> Result<(), AppError> {
        let mut errors = Vec::new();

        if let Some(ref n) = self.name {
            if n.trim().is_empty() {
                errors.push(FieldError::new("name", "required", "name cannot be empty"));
            } else if n.len() > 255 {
                errors.push(FieldError::new(
                    "name",
                    "max_length",
                    "name must be 255 characters or fewer",
                ));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(AppError::Validation(errors))
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct AddTeamMembersRequest {
    pub user_ids: Vec<Uuid>,
}

impl AddTeamMembersRequest {
    pub fn validate(&self) -> Result<(), AppError> {
        if self.user_ids.is_empty() {
            return Err(AppError::Validation(vec![FieldError::new(
                "user_ids",
                "min_items",
                "user_ids must contain at least one entry",
            )]));
        }
        Ok(())
    }
}

#[derive(Debug, Serialize)]
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

#[derive(Debug, Serialize)]
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
