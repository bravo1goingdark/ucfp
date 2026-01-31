# Changelog

All notable changes to the UCFP Server crate will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2024-01-15

### Added
- Initial server implementation with Axum framework
- REST API endpoints for document processing
- API key-based authentication with rate limiting
- Health check endpoints (/health, /ready)
- Prometheus-compatible metrics endpoint (/metrics)
- Configuration support via files and environment variables
- Middleware stack: compression, CORS, timeout, logging, request ID tracking
- Single document processing endpoint (POST /api/v1/process)
- Batch processing endpoint (POST /api/v1/batch)
- Index management endpoints (insert, search, list, delete)
- Document matching and comparison endpoints
- Comprehensive error handling with error codes
- Graceful shutdown on SIGTERM and Ctrl+C
- Structured logging with tracing
- In-memory index backend support
- Example configuration file
- API client examples in Rust

### Features
- **Authentication**: API key validation with X-API-Key or Authorization: Bearer headers
- **Rate Limiting**: Per-key rate limiting with configurable limits (default: 100/min)
- **Request Timeout**: Configurable request timeout (default: 30s)
- **CORS**: Full CORS support for browser clients
- **Compression**: Automatic gzip/brotli compression
- **Error Responses**: Consistent JSON error format with error codes
- **Health Probes**: Kubernetes-compatible liveness and readiness probes
- **Metrics**: Prometheus-compatible metrics for monitoring

### Security
- API key authentication required for all mutating endpoints
- Rate limiting to prevent abuse
- Request size limits to prevent memory exhaustion
- CORS configuration for cross-origin requests

### Documentation
- README.md with quick start guide
- API.md with complete endpoint documentation
- Inline code documentation
- Example configuration file
- API client examples

[0.1.0]: https://github.com/bravo1goingdark/ucfp/releases/tag/server-v0.1.0
