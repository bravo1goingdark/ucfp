//! Ingest source trait — where new records come from.
//!
//! The default implementation (axum POST handler under the `server`
//! feature) is in `bin/ucfp.rs`. Future `IngestSource` impls can pull
//! from S3 / GCS prefixes, NATS, or local files without touching the
//! matcher or index — see ARCHITECTURE §6 ("Ingest decoupling").

use crate::core::Record;
use crate::error::Result;

/// Pull-based source of fingerprint records.
///
/// Implementations:
/// - HTTP POST handler (default, under `server` feature) — async
/// - Local filesystem walker — for batch backfill
/// - S3 prefix scanner — for replay / cross-process decoupling
#[async_trait::async_trait]
pub trait IngestSource: Send + Sync {
    /// Pull up to `max` records from the source. Implementations may
    /// return fewer if the source is empty; an empty Vec means "no
    /// work right now, ask again later".
    async fn next_batch(&self, max: usize) -> Result<Vec<Record>>;

    /// Acknowledge that the given record IDs have been durably persisted
    /// downstream. For at-least-once sources (S3, queues), this advances
    /// the read cursor; for at-most-once (HTTP), it's a no-op.
    async fn ack(&self, ids: &[u64]) -> Result<()>;
}
