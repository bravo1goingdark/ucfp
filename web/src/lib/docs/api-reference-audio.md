---
title: API reference — audio
order: 5
description: Audio fingerprinting — Wang (Shazam-style), Panako, Haitsma, neural embeddings, and watermark detection.
---

# Audio API reference

`POST /v1/ingest/audio/{tenant}/{record}` accepts raw PCM samples or a decodable container (WAV, FLAC, MP3, OGG). Algorithm via `?algorithm=`.

## Algorithm matrix

| `algorithm` | Output | Use for |
| --- | --- | --- |
| `wang` | constellation-of-peaks landmark hashes | Shazam-style "name that song" matching |
| `panako` | scale-and-tempo robust landmark hashes | matching across pitch/tempo modifications |
| `haitsma` | block-energy bit pattern | dense fingerprint, fast lookup, classic Haitsma–Kalker |
| `neural` | dense embedding (FP32 vector) | semantic similarity (genre, mood, voice ID) |
| `watermark` | `WatermarkReport` (no record persisted) | detect ucfp/audiofp watermarks embedded by an earlier `/embed` call |

## Required parameters

### `sample_rate`

The decoder needs the sample rate for raw PCM. Common values: `16000`, `22050`, `44100`, `48000`. For containerised audio (WAV, MP3, FLAC, OGG) it is read from the header — pass the value anyway as a sanity check.

### `model_id` (neural only)

Selects the embedding model. Omit to use the server default. Available models on the hosted plane: `vggish`, `clap-htsat`, `pann-cnn14`. Self-hosters can preload custom models — see the Rust crate docs.

## Per-algorithm parameters

### `wang`

```bash
curl -sS -X POST \
  'https://ucfp.dev/v1/ingest/audio/17/01HZX…?algorithm=wang&sample_rate=44100' \
  -H 'Authorization: Bearer ucfp_…' \
  -H 'Content-Type: audio/wav' \
  --data-binary @clip.wav
```

Optional `WangConfig` body (multipart):

```json
{
  "fan_value": 15,
  "amp_min": 10,
  "peak_neighborhood": 20,
  "min_hash_time_delta": 0,
  "max_hash_time_delta": 200
}
```

### `panako`

Same shape, with a `PanakoConfig` body. Robust to ±10 % tempo and ±5 semitone pitch changes. Feature `audio-panako`.

### `haitsma`

Optional `HaitsmaConfig`:

```json
{ "frame_size": 2048, "frame_stride": 64 }
```

Output: one 32-bit subfingerprint per frame; matching uses Hamming over windows of ~256 frames. Feature `audio-haitsma`.

### `neural`

```bash
curl -sS -X POST \
  'https://ucfp.dev/v1/ingest/audio/17/01HZX…?algorithm=neural&sample_rate=16000&model_id=clap-htsat' \
  -H 'Authorization: Bearer ucfp_…' \
  -H 'Content-Type: audio/wav' \
  --data-binary @clip.wav
```

Returns an FP32 vector. Slow (model inference); cache aggressively. Feature `audio-neural`.

### `watermark`

`POST /v1/ingest/audio/{tenant}/{record}/watermark` — note the `/watermark` suffix.

Does **not** persist a record. Returns `WatermarkReport`:

```json
{
  "detected": true,
  "payload": "0x9f01a2c3",
  "confidence": 0.94
}
```

If `detected` is `false`, `payload` is `null` and `confidence` indicates the model's certainty in the negative. Feature `audio-watermark`.

## Streaming

`POST /v1/ingest/audio/{tenant}/{record}/stream` accepts a chunked body with framed PCM. Use this for live audio (mic capture, RTSP relay, broadcast monitor). The server emits incremental fingerprints as the stream advances.

Multipart form fields:

| Field | Type | Notes |
| --- | --- | --- |
| `sample_rate` | int | Required. |
| `algorithm` | string | `wang`, `panako`, or `haitsma`. `neural` and `watermark` are not streamable in v1. |
| `audio` | binary stream | The chunked body. |

Response: NDJSON, one fingerprint per line. The connection stays open until the client closes its body. Feature `audio-streaming`.

## Response (non-streaming)

```json
{
  "tenant_id": 17,
  "record_id": "01HZX…",
  "modality": "audio",
  "algorithm": "audiofp-wang-v1",
  "format_version": 1,
  "config_hash": "0x88a121bc4f0e7dd5",
  "fingerprint_bytes": 4096,
  "has_embedding": false,
  "embedding_dim": null,
  "model_id": null
}
```

For `neural`, `has_embedding` is `true` and `embedding_dim` is set.

## Supported input formats

WAV (PCM 16/24/32), FLAC, MP3, OGG Vorbis, raw PCM (with `sample_rate` query param). Multi-channel input is downmixed to mono before fingerprinting.
