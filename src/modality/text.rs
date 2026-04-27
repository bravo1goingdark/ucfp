//! Text fingerprinting via [`txtfp`].
//!
//! Default builder: NFKC canonicalization, word tokenization, 5-shingle
//! windowing, MinHash with H=128 slots. Override via builders if you
//! need different parameters.

use bytes::Bytes;
use txtfp::{
    Canonicalizer, Fingerprinter, MinHashFingerprinter, ShingleTokenizer, WordTokenizer,
    config_hash as txtfp_config_hash,
};

use crate::core::{Modality, Record};
use crate::error::{Error, Result};

/// Default shingle width — see ARCHITECTURE §4 / txtfp docs.
pub const DEFAULT_K: usize = 5;
/// Default MinHash slot count.
pub const DEFAULT_H: usize = 128;
/// Stable algorithm tag for the default MinHash configuration.
pub const ALGORITHM_MINHASH_128: &str = "minhash-h128";

/// Fingerprint UTF-8 text with the default canonicalizer + 5-shingle word
/// tokenizer + MinHash<128>.
pub fn fingerprint_minhash(text: &str, tenant_id: u32, record_id: u64) -> Result<Record> {
    let canon = Canonicalizer::default();
    let tok = ShingleTokenizer {
        k: DEFAULT_K,
        inner: WordTokenizer,
    };
    let fp: MinHashFingerprinter<_, DEFAULT_H> =
        MinHashFingerprinter::<_, DEFAULT_H>::new(canon.clone(), tok);

    let sig = fp
        .fingerprint(text)
        .map_err(|e| Error::Modality(e.to_string()))?;

    let bytes = Bytes::copy_from_slice(sig.as_bytes());
    let cfg = txtfp_config_hash(&canon, "word-uax29-shingle5", "minhash-h128");

    Ok(Record {
        tenant_id,
        record_id,
        modality: Modality::Text,
        format_version: txtfp::FORMAT_VERSION,
        algorithm: ALGORITHM_MINHASH_128.into(),
        config_hash: cfg,
        fingerprint: bytes,
        embedding: None,
        model_id: None,
        metadata: Bytes::new(),
    })
}
