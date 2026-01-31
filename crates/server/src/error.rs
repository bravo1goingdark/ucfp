use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::json;

pub type ServerResult<T> = Result<T, ServerError>;

/// Server error types
#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    #[error("Authentication failed: {0}")]
    Authentication(String),

    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    #[error("Request timeout")]
    Timeout,

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Payload too large: max {0}MB allowed")]
    PayloadTooLarge(usize),

    #[error("Pipeline error: {0}")]
    Pipeline(#[from] ucfp::PipelineError),

    #[error("Ingest error: {0}")]
    Ingest(#[from] ingest::IngestError),

    #[error("Canonical error: {0}")]
    Canonical(#[from] canonical::CanonicalError),

    #[error("Perceptual error: {0}")]
    Perceptual(#[from] perceptual::PerceptualError),

    #[error("Semantic error: {0}")]
    Semantic(#[from] semantic::SemanticError),

    #[error("Index error: {0}")]
    Index(#[from] index::IndexError),

    #[error("Match error: {0}")]
    Match(#[from] matcher::MatchError),

    #[error("Internal server error: {0}")]
    Internal(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Not found")]
    NotFound,
}

/// API error response structure
#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: ErrorDetail,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorDetail {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl ServerError {
    /// Get HTTP status code for this error
    fn status_code(&self) -> StatusCode {
        match self {
            ServerError::Authentication(_) => StatusCode::UNAUTHORIZED,
            ServerError::RateLimitExceeded => StatusCode::TOO_MANY_REQUESTS,
            ServerError::Timeout => StatusCode::REQUEST_TIMEOUT,
            ServerError::BadRequest(_) => StatusCode::BAD_REQUEST,
            ServerError::PayloadTooLarge(_) => StatusCode::PAYLOAD_TOO_LARGE,
            ServerError::NotFound => StatusCode::NOT_FOUND,
            ServerError::Pipeline(_)
            | ServerError::Ingest(_)
            | ServerError::Canonical(_)
            | ServerError::Perceptual(_)
            | ServerError::Semantic(_)
            | ServerError::Index(_)
            | ServerError::Match(_) => StatusCode::UNPROCESSABLE_ENTITY,
            ServerError::Internal(_) | ServerError::Config(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// Get error code string
    fn error_code(&self) -> &'static str {
        match self {
            ServerError::Authentication(_) => "AUTH_FAILED",
            ServerError::RateLimitExceeded => "RATE_LIMIT_EXCEEDED",
            ServerError::Timeout => "REQUEST_TIMEOUT",
            ServerError::BadRequest(_) => "BAD_REQUEST",
            ServerError::PayloadTooLarge(_) => "PAYLOAD_TOO_LARGE",
            ServerError::Pipeline(_) => "PIPELINE_ERROR",
            ServerError::Ingest(_) => "INGEST_ERROR",
            ServerError::Canonical(_) => "CANONICAL_ERROR",
            ServerError::Perceptual(_) => "PERCEPTUAL_ERROR",
            ServerError::Semantic(_) => "SEMANTIC_ERROR",
            ServerError::Index(_) => "INDEX_ERROR",
            ServerError::Match(_) => "MATCH_ERROR",
            ServerError::Internal(_) => "INTERNAL_ERROR",
            ServerError::Config(_) => "CONFIG_ERROR",
            ServerError::NotFound => "NOT_FOUND",
        }
    }
}

impl IntoResponse for ServerError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let error_code = self.error_code().to_string();
        let message = self.to_string();

        let body = Json(json!({
            "error": {
                "code": error_code,
                "message": message,
            }
        }));

        (status, body).into_response()
    }
}

impl From<std::net::AddrParseError> for ServerError {
    fn from(err: std::net::AddrParseError) -> Self {
        ServerError::Config(format!("Invalid address: {err}"))
    }
}

impl From<std::io::Error> for ServerError {
    fn from(err: std::io::Error) -> Self {
        ServerError::Internal(format!("IO error: {err}"))
    }
}

impl From<serde_json::Error> for ServerError {
    fn from(err: serde_json::Error) -> Self {
        ServerError::BadRequest(format!("JSON parse error: {err}"))
    }
}

impl From<anyhow::Error> for ServerError {
    fn from(err: anyhow::Error) -> Self {
        ServerError::Internal(err.to_string())
    }
}

// Display is automatically derived by thiserror::Error
