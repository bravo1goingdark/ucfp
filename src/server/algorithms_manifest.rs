//! `GET /v1/algorithms` — machine-readable schema of every algorithm
//! the server can run, with the full set of tunable knobs.
//!
//! The frontend reads this manifest at boot and renders the playground's
//! tuning form generically: one labeled control per [`Tunable`], typed by
//! [`TunableKind`], defaulted from [`Tunable::default_value`].
//!
//! When you add a knob upstream:
//!
//! 1. Extend the matching `*Params` DTO in `dto.rs`.
//! 2. Wire the field into the modality wrapper in `modality/{...}.rs`.
//! 3. Add a [`Tunable`] entry here.
//!
//! Everything else (UI, serde wire-format, defaults) flows from this file.

// Helper builders below are used only by the per-modality `*_catalog()`
// arms, each cfg-gated on a modality feature. Slim builds (no modality
// features) correctly flag them as unused — silence the warning rather
// than mirroring all six cfg gates per helper.
#![allow(dead_code)]

use serde::Serialize;

/// Machine-readable type of a tunable knob.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TunableKind {
    /// Boolean toggle.
    Bool,
    /// Integer with `min`/`max`/`step` honored by the UI.
    Int,
    /// Floating-point with `min`/`max`/`step` honored by the UI.
    Float,
    /// String enum — one of `enum_values`.
    Enum,
    /// Free-form string (e.g. `model_id`, file paths).
    String,
    /// Free-form string rendered as a password input (API keys).
    Secret,
}

/// One tunable knob the UI should expose.
#[derive(Clone, Debug, Serialize)]
pub struct Tunable {
    /// Wire field name (matches the `*Params` DTO field).
    pub name: &'static str,
    /// Display label.
    pub label: &'static str,
    /// One-line help text shown beneath the control.
    pub help: &'static str,
    pub kind: TunableKind,
    /// Numeric inclusive lower bound (Int / Float).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    /// Numeric inclusive upper bound (Int / Float).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    /// UI step (Int / Float).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<f64>,
    /// Allowed values (Enum). Wire payload is the string value verbatim.
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    pub enum_values: &'static [&'static str],
    /// Default value as JSON. `null` means "no override; SDK default applies."
    pub default_value: serde_json::Value,
}

/// One algorithm a user can pick.
#[derive(Clone, Debug, Serialize)]
pub struct Algorithm {
    /// Wire `algorithm` value (matches the matching `*Algorithm` enum
    /// serde-renamed kebab-case).
    pub id: &'static str,
    /// Display label.
    pub label: &'static str,
    /// Short prose describing what this algorithm produces.
    pub description: &'static str,
    /// Tunable knobs the UI should render.
    pub tunables: Vec<Tunable>,
    /// Named presets (`balanced`, `high-recall`, `fast`). Each maps a
    /// subset of `tunables` to opinionated values.
    pub presets: Vec<Preset>,
}

/// A named knob bundle the UI surfaces as a quick-pick.
#[derive(Clone, Debug, Serialize)]
pub struct Preset {
    pub id: &'static str,
    pub label: &'static str,
    pub values: serde_json::Value,
}

/// One modality and its full algorithm catalog.
#[derive(Clone, Debug, Serialize)]
pub struct ModalityCatalog {
    pub modality: &'static str,
    pub algorithms: Vec<Algorithm>,
}

/// Top-level response body for `GET /v1/algorithms`.
#[derive(Clone, Debug, Serialize)]
pub struct AlgorithmsResponse {
    pub modalities: Vec<ModalityCatalog>,
}

// ── Builders for compactness ──────────────────────────────────────────
//
// These are referenced from the per-modality `*_catalog()` arms below,
// each cfg-gated on a modality feature. The crate-level
// `#![allow(dead_code)]` above silences the unused warning on slim
// builds where every modality feature is off.

fn t_int(
    name: &'static str,
    label: &'static str,
    help: &'static str,
    min: i64,
    max: i64,
    step: i64,
) -> Tunable {
    Tunable {
        name,
        label,
        help,
        kind: TunableKind::Int,
        min: Some(min as f64),
        max: Some(max as f64),
        step: Some(step as f64),
        enum_values: &[],
        default_value: serde_json::Value::Null,
    }
}
fn t_float(
    name: &'static str,
    label: &'static str,
    help: &'static str,
    min: f64,
    max: f64,
    step: f64,
) -> Tunable {
    Tunable {
        name,
        label,
        help,
        kind: TunableKind::Float,
        min: Some(min),
        max: Some(max),
        step: Some(step),
        enum_values: &[],
        default_value: serde_json::Value::Null,
    }
}
fn t_bool(name: &'static str, label: &'static str, help: &'static str) -> Tunable {
    Tunable {
        name,
        label,
        help,
        kind: TunableKind::Bool,
        min: None,
        max: None,
        step: None,
        enum_values: &[],
        default_value: serde_json::Value::Null,
    }
}
fn t_enum(
    name: &'static str,
    label: &'static str,
    help: &'static str,
    values: &'static [&'static str],
) -> Tunable {
    Tunable {
        name,
        label,
        help,
        kind: TunableKind::Enum,
        min: None,
        max: None,
        step: None,
        enum_values: values,
        default_value: serde_json::Value::Null,
    }
}
fn t_string(name: &'static str, label: &'static str, help: &'static str) -> Tunable {
    Tunable {
        name,
        label,
        help,
        kind: TunableKind::String,
        min: None,
        max: None,
        step: None,
        enum_values: &[],
        default_value: serde_json::Value::Null,
    }
}
fn t_secret(name: &'static str, label: &'static str, help: &'static str) -> Tunable {
    Tunable {
        name,
        label,
        help,
        kind: TunableKind::Secret,
        min: None,
        max: None,
        step: None,
        enum_values: &[],
        default_value: serde_json::Value::Null,
    }
}

// ── Catalog construction ──────────────────────────────────────────────

/// Build the full algorithm catalog. Feature-gated arms are only emitted
/// when the matching `*-` feature flag is on, so the manifest reflects
/// the binary's actual capabilities.
pub fn build() -> AlgorithmsResponse {
    let modalities = vec![
        #[cfg(feature = "text")]
        text_catalog(),
        #[cfg(feature = "image")]
        image_catalog(),
        #[cfg(feature = "audio")]
        audio_catalog(),
    ];
    AlgorithmsResponse { modalities }
}

#[cfg(feature = "text")]
fn text_catalog() -> ModalityCatalog {
    let canon_tunables = || {
        vec![
            t_enum(
                "canon_normalization",
                "Normalization",
                "Unicode normalization form. NFKC collapses ligatures and full-width forms (default).",
                &["nfc", "nfkc", "none"],
            ),
            t_bool(
                "canon_case_fold",
                "Case fold",
                "Apply Unicode case folding (default on).",
            ),
            t_bool(
                "canon_strip_bidi",
                "Strip Bidi",
                "Remove Bidi-control codepoints (Trojan-Source defense).",
            ),
            t_bool(
                "canon_strip_format",
                "Strip format chars",
                "Remove Cf-category codepoints (BOM, ZWSP, …).",
            ),
            t_bool(
                "canon_apply_confusable",
                "UTS #39 confusable skeleton",
                "Requires the `text-security` feature.",
            ),
        ]
    };
    let common_tok = || {
        vec![
            t_int(
                "k",
                "Shingle k",
                "Width of the k-shingle window (default 5).",
                1,
                16,
                1,
            ),
            t_int(
                "h",
                "MinHash slots (H)",
                "Signature size; higher = better Jaccard estimate (default 128).",
                16,
                1024,
                16,
            ),
            t_enum(
                "tokenizer",
                "Tokenizer",
                "UAX #29 word/grapheme tokenizer or a CJK morphological segmenter.",
                &["word", "grapheme", "cjk-jp", "cjk-ko"],
            ),
            t_enum(
                "preprocess",
                "Preprocess",
                "Optional HTML/Markdown/PDF → text pass before fingerprinting.",
                &["html", "markdown", "pdf"],
            ),
        ]
    };
    let mut algorithms = vec![
        Algorithm {
            id: "minhash",
            label: "MinHash",
            description: "Set-similarity sketch. Best for near-duplicate detection by Jaccard.",
            tunables: {
                let mut v = common_tok();
                v.extend(canon_tunables());
                v
            },
            presets: vec![
                Preset {
                    id: "balanced",
                    label: "Balanced",
                    values: serde_json::json!({"k": 5, "h": 128, "tokenizer": "word"}),
                },
                Preset {
                    id: "high-recall",
                    label: "High recall",
                    values: serde_json::json!({"k": 3, "h": 256, "tokenizer": "word"}),
                },
                Preset {
                    id: "fast",
                    label: "Fast",
                    values: serde_json::json!({"k": 7, "h": 64, "tokenizer": "word"}),
                },
            ],
        },
        Algorithm {
            id: "simhash-tf",
            label: "SimHash (TF)",
            description: "64-bit Charikar SimHash with term-frequency weighting.",
            tunables: {
                let mut v = common_tok();
                v.extend(canon_tunables());
                v
            },
            presets: vec![],
        },
        Algorithm {
            id: "simhash-idf",
            label: "SimHash (TF·IDF)",
            description: "64-bit SimHash with TF·IDF weighting (uses the server's default IDF table).",
            tunables: {
                let mut v = common_tok();
                v.extend(canon_tunables());
                v
            },
            presets: vec![],
        },
        Algorithm {
            id: "lsh",
            label: "LSH (banded MinHash)",
            description: "MinHash signature keyed for sub-linear ANN lookup.",
            tunables: {
                let mut v = common_tok();
                v.extend(canon_tunables());
                v
            },
            presets: vec![],
        },
        Algorithm {
            id: "tlsh",
            label: "TLSH",
            description: "Byte-level locality-sensitive hash; good for malware-style fuzzy matching.",
            tunables: canon_tunables(),
            presets: vec![],
        },
        Algorithm {
            id: "semantic-local",
            label: "Semantic (local ONNX)",
            description: "Dense embedding via a local ONNX text encoder (BGE / E5 / MiniLM).",
            tunables: vec![t_string(
                "model_id",
                "Model ID",
                "HF Hub repo id (e.g. `BAAI/bge-small-en-v1.5`) or filesystem path.",
            )],
            presets: vec![],
        },
        Algorithm {
            id: "semantic-openai",
            label: "Semantic (OpenAI)",
            description: "Dense embedding via the OpenAI embeddings API.",
            tunables: vec![
                t_string("model_id", "Model", "e.g. `text-embedding-3-small`."),
                t_secret("api_key", "API key", "OpenAI API key."),
            ],
            presets: vec![],
        },
        Algorithm {
            id: "semantic-voyage",
            label: "Semantic (Voyage)",
            description: "Dense embedding via the Voyage embeddings API.",
            tunables: vec![
                t_string("model_id", "Model", "e.g. `voyage-2`."),
                t_secret("api_key", "API key", "Voyage API key."),
            ],
            presets: vec![],
        },
        Algorithm {
            id: "semantic-cohere",
            label: "Semantic (Cohere)",
            description: "Dense embedding via the Cohere embeddings API.",
            tunables: vec![
                t_string("model_id", "Model", "e.g. `embed-english-v3.0`."),
                t_secret("api_key", "API key", "Cohere API key."),
            ],
            presets: vec![],
        },
    ];
    // Drop hidden algorithms when their feature isn't compiled in.
    // Clippy's `match_like_matches_macro` triggers in slim builds where
    // every arm collapses to `false` — silence it here since the cfg!()
    // values are only meaningful as a runtime feature gate.
    #[allow(clippy::match_like_matches_macro)]
    algorithms.retain(|a| match a.id {
        "simhash-tf" | "simhash-idf" => cfg!(feature = "text-simhash"),
        "lsh" => cfg!(feature = "text-lsh"),
        "tlsh" => cfg!(feature = "text-tlsh"),
        "semantic-local" => cfg!(feature = "text-semantic-local"),
        "semantic-openai" => cfg!(feature = "text-semantic-openai"),
        "semantic-voyage" => cfg!(feature = "text-semantic-voyage"),
        "semantic-cohere" => cfg!(feature = "text-semantic-cohere"),
        _ => true,
    });
    ModalityCatalog {
        modality: "text",
        algorithms,
    }
}

#[cfg(feature = "image")]
fn image_catalog() -> ModalityCatalog {
    let preprocess_tunables = || {
        vec![
            t_int(
                "max_input_bytes",
                "Max input bytes",
                "Reject payloads above this size (default 50 MiB).",
                1024,
                1_073_741_824,
                1024,
            ),
            t_int(
                "max_dimension",
                "Max dimension (px)",
                "Reject images with width or height above this (default 8192).",
                32,
                32_768,
                1,
            ),
            t_int(
                "min_dimension",
                "Min dimension (px)",
                "Reject images with width or height below this (default 32).",
                1,
                4096,
                1,
            ),
        ]
    };
    let mut algorithms = vec![
        Algorithm {
            id: "multi",
            label: "Multi-hash (P + D + A)",
            description: "Bundles PHash + DHash + AHash; resilient across crops and recompressions.",
            tunables: preprocess_tunables(),
            presets: vec![],
        },
        Algorithm {
            id: "phash",
            label: "PHash (DCT)",
            description: "DCT-based perceptual hash; strong on geometric robustness.",
            tunables: preprocess_tunables(),
            presets: vec![],
        },
        Algorithm {
            id: "dhash",
            label: "DHash (gradient)",
            description: "Horizontal-gradient hash; cheapest of the three perceptual hashes.",
            tunables: preprocess_tunables(),
            presets: vec![],
        },
        Algorithm {
            id: "ahash",
            label: "AHash (mean)",
            description: "Mean-thresholded average hash; baseline perceptual hash.",
            tunables: preprocess_tunables(),
            presets: vec![],
        },
        Algorithm {
            id: "semantic",
            label: "Semantic (CLIP ONNX)",
            description: "Dense visual embedding via a local CLIP-style ONNX model.",
            tunables: {
                let mut v = preprocess_tunables();
                v.push(t_string(
                    "model_id",
                    "Model path",
                    "Filesystem path to a local CLIP ONNX directory.",
                ));
                v
            },
            presets: vec![],
        },
    ];
    #[allow(clippy::match_like_matches_macro)]
    algorithms.retain(|a| match a.id {
        "phash" | "dhash" | "ahash" => cfg!(feature = "image-perceptual"),
        "semantic" => cfg!(feature = "image-semantic"),
        _ => true,
    });
    ModalityCatalog {
        modality: "image",
        algorithms,
    }
}

#[cfg(feature = "audio")]
fn audio_catalog() -> ModalityCatalog {
    let sample_rate = || {
        t_int(
            "sample_rate",
            "Sample rate (Hz)",
            "Required — sampling rate of the inbound f32 PCM stream.",
            1,
            384_000,
            1,
        )
    };
    let mut algorithms = vec![
        Algorithm {
            id: "wang",
            label: "Wang (Shazam)",
            description: "Landmark-pair hashes; classic Shazam-style fingerprint.",
            tunables: vec![
                sample_rate(),
                t_int(
                    "fan_out",
                    "Fan-out",
                    "Target peaks paired with each anchor (default 10).",
                    1,
                    64,
                    1,
                ),
                t_int(
                    "target_zone_t",
                    "Target zone Δt (frames)",
                    "Max time delta for pairing (default 63).",
                    1,
                    512,
                    1,
                ),
                t_int(
                    "target_zone_f",
                    "Target zone Δf (bins)",
                    "Max frequency delta for pairing (default 64).",
                    1,
                    1024,
                    1,
                ),
                t_int(
                    "peaks_per_sec",
                    "Peaks per second",
                    "Per-second cap on peak count (default 30).",
                    1,
                    256,
                    1,
                ),
                t_float(
                    "min_anchor_mag_db",
                    "Min anchor magnitude (dB)",
                    "Magnitude floor for anchors (default -50).",
                    -120.0,
                    0.0,
                    1.0,
                ),
            ],
            presets: vec![Preset {
                id: "balanced",
                label: "Balanced",
                values: serde_json::json!({"fan_out": 10, "peaks_per_sec": 30}),
            }],
        },
        Algorithm {
            id: "panako",
            label: "Panako (triplets)",
            description: "Tempo-invariant (±5%) triplet-hash fingerprint.",
            tunables: vec![
                sample_rate(),
                t_int(
                    "panako_fan_out",
                    "Fan-out",
                    "Triplets per anchor (default 5).",
                    1,
                    64,
                    1,
                ),
                t_int(
                    "panako_target_zone_t",
                    "Target zone Δt (frames)",
                    "Max time delta (default 96).",
                    1,
                    512,
                    1,
                ),
                t_int(
                    "panako_target_zone_f",
                    "Target zone Δf (bins)",
                    "Max frequency delta (default 96).",
                    1,
                    1024,
                    1,
                ),
                t_int(
                    "panako_peaks_per_sec",
                    "Peaks per second",
                    "Per-second cap (default 30).",
                    1,
                    256,
                    1,
                ),
                t_float(
                    "panako_min_anchor_mag_db",
                    "Min anchor magnitude (dB)",
                    "Magnitude floor (default -50).",
                    -120.0,
                    0.0,
                    1.0,
                ),
            ],
            presets: vec![],
        },
        Algorithm {
            id: "haitsma",
            label: "Haitsma–Kalker",
            description: "Philips robust hash; band-power sign bits, very compact (312 B/sec).",
            tunables: vec![
                sample_rate(),
                t_float(
                    "haitsma_fmin",
                    "Lower band edge (Hz)",
                    "Default 300.",
                    1.0,
                    22_000.0,
                    1.0,
                ),
                t_float(
                    "haitsma_fmax",
                    "Upper band edge (Hz)",
                    "Default 2000.",
                    1.0,
                    22_000.0,
                    1.0,
                ),
            ],
            presets: vec![],
        },
        Algorithm {
            id: "neural",
            label: "Neural (ONNX)",
            description: "Generic log-mel ONNX embedder; per-window dense vectors.",
            tunables: vec![
                sample_rate(),
                t_string(
                    "model_id",
                    "Model path",
                    "Filesystem path to the ONNX model.",
                ),
                t_float(
                    "neural_fmax",
                    "Mel filterbank fmax (Hz)",
                    "Override; defaults to sample_rate / 2.",
                    1.0,
                    96_000.0,
                    1.0,
                ),
            ],
            presets: vec![],
        },
        Algorithm {
            id: "watermark",
            label: "Watermark detect (AudioSeal)",
            description: "Run an AudioSeal-style detector; no fingerprint stored, returns confidence + payload.",
            tunables: vec![
                sample_rate(),
                t_string(
                    "model_id",
                    "Model path",
                    "Filesystem path to the AudioSeal ONNX detector.",
                ),
                t_float(
                    "watermark_threshold",
                    "Detection threshold",
                    "Confidence cutoff in [0, 1] (default 0.5).",
                    0.0,
                    1.0,
                    0.01,
                ),
            ],
            presets: vec![],
        },
    ];
    #[allow(clippy::match_like_matches_macro)]
    algorithms.retain(|a| match a.id {
        "panako" => cfg!(feature = "audio-panako"),
        "haitsma" => cfg!(feature = "audio-haitsma"),
        "neural" => cfg!(feature = "audio-neural"),
        "watermark" => cfg!(feature = "audio-watermark"),
        _ => true,
    });
    ModalityCatalog {
        modality: "audio",
        algorithms,
    }
}
