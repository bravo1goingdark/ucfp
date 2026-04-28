//! Text fingerprinting via [`txtfp`].
//!
//! Wraps every algorithm exposed by `txtfp` (MinHash, SimHash, LSH,
//! TLSH, semantic embeddings) into a uniform [`Record`]. Per-algorithm
//! functions are gated behind the corresponding `text-*` features in
//! [`Cargo.toml`].
//!
//! # Algorithms
//!
//! | Function                       | Output                | Feature gate            |
//! | ------------------------------ | --------------------- | ----------------------- |
//! | [`fingerprint_minhash`]        | `MinHash<H=128>`      | `text` (default)        |
//! | [`fingerprint_minhash_with`]   | `MinHash<H>`          | `text` (default)        |
//! | [`fingerprint_simhash_tf`]     | SimHash64 + TF        | `text-simhash`          |
//! | [`fingerprint_simhash_idf`]    | SimHash64 + TF·IDF    | `text-simhash`          |
//! | [`fingerprint_lsh`]            | MinHash + (b,r) tag   | `text-lsh`              |
//! | [`fingerprint_tlsh`]           | TLSH 128/1            | `text-tlsh`             |
//! | [`fingerprint_semantic_local`] | local ONNX embedding  | `text-semantic-local`   |
//! | [`fingerprint_semantic_openai`] | OpenAI embedding     | `text-semantic-openai`  |
//! | [`fingerprint_semantic_voyage`] | Voyage embedding     | `text-semantic-voyage`  |
//! | [`fingerprint_semantic_cohere`] | Cohere embedding     | `text-semantic-cohere`  |
//! | [`StreamingMinHashSession`]    | push/finalize streamer | `text-streaming`       |
//!
//! [`TextOpts`] carries the canonicalizer + tokenizer + shingle/slot
//! parameters threaded through the per-algorithm functions. Most callers
//! should reach for [`TextOpts::default`] and override only the fields
//! they need.

use bytes::Bytes;
use txtfp::{
    Canonicalizer, Fingerprinter, GraphemeTokenizer, MinHashFingerprinter, ShingleTokenizer,
    Tokenizer, WordTokenizer, config_hash as txtfp_config_hash,
};

use crate::core::{Modality, Record};
use crate::error::{Error, Result};

/// Default shingle width — see ARCHITECTURE §4 / txtfp docs.
pub const DEFAULT_K: usize = 5;
/// Default MinHash slot count.
pub const DEFAULT_H: usize = 128;

/// Stable algorithm tag for the default MinHash configuration (H=128).
pub const ALGORITHM_MINHASH_128: &str = "minhash-h128";
/// Stable algorithm tag for SimHash with term-frequency weighting.
pub const ALGORITHM_SIMHASH_TF: &str = "simhash-b64-tf";
/// Stable algorithm tag for SimHash with TF·IDF weighting.
pub const ALGORITHM_SIMHASH_IDF: &str = "simhash-b64-idf";
/// Stable algorithm tag for the LSH-keyed MinHash sketch.
pub const ALGORITHM_LSH: &str = "minhash-lsh-h128";
/// Stable algorithm tag for TLSH 128/1.
pub const ALGORITHM_TLSH: &str = "tlsh-128-1";
/// Stable algorithm tag for embeddings produced by a local ONNX model.
pub const ALGORITHM_SEMANTIC_LOCAL: &str = "embedding-local";
/// Stable algorithm tag for embeddings produced by OpenAI.
pub const ALGORITHM_SEMANTIC_OPENAI: &str = "embedding-openai";
/// Stable algorithm tag for embeddings produced by Voyage.
pub const ALGORITHM_SEMANTIC_VOYAGE: &str = "embedding-voyage";
/// Stable algorithm tag for embeddings produced by Cohere.
pub const ALGORITHM_SEMANTIC_COHERE: &str = "embedding-cohere";

// ─────────────────────────────────────────────────────────────────────────
// TextOpts — configuration carrier threaded through every text fn.
// ─────────────────────────────────────────────────────────────────────────

/// Choice of base tokenizer used to feed the fingerprinters.
///
/// `Word` and `Grapheme` are always available; the CJK variants require
/// the `text-cjk-japanese` / `text-cjk-korean` feature flags to compile
/// in real morphological tokenizers (otherwise constructing them returns
/// [`Error::Modality`]).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum TokenizerKind {
    /// UAX #29 word-boundary tokenizer.
    #[default]
    Word,
    /// UAX #29 grapheme-cluster tokenizer.
    Grapheme,
    /// CJK morphological segmenter; Lindera + IPADIC for Japanese.
    CjkJp,
    /// CJK morphological segmenter; Lindera + ko-dic for Korean.
    CjkKo,
}

/// Optional preprocessing pass applied to the input before
/// canonicalization. R3 routes the per-format helpers
/// (`html_to_text` / `markdown_to_text` / `pdf_to_text`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum PreprocessKind {
    /// Strip HTML markup down to plain text.
    Html,
    /// Render Markdown to plain text.
    Markdown,
    /// Extract text from a PDF binary.
    Pdf,
}

/// Sentinel for the optional UTS #39 confusable-skeleton security mode.
///
/// This exists as a single-variant enum so the public surface stays
/// future-proofed: when the upstream `txtfp` crate adds further
/// security-mode variants we can extend without breaking callers.
#[cfg(feature = "text-security")]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum UtsMode {
    /// Apply the UTS #39 confusable skeleton on top of NFKC.
    Confusable,
}

/// Per-call configuration carrier for every text fingerprinter.
///
/// Sensible defaults: NFKC + simple casefold + Bidi/format strip via
/// `Canonicalizer::default()`, the UAX #29 word tokenizer, `k = 5`
/// shingles, `h = 128` MinHash slots, no security mode, no preprocess.
#[derive(Clone, Debug)]
pub struct TextOpts {
    /// Unicode canonicalization pipeline.
    pub canonicalizer: Canonicalizer,
    /// Base tokenizer choice.
    pub tokenizer: TokenizerKind,
    /// Shingle width (k-grams over tokenizer output).
    pub k: usize,
    /// MinHash slot count. Currently informational — the public
    /// MinHash entry point is generic over a const `H`, so callers that
    /// need a different `H` reach for [`fingerprint_minhash_with`] and
    /// supply it as a const generic.
    pub h: usize,
    /// Optional UTS #39 confusable-skeleton mode (`text-security`).
    #[cfg(feature = "text-security")]
    pub security_mode: Option<UtsMode>,
    /// Optional preprocessing pass (HTML / Markdown / PDF).
    pub preprocess: Option<PreprocessKind>,
}

impl Default for TextOpts {
    fn default() -> Self {
        Self {
            canonicalizer: Canonicalizer::default(),
            tokenizer: TokenizerKind::default(),
            k: DEFAULT_K,
            h: DEFAULT_H,
            #[cfg(feature = "text-security")]
            security_mode: None,
            preprocess: None,
        }
    }
}

impl TextOpts {
    /// Stable string identifier for this options bundle's tokenizer
    /// configuration. Used to derive a `txtfp::config_hash`.
    fn tokenizer_tag(&self) -> String {
        match self.tokenizer {
            TokenizerKind::Word => format!("shingle-k={}/word-uax29", self.k),
            TokenizerKind::Grapheme => format!("shingle-k={}/grapheme-uax29", self.k),
            TokenizerKind::CjkJp => format!("shingle-k={}/cjk-jp", self.k),
            TokenizerKind::CjkKo => format!("shingle-k={}/cjk-ko", self.k),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Backwards-compat shim — keeps the original signature green.
// ─────────────────────────────────────────────────────────────────────────

/// Fingerprint UTF-8 text with [`TextOpts::default`] + MinHash<128>.
///
/// Kept as the original two-argument helper for backwards compatibility
/// with the pre-R1 server handlers; new code should prefer
/// [`fingerprint_minhash_with`] for explicit control over canonicalizer,
/// tokenizer, and slot count.
pub fn fingerprint_minhash(text: &str, tenant_id: u32, record_id: u64) -> Result<Record> {
    fingerprint_minhash_with::<DEFAULT_H>(text, &TextOpts::default(), tenant_id, record_id)
}

// ─────────────────────────────────────────────────────────────────────────
// MinHash with explicit options + slot count.
// ─────────────────────────────────────────────────────────────────────────

/// Fingerprint UTF-8 text with the supplied [`TextOpts`] and a
/// caller-chosen MinHash slot count `H`.
pub fn fingerprint_minhash_with<const H: usize>(
    text: &str,
    opts: &TextOpts,
    tenant_id: u32,
    record_id: u64,
) -> Result<Record> {
    let canon = build_canonicalizer(opts);
    let prepared = preprocess(text, opts.preprocess)?;

    // Dispatch on the tokenizer kind. We monomorphize per tokenizer so
    // each call site lands on a specialised `MinHashFingerprinter<T, H>`
    // and the inner `for_each_token` is statically dispatched.
    let (sig_bytes, tag): (Vec<u8>, String) = match opts.tokenizer {
        TokenizerKind::Word => {
            let tok = ShingleTokenizer {
                k: opts.k,
                inner: WordTokenizer,
            };
            let fp = MinHashFingerprinter::<_, H>::new(canon.clone(), tok);
            let sig = fp
                .fingerprint(&prepared)
                .map_err(|e| Error::Modality(e.to_string()))?;
            (sig.as_bytes().to_vec(), opts.tokenizer_tag())
        }
        TokenizerKind::Grapheme => {
            let tok = ShingleTokenizer {
                k: opts.k,
                inner: GraphemeTokenizer,
            };
            let fp = MinHashFingerprinter::<_, H>::new(canon.clone(), tok);
            let sig = fp
                .fingerprint(&prepared)
                .map_err(|e| Error::Modality(e.to_string()))?;
            (sig.as_bytes().to_vec(), opts.tokenizer_tag())
        }
        TokenizerKind::CjkJp => cjk_minhash::<H>(&canon, opts, &prepared, /*korean=*/ false)?,
        TokenizerKind::CjkKo => cjk_minhash::<H>(&canon, opts, &prepared, /*korean=*/ true)?,
    };

    let cfg = txtfp_config_hash(&canon, &tag, ALGORITHM_MINHASH_128);

    Ok(Record {
        tenant_id,
        record_id,
        modality: Modality::Text,
        format_version: txtfp::FORMAT_VERSION,
        algorithm: ALGORITHM_MINHASH_128.into(),
        config_hash: cfg,
        fingerprint: Bytes::from(sig_bytes),
        embedding: None,
        model_id: None,
        metadata: Bytes::new(),
    })
}

/// CJK-tokenizer MinHash dispatcher, gated behind the matching feature.
#[cfg(any(feature = "text-cjk-japanese", feature = "text-cjk-korean"))]
fn cjk_minhash<const H: usize>(
    canon: &Canonicalizer,
    opts: &TextOpts,
    prepared: &str,
    korean: bool,
) -> Result<(Vec<u8>, String)> {
    use txtfp::{CjkSegmenter, CjkTokenizer};
    let segmenter = if korean {
        CjkSegmenter::LinderaKoDic
    } else {
        CjkSegmenter::Lindera
    };
    let tok = ShingleTokenizer {
        k: opts.k,
        inner: CjkTokenizer::new(segmenter),
    };
    let fp = MinHashFingerprinter::<_, H>::new(canon.clone(), tok);
    let sig = fp
        .fingerprint(prepared)
        .map_err(|e| Error::Modality(e.to_string()))?;
    Ok((sig.as_bytes().to_vec(), opts.tokenizer_tag()))
}

/// Stub for builds without any CJK feature: surfaces a clean error so
/// requests still land on a meaningful response.
#[cfg(not(any(feature = "text-cjk-japanese", feature = "text-cjk-korean")))]
fn cjk_minhash<const H: usize>(
    _canon: &Canonicalizer,
    _opts: &TextOpts,
    _prepared: &str,
    _korean: bool,
) -> Result<(Vec<u8>, String)> {
    Err(Error::Modality(
        "CJK tokenizer requested but no text-cjk-* feature is enabled".into(),
    ))
}

// ─────────────────────────────────────────────────────────────────────────
// SimHash — feature `text-simhash`.
// ─────────────────────────────────────────────────────────────────────────

/// SimHash with term-frequency weighting (default `txtfp` choice).
#[cfg(feature = "text-simhash")]
pub fn fingerprint_simhash_tf(
    text: &str,
    opts: &TextOpts,
    tenant_id: u32,
    record_id: u64,
) -> Result<Record> {
    simhash_dispatch(
        text,
        opts,
        txtfp::Weighting::Tf,
        ALGORITHM_SIMHASH_TF,
        tenant_id,
        record_id,
    )
}

/// SimHash with TF·IDF weighting against a caller-supplied [`txtfp::IdfTable`].
#[cfg(feature = "text-simhash")]
pub fn fingerprint_simhash_idf(
    text: &str,
    opts: &TextOpts,
    idf: &txtfp::IdfTable,
    tenant_id: u32,
    record_id: u64,
) -> Result<Record> {
    let weighting = txtfp::Weighting::IdfWeighted(idf.clone());
    simhash_dispatch(
        text,
        opts,
        weighting,
        ALGORITHM_SIMHASH_IDF,
        tenant_id,
        record_id,
    )
}

/// SimHash dispatcher monomorphic on tokenizer choice.
#[cfg(feature = "text-simhash")]
fn simhash_dispatch(
    text: &str,
    opts: &TextOpts,
    weighting: txtfp::Weighting,
    tag: &'static str,
    tenant_id: u32,
    record_id: u64,
) -> Result<Record> {
    use txtfp::SimHashFingerprinter;

    let canon = build_canonicalizer(opts);
    let prepared = preprocess(text, opts.preprocess)?;

    let bytes: Vec<u8> = match opts.tokenizer {
        TokenizerKind::Word => {
            let fp =
                SimHashFingerprinter::new(canon.clone(), WordTokenizer).with_weighting(weighting);
            fp.fingerprint(&prepared)
                .map_err(|e| Error::Modality(e.to_string()))?
                .as_bytes()
                .to_vec()
        }
        TokenizerKind::Grapheme => {
            let fp = SimHashFingerprinter::new(canon.clone(), GraphemeTokenizer)
                .with_weighting(weighting);
            fp.fingerprint(&prepared)
                .map_err(|e| Error::Modality(e.to_string()))?
                .as_bytes()
                .to_vec()
        }
        TokenizerKind::CjkJp | TokenizerKind::CjkKo => {
            return Err(Error::Modality(
                "SimHash with CJK tokenizers is not yet supported".into(),
            ));
        }
    };

    let tok_tag = match opts.tokenizer {
        TokenizerKind::Word => "word-uax29",
        TokenizerKind::Grapheme => "grapheme-uax29",
        TokenizerKind::CjkJp => "cjk-jp",
        TokenizerKind::CjkKo => "cjk-ko",
    };
    let cfg = txtfp_config_hash(&canon, tok_tag, tag);

    Ok(Record {
        tenant_id,
        record_id,
        modality: Modality::Text,
        format_version: txtfp::FORMAT_VERSION,
        algorithm: tag.into(),
        config_hash: cfg,
        fingerprint: Bytes::from(bytes),
        embedding: None,
        model_id: None,
        metadata: Bytes::new(),
    })
}

// ─────────────────────────────────────────────────────────────────────────
// LSH — feature `text-lsh`. Produces the MinHash signature LSH would
// key on; the actual LshIndex lives in R3's territory.
// ─────────────────────────────────────────────────────────────────────────

/// Compute the MinHash signature that an `LshIndex` would key on.
///
/// `LshIndex` itself is a query-side data structure (it accumulates
/// signatures and looks up neighbours), so a `Record`-shaped helper for
/// it can only mean "produce the signature LSH would store." That's
/// exactly [`fingerprint_minhash_with`] with the algorithm tag swapped
/// to [`ALGORITHM_LSH`] so the index layer knows to insert into the LSH
/// posting tables.
#[cfg(feature = "text-lsh")]
pub fn fingerprint_lsh(
    text: &str,
    opts: &TextOpts,
    tenant_id: u32,
    record_id: u64,
) -> Result<Record> {
    let mut rec = fingerprint_minhash_with::<DEFAULT_H>(text, opts, tenant_id, record_id)?;
    rec.algorithm = ALGORITHM_LSH.into();
    Ok(rec)
}

// ─────────────────────────────────────────────────────────────────────────
// TLSH — feature `text-tlsh`.
// ─────────────────────────────────────────────────────────────────────────

/// TLSH 128/1 fingerprint over the canonicalized text.
#[cfg(feature = "text-tlsh")]
pub fn fingerprint_tlsh(
    text: &str,
    opts: &TextOpts,
    tenant_id: u32,
    record_id: u64,
) -> Result<Record> {
    use txtfp::TlshFingerprinter;
    let canon = build_canonicalizer(opts);
    let prepared = preprocess(text, opts.preprocess)?;

    let fp = TlshFingerprinter::new(canon.clone());
    let sig = fp
        .fingerprint(&prepared)
        .map_err(|e| Error::Modality(e.to_string()))?;

    let cfg = txtfp_config_hash(&canon, "tlsh-bytes", ALGORITHM_TLSH);

    Ok(Record {
        tenant_id,
        record_id,
        modality: Modality::Text,
        format_version: txtfp::FORMAT_VERSION,
        algorithm: ALGORITHM_TLSH.into(),
        config_hash: cfg,
        fingerprint: Bytes::from(sig.hex.into_bytes()),
        embedding: None,
        model_id: None,
        metadata: Bytes::new(),
    })
}

// ─────────────────────────────────────────────────────────────────────────
// Semantic — local ONNX. Feature `text-semantic-local`.
// ─────────────────────────────────────────────────────────────────────────

/// Run a local ONNX text encoder (BGE / E5 / MiniLM / etc.) over the
/// input and store the resulting vector both as fingerprint bytes and
/// in the `embedding` slot for vector-knn.
///
/// `model_id` may be either a Hugging Face Hub identifier (e.g.
/// `"BAAI/bge-small-en-v1.5"`) — fetched via `hf-hub` on first use — or
/// an absolute filesystem path to a directory containing `model.onnx`
/// + `tokenizer.json`. Heuristic: if `model_id` contains a `/` and
///   starts with `/` it is treated as a path.
#[cfg(feature = "text-semantic-local")]
pub fn fingerprint_semantic_local(
    text: &str,
    model_id: &str,
    tenant_id: u32,
    record_id: u64,
) -> Result<Record> {
    use txtfp::semantic::{EmbeddingProvider, LocalProvider};

    let provider =
        LocalProvider::from_pretrained(model_id).map_err(|e| Error::Modality(e.to_string()))?;
    let emb = provider
        .embed(text)
        .map_err(|e| Error::Modality(e.to_string()))?;

    semantic_record(
        emb.vector,
        Some(model_id.to_string()),
        ALGORITHM_SEMANTIC_LOCAL,
        tenant_id,
        record_id,
    )
}

// ─────────────────────────────────────────────────────────────────────────
// Semantic — OpenAI / Voyage / Cohere. Each gated independently.
// ─────────────────────────────────────────────────────────────────────────

/// Embed via the OpenAI embeddings API.
#[cfg(feature = "text-semantic-openai")]
pub fn fingerprint_semantic_openai(
    text: &str,
    model: &str,
    api_key: &str,
    tenant_id: u32,
    record_id: u64,
) -> Result<Record> {
    use txtfp::semantic::{EmbeddingProvider, providers::openai::OpenAiProvider};

    let provider = OpenAiProvider::new(api_key)
        .map_err(|e| Error::Modality(e.to_string()))?
        .with_model(model.to_string());
    let emb = provider
        .embed(text)
        .map_err(|e| Error::Modality(e.to_string()))?;

    semantic_record(
        emb.vector,
        Some(model.to_string()),
        ALGORITHM_SEMANTIC_OPENAI,
        tenant_id,
        record_id,
    )
}

/// Embed via the Voyage embeddings API.
#[cfg(feature = "text-semantic-voyage")]
pub fn fingerprint_semantic_voyage(
    text: &str,
    model: &str,
    api_key: &str,
    tenant_id: u32,
    record_id: u64,
) -> Result<Record> {
    use txtfp::semantic::{EmbeddingProvider, providers::voyage::VoyageProvider};

    let provider = VoyageProvider::new(api_key)
        .map_err(|e| Error::Modality(e.to_string()))?
        .with_model(model.to_string());
    let emb = provider
        .embed(text)
        .map_err(|e| Error::Modality(e.to_string()))?;

    semantic_record(
        emb.vector,
        Some(model.to_string()),
        ALGORITHM_SEMANTIC_VOYAGE,
        tenant_id,
        record_id,
    )
}

/// Embed via the Cohere embeddings API.
#[cfg(feature = "text-semantic-cohere")]
pub fn fingerprint_semantic_cohere(
    text: &str,
    model: &str,
    api_key: &str,
    tenant_id: u32,
    record_id: u64,
) -> Result<Record> {
    use txtfp::semantic::{EmbeddingProvider, providers::cohere::CohereProvider};

    let provider = CohereProvider::new(api_key)
        .map_err(|e| Error::Modality(e.to_string()))?
        .with_model(model.to_string());
    let emb = provider
        .embed(text)
        .map_err(|e| Error::Modality(e.to_string()))?;

    semantic_record(
        emb.vector,
        Some(model.to_string()),
        ALGORITHM_SEMANTIC_COHERE,
        tenant_id,
        record_id,
    )
}

/// Common Record-builder for any `Vec<f32>` semantic embedding.
#[cfg(any(
    feature = "text-semantic-local",
    feature = "text-semantic-openai",
    feature = "text-semantic-voyage",
    feature = "text-semantic-cohere",
))]
fn semantic_record(
    vector: Vec<f32>,
    model_id: Option<String>,
    tag: &'static str,
    tenant_id: u32,
    record_id: u64,
) -> Result<Record> {
    if vector.is_empty() {
        return Err(Error::Modality("provider returned empty embedding".into()));
    }
    let bytes = Bytes::copy_from_slice(bytemuck::cast_slice(&vector));
    Ok(Record {
        tenant_id,
        record_id,
        modality: Modality::Text,
        format_version: txtfp::FORMAT_VERSION,
        algorithm: tag.into(),
        config_hash: 0,
        fingerprint: bytes,
        embedding: Some(vector),
        model_id,
        metadata: Bytes::new(),
    })
}

// ─────────────────────────────────────────────────────────────────────────
// Streaming MinHash — feature `text-streaming`.
// ─────────────────────────────────────────────────────────────────────────

/// Push-based streaming MinHash session.
///
/// R3 owns the HTTP NDJSON plumbing; this struct exposes the minimum
/// surface needed by it — `push` for each inbound chunk, `finalize` to
/// emit the final signature. `push` always returns an empty `Vec` (the
/// upstream `MinHashStreaming` is buffered, not online); only
/// `finalize` produces the single result `Record`. `finalize` consumes
/// the inner streamer (`StreamingFingerprinter::finalize` takes `self`),
/// so it returns `Err` if called twice.
#[cfg(feature = "text-streaming")]
pub struct StreamingMinHashSession {
    inner: Option<txtfp::MinHashStreaming<ShingleTokenizer<WordTokenizer>, DEFAULT_H>>,
    canon: Canonicalizer,
    tenant_id: u32,
    record_id: u64,
}

#[cfg(feature = "text-streaming")]
impl StreamingMinHashSession {
    /// Build a session with [`TextOpts`] applied to the inner
    /// canonicalizer. The tokenizer is pinned to word-shingles for
    /// streaming; callers needing other tokenizers should use the
    /// offline path.
    pub fn new(opts: &TextOpts, tenant_id: u32, record_id: u64) -> Self {
        let canon = build_canonicalizer(opts);
        let inner_fp = MinHashFingerprinter::<_, DEFAULT_H>::new(
            canon.clone(),
            ShingleTokenizer {
                k: opts.k,
                inner: WordTokenizer,
            },
        );
        Self {
            inner: Some(txtfp::MinHashStreaming::new(inner_fp)),
            canon,
            tenant_id,
            record_id,
        }
    }

    /// Append a UTF-8 chunk. Always returns an empty vec (the streamer
    /// is buffered until `finalize`).
    pub fn push(&mut self, chunk: &[u8]) -> Result<Vec<Record>> {
        use txtfp::StreamingFingerprinter;
        let inner = self
            .inner
            .as_mut()
            .ok_or_else(|| Error::Modality("streaming session already finalized".into()))?;
        inner
            .update(chunk)
            .map_err(|e| Error::Modality(e.to_string()))?;
        Ok(Vec::new())
    }

    /// Drain the buffered text into a single MinHash record. The session
    /// becomes unusable afterwards (subsequent `push`/`finalize` calls
    /// return `Err`).
    pub fn finalize(&mut self) -> Result<Vec<Record>> {
        use txtfp::StreamingFingerprinter;
        let inner = self
            .inner
            .take()
            .ok_or_else(|| Error::Modality("streaming session already finalized".into()))?;
        let sig = inner
            .finalize()
            .map_err(|e| Error::Modality(e.to_string()))?;
        let cfg = txtfp_config_hash(
            &self.canon,
            &format!("shingle-k={DEFAULT_K}/word-uax29"),
            ALGORITHM_MINHASH_128,
        );
        Ok(vec![Record {
            tenant_id: self.tenant_id,
            record_id: self.record_id,
            modality: Modality::Text,
            format_version: txtfp::FORMAT_VERSION,
            algorithm: ALGORITHM_MINHASH_128.into(),
            config_hash: cfg,
            fingerprint: Bytes::copy_from_slice(sig.as_bytes()),
            embedding: None,
            model_id: None,
            metadata: Bytes::new(),
        }])
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Helpers — canonicalizer construction and preprocess routing.
// ─────────────────────────────────────────────────────────────────────────

/// Build a [`Canonicalizer`] from the [`TextOpts`] flags. With the
/// `text-security` feature the `security_mode` field flips the
/// confusable-skeleton bit on top of the supplied canonicalizer.
fn build_canonicalizer(opts: &TextOpts) -> Canonicalizer {
    #[cfg(feature = "text-security")]
    {
        if let Some(UtsMode::Confusable) = opts.security_mode {
            let mut builder = opts.canonicalizer.config().clone();
            builder.apply_confusable = true;
            return builder.build();
        }
    }
    opts.canonicalizer.clone()
}

/// Apply the requested `PreprocessKind` (HTML → text, Markdown → text,
/// PDF → text). `None` returns the input unchanged. Calls into the
/// matching feature-gated `txtfp` helper; missing-feature requests
/// surface as [`Error::Modality`].
fn preprocess(text: &str, kind: Option<PreprocessKind>) -> Result<String> {
    match kind {
        None => Ok(text.to_string()),
        Some(PreprocessKind::Html) => preprocess_html(text),
        Some(PreprocessKind::Markdown) => preprocess_markdown(text),
        Some(PreprocessKind::Pdf) => preprocess_pdf(text.as_bytes()),
    }
}

#[cfg(feature = "text-markup")]
fn preprocess_html(text: &str) -> Result<String> {
    txtfp::html_to_text(text).map_err(|e| Error::Modality(e.to_string()))
}

#[cfg(not(feature = "text-markup"))]
fn preprocess_html(_text: &str) -> Result<String> {
    Err(Error::Modality(
        "HTML preprocess requested but text-markup feature is disabled".into(),
    ))
}

#[cfg(feature = "text-markup")]
fn preprocess_markdown(text: &str) -> Result<String> {
    txtfp::markdown_to_text(text).map_err(|e| Error::Modality(e.to_string()))
}

#[cfg(not(feature = "text-markup"))]
fn preprocess_markdown(_text: &str) -> Result<String> {
    Err(Error::Modality(
        "Markdown preprocess requested but text-markup feature is disabled".into(),
    ))
}

#[cfg(feature = "text-pdf")]
fn preprocess_pdf(bytes: &[u8]) -> Result<String> {
    txtfp::pdf_to_text(bytes).map_err(|e| Error::Modality(e.to_string()))
}

#[cfg(not(feature = "text-pdf"))]
fn preprocess_pdf(_bytes: &[u8]) -> Result<String> {
    Err(Error::Modality(
        "PDF preprocess requested but text-pdf feature is disabled".into(),
    ))
}

// Touch the `Tokenizer` import to suppress unused warnings on builds
// where no SimHash/CJK path consumes it directly. This is a no-op call
// site that the optimiser elides.
#[allow(dead_code)]
fn _touch_tokenizer_trait<T: Tokenizer>(_: &T) {}
