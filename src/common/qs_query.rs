use axum::{
    async_trait,
    extract::FromRequestParts,
    http::request::Parts,
};
use serde::de::DeserializeOwned;

use crate::error::AppError;

pub struct QsQuery<T>(pub T);

#[async_trait]
impl<T, S> FromRequestParts<S> for QsQuery<T>
where
    T: DeserializeOwned + Send,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let query = parts.uri.query().unwrap_or("");
        let value = serde_qs::from_str::<T>(query)
            .map_err(|e| AppError::BadRequest(format!("invalid query parameters: {}", e)))?;
        Ok(QsQuery(value))
    }
}
