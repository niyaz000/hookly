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
        };

        (status, Json(body)).into_response()
    }
}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        AppError::Database(err)
    }
}

impl From<redis::RedisError> for AppError {
    fn from(err: redis::RedisError) -> Self {
        AppError::Redis(err)
    }
}
