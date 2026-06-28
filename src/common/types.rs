use axum::{
    async_trait,
    extract::{rejection::JsonRejection, FromRequest, Json, Request},
};
use serde::{Deserialize, Serialize};
use serde::de::DeserializeOwned;
use uuid::Uuid;

use crate::error::{AppError, FieldError};

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
    pub organization_id: Uuid,
    pub tenant_id: Uuid,
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
            Err(rejection) => Err(json_rejection_to_app_error(rejection)),
        }
    }
}

fn json_rejection_to_app_error(rejection: JsonRejection) -> AppError {
    match rejection {
        JsonRejection::JsonDataError(e) => parse_data_error(&e.body_text()),
        JsonRejection::JsonSyntaxError(_) => {
            AppError::BadRequest("Malformed JSON: invalid syntax".to_owned())
        }
        JsonRejection::MissingJsonContentType(_) => {
            AppError::BadRequest("Content-Type must be application/json".to_owned())
        }
        JsonRejection::BytesRejection(_) => {
            AppError::BadRequest("Failed to read request body".to_owned())
        }
        _ => AppError::BadRequest("Invalid request body".to_owned()),
    }
}

/// Parses a serde_json data error message and returns a structured error.
///
/// serde_json patterns handled:
///   missing field `{field}` at …          → Validation required
///   unknown field `{field}`, expected …   → BadRequest
///   invalid type/value: … for key `{field}` → Validation invalid_value
fn parse_data_error(raw: &str) -> AppError {
    // axum wraps the serde message with a prefix; strip it so pattern matching
    // works on the raw serde string regardless of axum version.
    let msg = raw
        .split_once("target type: ")
        .map(|(_, rest)| rest)
        .unwrap_or(raw);

    if let Some(field) = extract_backtick(msg, "missing field `") {
        return AppError::Validation(vec![FieldError::new(
            field,
            "required",
            format!("'{}' is required", field),
        )]);
    }

    if let Some(field) = extract_backtick(msg, "unknown field `") {
        return AppError::BadRequest(format!("Unknown field: '{}'", field));
    }

    // "invalid type: …, expected … for key `{field}`"
    // "invalid value: …, expected … for key `{field}`"
    if let Some(field) = extract_backtick(msg, "for key `") {
        return AppError::Validation(vec![FieldError::new(
            field,
            "invalid_value",
            format!("'{}' has an invalid value", field),
        )]);
    }

    AppError::BadRequest("Invalid request body".to_owned())
}

fn extract_backtick<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    let start = s.find(prefix)? + prefix.len();
    let end = s[start..].find('`')?;
    Some(&s[start..start + end])
}
