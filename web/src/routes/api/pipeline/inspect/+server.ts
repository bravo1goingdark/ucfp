// POST /api/pipeline/inspect — proxy to upstream pipeline-stage
// inspector. Modality is taken from `?modality=` (only `text` is wired
// upstream today; image/audio return 501 until D3.1's stage extractors
// land for those modalities).

import { json } from '@sveltejs/kit';
import type { RequestHandler } from './$types';
import { authenticateApiKey, extractApiKey } from '$lib/server/apikeyAuth';

export const POST: RequestHandler = async (event) => {
  const { request, platform } = event;
  const env = platform?.env;
  if (!env || !env.UCFP_API_URL || !env.UCFP_API_TOKEN) {
    return json(
      { error: 'UCFP_API_URL or UCFP_API_TOKEN not configured.' },
      { status: 503 }
    );
  }

  // Identity → tenant_id, mirroring /api/fingerprint's three-path scheme.
  let tenantId = 0;
  const presented = extractApiKey(request.headers);
  if (presented) {
    const auth = await authenticateApiKey(event);
    if (!auth.ok) {
      return json({ error: auth.message }, { status: auth.status });
    }
    tenantId = auth.user.tenantId;
  } else if (event.locals.user) {
    tenantId = event.locals.user.tenantId;
  }

  const sp = event.url.searchParams;
  const modality = sp.get('modality') ?? 'text';
  if (modality !== 'text') {
    return json(
      { error: `pipeline inspect for ${modality} is not implemented yet — text only.` },
      { status: 501 }
    );
  }

  // Forward the same canonicalizer + tokenizer + preprocess + input_id
  // surface the upstream inspect_text handler accepts. Numeric / enum
  // params are not pre-validated here — upstream rejects bad values.
  const upstreamQuery = new URLSearchParams();
  for (const k of [
    'k','h','tokenizer','preprocess',
    'canon_normalization','canon_case_fold','canon_strip_bidi',
    'canon_strip_format','canon_apply_confusable',
    'input_id',
  ]) {
    const v = sp.get(k);
    if (v != null && v !== '') upstreamQuery.set(k, v);
  }

  const upstream =
    `${env.UCFP_API_URL.replace(/\/$/, '')}/v1/pipeline/inspect/text/${tenantId}` +
    (upstreamQuery.toString() ? `?${upstreamQuery.toString()}` : '');
  const body = await request.arrayBuffer();
  let res: Response;
  try {
    res = await fetch(upstream, {
      method: 'POST',
      headers: {
        'content-type': request.headers.get('content-type') ?? 'text/plain; charset=utf-8',
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
      'cache-control': 'no-store',
    },
  });
};
