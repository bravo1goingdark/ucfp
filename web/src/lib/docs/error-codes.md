---
title: Error codes
order: 6
description: Every HTTP status UCFP returns, what triggers it, and what to do.
---

# Error codes

UCFP returns a standard HTTP status plus a JSON body of the form:

```json
{
  "error": {
    "code": "rate_limited",
    "message": "Per-key minute budget exhausted",
    "retry_after_ms": 23000
  }
}
```

`code` is stable; `message` is human-readable and may change between versions.

## 400 Bad Request

The body or query string is malformed. Common causes:

- Invalid JSON.
- `?algorithm=` references a value the modality doesn't support.
- `h` is not one of `64 | 128 | 256` for MinHash.
- Required `sample_rate` missing on an audio call.

**What to do:** validate locally before sending. The error message names the offending field.

## 401 Unauthorized

The bearer token is missing, malformed, or revoked.

**What to do:**

- Confirm `Authorization: Bearer ucfp_…` (or `X-Api-Key: ucfp_…`) is set.
- Confirm the key is not revoked in `/dashboard/keys`.
- If you just rotated, double-check the new prefix.

## 403 Forbidden

The token authenticates but is not allowed to perform this action. Today the only trigger is **scope mismatch** — a key with `read` scope hitting an `ingest` route.

**What to do:** create a key with the correct scopes from the dashboard.

## 404 Not Found

Either the route doesn't exist, or `GET /v1/records/{tid}/{rid}` was hit for a record that was never ingested or was deleted.

**What to do:** verify the record ID. The dashboard lists every record per tenant.

## 415 Unsupported Media Type

`Content-Type` doesn't match a modality and there is no `?modality=` override.

**What to do:** set the right `Content-Type` (e.g. `image/jpeg`, `audio/wav`) or send `multipart/form-data` with an explicit `modality` field.

## 422 Unprocessable Entity

The request is well-formed and authorized, but the input fails a domain rule:

- TLSH input shorter than 50 bytes.
- Image with shortest edge below `min_dimension`.
- Audio sample rate not in the supported set for the chosen algorithm.
- `model_id` references a model that is not loaded.

**What to do:** the message names the rule. Adjust input or parameters and retry.

## 429 Too Many Requests

You hit either the per-minute budget or the daily quota. Headers:

- `Retry-After: <seconds>` — wall-clock seconds until the soonest retry.
- `X-RateLimit-Limit: <n>` — the bucket size you hit.
- `X-RateLimit-Remaining: 0`
- `X-RateLimit-Reset: <unix-epoch-seconds>`

**What to do:** back off until `Retry-After` elapses. For sustained traffic, request a quota bump from the dashboard, or shard across multiple keys.

See [Rate limits](/docs/rate-limits) for the full budget table.

## 500 Internal Server Error

Unhandled error in the upstream Rust process. These are bugs.

**What to do:** retry once with exponential backoff. If it persists, check `/status` for an outage banner. If green, file an issue with the `record_id` from the response (if present) — it lets us trace the call.

## 501 Not Implemented

The route exists but the requested feature isn't compiled in. Common in self-hosted setups: hitting `?algorithm=tlsh` on a binary built without `--features text-tlsh`.

**What to do:**

- Self-host: rebuild with the right features (`cargo install ucfp --features full`).
- Hosted: file an issue — every algorithm in the docs is enabled in production.

## 503 Service Unavailable

The Rust upstream is unreachable from the Worker, the demo proxy is unconfigured, or a dependency model is still loading at startup.

**What to do:** check `/status`. Demo callers should fall back to the local FNV-1a path (the bundled UI does this automatically and tags the result `FALLBACK · LOCAL FNV-1a`).

## Error code reference

| Status | `error.code` | Meaning |
| --- | --- | --- |
| 400 | `bad_request` | Malformed input |
| 401 | `unauthenticated` | Missing/invalid key |
| 403 | `forbidden` | Scope mismatch |
| 404 | `not_found` | Record or route absent |
| 415 | `unsupported_media_type` | Wrong `Content-Type` |
| 422 | `validation_failed` | Domain rule rejected input |
| 429 | `rate_limited` | Quota exhausted |
| 500 | `internal_error` | Bug |
| 501 | `unsupported` | Feature not compiled in |
| 503 | `unavailable` | Upstream unreachable |
