//! Download example for the semantic crate.
//!
//! This example demonstrates how to use the semantic crate with auto-downloaded
//! models from Hugging Face. The ONNX model and tokenizer are downloaded on first use.
//!
//! ## Key Points
//!
//! - Uses [`SemanticConfig`] with `model_url` and `tokenizer_url` for auto-download
//! - Uses `/resolve/` not `/blob/` for direct file downloads from Hugging Face
//! - The [`semanticize`] function is async and returns a Future
//!
//! ## Usage
//!
//! ```bash
//! cargo run -p content-semantic --example download
//! ```
//!
//! ## Requirements
//!
//! - Network access to download models from Hugging Face
//! - First run will be slower due to model download

use semantic::{semanticize, SemanticConfig};

#[tokio::main]
async fn main() {
    let doc_id = String::from("download-test");
    let cfg = SemanticConfig {
        normalize: true,
        model_url: Some(
            "https://huggingface.co/BAAI/bge-small-en-v1.5/resolve/main/onnx/model.onnx".into(),
        ),
        tokenizer_url: Some(
            "https://huggingface.co/BAAI/bge-small-en-v1.5/resolve/main/tokenizer.json".into(),
        ),
        ..Default::default()
    };
    let text = String::from("my name is ashutosh kumar");

    let embedding = semanticize(&doc_id, &text, &cfg).await;

    match embedding {
        Ok(contents) => {
            println!("contents: {:?}", contents)
        }
        Err(why) => println!("Error: {}", why),
    }
}
