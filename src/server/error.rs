//! HTTP error envelope wrapping [`crate::Error`].
//!
//! Lets handlers `?`-propagate without writing per-route mapping.
//! Status codes per [`crate::Error`] variant.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};

use crate::error::Error;

/// Wraps [`crate::Error`] so handler return types compose cleanly.
pub(super) struct ApiError(pub(super) Error);

impl From<Error> for ApiError {
    fn from(e: Error) -> Self {
        Self(e)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code) = match &self.0 {
            Error::Modality(_) => (StatusCode::BAD_REQUEST, "modality"),
            Error::Incompatible(_) => (StatusCode::CONFLICT, "incompatible"),
            Error::Index(_) => (StatusCode::INTERNAL_SERVER_ERROR, "index"),
            Error::Ingest(_) => (StatusCode::SERVICE_UNAVAILABLE, "ingest"),
            Error::Rerank(_) => (StatusCode::INTERNAL_SERVER_ERROR, "rerank"),
            Error::Io(_) => (StatusCode::INTERNAL_SERVER_ERROR, "io"),
        };
        let body = Json(serde_json::json!({
            "error": code,
            "message": self.0.to_string(),
        }));
        (status, body).into_response()
    }
}
