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
    let denom = rrf_k as f32;
    let mut acc: HashMap<(u32, u64), (f32, HitSource)> = HashMap::new();
    for ranking in rankings {
        for (rank, hit) in ranking.iter().enumerate() {
            let key = (hit.tenant_id, hit.record_id);
            let inc = 1.0 / (denom + (rank as f32 + 1.0));
            acc.entry(key)
                .and_modify(|(s, _)| *s += inc)
                .or_insert((inc, hit.source));
        }
    }
    let mut out: Vec<Hit> = acc
        .into_iter()
        .map(|((tenant_id, record_id), (score, _))| Hit {
            tenant_id,
            record_id,
            score,
            source: HitSource::Fused,
        })
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
pub struct Matcher<'a, I: IndexBackend, R: Reranker> {
    /// Storage + ANN backend.
    pub index: &'a I,
    /// Optional second-stage reranker. Pass `None` for retrieval-only.
    pub reranker: Option<&'a R>,
}

impl<'a, I: IndexBackend, R: Reranker> Matcher<'a, I, R> {
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
                // Sequential for now; benchmark before parallelizing — see
                // ARCHITECTURE §3 for the latency budget. When the parallel
                // path matters, add `futures` to core deps and `try_join!`.
                let terms: Vec<&str> = q.terms.iter().map(String::as_str).collect();
                let vec_hits = self
                    .index
                    .knn(q.tenant_id, v, q.k, q.filter.as_ref())
                    .await?;
                let bm_hits = self
                    .index
                    .bm25(q.tenant_id, &terms, q.k, q.filter.as_ref())
                    .await?;
                rrf(&[&vec_hits, &bm_hits], q.rrf_k)
            }
            (Some(v), true) => {
                self.index
                    .knn(q.tenant_id, v, q.k, q.filter.as_ref())
                    .await?
            }
            (None, false) => {
                let terms: Vec<&str> = q.terms.iter().map(String::as_str).collect();
                self.index
                    .bm25(q.tenant_id, &terms, q.k, q.filter.as_ref())
                    .await?
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
