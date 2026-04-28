//! Per-request usage events and pluggable sinks.
//!
//! `UsageSink` is the boundary between request processing and the
//! billing/observability fanout. Three impls are provided:
//!
//! - [`NoopUsageSink`] — drops every event; for tests.
//! - [`LogUsageSink`] — appends NDJSON to a local file; for self-hosters.
//! - [`WebhookUsageSink`] — batches events and POSTs them to a control
//!   plane (gated behind `multi-tenant`).
//!
//! Sinks must never block the request path. The webhook sink owns a
//! tokio task; `record` enqueues into an unbounded channel and returns
//! immediately. The log sink takes a tokio mutex but the write itself
//! is small enough that contention never dominates.
//!
//! Constructed by `bin/ucfp.rs` (R4); re-exported by `server/mod.rs`
//! (R3). Until then dead-code lints are silenced at the module level.

#![allow(dead_code)]

use std::path::Path;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use crate::core::Modality;

/// Operation that produced a [`UsageEvent`]. Matches the route table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsageOp {
    /// Modality-specific ingest endpoint (`POST /v1/ingest/...`).
    Ingest,
    /// Generic record upsert (`POST /v1/records`).
    Upsert,
    /// Search / similarity query (`POST /v1/query`).
    Query,
    /// Metadata describe (`GET /v1/records/...`).
    Describe,
    /// Record deletion (`DELETE /v1/records/...`).
    Delete,
}

/// One usage line. Constructed at the response boundary by the usage
/// middleware and handed to a [`UsageSink`].
///
/// The wire format serialises `ts` as unix milliseconds. Sinks that
/// pass the struct through `serde_json` get that conversion for free.
#[derive(Clone, Debug, Serialize)]
pub struct UsageEvent {
    /// Tenant the request was attributed to.
    pub tenant_id: u32,
    /// API key identifier (never the raw token).
    pub key_id: String,
    /// Which operation produced this event.
    pub op: UsageOp,
    /// Modality the request touched, when applicable.
    pub modality: Option<Modality>,
    /// Algorithm tag the request selected, when applicable.
    pub algorithm: Option<String>,
    /// Bytes of request body consumed.
    pub bytes_in: u64,
    /// Per-op work units (e.g. hashes produced, vectors searched). 1
    /// when the op has no natural unit.
    pub units: u64,
    /// Server-side latency in milliseconds.
    pub elapsed_ms: u64,
    /// HTTP status code returned to the client.
    pub status: u16,
    /// Wall-clock timestamp at event finalisation.
    #[serde(serialize_with = "serialize_systime_ms")]
    pub ts: SystemTime,
}

fn serialize_systime_ms<S: serde::Serializer>(ts: &SystemTime, s: S) -> Result<S::Ok, S::Error> {
    let ms = ts
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    s.serialize_u64(ms)
}

/// Pluggable usage event sink. Async because some impls do I/O.
///
/// Implementations must not block the request path; they may queue and
/// return immediately. Callers `await` the future but should not
/// `?`-propagate errors — sinks fail silently to keep the response
/// path latency-bounded.
#[async_trait::async_trait]
pub trait UsageSink: Send + Sync {
    /// Record one event. Implementations must be tolerant of bursts:
    /// a slow sink should drop or batch rather than block.
    async fn record(&self, ev: &UsageEvent);
}

// ── NoopUsageSink ───────────────────────────────────────────────────────

/// Drops every event. For tests and for builds that pipe usage out via
/// a separate observability stack.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoopUsageSink;

#[async_trait::async_trait]
impl UsageSink for NoopUsageSink {
    async fn record(&self, _ev: &UsageEvent) {}
}

// ── LogUsageSink ────────────────────────────────────────────────────────

/// Appends NDJSON lines to a local file. One line per event; the file
/// is held under a tokio mutex and writes are pushed to a blocking
/// thread (no `tokio/fs` feature dependency).
///
/// Acceptable at human-scale traffic; switch to `WebhookUsageSink` past
/// ~100 rps.
pub struct LogUsageSink {
    writer: tokio::sync::Mutex<std::fs::File>,
}

impl LogUsageSink {
    /// Open or create `path` for append. Returns the underlying I/O
    /// error (mapped to startup failure by the binary).
    pub fn open(path: &Path) -> std::io::Result<Self> {
        let f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        Ok(Self {
            writer: tokio::sync::Mutex::new(f),
        })
    }
}

#[async_trait::async_trait]
impl UsageSink for LogUsageSink {
    async fn record(&self, ev: &UsageEvent) {
        use std::io::Write;
        let mut line = match serde_json::to_vec(ev) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(error = %e, "usage event serialise failed");
                return;
            }
        };
        line.push(b'\n');
        let mut g = self.writer.lock().await;
        // Append is small (a few hundred bytes) and the file is opened
        // O_APPEND, so a synchronous write under the mutex is fine —
        // contention only matters past ~100 rps, at which point operators
        // should switch to WebhookUsageSink.
        if let Err(e) = g.write_all(&line) {
            tracing::warn!(error = %e, "usage event write failed");
        }
    }
}

// ── WebhookUsageSink ────────────────────────────────────────────────────

#[cfg(feature = "multi-tenant")]
mod webhook {
    use std::time::Duration;

    use tokio::sync::mpsc;

    use super::UsageEvent;

    /// HTTP-backed usage sink. Batches up to 32 events per POST and
    /// retries with exponential backoff on transient failures. The
    /// `record` call is a non-blocking enqueue.
    pub struct WebhookUsageSink {
        sender: mpsc::UnboundedSender<UsageEvent>,
    }

    impl WebhookUsageSink {
        /// Spawn the background task. The returned sink can be cloned
        /// freely (it holds an `mpsc::UnboundedSender`); dropping the
        /// last clone closes the channel and ends the task.
        pub fn spawn(client: reqwest::Client, url: reqwest::Url) -> Self {
            let (tx, rx) = mpsc::unbounded_channel();
            tokio::spawn(drain(client, url, rx));
            Self { sender: tx }
        }
    }

    impl Clone for WebhookUsageSink {
        fn clone(&self) -> Self {
            Self {
                sender: self.sender.clone(),
            }
        }
    }

    async fn drain(
        client: reqwest::Client,
        url: reqwest::Url,
        mut rx: mpsc::UnboundedReceiver<UsageEvent>,
    ) {
        const BATCH: usize = 32;
        let mut buf: Vec<UsageEvent> = Vec::with_capacity(BATCH);
        while let Some(first) = rx.recv().await {
            buf.push(first);
            // Drain anything already queued, up to BATCH.
            while buf.len() < BATCH {
                match rx.try_recv() {
                    Ok(ev) => buf.push(ev),
                    Err(_) => break,
                }
            }
            post_with_retry(&client, &url, &buf).await;
            buf.clear();
        }
    }

    async fn post_with_retry(client: &reqwest::Client, url: &reqwest::Url, batch: &[UsageEvent]) {
        let mut delay = Duration::from_millis(100);
        for attempt in 0..5u32 {
            match client.post(url.clone()).json(batch).send().await {
                Ok(r) if r.status().is_success() => return,
                Ok(r) => {
                    tracing::warn!(
                        status = r.status().as_u16(),
                        attempt,
                        "usage webhook non-success"
                    );
                }
                Err(e) => {
                    tracing::warn!(error = %e, attempt, "usage webhook send failed");
                }
            }
            tokio::time::sleep(delay).await;
            delay = (delay * 2).min(Duration::from_secs(5));
        }
        tracing::warn!(dropped = batch.len(), "usage webhook batch dropped");
    }

    #[async_trait::async_trait]
    impl super::UsageSink for WebhookUsageSink {
        async fn record(&self, ev: &UsageEvent) {
            // Cloning is cheap (small struct + Strings); the channel is
            // unbounded so this never awaits.
            if self.sender.send(ev.clone()).is_err() {
                tracing::warn!("usage webhook channel closed");
            }
        }
    }
}

#[cfg(feature = "multi-tenant")]
#[allow(unused_imports)] // re-export consumed by R3's wiring in mod.rs
pub use webhook::WebhookUsageSink;
