use axum::{
    response::{IntoResponse, Response},
    Json,
    http::StatusCode,
};
use serde::Serialize;

#[derive(Serialize)]
pub struct ErrorResponse {
    error: String,
}

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Failed to fetch data: {0}")]
    FetchError(String),
    
    #[error("LLM processing error: {0}")]
    LlmError(String),
    
    #[error("Error parsing content: {0}")]
    ParseError(String),
    
    #[error("Configuration error: {0}")]
    ConfigError(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AppError::FetchError(msg) => (StatusCode::BAD_REQUEST, msg),
            AppError::LlmError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            AppError::ParseError(msg) => (StatusCode::UNPROCESSABLE_ENTITY, msg),
            AppError::ConfigError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };

        let body = Json(ErrorResponse {
            error: error_message,
        });

        (status, body).into_response()
    }
}

impl From<reqwest::Error> for AppError {
    fn from(err: reqwest::Error) -> Self {
        AppError::FetchError(err.to_string())
    }
}

impl From<std::env::VarError> for AppError {
    fn from(err: std::env::VarError) -> Self {
        AppError::ConfigError(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, AppError>; 