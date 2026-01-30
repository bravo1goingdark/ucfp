use serde::{Deserialize, Serialize};

/// Configuration for the canonical text pipeline.
///
/// `version` is a monotonically increasing schema version for the
/// canonical layer. Any behavior change that can affect canonical text,
/// tokenization, or canonical hashes must be accompanied by a new
/// configuration version.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CanonicalizeConfig {
    /// Semantic version of the canonicalization configuration.
    pub version: u32,
    /// If true, apply Unicode NFKC normalization before other transforms.
    pub normalize_unicode: bool,
    /// If true, strip punctuation characters before tokenizing.
    pub strip_punctuation: bool,
    /// If true, lowercase the text.
    pub lowercase: bool,
}

impl Default for CanonicalizeConfig {
    fn default() -> Self {
        Self {
            version: 1,
            normalize_unicode: true,
            strip_punctuation: false,
            lowercase: true,
        }
    }
}
