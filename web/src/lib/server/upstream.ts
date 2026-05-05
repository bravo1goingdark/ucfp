// Typed fetch wrapper for the upstream Rust UCFP HTTP API.
//
// Always sets:
//   Authorization: Bearer ${UCFP_API_TOKEN}
//   X-Ucfp-Tenant: ${tenantId}
//
// Responsibilities are intentionally thin — callers (W5's `/api/fingerprint`,
// future dashboard helpers) decide modality, body shape, and how to surface
// errors.  The wrapper just normalises auth headers and ergonomics.

export type Modality = 'text' | 'image' | 'audio';

/** Mirrors the Rust `IngestResponse` (server/dto.rs). */
export interface IngestResponse {
  tenant_id: number;
  record_id: number | string; // u64 — JSON.parse may yield string for safety
  modality: 'text' | 'image' | 'audio';
  format_version: number;
  algorithm: string;
  config_hash: number | string;
  fingerprint_bytes: number;
  fingerprint_hex: string;
  has_embedding: boolean;
  /** Populated only when the request carries `?return_embedding=1` and the
   *  algorithm produces a dense vector (semantic-* / neural / image-semantic). */
  embedding?: number[];
}

/** Mirrors `FingerprintDescription` (dto.rs:363) — metadata only. */
export interface FingerprintDescription {
  tenant_id: number;
  record_id: number | string;
  modality: Modality;
  algorithm: string;
  format_version: number;
  config_hash: number | string;
  fingerprint_bytes: number;
  has_embedding: boolean;
  embedding_dim: number | null;
  model_id: string | null;
  metadata_bytes: number;
}

/** One ranked hit from `POST /v1/query`. */
export interface QueryHit {
  tenant_id: number;
  record_id: number | string;
  score: number;
  source: 'vector' | 'bm25' | 'filter' | 'reranker' | 'fused';
  /** Hybrid-only: vector ranker contribution to the fused RRF score. */
  vector_score?: number;
  /** Hybrid-only: BM25 ranker contribution to the fused RRF score. */
  bm25_score?: number;
  /** Hybrid-only: 1-indexed rank from the vector ranker. */
  vector_rank?: number;
  /** Hybrid-only: 1-indexed rank from the BM25 ranker. */
  bm25_rank?: number;
  /** BM25 explainability — populated when `?explain=1` is on the query. */
  term_hits?: QueryTermHit[];
}

export interface QueryTermHit {
  term: string;
  idf: number;
  tf: number;
  contribution: number;
}

export interface QueryResponse {
  hits: QueryHit[];
}

/** Mirrors `InfoResponse` (dto.rs). */
export interface InfoResponse {
  format_version: number;
  crate_version: string;
}

/** Mirrors `UpsertResponse` (dto.rs). */
export interface UpsertResponse {
  upserted: number;
}

/** Per-algorithm tunables forwarded to upstream as query params. Each
 *  field is optional; missing fields fall back to upstream defaults. */
export interface AlgorithmParams {
  // text
  k?: number;
  h?: number;
  tokenizer?: 'word' | 'grapheme' | 'cjk-jp' | 'cjk-ko';
  preprocess?: 'html' | 'markdown' | 'pdf';
  // text canonicalizer overrides
  canon_normalization?: 'nfc' | 'nfkc' | 'none';
  canon_case_fold?: boolean;
  canon_strip_bidi?: boolean;
  canon_strip_format?: boolean;
  canon_apply_confusable?: boolean;
  // audio Wang
  fan_out?: number;
  peaks_per_sec?: number;
  target_zone_t?: number;
  target_zone_f?: number;
  min_anchor_mag_db?: number;
  // audio Panako
  panako_fan_out?: number;
  panako_target_zone_t?: number;
  panako_target_zone_f?: number;
  panako_peaks_per_sec?: number;
  panako_min_anchor_mag_db?: number;
  // audio Haitsma
  haitsma_fmin?: number;
  haitsma_fmax?: number;
  // audio Neural / Watermark
  neural_fmax?: number;
  watermark_threshold?: number;
  // image preprocess
  max_dimension?: number;
  max_input_bytes?: number;
  min_dimension?: number;
  /** When true, the upstream response includes the dense embedding. */
  return_embedding?: boolean;
  /** Live-tune handle from a prior `POST /v1/inputs`. When set, upstream
   *  uses the cached bytes instead of the request body. */
  input_id?: number;
}

export interface UpstreamConfig {
  apiUrl: string;   // e.g. "https://api.ucfp.example.com"
  apiToken: string; // service bearer (UCFP_API_TOKEN)
  tenantId: number; // 0 for anonymous demo
}

export interface IngestArgs {
  modality: Modality;
  recordId: string;        // u64 decimal (see ulidU64)
  body: BodyInit;          // Bytes for image/audio; UTF-8 string for text
  contentType: string;     // e.g. "application/octet-stream", "text/plain"
  /** Audio-only: required by upstream `?sample_rate=` query parameter. */
  sampleRate?: number;
  /** Optional algorithm override (kebab-case, e.g. "simhash-tf"). */
  algorithm?: string;
  /** ONNX model path or provider model ID (e.g. "text-embedding-3-small"). */
  modelId?: string;
  /** API key for cloud semantic providers (OpenAI / Voyage / Cohere). */
  apiKey?: string;
  /** Optional per-algorithm tunables — appended as query params. */
  params?: AlgorithmParams;
  /** Optional AbortSignal for caller-side cancellation. */
  signal?: AbortSignal;
}

/** Returned by the upstream watermark detection endpoint. */
export interface WatermarkResult {
  detected: boolean;
  confidence: number;
  payload: string | null; // hex-encoded payload bytes, or null
}

export interface IngestOutcome {
  ok: boolean;
  status: number;
  /** Parsed body when JSON; raw text otherwise. */
  body: IngestResponse | string;
  /** Wall-clock ms spent in the upstream `fetch`. */
  latencyMs: number;
}

function joinUrl(base: string, path: string): string {
  return `${base.replace(/\/$/, '')}${path}`;
}

function genTraceId(): string {
  // Fast 128-bit hex id; UUIDv4-shaped without the dashes.
  if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
    return crypto.randomUUID();
  }
  const a = new Uint8Array(16);
  if (typeof crypto !== 'undefined' && crypto.getRandomValues) {
    crypto.getRandomValues(a);
  } else {
    for (let i = 0; i < 16; i++) a[i] = (Math.random() * 256) | 0;
  }
  // RFC4122 v4 layout
  a[6] = (a[6] & 0x0f) | 0x40;
  a[8] = (a[8] & 0x3f) | 0x80;
  let s = '';
  for (let i = 0; i < 16; i++) {
    s += a[i].toString(16).padStart(2, '0');
    if (i === 3 || i === 5 || i === 7 || i === 9) s += '-';
  }
  return s;
}

interface UpstreamFetchOpts {
  /** Permit retry on 5xx / network error. POSTs default false; GET/DELETE true. */
  idempotent?: boolean;
  /** Max retries (default 3). */
  retries?: number;
}

const RETRY_BACKOFF_MS = [120, 480, 1440];

/**
 * `fetch` wrapper that:
 *  - injects an `X-Request-Id` trace header on every request (echoed by upstream),
 *  - retries on 5xx and network errors with exponential backoff + jitter, but
 *    only when `idempotent` is set or the method is GET/DELETE,
 *  - propagates AbortSignal.
 *
 * Callers downstream (handlers in `routes/api/*+server.ts`) can read the
 * trace id off the response with `res.headers.get('x-request-id')` and
 * forward it back to the browser for correlation.
 */
export async function upstreamFetch(
  url: string | URL,
  init: RequestInit & { signal?: AbortSignal | null } = {},
  opts: UpstreamFetchOpts = {},
): Promise<Response> {
  const method = (init.method ?? 'GET').toUpperCase();
  const isIdempotent =
    opts.idempotent ??
    (method === 'GET' || method === 'HEAD' || method === 'DELETE' || method === 'OPTIONS');
  const maxRetries = isIdempotent ? Math.min(Math.max(0, opts.retries ?? 3), 3) : 0;
  const traceId = genTraceId();

  const headers = new Headers(init.headers ?? {});
  if (!headers.has('x-request-id')) headers.set('x-request-id', traceId);

  let attempt = 0;
  let lastError: unknown;
  while (true) {
    try {
      const res = await fetch(url, { ...init, headers });
      // Retry on transient upstream errors (5xx). 408/429 are deliberately
      // *not* retried here — 408 means client timed out (re-issue costs us
      // double bandwidth) and 429 is rate-limiting (caller should respect
      // Retry-After instead of hammering).
      if (res.status >= 500 && res.status <= 599 && attempt < maxRetries) {
        attempt++;
        await sleepWithJitter(RETRY_BACKOFF_MS[Math.min(attempt - 1, RETRY_BACKOFF_MS.length - 1)]);
        continue;
      }
      return res;
    } catch (err) {
      // Don't retry an explicit abort — the caller cancelled.
      if (err instanceof DOMException && err.name === 'AbortError') throw err;
      lastError = err;
      if (attempt < maxRetries) {
        attempt++;
        await sleepWithJitter(RETRY_BACKOFF_MS[Math.min(attempt - 1, RETRY_BACKOFF_MS.length - 1)]);
        continue;
      }
      throw lastError;
    }
  }
}

async function sleepWithJitter(baseMs: number): Promise<void> {
  const jitter = Math.random() * baseMs * 0.3;
  await new Promise((r) => setTimeout(r, baseMs + jitter));
}

function buildHeaders(cfg: UpstreamConfig, contentType: string): HeadersInit {
  return {
    'content-type': contentType,
    authorization: `Bearer ${cfg.apiToken}`,
    'x-ucfp-tenant': String(cfg.tenantId)
  };
}

function appendParams(qs: URLSearchParams, params?: AlgorithmParams): void {
  if (!params) return;
  for (const [k, v] of Object.entries(params)) {
    if (v == null || v === '') continue;
    // Rust serde deserialises `Option<bool>` from a query string as
    // `true`/`false` only — `1`/`0` is rejected with "not `true` or `false`".
    qs.set(k, typeof v === 'boolean' ? (v ? 'true' : 'false') : String(v));
  }
}

/** POST /v1/ingest/{modality}/{tenant}/{record}. */
export async function ingest(cfg: UpstreamConfig, args: IngestArgs): Promise<IngestOutcome> {
  let path = `/v1/ingest/${args.modality}/${cfg.tenantId}/${args.recordId}`;
  const qs = new URLSearchParams();
  if (args.modality === 'audio') qs.set('sample_rate', String(args.sampleRate ?? 8000));
  if (args.algorithm) qs.set('algorithm', args.algorithm);
  if (args.modelId)   qs.set('model_id',  args.modelId);
  if (args.apiKey)    qs.set('api_key',   args.apiKey);
  appendParams(qs, args.params);
  const qstr = qs.toString();
  if (qstr) path += '?' + qstr;

  const url = joinUrl(cfg.apiUrl, path);
  const t0 = typeof performance !== 'undefined' ? performance.now() : Date.now();

  const res = await upstreamFetch(url, {
    method: 'POST',
    headers: buildHeaders(cfg, args.contentType),
    body: args.body,
    signal: args.signal
  });

  const latencyMs =
    (typeof performance !== 'undefined' ? performance.now() : Date.now()) - t0;

  const ct = res.headers.get('content-type') ?? '';
  if (ct.includes('application/json')) {
    const parsed = (await res.json()) as IngestResponse;
    return { ok: res.ok, status: res.status, body: parsed, latencyMs };
  }
  const text = await res.text();
  return { ok: res.ok, status: res.status, body: text, latencyMs };
}

/** POST /v1/ingest/audio/{tenant}/{record}/watermark — detection only, no Record upserted. */
export async function ingestWatermark(
  cfg: UpstreamConfig,
  args: {
    recordId: string;
    body: BodyInit;
    contentType: string;
    sampleRate?: number;
    modelId?: string;
    signal?: AbortSignal;
  }
): Promise<
  | { status: number; result: WatermarkResult; latencyMs: number; error?: undefined }
  | { status: number; error: string; latencyMs: number; result?: undefined }
> {
  const qs = new URLSearchParams();
  qs.set('sample_rate', String(args.sampleRate ?? 8000));
  if (args.modelId) qs.set('model_id', args.modelId);
  const path = `/v1/ingest/audio/${cfg.tenantId}/${args.recordId}/watermark?${qs.toString()}`;
  const url = joinUrl(cfg.apiUrl, path);
  const t0 = typeof performance !== 'undefined' ? performance.now() : Date.now();

  const res = await upstreamFetch(url, {
    method: 'POST',
    headers: buildHeaders(cfg, args.contentType),
    body: args.body,
    signal: args.signal
  });

  const latencyMs = (typeof performance !== 'undefined' ? performance.now() : Date.now()) - t0;
  const ct = res.headers.get('content-type') ?? '';

  if (res.ok && ct.includes('application/json')) {
    const raw = (await res.json()) as {
      detected?: boolean;
      confidence?: number;
      payload?: number[] | null;
    };
    const payloadHex = Array.isArray(raw.payload) && raw.payload.length > 0
      ? raw.payload.map((b: number) => b.toString(16).padStart(2, '0')).join('')
      : null;
    return {
      status: res.status,
      result: { detected: raw.detected ?? false, confidence: raw.confidence ?? 0, payload: payloadHex },
      latencyMs
    };
  }
  // Upstream returned an error (e.g. 404 because `audio-watermark` feature is
  // off, or 501 with a clear "feature required" message). Surface the raw
  // body so the caller can forward an honest error instead of a fake "no
  // watermark detected" 200.
  return {
    status: res.status,
    error: await res.text().catch(() => ''),
    latencyMs
  };
}

// ── Records / search / info — wraps the rest of the upstream surface ──

/** GET /v1/records/{tenant}/{record} — metadata only. */
export async function describeRecord(
  cfg: UpstreamConfig,
  recordId: string,
  signal?: AbortSignal
): Promise<{ status: number; description: FingerprintDescription | null }> {
  const url = joinUrl(cfg.apiUrl, `/v1/records/${cfg.tenantId}/${recordId}`);
  const res = await upstreamFetch(url, {
    method: 'GET',
    headers: { authorization: `Bearer ${cfg.apiToken}`, 'x-ucfp-tenant': String(cfg.tenantId) },
    signal
  });
  if (!res.ok) return { status: res.status, description: null };
  return { status: res.status, description: (await res.json()) as FingerprintDescription };
}

/** DELETE /v1/records/{tenant}/{record}. */
export async function deleteRecord(
  cfg: UpstreamConfig,
  recordId: string,
  signal?: AbortSignal
): Promise<{ status: number }> {
  const url = joinUrl(cfg.apiUrl, `/v1/records/${cfg.tenantId}/${recordId}`);
  const res = await upstreamFetch(url, {
    method: 'DELETE',
    headers: { authorization: `Bearer ${cfg.apiToken}`, 'x-ucfp-tenant': String(cfg.tenantId) },
    signal
  });
  return { status: res.status };
}

/** POST /v1/records — bulk upsert raw `Record[]`. */
export async function upsertRecords(
  cfg: UpstreamConfig,
  records: unknown[],
  signal?: AbortSignal
): Promise<{ status: number; body: UpsertResponse | string }> {
  const url = joinUrl(cfg.apiUrl, '/v1/records');
  const res = await upstreamFetch(url, {
    method: 'POST',
    headers: buildHeaders(cfg, 'application/json'),
    body: JSON.stringify({ records }),
    signal
  });
  const ct = res.headers.get('content-type') ?? '';
  if (ct.includes('application/json')) {
    return { status: res.status, body: (await res.json()) as UpsertResponse };
  }
  return { status: res.status, body: await res.text() };
}

/** POST /v1/query — vector kNN. */
export async function query(
  cfg: UpstreamConfig,
  q: { modality: Modality; k: number; vector: number[]; explain?: boolean },
  signal?: AbortSignal
): Promise<{ status: number; body: QueryResponse | string; latencyMs: number }> {
  const path = q.explain ? '/v1/query?explain=1' : '/v1/query';
  const url = joinUrl(cfg.apiUrl, path);
  const t0 = typeof performance !== 'undefined' ? performance.now() : Date.now();
  const res = await upstreamFetch(url, {
    method: 'POST',
    headers: buildHeaders(cfg, 'application/json'),
    body: JSON.stringify({ tenant_id: cfg.tenantId, modality: q.modality, k: q.k, vector: q.vector }),
    signal
  });
  const latencyMs = (typeof performance !== 'undefined' ? performance.now() : Date.now()) - t0;
  const ct = res.headers.get('content-type') ?? '';
  if (ct.includes('application/json')) {
    return { status: res.status, body: (await res.json()) as QueryResponse, latencyMs };
  }
  return { status: res.status, body: await res.text(), latencyMs };
}

/** GET /v1/info — public, no auth. */
export async function getInfo(
  cfg: Pick<UpstreamConfig, 'apiUrl'>,
  signal?: AbortSignal
): Promise<{ status: number; info: InfoResponse | null }> {
  const res = await upstreamFetch(joinUrl(cfg.apiUrl, '/v1/info'), { method: 'GET', signal });
  if (!res.ok) return { status: res.status, info: null };
  return { status: res.status, info: (await res.json()) as InfoResponse };
}

/** POST /v1/ingest/text/{tenant}/{record}/preprocess/{kind}. */
export async function ingestTextPreprocess(
  cfg: UpstreamConfig,
  args: {
    recordId: string;
    kind: 'html' | 'markdown' | 'pdf';
    body: BodyInit;
    contentType: string;
    signal?: AbortSignal;
  }
): Promise<IngestOutcome> {
  const path = `/v1/ingest/text/${cfg.tenantId}/${args.recordId}/preprocess/${args.kind}`;
  const url = joinUrl(cfg.apiUrl, path);
  const t0 = typeof performance !== 'undefined' ? performance.now() : Date.now();
  const res = await upstreamFetch(url, {
    method: 'POST',
    headers: buildHeaders(cfg, args.contentType),
    body: args.body,
    signal: args.signal
  });
  const latencyMs = (typeof performance !== 'undefined' ? performance.now() : Date.now()) - t0;
  const ct = res.headers.get('content-type') ?? '';
  if (ct.includes('application/json')) {
    return { ok: res.ok, status: res.status, body: (await res.json()) as IngestResponse, latencyMs };
  }
  return { ok: res.ok, status: res.status, body: await res.text(), latencyMs };
}

/** POST /v1/ingest/text/{tenant}/{record}/stream — NDJSON. */
export async function ingestTextStream(
  cfg: UpstreamConfig,
  args: { recordId: string; ndjson: BodyInit; params?: AlgorithmParams; signal?: AbortSignal }
): Promise<IngestOutcome> {
  let path = `/v1/ingest/text/${cfg.tenantId}/${args.recordId}/stream`;
  const qs = new URLSearchParams();
  appendParams(qs, args.params);
  const qstr = qs.toString();
  if (qstr) path += '?' + qstr;
  const url = joinUrl(cfg.apiUrl, path);
  const t0 = typeof performance !== 'undefined' ? performance.now() : Date.now();
  const res = await upstreamFetch(url, {
    method: 'POST',
    headers: buildHeaders(cfg, 'application/x-ndjson'),
    body: args.ndjson,
    signal: args.signal
  });
  const latencyMs = (typeof performance !== 'undefined' ? performance.now() : Date.now()) - t0;
  const ct = res.headers.get('content-type') ?? '';
  if (ct.includes('application/json')) {
    return { ok: res.ok, status: res.status, body: (await res.json()) as IngestResponse, latencyMs };
  }
  return { ok: res.ok, status: res.status, body: await res.text(), latencyMs };
}

/** POST /v1/ingest/audio/{tenant}/{record}/stream — multipart. */
export async function ingestAudioStream(
  cfg: UpstreamConfig,
  args: { recordId: string; multipart: FormData; sampleRate: number; params?: AlgorithmParams; signal?: AbortSignal }
): Promise<IngestOutcome> {
  let path = `/v1/ingest/audio/${cfg.tenantId}/${args.recordId}/stream`;
  const qs = new URLSearchParams();
  qs.set('sample_rate', String(args.sampleRate));
  appendParams(qs, args.params);
  path += '?' + qs.toString();
  const url = joinUrl(cfg.apiUrl, path);
  const t0 = typeof performance !== 'undefined' ? performance.now() : Date.now();
  const res = await upstreamFetch(url, {
    method: 'POST',
    headers: { authorization: `Bearer ${cfg.apiToken}`, 'x-ucfp-tenant': String(cfg.tenantId) },
    body: args.multipart,
    signal: args.signal
  });
  const latencyMs = (typeof performance !== 'undefined' ? performance.now() : Date.now()) - t0;
  const ct = res.headers.get('content-type') ?? '';
  if (ct.includes('application/json')) {
    return { ok: res.ok, status: res.status, body: (await res.json()) as IngestResponse, latencyMs };
  }
  return { ok: res.ok, status: res.status, body: await res.text(), latencyMs };
}
