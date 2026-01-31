# UCFP Server

HTTP REST API server for Universal Content Fingerprinting (UCFP).

## Overview

The UCFP Server provides a production-ready HTTP interface to the UCFP content fingerprinting pipeline, exposing endpoints for:
- Document processing (perceptual + semantic fingerprinting)
- Batch processing
- Index management (insert/search)
- Document matching and comparison
- Health checks and metrics

## Features

- **RESTful API**: Standard HTTP JSON API with consistent error responses
- **Authentication**: API key-based authentication with rate limiting
- **Middleware Stack**: Compression, CORS, request ID tracking, structured logging
- **Health Checks**: Liveness (`/health`) and readiness (`/ready`) endpoints
- **Metrics**: Prometheus-compatible metrics endpoint
- **Graceful Shutdown**: Handles SIGTERM and Ctrl+C properly
- **Configurable**: Environment variable and file-based configuration

## Quick Start

### Running the Server

```bash
# Build the server
cargo build -p server --release

# Run with default settings (uses demo API key)
cargo run -p server

# Run with custom configuration
cargo run -p server -- --config server.yaml
```

### Testing the API

```bash
# Health check (no auth required)
curl http://localhost:8080/health

# Process a document (requires API key)
curl -X POST http://localhost:8080/api/v1/process \
  -H "Content-Type: application/json" \
  -H "X-API-Key: demo-key-12345" \
  -d '{
    "doc_id": "doc-001",
    "tenant_id": "tenant-001",
    "text": "This is a sample document for fingerprinting.",
    "enable_perceptual": true,
    "enable_semantic": true
  }'
```

## Configuration

Configuration can be provided via:
1. **Configuration file**: `server.yaml` or `server.json`
2. **Environment variables**: Prefixed with `UCFP_SERVER__`

### Configuration Options

| Option | Default | Description |
|--------|---------|-------------|
| `bind_addr` | `0.0.0.0` | Server bind address |
| `port` | `8080` | Server port |
| `timeout_secs` | `30` | Request timeout in seconds |
| `max_body_size_mb` | `10` | Maximum request body size |
| `rate_limit_per_minute` | `100` | Rate limit per API key |
| `api_keys` | `[]` | Valid API keys (empty = demo key) |
| `enable_cors` | `true` | Enable CORS headers |
| `log_level` | `info` | Logging level |
| `metrics_enabled` | `true` | Enable metrics endpoint |

### Example Configuration File

```yaml
# server.yaml
bind_addr: "0.0.0.0"
port: 8080
timeout_secs: 30
max_body_size_mb: 10
rate_limit_per_minute: 100
api_keys:
  - "prod-key-001"
  - "prod-key-002"
enable_cors: true
log_level: "info"
metrics_enabled: true
```

### Environment Variables

```bash
export UCFP_SERVER__PORT=9090
export UCFP_SERVER__LOG_LEVEL=debug
export UCFP_SERVER__API_KEYS='["my-api-key"]'
cargo run -p server
```

## API Endpoints

### Public Endpoints (No Authentication)

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/` | GET | API info and available endpoints |
| `/health` | GET | Liveness probe |
| `/ready` | GET | Readiness probe |
| `/metrics` | GET | Prometheus metrics |

### Protected Endpoints (API Key Required)

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/v1/process` | POST | Process single document |
| `/api/v1/batch` | POST | Process multiple documents |
| `/api/v1/index/insert` | POST | Insert record into index |
| `/api/v1/index/search` | GET | Search the index |
| `/api/v1/index/documents` | GET | List all documents |
| `/api/v1/index/documents/:doc_id` | DELETE | Delete document |
| `/api/v1/match` | POST | Match documents |
| `/api/v1/compare` | POST | Compare two documents |
| `/api/v1/metadata` | GET | Server metadata |

## API Documentation

### Authentication

All protected endpoints require an API key in one of these headers:
- `X-API-Key: your-api-key`
- `Authorization: Bearer your-api-key`

### Process Document

**Endpoint:** `POST /api/v1/process`

Process a single document through the fingerprinting pipeline.

**Request:**
```json
{
  "doc_id": "optional-doc-id",
  "tenant_id": "optional-tenant-id",
  "text": "Document content to fingerprint",
  "enable_perceptual": true,
  "enable_semantic": true,
  "perceptual_config": {
    "k": 9,
    "w": 4,
    "minhash_bands": 16
  },
  "semantic_config": {
    "tier": "balanced",
    "normalize": true
  }
}
```

**Response:**
```json
{
  "doc_id": "doc-id",
  "tenant_id": "tenant-id",
  "status": "success",
  "canonical_hash": "sha256-hash",
  "perceptual_fingerprint": {
    "minhash": [12345, 67890, ...],
    "minhash_bands": 16
  },
  "semantic_embedding": {
    "vector": [0.1, 0.2, ...],
    "model_name": "model-name",
    "embedding_dim": 384
  }
}
```

### Batch Processing

**Endpoint:** `POST /api/v1/batch`

Process multiple documents efficiently.

**Request:**
```json
{
  "documents": [
    {"doc_id": "1", "text": "First document"},
    {"doc_id": "2", "text": "Second document"}
  ],
  "enable_perceptual": true,
  "enable_semantic": false
}
```

**Response:**
```json
{
  "processed": 2,
  "successful": 2,
  "failed": 0,
  "results": [...]
}
```

### Index Operations

**Insert:** `POST /api/v1/index/insert`
```json
{
  "doc_id": "doc-001",
  "tenant_id": "tenant-001",
  "canonical_hash": "sha256-hash",
  "perceptual_fingerprint": [12345, 67890],
  "semantic_embedding": [0.1, 0.2, ...],
  "metadata": {"key": "value"}
}
```

**Search:** `GET /api/v1/index/search?query=text&strategy=perceptual&top_k=10`

**Response:**
```json
{
  "query": "text",
  "strategy": "perceptual",
  "total_hits": 5,
  "hits": [
    {"doc_id": "doc-001", "score": 0.95, "tenant_id": "tenant-001"}
  ]
}
```

### Matching

**Endpoint:** `POST /api/v1/match`

Find similar documents.

**Request:**
```json
{
  "query": "Search query text",
  "tenant_id": "tenant-001",
  "strategy": "hybrid",
  "max_results": 10,
  "min_score": 0.8
}
```

## Error Responses

All errors follow this format:

```json
{
  "error": {
    "code": "ERROR_CODE",
    "message": "Human-readable error description"
  }
}
```

### Error Codes

| Code | HTTP Status | Description |
|------|-------------|-------------|
| `AUTH_FAILED` | 401 | Invalid or missing API key |
| `RATE_LIMIT_EXCEEDED` | 429 | Too many requests |
| `REQUEST_TIMEOUT` | 408 | Request timed out |
| `BAD_REQUEST` | 400 | Invalid request format |
| `PAYLOAD_TOO_LARGE` | 413 | Request body exceeds limit |
| `PIPELINE_ERROR` | 422 | Processing pipeline error |
| `INTERNAL_ERROR` | 500 | Internal server error |

## Deployment

### Docker

```dockerfile
FROM rust:1.75 as builder
WORKDIR /app
COPY . .
RUN cargo build -p server --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates
COPY --from=builder /app/target/release/server /usr/local/bin/
EXPOSE 8080
CMD ["server"]
```

### Kubernetes

See `k8s/` directory for example deployment manifests.

### systemd Service

```ini
[Unit]
Description=UCFP Server
After=network.target

[Service]
Type=simple
User=ucfp
WorkingDirectory=/opt/ucfp
Environment="UCFP_SERVER__PORT=8080"
Environment="UCFP_SERVER__LOG_LEVEL=info"
ExecStart=/opt/ucfp/server
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
```

## Development

### Building

```bash
# Debug build
cargo build -p server

# Release build
cargo build -p server --release
```

### Testing

```bash
# Run unit tests
cargo test -p server

# Run integration tests (requires server running)
cargo test -p server -- --ignored
```

### Adding New Endpoints

1. Add route handler in `src/routes/`
2. Register route in `src/server.rs`
3. Update API documentation
4. Add tests

## Architecture

```
┌─────────────────────────────────────────┐
│           Axum HTTP Server              │
├─────────────────────────────────────────┤
│ Middleware:                             │
│   - Request ID                          │
│   - Logging                             │
│   - Compression                         │
│   - CORS                                │
│   - Timeout                             │
│   - API Key Auth                        │
│   - Rate Limiting                       │
├─────────────────────────────────────────┤
│ Routes:                                 │
│   - /api/v1/process                     │
│   - /api/v1/batch                       │
│   - /api/v1/index/*                     │
│   - /api/v1/match                       │
│   - /health, /ready, /metrics           │
├─────────────────────────────────────────┤
│ State:                                  │
│   - Shared Index                        │
│   - Shared Matcher                      │
│   - Rate Limiter                        │
│   - Configuration                       │
└─────────────────────────────────────────┘
```

## License

Apache 2.0 - See [LICENSE](../LICENSE) for details.
