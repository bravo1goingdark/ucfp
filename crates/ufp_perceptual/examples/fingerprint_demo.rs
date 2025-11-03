//! Demonstrates generating a perceptual fingerprint (shingles, winnowed minima,
//! and MinHash signature) for a short token sequence.
//! Run with `cargo run --package ufp_perceptual --example fingerprint_demo`.

use ufp_perceptual::{perceptualize_tokens, PerceptualConfig};

fn main() {
    let text = "The quick brown fox jumps over the lazy dog";
    let tokens: Vec<String> = text.split_whitespace().map(|t| t.to_string()).collect();

    let mut cfg = PerceptualConfig::default();
    cfg.use_parallel = true;
    cfg.seed = 0xDEADBEEF;

    match perceptualize_tokens(&tokens, &cfg) {
        Ok(fingerprint) => {
            println!("Shingles ({}): {:?}", fingerprint.shingles.len(), fingerprint.shingles);
            println!(
                "Winnowed selections: {:?}",
                fingerprint
                    .winnowed
                    .iter()
                    .map(|w| (w.start_idx, w.hash))
                    .collect::<Vec<_>>()
            );
            println!(
                "MinHash signature ({} values): {:?}",
                fingerprint.minhash.len(),
                fingerprint.minhash
            );
            println!("Metadata: {:?}", fingerprint.meta);
        }
        Err(err) => eprintln!("fingerprinting failed: {err}"),
    }
}
