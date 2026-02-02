use std::{env, error::Error, path::PathBuf};

use semantic::{semanticize, SemanticConfig, SemanticEmbedding};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args().skip(1);
    let doc_id = args.next().unwrap_or_else(|| "example-doc".into());
    let text = args
        .next()
        .unwrap_or_else(|| "Hello world from semantic.".into());

    let mut cfg = SemanticConfig {
        tier: "balanced".into(),
        ..SemanticConfig::default()
    };

    match locate_model_assets() {
        Some((model_path, tokenizer_path)) => {
            cfg.model_path = model_path;
            cfg.tokenizer_path = Some(tokenizer_path);
            println!(
                "Running balanced tier with ONNX model at {}",
                cfg.model_path.display()
            );
        }
        None => {
            cfg.tier = "fast".into();
            println!("ONNX assets not found, falling back to deterministic stub tier");
        }
    }

    let embedding: SemanticEmbedding = semanticize(&doc_id, &text, &cfg).await?;
    println!("doc_id: {}", embedding.doc_id);
    println!("tier: {}", embedding.tier);
    println!("dim: {}", embedding.embedding_dim);
    println!(
        "first values: {:?}",
        &embedding.vector[..embedding.vector.len().min(8)]
    );
    println!("normalized: {}", embedding.normalized);

    Ok(())
}

fn locate_model_assets() -> Option<(PathBuf, PathBuf)> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir.parent()?.parent()?;

    let model_dir = workspace_root.join("models").join("bge-small-en-v1.5");
    let model_path = model_dir.join("onnx").join("model.onnx");
    let tokenizer_path = model_dir.join("tokenizer.json");

    if model_path.exists() && tokenizer_path.exists() {
        Some((model_path, tokenizer_path))
    } else {
        None
    }
}
