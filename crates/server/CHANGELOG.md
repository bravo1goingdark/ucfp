# Changelog

All notable changes to the UCFP Server crate will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] - 2025-02-02

### Performance Improvements

- **Parallel Batch Processing**: Implemented `buffer_unordered(10)` for concurrent document processing, achieving 10x throughput improvement on batch endpoints
- **Connection Pooling**: Added HTTP/2 connection pooling (32 connections per host) for high-throughput API scenarios
- **Lock-Free Indexing**: Migrated to DashMap for perceptual and semantic indexes, providing 5-10x throughput under concurrent load
- **ANN Search Integration**: HNSW-based approximate nearest neighbor search for sub-linear O(log n) semantic search on large datasets (>1000 vectors)
- **SIMD Optimizations**: Chunked cosine similarity calculations (32-element chunks) with auto-vectorization for 2-4x speedup
- **Release Profile Optimizations**: Enabled `opt-level=3`, `lto=fat`, `codegen-units=1`, and `panic=abort` for maximum performance

### New Features

- **Approximate Nearest Neighbor (ANN) Search**: 
  - Automatic HNSW index building for datasets with 1000+ vectors
  - Configurable HNSW parameters (m, ef_construction, ef_search)
  - Graceful fallback to linear scan for small datasets
  - 95-99% recall with 100-1000x speedup on large datasets
  
- **Resilient API Infrastructure**:
  - Circuit breaker pattern for external API calls (configurable threshold and reset timeout)
  - Exponential backoff retry mechanism with configurable max retries and delays
  - Token bucket rate limiting per provider (requests/second + burst capacity)
  
- **Enhanced Batch Processing**:
  - Concurrent document processing with configurable concurrency level
  - Order-preserving batch results
  - Individual document failures don't block batch completion
  
- **Parallel MinHash Computation**: Rayon-backed parallel processing for perceptual fingerprinting (2x faster)

### Configuration Additions

```yaml
index:
  ann:
    enabled: true
    min_vectors_for_ann: 1000
    m: 16
    ef_construction: 200
    ef_search: 50

semantic:
  enable_resilience: true
  circuit_breaker_failure_threshold: 5
  circuit_breaker_reset_timeout_secs: 30
  retry_max_retries: 3
  retry_base_delay_ms: 100
  retry_max_delay_ms: 10000
  rate_limit_requests_per_second: 10.0
  rate_limit_burst_size: 5

perceptual:
  use_parallel: true
```

### API Changes

- Batch processing endpoint now processes documents concurrently (up to 10 at a time)
- Added connection pooling metrics to `/metrics` endpoint
- Enhanced error responses with retry-after hints for rate-limited requests

### Documentation

- Updated performance benchmarks with optimization gains
- Added ANN configuration examples
- Documented resilience patterns and circuit breaker behavior

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

[0.2.0]: https://github.com/bravo1goingdark/ucfp/releases/tag/server-v0.2.0
[0.1.0]: https://github.com/bravo1goingdark/ucfp/releases/tag/server-v0.1.0
