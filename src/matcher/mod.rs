//! Matcher — composes [`crate::IndexBackend`] calls into a single
//! ranked result list, optionally with a [`crate::Reranker`] pass.
//!
//! Hybrid retrieval (vector + BM25) is fused via Reciprocal Rank Fusion
//! (RRF), per ARCHITECTURE §4. RRF needs no score normalization and
//! costs ~20 lines.

use std::collections::HashMap;

use crate::core::{Hit, HitSource, Query};
use crate::error::Result;
use crate::index::IndexBackend;
use crate::rerank::Reranker;

/// Reciprocal Rank Fusion of multiple ranker outputs.
///
/// Each input list is treated as a ranking. Score per doc:
/// `Σ_i 1 / (rrf_k + rank_i(d))`. Higher is better.
///
/// `rrf_k = 60` is the universal default (Azure AI Search, OpenSearch,
/// Qdrant, Weaviate, Elasticsearch).
pub fn rrf(rankings: &[&[Hit]], rrf_k: u32) -> Vec<Hit> {
    rrf_with_sources(rankings, &[], rrf_k)
}

/// Like [`rrf`] but tags each input ranking with its [`HitSource`] so the
/// per-source contribution to the fused score can be surfaced for
/// explainability. `sources[i]` corresponds to `rankings[i]`. Sources
/// for indices past `sources.len()` default to the first hit's reported
/// source (best-effort backward compatibility for older callers).
pub fn rrf_with_sources(rankings: &[&[Hit]], sources: &[HitSource], rrf_k: u32) -> Vec<Hit> {
    let denom = rrf_k as f32;
    // Per-doc accumulator: (vec_score, bm25_score, vec_rank, bm25_rank, fallback_source).
    let mut acc: HashMap<
        (u32, u64),
        (Option<f32>, Option<f32>, Option<u32>, Option<u32>, HitSource),
    > = HashMap::new();
    for (i, ranking) in rankings.iter().enumerate() {
        let src = sources
            .get(i)
            .copied()
            .or_else(|| ranking.first().map(|h| h.source))
            .unwrap_or(HitSource::Fused);
        for (rank0, hit) in ranking.iter().enumerate() {
            let key = (hit.tenant_id, hit.record_id);
            let rank1 = (rank0 as u32) + 1;
            let inc = 1.0 / (denom + rank1 as f32);
            let entry = acc
                .entry(key)
                .or_insert((None, None, None, None, hit.source));
            match src {
                HitSource::Vector => {
                    entry.0 = Some(entry.0.unwrap_or(0.0) + inc);
                    entry.2 = entry.2.or(Some(rank1));
                }
                HitSource::Bm25 => {
                    entry.1 = Some(entry.1.unwrap_or(0.0) + inc);
                    entry.3 = entry.3.or(Some(rank1));
                }
                _ => {
                    // Unknown source: fold into vector_score so the total
                    // still matches Σ inc; rank info unavailable.
                    entry.0 = Some(entry.0.unwrap_or(0.0) + inc);
                }
            }
        }
    }
    let mut out: Vec<Hit> = acc
        .into_iter()
        .map(
            |((tenant_id, record_id), (vs, bs, vr, br, _))| {
                let total = vs.unwrap_or(0.0) + bs.unwrap_or(0.0);
                Hit {
                    tenant_id,
                    record_id,
                    score: total,
                    source: HitSource::Fused,
                    vector_score: vs,
                    bm25_score: bs,
                    vector_rank: vr,
                    bm25_rank: br,
                    term_hits: Vec::new(),
                }
            },
        )
        .collect();
    out.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    out
}

/// Query-time orchestrator. Holds references to the index backend and
/// (optionally) a reranker; `search` runs the right combination given
/// the query shape.
///
/// Default `R = NoopReranker` lets retrieval-only callers write
/// `Matcher::new(&index)` without a turbofish.
pub struct Matcher<'a, I: IndexBackend, R: Reranker = crate::rerank::NoopReranker> {
    /// Storage + ANN backend.
    pub index: &'a I,
    /// Optional second-stage reranker. Pass `None` for retrieval-only.
    pub reranker: Option<&'a R>,
}

impl<'a, I: IndexBackend> Matcher<'a, I, crate::rerank::NoopReranker> {
    /// Construct a retrieval-only matcher (no reranker).
    pub fn new(index: &'a I) -> Self {
        Self {
            index,
            reranker: None,
        }
    }
}

impl<'a, I: IndexBackend, R: Reranker> Matcher<'a, I, R> {
    /// Construct a matcher with a custom reranker. The reranker runs
    /// on the top-`k` after retrieval / RRF fusion.
    pub fn with_reranker(index: &'a I, reranker: &'a R) -> Self {
        Self {
            index,
            reranker: Some(reranker),
        }
    }

    /// Run retrieval per the query shape:
    /// - `vector.is_some() && !terms.is_empty()` → hybrid (vector ∥ BM25 → RRF)
    /// - `vector.is_some()` → vector-only
    /// - `!terms.is_empty()` → BM25-only
    /// - else → empty result (caller error)
    ///
    /// Reranker, when present, is applied to the top-`k` after fusion.
    pub async fn search(&self, q: &Query) -> Result<Vec<Hit>> {
        let mut fused: Vec<Hit> = match (q.vector.as_ref(), q.terms.is_empty()) {
            (Some(v), false) => {
                // Hybrid: kick off knn + bm25 in parallel via tokio::join.
                // Both calls are spawn_blocking-backed inside the embedded
                // backend, so they actually use independent worker threads.
                let terms: Vec<&str> = q.terms.iter().map(String::as_str).collect();
                let knn_fut = self.index.knn(q.tenant_id, v, q.k, q.filter.as_ref());
                let (vec_hits, bm_hits) = if q.explain {
                    let bm_fut =
                        self.index.bm25_explain(q.tenant_id, &terms, q.k, q.filter.as_ref());
                    tokio::try_join!(knn_fut, bm_fut)?
                } else {
                    let bm_fut = self.index.bm25(q.tenant_id, &terms, q.k, q.filter.as_ref());
                    tokio::try_join!(knn_fut, bm_fut)?
                };
                let mut fused = rrf_with_sources(
                    &[&vec_hits, &bm_hits],
                    &[HitSource::Vector, HitSource::Bm25],
                    q.rrf_k,
                );
                // Carry forward term_hits from the BM25 ranking onto the
                // fused output (RRF doesn't see them otherwise).
                if q.explain {
                    use std::collections::HashMap;
                    let mut by_id: HashMap<(u32, u64), Vec<crate::core::TermHit>> =
                        HashMap::new();
                    for h in bm_hits {
                        if !h.term_hits.is_empty() {
                            by_id.insert((h.tenant_id, h.record_id), h.term_hits);
                        }
                    }
                    for h in fused.iter_mut() {
                        if let Some(th) = by_id.remove(&(h.tenant_id, h.record_id)) {
                            h.term_hits = th;
                        }
                    }
                }
                fused
            }
            (Some(v), true) => {
                self.index
                    .knn(q.tenant_id, v, q.k, q.filter.as_ref())
                    .await?
            }
            (None, false) => {
                let terms: Vec<&str> = q.terms.iter().map(String::as_str).collect();
                if q.explain {
                    self.index
                        .bm25_explain(q.tenant_id, &terms, q.k, q.filter.as_ref())
                        .await?
                } else {
                    self.index
                        .bm25(q.tenant_id, &terms, q.k, q.filter.as_ref())
                        .await?
                }
            }
            (None, true) => Vec::new(),
        };

        fused.truncate(q.k);

        if let Some(rr) = self.reranker {
            fused = rr.rerank(q, fused).await?;
        }

        Ok(fused)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(rid: u64, score: f32, src: HitSource) -> Hit {
        Hit {
            tenant_id: 1,
            record_id: rid,
            score,
            source: src,
            vector_score: None,
            bm25_score: None,
            vector_rank: None,
            bm25_rank: None,
            term_hits: Vec::new(),
        }
    }

    #[test]
    fn rrf_with_sources_populates_breakdown_for_overlap() {
        let vec_hits = vec![
            h(10, 0.9, HitSource::Vector),
            h(20, 0.8, HitSource::Vector),
        ];
        let bm_hits = vec![
            h(20, 4.5, HitSource::Bm25),
            h(30, 4.0, HitSource::Bm25),
        ];
        let fused = rrf_with_sources(
            &[&vec_hits, &bm_hits],
            &[HitSource::Vector, HitSource::Bm25],
            60,
        );
        // Doc 20 is in both rankings — must carry both contributions and ranks.
        let twenty = fused.iter().find(|h| h.record_id == 20).unwrap();
        assert!(twenty.vector_score.is_some(), "vec_score should be set on overlapping doc");
        assert!(twenty.bm25_score.is_some(), "bm_score should be set on overlapping doc");
        assert_eq!(twenty.vector_rank, Some(2));
        assert_eq!(twenty.bm25_rank, Some(1));
        // Score equals the sum of components.
        let expected = twenty.vector_score.unwrap() + twenty.bm25_score.unwrap();
        assert!((twenty.score - expected).abs() < 1e-6);
        // Doc 10 was only in vector → bm25 fields stay None.
        let ten = fused.iter().find(|h| h.record_id == 10).unwrap();
        assert!(ten.vector_score.is_some());
        assert!(ten.bm25_score.is_none());
        // Source is always Fused for rrf output.
        for hit in &fused {
            assert_eq!(hit.source, HitSource::Fused);
        }
    }

    #[test]
    fn rrf_legacy_is_equivalent_to_with_sources_total() {
        let vec_hits = vec![h(10, 0.9, HitSource::Vector), h(20, 0.8, HitSource::Vector)];
        let bm_hits = vec![h(20, 4.5, HitSource::Bm25), h(30, 4.0, HitSource::Bm25)];
        let legacy = rrf(&[&vec_hits, &bm_hits], 60);
        let with_src = rrf_with_sources(
            &[&vec_hits, &bm_hits],
            &[HitSource::Vector, HitSource::Bm25],
            60,
        );
        assert_eq!(legacy.len(), with_src.len());
        // Order should match (sorted by total score).
        for (a, b) in legacy.iter().zip(with_src.iter()) {
            assert_eq!(a.record_id, b.record_id);
            assert!((a.score - b.score).abs() < 1e-6);
        }
    }
}
