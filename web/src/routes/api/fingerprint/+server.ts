// POST /api/fingerprint — multimodal proxy to the upstream Rust ucfp server.
//
// Identity:
//   1. If an API key is presented (Bearer / X-Api-Key) → W3's
//      `authenticateApiKey(event)` resolves user, applies per-key minute
//      + daily quota. On failure (401/429/503) the result is forwarded.
//   2. No key present → anonymous demo path:
//        • require Turnstile token (when TURNSTILE_SECRET set),
//        • per-IP `checkDemoLimit` from W3's ratelimit module,
//        • tenant_id = 0.
//
// Modality (from Content-Type):
//   • multipart/form-data → first File field; modality from file.type
//   • image/*             → image, body = bytes
//   • audio/*             → audio, body = bytes (caller supplies raw f32 LE
//                            samples; the demo UI decodes via WebAudio)
//   • text/* | else       → text, body = utf-8 string
//
// On success the upstream JSON body is returned to the caller with an
// `X-Proxied-Latency` header. A usage event is recorded in the background
// via `event.platform.context.waitUntil(...)`.

import { json, error } from '@sveltejs/kit';
import type { RequestHandler } from './$types';
import { ulidU64 } from '$lib/server/ulid';
import {
  ingest,
  ingestWatermark,
  ingestTextPreprocess,
  type Modality,
  type IngestResponse,
  type AlgorithmParams
} from '$lib/server/upstream';
import { verifyTurnstile } from '$lib/server/turnstile';
import { authenticateApiKey, extractApiKey } from '$lib/server/apikeyAuth';
import { checkDemoLimit } from '$lib/server/ratelimit';
import { recordUsage, type UsageRecord } from '$lib/server/usage';

interface ParsedBody {
  modality: Modality;
  body: BodyInit;
  contentType: string;
  bytesIn: number;
  sampleRate?: number;
}

async function parseRequest(request: Request): Promise<ParsedBody> {
  const ct = (request.headers.get('content-type') ?? '').toLowerCase();

  if (ct.startsWith('multipart/form-data')) {
    const form = await request.formData();
    let file: File | null = null;
    for (const v of form.values()) {
      if (v instanceof File) { file = v; break; }
    }
    if (!file) error(400, 'multipart body must include a file field');

    const fileType = (file.type || '').toLowerCase();
    let modality: Modality;
    if (fileType.startsWith('image/')) modality = 'image';
    else if (fileType.startsWith('audio/')) modality = 'audio';
    else error(415, `unsupported file type: ${fileType || 'unknown'}`);

    const buf = new Uint8Array(await file.arrayBuffer());
    const sampleRateField = form.get('sample_rate');
    const sampleRate = sampleRateField != null ? Number(sampleRateField) : NaN;
    return {
      modality,
      body: buf,
      contentType: 'application/octet-stream',
      bytesIn: buf.byteLength,
      sampleRate: modality === 'audio' ? (Number.isFinite(sampleRate) ? sampleRate : 8000) : undefined
    };
  }

  if (ct.startsWith('image/')) {
    const buf = new Uint8Array(await request.arrayBuffer());
    if (buf.byteLength === 0) error(400, 'empty body');
    return {
      modality: 'image',
      body: buf,
      contentType: 'application/octet-stream',
      bytesIn: buf.byteLength
    };
  }

  if (ct.startsWith('audio/')) {
    const buf = new Uint8Array(await request.arrayBuffer());
    if (buf.byteLength === 0) error(400, 'empty body');
    return {
      modality: 'audio',
      body: buf,
      contentType: 'application/octet-stream',
      bytesIn: buf.byteLength,
      sampleRate: 8000
    };
  }

  // Default: treat as UTF-8 text.
  const text = await request.text();
  if (text.length === 0) error(400, 'empty body');
  return {
    modality: 'text',
    body: text,
    contentType: 'text/plain; charset=utf-8',
    bytesIn: new TextEncoder().encode(text).byteLength
  };
}

export const POST: RequestHandler = async (event) => {
  const { request, platform, getClientAddress } = event;
  const env = platform?.env;
  if (!env || !env.UCFP_API_URL || !env.UCFP_API_TOKEN) {
    return json(
      { proxied: false, reason: 'UCFP_API_URL or UCFP_API_TOKEN not configured.' },
      { status: 503 }
    );
  }

  // ── identity ─────────────────────────────────────────────────────────
  let tenantId: number;
  let userId: string | null;
  let keyId: string | null;
  const presentedKey = extractApiKey(request.headers);

  if (presentedKey) {
    const auth = await authenticateApiKey(event);
    if (!auth.ok) {
      const headers: Record<string, string> = {};
      if (auth.retryAfter) headers['retry-after'] = String(auth.retryAfter);
      return json(
        { proxied: false, reason: auth.message, retryAfter: auth.retryAfter },
        { status: auth.status, headers }
      );
    }
    tenantId = auth.user.tenantId;
    userId = auth.user.id;
    keyId = auth.keyId;
  } else if (event.locals.user) {
    // Session-authenticated path — dashboard/playground uses this.
    tenantId = event.locals.user.tenantId;
    userId = event.locals.user.id;
    keyId = null;
  } else {
    // Anonymous demo path.
    let clientIp = '0.0.0.0';
    try { clientIp = getClientAddress(); } catch { /* prerender / dev */ }

    if (env.TURNSTILE_SECRET) {
      const token = request.headers.get('x-turnstile-token');
      const result = await verifyTurnstile(env.TURNSTILE_SECRET, token, clientIp);
      if (!result.success) {
        return json({ proxied: false, reason: 'turnstile-failed' }, { status: 403 });
      }
    }
    if (env.RATE_LIMIT) {
      const rl = await checkDemoLimit(env.RATE_LIMIT, clientIp);
      if (!rl.ok) {
        return json(
          { proxied: false, reason: 'rate-limited', retryAfter: rl.retryAfter },
          { status: 429, headers: { 'retry-after': String(rl.retryAfter) } }
        );
      }
    }
    tenantId = 0;
    userId = null;
    keyId = null;
  }

  // ── modality + body ──────────────────────────────────────────────────
  const parsed = await parseRequest(request);
  const recordId = ulidU64();
  const sp = event.url.searchParams;
  const algorithmParam = sp.get('algorithm') ?? undefined;
  const modelId = sp.get('model_id') ?? undefined;
  const apiKey  = sp.get('api_key')  ?? undefined;

  // Forward every per-algorithm tunable upstream understands. Missing
  // params fall through to upstream defaults (see src/server/dto.rs).
  const algoParams: AlgorithmParams = {};
  const numKeys = [
    // common + Wang
    'k','h','fan_out','peaks_per_sec','target_zone_t','target_zone_f','min_anchor_mag_db',
    // image preprocess
    'max_dimension','max_input_bytes','min_dimension',
    // Panako
    'panako_fan_out','panako_target_zone_t','panako_target_zone_f',
    'panako_peaks_per_sec','panako_min_anchor_mag_db',
    // Haitsma
    'haitsma_fmin','haitsma_fmax',
    // Neural / Watermark
    'neural_fmax','watermark_threshold',
    // Live-tune handle
    'input_id',
  ] as const;
  for (const k of numKeys) {
    const v = sp.get(k);
    if (v != null && v !== '') {
      const n = Number(v);
      if (Number.isFinite(n)) (algoParams as Record<string, unknown>)[k] = n;
    }
  }
  const tokenizer = sp.get('tokenizer');
  if (tokenizer === 'word' || tokenizer === 'grapheme' || tokenizer === 'cjk-jp' || tokenizer === 'cjk-ko') {
    algoParams.tokenizer = tokenizer;
  }
  // Text canonicalizer knobs.
  const normalization = sp.get('canon_normalization');
  if (normalization === 'nfc' || normalization === 'nfkc' || normalization === 'none') {
    algoParams.canon_normalization = normalization;
  }
  for (const k of ['canon_case_fold','canon_strip_bidi','canon_strip_format','canon_apply_confusable'] as const) {
    const v = sp.get(k);
    if (v === 'true' || v === 'false') {
      (algoParams as Record<string, unknown>)[k] = v === 'true';
    }
  }
  if (sp.get('return_embedding') === '1') algoParams.return_embedding = true;

  const upstreamCfg = { apiUrl: env.UCFP_API_URL, apiToken: env.UCFP_API_TOKEN, tenantId };

  // ── preprocess short-circuit (text only) ─────────────────────────────
  // ?preprocess=html|markdown|pdf hits the dedicated upstream endpoint.
  const preprocessKind = sp.get('preprocess');
  if (preprocessKind === 'html' || preprocessKind === 'markdown' || preprocessKind === 'pdf') {
    if (parsed.modality !== 'text' && preprocessKind !== 'pdf') {
      error(400, `preprocess=${preprocessKind} only valid for text bodies`);
    }
    let outcome;
    try {
      outcome = await ingestTextPreprocess(upstreamCfg, {
        recordId,
        kind: preprocessKind,
        body: parsed.body,
        contentType: preprocessKind === 'pdf' ? 'application/pdf' : parsed.contentType
      });
    } catch (e) {
      error(502, `upstream unreachable: ${(e as Error).message}`);
    }
    if (userId) {
      platform?.context?.waitUntil?.(
        recordUsage({ db: env.DB, analytics: env.ANALYTICS }, {
          userId, apiKeyId: keyId, modality: 'text', algorithm: `preprocess-${preprocessKind}`,
          bytesIn: parsed.bytesIn, status: outcome.status, latencyMs: Math.round(outcome.latencyMs)
        })
      );
    }
    return new Response(
      typeof outcome.body === 'string' ? outcome.body : JSON.stringify(outcome.body),
      { status: outcome.status, headers: {
        'content-type': 'application/json',
        'x-proxied-latency': String(Math.round(outcome.latencyMs))
      }}
    );
  }

  // ── watermark special path ────────────────────────────────────────────
  if (algorithmParam === 'watermark' && parsed.modality === 'audio') {
    let wm;
    try {
      wm = await ingestWatermark(upstreamCfg, {
        recordId,
        body: parsed.body,
        contentType: parsed.contentType,
        sampleRate: parsed.sampleRate,
        modelId
      });
    } catch (e) {
      error(502, `upstream unreachable: ${(e as Error).message}`);
    }
    if (userId) {
      platform?.context?.waitUntil?.(
        recordUsage({ db: env.DB, analytics: env.ANALYTICS }, {
          userId, apiKeyId: keyId, modality: 'audio', algorithm: 'watermark',
          bytesIn: parsed.bytesIn, status: wm.status, latencyMs: Math.round(wm.latencyMs)
        })
      );
    }
    if (wm.error !== undefined) {
      // Upstream returned a non-success status (e.g. 404 because the
      // `audio-watermark` feature isn't compiled in). Surface the real
      // status + message instead of pretending we got a clean 200.
      return new Response(
        JSON.stringify({
          watermark: true,
          error: 'upstream',
          status: wm.status,
          message: wm.error.slice(0, 400)
        }),
        { status: wm.status, headers: {
            'content-type': 'application/json',
            'x-proxied-latency': String(Math.round(wm.latencyMs))
        }}
      );
    }
    return json(
      { watermark: true, detected: wm.result.detected,
        confidence: wm.result.confidence, payload: wm.result.payload },
      { status: 200, headers: {
          'content-type': 'application/json',
          'x-proxied-latency': String(Math.round(wm.latencyMs))
        }
      }
    );
  }

  // ── upstream ─────────────────────────────────────────────────────────
  let outcome;
  try {
    outcome = await ingest(upstreamCfg, {
      modality: parsed.modality,
      recordId,
      body: parsed.body,
      contentType: parsed.contentType,
      sampleRate: parsed.sampleRate,
      algorithm: algorithmParam,
      modelId,
      apiKey,
      params: algoParams
    });
  } catch (e) {
    error(502, `upstream unreachable: ${(e as Error).message}`);
  }

  // ── usage (background) ───────────────────────────────────────────────
  // Only persist a D1 row when we have an authenticated user; anonymous
  // demo traffic still goes to Analytics Engine when configured.
  const algorithm =
    typeof outcome.body === 'object' && outcome.body && 'algorithm' in outcome.body
      ? (outcome.body as IngestResponse).algorithm
      : null;
  if (userId) {
    const usageRec: UsageRecord = {
      userId,
      apiKeyId: keyId,
      modality: parsed.modality,
      algorithm,
      bytesIn: parsed.bytesIn,
      status: outcome.status,
      latencyMs: Math.round(outcome.latencyMs)
    };
    platform?.context?.waitUntil?.(
      recordUsage({ db: env.DB, analytics: env.ANALYTICS }, usageRec)
    );
  } else if (env.ANALYTICS) {
    try {
      env.ANALYTICS.writeDataPoint({
        blobs: ['demo', parsed.modality, algorithm ?? ''],
        doubles: [Math.round(outcome.latencyMs), parsed.bytesIn],
        indexes: ['anonymous']
      });
    } catch { /* non-fatal */ }
  }

  // ── reply ────────────────────────────────────────────────────────────
  const headers: Record<string, string> = {
    'content-type': 'application/json',
    'x-proxied-latency': String(Math.round(outcome.latencyMs))
  };
  return new Response(
    typeof outcome.body === 'string' ? outcome.body : JSON.stringify(outcome.body),
    { status: outcome.status, headers }
  );
};
