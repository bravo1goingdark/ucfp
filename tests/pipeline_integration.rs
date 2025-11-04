use ucfp::{PerceptualConfig, PipelineError, big_text_demo};

#[test]
fn big_text_demo_integration() -> Result<(), PipelineError> {
    let cfg = PerceptualConfig::default();

    let (doc, fingerprint) = big_text_demo(&cfg)?;

    assert!(
        !doc.canonical_text.is_empty(),
        "canonical text should exist"
    );
    assert!(
        !fingerprint.shingles.is_empty(),
        "shingles should be populated"
    );
    assert!(
        !fingerprint.winnowed.is_empty(),
        "winnowed windows should be present"
    );
    assert!(
        !fingerprint.minhash.is_empty(),
        "minhash signature should be present"
    );
    assert_eq!(
        fingerprint.meta.k, cfg.k,
        "fingerprint metadata should reflect config"
    );

    Ok(())
}
