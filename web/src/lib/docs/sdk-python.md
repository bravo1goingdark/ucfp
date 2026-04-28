---
title: SDK — Python
order: 10
description: Install the ucfp pip package and call every modality from a synchronous, type-annotated client.
---

# Python SDK

`ucfp` is a small, synchronous wrapper around the HTTP API. Type-annotated, no runtime deps beyond `httpx`. Async client (`UcfpAsync`) ships in the same package for `asyncio` callers.

## Install

```bash
pip install ucfp
# or
uv add ucfp
```

Requires Python ≥ 3.10.

## Quick start

```python
from ucfp import Ucfp

client = Ucfp(api_key="ucfp_…")

fp = client.text("The quick brown fox.")
print(fp.algorithm, fp.fingerprint_bytes)
```

## Text

```python
fp = client.text(
    "Hello world.",
    algorithm="minhash",
    h=128,
    k=5,
    tokenizer="Word",
    canonicalizer={"case_fold": True, "normalization": "Nfkc"},
)
```

Returns a `TextFingerprint` dataclass:

```python
@dataclass
class TextFingerprint:
    record_id: str
    algorithm: str
    format_version: int
    config_hash: str
    fingerprint_bytes: int
    has_embedding: bool
    embedding_dim: int | None = None
    model_id: str | None = None
```

### Streaming

```python
for fp in client.text_stream(["doc 1", "doc 2", "doc 3"]):
    print(fp.record_id)
```

## Image

```python
from pathlib import Path

bytes_ = Path("photo.jpg").read_bytes()
fp = client.image(
    bytes_,
    algorithm="multi",
    preprocess={"max_dimension": 2048, "min_dimension": 32},
)
```

`bytes` works directly; pass a `pathlib.Path` and the client reads it for you.

## Audio

```python
fp = client.audio(
    Path("clip.wav"),
    algorithm="wang",
    sample_rate=44100,
)
```

Watermark detection:

```python
report = client.audio_watermark(Path("clip.wav"), sample_rate=44100)
if report.detected:
    print(f"Payload: {report.payload}, conf: {report.confidence:.2f}")
```

## Records

```python
meta = client.get_record(fp.record_id)
print(meta.algorithm, meta.fingerprint_bytes)

client.delete_record(fp.record_id)
```

## Errors

```python
from ucfp import UcfpError

try:
    fp = client.text(some_input)
except UcfpError as e:
    print(e.status, e.code, e.message)
    if e.status == 429:
        time.sleep((e.retry_after_ms or 1000) / 1000)
```

`UcfpError` fields: `status: int`, `code: str` (one of the [error codes](/docs/error-codes)), `message: str`, `retry_after_ms: int | None`, `record_id: str | None`.

## Configuration

```python
client = Ucfp(
    api_key="…",                       # required
    base_url="https://ucfp.dev",       # override for self-host
    timeout_s=30.0,                    # per-request
    retries=3,                         # exponential on 5xx + 429
    transport=None,                    # optional httpx.HTTPTransport for testing
)
```

## Async

```python
import asyncio
from ucfp import UcfpAsync

async def main():
    async with UcfpAsync(api_key="ucfp_…") as client:
        fp = await client.text("hello")
        print(fp.record_id)

asyncio.run(main())
```

API surface mirrors the sync client: every method is `async`, every iterator is an async iterator.

## Bulk helper

```python
from ucfp import bulk_text

results = bulk_text(
    client,
    inputs=corpus_iter(),         # any iterable of str
    concurrency=16,
    algorithm="minhash",
    h=128,
)
for fp, source in results:
    save(source.id, fp)
```

`bulk_text` (and `bulk_image`, `bulk_audio`) handles concurrency, backoff on `429`, and progress reporting.

## Self-host

```python
client = Ucfp(
    api_key="your-UCFP_TOKEN-value",
    base_url="https://ucfp.your-company.internal",
)
```

Identical surface area; the client doesn't care whether the upstream is hosted or self-run.

## Source

The package is open-source — see [github.com/bravo1goingdark/ucfp](https://github.com/bravo1goingdark/ucfp) under `clients/python/`.
