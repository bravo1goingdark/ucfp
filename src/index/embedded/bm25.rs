//! BM25 inverted index for the embedded backend.
//!
//! Per ARCHITECTURE §4 the day-one keyword search is BM25 backed by an
//! `fst::Map<term, term_id>` for the dictionary plus redb tables for
//! postings, doc lengths, and corpus stats. No tantivy — phrase / fuzzy
//! / regex queries are explicit non-goals at this scale.
//!
//! All updates run inside the *same* redb write transaction as the
//! fingerprint upsert, so the index never lags behind the catalog.
//!
//! ## Layout
//!
//! | Table                          | Key                | Value                                         |
//! | ------------------------------ | ------------------ | --------------------------------------------- |
//! | `ucfp/bm25/term_fst/v1`        | `tenant_id`        | serialized `fst::Map<term, term_id>`          |
//! | `ucfp/bm25/postings/v1`        | `(tenant, term_id)`| serialized `RoaringTreemap<doc_id>`           |
//! | `ucfp/bm25/scoring/v1`         | `(tenant, term_id)`| packed `[doc_id u64 le \|\| tf u32 le]*`      |
//! | `ucfp/bm25/doc_lens/v1`        | `(tenant, doc_id)` | `u32` term count                              |
//! | `ucfp/bm25/corpus/v1`          | `tenant_id`        | `CorpusStats` (doc_count, total_len, next_id) |
//!
//! ## Scoring
//!
//! Standard Okapi BM25 with `k1 = 1.2`, `b = 0.75`. The IDF uses the
//! BM25+ smoothing `ln((N − n + 0.5) / (n + 0.5) + 1)` to keep scores
//! non-negative.

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::io::Cursor;

use fst::{IntoStreamer, Map as FstMap, MapBuilder, Streamer};
use redb::{ReadableTable, TableDefinition, WriteTransaction};
use roaring::RoaringTreemap;

use crate::core::{Hit, HitSource};
use crate::error::{Error, Result};

// ── Tables ──────────────────────────────────────────────────────────────

pub(super) const BM25_TERM_FST: TableDefinition<'_, u32, &[u8]> =
    TableDefinition::new("ucfp/bm25/term_fst/v1");

pub(super) const BM25_POSTINGS: TableDefinition<'_, (u32, u64), &[u8]> =
    TableDefinition::new("ucfp/bm25/postings/v1");

pub(super) const BM25_SCORING: TableDefinition<'_, (u32, u64), &[u8]> =
    TableDefinition::new("ucfp/bm25/scoring/v1");

pub(super) const BM25_DOC_LENS: TableDefinition<'_, (u32, u64), u32> =
    TableDefinition::new("ucfp/bm25/doc_lens/v1");

pub(super) const BM25_CORPUS: TableDefinition<'_, u32, &[u8]> =
    TableDefinition::new("ucfp/bm25/corpus/v1");

// BM25 hyper-params (Robertson/Spärck Jones 1995 defaults).
const K1: f32 = 1.2;
const B: f32 = 0.75;

// ── Tokenizer ───────────────────────────────────────────────────────────
//
// Lowercase + split on non-alphanumeric. Good enough for tags/titles per
// ARCHITECTURE §4. Linguistic / phrase / fuzzy queries are out of scope
// — promote to tantivy if those become product requirements.

pub(super) fn tokenize(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    for chunk in s.split(|c: char| !c.is_alphanumeric()) {
        if chunk.is_empty() {
            continue;
        }
        out.push(chunk.to_lowercase());
    }
    out
}

// ── Corpus stats ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct CorpusStats {
    pub doc_count: u64,
    pub total_doc_len: u64,
    pub next_term_id: u64,
}

impl CorpusStats {
    fn pack(&self) -> [u8; 24] {
        let mut buf = [0u8; 24];
        buf[..8].copy_from_slice(&self.doc_count.to_le_bytes());
        buf[8..16].copy_from_slice(&self.total_doc_len.to_le_bytes());
        buf[16..24].copy_from_slice(&self.next_term_id.to_le_bytes());
        buf
    }

    fn unpack(b: &[u8]) -> Self {
        if b.len() != 24 {
            return Self::default();
        }
        let mut dc = [0u8; 8];
        dc.copy_from_slice(&b[..8]);
        let mut tl = [0u8; 8];
        tl.copy_from_slice(&b[8..16]);
        let mut nt = [0u8; 8];
        nt.copy_from_slice(&b[16..24]);
        CorpusStats {
            doc_count: u64::from_le_bytes(dc),
            total_doc_len: u64::from_le_bytes(tl),
            next_term_id: u64::from_le_bytes(nt),
        }
    }

    fn avgdl(&self) -> f32 {
        if self.doc_count == 0 {
            return 0.0;
        }
        self.total_doc_len as f32 / self.doc_count as f32
    }
}

// ── Scoring postings: packed [doc_id u64 LE || tf u32 LE]* ──────────────

fn pack_scoring(entries: &[(u64, u32)]) -> Vec<u8> {
    let mut out = Vec::with_capacity(entries.len() * 12);
    for (doc, tf) in entries {
        out.extend_from_slice(&doc.to_le_bytes());
        out.extend_from_slice(&tf.to_le_bytes());
    }
    out
}

fn unpack_scoring(b: &[u8]) -> Vec<(u64, u32)> {
    let mut out = Vec::with_capacity(b.len() / 12);
    let mut i = 0;
    while i + 12 <= b.len() {
        let mut d = [0u8; 8];
        d.copy_from_slice(&b[i..i + 8]);
        let mut t = [0u8; 4];
        t.copy_from_slice(&b[i + 8..i + 12]);
        out.push((u64::from_le_bytes(d), u32::from_le_bytes(t)));
        i += 12;
    }
    out
}

// ── Read helpers ────────────────────────────────────────────────────────

pub(super) fn read_corpus(txn: &WriteTransaction, tenant_id: u32) -> Result<CorpusStats> {
    let table = txn
        .open_table(BM25_CORPUS)
        .map_err(|e| Error::Index(e.to_string()))?;
    match table
        .get(tenant_id)
        .map_err(|e| Error::Index(e.to_string()))?
    {
        Some(v) => Ok(CorpusStats::unpack(v.value())),
        None => Ok(CorpusStats::default()),
    }
}

pub(super) fn read_term_dict(
    txn: &WriteTransaction,
    tenant_id: u32,
) -> Result<BTreeMap<String, u64>> {
    let table = txn
        .open_table(BM25_TERM_FST)
        .map_err(|e| Error::Index(e.to_string()))?;
    let row = table
        .get(tenant_id)
        .map_err(|e| Error::Index(e.to_string()))?;
    let bytes = match row {
        Some(v) => v.value().to_vec(),
        None => return Ok(BTreeMap::new()),
    };
    let map = FstMap::new(bytes).map_err(|e| Error::Index(format!("fst load: {e}")))?;
    let mut out = BTreeMap::new();
    let mut stream = map.into_stream();
    while let Some((term_bytes, term_id)) = stream.next() {
        let term =
            String::from_utf8(term_bytes.to_vec()).map_err(|e| Error::Index(e.to_string()))?;
        out.insert(term, term_id);
    }
    Ok(out)
}

fn read_postings(txn: &WriteTransaction, tenant_id: u32, term_id: u64) -> Result<RoaringTreemap> {
    let table = txn
        .open_table(BM25_POSTINGS)
        .map_err(|e| Error::Index(e.to_string()))?;
    let row = table
        .get((tenant_id, term_id))
        .map_err(|e| Error::Index(e.to_string()))?;
    match row {
        Some(v) => RoaringTreemap::deserialize_from(Cursor::new(v.value()))
            .map_err(|e| Error::Index(format!("roaring deser: {e}"))),
        None => Ok(RoaringTreemap::new()),
    }
}

fn read_scoring(txn: &WriteTransaction, tenant_id: u32, term_id: u64) -> Result<Vec<(u64, u32)>> {
    let table = txn
        .open_table(BM25_SCORING)
        .map_err(|e| Error::Index(e.to_string()))?;
    let row = table
        .get((tenant_id, term_id))
        .map_err(|e| Error::Index(e.to_string()))?;
    Ok(match row {
        Some(v) => unpack_scoring(v.value()),
        None => Vec::new(),
    })
}

fn read_doc_len(txn: &WriteTransaction, tenant_id: u32, doc_id: u64) -> Result<Option<u32>> {
    let table = txn
        .open_table(BM25_DOC_LENS)
        .map_err(|e| Error::Index(e.to_string()))?;
    Ok(table
        .get((tenant_id, doc_id))
        .map_err(|e| Error::Index(e.to_string()))?
        .map(|v| v.value()))
}

// ── Write helpers ───────────────────────────────────────────────────────

fn write_term_dict(
    txn: &WriteTransaction,
    tenant_id: u32,
    dict: &BTreeMap<String, u64>,
) -> Result<()> {
    let mut buf = Vec::new();
    let mut builder =
        MapBuilder::new(&mut buf).map_err(|e| Error::Index(format!("fst builder: {e}")))?;
    // BTreeMap iterates sorted by key — exactly what fst::MapBuilder needs.
    for (term, id) in dict {
        builder
            .insert(term.as_bytes(), *id)
            .map_err(|e| Error::Index(format!("fst insert: {e}")))?;
    }
    builder
        .finish()
        .map_err(|e| Error::Index(format!("fst finish: {e}")))?;
    let mut table = txn
        .open_table(BM25_TERM_FST)
        .map_err(|e| Error::Index(e.to_string()))?;
    table
        .insert(tenant_id, buf.as_slice())
        .map_err(|e| Error::Index(e.to_string()))?;
    Ok(())
}

fn write_postings(
    txn: &WriteTransaction,
    tenant_id: u32,
    term_id: u64,
    bm: &RoaringTreemap,
) -> Result<()> {
    let mut buf = Vec::with_capacity(bm.serialized_size());
    bm.serialize_into(&mut buf)
        .map_err(|e| Error::Index(format!("roaring ser: {e}")))?;
    let mut table = txn
        .open_table(BM25_POSTINGS)
        .map_err(|e| Error::Index(e.to_string()))?;
    table
        .insert((tenant_id, term_id), buf.as_slice())
        .map_err(|e| Error::Index(e.to_string()))?;
    Ok(())
}

fn write_scoring(
    txn: &WriteTransaction,
    tenant_id: u32,
    term_id: u64,
    entries: &[(u64, u32)],
) -> Result<()> {
    let buf = pack_scoring(entries);
    let mut table = txn
        .open_table(BM25_SCORING)
        .map_err(|e| Error::Index(e.to_string()))?;
    table
        .insert((tenant_id, term_id), buf.as_slice())
        .map_err(|e| Error::Index(e.to_string()))?;
    Ok(())
}

fn write_doc_len(txn: &WriteTransaction, tenant_id: u32, doc_id: u64, len: u32) -> Result<()> {
    let mut table = txn
        .open_table(BM25_DOC_LENS)
        .map_err(|e| Error::Index(e.to_string()))?;
    table
        .insert((tenant_id, doc_id), len)
        .map_err(|e| Error::Index(e.to_string()))?;
    Ok(())
}

fn write_corpus(txn: &WriteTransaction, tenant_id: u32, stats: &CorpusStats) -> Result<()> {
    let mut table = txn
        .open_table(BM25_CORPUS)
        .map_err(|e| Error::Index(e.to_string()))?;
    table
        .insert(tenant_id, stats.pack().as_slice())
        .map_err(|e| Error::Index(e.to_string()))?;
    Ok(())
}

// ── Upsert ──────────────────────────────────────────────────────────────

/// Index (or re-index) a single document inside an in-flight write txn.
/// Caller is responsible for committing the transaction.
///
/// Idempotent: re-indexing the same `(tenant_id, record_id)` replaces
/// the prior tf contribution rather than double-counting.
pub(super) fn upsert_one(
    txn: &WriteTransaction,
    tenant_id: u32,
    record_id: u64,
    text: &str,
) -> Result<()> {
    // First clear any prior contribution for this doc — keeps re-ingest
    // semantically clean.
    let prev_len = clear_one(txn, tenant_id, record_id)?;

    let tokens = tokenize(text);
    let doc_len = u32::try_from(tokens.len()).unwrap_or(u32::MAX);

    let mut tf: HashMap<String, u32> = HashMap::new();
    for tok in tokens {
        *tf.entry(tok).or_insert(0) += 1;
    }

    let mut corpus = read_corpus(txn, tenant_id)?;
    let mut term_to_id = read_term_dict(txn, tenant_id)?;

    let mut new_term_added = false;
    for term in tf.keys() {
        if !term_to_id.contains_key(term) {
            let tid = corpus.next_term_id;
            corpus.next_term_id = tid
                .checked_add(1)
                .ok_or_else(|| Error::Index("BM25 term_id overflow".into()))?;
            term_to_id.insert(term.clone(), tid);
            new_term_added = true;
        }
    }

    for (term, tf_in_doc) in &tf {
        let tid = term_to_id[term];
        let mut bm = read_postings(txn, tenant_id, tid)?;
        bm.insert(record_id);
        write_postings(txn, tenant_id, tid, &bm)?;
        let mut entries = read_scoring(txn, tenant_id, tid)?;
        entries.push((record_id, *tf_in_doc));
        write_scoring(txn, tenant_id, tid, &entries)?;
    }

    write_doc_len(txn, tenant_id, record_id, doc_len)?;

    // Doc count: clear_one already decremented if the doc existed; we
    // unconditionally re-add since the doc is being indexed now.
    if prev_len.is_none() {
        // truly new doc
    }
    corpus.doc_count = corpus.doc_count.saturating_add(1);
    corpus.total_doc_len = corpus.total_doc_len.saturating_add(doc_len as u64);

    if new_term_added {
        write_term_dict(txn, tenant_id, &term_to_id)?;
    }
    write_corpus(txn, tenant_id, &corpus)?;
    Ok(())
}

/// Remove a doc's contribution from postings, scoring, doc_lens, and
/// corpus stats. Returns the previous doc_len if one existed.
pub(super) fn clear_one(
    txn: &WriteTransaction,
    tenant_id: u32,
    record_id: u64,
) -> Result<Option<u32>> {
    let prev_len = read_doc_len(txn, tenant_id, record_id)?;
    let Some(prev_len_v) = prev_len else {
        return Ok(None);
    };

    let term_to_id = read_term_dict(txn, tenant_id)?;
    // Walk every term — for a v1 with hundreds of terms per tenant this
    // is fine; an inverse-doc-id index would tighten this hot path.
    for tid in term_to_id.values() {
        let mut entries = read_scoring(txn, tenant_id, *tid)?;
        let before = entries.len();
        entries.retain(|(d, _)| *d != record_id);
        if entries.len() != before {
            write_scoring(txn, tenant_id, *tid, &entries)?;
            let mut bm = read_postings(txn, tenant_id, *tid)?;
            bm.remove(record_id);
            write_postings(txn, tenant_id, *tid, &bm)?;
        }
    }

    let mut doc_lens = txn
        .open_table(BM25_DOC_LENS)
        .map_err(|e| Error::Index(e.to_string()))?;
    doc_lens
        .remove((tenant_id, record_id))
        .map_err(|e| Error::Index(e.to_string()))?;
    drop(doc_lens);

    let mut corpus = read_corpus(txn, tenant_id)?;
    corpus.doc_count = corpus.doc_count.saturating_sub(1);
    corpus.total_doc_len = corpus.total_doc_len.saturating_sub(prev_len_v as u64);
    write_corpus(txn, tenant_id, &corpus)?;

    Ok(Some(prev_len_v))
}

// ── Query ───────────────────────────────────────────────────────────────

/// BM25 top-k search inside `tenant_id`. `terms` is the already-
/// tokenized query (caller is responsible for matching the index-time
/// tokenizer).
pub(super) fn search(
    db: &redb::Database,
    tenant_id: u32,
    terms: &[&str],
    k: usize,
) -> Result<Vec<Hit>> {
    use redb::ReadableDatabase;
    if k == 0 || terms.is_empty() {
        return Ok(Vec::new());
    }
    let read = db.begin_read().map_err(|e| Error::Index(e.to_string()))?;

    // Corpus stats
    let corpus = match read
        .open_table(BM25_CORPUS)
        .map_err(|e| Error::Index(e.to_string()))?
        .get(tenant_id)
        .map_err(|e| Error::Index(e.to_string()))?
    {
        Some(v) => CorpusStats::unpack(v.value()),
        None => return Ok(Vec::new()),
    };
    if corpus.doc_count == 0 {
        return Ok(Vec::new());
    }
    let avgdl = corpus.avgdl();
    let n = corpus.doc_count as f32;

    // FST term dict
    let fst_table = read
        .open_table(BM25_TERM_FST)
        .map_err(|e| Error::Index(e.to_string()))?;
    let dict_bytes = match fst_table
        .get(tenant_id)
        .map_err(|e| Error::Index(e.to_string()))?
    {
        Some(v) => v.value().to_vec(),
        None => return Ok(Vec::new()),
    };
    let fst_map = FstMap::new(dict_bytes).map_err(|e| Error::Index(format!("fst load: {e}")))?;

    let scoring_table = read
        .open_table(BM25_SCORING)
        .map_err(|e| Error::Index(e.to_string()))?;
    let doc_lens_table = read
        .open_table(BM25_DOC_LENS)
        .map_err(|e| Error::Index(e.to_string()))?;

    let mut accum: HashMap<u64, f32> = HashMap::new();
    // Tokenize each provided query term again so single-token args like
    // "Hello, World" still split sensibly.
    let mut query_terms: Vec<String> = Vec::new();
    for raw in terms {
        for tok in tokenize(raw) {
            query_terms.push(tok);
        }
    }

    for term in &query_terms {
        let Some(term_id) = fst_map.get(term.as_bytes()) else {
            continue;
        };
        let row = scoring_table
            .get((tenant_id, term_id))
            .map_err(|e| Error::Index(e.to_string()))?;
        let entries = match row {
            Some(v) => unpack_scoring(v.value()),
            None => continue,
        };
        if entries.is_empty() {
            continue;
        }
        let n_with_term = entries.len() as f32;
        // BM25+ smoothed IDF — non-negative.
        let idf = ((n - n_with_term + 0.5) / (n_with_term + 0.5) + 1.0).ln();

        for (doc_id, tf) in entries {
            let dl = doc_lens_table
                .get((tenant_id, doc_id))
                .map_err(|e| Error::Index(e.to_string()))?
                .map(|v| v.value())
                .unwrap_or(0) as f32;
            let denom = (tf as f32) + K1 * (1.0 - B + B * dl / avgdl.max(1.0));
            let contribution = idf * ((tf as f32) * (K1 + 1.0)) / denom.max(1e-6);
            *accum.entry(doc_id).or_insert(0.0) += contribution;
        }
    }

    let mut hits: Vec<Hit> = accum
        .into_iter()
        .map(|(record_id, score)| Hit {
            tenant_id,
            record_id,
            score,
            source: HitSource::Bm25,
        })
        .collect();

    // Top-k by score (partial sort).
    if hits.len() > k {
        hits.select_nth_unstable_by(k, |a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        hits.truncate(k);
    }
    hits.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok(hits)
}

// ── Schema bootstrap ────────────────────────────────────────────────────

/// Touch every BM25 table inside the given write txn so first-time
/// readers don't see `TableDoesNotExist` errors. The caller commits.
pub(super) fn bootstrap_tables(txn: &WriteTransaction) -> Result<()> {
    let _ = txn
        .open_table(BM25_TERM_FST)
        .map_err(|e| Error::Index(e.to_string()))?;
    let _ = txn
        .open_table(BM25_POSTINGS)
        .map_err(|e| Error::Index(e.to_string()))?;
    let _ = txn
        .open_table(BM25_SCORING)
        .map_err(|e| Error::Index(e.to_string()))?;
    let _ = txn
        .open_table(BM25_DOC_LENS)
        .map_err(|e| Error::Index(e.to_string()))?;
    let _ = txn
        .open_table(BM25_CORPUS)
        .map_err(|e| Error::Index(e.to_string()))?;
    Ok(())
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use redb::Database;
    use tempfile::tempdir;

    fn open_db(path: &std::path::Path) -> Database {
        Database::create(path).unwrap()
    }

    fn upsert(db: &Database, tenant: u32, rid: u64, text: &str) {
        let txn = db.begin_write().unwrap();
        bootstrap_tables(&txn).unwrap();
        upsert_one(&txn, tenant, rid, text).unwrap();
        txn.commit().unwrap();
    }

    #[test]
    fn tokenize_lowercases_and_splits() {
        assert_eq!(
            tokenize("Hello, World!  It's GREAT."),
            vec!["hello", "world", "it", "s", "great"]
        );
    }

    #[test]
    fn round_trip_single_doc() {
        let dir = tempdir().unwrap();
        let db = open_db(&dir.path().join("u.redb"));
        upsert(&db, 1, 100, "the quick brown fox");

        let hits = search(&db, 1, &["fox"], 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].record_id, 100);
        assert!(hits[0].score > 0.0);
    }

    #[test]
    fn ranks_by_relevance() {
        let dir = tempdir().unwrap();
        let db = open_db(&dir.path().join("u.redb"));
        upsert(&db, 1, 100, "rust rust rust async");
        upsert(&db, 1, 101, "rust async language");
        upsert(&db, 1, 102, "go language");

        // "rust" scoring: doc 100 has tf=3, doc 101 tf=1. Doc 102 absent.
        let hits = search(&db, 1, &["rust"], 10).unwrap();
        let ids: Vec<u64> = hits.iter().map(|h| h.record_id).collect();
        assert_eq!(ids, vec![100, 101]);
    }

    #[test]
    fn multi_term_query() {
        let dir = tempdir().unwrap();
        let db = open_db(&dir.path().join("u.redb"));
        upsert(&db, 1, 1, "rust async language");
        upsert(&db, 1, 2, "go async language");
        upsert(&db, 1, 3, "rust safety");

        let hits = search(&db, 1, &["rust", "async"], 10).unwrap();
        // Doc 1 hits both terms; should outrank docs that hit only one.
        assert_eq!(hits[0].record_id, 1);
    }

    #[test]
    fn tenant_isolation() {
        let dir = tempdir().unwrap();
        let db = open_db(&dir.path().join("u.redb"));
        upsert(&db, 1, 100, "tenant one document");
        upsert(&db, 2, 200, "tenant two document");

        let hits1 = search(&db, 1, &["document"], 10).unwrap();
        assert_eq!(hits1.len(), 1);
        assert_eq!(hits1[0].record_id, 100);

        let hits2 = search(&db, 2, &["document"], 10).unwrap();
        assert_eq!(hits2.len(), 1);
        assert_eq!(hits2[0].record_id, 200);
    }

    #[test]
    fn unknown_term_returns_empty() {
        let dir = tempdir().unwrap();
        let db = open_db(&dir.path().join("u.redb"));
        upsert(&db, 1, 1, "the quick brown fox");
        let hits = search(&db, 1, &["zebra"], 10).unwrap();
        assert!(hits.is_empty());
    }

    #[test]
    fn delete_removes_from_scoring() {
        let dir = tempdir().unwrap();
        let db = open_db(&dir.path().join("u.redb"));
        upsert(&db, 1, 100, "rust async");
        upsert(&db, 1, 101, "rust safety");

        // Clear doc 100.
        let txn = db.begin_write().unwrap();
        clear_one(&txn, 1, 100).unwrap();
        txn.commit().unwrap();

        let hits = search(&db, 1, &["rust"], 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].record_id, 101);
    }

    #[test]
    fn re_upsert_replaces_tf() {
        let dir = tempdir().unwrap();
        let db = open_db(&dir.path().join("u.redb"));
        // First with very high tf.
        upsert(&db, 1, 100, "rust rust rust rust rust rust");
        // Re-index with low tf.
        upsert(&db, 1, 100, "rust other words here");

        // Compare to a fresh competing doc.
        upsert(&db, 1, 101, "rust rust rust rust rust");

        let hits = search(&db, 1, &["rust"], 10).unwrap();
        // After re-ingest, doc 101 (5x rust) should outrank doc 100 (1x rust).
        assert_eq!(hits[0].record_id, 101);
    }

    #[test]
    fn empty_text_records_doc_len_zero() {
        let dir = tempdir().unwrap();
        let db = open_db(&dir.path().join("u.redb"));
        upsert(&db, 1, 100, "   ,,, ...   ");
        // No terms, so query returns nothing.
        let hits = search(&db, 1, &["anything"], 10).unwrap();
        assert!(hits.is_empty());
    }
}
