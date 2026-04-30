//! Per-tenant rate limiting.
//!
//! `TenantRateLimiter` is the boundary the request layer crosses before
//! a handler runs. Three impls are provided so the same router can be
//! wired against very different ceilings:
//!
//! - [`NoopRateLimiter`] — always allows; for tests and self-hosters
//!   running behind another rate-limit layer.
//! - [`InMemoryTokenBucket`] — process-local leaky bucket; one instance
//!   per tenant, refilled at a fixed rps.
//! - `WebhookRateLimiter` — defers the decision to a control plane
//!   (gated behind `multi-tenant`).
//!
//! The decision shape is rich enough to drive `Retry-After` headers and
//! `X-RateLimit-Remaining` without leaking the limiter implementation.
//!
//! These types are wired into the request layer by `bin/ucfp.rs` (R4)
//! and re-exported by `server/mod.rs` (R3). Until then dead-code lints
//! are silenced at the module level.

#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::RwLock;
use std::time::Instant;

use super::apikey::ApiKeyContext;

/// Decision returned from [`TenantRateLimiter::check`].
#[derive(Clone, Copy, Debug)]
pub enum RateDecision {
    /// Allowed. `remaining` is the post-charge token count; `reset_ms`
    /// is the time until the bucket is fully refilled.
    Allow {
        /// Tokens left after deducting `cost`.
        remaining: u64,
        /// Milliseconds until the bucket is fully refilled.
        reset_ms: u64,
    },
    /// Denied. Caller should send 429 with `Retry-After: retry_after_ms / 1000`
    /// (rounded up to keep clients from re-trying inside the window).
    Deny {
        /// Milliseconds the caller should wait before retrying.
        retry_after_ms: u64,
    },
}

/// Pluggable per-tenant rate limiter. Async because some impls hit the
/// network.
#[async_trait::async_trait]
pub trait TenantRateLimiter: Send + Sync {
    /// Charge `cost` tokens against `ctx`'s tenant bucket. Returns a
    /// decision; never panics on overflow (cost is clamped at the
    /// bucket capacity).
    async fn check(&self, ctx: &ApiKeyContext, cost: u32) -> crate::error::Result<RateDecision>;
}

// ── NoopRateLimiter ─────────────────────────────────────────────────────

/// Always allows. `remaining` is reported as `u64::MAX` so callers that
/// echo the value into a header still produce a meaningful response.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoopRateLimiter;

#[async_trait::async_trait]
impl TenantRateLimiter for NoopRateLimiter {
    async fn check(&self, _ctx: &ApiKeyContext, _cost: u32) -> crate::error::Result<RateDecision> {
        Ok(RateDecision::Allow {
            remaining: u64::MAX,
            reset_ms: 0,
        })
    }
}

// ── InMemoryTokenBucket ─────────────────────────────────────────────────

/// One leaky bucket per tenant. Tokens are stored as `f64` so partial
/// refills accumulate without integer truncation.
#[derive(Debug)]
struct Bucket {
    tokens: f64,
    last_refill: Instant,
}

/// In-process token bucket per tenant.
///
/// Constructed with a refill rate (tokens/second) and a burst capacity.
/// Defaults match the plan: 100 rps × 200 burst.
#[derive(Debug)]
pub struct InMemoryTokenBucket {
    rps: f64,
    burst: f64,
    buckets: RwLock<HashMap<u32, std::sync::Mutex<Bucket>>>,
}

impl InMemoryTokenBucket {
    /// Construct with defaults (100 rps, 200 burst).
    pub fn new() -> Self {
        Self::with_limits(100, 200)
    }

    /// Construct with custom `rps` (tokens/second refill) and `burst`
    /// (bucket capacity).
    pub fn with_limits(rps: u32, burst: u32) -> Self {
        Self {
            rps: f64::from(rps),
            burst: f64::from(burst),
            buckets: RwLock::new(HashMap::new()),
        }
    }

    fn refill(&self, b: &mut Bucket, now: Instant) {
        let elapsed = now.saturating_duration_since(b.last_refill).as_secs_f64();
        b.tokens = (b.tokens + elapsed * self.rps).min(self.burst);
        b.last_refill = now;
    }
}

impl Default for InMemoryTokenBucket {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl TenantRateLimiter for InMemoryTokenBucket {
    async fn check(&self, ctx: &ApiKeyContext, cost: u32) -> crate::error::Result<RateDecision> {
        let cost_f = f64::from(cost).min(self.burst);
        let now = Instant::now();

        // Fast path: bucket already exists. Take a read lock, then a
        // per-bucket Mutex — no global write contention on the hot path.
        {
            let g = self.buckets.read().map_err(poisoned)?;
            if let Some(slot) = g.get(&ctx.tenant_id) {
                let mut b = slot.lock().map_err(poisoned)?;
                self.refill(&mut b, now);
                return Ok(decide(&mut b, cost_f, self.rps, self.burst));
            }
        }

        // Slow path: insert a fresh bucket. Re-check under write lock to
        // avoid double-insert on races.
        let mut g = self.buckets.write().map_err(poisoned)?;
        let slot = g.entry(ctx.tenant_id).or_insert_with(|| {
            std::sync::Mutex::new(Bucket {
                tokens: self.burst,
                last_refill: now,
            })
        });
        // We hold the unique RwLock write guard, so no other thread can
        // observe `slot`; `get_mut` skips the inner Mutex syscall.
        let b = slot.get_mut().map_err(poisoned)?;
        self.refill(b, now);
        Ok(decide(b, cost_f, self.rps, self.burst))
    }
}

fn decide(b: &mut Bucket, cost: f64, rps: f64, burst: f64) -> RateDecision {
    if b.tokens >= cost {
        b.tokens -= cost;
        let reset_ms = if rps > 0.0 {
            ((burst - b.tokens) / rps * 1000.0) as u64
        } else {
            0
        };
        RateDecision::Allow {
            remaining: b.tokens as u64,
            reset_ms,
        }
    } else {
        let need = cost - b.tokens;
        let retry_after_ms = if rps > 0.0 {
            (need / rps * 1000.0).ceil() as u64
        } else {
            u64::MAX
        };
        RateDecision::Deny { retry_after_ms }
    }
}

fn poisoned<T>(_e: std::sync::PoisonError<T>) -> crate::error::Error {
    crate::error::Error::Index("rate-limit lock poisoned".into())
}

// ── WebhookRateLimiter ──────────────────────────────────────────────────

#[cfg(feature = "multi-tenant")]
mod webhook {
    use super::{ApiKeyContext, RateDecision};
    use crate::error::Error;

    /// HTTP-backed rate limiter. Posts the decision request to a control
    /// plane and parses the canonical [`RateDecision`] from the body.
    pub struct WebhookRateLimiter {
        client: reqwest::Client,
        url: reqwest::Url,
    }

    impl WebhookRateLimiter {
        /// Construct with a shared client and the decision endpoint.
        pub fn new(client: reqwest::Client, url: reqwest::Url) -> Self {
            Self { client, url }
        }
    }

    #[derive(serde::Deserialize)]
    #[serde(tag = "decision", rename_all = "snake_case")]
    enum WireDecision {
        Allow { remaining: u64, reset_ms: u64 },
        Deny { retry_after_ms: u64 },
    }

    #[async_trait::async_trait]
    impl super::TenantRateLimiter for WebhookRateLimiter {
        async fn check(
            &self,
            ctx: &ApiKeyContext,
            cost: u32,
        ) -> crate::error::Result<RateDecision> {
            let body = serde_json::json!({
                "tenant_id": ctx.tenant_id,
                "key_id": ctx.key_id,
                "rate_class": ctx.rate_class,
                "cost": cost,
            });
            let resp = self
                .client
                .post(self.url.clone())
                .json(&body)
                .send()
                .await
                .map_err(|e| Error::Ingest(format!("ratelimit webhook: {e}")))?;
            if !resp.status().is_success() {
                return Err(Error::Ingest(format!(
                    "ratelimit webhook: status {}",
                    resp.status()
                )));
            }
            let wire: WireDecision = resp
                .json()
                .await
                .map_err(|e| Error::Ingest(format!("ratelimit webhook decode: {e}")))?;
            Ok(match wire {
                WireDecision::Allow {
                    remaining,
                    reset_ms,
                } => RateDecision::Allow {
                    remaining,
                    reset_ms,
                },
                WireDecision::Deny { retry_after_ms } => RateDecision::Deny { retry_after_ms },
            })
        }
    }
}

#[cfg(feature = "multi-tenant")]
#[allow(unused_imports)] // re-export consumed by R3's wiring in mod.rs
pub use webhook::WebhookRateLimiter;
