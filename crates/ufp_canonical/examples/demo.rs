use std::fs;
use ufp_canonical::{CanonicalizeConfig, CanonicalizedDocument, canonicalize};

fn main() {
    let file_path = "crates/ufp_canonical/examples/big_text.txt";
    let content = fs::read_to_string(file_path).expect("file path invalid");

    let cfg = CanonicalizeConfig {
        strip_punctuation: true,
        lowercase: true,
    };
    let doc: CanonicalizedDocument = canonicalize(&content, &cfg);
    println!("canonical: {}", doc.canonical_text);
    println!();
    println!("tokens: {:?}", doc.tokens);
    println!();
    println!("sha256: {}", doc.sha256_hex);
}
