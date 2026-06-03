use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use uuid::Uuid;

tokio::task_local! {
    pub static REQUEST_ID: Uuid;
}

#[derive(Debug, Serialize, Clone)]
pub struct FieldError {
    pub field: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    pub code: String,
    pub message: String,
}

impl FieldError {
    pub fn new(
        field: impl Into<String>,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            field: field.into(),
            value: None,
            code: code.into(),
            message: message.into(),
        }
    }

    pub fn with_value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }
}

#[derive(Serialize)]
struct ErrorBody {
    error_code: String,
    error_message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    errors: Option<Vec<FieldError>>,
    request_id: String,
    doc_url: String,
}

impl ErrorBody {
    fn new(error_code: &str, error_message: impl Into<String>) -> Self {
        let request_id = REQUEST_ID
            .try_with(|id| id.to_string())
            .unwrap_or_else(|_| Uuid::new_v4().to_string());
        Self {
            error_code: error_code.to_owned(),
            error_message: error_message.into(),
            errors: None,
            request_id,
            doc_url: format!("https://docs.hookly.dev/errors/{error_code}"),
        }
    }

    fn with_errors(mut self, errors: Vec<FieldError>) -> Self {
        self.errors = Some(errors);
        self
    }
}

#[derive(Debug)]
pub enum AppError {
    Database(sqlx::Error),
    Redis(redis::RedisError),
    NotFound(String),
    BadRequest(String),
    Validation(Vec<FieldError>),
    Conflict(String),
    Internal(String),
    Unauthorized(String),
    PayloadTooLarge,
    UriTooLong,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, body) = match self {
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, ErrorBody::new("not_found", msg)),
            AppError::BadRequest(msg) => {
                (StatusCode::BAD_REQUEST, ErrorBody::new("bad_request", msg))
            }
            AppError::Validation(errs) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                ErrorBody::new("validation_error", "Request validation failed").with_errors(errs),
            ),
            AppError::Conflict(msg) => (StatusCode::CONFLICT, ErrorBody::new("conflict", msg)),
            AppError::Database(e) => {
                tracing::error!(error = %e, "database error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    ErrorBody::new("internal_error", "An internal error occurred"),
                )
            }
            AppError::Redis(e) => {
                tracing::error!(error = %e, "redis error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    ErrorBody::new("internal_error", "An internal error occurred"),
                )
            }
            AppError::Internal(msg) => {
                tracing::error!(error = %msg, "internal error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    ErrorBody::new("internal_error", "An internal error occurred"),
                )
            }
            AppError::Unauthorized(msg) => {
                (StatusCode::UNAUTHORIZED, ErrorBody::new("unauthorized", msg))
            }
            AppError::PayloadTooLarge => (
                StatusCode::PAYLOAD_TOO_LARGE,
                ErrorBody::new("payload_too_large", "Request body exceeds the 256 KB limit"),
            ),
            AppError::UriTooLong => (
                StatusCode::URI_TOO_LONG,
                ErrorBody::new("uri_too_long", "Request URI exceeds the 512 character limit"),
            ),
        };

        (status, Json(body)).into_response()
    }
}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        if let sqlx::Error::Database(ref db_err) = err {
            match db_err.code().as_deref() {
                Some("23505") => {
                    let msg = conflict_message(db_err.constraint(), db_err.table());
                    return AppError::Conflict(msg);
                }
                Some("23503") => {
                    return AppError::BadRequest("Referenced resource does not exist".into());
                }
                _ => {}
            }
        }
        AppError::Database(err)
    }
}

// Builds a human-readable conflict message from the constraint and table names.
// Convention: `{table}_{field}_uq` or `idx_{table}_{field}`.
fn conflict_message(constraint: Option<&str>, table: Option<&str>) -> String {
    let field = constraint.and_then(|c| {
        let c = c.strip_prefix("idx_").unwrap_or(c);
        let c = c.strip_suffix("_uq").unwrap_or(c);
        let c = if let Some(t) = table {
            c.strip_prefix(t).and_then(|r| r.strip_prefix('_')).unwrap_or(c)
        } else {
            c
        };
        if c.is_empty() { None } else { Some(c.replace('_', " ")) }
    });

    match field {
        Some(f) => format!("A resource with this {} already exists", f),
        None => "A resource with this value already exists".into(),
    }
}

impl From<validator::ValidationErrors> for AppError {
    fn from(errs: validator::ValidationErrors) -> Self {
        let field_errors = errs
            .field_errors()
            .into_iter()
            .flat_map(|(field, errors)| {
                errors.iter().map(move |e| {
                    let code = remap_validator_code(e);
                    let message = e
                        .message
                        .as_deref()
                        .unwrap_or(e.code.as_ref())
                        .to_owned();
                    FieldError::new(field, code, message)
                })
            })
            .collect();
        AppError::Validation(field_errors)
    }
}

fn remap_validator_code(e: &validator::ValidationError) -> &'static str {
    match e.code.as_ref() {
        "email" => "invalid_format",
        "length" => {
            let max = e.params.get("max").and_then(|v| v.as_u64());
            let min = e.params.get("min").and_then(|v| v.as_u64());
            let is_array = e.params.get("value").map(|v| v.is_array()).unwrap_or(false);
            let actual: Option<u64> = e.params.get("value").and_then(|v| match v {
                serde_json::Value::String(s) => Some(s.len() as u64),
                serde_json::Value::Array(a) => Some(a.len() as u64),
                _ => None,
            });
            if let (Some(mx), Some(len)) = (max, actual) {
                if len > mx {
                    return "max_length";
                }
            }
            if min.is_some() {
                if is_array {
                    return "min_items";
                }
                return "required";
            }
            "length"
        }
        other => {
            // Custom validators set their own codes; leak to 'static via a fixed set.
            match other {
                "required" => "required",
                "invalid_format" => "invalid_format",
                "invalid_value" => "invalid_value",
                "min_items" => "min_items",
                _ => "validation_error",
            }
        }
    }
}

impl From<redis::RedisError> for AppError {
    fn from(err: redis::RedisError) -> Self {
        AppError::Redis(err)
    }
}
