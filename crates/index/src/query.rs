use crate::{IndexError, IndexRecord, UfpIndex};
use hashbrown::HashSet;
use std::cmp::Ordering;

/// Result entry for a similarity query.
#[derive(Debug, Clone)]
pub struct QueryResult {
    /// Canonical hash of the matched document.
    pub canonical_hash: String,
    /// Similarity score (0.0 to 1.0, higher is more similar).
    pub score: f32,
    /// Metadata associated with the matched document.
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

/// Chunk size for SIMD-optimized operations
const SIMD_CHUNK_SIZE: usize = 32;

/// Provides semantic & perceptual retrieval methods
impl UfpIndex {
    /// Compute cosine similarity between two quantized vectors.
    /// The dot product is computed on the i8 values, then normalized.
    /// Uses chunked processing for better auto-vectorization.
    #[inline]
    fn cosine_similarity(a: &[i8], b: &[i8]) -> f32 {
        if a.len() != b.len() || a.is_empty() {
            return 0.0;
        }

        let len = a.len();
        let mut dot: i32 = 0;
        let mut norm_a: i32 = 0;
        let mut norm_b: i32 = 0;

        // Process in chunks for better cache locality and auto-vectorization
        let chunks = len / SIMD_CHUNK_SIZE;
        let remainder = len % SIMD_CHUNK_SIZE;

        // Process full chunks
        for chunk_idx in 0..chunks {
            let offset = chunk_idx * SIMD_CHUNK_SIZE;
            let chunk_dot = Self::compute_dot_chunk(
                &a[offset..offset + SIMD_CHUNK_SIZE],
                &b[offset..offset + SIMD_CHUNK_SIZE],
            );
            let (chunk_norm_a, chunk_norm_b) = Self::compute_norms_chunk(
                &a[offset..offset + SIMD_CHUNK_SIZE],
                &b[offset..offset + SIMD_CHUNK_SIZE],
            );
            dot += chunk_dot;
            norm_a += chunk_norm_a;
            norm_b += chunk_norm_b;
        }

        // Process remainder
        if remainder > 0 {
            let offset = chunks * SIMD_CHUNK_SIZE;
            let chunk_dot = Self::compute_dot_chunk(&a[offset..], &b[offset..]);
            let (chunk_norm_a, chunk_norm_b) =
                Self::compute_norms_chunk(&a[offset..], &b[offset..]);
            dot += chunk_dot;
            norm_a += chunk_norm_a;
            norm_b += chunk_norm_b;
        }

        let norm_a_f = (norm_a as f32).sqrt();
        let norm_b_f = (norm_b as f32).sqrt();

        if norm_a_f == 0.0 || norm_b_f == 0.0 {
            return 0.0;
        }

        dot as f32 / (norm_a_f * norm_b_f)
    }

    /// Compute dot product for a chunk with auto-vectorization hints
    #[inline(always)]
    fn compute_dot_chunk(a: &[i8], b: &[i8]) -> i32 {
        a.iter()
            .zip(b.iter())
            .map(|(&x, &y)| (x as i32) * (y as i32))
            .sum()
    }

    /// Compute norms for a chunk with auto-vectorization hints
    #[inline(always)]
    fn compute_norms_chunk(a: &[i8], b: &[i8]) -> (i32, i32) {
        let norm_a: i32 = a.iter().map(|&x| (x as i32) * (x as i32)).sum();
        let norm_b: i32 = b.iter().map(|&x| (x as i32) * (x as i32)).sum();
        (norm_a, norm_b)
    }

    /// Compute Jaccard similarity for perceptual fingerprints (MinHash).
    /// This is the size of the intersection divided by the size of the union.
    #[inline]
    fn jaccard_similarity(
        query: &HashSet<u64>,
        candidate: &[u64],
        scratch: &mut HashSet<u64>,
    ) -> f32 {
        if query.is_empty() || candidate.is_empty() {
            return 0.0;
        }
        // The scratch space is used to avoid re-allocating a HashSet for each candidate.
        scratch.clear();

        let mut intersection = 0usize;
        let mut union = query.len();

        for &value in candidate {
            // If the value is already in the scratch set, it's a duplicate in the candidate.
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

    /// Search for top-k most similar entries.
    pub fn search(
        &self,
        query: &IndexRecord,
        mode: QueryMode,
        top_k: usize,
    ) -> Result<Vec<QueryResult>, IndexError> {
        if top_k == 0 {
            return Ok(Vec::new());
        }

        // Extract the query vectors, returning early if they are empty for the selected mode.
        let query_embedding = query.embedding.as_ref().filter(|emb| !emb.is_empty());
        let query_perceptual = query.perceptual.as_ref().filter(|mh| !mh.is_empty());

        if matches!(mode, QueryMode::Semantic) && query_embedding.is_none() {
            return Ok(Vec::new());
        }
        if matches!(mode, QueryMode::Perceptual) && query_perceptual.is_none() {
            return Ok(Vec::new());
        }

        // For perceptual search, convert the query MinHash vector to a HashSet for efficient lookups.
        let perceptual_set = query_perceptual.map(|mh| {
            let mut set = HashSet::with_capacity(mh.len());
            set.extend(mh.iter().copied());
            set
        });

        let mut results = Vec::new();
        let mut scratch = HashSet::new();
        let mut processed_hashes = std::collections::HashSet::new();

        match mode {
            QueryMode::Perceptual => {
                if let (Some(query_set), Some(_)) = (perceptual_set.as_ref(), query_perceptual) {
                    // Count candidate frequencies using the lock-free inverted index
                    let mut candidate_counts = std::collections::HashMap::new();
                    for &hash_val in query_set {
                        if let Some(candidates) = self.perceptual_index.get(&hash_val) {
                            for candidate_hash in candidates.value() {
                                *candidate_counts.entry(candidate_hash.clone()).or_insert(0) += 1;
                            }
                        }
                    }

                    // Calculate Jaccard similarity for candidates
                    for (candidate_hash, intersection_size) in candidate_counts {
                        if intersection_size > 0 {
                            if let Some(rec_data) = self.backend.get(&candidate_hash)? {
                                let rec = self.decode_record(&rec_data);
                                if let Ok(record) = rec {
                                    if let Some(rp) = &record.perceptual {
                                        let score =
                                            Self::jaccard_similarity(query_set, rp, &mut scratch);
                                        if score > 0.0 {
                                            results.push(QueryResult {
                                                canonical_hash: candidate_hash.clone(),
                                                score,
                                                metadata: record.metadata.clone(),
                                            });
                                            processed_hashes.insert(candidate_hash);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            QueryMode::Semantic => {
                // Try to use ANN if available and dataset is large enough
                self.rebuild_ann_if_needed();

                if let Some(query_embedding) = query_embedding {
                    if self.should_use_ann() {
                        // Use ANN for approximate search
                        if let Ok(ann_lock) = self.ann_index.try_lock() {
                            if let Some(ref ann) = *ann_lock {
                                // Convert query from i8 to f32
                                let query_f32: Vec<f32> =
                                    query_embedding.iter().map(|&v| v as f32 / 100.0).collect();

                                if let Ok(ann_results) = ann.search(&query_f32, top_k * 2) {
                                    for ann_result in ann_results {
                                        if let Some(candidate_hash) = ann.get_id(ann_result.index) {
                                            if let Some(rec_data) =
                                                self.backend.get(candidate_hash)?
                                            {
                                                if let Ok(record) = self.decode_record(&rec_data) {
                                                    // Convert distance back to similarity
                                                    let score =
                                                        1.0 - ann_result.distance.clamp(0.0, 1.0);
                                                    if score > 0.0 {
                                                        results.push(QueryResult {
                                                            canonical_hash: candidate_hash.clone(),
                                                            score,
                                                            metadata: record.metadata.clone(),
                                                        });
                                                        processed_hashes
                                                            .insert(candidate_hash.clone());
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Fall back to linear scan if ANN not available or didn't return enough results
                    if results.is_empty() {
                        // Simple vector search using lock-free DashMap
                        for entry in self.semantic_index.iter() {
                            let candidate_hash = entry.key();
                            let candidate_embedding = entry.value();
                            let score =
                                Self::cosine_similarity(query_embedding, candidate_embedding);
                            if score > 0.0 && !processed_hashes.contains(candidate_hash) {
                                if let Some(rec_data) = self.backend.get(candidate_hash)? {
                                    let rec = self.decode_record(&rec_data);
                                    if let Ok(record) = rec {
                                        results.push(QueryResult {
                                            canonical_hash: candidate_hash.clone(),
                                            score,
                                            metadata: record.metadata.clone(),
                                        });
                                        processed_hashes.insert(candidate_hash.clone());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Sort results by score in descending order.
        // Ties are broken by the canonical hash to ensure deterministic ordering.
        results.sort_unstable_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(Ordering::Equal)
                .then_with(|| a.canonical_hash.cmp(&b.canonical_hash))
        });
        // Return only the top-k results.
        results.truncate(top_k);
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BackendConfig, IndexConfig, INDEX_SCHEMA_VERSION};
    use serde_json::json;

    #[test]
    fn jaccard_similarity_counts_value_matches() {
        let mut query_set = HashSet::new();
        query_set.extend([1_u64, 2, 3, 4]);

        let mut scratch = HashSet::new();
        let candidate = vec![4_u64, 2, 8, 9];
        let score = UfpIndex::jaccard_similarity(&query_set, &candidate, &mut scratch);

        assert!((score - (2.0 / 6.0)).abs() < f32::EPSILON);
    }

    #[test]
    fn semantic_search_orders_by_score_and_tie_breaks_hashes() {
        let index = seed_index(vec![
            semantic_record("doc-b", &[5, 0, 0, 0]),
            semantic_record("doc-a", &[5, 0, 0, 0]),
            semantic_record("doc-c", &[1, 1, 1, 1]),
        ]);

        let query = IndexRecord {
            schema_version: INDEX_SCHEMA_VERSION,
            canonical_hash: "query".into(),
            perceptual: None,
            embedding: Some(vec![5, 0, 0, 0]),
            metadata: json!({}),
        };

        let hits = index
            .search(&query, QueryMode::Semantic, 3)
            .expect("semantic search");
        assert_eq!(hits.len(), 3);
        assert_eq!(hits[0].canonical_hash, "doc-a");
        assert_eq!(hits[1].canonical_hash, "doc-b");
        assert_eq!(hits[2].canonical_hash, "doc-c");
        assert!((hits[0].score - hits[1].score).abs() < f32::EPSILON);
    }

    #[test]
    fn perceptual_search_respects_top_k_and_filters_zero_scores() {
        let index = seed_index(vec![
            perceptual_record("doc-a", &[1, 2, 9, 10]),
            perceptual_record("doc-b", &[3, 4, 7, 8]),
            perceptual_record("doc-c", &[10, 11, 12, 13]),
        ]);

        let query = IndexRecord {
            schema_version: INDEX_SCHEMA_VERSION,
            canonical_hash: "query".into(),
            perceptual: Some(vec![3, 4, 7, 8]),
            embedding: None,
            metadata: json!({}),
        };

        let hits = index
            .search(&query, QueryMode::Perceptual, 1)
            .expect("perceptual search");

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].canonical_hash, "doc-b");
        assert!(hits[0].score > 0.0);
    }

    #[test]
    fn zero_top_k_short_circuits() {
        let index = seed_index(vec![semantic_record("doc-a", &[1, 0, 0, 0])]);
        let query = IndexRecord {
            schema_version: INDEX_SCHEMA_VERSION,
            canonical_hash: "query".into(),
            embedding: Some(vec![1, 0, 0, 0]),
            perceptual: None,
            metadata: json!({}),
        };

        let hits = index
            .search(&query, QueryMode::Semantic, 0)
            .expect("semantic search");
        assert!(hits.is_empty());
    }

    #[test]
    fn cosine_similarity_chunked_matches_scalar() {
        // Test that chunked implementation matches reference
        let a = vec![10_i8, 20, 30, 40, 50];
        let b = vec![5_i8, 10, 15, 20, 25];

        let result = UfpIndex::cosine_similarity(&a, &b);

        // Compute reference
        let dot: i32 = a
            .iter()
            .zip(&b)
            .map(|(&x, &y)| (x as i32) * (y as i32))
            .sum();
        let norm_a = (a.iter().map(|&x| (x as i32) * (x as i32)).sum::<i32>() as f32).sqrt();
        let norm_b = (b.iter().map(|&x| (x as i32) * (x as i32)).sum::<i32>() as f32).sqrt();
        let expected = dot as f32 / (norm_a * norm_b);

        assert!((result - expected).abs() < 0.0001);
    }

    #[test]
    fn cosine_similarity_large_vector() {
        // Test with vector larger than chunk size
        let a: Vec<i8> = (0..100).map(|i| (i % 127) as i8).collect();
        let b: Vec<i8> = (0..100).map(|i| ((i + 10) % 127) as i8).collect();

        let result = UfpIndex::cosine_similarity(&a, &b);
        assert!((0.0..=1.0).contains(&result));
    }

    fn seed_index(records: Vec<IndexRecord>) -> UfpIndex {
        let cfg = IndexConfig::new().with_backend(BackendConfig::in_memory());
        let index = UfpIndex::new(cfg).expect("index init");
        for record in records {
            index.upsert(&record).expect("seed record");
        }
        index
    }

    fn semantic_record(hash: &str, embedding: &[i8]) -> IndexRecord {
        IndexRecord {
            schema_version: INDEX_SCHEMA_VERSION,
            canonical_hash: hash.into(),
            perceptual: None,
            embedding: Some(embedding.to_vec()),
            metadata: json!({ "hash": hash }),
        }
    }

    fn perceptual_record(hash: &str, fingerprint: &[u64]) -> IndexRecord {
        IndexRecord {
            schema_version: INDEX_SCHEMA_VERSION,
            canonical_hash: hash.into(),
            perceptual: Some(fingerprint.to_vec()),
            embedding: None,
            metadata: json!({ "hash": hash }),
        }
    }
}
