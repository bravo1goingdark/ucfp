//! In-memory session-cached input store powering the playground's
//! live-tune flow.
//!
//! Frontends upload a payload once via `POST /v1/inputs`, get back an
//! `input_id`, then can re-fingerprint with different opts repeatedly
//! by passing `?input_id=…` on `POST /v1/ingest/...` instead of the
//! payload. This keeps slider movements on a 10 MiB image cheap.
//!
//! The cache is process-local, single-tenant-scoped (entries are keyed
//! by `(tenant_id, input_id)`), bounded by both a TTL and a per-tenant
//! byte budget, and not persisted. It is intentionally feature-gated so
//! production deploys that don't run the playground can leave it off.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use bytes::Bytes;

use crate::core::Modality;

/// One stored input. Cheap to clone (Bytes is ref-counted).
///
/// `modality` and `content_type` are recorded for the upcoming
/// pipeline-inspect endpoint (D3.1) and surfaced in the put response;
/// the live-tune ingest handlers only consume `bytes` + `sample_rate`
/// today. Allowed dead until D3.1 lands.
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct CachedInput {
    pub modality: Modality,
    pub bytes: Bytes,
    pub content_type: String,
    pub sample_rate: Option<u32>,
    inserted_at: Instant,
}

impl CachedInput {
    /// `true` when this entry is older than `ttl`.
    pub fn is_expired(&self, ttl: Duration) -> bool {
        self.inserted_at.elapsed() > ttl
    }
}

/// Cache configuration. All values are static for this revision; expose
/// via env vars later if a deployment needs different limits.
#[derive(Copy, Clone, Debug)]
pub struct CacheConfig {
    /// Per-entry expiry; entries past this are evicted lazily on
    /// insert/lookup and ignored when read.
    pub ttl: Duration,
    /// Soft per-tenant byte cap. New inserts evict the oldest entries
    /// of the same tenant until under this cap.
    pub per_tenant_bytes: usize,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            ttl: Duration::from_secs(600),
            per_tenant_bytes: 200 * 1024 * 1024,
        }
    }
}

/// Process-local input cache.
pub struct InputsCache {
    cfg: CacheConfig,
    inner: Mutex<HashMap<(u32, u64), CachedInput>>,
    next_id: std::sync::atomic::AtomicU64,
}

impl InputsCache {
    pub fn new(cfg: CacheConfig) -> Self {
        // Seed the counter with the current nanos so IDs are unique
        // across server restarts within the same wall-clock second.
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(1);
        Self {
            cfg,
            inner: Mutex::new(HashMap::new()),
            next_id: std::sync::atomic::AtomicU64::new(seed | 1), // odd → never zero
        }
    }

    /// Mint a fresh input ID (monotonic per-process).
    fn mint_id(&self) -> u64 {
        self.next_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }

    /// Insert a new entry. Returns the assigned input_id. Performs a
    /// lazy expiry sweep + budget eviction on the requesting tenant.
    pub fn put(&self, tenant_id: u32, input: CachedInput) -> u64 {
        let id = self.mint_id();
        let mut map = self.inner.lock().expect("inputs cache mutex poisoned");
        // Lazy global expiry sweep — bounded by `inner.len()`, runs at
        // insert frequency only. Acceptable for a few hundred entries.
        map.retain(|_, v| !v.is_expired(self.cfg.ttl));
        // Per-tenant budget enforcement.
        let mut tenant_total: usize = map
            .iter()
            .filter(|((t, _), _)| *t == tenant_id)
            .map(|(_, v)| v.bytes.len())
            .sum();
        let incoming = input.bytes.len();
        if incoming > self.cfg.per_tenant_bytes {
            // Reject inserts larger than the entire tenant budget; the
            // handler maps this to 413 / clears the cache for that tenant.
            // For now just don't insert and return id — the handler can
            // detect zero-byte cache state and re-fail next request.
            return id;
        }
        while tenant_total + incoming > self.cfg.per_tenant_bytes {
            // Pop the oldest entry of this tenant.
            let oldest = map
                .iter()
                .filter(|((t, _), _)| *t == tenant_id)
                .min_by_key(|(_, v)| v.inserted_at)
                .map(|(k, v)| (*k, v.bytes.len()));
            match oldest {
                Some((k, n)) => {
                    map.remove(&k);
                    tenant_total = tenant_total.saturating_sub(n);
                }
                None => break,
            }
        }
        map.insert((tenant_id, id), input);
        id
    }

    /// Fetch an entry, returning a clone (cheap — Bytes is shared).
    /// Returns `None` when missing or expired.
    pub fn get(&self, tenant_id: u32, input_id: u64) -> Option<CachedInput> {
        let map = self.inner.lock().expect("inputs cache mutex poisoned");
        match map.get(&(tenant_id, input_id)) {
            Some(v) if !v.is_expired(self.cfg.ttl) => Some(v.clone()),
            _ => None,
        }
    }

    /// Explicit removal. Returns true if an entry was dropped.
    pub fn remove(&self, tenant_id: u32, input_id: u64) -> bool {
        let mut map = self.inner.lock().expect("inputs cache mutex poisoned");
        map.remove(&(tenant_id, input_id)).is_some()
    }

    /// Insert raw bytes with the supplied modality + headers and return
    /// the assigned input_id along with the inserted entry's size.
    pub fn put_bytes(
        &self,
        tenant_id: u32,
        modality: Modality,
        bytes: Bytes,
        content_type: String,
        sample_rate: Option<u32>,
    ) -> (u64, usize) {
        let size = bytes.len();
        let id = self.put(
            tenant_id,
            CachedInput {
                modality,
                bytes,
                content_type,
                sample_rate,
                inserted_at: Instant::now(),
            },
        );
        (id, size)
    }
}

/// Process-wide cache instance. Lazily initialised on first access so
/// builds without the `inspect` feature pay nothing.
static CACHE: OnceLock<InputsCache> = OnceLock::new();

/// Borrow the process-wide cache, initialising on first call.
pub fn cache() -> &'static InputsCache {
    CACHE.get_or_init(|| InputsCache::new(CacheConfig::default()))
}
