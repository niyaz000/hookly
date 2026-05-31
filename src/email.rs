use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::error::AppError;

#[async_trait]
pub trait EmailService: Send + Sync {
    async fn send_invite(
        &self,
        to: &str,
        token: &str,
        role: &str,
        expires_at: DateTime<Utc>,
    ) -> Result<(), AppError>;
}

pub struct NoopEmailService;

#[async_trait]
impl EmailService for NoopEmailService {
    async fn send_invite(
        &self,
        to: &str,
        _token: &str,
        role: &str,
        expires_at: DateTime<Utc>,
    ) -> Result<(), AppError> {
        tracing::info!(
            to = %to,
            role = %role,
            expires_at = %expires_at,
            "noop: skipping invite email delivery"
        );
        Ok(())
    }
}
