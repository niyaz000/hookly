use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use uuid::Uuid;

use crate::error::{AppError, FieldError};

// --- DB row structs ---

#[allow(dead_code)]
#[derive(sqlx::FromRow, Debug, Clone)]
pub struct ScheduleRow {
    pub id: Uuid,
    pub public_id: String,
    pub name: String,
    pub description: Option<String>,
    pub tenant_id: Uuid,
    pub organization_id: Uuid,
    pub event_type_id: Uuid,
    pub payload: Json<serde_json::Value>,
    pub cron_expression: String,
    pub timezone: String,
    pub status: String,
    pub next_run_at: Option<DateTime<Utc>>,
    pub last_run_at: Option<DateTime<Utc>>,
    pub last_run_status: Option<String>,
    pub created_by: Uuid,
    pub updated_by: Uuid,
    pub request_id: Uuid,
    pub version: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub event_type_public_id: String,
    pub endpoint_public_ids: Vec<String>,
}

#[allow(dead_code)]
#[derive(sqlx::FromRow, Debug, Clone)]
pub struct ScheduleExecutionRow {
    pub id: Uuid,
    pub public_id: String,
    pub schedule_id: Uuid,
    pub schedule_public_id: String,
    pub tenant_id: Uuid,
    pub organization_id: Uuid,
    pub status: String,
    pub triggered_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
}

// --- Request types ---

#[derive(Debug, Deserialize)]
pub struct CreateScheduleRequest {
    pub name: String,
    pub description: Option<String>,
    pub tenant_id: Uuid,
    pub organization_id: Uuid,
    pub event_type_id: String,
    pub endpoint_ids: Vec<String>,
    pub payload: serde_json::Value,
    pub cron_expression: String,
    pub timezone: Option<String>,
}

impl CreateScheduleRequest {
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

        if self.event_type_id.trim().is_empty() {
            errors.push(FieldError::new(
                "event_type_id",
                "required",
                "event_type_id is required",
            ));
        }

        if self.endpoint_ids.is_empty() {
            errors.push(FieldError::new(
                "endpoint_ids",
                "min_items",
                "endpoint_ids must contain at least one entry",
            ));
        }

        if self.cron_expression.trim().is_empty() {
            errors.push(FieldError::new(
                "cron_expression",
                "required",
                "cron_expression is required",
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
pub struct UpdateScheduleRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub endpoint_ids: Option<Vec<String>>,
    pub payload: Option<serde_json::Value>,
    pub cron_expression: Option<String>,
    pub timezone: Option<String>,
}

impl UpdateScheduleRequest {
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
        if let Some(ref expr) = self.cron_expression {
            if expr.trim().is_empty() {
                errors.push(FieldError::new(
                    "cron_expression",
                    "required",
                    "cron_expression cannot be empty",
                ));
            }
        }
        if let Some(ref ids) = self.endpoint_ids {
            if ids.is_empty() {
                errors.push(FieldError::new(
                    "endpoint_ids",
                    "min_items",
                    "endpoint_ids must contain at least one entry",
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
pub struct ListSchedulesQuery {
    pub limit: Option<i64>,
    pub cursor: Option<String>,
    pub organization_id: Option<Uuid>,
    pub tenant_id: Option<Uuid>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ListExecutionsQuery {
    pub limit: Option<i64>,
    pub cursor: Option<String>,
}

// --- Response types ---

#[derive(Debug, Serialize)]
pub struct ScheduleResponse {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub tenant_id: Uuid,
    pub organization_id: Uuid,
    pub event_type_id: String,
    pub endpoint_ids: Vec<String>,
    pub payload: serde_json::Value,
    pub cron_expression: String,
    pub timezone: String,
    pub status: String,
    pub next_run_at: Option<DateTime<Utc>>,
    pub last_run_at: Option<DateTime<Utc>>,
    pub last_run_status: Option<String>,
    pub created_by: Uuid,
    pub updated_by: Uuid,
    pub request_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<ScheduleRow> for ScheduleResponse {
    fn from(r: ScheduleRow) -> Self {
        Self {
            id: r.public_id,
            name: r.name,
            description: r.description,
            tenant_id: r.tenant_id,
            organization_id: r.organization_id,
            event_type_id: r.event_type_public_id,
            endpoint_ids: r.endpoint_public_ids,
            payload: r.payload.0,
            cron_expression: r.cron_expression,
            timezone: r.timezone,
            status: r.status,
            next_run_at: r.next_run_at,
            last_run_at: r.last_run_at,
            last_run_status: r.last_run_status,
            created_by: r.created_by,
            updated_by: r.updated_by,
            request_id: r.request_id,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ScheduleExecutionResponse {
    pub id: String,
    pub schedule_id: String,
    pub tenant_id: Uuid,
    pub organization_id: Uuid,
    pub status: String,
    pub triggered_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl From<ScheduleExecutionRow> for ScheduleExecutionResponse {
    fn from(r: ScheduleExecutionRow) -> Self {
        Self {
            id: r.public_id,
            schedule_id: r.schedule_public_id,
            tenant_id: r.tenant_id,
            organization_id: r.organization_id,
            status: r.status,
            triggered_at: r.triggered_at,
            started_at: r.started_at,
            completed_at: r.completed_at,
            error_message: r.error_message,
            created_at: r.created_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ListSchedulesResponse {
    pub items: Vec<ScheduleResponse>,
    pub next_cursor: Option<String>,
    pub limit: i64,
}

#[derive(Debug, Serialize)]
pub struct ListExecutionsResponse {
    pub items: Vec<ScheduleExecutionResponse>,
    pub next_cursor: Option<String>,
    pub limit: i64,
}
