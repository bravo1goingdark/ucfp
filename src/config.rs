//! YAML Configuration File Support for UCFP
//!
//! This module provides support for loading UCFP pipeline configurations from YAML files.
//! It allows users to define all stage configurations (ingest, canonical, perceptual, semantic, index, match)
//! in a single YAML file and load them at runtime.
//!
//! ## Example YAML Configuration
//!
//! ```yaml
//! # UCFP Pipeline Configuration
//! version: "1.0"
//!
//! ingest:
//!   version: 1
//!   default_tenant_id: "default"
//!   doc_id_namespace: "6ba7b810-9dad-11d1-80b4-00c04fd430c8"
//!   strip_control_chars: true
//!
//! canonical:
//!   version: 1
//!   normalize_unicode: true
//!   lowercase: true
//!   strip_punctuation: false
//!
//! perceptual:
//!   version: 1
//!   k: 9
//!   w: 4
//!   minhash_bands: 16
//!   minhash_rows_per_band: 8
//!   seed: 1732584193
//!   use_parallel: false
//!   include_intermediates: true
//!
//! semantic:
//!   tier: "balanced"
//!   mode: "fast"
//!   model_name: "bge-small-en-v1.5"
//!   normalize: true
//!
//! index:
//!   backend: "in_memory"
//!   compression: "zstd"
//!   quantization: "i8"
//!
//! matcher:
//!   version: 1
//!   max_results: 10
//!   tenant_enforce: true
//!   mode: "semantic"
//! ```

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that can occur when loading YAML configuration files
#[derive(Debug, Error)]
pub enum ConfigLoadError {
    #[error("failed to read config file: {0}")]
    FileRead(#[from] std::io::Error),

    #[error("failed to parse YAML: {0}")]
    YamlParse(#[from] serde_yaml::Error),

    #[error("validation error: {0}")]
    Validation(String),

    #[error("unsupported config version: {0}")]
    UnsupportedVersion(String),

    #[error("missing required field: {0}")]
    MissingField(String),
}

/// Top-level YAML configuration structure for the entire UCFP pipeline
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct UcfpConfig {
    /// Configuration format version
    pub version: String,

    /// Optional configuration name/description
    #[serde(default)]
    pub name: Option<String>,

    /// Ingest stage configuration
    #[serde(default)]
    pub ingest: IngestYamlConfig,

    /// Canonicalization stage configuration
    #[serde(default)]
    pub canonical: CanonicalYamlConfig,

    /// Perceptual fingerprinting configuration
    #[serde(default)]
    pub perceptual: PerceptualYamlConfig,

    /// Semantic embedding configuration
    #[serde(default)]
    pub semantic: SemanticYamlConfig,

    /// Index configuration
    #[serde(default)]
    pub index: IndexYamlConfig,

    /// Matcher configuration
    #[serde(default)]
    pub matcher: MatchYamlConfig,

    /// Optional environment variable overrides
    #[serde(default)]
    pub env_overrides: HashMap<String, String>,
}

impl UcfpConfig {
    /// Load a YAML configuration file from the given path
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, ConfigLoadError> {
        let content = fs::read_to_string(path)?;
        Self::from_yaml(&content)
    }

    /// Parse YAML configuration from a string
    pub fn from_yaml(yaml: &str) -> Result<Self, ConfigLoadError> {
        let config: UcfpConfig = serde_yaml::from_str(yaml)?;
        config.validate()?;
        Ok(config)
    }

    /// Validate the configuration
    fn validate(&self) -> Result<(), ConfigLoadError> {
        // Check version
        match self.version.as_str() {
            "1.0" | "1" => Ok(()),
            v => Err(ConfigLoadError::UnsupportedVersion(v.to_string())),
        }?;

        // Validate individual stage configs
        self.ingest.validate()?;
        self.canonical.validate()?;
        self.perceptual.validate()?;
        self.semantic.validate()?;
        self.index.validate()?;
        self.matcher.validate()?;

        Ok(())
    }
}

impl Default for UcfpConfig {
    fn default() -> Self {
        Self {
            version: "1.0".to_string(),
            name: None,
            ingest: IngestYamlConfig::default(),
            canonical: CanonicalYamlConfig::default(),
            perceptual: PerceptualYamlConfig::default(),
            semantic: SemanticYamlConfig::default(),
            index: IndexYamlConfig::default(),
            matcher: MatchYamlConfig::default(),
            env_overrides: HashMap::new(),
        }
    }
}

/// Ingest stage YAML configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestYamlConfig {
    #[serde(default = "default_version")]
    pub version: u32,

    #[serde(default = "default_tenant_id")]
    pub default_tenant_id: String,

    #[serde(default = "default_doc_id_namespace")]
    pub doc_id_namespace: String,

    #[serde(default = "true_value")]
    pub strip_control_chars: bool,

    #[serde(default)]
    pub max_payload_bytes: Option<usize>,

    #[serde(default)]
    pub max_normalized_bytes: Option<usize>,

    /// Maximum attribute size in bytes (stored in metadata_policy)
    #[serde(default)]
    pub max_attribute_bytes: Option<usize>,

    /// Required fields (stored in metadata_policy.required_fields)
    #[serde(default)]
    pub required_fields: Vec<String>,
}

impl IngestYamlConfig {
    fn validate(&self) -> Result<(), ConfigLoadError> {
        if self.version == 0 {
            return Err(ConfigLoadError::Validation(
                "ingest.version must be >= 1".to_string(),
            ));
        }
        Ok(())
    }
}

impl Default for IngestYamlConfig {
    fn default() -> Self {
        Self {
            version: 1,
            default_tenant_id: "default".to_string(),
            doc_id_namespace: "6ba7b810-9dad-11d1-80b4-00c04fd430c8".to_string(),
            strip_control_chars: true,
            max_payload_bytes: Some(10 * 1024 * 1024), // 10MB
            max_normalized_bytes: Some(100 * 1024),    // 100KB
            max_attribute_bytes: Some(10 * 1024),      // 10KB
            required_fields: vec![],
        }
    }
}

/// Canonicalization stage YAML configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalYamlConfig {
    #[serde(default = "default_version")]
    pub version: u32,

    #[serde(default = "true_value")]
    pub normalize_unicode: bool,

    #[serde(default = "true_value")]
    pub lowercase: bool,

    #[serde(default)]
    pub strip_punctuation: bool,
}

impl CanonicalYamlConfig {
    fn validate(&self) -> Result<(), ConfigLoadError> {
        if self.version == 0 {
            return Err(ConfigLoadError::Validation(
                "canonical.version must be >= 1".to_string(),
            ));
        }
        Ok(())
    }
}

impl Default for CanonicalYamlConfig {
    fn default() -> Self {
        Self {
            version: 1,
            normalize_unicode: true,
            lowercase: true,
            strip_punctuation: false,
        }
    }
}

/// Perceptual fingerprinting YAML configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerceptualYamlConfig {
    #[serde(default = "default_version")]
    pub version: u32,

    #[serde(default = "default_k")]
    pub k: usize,

    #[serde(default = "default_w")]
    pub w: usize,

    #[serde(default = "default_minhash_bands")]
    pub minhash_bands: usize,

    #[serde(default = "default_minhash_rows")]
    pub minhash_rows_per_band: usize,

    #[serde(default = "default_seed")]
    pub seed: u64,

    #[serde(default)]
    pub use_parallel: bool,

    #[serde(default = "true_value")]
    pub include_intermediates: bool,
}

impl PerceptualYamlConfig {
    fn validate(&self) -> Result<(), ConfigLoadError> {
        if self.version == 0 {
            return Err(ConfigLoadError::Validation(
                "perceptual.version must be >= 1".to_string(),
            ));
        }
        if self.k == 0 {
            return Err(ConfigLoadError::Validation(
                "perceptual.k must be >= 1".to_string(),
            ));
        }
        if self.w == 0 {
            return Err(ConfigLoadError::Validation(
                "perceptual.w must be >= 1".to_string(),
            ));
        }
        if self.minhash_bands == 0 {
            return Err(ConfigLoadError::Validation(
                "perceptual.minhash_bands must be >= 1".to_string(),
            ));
        }
        if self.minhash_rows_per_band == 0 {
            return Err(ConfigLoadError::Validation(
                "perceptual.minhash_rows_per_band must be >= 1".to_string(),
            ));
        }
        Ok(())
    }
}

impl Default for PerceptualYamlConfig {
    fn default() -> Self {
        Self {
            version: 1,
            k: 9,
            w: 4,
            minhash_bands: 16,
            minhash_rows_per_band: 8,
            seed: 0xF00D_BAAD_F00D_BAAD,
            use_parallel: false,
            include_intermediates: true,
        }
    }
}

/// Semantic embedding YAML configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticYamlConfig {
    #[serde(default = "default_tier")]
    pub tier: String,

    #[serde(default = "default_mode")]
    pub mode: String,

    #[serde(default = "default_model_name")]
    pub model_name: String,

    #[serde(default)]
    pub model_path: Option<String>,

    #[serde(default)]
    pub model_url: Option<String>,

    #[serde(default)]
    pub api_url: Option<String>,

    #[serde(default)]
    pub api_auth_header: Option<String>,

    #[serde(default)]
    pub api_provider: Option<String>,

    #[serde(default = "default_timeout")]
    pub api_timeout_secs: Option<u64>,

    #[serde(default)]
    pub tokenizer_path: Option<String>,

    #[serde(default)]
    pub tokenizer_url: Option<String>,

    #[serde(default = "true_value")]
    pub normalize: bool,

    #[serde(default = "default_device")]
    pub device: String,
}

impl SemanticYamlConfig {
    fn validate(&self) -> Result<(), ConfigLoadError> {
        let valid_tiers = ["fast", "balanced", "accurate"];
        if !valid_tiers.contains(&self.tier.as_str()) {
            return Err(ConfigLoadError::Validation(format!(
                "semantic.tier must be one of: {valid_tiers:?}"
            )));
        }

        let valid_modes = ["fast", "onnx", "api"];
        if !valid_modes.contains(&self.mode.as_str()) {
            return Err(ConfigLoadError::Validation(format!(
                "semantic.mode must be one of: {valid_modes:?}"
            )));
        }

        Ok(())
    }
}

impl Default for SemanticYamlConfig {
    fn default() -> Self {
        Self {
            tier: "balanced".to_string(),
            mode: "fast".to_string(),
            model_name: "bge-small-en-v1.5".to_string(),
            model_path: None,
            model_url: None,
            api_url: None,
            api_auth_header: None,
            api_provider: None,
            api_timeout_secs: Some(30),
            tokenizer_path: None,
            tokenizer_url: None,
            normalize: true,
            device: "cpu".to_string(),
        }
    }
}

/// Index YAML configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexYamlConfig {
    #[serde(default = "default_backend")]
    pub backend: String,

    #[serde(default)]
    pub rocksdb_path: Option<String>,

    #[serde(default = "default_compression")]
    pub compression: String,

    #[serde(default = "default_quantization")]
    pub quantization: String,
}

impl IndexYamlConfig {
    fn validate(&self) -> Result<(), ConfigLoadError> {
        let valid_backends = ["in_memory", "rocksdb"];
        if !valid_backends.contains(&self.backend.as_str()) {
            return Err(ConfigLoadError::Validation(format!(
                "index.backend must be one of: {valid_backends:?}"
            )));
        }

        if self.backend == "rocksdb" && self.rocksdb_path.is_none() {
            return Err(ConfigLoadError::Validation(
                "index.rocksdb_path is required when backend is 'rocksdb'".to_string(),
            ));
        }

        Ok(())
    }
}

impl Default for IndexYamlConfig {
    fn default() -> Self {
        Self {
            backend: "in_memory".to_string(),
            rocksdb_path: None,
            compression: "zstd".to_string(),
            quantization: "i8".to_string(),
        }
    }
}

/// Matcher YAML configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchYamlConfig {
    #[serde(default = "default_version")]
    pub version: u32,

    #[serde(default = "default_policy_id")]
    pub policy_id: String,

    #[serde(default = "default_policy_version")]
    pub policy_version: String,

    #[serde(default = "default_match_mode")]
    pub mode: String,

    #[serde(default = "default_strategy")]
    pub strategy: String,

    #[serde(default = "default_max_results")]
    pub max_results: usize,

    #[serde(default = "true_value")]
    pub tenant_enforce: bool,

    #[serde(default = "default_oversample")]
    pub oversample_factor: f32,

    #[serde(default)]
    pub explain: bool,
}

impl MatchYamlConfig {
    fn validate(&self) -> Result<(), ConfigLoadError> {
        if self.version == 0 {
            return Err(ConfigLoadError::Validation(
                "matcher.version must be >= 1".to_string(),
            ));
        }
        if self.max_results == 0 {
            return Err(ConfigLoadError::Validation(
                "matcher.max_results must be >= 1".to_string(),
            ));
        }
        if self.oversample_factor < 1.0 {
            return Err(ConfigLoadError::Validation(
                "matcher.oversample_factor must be >= 1.0".to_string(),
            ));
        }
        Ok(())
    }
}

impl Default for MatchYamlConfig {
    fn default() -> Self {
        Self {
            version: 1,
            policy_id: "default-policy".to_string(),
            policy_version: "v1".to_string(),
            mode: "semantic".to_string(),
            strategy: "weighted".to_string(),
            max_results: 10,
            tenant_enforce: true,
            oversample_factor: 2.0,
            explain: false,
        }
    }
}

// Helper functions for serde defaults
fn default_version() -> u32 {
    1
}
fn default_tenant_id() -> String {
    "default".to_string()
}
fn default_doc_id_namespace() -> String {
    "6ba7b810-9dad-11d1-80b4-00c04fd430c8".to_string()
}
fn true_value() -> bool {
    true
}
fn default_k() -> usize {
    9
}
fn default_w() -> usize {
    4
}
fn default_minhash_bands() -> usize {
    16
}
fn default_minhash_rows() -> usize {
    8
}
fn default_seed() -> u64 {
    0xF00D_BAAD_F00D_BAAD
}
fn default_tier() -> String {
    "balanced".to_string()
}
fn default_mode() -> String {
    "fast".to_string()
}
fn default_model_name() -> String {
    "bge-small-en-v1.5".to_string()
}
fn default_timeout() -> Option<u64> {
    Some(30)
}
fn default_device() -> String {
    "cpu".to_string()
}
fn default_backend() -> String {
    "in_memory".to_string()
}
fn default_compression() -> String {
    "zstd".to_string()
}
fn default_quantization() -> String {
    "i8".to_string()
}
fn default_policy_id() -> String {
    "default-policy".to_string()
}
fn default_policy_version() -> String {
    "v1".to_string()
}
fn default_match_mode() -> String {
    "semantic".to_string()
}
fn default_strategy() -> String {
    "weighted".to_string()
}
fn default_max_results() -> usize {
    10
}
fn default_oversample() -> f32 {
    2.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_load_valid_yaml() {
        let yaml = r#"
version: "1.0"
name: "test config"
ingest:
  version: 1
  default_tenant_id: "test-tenant"
canonical:
  version: 1
  lowercase: true
"#;

        let config = UcfpConfig::from_yaml(yaml).unwrap();
        assert_eq!(config.version, "1.0");
        assert_eq!(config.name, Some("test config".to_string()));
        assert_eq!(config.ingest.default_tenant_id, "test-tenant");
        assert!(config.canonical.lowercase);
    }

    #[test]
    fn test_load_from_file() {
        let yaml = r#"
version: "1.0"
ingest:
  version: 1
canonical:
  version: 1
"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(yaml.as_bytes()).unwrap();

        let config = UcfpConfig::from_file(temp_file.path()).unwrap();
        assert_eq!(config.version, "1.0");
    }

    #[test]
    fn test_default_config() {
        let config = UcfpConfig::default();
        assert_eq!(config.version, "1.0");
        assert!(config.name.is_none());
    }

    #[test]
    fn test_perceptual_validation() {
        let yaml = r#"
version: "1.0"
perceptual:
  version: 1
  k: 0
"#;

        let result = UcfpConfig::from_yaml(yaml);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("k must be >= 1"));
    }

    #[test]
    fn test_semantic_validation() {
        let yaml = r#"
version: "1.0"
semantic:
  tier: "invalid"
"#;

        let result = UcfpConfig::from_yaml(yaml);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("tier"));
    }

    #[test]
    fn test_full_yaml_roundtrip() {
        let yaml = r#"
version: "1.0"
name: "production"
ingest:
  version: 1
  default_tenant_id: "acme"
  strip_control_chars: true
  max_payload_bytes: 10485760
  
canonical:
  version: 1
  normalize_unicode: true
  lowercase: true
  strip_punctuation: false
  
perceptual:
  version: 1
  k: 9
  w: 4
  minhash_bands: 16
  minhash_rows_per_band: 8
  seed: 1732584193
  use_parallel: false
  include_intermediates: true
  
semantic:
  tier: "balanced"
  mode: "fast"
  model_name: "bge-small-en-v1.5"
  normalize: true
  device: "cpu"
  
index:
  backend: "in_memory"
  compression: "zstd"
  quantization: "i8"
  
matcher:
  version: 1
  max_results: 10
  tenant_enforce: true
  mode: "semantic"
"#;

        let config = UcfpConfig::from_yaml(yaml).unwrap();

        // Verify all stages
        assert_eq!(config.ingest.default_tenant_id, "acme");
        assert!(config.canonical.normalize_unicode);
        assert_eq!(config.perceptual.k, 9);
        assert_eq!(config.semantic.tier, "balanced");
        assert_eq!(config.index.backend, "in_memory");
        assert_eq!(config.matcher.max_results, 10);
    }
}
