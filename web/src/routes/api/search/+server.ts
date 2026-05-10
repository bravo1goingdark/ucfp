// POST /api/search — vector kNN against upstream `/v1/query`.
// Body: { modality: 'text'|'image'|'audio', k: number, vector: number[] }
// Auth required. Counted as `op=search` in usage events.

import { json } from '@sveltejs/kit';
import type { RequestHandler } from './$types';
import { query, type Modality } from '$lib/server/upstream';
import { requireAuth } from '$lib/server/requireAuth';
import { recordUsage } from '$lib/server/usage';

const MAX_K = 100;

export const POST: RequestHandler = async (event) => {
  const env = event.platform?.env;
  if (!env?.UCFP_API_URL || !env.UCFP_API_TOKEN) {
    return json({ error: 'not_configured', message: 'upstream not configured' }, { status: 503 });
  }
  const auth = await requireAuth(event);
  if (!auth.ok) return auth.response;
  const { tenantId, userId, keyId } = auth.identity;

  let body: { modality: Modality; k: number; vector: number[] };
  try {
    body = (await event.request.json()) as { modality: Modality; k: number; vector: number[] };
  } catch (e) {
    return json({ error: 'bad_request', message: `invalid JSON body: ${(e as Error).message}` }, { status: 400 });
  }
  if (body.modality !== 'text' && body.modality !== 'image' && body.modality !== 'audio') {
    return json({ error: 'bad_request', message: 'modality must be text|image|audio' }, { status: 400 });
  }
  if (!Array.isArray(body.vector) || body.vector.length === 0) {
    return json({ error: 'bad_request', message: 'vector must be a non-empty number array' }, { status: 400 });
  }
  for (const v of body.vector) {
    if (typeof v !== 'number' || !Number.isFinite(v)) {
      return json({ error: 'bad_request', message: 'vector must contain only finite numbers' }, { status: 400 });
    }
  }
  const k = Math.min(MAX_K, Math.max(1, Math.floor(Number(body.k) || 10)));

  const cfg = { apiUrl: env.UCFP_API_URL, apiToken: env.UCFP_API_TOKEN, tenantId };
  const explain = event.url.searchParams.get('explain') === '1';
  let out;
  try {
    out = await query(cfg, { modality: body.modality, k, vector: body.vector, explain });
  } catch (e) {
    return json({ error: 'upstream_unreachable', message: `upstream unreachable: ${(e as Error).message}` }, { status: 502 });
  }

  // Record search usage so the dashboard can show search vs ingest split.
  const bytesIn = body.vector.length * 4; // f32 estimate
  event.platform?.context?.waitUntil?.(
    recordUsage({ db: env.DB, analytics: env.ANALYTICS }, {
      userId, apiKeyId: keyId, modality: body.modality, algorithm: 'search-knn',
      bytesIn, status: out.status, latencyMs: Math.round(out.latencyMs)
    })
  );

  return new Response(
    typeof out.body === 'string' ? out.body : JSON.stringify(out.body),
    { status: out.status, headers: {
      'content-type': 'application/json',
      'x-proxied-latency': String(Math.round(out.latencyMs))
    }}
  );
};
