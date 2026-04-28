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
  has_embedding: boolean;
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

function buildHeaders(cfg: UpstreamConfig, contentType: string): HeadersInit {
  return {
    'content-type': contentType,
    authorization: `Bearer ${cfg.apiToken}`,
    'x-ucfp-tenant': String(cfg.tenantId)
  };
}

/** POST /v1/ingest/{modality}/{tenant}/{record}. */
export async function ingest(cfg: UpstreamConfig, args: IngestArgs): Promise<IngestOutcome> {
  let path = `/v1/ingest/${args.modality}/${cfg.tenantId}/${args.recordId}`;
  const qs = new URLSearchParams();
  if (args.modality === 'audio') qs.set('sample_rate', String(args.sampleRate ?? 8000));
  if (args.algorithm) qs.set('algorithm', args.algorithm);
  if (args.modelId)   qs.set('model_id',  args.modelId);
  if (args.apiKey)    qs.set('api_key',   args.apiKey);
  const qstr = qs.toString();
  if (qstr) path += '?' + qstr;

  const url = joinUrl(cfg.apiUrl, path);
  const t0 = typeof performance !== 'undefined' ? performance.now() : Date.now();

  const res = await fetch(url, {
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
): Promise<{ status: number; result: WatermarkResult; latencyMs: number }> {
  const qs = new URLSearchParams();
  qs.set('sample_rate', String(args.sampleRate ?? 8000));
  if (args.modelId) qs.set('model_id', args.modelId);
  const path = `/v1/ingest/audio/${cfg.tenantId}/${args.recordId}/watermark?${qs.toString()}`;
  const url = joinUrl(cfg.apiUrl, path);
  const t0 = typeof performance !== 'undefined' ? performance.now() : Date.now();

  const res = await fetch(url, {
    method: 'POST',
    headers: buildHeaders(cfg, args.contentType),
    body: args.body,
    signal: args.signal
  });

  const latencyMs = (typeof performance !== 'undefined' ? performance.now() : Date.now()) - t0;
  const ct = res.headers.get('content-type') ?? '';

  if (ct.includes('application/json')) {
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
  return {
    status: res.status,
    result: { detected: false, confidence: 0, payload: null },
    latencyMs
  };
}
