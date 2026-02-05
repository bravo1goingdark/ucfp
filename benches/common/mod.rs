//! Common utilities for UCFP benchmarks
//!
//! This module provides shared helper functions for generating test data,
//! setting up configurations, and common benchmark patterns.

#![allow(dead_code)]

use index::{BackendConfig, IndexConfig, IndexRecord, UfpIndex, INDEX_SCHEMA_VERSION};
use serde_json::json;
use std::path::PathBuf;

/// Sample texts of various lengths for benchmarking
pub const SAMPLE_SHORT_TEXT: &str = "The quick brown fox jumps over the lazy dog.";

pub const SAMPLE_MEDIUM_TEXT: &str = "Rust is a systems programming language that runs blazingly fast, \
prevents segfaults, and guarantees thread safety. It features zero-cost abstractions, move semantics, \
guaranteed memory safety, threads without data races, trait-based generics, pattern matching, type \
inference, minimal runtime, and efficient C bindings. The language empowers developers to build \
reliable and efficient software.";

pub const SAMPLE_LONG_TEXT: &str = include_str!("../fixtures/sample_long_text.txt");

/// Generate a corpus of random words
pub fn generate_word_corpus(word_count: usize) -> Vec<String> {
    let words = vec![
        "the",
        "quick",
        "brown",
        "fox",
        "jumps",
        "over",
        "lazy",
        "dog",
        "rust",
        "programming",
        "language",
        "memory",
        "safety",
        "performance",
        "async",
        "await",
        "future",
        "tokio",
        "concurrent",
        "parallel",
        "embedding",
        "vector",
        "semantic",
        "search",
        "similarity",
        "hash",
        "fingerprint",
        "perceptual",
        "canonical",
        "normalize",
        "index",
        "query",
        "match",
        "document",
        "content",
        "text",
    ];

    let mut corpus = Vec::with_capacity(word_count);
    for i in 0..word_count {
        corpus.push(words[i % words.len()].to_string());
    }
    corpus
}

/// Generate a text of approximately N words
pub fn generate_text_word_count(word_count: usize) -> String {
    let words = generate_word_corpus(word_count);
    words.join(" ")
}

/// Generate a text of approximately N bytes
pub fn generate_text_byte_count(byte_count: usize) -> String {
    let mut text = String::with_capacity(byte_count);
    let sentence = "The quick brown fox jumps over the lazy dog. ";
    while text.len() < byte_count {
        text.push_str(sentence);
    }
    text.truncate(byte_count);
    text
}

/// Create sample index records for bulk operations
pub fn create_sample_records(count: usize) -> Vec<IndexRecord> {
    (0..count)
        .map(|i| IndexRecord {
            schema_version: INDEX_SCHEMA_VERSION,
            canonical_hash: format!("hash-{}", i),
            perceptual: Some(vec![i as u64, (i + 1) as u64, (i + 2) as u64]),
            embedding: Some(vec![
                (i % 128) as i8,
                ((i + 1) % 128) as i8,
                ((i + 2) % 128) as i8,
            ]),
            metadata: json!({"id": i, "tenant": "bench"}),
        })
        .collect()
}

/// Setup an in-memory index for benchmarks
pub fn setup_in_memory_index() -> UfpIndex {
    let config = IndexConfig::new().with_backend(BackendConfig::in_memory());
    UfpIndex::new(config).expect("Failed to create in-memory index")
}

/// Setup a Redb-backed index for benchmarks (uses temp file)
pub fn setup_redb_index() -> (UfpIndex, tempfile::TempDir) {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp directory");
    let db_path = temp_dir.path().join("bench.redb");
    let config =
        IndexConfig::new().with_backend(BackendConfig::redb(db_path.to_string_lossy().to_string()));
    let index = UfpIndex::new(config).expect("Failed to create Redb index");
    (index, temp_dir)
}

/// Get semantic model path for benchmarks
pub fn get_model_path() -> Option<PathBuf> {
    let model_path = PathBuf::from("models/bge-small-en-v1.5/onnx/model.onnx");
    if model_path.exists() {
        Some(model_path)
    } else {
        None
    }
}

/// Get tokenizer path for benchmarks
pub fn get_tokenizer_path() -> Option<PathBuf> {
    let tokenizer_path = PathBuf::from("models/bge-small-en-v1.5/tokenizer.json");
    if tokenizer_path.exists() {
        Some(tokenizer_path)
    } else {
        None
    }
}

/// Check if ONNX models are available
pub fn models_available() -> bool {
    get_model_path().is_some() && get_tokenizer_path().is_some()
}

/// Get sample texts for different length categories
pub fn get_sample_text(length: TextLength) -> &'static str {
    match length {
        TextLength::Short => SAMPLE_SHORT_TEXT,
        TextLength::Medium => SAMPLE_MEDIUM_TEXT,
        TextLength::Long => SAMPLE_LONG_TEXT,
    }
}

/// Text length categories for benchmarks
#[derive(Clone, Copy, Debug)]
pub enum TextLength {
    Short,  // ~50 chars
    Medium, // ~500 chars
    Long,   // ~5000 chars
}

/// Benchmark iteration counts for different test sizes
pub const ITERATIONS_SMALL: usize = 1000;
pub const ITERATIONS_MEDIUM: usize = 100;
pub const ITERATIONS_LARGE: usize = 10;
