use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("{0}")]
    General(String),
    
    #[error("Not Found: {0}")]
    NotFound(String),
    
    #[error("Unauthorized: {0}")]
    Unauthorized(String),
    
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    
    #[error(transparent)]
    Io(#[from] std::io::Error),
    
    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),
    #[error(transparent)]
    ZipError(#[from] zip::result::ZipError),
    #[error(transparent)]
    BoxDynError(#[from] Box<dyn std::error::Error>),
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),
}

impl AppError {
    pub fn new(msg: &str) -> Self {
        AppError::General(msg.to_string())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            AppError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg),
            AppError::Sqlx(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
            AppError::Io(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
            AppError::Anyhow(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
            AppError::General(msg) => (StatusCode::OK, msg),
            AppError::ZipError(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
            AppError::BoxDynError(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
            AppError::SerdeJson(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
            // Since CodePush client expects 200 OK with custom error format or standard HTTP statuses depending on route,
            // we default to OK with message for now for AppError::General, and 500 for others.
            _ => (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string()),
        };

        let body = Json(json!({
            "status": "ERROR",
            "message": error_message,
        }));

        (status, body).into_response()
    }
}

// Implement converters from String
impl From<String> for AppError {
    fn from(err: String) -> Self {
        AppError::General(err)
    }
}

impl From<&str> for AppError {
    fn from(err: &str) -> Self {
        AppError::General(err.to_string())
    }
}
