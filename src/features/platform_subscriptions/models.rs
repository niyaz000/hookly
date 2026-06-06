use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{AppError, FieldError};

#[derive(sqlx::FromRow, Debug, Clone)]
pub struct PlatformSubscription {
    pub tenant_id: Uuid,
    pub event_type_public_id: String,
    pub created_at: DateTime<Utc>,
}

// ── Request types ──────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ListSubscriptionsQuery {
    pub tenant_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct UnsubscribeQuery {
    pub tenant_id: Uuid,
    pub event_type_id: String,
}

#[derive(Debug, Deserialize)]
pub struct SubscribeRequest {
    pub tenant_id: Uuid,
    /// Public IDs of platform event types to subscribe to.
    pub event_type_ids: Vec<String>,
}

impl SubscribeRequest {
    pub fn validate_all(&self) -> Result<(), AppError> {
        if self.event_type_ids.is_empty() {
            return Err(AppError::Validation(vec![FieldError::new(
                "event_type_ids",
                "required",
                "event_type_ids must not be empty",
            )]));
        }
        if self.event_type_ids.len() > 100 {
            return Err(AppError::Validation(vec![FieldError::new(
                "event_type_ids",
                "max_items",
                "event_type_ids must contain 100 or fewer items",
            )]));
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
pub struct ReplaceSubscriptionsRequest {
    pub tenant_id: Uuid,
    /// Replaces the tenant's full subscription list with these event type public IDs.
    pub event_type_ids: Vec<String>,
}

impl ReplaceSubscriptionsRequest {
    pub fn validate_all(&self) -> Result<(), AppError> {
        if self.event_type_ids.len() > 100 {
            return Err(AppError::Validation(vec![FieldError::new(
                "event_type_ids",
                "max_items",
                "event_type_ids must contain 100 or fewer items",
            )]));
        }
        Ok(())
    }
}

// ── Response types ─────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct SubscriptionItemResponse {
    pub event_type_id: String,
    pub subscribed_at: DateTime<Utc>,
}

impl From<PlatformSubscription> for SubscriptionItemResponse {
    fn from(s: PlatformSubscription) -> Self {
        Self {
            event_type_id: s.event_type_public_id,
            subscribed_at: s.created_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ListSubscriptionsResponse {
    pub tenant_id: Uuid,
    pub items: Vec<SubscriptionItemResponse>,
}

#[derive(Debug, Serialize)]
pub struct SubscribeResponse {
    pub tenant_id: Uuid,
    pub subscribed: usize,
    pub already_present: usize,
    pub invalid_event_type_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ReplaceSubscriptionsResponse {
    pub tenant_id: Uuid,
    pub subscribed: usize,
    pub removed: usize,
    pub invalid_event_type_ids: Vec<String>,
}
