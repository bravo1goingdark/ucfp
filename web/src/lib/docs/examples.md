---
title: Examples
order: 8
category: Recipes
description: End-to-end recipes — dedup a pretraining corpus, gate image uploads, and match audio against a catalogue.
---

# Examples

Three full recipes. Copy, adapt, ship.

## 1. Dedup a pretraining corpus

You have 50 M short documents. You want to drop near-duplicates before training a language model. Use MinHash + LSH for cheap bucket lookup.

```python
import requests, itertools, json

API = "https://ucfp.dev/api/fingerprint"
KEY = "ucfp_…"
HEADERS = {"Authorization": f"Bearer {KEY}", "Content-Type": "application/json"}

def fingerprint(text: str) -> dict:
    r = requests.post(API, headers=HEADERS, json={
        "text": text,
        "params": {"algorithm": "lsh", "h": 128, "k": 5}
    })
    r.raise_for_status()
    return r.json()

seen_buckets: set[tuple] = set()
kept = []
for doc in itertools.islice(corpus(), 1_000_000):
    fp = fingerprint(doc["text"])
    bucket = tuple(fp["bands"][:3])  # rough bucket key
    if bucket in seen_buckets:
        continue
    seen_buckets.add(bucket)
    kept.append(doc)

print(f"kept {len(kept):,} of {1_000_000:,}")
```

Practical notes:

- Send in batches of 100–500 with `aiohttp` to amortise TLS. The server is happy with concurrency up to your per-minute budget.
- Persist the `record_id` per kept document so you can later compute exact Jaccard between any pair via `GET /v1/records/{tid}/{rid}`.
- For full Jaccard similarity (not just bucket-equality), pull the full fingerprint with `?include=fingerprint` and compute MinHash overlap client-side.

## 2. Image dedup at upload

Every time a user uploads an image, you want to (a) reject exact dupes; (b) flag near-dupes for moderator review.

```typescript
async function ingestUpload(file: File): Promise<UploadDecision> {
  const fp = await fetch('/api/fingerprint', {
    method: 'POST',
    headers: {
      'Authorization': `Bearer ${process.env.UCFP_KEY}`,
      'Content-Type': file.type
    },
    body: file
  }).then(r => r.json());

  // 1. Exact match: same fingerprint bytes ⇒ same image.
  const exact = await db.query(
    'SELECT id FROM uploads WHERE phash = ? LIMIT 1',
    [fp.phash_hex]
  );
  if (exact.length) return { decision: 'reject_exact', existing: exact[0].id };

  // 2. Near match: Hamming distance ≤ 8 on the 64-bit pHash.
  const near = await db.query(
    'SELECT id, BIT_COUNT(phash ^ ?) AS d FROM uploads HAVING d <= 8 ORDER BY d ASC LIMIT 5',
    [fp.phash_hex]
  );
  if (near.length) return { decision: 'review', similar: near };

  // 3. Novel: persist and accept.
  await db.execute(
    'INSERT INTO uploads (id, phash, ucfp_record) VALUES (?, ?, ?)',
    [crypto.randomUUID(), fp.phash_hex, fp.record_id]
  );
  return { decision: 'accept' };
}
```

Practical notes:

- Use `algorithm=multi` for the strongest signal; if you need raw 64-bit pHash for an indexed Hamming column, request `algorithm=phash` separately.
- Storage: the 64-bit pHash fits in a `BIGINT` column. Build a Hamming-distance index by partitioning on the high 16 bits (BK-tree style) for sub-linear neighbour search.

## 3. Audio matching against a catalogue

You want a Shazam-style "what song is this?" service over a 100k-track catalogue.

Pre-fingerprint the catalogue once:

```bash
for track in catalogue/*.flac; do
  curl -sS -X POST \
    "https://ucfp.dev/v1/ingest/audio/17/$(basename "$track" .flac)?algorithm=wang&sample_rate=44100" \
    -H "Authorization: Bearer ucfp_…" \
    -H "Content-Type: audio/flac" \
    --data-binary "@$track" \
    > "/tmp/$(basename "$track" .flac).json"
done
```

Match a clip:

```bash
curl -sS -X POST \
  'https://ucfp.dev/api/query?modality=audio&algorithm=wang&top=5' \
  -H 'Authorization: Bearer ucfp_…' \
  -H 'Content-Type: audio/wav' \
  --data-binary @clip.wav
```

Response (sketch):

```json
{
  "matches": [
    { "record_id": "track_00471", "score": 0.94, "offset_ms": 12340 },
    { "record_id": "track_19022", "score": 0.41, "offset_ms": 0 }
  ]
}
```

A score above ~0.7 is a confident match for Wang landmarks. Below ~0.4 is noise.

Practical notes:

- Live recognition: use the streaming subroute `POST /v1/ingest/audio/{tid}/{rid}/stream` to keep a connection open and score against the catalogue as audio arrives.
- For pitch- or tempo-shifted versions (DJ mixes, sped-up uploads), use `algorithm=panako` instead of `wang`.
- For "songs that *sound* like this" rather than literal matches, use `algorithm=neural` with `model_id=clap-htsat`.

## More recipes

- See [API reference: text](/docs/api-reference-text), [image](/docs/api-reference-image), [audio](/docs/api-reference-audio) for every parameter.
- The [JS SDK](/docs/sdk-javascript) and [Python SDK](/docs/sdk-python) wrap these recipes for you.
