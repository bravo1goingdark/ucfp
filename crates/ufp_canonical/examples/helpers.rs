use ufp_canonical::{
    canonicalize, collapse_whitespace, hash_text, tokenize, CanonicalizeConfig,
    CanonicalizedDocument,
};

fn main() {
    let raw_text = "  Café\tRUST!  ";

    let collapsed = collapse_whitespace(raw_text);
    println!("collapsed -> \"{collapsed}\"");

    let cfg = CanonicalizeConfig {
        strip_punctuation: true,
        ..Default::default()
    };

    let CanonicalizedDocument {
        doc_id: _,
        canonical_text,
        tokens: _tokens,
        sha256_hex,
    } = canonicalize("demo-helper", &collapsed, &cfg).expect("canonicalization succeeds");

    println!("canonical text -> \"{canonical_text}\"");
    println!("sha256 -> {sha256_hex}");

    let helper_tokens = tokenize(&canonical_text);
    println!("tokens via helper -> {helper_tokens:?}");

    let checksum_again = hash_text(&canonical_text);
    println!("hash_text helper -> {checksum_again}");
}
