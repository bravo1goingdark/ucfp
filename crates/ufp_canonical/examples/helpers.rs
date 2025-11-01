use ufp_canonical::{
    CanonicalizeConfig, CanonicalizedDocument, canonicalize, collapse_whitespace, hash_text,
    tokenize,
};

fn main() {
    let raw_text = "  Café\tRUST!  ";

    let collapsed = collapse_whitespace(raw_text);
    println!("collapsed -> \"{collapsed}\"");

    let cfg = CanonicalizeConfig {
        strip_punctuation: true,
        lowercase: true,
    };

    let CanonicalizedDocument {
        canonical_text,
        tokens,
        sha256_hex,
    } = canonicalize(&collapsed, &cfg);

    println!("canonical text -> \"{canonical_text}\"");
    println!("sha256 -> {sha256_hex}");

    let helper_tokens = tokenize(&canonical_text);
    println!("tokens via helper -> {:?}", helper_tokens);

    let checksum_again = hash_text(&canonical_text);
    println!("hash_text helper -> {checksum_again}");
}
