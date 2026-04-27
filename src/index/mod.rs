//! Storage + ANN behind one trait.
//!
//! The embedded backend (redb + hnsw_rs + roaring) lives in
//! [`embedded`]. Future backends (Qdrant, LanceDB) plug in as separate
//! `IndexBackend` impls without touching the matcher.

use bytes::Bytes;

use crate::core::{Hit, Record};
use crate::error::Result;

#[cfg(feature = "embedded")]
pub mod embedded;

/// Storage + ANN abstraction. The matcher composes calls against this
/// trait; concrete backends provide the persistence.
#[async_trait::async_trait]
pub trait IndexBackend: Send + Sync {
    /// Insert or replace records by `(tenant_id, record_id)`.
    async fn upsert(&self, batch: &[Record]) -> Result<()>;

    /// Remove records by `(tenant_id, record_id)`. Idempotent — missing
    /// IDs are silently ignored.
    async fn delete(&self, tenant_id: u32, ids: &[u64]) -> Result<()>;

    /// Dense-vector k-NN inside `tenant_id`, optionally restricted to
    /// records that pass `filter` (a backend-specific encoded predicate;
    /// for the embedded backend, a roaring-bitmap expression).
    async fn knn(
        &self,
        tenant_id: u32,
        query: &[f32],
        k: usize,
        filter: Option<&Bytes>,
    ) -> Result<Vec<Hit>>;

    /// Sparse BM25 over indexed text fields. Returns top-k by score.
    async fn bm25(
        &self,
        tenant_id: u32,
        terms: &[&str],
        k: usize,
        filter: Option<&Bytes>,
    ) -> Result<Vec<Hit>>;

    /// Force pending writes to disk. Backends should already commit per
    /// upsert batch; this exists for explicit shutdown / snapshot points.
    async fn flush(&self) -> Result<()>;
}
