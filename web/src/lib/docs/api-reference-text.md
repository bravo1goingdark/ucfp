---
title: API reference — text
order: 3
description: Every text fingerprinting algorithm UCFP exposes — MinHash, SimHash, LSH, TLSH, semantic embeddings — with full parameter coverage.
---

# Text API reference

`POST /v1/ingest/text/{tenant}/{record}` is the canonical endpoint. Algorithm is selected via query string or the JSON body.

## Algorithm matrix

| `algorithm` | Output kind | Use for |
| --- | --- | --- |
| `minhash` | hash set (h slots, 64-bit each) | near-duplicate dedup at scale (Jaccard similarity) |
| `simhash-tf` | 64-bit signature | near-duplicate dedup, simpler than minhash |
| `simhash-idf` | 64-bit signature | same, but weights tokens by an IDF table |
| `lsh` | banded minhash | fast bucket lookup over millions of records |
| `tlsh` | 35-byte locality digest | malware-style fuzzy matching, robust to small edits |
| `semantic-local` | dense embedding (FP32 vector) | semantic similarity, on-device model |
| `semantic-openai` | dense embedding | OpenAI `text-embedding-3-…` models |
| `semantic-voyage` | dense embedding | Voyage AI models |
| `semantic-cohere` | dense embedding | Cohere `embed-…` models |

## Common parameters

These apply to every text algorithm. Send as JSON body when the call needs structured params; for simple text you can POST raw text and rely on defaults.

```json
{
  "text": "The quick brown fox.",
  "params": {
    "algorithm": "minhash",
    "k": 5,
    "h": 128,
    "tokenizer": "Word",
    "canonicalizer": {
      "normalization": "Nfkc",
      "case_fold": true,
      "strip_bidi": true,
      "strip_format": true,
      "apply_confusable": false
    },
    "preprocess": null,
    "security_mode": null
  }
}
```

### `k`

Shingle width. Default `5` for word tokenizer, `9` for grapheme. Smaller `k` is more sensitive to local edits; larger `k` is stricter.

### `h` (MinHash slot count)

`64`, `128`, or `256`. Higher `h` is slower and more bytes on the wire but yields a tighter Jaccard estimate. The dispatched route is `fingerprint_minhash_with::<H>` — passing any other value rejects with `422`.

### `tokenizer`: `TokenizerKind`

| Value | Behaviour |
| --- | --- |
| `Word` | Unicode word-segment, default. |
| `Grapheme` | Grapheme-cluster shingles. Use for code, IDs, languages without word breaks. |
| `CjkJp` | MeCab-backed Japanese segmentation. Requires `text-cjk-japanese` feature. |
| `CjkKo` | Korean morpheme segmentation. Requires `text-cjk-korean` feature. |

### `canonicalizer`: `CanonicalizerDto`

| Field | Default | Effect |
| --- | --- | --- |
| `normalization` | `"Nfkc"` | Unicode normalization form. `"Nfc"`, `"Nfd"`, `"Nfkc"`, `"Nfkd"`, `"None"`. |
| `case_fold` | `true` | Apply Unicode case folding. |
| `strip_bidi` | `true` | Remove bidirectional control marks (U+202A–U+202E, U+2066–U+2069). |
| `strip_format` | `true` | Remove invisible format characters (zero-width joiner, soft hyphen, …). |
| `apply_confusable` | `false` | Replace confusable glyphs with their Latin equivalents (Cyrillic `а` → Latin `a`). Slower, useful for anti-spam. |

### `preprocess`: `PreprocessKind | null`

| Value | Effect | Feature |
| --- | --- | --- |
| `null` | Treat input as plain text. | always |
| `"Html"` | Strip tags, extract visible text. | `text-markup` |
| `"Markdown"` | Render to text, drop formatting. | `text-markup` |
| `"Pdf"` | Extract text from PDF bytes. | `text-pdf` |

For `"Html"`/`"Markdown"`/`"Pdf"`, the dedicated subroute `POST /v1/ingest/text/{tid}/{rid}/preprocess/{html|markdown|pdf}` is more efficient — the body is sent raw and parsed once, rather than wrapped in JSON.

### `security_mode`: `UtsMode | null`

Hardens canonicalization against Unicode-based attacks (homoglyph spoofing, RTLO injection). Values: `"Strict"`, `"Lenient"`, `null` (off). Requires `text-security` feature.

## Per-algorithm parameters

### `minhash`

```json
{ "algorithm": "minhash", "h": 128, "k": 5 }
```

`h ∈ {64, 128, 256}`. Output is `h × 8` bytes plus a tiny header.

### `simhash-tf`

```json
{ "algorithm": "simhash-tf", "k": 5 }
```

64-bit signature. Term frequency–weighted by default.

### `simhash-idf`

```json
{
  "algorithm": "simhash-idf",
  "k": 5,
  "weighting": { "kind": "idf", "idf_table_ref": "english-wikipedia-2024" }
}
```

`idf_table_ref` resolves a server-side preloaded IDF table. Omitting it falls back to TF.

### `lsh`

```json
{ "algorithm": "lsh", "h": 128, "k": 5 }
```

Returns a banded MinHash suitable for bucket lookup. Internally `bands × rows = h`; default `bands=16, rows=8` for `h=128`.

### `tlsh`

```json
{ "algorithm": "tlsh" }
```

Returns the 35-byte TLSH digest. Requires input ≥ 50 bytes; smaller inputs reject with `422`. Feature `text-tlsh`.

### `semantic-local`

```json
{ "algorithm": "semantic-local", "model_id": "all-MiniLM-L6-v2" }
```

Runs a quantized sentence-transformer on the server. `model_id` must be one of the preloaded models (see `/healthz` for the list). Feature `text-semantic-local`.

### `semantic-openai`

```json
{ "algorithm": "semantic-openai", "model_id": "text-embedding-3-small" }
```

The server holds the OpenAI key. Latency dominated by the upstream call. Returns the FP32 vector inline. Feature `text-semantic-openai`.

### `semantic-voyage`

```json
{ "algorithm": "semantic-voyage", "model_id": "voyage-3" }
```

Feature `text-semantic-voyage`.

### `semantic-cohere`

```json
{ "algorithm": "semantic-cohere", "model_id": "embed-english-v3.0" }
```

Feature `text-semantic-cohere`.

## Streaming

`POST /v1/ingest/text/{tid}/{rid}/stream` accepts NDJSON: one JSON object per line, each `{ "text": "…" }`. The server returns one fingerprint per input. Useful for ingesting a corpus line-at-a-time without round-trips.

Feature `text-streaming`.

## Response shape

Every text route returns the same envelope:

```json
{
  "tenant_id": 17,
  "record_id": "01HZX…",
  "modality": "text",
  "algorithm": "txtfp-minhash-h128-v1",
  "format_version": 1,
  "config_hash": "0x9c1ab40f5fe2c7d3",
  "fingerprint_bytes": 1024,
  "has_embedding": false,
  "embedding_dim": null,
  "model_id": null,
  "metadata_bytes": 0
}
```

`config_hash` is the txtfp `config_hash(canon, tokenizer_tag, algorithm_tag)` — two records with the same `config_hash` are directly comparable; different `config_hash` means you must re-fingerprint to compare.
