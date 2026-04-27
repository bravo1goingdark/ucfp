//! Embedded `IndexBackend` impl — redb storage + brute-force cosine k-NN.
//!
//! Layout (per ARCHITECTURE §2 + §8.1):
//! ```text
//! fingerprints  (tenant_id: u32, record_id: u64) → bytemuck-cast SDK bytes
//! metadata      (tenant_id: u32, record_id: u64) → application metadata
//! vectors       (tenant_id: u32, record_id: u64) → f32 array (raw little-endian)
//! catalog       (tenant_id: u32, record_id: u64) → CatalogEntry (algorithm, fmt_ver, ...)
//! ```
//!
//! Per ARCHITECTURE §3, this implementation uses **brute-force cosine**
//! over the `vectors` table. That's the correct path below ~1M vectors;
//! HNSW lands as a follow-up under the same trait.
//!
//! `bm25` is not yet implemented — returns [`Error::Index`] with a clear
//! message until the FST + roaring postings layout from §4 is wired.

use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use bytes::Bytes;
use redb::{Database, ReadableDatabase, TableDefinition};

use crate::core::{Hit, HitSource, Record};
use crate::error::{Error, Result};
use crate::index::IndexBackend;

// ── Schema ──────────────────────────────────────────────────────────────
//
// Tuple keys keep the per-tenant range scan a single redb call:
// `table.range((tid, 0)..(tid, u64::MAX))`. See ARCHITECTURE §8.1.

const FINGERPRINTS: TableDefinition<'_, (u32, u64), &[u8]> =
    TableDefinition::new("ucfp/fingerprints/v1");
const METADATA: TableDefinition<'_, (u32, u64), &[u8]> = TableDefinition::new("ucfp/metadata/v1");
const VECTORS: TableDefinition<'_, (u32, u64), &[u8]> = TableDefinition::new("ucfp/vectors/v1");
const CATALOG: TableDefinition<'_, (u32, u64), &[u8]> = TableDefinition::new("ucfp/catalog/v1");

/// Single-file embedded backend.
///
/// One redb database, multiple tables, MVCC-snapshotted reads. Back up
/// with `cp` while the writer is open — redb's COW guarantees a
/// consistent snapshot (see ARCHITECTURE §8.2).
pub struct EmbeddedBackend {
    db: Arc<Database>,
    path: PathBuf,
}

impl EmbeddedBackend {
    /// Open or create a UCFP database at `path`. Creates the parent
    /// directory if it doesn't exist.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let db = Database::create(&path).map_err(|e| Error::Index(e.to_string()))?;

        // Touch every table so range scans on a fresh DB don't TableDoesNotExist.
        let txn = db.begin_write().map_err(|e| Error::Index(e.to_string()))?;
        {
            let _ = txn
                .open_table(FINGERPRINTS)
                .map_err(|e| Error::Index(e.to_string()))?;
            let _ = txn
                .open_table(METADATA)
                .map_err(|e| Error::Index(e.to_string()))?;
            let _ = txn
                .open_table(VECTORS)
                .map_err(|e| Error::Index(e.to_string()))?;
            let _ = txn
                .open_table(CATALOG)
                .map_err(|e| Error::Index(e.to_string()))?;
        }
        txn.commit().map_err(|e| Error::Index(e.to_string()))?;

        Ok(Self {
            db: Arc::new(db),
            path,
        })
    }

    /// On-disk path of the database file.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Compact catalog row carried in the `catalog` table — lets the matcher
/// answer "what does this fingerprint mean?" without re-decoding the
/// metadata blob.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct CatalogEntry {
    /// Modality discriminator: 0 = audio, 1 = image, 2 = text.
    modality: u32,
    /// Producing SDK's FORMAT_VERSION at ingest time.
    format_version: u32,
    /// SDK-specific config hash.
    config_hash: u64,
    /// Length of the fingerprint blob in bytes (sanity check on read).
    fingerprint_len: u32,
    /// Length of the embedding vector in `f32`s (0 = no embedding).
    embedding_dim: u32,
}

#[async_trait::async_trait]
impl IndexBackend for EmbeddedBackend {
    async fn upsert(&self, batch: &[Record]) -> Result<()> {
        let db = self.db.clone();
        let batch: Vec<Record> = batch.to_vec();

        tokio::task::spawn_blocking(move || -> Result<()> {
            let txn = db.begin_write().map_err(|e| Error::Index(e.to_string()))?;
            {
                let mut fps = txn
                    .open_table(FINGERPRINTS)
                    .map_err(|e| Error::Index(e.to_string()))?;
                let mut meta = txn
                    .open_table(METADATA)
                    .map_err(|e| Error::Index(e.to_string()))?;
                let mut vecs = txn
                    .open_table(VECTORS)
                    .map_err(|e| Error::Index(e.to_string()))?;
                let mut cat = txn
                    .open_table(CATALOG)
                    .map_err(|e| Error::Index(e.to_string()))?;

                for rec in &batch {
                    let key = (rec.tenant_id, rec.record_id);
                    fps.insert(key, rec.fingerprint.as_ref())
                        .map_err(|e| Error::Index(e.to_string()))?;
                    meta.insert(key, rec.metadata.as_ref())
                        .map_err(|e| Error::Index(e.to_string()))?;

                    let embedding_dim = rec.embedding.as_ref().map(|v| v.len()).unwrap_or(0) as u32;
                    if let Some(v) = rec.embedding.as_ref() {
                        vecs.insert(key, bytemuck::cast_slice::<f32, u8>(v))
                            .map_err(|e| Error::Index(e.to_string()))?;
                    } else {
                        // Drop any stale vector for this key.
                        vecs.remove(key).map_err(|e| Error::Index(e.to_string()))?;
                    }

                    let entry = CatalogEntry {
                        modality: rec.modality as u32,
                        format_version: rec.format_version,
                        config_hash: rec.config_hash,
                        fingerprint_len: rec.fingerprint.len() as u32,
                        embedding_dim,
                    };
                    cat.insert(key, bytemuck::bytes_of(&entry))
                        .map_err(|e| Error::Index(e.to_string()))?;
                }
            }
            txn.commit().map_err(|e| Error::Index(e.to_string()))?;
            Ok(())
        })
        .await
        .map_err(|e| Error::Index(format!("join error: {e}")))?
    }

    async fn delete(&self, tenant_id: u32, ids: &[u64]) -> Result<()> {
        let db = self.db.clone();
        let ids = ids.to_vec();

        tokio::task::spawn_blocking(move || -> Result<()> {
            let txn = db.begin_write().map_err(|e| Error::Index(e.to_string()))?;
            {
                let mut fps = txn
                    .open_table(FINGERPRINTS)
                    .map_err(|e| Error::Index(e.to_string()))?;
                let mut meta = txn
                    .open_table(METADATA)
                    .map_err(|e| Error::Index(e.to_string()))?;
                let mut vecs = txn
                    .open_table(VECTORS)
                    .map_err(|e| Error::Index(e.to_string()))?;
                let mut cat = txn
                    .open_table(CATALOG)
                    .map_err(|e| Error::Index(e.to_string()))?;
                for id in &ids {
                    let key = (tenant_id, *id);
                    fps.remove(key).map_err(|e| Error::Index(e.to_string()))?;
                    meta.remove(key).map_err(|e| Error::Index(e.to_string()))?;
                    vecs.remove(key).map_err(|e| Error::Index(e.to_string()))?;
                    cat.remove(key).map_err(|e| Error::Index(e.to_string()))?;
                }
            }
            txn.commit().map_err(|e| Error::Index(e.to_string()))?;
            Ok(())
        })
        .await
        .map_err(|e| Error::Index(format!("join error: {e}")))?
    }

    async fn knn(
        &self,
        tenant_id: u32,
        query: &[f32],
        k: usize,
        _filter: Option<&Bytes>,
    ) -> Result<Vec<Hit>> {
        if query.is_empty() || k == 0 {
            return Ok(Vec::new());
        }

        let db = self.db.clone();
        let query: Vec<f32> = query.to_vec();

        tokio::task::spawn_blocking(move || -> Result<Vec<Hit>> {
            let txn = db.begin_read().map_err(|e| Error::Index(e.to_string()))?;
            let table = txn
                .open_table(VECTORS)
                .map_err(|e| Error::Index(e.to_string()))?;

            let q_norm = l2_norm(&query);
            if q_norm == 0.0 {
                return Ok(Vec::new());
            }

            // Min-heap keyed by score so we evict the worst when full.
            let mut heap: BinaryHeap<HeapItem> = BinaryHeap::with_capacity(k);

            for entry in table
                .range((tenant_id, 0u64)..=(tenant_id, u64::MAX))
                .map_err(|e| Error::Index(e.to_string()))?
            {
                let (key_guard, val_guard) = entry.map_err(|e| Error::Index(e.to_string()))?;
                let (_tid, rid) = key_guard.value();
                let bytes = val_guard.value();

                if bytes.len() % std::mem::size_of::<f32>() != 0 {
                    continue;
                }
                let stored: &[f32] = bytemuck::cast_slice(bytes);
                if stored.len() != query.len() {
                    continue;
                }

                let s_norm = l2_norm(stored);
                if s_norm == 0.0 {
                    continue;
                }
                let dot = dot_product(&query, stored);
                let cos = dot / (q_norm * s_norm);

                let item = HeapItem {
                    record_id: rid,
                    score: cos,
                };
                if heap.len() < k {
                    heap.push(item);
                } else if let Some(top) = heap.peek() {
                    // BinaryHeap is a max-heap on `Ord`; we invert via
                    // HeapItem's Ord so the *worst* score sits at top.
                    if item < *top {
                        heap.pop();
                        heap.push(item);
                    }
                }
            }

            let mut hits: Vec<Hit> = heap
                .into_iter()
                .map(|h| Hit {
                    tenant_id,
                    record_id: h.record_id,
                    score: h.score,
                    source: HitSource::Vector,
                })
                .collect();
            hits.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(Ordering::Equal));
            Ok(hits)
        })
        .await
        .map_err(|e| Error::Index(format!("join error: {e}")))?
    }

    async fn bm25(
        &self,
        _tenant_id: u32,
        _terms: &[&str],
        _k: usize,
        _filter: Option<&Bytes>,
    ) -> Result<Vec<Hit>> {
        Err(Error::Index(
            "EmbeddedBackend::bm25 not implemented (TODO: FST + roaring postings, ARCHITECTURE §4)"
                .into(),
        ))
    }

    async fn flush(&self) -> Result<()> {
        // redb commits on every write tx; nothing to do beyond verifying
        // the database is reachable.
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || -> Result<()> {
            let txn = db.begin_read().map_err(|e| Error::Index(e.to_string()))?;
            drop(txn);
            Ok(())
        })
        .await
        .map_err(|e| Error::Index(format!("join error: {e}")))?
    }
}

// ── helpers ─────────────────────────────────────────────────────────────

#[inline]
fn dot_product(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

#[inline]
fn l2_norm(v: &[f32]) -> f32 {
    dot_product(v, v).sqrt()
}

/// Min-heap item: smaller score = "worse", floats to top so the worst
/// is evicted when the heap is full.
#[derive(Copy, Clone, Debug)]
struct HeapItem {
    record_id: u64,
    score: f32,
}

impl PartialEq for HeapItem {
    fn eq(&self, other: &Self) -> bool {
        self.score.eq(&other.score) && self.record_id == other.record_id
    }
}
impl Eq for HeapItem {}

impl PartialOrd for HeapItem {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for HeapItem {
    fn cmp(&self, other: &Self) -> Ordering {
        // BinaryHeap is a max-heap; we want the *worst* (lowest) score
        // at the top so it gets popped when a better candidate arrives.
        // So Self < Other when Self has a HIGHER score.
        other
            .score
            .partial_cmp(&self.score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| self.record_id.cmp(&other.record_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Modality, Record};

    fn fixture(path: &Path) -> EmbeddedBackend {
        EmbeddedBackend::open(path).unwrap()
    }

    fn rec(tenant: u32, rid: u64, embedding: Vec<f32>) -> Record {
        Record {
            tenant_id: tenant,
            record_id: rid,
            modality: Modality::Image,
            format_version: 1,
            algorithm: "test",
            config_hash: 0,
            fingerprint: Bytes::from_static(b"fp"),
            embedding: Some(embedding),
            model_id: Some("test-model".into()),
            metadata: Bytes::new(),
        }
    }

    #[tokio::test]
    async fn upsert_and_knn_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let db = fixture(&dir.path().join("ucfp.redb"));

        // Seed three records with three orthogonal-ish embeddings.
        let records = vec![
            rec(1, 100, vec![1.0, 0.0, 0.0]),
            rec(1, 200, vec![0.0, 1.0, 0.0]),
            rec(1, 300, vec![0.7, 0.7, 0.0]),
        ];
        db.upsert(&records).await.unwrap();

        // Query close to record 300.
        let hits = db.knn(1, &[0.6, 0.6, 0.0], 2, None).await.unwrap();
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].record_id, 300, "closest match should be 300");
        assert!(hits[0].score > hits[1].score);
        for hit in &hits {
            assert_eq!(hit.tenant_id, 1);
            assert_eq!(hit.source, HitSource::Vector);
        }
    }

    #[tokio::test]
    async fn knn_ignores_other_tenants() {
        let dir = tempfile::tempdir().unwrap();
        let db = fixture(&dir.path().join("ucfp.redb"));

        db.upsert(&[rec(1, 1, vec![1.0, 0.0]), rec(2, 1, vec![1.0, 0.0])])
            .await
            .unwrap();

        let hits = db.knn(1, &[1.0, 0.0], 10, None).await.unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].tenant_id, 1);
    }

    #[tokio::test]
    async fn delete_removes_records() {
        let dir = tempfile::tempdir().unwrap();
        let db = fixture(&dir.path().join("ucfp.redb"));

        db.upsert(&[rec(1, 1, vec![1.0, 0.0]), rec(1, 2, vec![0.0, 1.0])])
            .await
            .unwrap();
        db.delete(1, &[1]).await.unwrap();

        let hits = db.knn(1, &[1.0, 0.0], 10, None).await.unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].record_id, 2);
    }

    #[tokio::test]
    async fn knn_skips_records_with_no_embedding() {
        let dir = tempfile::tempdir().unwrap();
        let db = fixture(&dir.path().join("ucfp.redb"));

        let mut without = rec(1, 9, vec![1.0]);
        without.embedding = None;
        db.upsert(&[without, rec(1, 10, vec![1.0, 0.0])])
            .await
            .unwrap();

        let hits = db.knn(1, &[1.0, 0.0], 10, None).await.unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].record_id, 10);
    }

    #[tokio::test]
    async fn bm25_returns_not_implemented() {
        let dir = tempfile::tempdir().unwrap();
        let db = fixture(&dir.path().join("ucfp.redb"));
        let result = db.bm25(1, &["foo"], 10, None).await;
        assert!(matches!(result, Err(Error::Index(_))));
    }
}
