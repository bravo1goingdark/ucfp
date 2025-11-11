use crate::{IndexError, IndexRecord, UfpIndex};
use hashbrown::HashSet;
use std::cmp::Ordering;

/// Result entry for a similarity query
#[derive(Debug, Clone)]
pub struct QueryResult {
    pub canonical_hash: String,
    pub score: f32,
    pub metadata: serde_json::Value,
}

/// Defines the search mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryMode {
    /// Compare quantized embeddings with cosine similarity
    Semantic,
    /// Compare perceptual MinHash signatures with Jaccard similarity
    Perceptual,
}

/// Provides semantic & perceptual retrieval methods
impl UfpIndex {
    /// Compute cosine similarity between two quantized vectors
    #[inline]
    fn cosine_similarity(a: &[i8], b: &[i8]) -> f32 {
        if a.len() != b.len() || a.is_empty() {
            return 0.0;
        }
        let dot: i32 = a.iter().zip(b).map(|(&x, &y)| x as i32 * y as i32).sum();
        let norm_a = (a.iter().map(|&x| (x as i32).pow(2)).sum::<i32>() as f32).sqrt();
        let norm_b = (b.iter().map(|&x| (x as i32).pow(2)).sum::<i32>() as f32).sqrt();
        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }
        dot as f32 / (norm_a * norm_b)
    }

    /// Compute Jaccard similarity for perceptual fingerprints (MinHash)
    #[inline]
    fn jaccard_similarity(
        query: &HashSet<u64>,
        candidate: &[u64],
        scratch: &mut HashSet<u64>,
    ) -> f32 {
        if query.is_empty() || candidate.is_empty() {
            return 0.0;
        }
        scratch.clear();

        let mut intersection = 0usize;
        let mut union = query.len();

        for &value in candidate {
            if !scratch.insert(value) {
                continue;
            }

            if query.contains(&value) {
                intersection += 1;
            } else {
                union += 1;
            }
        }

        if union == 0 {
            0.0
        } else {
            intersection as f32 / union as f32
        }
    }

    /// Search for top-k most similar entries
    pub fn search(
        &self,
        query: &IndexRecord,
        mode: QueryMode,
        top_k: usize,
    ) -> Result<Vec<QueryResult>, IndexError> {
        if top_k == 0 {
            return Ok(Vec::new());
        }

        let query_embedding = query.embedding.as_ref().filter(|emb| !emb.is_empty());
        let query_perceptual = query.perceptual.as_ref().filter(|mh| !mh.is_empty());

        if matches!(mode, QueryMode::Semantic) && query_embedding.is_none() {
            return Ok(Vec::new());
        }
        if matches!(mode, QueryMode::Perceptual) && query_perceptual.is_none() {
            return Ok(Vec::new());
        }

        let perceptual_set = query_perceptual.map(|mh| {
            let mut set = HashSet::with_capacity(mh.len());
            set.extend(mh.iter().copied());
            set
        });

        let mut results = Vec::new();
        let mut scratch = HashSet::new();

        // Full scan (can be optimized later with ANN index)
        self.backend.scan(&mut |value| {
            let rec = self.decode_record(value)?;

            let score = match mode {
                QueryMode::Semantic => match (query_embedding, &rec.embedding) {
                    (Some(qe), Some(re)) => Self::cosine_similarity(qe, re),
                    _ => 0.0,
                },
                QueryMode::Perceptual => match (perceptual_set.as_ref(), &rec.perceptual) {
                    (Some(query_set), Some(rp)) => {
                        Self::jaccard_similarity(query_set, rp, &mut scratch)
                    }
                    _ => 0.0,
                },
            };

            if score > 0.0 {
                results.push(QueryResult {
                    canonical_hash: rec.canonical_hash.clone(),
                    score,
                    metadata: rec.metadata.clone(),
                });
            }
            Ok(())
        })?;

        // Sort descending by similarity
        // Break ties lexicographically so deterministic ordering doesn't depend on backend scan order.
        results.sort_unstable_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(Ordering::Equal)
                .then_with(|| a.canonical_hash.cmp(&b.canonical_hash))
        });
        results.truncate(top_k);
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jaccard_similarity_counts_value_matches() {
        let mut query_set = HashSet::new();
        query_set.extend([1_u64, 2, 3, 4]);

        let mut scratch = HashSet::new();
        let candidate = vec![4_u64, 2, 8, 9];
        let score = UfpIndex::jaccard_similarity(&query_set, &candidate, &mut scratch);

        assert!((score - (2.0 / 6.0)).abs() < f32::EPSILON);
    }
}
