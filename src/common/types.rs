use axum::{
    async_trait,
    extract::{rejection::JsonRejection, FromRequest, Json, Request},
};
use serde::{Deserialize, Serialize};
use serde::de::DeserializeOwned;
use uuid::Uuid;

use crate::error::AppError;

#[allow(dead_code)]
#[derive(Serialize, Deserialize, Debug)]
pub struct PaginatedResponse<T> {
    pub items: Vec<T>,
    pub total: i64,
    pub page: i32,
    pub limit: i32,
}

/// Shared metadata for tracing and audit on write operations.
#[derive(Debug, Clone, Copy)]
pub struct RequestContext {
    pub request_id: Uuid,
    pub created_by: Uuid,
}

/// JSON body extractor that converts Axum's JsonRejection into a structured AppError.
pub struct ValidatedJson<T>(pub T);

#[async_trait]
impl<T, S> FromRequest<S> for ValidatedJson<T>
where
    T: DeserializeOwned + Send,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        match Json::<T>::from_request(req, state).await {
            Ok(Json(val)) => Ok(ValidatedJson(val)),
            Err(rejection) => Err(AppError::BadRequest(json_rejection_message(rejection))),
        }
    }
}

fn json_rejection_message(rejection: JsonRejection) -> String {
    match rejection {
        JsonRejection::JsonDataError(e) => format!("Invalid request body: {}", e.body_text()),
        JsonRejection::JsonSyntaxError(e) => format!("Malformed JSON: {}", e.body_text()),
        JsonRejection::MissingJsonContentType(_) => {
            "Content-Type must be application/json".to_owned()
        }
        JsonRejection::BytesRejection(_) => "Failed to read request body".to_owned(),
        _ => "Invalid request body".to_owned(),
    }
}
