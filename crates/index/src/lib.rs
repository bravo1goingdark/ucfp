//! # UCFP Index
//!
//! This crate provides a backend-agnostic index for storing and searching
//! Universal Content Fingerprinting (UCFP) records. It is designed to handle
//! canonical hashes, perceptual fingerprints, and semantic embeddings, offering
//! a unified interface for persistence and retrieval.
//!
//! ## Core Features
//!
//! - **Pluggable Backends**: Supports multiple storage backends through a common
//!   [`IndexBackend`] trait. Out of the box, it provides:
//!   - An in-memory `HashMap`-based backend for fast, ephemeral storage (ideal for testing).
//!   - A RocksDB backend for persistent, on-disk storage (enabled via the `backend-rocksdb` feature).
//! - **Flexible Configuration**: All behaviors, including the choice of backend,
//!   compression, and quantization strategies, are configured at runtime via the
//!   [`IndexConfig`] struct.
//! - **Efficient Storage**:
//!   - **Quantization**: Provides utilities to quantize `f32` embeddings into `i8` vectors
//!     to reduce storage space and improve query performance. Use the `quantize` or
//!     `quantize_with_strategy` methods before creating `IndexRecord` instances.//!   - **Compression**: Compresses serialized records (using Zstd by default) before
//!     writing to the backend.
//! - **Similarity Search**: Provides search capabilities for both semantic and
//!   perceptual fingerprints:
//!   - **Semantic Search**: Computes cosine similarity on quantized embeddings.
//!   - **Perceptual Search**: Computes Jaccard similarity on MinHash signatures.
//!
//! ## Key Concepts
//!
//! The central struct is [`UfpIndex`], which provides a high-level API for
//! interacting with the index. It handles the details of serialization,
//! compression, and quantization, allowing callers to work with the simple
//! [`IndexRecord`] struct.
//!
//! The [`IndexBackend`] trait abstracts the underlying storage mechanism, making
//! it easy to swap out backends or implement custom ones.
//!
//! ## Example Usage
//!
//! ```
//! use index::{UfpIndex, IndexConfig, BackendConfig, IndexRecord, QueryMode, INDEX_SCHEMA_VERSION};
//! use serde_json::json;
//!
//! // Configure an in-memory index
//! let config = IndexConfig::new().with_backend(BackendConfig::in_memory());
//! let index = UfpIndex::new(config).unwrap();
//!
//! // Create and insert a record
//! let record = IndexRecord {
//!     schema_version: INDEX_SCHEMA_VERSION,
//!     canonical_hash: "doc-1".to_string(),
//!     perceptual: Some(vec![1, 2, 3]),
//!     embedding: Some(vec![10, 20, 30]),
//!     metadata: json!({ "title": "My Document" }),
//! };
//! index.upsert(&record).unwrap();
//!
//! // Search for similar records
//! let query_record = IndexRecord {
//!     schema_version: INDEX_SCHEMA_VERSION,
//!     canonical_hash: "query-1".to_string(),
//!     perceptual: Some(vec![1, 2, 4]),
//!     embedding: Some(vec![11, 22, 33]),
//!     metadata: json!({}),
//! };
//!
//! let results = index.search(&query_record, QueryMode::Perceptual, 10).unwrap();
//! // assert_eq!(results.len(), 1);
//! // assert_eq!(results[0].canonical_hash, "doc-1");
//! ```

pub mod ann;
mod backend;
mod query;

use crate::ann::AnnConfig;
use std::sync::RwLock;

mod metadata_serde {
    use serde::de::Error as DeError;
    use serde::ser::Error as SerError;
    use serde::{Deserialize, Deserializer, Serializer};
    use serde_json::Value;

    pub(super) fn serialize<S>(value: &Value, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let bytes = serde_json::to_vec(value).map_err(SerError::custom)?;
        serializer.serialize_bytes(&bytes)
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        let bytes = Vec::<u8>::deserialize(deserializer)?;
        serde_json::from_slice(&bytes).map_err(DeError::custom)
    }
}

#[cfg(feature = "backend-rocksdb")]
pub use backend::RocksDbBackend;
pub use backend::{BackendConfig, InMemoryBackend, IndexBackend};
pub use query::{QueryMode, QueryResult};

use bincode::config::standard;
use bincode::error::{DecodeError, EncodeError};
use bincode::serde::{decode_from_slice, encode_to_vec};
use ndarray::Array1;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use zstd::{decode_all, encode_all};

/// Bump this value whenever the on-disk `IndexRecord` layout changes.
pub const INDEX_SCHEMA_VERSION: u16 = 1;

/// Quantized embedding type (compact float representation)
pub type QuantizedVec = Vec<i8>;

/// Unified index record for any modality
/// Unified index record for any modality.
///
/// This struct represents a document in the index with its canonical hash,
/// perceptual fingerprints (MinHash), semantic embedding (quantized), and metadata.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct IndexRecord {
    /// Schema version for backward compatibility when deserializing.
    #[serde(default = "default_schema_version")]
    pub schema_version: u16,
    /// Canonical hash (SHA-256 hex) that uniquely identifies the document.
    pub canonical_hash: String,
    /// Perceptual fingerprint (MinHash signature) for similarity search.
    pub perceptual: Option<Vec<u64>>,
    /// Quantized semantic embedding for vector similarity search.
    pub embedding: Option<QuantizedVec>,
    /// Arbitrary metadata associated with the document (JSON).
    #[serde(with = "metadata_serde")]
    pub metadata: serde_json::Value,
}

const fn default_schema_version() -> u16 {
    INDEX_SCHEMA_VERSION
}

/// Compression codec options for index storage.
#[derive(Clone, Debug, Default)]
pub enum CompressionCodec {
    /// No compression (useful for debugging or when storage is not a concern).
    None,
    /// Zstd compression (default, good balance of speed and ratio).
    #[default]
    Zstd,
}

/// Compression behavior configuration.
#[derive(Clone, Debug)]
pub struct CompressionConfig {
    /// The compression codec to use (None or Zstd).
    pub codec: CompressionCodec,
    /// Compression level (1-22 for Zstd, where higher = better compression but slower).
    pub level: i32,
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            codec: CompressionCodec::default(),
            level: 3,
        }
    }
}

impl CompressionConfig {
    pub fn new(codec: CompressionCodec, level: i32) -> Self {
        Self { codec, level }
    }

    pub fn with_codec(mut self, codec: CompressionCodec) -> Self {
        self.codec = codec;
        self
    }

    pub fn with_level(mut self, level: i32) -> Self {
        self.level = level;
        self
    }

    fn compress(&self, data: &[u8]) -> Result<Vec<u8>, IndexError> {
        match self.codec {
            CompressionCodec::None => Ok(data.to_vec()),
            CompressionCodec::Zstd => Ok(encode_all(data, self.level)?),
        }
    }

    fn decompress(&self, data: &[u8]) -> Result<Vec<u8>, IndexError> {
        match self.codec {
            CompressionCodec::None => Ok(data.to_vec()),
            CompressionCodec::Zstd => Ok(decode_all(data)?),
        }
    }
}

/// Quantization strategies for embeddings.
///
/// Quantization reduces memory usage by converting f32 embeddings to smaller types.
#[derive(Clone, Debug)]
pub enum QuantizationConfig {
    /// 8-bit integer quantization using a linear scale factor.
    ///
    /// Values are computed as: `quantized = (value * scale).clamp(-128.0, 127.0) as i8`
    Int8 {
        /// The scaling factor for quantization (default: 100.0).
        scale: f32,
    },
}

impl Default for QuantizationConfig {
    fn default() -> Self {
        QuantizationConfig::Int8 { scale: 100.0 }
    }
}

impl QuantizationConfig {
    pub fn scale(&self) -> f32 {
        match self {
            QuantizationConfig::Int8 { scale } => *scale,
        }
    }

    pub fn with_scale(mut self, scale: f32) -> Self {
        match &mut self {
            QuantizationConfig::Int8 { scale: existing } => *existing = scale,
        }
        self
    }
}

/// Config for initializing the index.
#[derive(Clone, Debug, Default)]
pub struct IndexConfig {
    /// Backend storage configuration (in-memory or RocksDB).
    pub backend: BackendConfig,
    /// Compression settings for stored records.
    pub compression: CompressionConfig,
    /// Quantization settings for embeddings.
    pub quantization: QuantizationConfig,
    /// ANN (Approximate Nearest Neighbor) configuration for semantic search.
    /// When enabled, uses HNSW algorithm for sub-linear search on large datasets.
    pub ann: AnnConfig,
}

impl IndexConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_backend(mut self, backend: BackendConfig) -> Self {
        self.backend = backend;
        self
    }

    pub fn with_compression(mut self, compression: CompressionConfig) -> Self {
        self.compression = compression;
        self
    }

    pub fn with_quantization(mut self, quantization: QuantizationConfig) -> Self {
        self.quantization = quantization;
        self
    }

    pub fn with_ann(mut self, ann: AnnConfig) -> Self {
        self.ann = ann;
        self
    }
}

/// Custom error type
#[derive(Error, Debug, Clone)]
pub enum IndexError {
    #[error("Backend error: {0}")]
    Backend(String),
    #[error("Serialization encode error: {0}")]
    Encode(String),
    #[error("Serialization decode error: {0}")]
    Decode(String),
    #[error("Compression error: {0}")]
    Zstd(String),
}

impl From<EncodeError> for IndexError {
    fn from(e: EncodeError) -> Self {
        IndexError::Encode(e.to_string())
    }
}

impl From<DecodeError> for IndexError {
    fn from(e: DecodeError) -> Self {
        IndexError::Decode(e.to_string())
    }
}

impl From<std::io::Error> for IndexError {
    fn from(e: std::io::Error) -> Self {
        IndexError::Zstd(e.to_string())
    }
}

impl IndexError {
    pub fn backend<E: std::fmt::Display>(err: E) -> Self {
        Self::Backend(err.to_string())
    }
}

/// Index structure
pub struct UfpIndex {
    /// The backend used for storage, abstracted behind a trait.
    backend: Box<dyn IndexBackend>,
    /// The configuration for index.
    cfg: IndexConfig,
    /// Inverted index for MinHash signatures to enable O(log n) lookup instead of O(n) scan
    perceptual_index: RwLock<hashbrown::HashMap<u64, Vec<String>>>,
    /// Simple vector index for semantic embeddings to reduce full scans
    semantic_index: RwLock<Vec<(String, QuantizedVec)>>,
}

impl UfpIndex {
    /// Initialize or open an index using the configured backend.
    /// This will build the backend from the config.
    pub fn new(cfg: IndexConfig) -> Result<Self, IndexError> {
        let backend = cfg.backend.build()?;
        Ok(Self::with_backend(cfg, backend))
    }

    /// Build an index with a custom backend (e.g., in-memory for tests).
    /// This is useful for dependency injection and testing.
    pub fn with_backend(cfg: IndexConfig, backend: Box<dyn IndexBackend>) -> Self {
        Self {
            backend,
            cfg,
            perceptual_index: RwLock::new(hashbrown::HashMap::new()),
            semantic_index: RwLock::new(Vec::new()),
        }
    }

    /// Quantize float embeddings -> i8 using a raw scale.
    /// This is a simple linear quantization with clamping.
    pub fn quantize(vec: &Array1<f32>, scale: f32) -> QuantizedVec {
        let mut out = Vec::with_capacity(vec.len());
        out.extend(vec.iter().map(|&v| (v * scale).clamp(-128.0, 127.0) as i8));
        out
    }

    /// Quantize using a configured strategy.
    /// This allows for different quantization strategies to be used in the future.
    pub fn quantize_with_strategy(vec: &Array1<f32>, cfg: &QuantizationConfig) -> QuantizedVec {
        Self::quantize(vec, cfg.scale())
    }

    /// Insert or update a record.
    /// The record is encoded and compressed before being sent to the backend.
    pub fn upsert(&self, rec: &IndexRecord) -> Result<(), IndexError> {
        let payload = self.encode_record(rec)?;

        // Update auxiliary indexes for faster queries
        if let Some(ref perceptual) = rec.perceptual {
            let mut p_index = self.perceptual_index.write().unwrap();
            for &hash_val in perceptual {
                p_index
                    .entry(hash_val)
                    .or_insert_with(Vec::new)
                    .push(rec.canonical_hash.clone());
            }
        }

        if let Some(ref embedding) = rec.embedding {
            let mut s_index = self.semantic_index.write().unwrap();
            s_index.push((rec.canonical_hash.clone(), embedding.clone()));
        }

        self.backend.put(&rec.canonical_hash, &payload)
    }

    /// Remove a record from the index.
    pub fn delete(&self, hash: &str) -> Result<(), IndexError> {
        self.backend.delete(hash)
    }

    /// Flush backend buffers if supported.
    /// This is useful for ensuring data is written to disk.
    pub fn flush(&self) -> Result<(), IndexError> {
        self.backend.flush()
    }

    /// Retrieve a record by hash.
    /// The record is decompressed and decoded after being retrieved from the backend.
    pub fn get(&self, hash: &str) -> Result<Option<IndexRecord>, IndexError> {
        if let Some(data) = self.backend.get(hash)? {
            let record = self.decode_record(&data)?;
            Ok(Some(record))
        } else {
            Ok(None)
        }
    }

    /// Scan all records in the index.
    /// Iterates over all records in the backend, decodes them, and passes them to the visitor.
    pub fn scan(
        &self,
        visitor: &mut dyn FnMut(&IndexRecord) -> Result<(), IndexError>,
    ) -> Result<(), IndexError> {
        self.backend.scan(&mut |data: &[u8]| {
            let record = self.decode_record(data)?;
            visitor(&record)
        })
    }

    /// Batch insert multiple records (efficient for large datasets).
    /// This can be much faster than calling `upsert` in a loop.
    pub fn batch_insert(&self, records: &[IndexRecord]) -> Result<(), IndexError> {
        let mut entries = Vec::with_capacity(records.len());
        let mut perceptual_updates: Vec<(u64, &str)> = Vec::new();
        let mut semantic_updates: Vec<(&str, &QuantizedVec)> = Vec::new();

        for rec in records {
            entries.push((rec.canonical_hash.clone(), self.encode_record(rec)?));
            if let Some(ref perceptual) = rec.perceptual {
                for &hash_val in perceptual {
                    perceptual_updates.push((hash_val, rec.canonical_hash.as_str()));
                }
            }
            if let Some(ref embedding) = rec.embedding {
                semantic_updates.push((rec.canonical_hash.as_str(), embedding));
            }
        }

        if !perceptual_updates.is_empty() {
            let mut p_index = self.perceptual_index.write().unwrap();
            for (hash_val, canonical_hash) in perceptual_updates {
                p_index
                    .entry(hash_val)
                    .or_insert_with(Vec::new)
                    .push(canonical_hash.to_string());
            }
        }

        if !semantic_updates.is_empty() {
            let mut s_index = self.semantic_index.write().unwrap();
            s_index.reserve(semantic_updates.len());
            for (canonical_hash, embedding) in semantic_updates {
                s_index.push((canonical_hash.to_string(), embedding.clone()));
            }
        }

        self.backend.batch_put(entries)
    }

    /// Decodes and decompresses a record from the backend.
    pub(crate) fn decode_record(&self, data: &[u8]) -> Result<IndexRecord, IndexError> {
        let decompressed = self.cfg.compression.decompress(data)?;
        let (record, _) = decode_from_slice(&decompressed, standard())?;
        Ok(record)
    }

    /// Encodes and compresses a record for storage in the backend.
    fn encode_record(&self, rec: &IndexRecord) -> Result<Vec<u8>, IndexError> {
        let encoded = encode_to_vec(rec, standard())?;
        self.cfg.compression.compress(&encoded)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn test_config() -> IndexConfig {
        IndexConfig::new().with_backend(BackendConfig::InMemory)
    }

    fn sample_record(hash: &str, embedding: Vec<i8>, perceptual: Vec<u64>) -> IndexRecord {
        IndexRecord {
            schema_version: INDEX_SCHEMA_VERSION,
            canonical_hash: hash.to_string(),
            perceptual: Some(perceptual),
            embedding: Some(embedding),
            metadata: json!({ "source": hash }),
        }
    }

    #[test]
    fn in_memory_backend_roundtrip() {
        let backend = Box::new(InMemoryBackend::new());
        let index = UfpIndex::with_backend(test_config(), backend);

        let rec = sample_record("doc-a", vec![1, 2, 3], vec![10, 20, 30]);
        index.upsert(&rec).expect("upsert succeeds");

        let fetched = index.get("doc-a").expect("get ok").expect("record exists");
        assert_eq!(fetched.canonical_hash, "doc-a");
        assert_eq!(fetched.metadata, rec.metadata);
    }

    #[test]
    fn search_uses_backend_scan() {
        let backend = Box::new(InMemoryBackend::new());
        let index = UfpIndex::with_backend(test_config(), backend);

        let records = vec![
            sample_record("doc-a", vec![10, 0], vec![1, 2, 3]),
            sample_record("doc-b", vec![9, 0], vec![3, 4, 5]),
        ];
        for rec in &records {
            index.upsert(rec).unwrap();
        }

        let query = IndexRecord {
            schema_version: INDEX_SCHEMA_VERSION,
            canonical_hash: "query".into(),
            perceptual: Some(vec![3, 5]),
            embedding: Some(vec![10, 0]),
            metadata: json!({}),
        };

        let semantic = index
            .search(&query, QueryMode::Semantic, 2)
            .expect("semantic search");
        assert_eq!(semantic.len(), 2);
        assert_eq!(semantic[0].canonical_hash, "doc-a");

        let perceptual = index
            .search(&query, QueryMode::Perceptual, 2)
            .expect("perceptual search");
        assert_eq!(perceptual[0].canonical_hash, "doc-b");
    }
}
