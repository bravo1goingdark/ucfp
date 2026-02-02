# UCFP Server API Reference

Complete API documentation for the UCFP Server REST endpoints.

## Base URL

```
http://localhost:8080
```

## Authentication

All protected endpoints require authentication via API key.

### Headers
```
X-API-Key: your-api-key
# or
Authorization: Bearer your-api-key
```

### Response (401 Unauthorized)
```json
{
  "error": {
    "code": "AUTH_FAILED",
    "message": "API key required. Provide it in 'X-API-Key' or 'Authorization: Bearer <key>' header"
  }
}
```

---

## Public Endpoints

### GET /

Returns API information and available endpoints.

**Response:**
```json
{
  "name": "UCFP Server",
  "version": "0.1.0",
  "api_version": "v1",
  "endpoints": [
    "/api/v1/process",
    "/api/v1/batch",
    "/api/v1/index/insert",
    "/api/v1/index/search",
    "/api/v1/match",
    "/health",
    "/ready",
    "/metrics"
  ]
}
```

---

### GET /health

Liveness probe. Returns 200 if server is running.

**Response:**
```json
{
  "status": "healthy",
  "service": "ucfp-server",
  "timestamp": "2024-01-15T10:30:00Z",
  "uptime_seconds": 3600
}
```

---

### GET /ready

Readiness probe. Returns 200 if server is ready to accept requests.

**Response:**
```json
{
  "status": "ready",
  "service": "ucfp-server",
  "timestamp": "2024-01-15T10:30:00Z",
  "uptime_seconds": 3600,
  "components": {
    "api": "ready",
    "index": "ready"
  }
}
```

---

### GET /metrics

Prometheus-compatible metrics endpoint.

**Response:**
```json
{
  "uptime_seconds": 3600
}
```

---

## Protected Endpoints

### POST /api/v1/process

Process a single document through the fingerprinting pipeline.

**Request Headers:**
- `Content-Type: application/json`
- `X-API-Key: your-api-key`

**Request Body:**
```json
{
  "doc_id": "optional-custom-id",
  "tenant_id": "optional-tenant-id",
  "text": "Document content to fingerprint",
  "enable_perceptual": true,
  "enable_semantic": true,
  "perceptual_config": {
    "k": 9,
    "w": 4,
    "minhash_bands": 16,
    "use_parallel": true
  },
  "semantic_config": {
    "tier": "balanced",
    "normalize": true
  }
}
```

**Request Fields:**


| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `doc_id` | string | No | UUID | Unique document identifier |
| `tenant_id` | string | No | API key | Tenant for multi-tenancy |
| `text` | string | Yes | - | Document content |
| `enable_perceptual` | boolean | No | true | Generate perceptual fingerprint |
| `enable_semantic` | boolean | No | true | Generate semantic embedding |
| `perceptual_config` | object | No | defaults | Perceptual processing config |
| `semantic_config` | object | No | defaults | Semantic processing config |

**Response (Success):**
```json
{
  "doc_id": "doc-001",
  "tenant_id": "tenant-001",
  "status": "success",
  "canonical_hash": "a1b2c3d4e5f6...",
  "perceptual_fingerprint": {
    "minhash": [123456789, 987654321, ...],
    "minhash_bands": 16,
    "minhash_rows_per_band": 8
  },
  "semantic_embedding": {
    "doc_id": "doc-001",
    "vector": [0.123, -0.456, ...],
    "model_name": "all-MiniLM-L6-v2",
    "embedding_dim": 384,
    "tier": "balanced",
    "normalized": true
  }
}
```

**Response (Error):**
```json
{
  "doc_id": "doc-001",
  "tenant_id": "tenant-001",
  "status": "error",
  "canonical_hash": null,
  "perceptual_fingerprint": null,
  "semantic_embedding": null,
  "error": "Error message details"
}
```

**Error Codes:**
- `400 Bad Request` - Invalid JSON or missing required fields
- `401 Unauthorized` - Missing or invalid API key
- `413 Payload Too Large` - Text exceeds max size
- `422 PIPELINE_ERROR` - Processing failed
- `429 Rate Limit Exceeded` - Too many requests

**Example:**
```bash
curl -X POST http://localhost:8080/api/v1/process \
  -H "Content-Type: application/json" \
  -H "X-API-Key: demo-key-12345" \
  -d '{
    "doc_id": "article-001",
    "tenant_id": "news-corp",
    "text": "Breaking news: Major discovery in quantum computing...",
    "enable_perceptual": true,
    "enable_semantic": true
  }'
```

---

### POST /api/v1/batch

Process multiple documents in a single request.

**Request Body:**
```json
{
  "documents": [
    {
      "doc_id": "doc-001",
      "tenant_id": "tenant-001",
      "text": "First document"
    },
    {
      "doc_id": "doc-002",
      "tenant_id": "tenant-001",
      "text": "Second document"
    }
  ],
  "enable_perceptual": true,
  "enable_semantic": true
}
```

**Response:**
```json
{
  "processed": 2,
  "successful": 2,
  "failed": 0,
  "results": [
    {
      "doc_id": "doc-001",
      "tenant_id": "tenant-001",
      "status": "success",
      "canonical_hash": "...",
      "perceptual_fingerprint": {...},
      "semantic_embedding": {...}
    },
    {
      "doc_id": "doc-002",
      "tenant_id": "tenant-001",
      "status": "success",
      "canonical_hash": "...",
      "perceptual_fingerprint": {...},
      "semantic_embedding": {...}
    }
  ]
}
```

**Performance Notes:**
- Documents are processed concurrently (up to 10 at a time) for optimal throughput
- Order of results matches the order of input documents
- Individual document failures don't block the entire batch
- Recommended batch size: 50-100 documents for best performance

**Error Handling:**
- Failed documents return with `"status": "error"` and an `error` field
- Successful documents are still returned even if some fail
- Check `successful` vs `failed` counts in the response

---

### POST /api/v1/index/insert

Insert a processed record into the index for later searching.

**Request Body:**
```json
{
  "doc_id": "doc-001",
  "tenant_id": "tenant-001",
  "canonical_hash": "a1b2c3d4...",
  "perceptual_fingerprint": [12345, 67890, ...],
  "semantic_embedding": [0.1, 0.2, ...],
  "metadata": {
    "category": "news",
    "author": "John Doe"
  }
}
```

**Response:**
```json
{
  "doc_id": "doc-001",
  "status": "inserted"
}
```

---

### GET /api/v1/index/search

Search the index for similar documents.

**Query Parameters:**

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `query` | string | Yes | - | Search query text |
| `strategy` | string | No | `perceptual` | Search strategy: `perceptual`, `semantic`, `hybrid` |
| `top_k` | integer | No | 10 | Number of results to return |
| `tenant_id` | string | No | - | Filter by tenant |

**Response:**
```json
{
  "query": "search text",
  "strategy": "perceptual",
  "total_hits": 5,
  "hits": [
    {
      "doc_id": "doc-001",
      "score": 0.95,
      "tenant_id": "tenant-001",
      "metadata": {
        "category": "news"
      }
    },
    {
      "doc_id": "doc-002",
      "score": 0.87,
      "tenant_id": "tenant-001"
    }
  ]
}
```

---

### GET /api/v1/index/documents

List all documents in the index.

**Response:**
```json
{
  "documents": [
    {
      "doc_id": "doc-001",
      "tenant_id": "tenant-001",
      "canonical_hash": "..."
    }
  ],
  "total": 1
}
```

---

### DELETE /api/v1/index/documents/:doc_id

Delete a document from the index.

**Response:**
```json
{
  "doc_id": "doc-001",
  "status": "deleted"
}
```

---

### POST /api/v1/match

Find documents matching a query using perceptual/semantic similarity.

**Request Body:**
```json
{
  "query": "Query text to match",
  "tenant_id": "tenant-001",
  "strategy": "hybrid",
  "max_results": 10,
  "oversample_factor": 1.5,
  "min_score": 0.8
}
```

**Request Fields:**

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `query` | string | Yes | - | Query text |
| `tenant_id` | string | No | - | Filter by tenant |
| `strategy` | string | No | `hybrid` | `perceptual`, `semantic`, or `hybrid` |
| `max_results` | integer | No | 10 | Maximum results |
| `oversample_factor` | float | No | 1.5 | For semantic search |
| `min_score` | float | No | - | Minimum similarity threshold (0.0-1.0) |

**Search Performance Notes:**
- **Semantic search** uses HNSW ANN (Approximate Nearest Neighbor) for datasets with 1000+ vectors
- ANN provides **100-1000x faster** O(log n) search vs linear O(n) scan
- 95-99% recall rate - may miss ~1-5% of true nearest neighbors
- Falls back to exact linear scan for small datasets (<1000 vectors)

**Response:**
```json
{
  "query": "Query text to match",
  "strategy": "hybrid",
  "total_matches": 3,
  "matches": [
    {
      "doc_id": "doc-001",
      "score": 0.92,
      "rank": 1,
      "tenant_id": "tenant-001"
    },
    {
      "doc_id": "doc-002",
      "score": 0.85,
      "rank": 2,
      "tenant_id": "tenant-001"
    }
  ]
}
```

---

### POST /api/v1/compare

Compare two documents directly for similarity.

**Request Body:**
```json
{
  "doc1": {
    "text": "First document content",
    "doc_id": "doc-a"
  },
  "doc2": {
    "text": "Second document content",
    "doc_id": "doc-b"
  }
}
```

**Response:**
```json
{
  "similarity_score": 0.75,
  "perceptual_similarity": 0.80,
  "semantic_similarity": 0.70
}
```

---

### GET /api/v1/metadata

Get server metadata (version, uptime).

**Response:**
```json
{
  "version": "0.1.0",
  "uptime_seconds": 3600
}
```

---

## Configuration Options

### Perceptual Config

```json
{
  "k": 9,                      // Shingle size (tokens per shingle)
  "w": 4,                      // Window size for winnowing
  "minhash_bands": 16,         // Number of MinHash bands
  "minhash_rows_per_band": 8,  // Rows per band
  "use_parallel": true,        // Use parallel processing
  "seed": 12345                // Random seed for hashing
}
```

### Semantic Config

```json
{
  "tier": "balanced",                // Model tier: fast, balanced, accurate
  "normalize": true,                 // L2 normalize embeddings
  "model_name": "...",               // Specific model (optional)
  "mode": "onnx",                    // Processing mode: onnx, api, fast
  "max_sequence_length": 512,        // Model's token limit (512 for BERT, 4096 for Longformer, etc.)
  "enable_chunking": false,          // Enable sliding-window chunking for long documents
  "chunk_overlap_ratio": 0.5,        // Overlap between chunks (0.0-1.0, default 0.5 = 50%)
  "pooling_strategy": "weighted_mean", // Pooling: mean, weighted_mean, max, first
  
  // Resilience configuration (for API mode)
  "enable_resilience": true,
  "circuit_breaker_failure_threshold": 5,
  "circuit_breaker_reset_timeout_secs": 30,
  "retry_max_retries": 3,
  "retry_base_delay_ms": 100,
  "retry_max_delay_ms": 10000,
  "rate_limit_requests_per_second": 10.0,
  "rate_limit_burst_size": 5
}
```

**Resilience Features:**
- **Circuit Breaker**: Prevents cascade failures when external APIs are down
  - Opens after `failure_threshold` consecutive failures
  - Automatically resets after `reset_timeout_secs` to test if service recovered
  - Returns "Service temporarily unavailable" while open
  
- **Retry with Exponential Backoff**: Automatically retries failed requests
  - Base delay doubles with each attempt (1x, 2x, 4x, ...)
  - Caps at `max_delay_ms`
  - Only retries on retryable errors (timeouts, 5xx, 429)
  
- **Rate Limiting**: Token bucket algorithm per provider
  - `requests_per_second`: Steady-state rate limit
  - `burst_size`: Maximum burst capacity

**Chunking for Long Documents:**

When `enable_chunking` is `true` and text exceeds `max_sequence_length` tokens:
- Text is split into overlapping chunks (controlled by `chunk_overlap_ratio`)
- Each chunk is embedded independently
- Embeddings are pooled using the specified strategy:
  - `mean`: Simple average of all chunks
  - `weighted_mean`: Center-weighted average (default) - center chunks get higher weight
  - `max`: Element-wise maximum across chunks
  - `first`: Use only the first chunk

**Example with Chunking:**

```json
{
  "text": "Very long document content...",
  "enable_semantic": true,
  "semantic_config": {
    "max_sequence_length": 512,
    "enable_chunking": true,
    "chunk_overlap_ratio": 0.5,
    "pooling_strategy": "weighted_mean"
  }
}
```

### Index / ANN Config

```json
{
  "backend": "redb",              // Storage backend: redb, memory
  "redb_path": "./data/index",    // Path for Redb database
  
  // ANN (Approximate Nearest Neighbor) Configuration
  "ann": {
    "enabled": true,              // Enable HNSW ANN search
    "min_vectors_for_ann": 1000,  // Auto-switch to ANN at this threshold
    "m": 16,                      // HNSW neighbors per node (higher = better quality)
    "ef_construction": 200,       // Build quality (higher = better, slower build)
    "ef_search": 50              // Search quality (higher = better, slower search)
  }
}
```

**ANN (HNSW) Search:**
- Automatically enables when index contains `min_vectors_for_ann` or more vectors
- Uses Hierarchical Navigable Small World (HNSW) graphs for sub-linear O(log n) search
- **Performance**: 100-1000x faster than linear scan for large datasets
- **Recall**: 95-99% (may miss 1-5% of true nearest neighbors)
- **Trade-offs**: Higher memory usage, longer build time vs query speed

**Parameter Guidelines:**
- `m`: 16-64 (16 for speed, 64 for quality)
- `ef_construction`: 100-400 (higher = better index quality)
- `ef_search`: 50-200 (higher = better search quality)
- `min_vectors_for_ann`: 500-2000 (threshold to enable ANN vs linear scan)

---

## Error Codes

| Code | HTTP Status | Description |
|------|-------------|-------------|
| `AUTH_FAILED` | 401 | Invalid or missing API key |
| `RATE_LIMIT_EXCEEDED` | 429 | Too many requests (per-minute limit exceeded) |
| `REQUEST_TIMEOUT` | 408 | Request processing timed out |
| `BAD_REQUEST` | 400 | Invalid request format or parameters |
| `PAYLOAD_TOO_LARGE` | 413 | Request body exceeds configured limit |
| `NOT_FOUND` | 404 | Requested resource not found |
| `PIPELINE_ERROR` | 422 | Processing pipeline error |
| `INGEST_ERROR` | 422 | Document ingestion/validation error |
| `CANONICAL_ERROR` | 422 | Canonicalization error |
| `PERCEPTUAL_ERROR` | 422 | Perceptual fingerprinting error |
| `SEMANTIC_ERROR` | 422 | Semantic embedding error |
| `INDEX_ERROR` | 422 | Index operation error |
| `MATCH_ERROR` | 422 | Matching operation error |
| `CONFIG_ERROR` | 500 | Server configuration error |
| `INTERNAL_ERROR` | 500 | Unexpected internal error |

---

## Rate Limiting

- Default: 100 requests per minute per API key
- Rate limit headers (planned):
  - `X-RateLimit-Limit`: Maximum requests allowed
  - `X-RateLimit-Remaining`: Remaining requests in window
  - `X-RateLimit-Reset`: Unix timestamp when limit resets

**Response (429 Too Many Requests):**
```json
{
  "error": {
    "code": "RATE_LIMIT_EXCEEDED",
    "message": "Rate limit exceeded"
  }
}
```

---

## Request/Response Headers

### Request Headers
- `Content-Type: application/json` - Required for POST requests
- `X-API-Key: <key>` or `Authorization: Bearer <key>` - Required for protected endpoints
- `X-Request-ID: <uuid>` - Optional, for request tracing

### Response Headers
- `Content-Type: application/json` - Always JSON
- `X-Request-ID: <uuid>` - Request tracking ID
- `Content-Encoding: gzip` - If compression enabled

---

## Client Examples

### Python
```python
import requests

API_KEY = "demo-key-12345"
BASE_URL = "http://localhost:8080"

headers = {
    "X-API-Key": API_KEY,
    "Content-Type": "application/json"
}

# Process document
response = requests.post(
    f"{BASE_URL}/api/v1/process",
    headers=headers,
    json={
        "doc_id": "doc-001",
        "text": "Sample document",
        "enable_perceptual": True,
        "enable_semantic": True
    }
)
result = response.json()
print(result)
```

### JavaScript/Node.js
```javascript
const axios = require('axios');

const API_KEY = 'demo-key-12345';
const BASE_URL = 'http://localhost:8080';

const client = axios.create({
  baseURL: BASE_URL,
  headers: {
    'X-API-Key': API_KEY,
    'Content-Type': 'application/json'
  }
});

// Process document
async function processDocument() {
  const response = await client.post('/api/v1/process', {
    doc_id: 'doc-001',
    text: 'Sample document',
    enable_perceptual: true,
    enable_semantic: true
  });
  console.log(response.data);
}
```

### curl
```bash
# Process document
curl -X POST http://localhost:8080/api/v1/process \
  -H "Content-Type: application/json" \
  -H "X-API-Key: demo-key-12345" \
  -d '{"doc_id":"doc-001","text":"Sample"}'

# Health check
curl http://localhost:8080/health

# Batch process
curl -X POST http://localhost:8080/api/v1/batch \
  -H "Content-Type: application/json" \
  -H "X-API-Key: demo-key-12345" \
  -d '{"documents":[{"doc_id":"1","text":"Doc 1"},{"doc_id":"2","text":"Doc 2"}]}'
```
