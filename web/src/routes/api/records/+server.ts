// POST /api/records — bulk upsert raw `Record[]` to upstream `/v1/records`.
// Auth required (session or API key). Anonymous demo users get 401.
//
// Body shape: `{ records: Record[] }`. Records are passed through unchanged.

import { json, error } from '@sveltejs/kit';
import type { RequestHandler } from './$types';
import { upsertRecords } from '$lib/server/upstream';
import { requireAuth } from '$lib/server/requireAuth';
import { recordUsage } from '$lib/server/usage';

export const POST: RequestHandler = async (event) => {
  const env = event.platform?.env;
  if (!env?.UCFP_API_URL || !env.UCFP_API_TOKEN) {
    return json({ reason: 'upstream not configured' }, { status: 503 });
  }
  const auth = await requireAuth(event);
  if (!auth.ok) return auth.response;
  const { tenantId, userId, keyId } = auth.identity;

  let body: { records: unknown[] };
  try {
    body = (await event.request.json()) as { records: unknown[] };
  } catch (e) {
    error(400, `invalid JSON body: ${(e as Error).message}`);
  }
  if (!Array.isArray(body?.records) || body.records.length === 0) {
    error(400, 'body.records must be a non-empty array');
  }

  const cfg = { apiUrl: env.UCFP_API_URL, apiToken: env.UCFP_API_TOKEN, tenantId };
  let out;
  try {
    out = await upsertRecords(cfg, body.records);
  } catch (e) {
    error(502, `upstream unreachable: ${(e as Error).message}`);
  }

  // No modality on bulk upsert — record as text+bulk so the events table
  // still threads. Bytes-in is the JSON body size (rough indicator).
  const bytesIn = new TextEncoder().encode(JSON.stringify(body.records)).byteLength;
  event.platform?.context?.waitUntil?.(
    recordUsage({ db: env.DB, analytics: env.ANALYTICS }, {
      userId, apiKeyId: keyId, modality: 'text', algorithm: 'bulk-upsert',
      bytesIn, status: out.status, latencyMs: 0
    })
  );

  return new Response(
    typeof out.body === 'string' ? out.body : JSON.stringify(out.body),
    { status: out.status, headers: { 'content-type': 'application/json' } }
  );
};
