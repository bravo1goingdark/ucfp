use std::error::Error;

use ucfp::{big_text_demo, PerceptualConfig};

fn main() -> Result<(), Box<dyn Error>> {
    let mut cfg : PerceptualConfig = PerceptualConfig::default();
    cfg.use_parallel = true;

    let (_doc, fingerprint) = big_text_demo(&cfg)?;

    println!(
        "Perceptual MinHash signature ({} values): {:?}",
        fingerprint.minhash.len(),
        fingerprint.minhash
    );

    Ok(())
}
