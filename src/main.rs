//! Main entry point for the UCFP CLI demo.
//!
//! This binary runs the `big_text_demo` function, which processes a large text
//! file through the perceptual fingerprinting pipeline and prints the resulting
//! MinHash signature.

use std::error::Error;

use ucfp::{PerceptualConfig, big_text_demo};

fn main() -> Result<(), Box<dyn Error>> {
    // Configure the perceptual fingerprinting pipeline to use parallelism.
    let cfg: PerceptualConfig = PerceptualConfig {
        use_parallel: true,
        ..PerceptualConfig::default()
    };

    // Run the demo function with the specified configuration.
    let (_doc, fingerprint) = big_text_demo(&cfg)?;

    // Print the resulting MinHash signature to the console.
    println!(
        "Perceptual MinHash signature ({} values): {:?}",
        fingerprint.minhash.len(),
        fingerprint.minhash
    );

    Ok(())
}
