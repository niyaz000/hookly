use axum::{async_trait, extract::{FromRef, FromRequestParts}, http::request::Parts};
use chrono::Utc;
use uuid::Uuid;

use crate::error::AppError;
use crate::state::AppState;

use super::crypto;
use super::models::ApiKeyStatus;
use super::repository::ApiKeyRepository;

/// Injected into request extensions by the `authenticate` middleware for all protected routes.
#[derive(Clone)]
pub struct ApiKeyPrincipal {
    pub api_key_public_id: String,
    pub tenant_id: Uuid,
    pub user_id: Uuid,
}

#[async_trait]
impl<S> FromRequestParts<S> for ApiKeyPrincipal
where
    S: Send + Sync,
    AppState: axum::extract::FromRef<S>,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let app_state = AppState::from_ref(state);

        let auth_header = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| AppError::Unauthorized("missing Authorization header".into()))?;

        let token = auth_header.strip_prefix("Bearer ").ok_or_else(|| {
            AppError::Unauthorized("Authorization header must be a Bearer token".into())
        })?;

        if token.is_empty() {
            return Err(AppError::Unauthorized("Bearer token must not be empty".into()));
        }

        let key_hash = crypto::hash_key(token);
        let repo = ApiKeyRepository::new(app_state.db.clone());

        let api_key = repo
            .get_by_hash(&key_hash)
            .await?
            .ok_or_else(|| AppError::Unauthorized("invalid or revoked api key".into()))?;

        if api_key.deleted_at.is_some() {
            return Err(AppError::Unauthorized("api key has been revoked".into()));
        }
        if api_key.status == ApiKeyStatus::Expired {
            return Err(AppError::Unauthorized("api key has expired".into()));
        }
        if let Some(expires_at) = api_key.expires_at {
            if expires_at < Utc::now() {
                return Err(AppError::Unauthorized("api key has expired".into()));
            }
        }

        // Update last_used_at asynchronously — do not block the request
        let pool = app_state.db.clone();
        let key_id = api_key.id;
        tokio::spawn(async move {
            let _ = sqlx::query("UPDATE api_keys SET last_used_at = NOW() WHERE id = $1")
                .bind(key_id)
                .execute(&pool)
                .await;
        });

        Ok(ApiKeyPrincipal {
            api_key_public_id: api_key.public_id,
            tenant_id: api_key.tenant_id,
            user_id: api_key.user_id,
        })
    }
}
