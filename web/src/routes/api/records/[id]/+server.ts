// GET    /api/records/:id  — describe (metadata only) via upstream `/v1/records/{tid}/{rid}`
// DELETE /api/records/:id  — delete via upstream `DELETE /v1/records/{tid}/{rid}`
//
// Both require authenticated identity (no anonymous tenant=0).

import { json } from '@sveltejs/kit';
import type { RequestHandler } from './$types';
import { describeRecord, deleteRecord } from '$lib/server/upstream';
import { requireAuth } from '$lib/server/requireAuth';

function isU64(s: string): boolean {
  return /^\d+$/.test(s) && s.length > 0 && s.length <= 20;
}

export const GET: RequestHandler = async (event) => {
  const env = event.platform?.env;
  if (!env?.UCFP_API_URL || !env.UCFP_API_TOKEN) {
    return json({ reason: 'upstream not configured' }, { status: 503 });
  }
  const auth = await requireAuth(event);
  if (!auth.ok) return auth.response;
  const id = event.params.id ?? '';
  if (!isU64(id)) return json({ reason: 'record id must be a u64 decimal' }, { status: 400 });

  const cfg = { apiUrl: env.UCFP_API_URL, apiToken: env.UCFP_API_TOKEN, tenantId: auth.identity.tenantId };
  let out;
  try {
    out = await describeRecord(cfg, id);
  } catch (e) {
    return json({ reason: `upstream unreachable: ${(e as Error).message}` }, { status: 502 });
  }
  if (!out.description) return json({ reason: 'not found' }, { status: out.status });
  return json(out.description);
};

export const DELETE: RequestHandler = async (event) => {
  const env = event.platform?.env;
  if (!env?.UCFP_API_URL || !env.UCFP_API_TOKEN) {
    return json({ reason: 'upstream not configured' }, { status: 503 });
  }
  const auth = await requireAuth(event);
  if (!auth.ok) return auth.response;
  const id = event.params.id ?? '';
  if (!isU64(id)) return json({ reason: 'record id must be a u64 decimal' }, { status: 400 });

  const cfg = { apiUrl: env.UCFP_API_URL, apiToken: env.UCFP_API_TOKEN, tenantId: auth.identity.tenantId };
  try {
    const out = await deleteRecord(cfg, id);
    return new Response(null, { status: out.status === 204 ? 204 : out.status });
  } catch (e) {
    return json({ reason: `upstream unreachable: ${(e as Error).message}` }, { status: 502 });
  }
};
