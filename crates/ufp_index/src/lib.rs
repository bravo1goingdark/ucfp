#![doc = include_str!("../README.md")]

mod backend;
mod query;

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

use ndarray::Array1;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use zstd::{decode_all, encode_all};

/// Bump this value whenever the on-disk `IndexRecord` layout changes.
pub const INDEX_SCHEMA_VERSION: u16 = 1;

/// Quantized embedding type (compact float representation)
pub type QuantizedVec = Vec<i8>;

/// Unified index record for any modality
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct IndexRecord {
    #[serde(default = "default_schema_version")]
    pub schema_version: u16,
    pub canonical_hash: String,
    pub perceptual: Option<Vec<u64>>,
    pub embedding: Option<QuantizedVec>,
    #[serde(with = "metadata_serde")]
    pub metadata: serde_json::Value,
}

const fn default_schema_version() -> u16 {
    INDEX_SCHEMA_VERSION
}

/// Compression codec options
#[derive(Clone, Debug, Default)]
pub enum CompressionCodec {
    None,
    #[default]
    Zstd,
}

/// Compression behavior configuration
#[derive(Clone, Debug)]
pub struct CompressionConfig {
    pub codec: CompressionCodec,
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

/// Quantization strategies for embeddings
#[derive(Clone, Debug)]
pub enum QuantizationConfig {
    Int8 { scale: f32 },
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

/// Config for initializing the index
#[derive(Clone, Debug, Default)]
pub struct IndexConfig {
    pub backend: BackendConfig,
    pub compression: CompressionConfig,
    pub quantization: QuantizationConfig,
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
}

/// Custom error type
#[derive(Error, Debug)]
pub enum IndexError {
    #[error("Backend error: {0}")]
    Backend(String),
    #[error("Serialization error: {0}")]
    Serde(#[from] bincode::Error),
    #[error("Compression error: {0}")]
    Zstd(#[from] std::io::Error),
}

impl IndexError {
    pub fn backend<E: std::fmt::Display>(err: E) -> Self {
        Self::Backend(err.to_string())
    }
}

/// Index structure
pub struct UfpIndex {
    backend: Box<dyn IndexBackend>,
    cfg: IndexConfig,
}

impl UfpIndex {
    /// Initialize or open an index using the configured backend.
    pub fn new(cfg: IndexConfig) -> Result<Self, IndexError> {
        let backend = cfg.backend.build()?;
        Ok(Self::with_backend(cfg, backend))
    }

    /// Build an index with a custom backend (e.g., in-memory for tests).
    pub fn with_backend(cfg: IndexConfig, backend: Box<dyn IndexBackend>) -> Self {
        Self { backend, cfg }
    }

    /// Quantize float embeddings -> i8 using a raw scale.
    pub fn quantize(vec: &Array1<f32>, scale: f32) -> QuantizedVec {
        vec.iter()
            .map(|&v| (v * scale).clamp(-128.0, 127.0) as i8)
            .collect()
    }

    /// Quantize using a configured strategy.
    pub fn quantize_with_strategy(vec: &Array1<f32>, cfg: &QuantizationConfig) -> QuantizedVec {
        Self::quantize(vec, cfg.scale())
    }

    /// Insert or update a record.
    pub fn upsert(&self, rec: &IndexRecord) -> Result<(), IndexError> {
        let payload = self.encode_record(rec)?;
        self.backend.put(&rec.canonical_hash, &payload)
    }

    /// Remove a record from the index.
    pub fn delete(&self, hash: &str) -> Result<(), IndexError> {
        self.backend.delete(hash)
    }

    /// Flush backend buffers if supported.
    pub fn flush(&self) -> Result<(), IndexError> {
        self.backend.flush()
    }

    /// Retrieve a record by hash.
    pub fn get(&self, hash: &str) -> Result<Option<IndexRecord>, IndexError> {
        if let Some(data) = self.backend.get(hash)? {
            let record = self.decode_record(&data)?;
            Ok(Some(record))
        } else {
            Ok(None)
        }
    }

    /// Batch insert multiple records (efficient for large datasets)
    pub fn batch_insert(&self, records: &[IndexRecord]) -> Result<(), IndexError> {
        let mut entries = Vec::with_capacity(records.len());
        for rec in records {
            entries.push((rec.canonical_hash.clone(), self.encode_record(rec)?));
        }
        self.backend.batch_put(entries)
    }

    pub(crate) fn decode_record(&self, data: &[u8]) -> Result<IndexRecord, IndexError> {
        let decompressed = self.cfg.compression.decompress(data)?;
        Ok(bincode::deserialize(&decompressed)?)
    }

    fn encode_record(&self, rec: &IndexRecord) -> Result<Vec<u8>, IndexError> {
        let encoded = bincode::serialize(rec)?;
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
