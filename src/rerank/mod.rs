//! Optional second-stage rerank.
//!
//! Day-one default is no rerank (the matcher returns RRF-fused results
//! directly). Plug in a cross-encoder via the `rerank` feature when
//! recall@k matters more than the latency cost.

use crate::core::{Hit, Query};
use crate::error::Result;

/// Re-orders a candidate set with a richer (and slower) signal than
/// retrieval — typically a cross-encoder ONNX model, sometimes a
/// learned-to-rank GBM.
///
/// Implementations should preserve `tenant_id` and `record_id` in each
/// returned [`Hit`]; only `score` and `source` change.
#[async_trait::async_trait]
pub trait Reranker: Send + Sync {
    /// Rerank `hits` for `query`. Returning `hits` unchanged is valid
    /// (acts as identity).
    async fn rerank(&self, query: &Query, hits: Vec<Hit>) -> Result<Vec<Hit>>;
}

/// Identity reranker — returns input unchanged. Used as the default
/// generic argument so [`crate::Matcher`] users can opt out cleanly.
pub struct NoopReranker;

#[async_trait::async_trait]
impl Reranker for NoopReranker {
    async fn rerank(&self, _query: &Query, hits: Vec<Hit>) -> Result<Vec<Hit>> {
        Ok(hits)
    }
}
