//! Examples for using the UCFP Server API

use reqwest::Client;
use serde_json::json;

const SERVER_URL: &str = "http://localhost:8080";
const API_KEY: &str = "demo-key-12345";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = Client::new();

    // Example 1: Health check
    println!("1. Health Check:");
    let resp = client.get(format!("{SERVER_URL}/health")).send().await?;
    println!("Status: {}", resp.status());
    println!("Body: {}", resp.text().await?);
    println!();

    // Example 2: Process a single document
    println!("2. Process Single Document:");
    let resp = client
        .post(format!("{SERVER_URL}/api/v1/process"))
        .header("X-API-Key", API_KEY)
        .json(&json!({
            "doc_id": "example-001",
            "tenant_id": "tenant-001",
            "text": "This is a sample document for content fingerprinting.",
            "enable_perceptual": true,
            "enable_semantic": true
        }))
        .send()
        .await?;
    println!("Status: {}", resp.status());
    println!("Body: {}", resp.text().await?);
    println!();

    // Example 3: Batch process documents
    println!("3. Batch Process Documents:");
    let resp = client
        .post(format!("{SERVER_URL}/api/v1/batch"))
        .header("X-API-Key", API_KEY)
        .json(&json!({
            "documents": [
                {
                    "doc_id": "batch-001",
                    "tenant_id": "tenant-001",
                    "text": "First document in batch."
                },
                {
                    "doc_id": "batch-002",
                    "tenant_id": "tenant-001",
                    "text": "Second document in batch."
                },
                {
                    "doc_id": "batch-003",
                    "tenant_id": "tenant-002",
                    "text": "Third document with different tenant."
                }
            ],
            "enable_perceptual": true,
            "enable_semantic": false
        }))
        .send()
        .await?;
    println!("Status: {}", resp.status());
    println!("Body: {}", resp.text().await?);
    println!();

    // Example 4: Insert into index
    println!("4. Insert into Index:");
    let resp = client
        .post(format!("{SERVER_URL}/api/v1/index/insert"))
        .header("X-API-Key", API_KEY)
        .json(&json!({
            "doc_id": "indexed-001",
            "tenant_id": "tenant-001",
            "canonical_hash": "abcdef1234567890",
            "perceptual_fingerprint": [12345, 67890, 11111, 22222],
            "semantic_embedding": [0.1, 0.2, 0.3, 0.4],
            "metadata": {
                "source": "api",
                "category": "test"
            }
        }))
        .send()
        .await?;
    println!("Status: {}", resp.status());
    println!("Body: {}", resp.text().await?);
    println!();

    // Example 5: Search index
    println!("5. Search Index:");
    let resp = client
        .get(format!("{SERVER_URL}/api/v1/index/search"))
        .header("X-API-Key", API_KEY)
        .query(&[
            ("query", "sample document"),
            ("strategy", "perceptual"),
            ("top_k", "10"),
        ])
        .send()
        .await?;
    println!("Status: {}", resp.status());
    println!("Body: {}", resp.text().await?);
    println!();

    // Example 6: Match documents
    println!("6. Match Documents:");
    let resp = client
        .post(format!("{SERVER_URL}/api/v1/match"))
        .header("X-API-Key", API_KEY)
        .json(&json!({
            "query": "Find documents similar to this query text",
            "tenant_id": "tenant-001",
            "strategy": "hybrid",
            "max_results": 5,
            "min_score": 0.8
        }))
        .send()
        .await?;
    println!("Status: {}", resp.status());
    println!("Body: {}", resp.text().await?);
    println!();

    // Example 7: Compare two documents
    println!("7. Compare Two Documents:");
    let resp = client
        .post(format!("{SERVER_URL}/api/v1/compare"))
        .header("X-API-Key", API_KEY)
        .json(&json!({
            "doc1": {
                "text": "This is the first document to compare."
            },
            "doc2": {
                "text": "This is the second document to compare."
            }
        }))
        .send()
        .await?;
    println!("Status: {}", resp.status());
    println!("Body: {}", resp.text().await?);
    println!();

    // Example 8: Server metadata
    println!("8. Server Metadata:");
    let resp = client
        .get(format!("{SERVER_URL}/api/v1/metadata"))
        .header("X-API-Key", API_KEY)
        .send()
        .await?;
    println!("Status: {}", resp.status());
    println!("Body: {}", resp.text().await?);
    println!();

    // Example 9: Metrics
    println!("9. Prometheus Metrics:");
    let resp = client.get(format!("{SERVER_URL}/metrics")).send().await?;
    println!("Status: {}", resp.status());
    println!("Body: {}", resp.text().await?);
    println!();

    println!("All examples completed!");
    Ok(())
}
