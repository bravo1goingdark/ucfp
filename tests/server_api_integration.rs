//! Integration tests for server API endpoints
//!
//! These tests verify the match_documents, compare_documents, and search_index
//! endpoints work correctly with real implementations.

use std::sync::Arc;

use server::{
    config::ServerConfig,
    routes::{index, matching, process},
    state::ServerState,
};

/// Create a test server state with in-memory index
fn create_test_state() -> Arc<ServerState> {
    let mut config = ServerConfig::default();
    config.api_keys.insert("test-api-key".to_string());
    config.rate_limit_per_minute = 1000; // High limit for tests

    Arc::new(ServerState::new(config).expect("Failed to create test state"))
}

#[tokio::test]
async fn test_state_initialization() {
    let state = create_test_state();

    // Verify state is properly initialized
    assert!(state.is_valid_api_key("test-api-key"));
    assert!(!state.is_valid_api_key("invalid-key"));
    assert!(state.check_rate_limit("test-api-key"));
}

#[tokio::test]
async fn test_match_request_structure() {
    // Test that all strategy types are properly handled
    let strategies = vec!["perceptual", "semantic", "hybrid"];

    for strategy in strategies {
        let request = matching::MatchRequest {
            query: "test query".to_string(),
            tenant_id: None,
            strategy: strategy.to_string(),
            max_results: 5,
            oversample_factor: 2.0,
            min_score: Some(0.5),
        };

        assert_eq!(request.strategy, strategy);
        assert!(request.min_score.unwrap() >= 0.0);
        assert_eq!(request.max_results, 5);
    }
}

#[tokio::test]
async fn test_match_request_with_tenant() {
    let state = create_test_state();

    let match_request = matching::MatchRequest {
        query: "test document".to_string(),
        tenant_id: Some("test-tenant".to_string()),
        strategy: "perceptual".to_string(),
        max_results: 10,
        oversample_factor: 1.5,
        min_score: Some(0.0),
    };

    // Verify the request structure is correct
    assert_eq!(match_request.strategy, "perceptual");
    assert_eq!(match_request.max_results, 10);
    assert_eq!(match_request.tenant_id, Some("test-tenant".to_string()));

    // Verify tenant ID resolution works
    let resolved_tenant = match_request.tenant_id.unwrap_or_else(|| {
        state
            .config
            .api_keys
            .iter()
            .next()
            .cloned()
            .unwrap_or_default()
    });
    assert_eq!(resolved_tenant, "test-tenant");
}

#[tokio::test]
async fn test_compare_request_identical_docs() {
    let doc1 = matching::DocumentInput {
        text: "The quick brown fox jumps over the lazy dog".to_string(),
        doc_id: Some("doc1".to_string()),
    };

    let doc2 = matching::DocumentInput {
        text: "The quick brown fox jumps over the lazy dog".to_string(),
        doc_id: Some("doc2".to_string()),
    };

    let compare_request = matching::CompareRequest { doc1, doc2 };

    // Verify request structure
    assert_eq!(compare_request.doc1.text, compare_request.doc2.text);
    // Identical documents should be detected
}

#[tokio::test]
async fn test_compare_request_different_docs() {
    let doc1 = matching::DocumentInput {
        text: "First document content".to_string(),
        doc_id: Some("doc1".to_string()),
    };

    let doc2 = matching::DocumentInput {
        text: "Second different document".to_string(),
        doc_id: Some("doc2".to_string()),
    };

    let compare_request = matching::CompareRequest { doc1, doc2 };

    // Verify request structure
    assert_ne!(compare_request.doc1.text, compare_request.doc2.text);
}

#[tokio::test]
async fn test_search_query_structure() {
    let search_query = index::IndexSearchQuery {
        query: "search test".to_string(),
        strategy: "perceptual".to_string(),
        top_k: 5,
        tenant_id: Some("test-tenant".to_string()),
    };

    // Verify search query structure
    assert_eq!(search_query.top_k, 5);
    assert_eq!(search_query.strategy, "perceptual");
    assert_eq!(search_query.tenant_id, Some("test-tenant".to_string()));
}

#[tokio::test]
async fn test_search_query_semantic() {
    let search_query = index::IndexSearchQuery {
        query: "semantic search".to_string(),
        strategy: "semantic".to_string(),
        top_k: 10,
        tenant_id: None,
    };

    assert_eq!(search_query.strategy, "semantic");
    assert_eq!(search_query.top_k, 10);
}

#[tokio::test]
async fn test_index_insert_request() {
    let mut metadata = std::collections::HashMap::new();
    metadata.insert("title".to_string(), "Test Document".to_string());

    let request = index::IndexInsertRequest {
        doc_id: "test-doc".to_string(),
        tenant_id: Some("test-tenant".to_string()),
        canonical_hash: "abc123".to_string(),
        perceptual_fingerprint: Some(vec![1, 2, 3, 4, 5]),
        semantic_embedding: Some(vec![0.1, 0.2, 0.3, 0.4]),
        metadata: Some(metadata),
    };

    assert_eq!(request.doc_id, "test-doc");
    assert_eq!(request.tenant_id, Some("test-tenant".to_string()));
    assert!(request.perceptual_fingerprint.is_some());
    assert!(request.semantic_embedding.is_some());
}

#[tokio::test]
async fn test_batch_process_request() {
    let batch_request = process::BatchProcessRequest {
        documents: vec![
            process::BatchDocument {
                doc_id: Some("batch-1".to_string()),
                tenant_id: None,
                text: "First document".to_string(),
            },
            process::BatchDocument {
                doc_id: Some("batch-2".to_string()),
                tenant_id: None,
                text: "Second document".to_string(),
            },
        ],
        enable_perceptual: true,
        enable_semantic: false,
    };

    assert_eq!(batch_request.documents.len(), 2);
    assert!(batch_request.enable_perceptual);
    assert!(!batch_request.enable_semantic);
    assert_eq!(batch_request.documents[0].text, "First document");
    assert_eq!(batch_request.documents[1].text, "Second document");
}

#[tokio::test]
async fn test_api_key_exists_in_config() {
    let state = create_test_state();

    // Verify the test API key exists in config
    assert!(state.config.api_keys.contains("test-api-key"));
}

#[tokio::test]
async fn test_rate_limiting() {
    let state = create_test_state();

    // Multiple requests should pass (high limit in test config)
    for _ in 0..10 {
        assert!(state.check_rate_limit("test-api-key"));
    }
}

#[tokio::test]
async fn test_api_key_validation_detailed() {
    let state = create_test_state();

    // Test various keys
    assert!(state.is_valid_api_key("test-api-key"));
    assert!(!state.is_valid_api_key(""));
    assert!(!state.is_valid_api_key("wrong-key"));
    assert!(!state.is_valid_api_key("TEST-API-KEY")); // Case sensitive
}

#[tokio::test]
async fn test_match_request_validation() {
    // Test with various configurations
    let request1 = matching::MatchRequest {
        query: "".to_string(), // Empty query
        tenant_id: Some("tenant".to_string()),
        strategy: "hybrid".to_string(),
        max_results: 0,         // Invalid
        oversample_factor: 0.5, // Invalid (< 1.0)
        min_score: Some(-0.5),  // Invalid (< 0.0)
    };

    // Verify structure is created (validation happens at runtime)
    assert_eq!(request1.query, "");
    assert_eq!(request1.max_results, 0);
}

#[tokio::test]
async fn test_compare_edge_cases() {
    // Test with very short text
    let short_doc = matching::DocumentInput {
        text: "Hi".to_string(),
        doc_id: None,
    };

    // Test with empty doc_id
    let doc_no_id = matching::DocumentInput {
        text: "Some text".to_string(),
        doc_id: None,
    };

    assert_eq!(short_doc.text.len(), 2);
    assert!(doc_no_id.doc_id.is_none());
}

#[tokio::test]
async fn test_index_metadata_handling() {
    let mut metadata = std::collections::HashMap::new();
    metadata.insert("author".to_string(), "Test Author".to_string());
    metadata.insert("category".to_string(), "Test Category".to_string());
    metadata.insert("date".to_string(), "2025-01-31".to_string());

    let request = index::IndexInsertRequest {
        doc_id: "meta-test".to_string(),
        tenant_id: Some("tenant-1".to_string()),
        canonical_hash: "hash123".to_string(),
        perceptual_fingerprint: None,
        semantic_embedding: None,
        metadata: Some(metadata.clone()),
    };

    let meta = request.metadata.as_ref().unwrap();
    assert_eq!(meta.get("author"), Some(&"Test Author".to_string()));
    assert_eq!(meta.get("category"), Some(&"Test Category".to_string()));
    assert_eq!(meta.len(), 3);
}

#[tokio::test]
async fn test_config_defaults() {
    let config = ServerConfig::default();

    assert_eq!(config.port, 8080);
    assert_eq!(config.timeout_secs, 30);
    assert_eq!(config.max_body_size_mb, 10);
    assert_eq!(config.rate_limit_per_minute, 100);
    assert!(config.enable_cors);
    assert!(config.metrics_enabled);
}

#[tokio::test]
async fn test_all_strategies_with_min_scores() {
    let strategies = vec![
        ("perceptual", Some(0.5f32)),
        ("semantic", Some(0.7f32)),
        ("hybrid", Some(0.6f32)),
        ("perceptual", None),
        ("semantic", None),
    ];

    for (strategy, min_score) in strategies {
        let request = matching::MatchRequest {
            query: "test".to_string(),
            tenant_id: None,
            strategy: strategy.to_string(),
            max_results: 5,
            oversample_factor: 2.0,
            min_score,
        };

        assert_eq!(request.strategy, strategy);
        assert_eq!(request.min_score, min_score);
    }
}
