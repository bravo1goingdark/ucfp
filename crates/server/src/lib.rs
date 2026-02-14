//! UCFP Server - HTTP REST API for Universal Content Fingerprinting
//!
//! This crate provides a production-ready HTTP server that exposes UCFP
//! functionality via a REST API. It supports:
//!
//! - **Document Processing**: Single and batch document fingerprinting
//! - **Index Management**: Insert, search, and manage document indexes
//! - **Document Matching**: Find similar documents using perceptual/semantic similarity
//! - **Health & Metrics**: Liveness/readiness probes and Prometheus-compatible metrics
//!
//! # Features
//!
//! - **Authentication**: API key-based authentication with rate limiting
//! - **Middleware**: Compression, CORS, request ID tracking, structured logging
//! - **Configuration**: Environment variable and file-based configuration
//! - **Error Handling**: Comprehensive error responses with error codes
//! - **Graceful Shutdown**: Proper signal handling for production deployments
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use server::ServerConfig;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let config = ServerConfig::load()?;
//!     server::start_server(config).await?;
//!     Ok(())
//! }
//! ```
//!
//! # API Endpoints
//!
//! ## Public Endpoints (No Authentication)
//!
//! - `GET /` - API information
//! - `GET /health` - Liveness probe
//! - `GET /ready` - Readiness probe
//! - `GET /metrics` - Prometheus metrics
//!
//! ## Protected Endpoints (API Key Required)
//!
//! - `POST /api/v1/process` - Process single document
//! - `POST /api/v1/batch` - Batch process documents
//! - `POST /api/v1/index/insert` - Insert into index
//! - `GET /api/v1/index/search` - Search index
//! - `GET /api/v1/index/stats` - Index statistics
//! - `GET /api/v1/index/documents` - List documents
//! - `GET /api/v1/index/documents/:id` - Get document by ID
//! - `DELETE /api/v1/index/documents/:id` - Delete document
//! - `POST /api/v1/match` - Match documents
//! - `POST /api/v1/compare` - Compare two documents
//! - `GET /api/v1/pipeline/status` - Pipeline status
//! - `GET /api/v1/metadata` - Server metadata
//!
//! See the README.md and API.md files for complete documentation.

pub mod config;
pub mod error;
pub mod middleware;
pub mod routes;
pub mod server;
pub mod state;

pub use config::ServerConfig;
pub use error::{ServerError, ServerResult};
pub use server::start_server;
pub use state::ServerState;
