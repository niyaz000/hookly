use axum::{async_trait, extract::FromRequestParts, http::request::Parts};

use crate::error::AppError;

/// Injected into request extensions by the `authenticate` middleware for all protected routes.
#[derive(Clone)]
pub struct ApiKeyPrincipal {
    pub api_key_public_id: String,
    pub organization_id: uuid::Uuid,
    pub tenant_id: uuid::Uuid,
    pub user_id: uuid::Uuid,
}

/// Reads the principal that the `authenticate` middleware already placed in extensions.
/// Returns Unauthorized if called on a route that isn't covered by that middleware.
#[async_trait]
impl<S> FromRequestParts<S> for ApiKeyPrincipal
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<ApiKeyPrincipal>()
            .cloned()
            .ok_or_else(|| AppError::Unauthorized("not authenticated".into()))
    }
}
