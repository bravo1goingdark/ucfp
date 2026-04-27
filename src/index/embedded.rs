//! Embedded `IndexBackend` impl — redb + hnsw_rs + roaring.
//!
//! See ARCHITECTURE §2 (persistence), §3 (ANN), §4 (hybrid retrieval),
//! §8.1 (per-tenant key prefixing). This file currently stubs the impl;
//! the real wiring lands as the rebuild progresses.

use std::path::Path;

use bytes::Bytes;

use crate::core::{Hit, Record};
use crate::error::{Error, Result};
use crate::index::IndexBackend;

/// Single-file embedded backend. One redb database holds:
/// - `fingerprints` table  → `(tenant_id, record_id) → bytemuck bytes`
/// - `metadata` table      → `(tenant_id, record_id) → rkyv archived`
/// - `facets` table        → `(tenant_id, facet_id)  → roaring bitmap`
/// - `vectors` table       → `(tenant_id, record_id) → f32 array bytes`
/// - `hnsw_dump` table     → `tenant_id              → hnsw_rs dump`
pub struct EmbeddedBackend {
    // TODO: redb::Database, the hnsw_rs index handle(s), and the
    //       per-tenant facet bitmap cache. See ARCHITECTURE §2 + §8.1.
    _path: std::path::PathBuf,
}

impl EmbeddedBackend {
    /// Open or create a UCFP database at `path`. The file is a single
    /// redb database; back it up with `cp` while the writer is open
    /// (see ARCHITECTURE §8.2).
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Ok(Self {
            _path: path.as_ref().to_path_buf(),
        })
    }
}

#[async_trait::async_trait]
impl IndexBackend for EmbeddedBackend {
    async fn upsert(&self, _batch: &[Record]) -> Result<()> {
        Err(Error::Index(
            "EmbeddedBackend::upsert not implemented".into(),
        ))
    }

    async fn delete(&self, _tenant_id: u32, _ids: &[u64]) -> Result<()> {
        Err(Error::Index(
            "EmbeddedBackend::delete not implemented".into(),
        ))
    }

    async fn knn(
        &self,
        _tenant_id: u32,
        _query: &[f32],
        _k: usize,
        _filter: Option<&Bytes>,
    ) -> Result<Vec<Hit>> {
        Err(Error::Index("EmbeddedBackend::knn not implemented".into()))
    }

    async fn bm25(
        &self,
        _tenant_id: u32,
        _terms: &[&str],
        _k: usize,
        _filter: Option<&Bytes>,
    ) -> Result<Vec<Hit>> {
        Err(Error::Index("EmbeddedBackend::bm25 not implemented".into()))
    }

    async fn flush(&self) -> Result<()> {
        Ok(())
    }
}
