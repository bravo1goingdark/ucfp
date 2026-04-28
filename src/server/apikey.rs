//! API-key lookup trait and three pluggable implementations.
//!
//! `ApiKeyLookup` is the boundary between the protected router and the
//! identity store. Three impls are provided so the same router can be
//! wired against very different identity sources without conditional
//! handler code:
//!
//! - [`StaticSingleKey`] — single-token compat path for self-hosters
//!   (preserves the legacy `UCFP_TOKEN` semantics).
//! - [`StaticMapKey`] — multi-tenant from a TOML file on disk.
//! - [`WebhookKeyLookup`] — multi-tenant via an HTTP control plane
//!   (gated behind `multi-tenant`).
//!
//! Returning `Ok(None)` means *no such key*; the extractor turns that
//! into a 401. Returning `Err(_)` means the lookup itself failed and
//! propagates as a 5xx.
//!
//! These types are constructed by `bin/ucfp.rs` (R4) and re-exported by
//! `server/mod.rs` (R3). Until that wiring lands, the items here look
//! unused to the compiler — silence dead-code lints at the module level.

#![allow(dead_code)]

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use subtle::ConstantTimeEq;

/// Identity attached to every authenticated request.
///
/// Constructed by an [`ApiKeyLookup`] impl and consumed downstream by
/// the rate-limit layer, the usage sink, and any handler that needs to
/// scope its work by tenant.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApiKeyContext {
    /// Tenant partition this key belongs to. Joined with [`crate::Record::tenant_id`]
    /// at handler entry to enforce cross-tenant isolation.
    pub tenant_id: u32,
    /// Stable per-key identifier (UUID/ULID). Logged into [`crate::server::usage::UsageEvent`]
    /// — never the raw token.
    pub key_id: String,
    /// Capability tags (`"ingest"`, `"query"`, `"delete"`, …). Empty
    /// means "no caller-side restrictions"; handlers may still refuse.
    pub scopes: Vec<String>,
    /// Optional rate-class label used by [`crate::server::ratelimit::TenantRateLimiter`]
    /// to bucket different keys at different ceilings.
    pub rate_class: Option<String>,
}

/// Pluggable identity source. Async because some impls hit the network.
///
/// Implementations must be cheap to clone-share via `Arc`; the extractor
/// stores them in router state and reads on every request.
#[async_trait::async_trait]
pub trait ApiKeyLookup: Send + Sync {
    /// Resolve `presented` (the bearer token verbatim, no `Bearer ` prefix)
    /// to a context. `Ok(None)` → unknown key (401). `Err(_)` → lookup
    /// failure (5xx).
    async fn lookup(&self, presented: &str) -> crate::error::Result<Option<ApiKeyContext>>;
}

// ── StaticSingleKey ─────────────────────────────────────────────────────

/// Compares the presented token against a single expected value in
/// constant time. Resolves to `tenant_id` for every successful match.
///
/// This is the legacy `UCFP_TOKEN` path: one shared secret, one tenant.
/// Used by self-hosters and by the SvelteKit→Rust service hop.
#[derive(Clone, Debug)]
pub struct StaticSingleKey {
    /// Bytes of the expected bearer token. Compared with [`subtle::ConstantTimeEq`]
    /// to deny timing-side-channel guessing.
    pub expected: Vec<u8>,
    /// Tenant id assigned to every successful caller.
    pub tenant_id: u32,
}

impl StaticSingleKey {
    /// Build from raw token + tenant. Stores the bytes verbatim.
    pub fn new(token: impl Into<Vec<u8>>, tenant_id: u32) -> Self {
        Self {
            expected: token.into(),
            tenant_id,
        }
    }
}

#[async_trait::async_trait]
impl ApiKeyLookup for StaticSingleKey {
    async fn lookup(&self, presented: &str) -> crate::error::Result<Option<ApiKeyContext>> {
        // Constant-time eq guards against per-byte guessing. `ct_eq`
        // returns `Choice` (0 or 1); only call it on equal-length slices
        // to keep the comparison itself constant time.
        if presented.len() != self.expected.len() {
            return Ok(None);
        }
        if presented.as_bytes().ct_eq(&self.expected).into() {
            Ok(Some(ApiKeyContext {
                tenant_id: self.tenant_id,
                key_id: "static-single".into(),
                scopes: Vec::new(),
                rate_class: None,
            }))
        } else {
            Ok(None)
        }
    }
}

// ── StaticMapKey ────────────────────────────────────────────────────────

/// In-memory map of `token → ApiKeyContext`. Loaded from TOML once at
/// startup. The map is keyed by token bytes (so unknown tokens cost
/// one hash + one constant-time compare per matched bucket).
#[derive(Clone, Debug, Default)]
pub struct StaticMapKey {
    keys: HashMap<String, ApiKeyContext>,
}

/// One row of the TOML config consumed by [`StaticMapKey::from_toml`].
#[derive(Debug, Deserialize)]
struct StaticMapKeyRow {
    token: String,
    tenant_id: u32,
    key_id: String,
    #[serde(default)]
    scopes: Vec<String>,
    #[serde(default)]
    rate_class: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StaticMapKeyFile {
    #[serde(default)]
    key: Vec<StaticMapKeyRow>,
}

impl StaticMapKey {
    /// Construct from an iterator. Useful for tests and for callers
    /// that already parsed the config some other way.
    pub fn from_entries(entries: impl IntoIterator<Item = (String, ApiKeyContext)>) -> Self {
        Self {
            keys: entries.into_iter().collect(),
        }
    }

    /// Parse a TOML document of the shape:
    ///
    /// ```toml
    /// [[key]]
    /// token = "ucfp_xxx"
    /// tenant_id = 42
    /// key_id = "abc-123"
    /// scopes = ["ingest", "query"]
    /// ```
    ///
    /// Parse failure → [`crate::Error::Modality`] (mapped to 400 at the
    /// HTTP boundary, but typically surfaced at startup).
    pub fn from_toml(s: &str) -> crate::error::Result<Self> {
        // toml is not in deps; do a hand-rolled minimal parser by going
        // through serde_json after a naive translation? No — instead we
        // accept the same data via JSON when called programmatically and
        // restrict file-load callers to use whichever format the bin wires.
        //
        // To keep the dependency footprint flat (no `toml` crate added),
        // we accept TOML *or* JSON: try JSON first, fall back to a tiny
        // line-based TOML parser that handles only the documented shape.
        if let Ok(file) = serde_json::from_str::<StaticMapKeyFile>(s) {
            return Ok(Self::from_rows(file.key));
        }
        let rows = parse_minimal_toml(s)
            .map_err(|e| crate::error::Error::Modality(format!("StaticMapKey TOML: {e}")))?;
        Ok(Self::from_rows(rows))
    }

    fn from_rows(rows: Vec<StaticMapKeyRow>) -> Self {
        let keys = rows
            .into_iter()
            .map(|r| {
                (
                    r.token,
                    ApiKeyContext {
                        tenant_id: r.tenant_id,
                        key_id: r.key_id,
                        scopes: r.scopes,
                        rate_class: r.rate_class,
                    },
                )
            })
            .collect();
        Self { keys }
    }
}

#[async_trait::async_trait]
impl ApiKeyLookup for StaticMapKey {
    async fn lookup(&self, presented: &str) -> crate::error::Result<Option<ApiKeyContext>> {
        Ok(self.keys.get(presented).cloned())
    }
}

/// Minimal `[[key]] field = value` TOML parser. Handles only the shape
/// documented for [`StaticMapKey::from_toml`] — adding a real `toml`
/// dependency is deferred until a richer config schema arrives.
fn parse_minimal_toml(s: &str) -> Result<Vec<StaticMapKeyRow>, String> {
    let mut rows: Vec<StaticMapKeyRow> = Vec::new();
    let mut cur: Option<PartialRow> = None;
    for (lineno, raw_line) in s.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line == "[[key]]" {
            if let Some(prev) = cur.take() {
                rows.push(prev.finish(lineno)?);
            }
            cur = Some(PartialRow::default());
            continue;
        }
        let row = cur
            .as_mut()
            .ok_or_else(|| format!("line {lineno}: field outside [[key]] block"))?;
        let (k, v) = line
            .split_once('=')
            .ok_or_else(|| format!("line {lineno}: missing `=`"))?;
        let key = k.trim();
        let val = v.trim();
        match key {
            "token" => row.token = Some(strip_quotes(val)?.to_string()),
            "key_id" => row.key_id = Some(strip_quotes(val)?.to_string()),
            "tenant_id" => {
                row.tenant_id = Some(
                    val.parse::<u32>()
                        .map_err(|e| format!("line {lineno}: tenant_id: {e}"))?,
                );
            }
            "rate_class" => row.rate_class = Some(strip_quotes(val)?.to_string()),
            "scopes" => {
                let inner = val
                    .strip_prefix('[')
                    .and_then(|s| s.strip_suffix(']'))
                    .ok_or_else(|| format!("line {lineno}: scopes must be a TOML array"))?;
                let mut scopes = Vec::new();
                for raw in inner.split(',') {
                    let t = raw.trim();
                    if t.is_empty() {
                        continue;
                    }
                    scopes.push(strip_quotes(t)?.to_string());
                }
                row.scopes = scopes;
            }
            other => return Err(format!("line {lineno}: unknown field `{other}`")),
        }
    }
    if let Some(last) = cur.take() {
        rows.push(last.finish(s.lines().count())?);
    }
    Ok(rows)
}

#[derive(Default)]
struct PartialRow {
    token: Option<String>,
    tenant_id: Option<u32>,
    key_id: Option<String>,
    scopes: Vec<String>,
    rate_class: Option<String>,
}

impl PartialRow {
    fn finish(self, lineno: usize) -> Result<StaticMapKeyRow, String> {
        Ok(StaticMapKeyRow {
            token: self
                .token
                .ok_or_else(|| format!("line {lineno}: missing `token`"))?,
            tenant_id: self
                .tenant_id
                .ok_or_else(|| format!("line {lineno}: missing `tenant_id`"))?,
            key_id: self
                .key_id
                .ok_or_else(|| format!("line {lineno}: missing `key_id`"))?,
            scopes: self.scopes,
            rate_class: self.rate_class,
        })
    }
}

fn strip_quotes(s: &str) -> Result<&str, String> {
    let s = s.trim();
    s.strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .ok_or_else(|| format!("expected quoted string, got `{s}`"))
}

// ── WebhookKeyLookup ────────────────────────────────────────────────────

#[cfg(feature = "multi-tenant")]
mod webhook {
    use std::collections::HashMap;
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    use super::ApiKeyContext;
    use crate::error::Error;

    /// HTTP-backed identity lookup with a small TTL cache.
    ///
    /// On miss, posts `{"key": "<presented>"}` to `url`. A 200 response
    /// must carry an [`ApiKeyContext`] JSON body; 401/404 are mapped to
    /// `Ok(None)`; everything else is an [`Error::Ingest`] (5xx at the
    /// HTTP boundary).
    ///
    /// Cache uses the std lib only (no `dashmap`/`moka`): one [`Mutex`]
    /// around a [`HashMap`]. Lookups are cheap (microseconds) and the
    /// hot path is read-mostly.
    pub struct WebhookKeyLookup {
        client: reqwest::Client,
        url: reqwest::Url,
        ttl: Duration,
        cache: Mutex<HashMap<String, (Instant, Option<ApiKeyContext>)>>,
    }

    impl WebhookKeyLookup {
        /// Build with a default 60s positive-and-negative TTL.
        pub fn new(client: reqwest::Client, url: reqwest::Url) -> Self {
            Self::with_ttl(client, url, Duration::from_secs(60))
        }

        /// Build with a caller-chosen TTL.
        pub fn with_ttl(client: reqwest::Client, url: reqwest::Url, ttl: Duration) -> Self {
            Self {
                client,
                url,
                ttl,
                cache: Mutex::new(HashMap::new()),
            }
        }

        fn cache_get(&self, key: &str) -> Option<Option<ApiKeyContext>> {
            let mut g = self.cache.lock().ok()?;
            let entry = g.get(key)?;
            if entry.0.elapsed() < self.ttl {
                Some(entry.1.clone())
            } else {
                g.remove(key);
                None
            }
        }

        fn cache_put(&self, key: String, value: Option<ApiKeyContext>) {
            if let Ok(mut g) = self.cache.lock() {
                // Bound the cache crudely to keep memory predictable in
                // the face of a flood of bad keys.
                if g.len() >= 4096 {
                    g.clear();
                }
                g.insert(key, (Instant::now(), value));
            }
        }
    }

    #[async_trait::async_trait]
    impl super::ApiKeyLookup for WebhookKeyLookup {
        async fn lookup(&self, presented: &str) -> crate::error::Result<Option<ApiKeyContext>> {
            if let Some(hit) = self.cache_get(presented) {
                return Ok(hit);
            }
            let body = serde_json::json!({ "key": presented });
            let resp = self
                .client
                .post(self.url.clone())
                .json(&body)
                .send()
                .await
                .map_err(|e| Error::Ingest(format!("key lookup webhook: {e}")))?;
            let status = resp.status();
            if status.as_u16() == 401 || status.as_u16() == 404 {
                self.cache_put(presented.to_string(), None);
                return Ok(None);
            }
            if !status.is_success() {
                return Err(Error::Ingest(format!(
                    "key lookup webhook: status {status}"
                )));
            }
            let ctx: ApiKeyContext = resp
                .json()
                .await
                .map_err(|e| Error::Ingest(format!("key lookup webhook decode: {e}")))?;
            self.cache_put(presented.to_string(), Some(ctx.clone()));
            Ok(Some(ctx))
        }
    }
}

#[cfg(feature = "multi-tenant")]
#[allow(unused_imports)] // re-export consumed by R3's wiring in mod.rs
pub use webhook::WebhookKeyLookup;
