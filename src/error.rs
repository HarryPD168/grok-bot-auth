use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Msg(String),
    #[error(transparent)]
    Http(#[from] reqwest::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let status = match &self {
            Error::Msg(message) if message.contains("not a Cursor") => StatusCode::BAD_REQUEST,
            Error::Msg(message) if message.contains("missing") => StatusCode::BAD_REQUEST,
            Error::Msg(message) if message.contains("sign in") => StatusCode::UNAUTHORIZED,
            _ => StatusCode::BAD_GATEWAY,
        };
        (status, Json(json!({ "error": self.to_string() }))).into_response()
    }
}
