use canonical::{canonicalize, CanonicalizeConfig, CanonicalizedDocument};
use std::fs;

fn main() {
    let file_path = "crates/canonical/examples/big_text.txt";
    let content = fs::read_to_string(file_path).expect("file path invalid");

    let cfg = CanonicalizeConfig {
        strip_punctuation: true,
        ..Default::default()
    };

    let doc: CanonicalizedDocument =
        canonicalize("demo-doc", &content, &cfg).expect("canonicalization succeeds");
    println!("canonical: {}", doc.canonical_text);
    println!();
    println!("tokens: {:?}", doc.tokens);
    println!();
    println!("sha256: {}", doc.sha256_hex);
}
