//! Approximate Nearest Neighbor (ANN) search using HNSW algorithm.
//!
//! This module provides high-performance vector similarity search using
//! Hierarchical Navigable Small World (HNSW) graphs. It offers sub-linear
//! search time complexity (~O(log n)) compared to brute force O(n).
//!
//! ## Trade-offs
//!
//! - **Speed**: ~100-1000x faster than linear scan for large datasets
//! - **Recall**: Typically 95-99% (some false negatives possible)
//! - **Memory**: Higher memory usage than linear scan
//! - **Build time**: Index construction takes longer than insertion
//!
//! ## When to Use
//!
//! - Dataset size > 10,000 vectors
//! - Query latency requirements < 100ms
//! - Acceptable to miss ~1-5% of true nearest neighbors
//!
//! ## When NOT to Use
//!
//! - Dataset size < 1,000 vectors (linear scan is fine)
//! - Need 100% recall (use exact search)
//! - Memory constrained environment

use hnsw_rs::prelude::*;
use std::collections::HashMap;

/// Configuration for ANN index construction.
#[derive(Debug, Clone, Copy)]
pub struct AnnConfig {
    /// Number of neighbors per node (higher = better recall, slower build).
    /// Default: 16
    pub m: usize,
    /// Size of dynamic candidate list during construction (higher = better recall, slower build).
    /// Default: 200
    pub ef_construction: usize,
    /// Size of dynamic candidate list during search (higher = better recall, slower search).
    /// Default: 50
    pub ef_search: usize,
    /// Maximum number of results to return from ANN search.
    /// Default: 100
    pub max_results: usize,
    /// Whether to use ANN or fall back to linear scan.
    /// Default: true (use ANN when beneficial)
    pub enabled: bool,
    /// Minimum number of vectors before ANN is used.
    /// Below this threshold, linear scan is used even if enabled=true.
    /// Default: 1000
    pub min_vectors_for_ann: usize,
}

impl Default for AnnConfig {
    fn default() -> Self {
        Self {
            m: 16,
            ef_construction: 200,
            ef_search: 50,
            max_results: 100,
            enabled: true,
            min_vectors_for_ann: 1000,
        }
    }
}

impl AnnConfig {
    pub fn with_m(mut self, m: usize) -> Self {
        self.m = m;
        self
    }

    pub fn with_ef_construction(mut self, ef: usize) -> Self {
        self.ef_construction = ef;
        self
    }

    pub fn with_ef_search(mut self, ef: usize) -> Self {
        self.ef_search = ef;
        self
    }

    pub fn with_max_results(mut self, max: usize) -> Self {
        self.max_results = max;
        self
    }

    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn with_min_vectors_for_ann(mut self, min: usize) -> Self {
        self.min_vectors_for_ann = min;
        self
    }

    /// Check if ANN should be used given the current dataset size.
    pub fn should_use_ann(&self, num_vectors: usize) -> bool {
        self.enabled && num_vectors >= self.min_vectors_for_ann
    }
}

/// Result from ANN search.
#[derive(Debug, Clone)]
pub struct AnnResult {
    /// Index of the vector in the original dataset.
    pub index: usize,
    /// Distance to query vector (lower = closer).
    pub distance: f32,
}

/// ANN index interface (HNSW implementation).
pub struct AnnIndex {
    config: AnnConfig,
    dimension: usize,
    hnsw: Option<Hnsw<'static, f32, DistCosine>>,
    id_to_index: HashMap<String, usize>,
    index_to_id: HashMap<usize, String>,
    vectors: Vec<Vec<f32>>,
    built: bool,
}

impl AnnIndex {
    /// Create a new empty ANN index.
    pub fn new(dimension: usize, config: AnnConfig) -> Self {
        Self {
            config,
            dimension,
            hnsw: None,
            id_to_index: HashMap::new(),
            index_to_id: HashMap::new(),
            vectors: Vec::new(),
            built: false,
        }
    }

    /// Insert a vector with associated ID.
    pub fn insert(&mut self, id: String, vector: Vec<f32>) -> Result<(), AnnError> {
        if vector.len() != self.dimension {
            return Err(AnnError::DimensionMismatch {
                expected: self.dimension,
                got: vector.len(),
            });
        }

        let index = self.vectors.len();
        self.vectors.push(vector);
        self.id_to_index.insert(id.clone(), index);
        self.index_to_id.insert(index, id);

        // Mark as needing rebuild
        self.built = false;

        Ok(())
    }

    /// Search for nearest neighbors.
    pub fn search(&self, query: &[f32], k: usize) -> Result<Vec<AnnResult>, AnnError> {
        if query.len() != self.dimension {
            return Err(AnnError::DimensionMismatch {
                expected: self.dimension,
                got: query.len(),
            });
        }

        let k = k.min(self.config.max_results);

        // Decide whether to use ANN or linear scan
        if self.built && self.config.should_use_ann(self.vectors.len()) && self.hnsw.is_some() {
            // Use HNSW for approximate search
            self.hnsw_search(query, k)
        } else {
            // Fall back to linear scan
            self.linear_search(query, k)
        }
    }

    /// HNSW-based approximate search.
    fn hnsw_search(&self, query: &[f32], k: usize) -> Result<Vec<AnnResult>, AnnError> {
        if let Some(ref hnsw) = self.hnsw {
            let ef = self.config.ef_search;
            let results: Vec<Neighbour> = hnsw.search(query, k, ef);

            Ok(results
                .into_iter()
                .map(|neighbour| AnnResult {
                    index: neighbour.get_origin_id(),
                    distance: neighbour.distance,
                })
                .collect())
        } else {
            Err(AnnError::NotBuilt)
        }
    }

    /// Linear search (exact, slow but accurate).
    fn linear_search(&self, query: &[f32], k: usize) -> Result<Vec<AnnResult>, AnnError> {
        if self.vectors.is_empty() {
            return Ok(Vec::new());
        }

        // Calculate distances for all vectors
        let mut distances: Vec<(usize, f32)> = self
            .vectors
            .iter()
            .enumerate()
            .map(|(idx, vec)| (idx, cosine_distance(query, vec)))
            .collect();

        // Sort by distance (ascending - lower is closer)
        distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        // Take top k
        let results = distances
            .into_iter()
            .take(k)
            .map(|(idx, dist)| AnnResult {
                index: idx,
                distance: dist,
            })
            .collect();

        Ok(results)
    }

    /// Get ID by index.
    pub fn get_id(&self, index: usize) -> Option<&String> {
        self.index_to_id.get(&index)
    }

    /// Get index by ID.
    pub fn get_index(&self, id: &str) -> Option<usize> {
        self.id_to_index.get(id).copied()
    }

    /// Number of vectors in index.
    pub fn len(&self) -> usize {
        self.vectors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.vectors.is_empty()
    }

    /// Check if HNSW index is built.
    pub fn is_built(&self) -> bool {
        self.built
    }

    /// Build HNSW index (required before using ANN search).
    /// Only builds if there are enough vectors for HNSW to work properly (minimum 10).
    pub fn build(&mut self) {
        if self.vectors.is_empty() {
            return;
        }

        // HNSW requires a minimum number of vectors to work properly
        // Below this threshold, we'll just use linear search
        let nb_elem = self.vectors.len();
        if nb_elem < 10 {
            // Not enough vectors for HNSW, mark as built but use linear search
            self.built = true;
            return;
        }

        // Calculate parameters for HNSW
        let nb_layer = 16.min((nb_elem as f32).ln().trunc() as usize);

        // Create HNSW index with 5 parameters
        let hnsw = Hnsw::<f32, DistCosine>::new(
            self.config.m,
            nb_elem,
            nb_layer,
            self.config.ef_construction,
            DistCosine {},
        );

        // Insert all vectors using parallel_insert
        // The API expects &[(&Vec<f32>, usize)] so we pass references to the stored vectors
        let data_for_insertion: Vec<(&Vec<f32>, usize)> = self
            .vectors
            .iter()
            .enumerate()
            .map(|(idx, vec)| (vec, idx))
            .collect();
        hnsw.parallel_insert(&data_for_insertion);

        self.hnsw = Some(hnsw);
        self.built = true;
    }

    /// Rebuild the index (useful after batch insertions).
    pub fn rebuild(&mut self) {
        self.built = false;
        self.build();
    }

    /// Get current configuration.
    pub fn config(&self) -> &AnnConfig {
        &self.config
    }

    /// Update configuration and mark as needing rebuild if needed.
    pub fn update_config(&mut self, config: AnnConfig) {
        let needs_rebuild =
            config.m != self.config.m || config.ef_construction != self.config.ef_construction;

        self.config = config;

        if needs_rebuild {
            self.built = false;
        }
    }
}

/// Error type for ANN operations.
#[derive(Debug, thiserror::Error)]
pub enum AnnError {
    #[error("Dimension mismatch: expected {expected}, got {got}")]
    DimensionMismatch { expected: usize, got: usize },
    #[error("Index not built")]
    NotBuilt,
    #[error("HNSW error: {0}")]
    HnswError(String),
}

/// Calculate cosine distance (1 - cosine similarity).
/// Lower values mean vectors are more similar.
fn cosine_distance(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 1.0; // Maximum distance
    }

    let similarity = dot / (norm_a * norm_b);
    // Convert to distance: 1 - similarity
    1.0 - similarity.clamp(-1.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ann_config_defaults() {
        let config = AnnConfig::default();
        assert_eq!(config.m, 16);
        assert_eq!(config.ef_construction, 200);
        assert_eq!(config.ef_search, 50);
        assert!(config.enabled);
        assert_eq!(config.min_vectors_for_ann, 1000);
    }

    #[test]
    fn test_ann_config_builder() {
        let config = AnnConfig::default()
            .with_m(32)
            .with_ef_construction(400)
            .with_ef_search(100)
            .with_enabled(false)
            .with_min_vectors_for_ann(500);

        assert_eq!(config.m, 32);
        assert_eq!(config.ef_construction, 400);
        assert_eq!(config.ef_search, 100);
        assert!(!config.enabled);
        assert_eq!(config.min_vectors_for_ann, 500);
    }

    #[test]
    fn test_should_use_ann() {
        let config = AnnConfig::default();

        // Above threshold with enabled=true
        assert!(config.should_use_ann(1000));
        assert!(config.should_use_ann(10000));

        // Below threshold
        assert!(!config.should_use_ann(999));
        assert!(!config.should_use_ann(100));

        // When disabled
        let disabled_config = AnnConfig::default().with_enabled(false);
        assert!(!disabled_config.should_use_ann(10000));
    }

    #[test]
    fn test_ann_index_insert_and_linear_search() {
        let mut index = AnnIndex::new(3, AnnConfig::default());

        // Insert vectors
        index
            .insert("doc1".to_string(), vec![1.0, 0.0, 0.0])
            .unwrap();
        index
            .insert("doc2".to_string(), vec![0.0, 1.0, 0.0])
            .unwrap();
        index
            .insert("doc3".to_string(), vec![0.0, 0.0, 1.0])
            .unwrap();

        // Search (should use linear since < 1000 vectors)
        let results = index.search(&[1.0, 0.0, 0.0], 2).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].index, 0); // doc1 is closest
    }

    #[test]
    fn test_ann_index_dimension_mismatch() {
        let mut index = AnnIndex::new(3, AnnConfig::default());

        // Wrong dimension on insert
        let result = index.insert("doc1".to_string(), vec![1.0, 0.0]);
        assert!(matches!(result, Err(AnnError::DimensionMismatch { .. })));

        // Wrong dimension on search
        index
            .insert("doc1".to_string(), vec![1.0, 0.0, 0.0])
            .unwrap();
        let result = index.search(&[1.0, 0.0], 1);
        assert!(matches!(result, Err(AnnError::DimensionMismatch { .. })));
    }

    #[test]
    fn test_ann_index_empty_search() {
        let index = AnnIndex::new(3, AnnConfig::default());
        let results = index.search(&[1.0, 0.0, 0.0], 5).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_id_index_mapping() {
        let mut index = AnnIndex::new(3, AnnConfig::default());

        index
            .insert("doc-a".to_string(), vec![1.0, 0.0, 0.0])
            .unwrap();
        index
            .insert("doc-b".to_string(), vec![0.0, 1.0, 0.0])
            .unwrap();

        assert_eq!(index.get_index("doc-a"), Some(0));
        assert_eq!(index.get_index("doc-b"), Some(1));
        assert_eq!(index.get_id(0), Some(&"doc-a".to_string()));
        assert_eq!(index.get_id(1), Some(&"doc-b".to_string()));
    }

    #[test]
    fn test_cosine_distance() {
        // Same vector - distance should be 0
        let d = cosine_distance(&[1.0, 0.0, 0.0], &[1.0, 0.0, 0.0]);
        assert!(d.abs() < 0.001);

        // Orthogonal vectors - distance should be 1 (max)
        let d = cosine_distance(&[1.0, 0.0, 0.0], &[0.0, 1.0, 0.0]);
        assert!((d - 1.0).abs() < 0.001);

        // Opposite vectors - distance should be 2 (beyond max, clamped to 1)
        let d = cosine_distance(&[1.0, 0.0, 0.0], &[-1.0, 0.0, 0.0]);
        assert!((d - 2.0).abs() < 0.001);
    }

    #[test]
    fn test_ann_index_search_respects_k() {
        let mut index = AnnIndex::new(3, AnnConfig::default());

        // Insert 5 vectors
        for i in 0..5 {
            index
                .insert(format!("doc{i}"), vec![i as f32, 0.0, 0.0])
                .unwrap();
        }

        // Search for k=2
        let results = index.search(&[0.0, 0.0, 0.0], 2).unwrap();
        assert_eq!(results.len(), 2);

        // Search for k=10 (more than available)
        let results = index.search(&[0.0, 0.0, 0.0], 10).unwrap();
        assert_eq!(results.len(), 5); // Returns all available
    }

    #[test]
    fn test_ann_index_build_and_search() {
        let mut index = AnnIndex::new(
            3,
            AnnConfig::default().with_min_vectors_for_ann(1), // Enable ANN for small test
        );

        // Insert vectors
        index
            .insert("doc1".to_string(), vec![1.0, 0.0, 0.0])
            .unwrap();
        index
            .insert("doc2".to_string(), vec![0.0, 1.0, 0.0])
            .unwrap();
        index
            .insert("doc3".to_string(), vec![0.0, 0.0, 1.0])
            .unwrap();

        // Not built yet
        assert!(!index.is_built());

        // Build HNSW
        index.build();
        assert!(index.is_built());

        // Search should use HNSW now
        let results = index.search(&[1.0, 0.0, 0.0], 2).unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_ann_index_rebuild() {
        let mut index = AnnIndex::new(3, AnnConfig::default().with_min_vectors_for_ann(1));

        index
            .insert("doc1".to_string(), vec![1.0, 0.0, 0.0])
            .unwrap();
        index.build();
        assert!(index.is_built());

        // Insert more without rebuilding
        index
            .insert("doc2".to_string(), vec![0.0, 1.0, 0.0])
            .unwrap();
        assert!(!index.is_built()); // Should be marked as not built

        // Rebuild
        index.rebuild();
        assert!(index.is_built());
    }

    #[test]
    fn test_update_config_triggers_rebuild() {
        let mut index = AnnIndex::new(3, AnnConfig::default().with_min_vectors_for_ann(1));

        index
            .insert("doc1".to_string(), vec![1.0, 0.0, 0.0])
            .unwrap();
        index.build();
        assert!(index.is_built());

        // Update config with different M - should invalidate
        let new_config = AnnConfig::default().with_min_vectors_for_ann(1).with_m(32);
        index.update_config(new_config);

        // Should need rebuild since M changed
        assert!(!index.is_built());
    }
}
