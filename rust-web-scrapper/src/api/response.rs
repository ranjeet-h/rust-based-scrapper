use serde::Serialize;
use axum::Json;
use axum::http::StatusCode;
use chrono::Utc;

#[derive(Serialize)]
pub struct ApiResponse<T> {
    pub data: Option<T>,
    pub meta: ResponseMeta,
}

#[derive(Serialize)]
pub struct ResponseMeta {
    pub status: String,
    pub status_code: u16,
    pub timestamp: String,
    pub message: Option<String>,
}

pub fn success<T: Serialize>(data: T) -> (StatusCode, Json<ApiResponse<T>>) {
    let meta = ResponseMeta {
        status: "success".to_string(),
        status_code: StatusCode::OK.as_u16(),
        timestamp: Utc::now().to_rfc3339(),
        message: None,
    };

    (
        StatusCode::OK,
        Json(ApiResponse {
            data: Some(data),
            meta,
        }),
    )
}

pub fn error<T>(status: StatusCode, message: String) -> (StatusCode, Json<ApiResponse<T>>) {
    let meta = ResponseMeta {
        status: "error".to_string(),
        status_code: status.as_u16(),
        timestamp: Utc::now().to_rfc3339(),
        message: Some(message),
    };

    (
        status,
        Json(ApiResponse {
            data: None,
            meta,
        }),
    )
} 