// POST /api/inputs — proxy to upstream `POST /v1/inputs?…`
// DELETE /api/inputs/[tenant_id]/[input_id] — handled by the dynamic route
//   `+server.ts` next to it; this file only handles the POST entry.
//
// Caches a payload server-side so the playground's live-tune flow can
// re-fingerprint with new opts without re-uploading the bytes on each
// slider tick. Authentication / tenant scoping mirrors /api/fingerprint.

import { json } from '@sveltejs/kit';
import type { RequestHandler } from './$types';
import { extractApiKey, authenticateApiKey } from '$lib/server/apikeyAuth';

export const POST: RequestHandler = async (event) => {
  const { request, platform } = event;
  const env = platform?.env;
  if (!env || !env.UCFP_API_URL || !env.UCFP_API_TOKEN) {
    return json(
      { proxied: false, reason: 'UCFP_API_URL or UCFP_API_TOKEN not configured.' },
      { status: 503 }
    );
  }

  // Identity → tenant_id (mirror /api/fingerprint's three-path identity).
  let tenantId = 0;
  const presented = extractApiKey(request.headers);
  if (presented) {
    const auth = await authenticateApiKey(event);
    if (!auth.ok) {
      const headers: Record<string, string> = {};
      if (auth.retryAfter) headers['retry-after'] = String(auth.retryAfter);
      return json(
        { proxied: false, reason: auth.message },
        { status: auth.status, headers }
      );
    }
    tenantId = auth.user.tenantId;
  } else if (event.locals.user) {
    tenantId = event.locals.user.tenantId;
  }

  const sp = event.url.searchParams;
  const modality = sp.get('modality') ?? '';
  if (!['text', 'image', 'audio'].includes(modality)) {
    return json({ error: 'modality must be one of text|image|audio' }, { status: 400 });
  }
  const sampleRate = sp.get('sample_rate');
  const upstreamQuery = new URLSearchParams({
    tenant_id: String(tenantId),
    modality,
  });
  if (sampleRate != null && sampleRate !== '') upstreamQuery.set('sample_rate', sampleRate);

  const body = await request.arrayBuffer();
  if (body.byteLength === 0) {
    return json({ error: 'empty body' }, { status: 400 });
  }

  const upstream = `${env.UCFP_API_URL.replace(/\/$/, '')}/v1/inputs?${upstreamQuery.toString()}`;
  let res: Response;
  try {
    res = await fetch(upstream, {
      method: 'POST',
      headers: {
        'content-type': request.headers.get('content-type') ?? 'application/octet-stream',
        authorization: `Bearer ${env.UCFP_API_TOKEN}`,
        'x-ucfp-tenant': String(tenantId),
      },
      body,
    });
  } catch (e) {
    return json(
      { error: `upstream unreachable: ${(e as Error).message}` },
      { status: 502 }
    );
  }
  const text = await res.text();
  return new Response(text, {
    status: res.status,
    headers: {
      'content-type': res.headers.get('content-type') ?? 'application/json',
      // Inputs cache is per-tenant and short-lived; no caching at this layer.
      'cache-control': 'no-store',
    },
  });
};
