use serde_json::json;
use index::{
    BackendConfig, IndexConfig, IndexRecord, QueryMode, UfpIndex, INDEX_SCHEMA_VERSION,
};

fn main() -> anyhow::Result<()> {
    std::fs::create_dir_all("data")?;

    let cfg = IndexConfig::new().with_backend(BackendConfig::rocksdb("data/index"));

    let index = UfpIndex::new(cfg)?;

    // Seed the index with two records that share semantic and perceptual traits.
    let records = vec![
        IndexRecord {
            schema_version: INDEX_SCHEMA_VERSION,
            canonical_hash: "doc-1".into(),
            perceptual: Some(vec![111, 222, 333, 444]),
            embedding: Some(vec![10, -3, 7, 5]),
            metadata: json!({"source": "guide.md"}),
        },
        IndexRecord {
            schema_version: INDEX_SCHEMA_VERSION,
            canonical_hash: "doc-2".into(),
            perceptual: Some(vec![333, 444, 555]),
            embedding: Some(vec![8, -2, 6, 4]),
            metadata: json!({"source": "spec.md"}),
        },
    ];

    for rec in &records {
        index.upsert(rec)?;
    }
    println!("Inserted {} records.", records.len());

    // Construct a query embedding + MinHash vector.
    let query = IndexRecord {
        schema_version: INDEX_SCHEMA_VERSION,
        canonical_hash: "query".into(),
        perceptual: Some(vec![333, 444, 777]),
        embedding: Some(vec![9, -2, 6, 5]),
        metadata: json!({}),
    };

    let semantic_hits = index.search(&query, QueryMode::Semantic, 2)?;
    let perceptual_hits = index.search(&query, QueryMode::Perceptual, 2)?;

    println!("Semantic hits: {semantic_hits:#?}");
    println!("Perceptual hits: {perceptual_hits:#?}");

    if let Some(stored) = index.get("doc-1")? {
        println!("Re-fetched doc-1: {:?}", stored.metadata);
    }

    Ok(())
}
