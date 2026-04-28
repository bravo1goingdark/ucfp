---
title: API reference — image
order: 4
description: Image fingerprinting — multi-hash, perceptual hashes (pHash, dHash, aHash), and semantic embeddings.
---

# Image API reference

`POST /v1/ingest/image/{tenant}/{record}` accepts the raw image bytes. Algorithm is selected via `?algorithm=`.

## Algorithm matrix

| `algorithm` | Output | Use for |
| --- | --- | --- |
| `multi` | weighted blend of pHash + dHash + aHash + global + block | best general-purpose default; survives crops, rotations, watermark overlays |
| `phash` | 64-bit perceptual hash (DCT-based) | near-duplicate detection, robust to scaling and mild colour shift |
| `dhash` | 64-bit difference hash | very fast, robust to gamma changes, brittle to crops |
| `ahash` | 64-bit average hash | fastest, weakest; debugging or low-stakes dedup |
| `semantic` | dense embedding (FP32 vector) | content-similarity search (CLIP-class models) |

## Request

```bash
curl -sS https://ucfp.dev/api/fingerprint \
  -H 'Authorization: Bearer ucfp_…' \
  -H 'Content-Type: image/jpeg' \
  --data-binary @photo.jpg
```

Or, with parameters as a multipart upload:

```bash
curl -sS -X POST 'https://ucfp.dev/v1/ingest/image/17/01HZX…?algorithm=multi' \
  -H 'Authorization: Bearer ucfp_…' \
  -F 'image=@photo.jpg' \
  -F 'preprocess={"max_input_bytes":10485760,"max_dimension":2048,"min_dimension":32};type=application/json' \
  -F 'multi_config={"phash_weight":0.5,"dhash_weight":0.3,"ahash_weight":0.2};type=application/json'
```

## `PreprocessConfigDto`

Applies to every image algorithm. Validates and resizes before hashing.

| Field | Default | Effect |
| --- | --- | --- |
| `max_input_bytes` | `10485760` (10 MiB) | Reject larger uploads with `413`. |
| `max_dimension` | `2048` | Downscale longest edge to this if larger. Saves CPU, lossy on tiny detail. |
| `min_dimension` | `32` | Reject inputs whose shortest edge is smaller, with `422`. |

## `MultiHashConfigDto` (multi only)

Controls the blended digest. Weights are normalised before hashing — only their ratios matter.

| Field | Default | Effect |
| --- | --- | --- |
| `phash_weight` | `0.4` | Contribution of pHash. |
| `dhash_weight` | `0.3` | Contribution of dHash. |
| `ahash_weight` | `0.1` | Contribution of aHash. |
| `global_weight` | `0.1` | Contribution of the global colour-histogram component. |
| `block_weight` | `0.1` | Contribution of the per-block descriptor. |
| `block_distance_threshold` | `12` | Hamming threshold below which two block-descriptors count as a match. |

Set any weight to `0` to disable that component.

## Per-algorithm notes

### `multi` (default)

Blends pHash, dHash, aHash, a 64-bin colour histogram (global), and a 4×4 block descriptor. Best survival against rotation, crop, and watermark. Largest output (~136 bytes) but still cheap.

### `phash`

DCT-based. Output: 8 bytes. Reliable down to ~5 % rescale. Standard recommendation for "find near-identical photos". Feature `image-perceptual`.

### `dhash`

Difference hash. Output: 8 bytes. Faster than pHash (no DCT) and more robust to gamma; fails on tight crops. Feature `image-perceptual`.

### `ahash`

Average hash. Output: 8 bytes. Compute on resized greyscale, threshold each pixel against the mean. Easy to reason about, weakest signal. Feature `image-perceptual`.

### `semantic`

Runs a CLIP-class image encoder. Returns an FP32 vector (typically 384-d or 512-d). Required for "find images that look like this image" search across diverse content. Specify `model_id` (e.g. `clip-vit-b32`); see `/healthz` for the loaded model list. Feature `image-semantic`.

```bash
curl -sS -X POST 'https://ucfp.dev/v1/ingest/image/17/01HZX…?algorithm=semantic&model_id=clip-vit-b32' \
  -H 'Authorization: Bearer ucfp_…' \
  -H 'Content-Type: image/png' \
  --data-binary @photo.png
```

## Response

```json
{
  "tenant_id": 17,
  "record_id": "01HZX…",
  "modality": "image",
  "algorithm": "imgfprint-multi-v1",
  "format_version": 1,
  "config_hash": "0x4f01c882b1ea93de",
  "fingerprint_bytes": 136,
  "has_embedding": false,
  "embedding_dim": null,
  "model_id": null
}
```

For `semantic`, `has_embedding` is `true` and `embedding_dim` is set.

## Supported input formats

JPEG, PNG, WebP, GIF (first frame), BMP, TIFF. Animated content uses the first frame; submit per-frame to fingerprint a video, or use the upcoming video modality (out of v1).
