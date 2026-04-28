---
title: SDK — JavaScript
order: 9
description: Install the @ucfp/client npm package and call every modality from Node, Bun, Deno, or the browser.
---

# JavaScript / TypeScript SDK

`@ucfp/client` is a thin, dependency-free wrapper over the HTTP API. Returns native `Promise`s. Works in Node ≥ 18, Bun, Deno, and the browser (via the demo proxy — never embed a server key in a browser bundle).

## Install

```bash
npm install @ucfp/client
# or
pnpm add @ucfp/client
# or
bun add @ucfp/client
```

## Quick start

```typescript
import { UcfpClient } from '@ucfp/client';

const ucfp = new UcfpClient({
  apiKey: process.env.UCFP_KEY!,            // ucfp_…
  baseUrl: 'https://ucfp.dev'               // optional, default
});

const fp = await ucfp.text('The quick brown fox.');
console.log(fp.algorithm, fp.fingerprintBytes);
```

## Text

```typescript
const fp = await ucfp.text('Hello world.', {
  algorithm: 'minhash',
  h: 128,
  k: 5,
  tokenizer: 'Word',
  canonicalizer: { caseFold: true, normalization: 'Nfkc' }
});
```

Returns:

```typescript
type TextFingerprint = {
  recordId: string;
  algorithm: string;
  formatVersion: number;
  configHash: string;
  fingerprintBytes: number;
  hasEmbedding: boolean;
  embeddingDim?: number;
  modelId?: string;
};
```

### Streaming

```typescript
for await (const fp of ucfp.textStream(asyncIterableOfStrings)) {
  console.log(fp.recordId);
}
```

## Image

```typescript
import { readFile } from 'node:fs/promises';

const bytes = await readFile('./photo.jpg');
const fp = await ucfp.image(bytes, {
  algorithm: 'multi',
  preprocess: { maxDimension: 2048, minDimension: 32 }
});
```

In the browser:

```typescript
async function onUpload(file: File) {
  const fp = await ucfp.image(file, { algorithm: 'phash' });
  console.log(fp.recordId);
}
```

## Audio

```typescript
const wavBytes = await readFile('./clip.wav');
const fp = await ucfp.audio(wavBytes, {
  algorithm: 'wang',
  sampleRate: 44100
});
```

Watermark detection (no record persisted):

```typescript
const report = await ucfp.audioWatermark(wavBytes, { sampleRate: 44100 });
if (report.detected) console.log('Payload:', report.payload, 'conf:', report.confidence);
```

## Records

```typescript
const meta = await ucfp.getRecord(fp.recordId);
console.log(meta.algorithm, meta.fingerprintBytes);

await ucfp.deleteRecord(fp.recordId);
```

## Errors

Every method returns a typed `Promise<…>`; failures throw `UcfpError`:

```typescript
import { UcfpError } from '@ucfp/client';

try {
  await ucfp.text(input);
} catch (e) {
  if (e instanceof UcfpError) {
    console.error(e.status, e.code, e.message);
    if (e.status === 429) await sleep(e.retryAfterMs ?? 1000);
  }
}
```

`UcfpError` fields: `status`, `code` (one of the [error codes](/docs/error-codes)), `message`, `retryAfterMs?`, `recordId?`.

## Configuration

```typescript
new UcfpClient({
  apiKey: '…',                      // required
  baseUrl: 'https://ucfp.dev',      // override for self-host
  fetch: globalThis.fetch,          // override for testing
  timeoutMs: 30_000,                // per-request, default 30s
  retry: { attempts: 3, base: 250 } // exponential backoff for 5xx + 429
});
```

## Browser caveats

Do **not** ship a server-side `apiKey` to the browser. For browser usage, point the client at your own backend that proxies to UCFP, or use the public demo path with `apiKey: undefined` (rate-limited by IP, requires Turnstile token in the body).

```typescript
const ucfp = new UcfpClient({
  baseUrl: '/api/fingerprint',  // your own proxy
  apiKey: undefined
});
```

## Self-host

Point at your own Rust binary:

```typescript
const ucfp = new UcfpClient({
  apiKey: 'your-UCFP_TOKEN-value',
  baseUrl: 'https://ucfp.your-company.internal'
});
```

The client is otherwise identical.

## Source

The package is open-source — see [github.com/bravo1goingdark/ucfp](https://github.com/bravo1goingdark/ucfp) under `clients/js/`.
