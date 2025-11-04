use std::error::Error;

use ucfp::{PerceptualConfig, big_text_demo};

fn main() -> Result<(), Box<dyn Error>> {
    let cfg: PerceptualConfig = PerceptualConfig {
        use_parallel: true,
        ..PerceptualConfig::default()
    };

    let (_doc, fingerprint) = big_text_demo(&cfg)?;

    println!(
        "Perceptual MinHash signature ({} values): {:?}",
        fingerprint.minhash.len(),
        fingerprint.minhash
    );

    Ok(())
}
