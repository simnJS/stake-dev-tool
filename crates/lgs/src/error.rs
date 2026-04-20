use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("missing field: {0}")]
    MissingField(&'static str),
    #[error("session not found")]
    SessionNotFound,
    #[error("insufficient balance")]
    InsufficientBalance,
    #[error("mode \"{mode}\" not found for game \"{game}\"")]
    ModeNotFound { game: String, mode: String },
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("zstd error: {0}")]
    Zstd(String),
    #[error("parse error: {0}")]
    Parse(String),
    #[error("internal: {0}")]
    Internal(#[from] anyhow::Error),
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match &self {
            AppError::MissingField(_) | AppError::InsufficientBalance => StatusCode::BAD_REQUEST,
            AppError::SessionNotFound | AppError::ModeNotFound { .. } => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        let body = Json(ErrorBody { error: self.to_string() });
        (status, body).into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;
