// POST /api/pipeline/inspect — proxy to upstream pipeline-stage
// inspector for text / image / audio. Modality is taken from `?modality=`.

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
  if (modality !== 'text' && modality !== 'image' && modality !== 'audio') {
    return json(
      { error: `unknown modality '${modality}' (want text|image|audio).` },
      { status: 400 }
    );
  }

  // Forward the param allowlist that the upstream handler for this
  // modality reads. Anything not in the list is silently dropped.
  const TEXT_KEYS = [
    'k','h','tokenizer','preprocess',
    'canon_normalization','canon_case_fold','canon_strip_bidi',
    'canon_strip_format','canon_apply_confusable',
    'input_id',
  ];
  const IMAGE_KEYS = ['max_input_bytes','max_dimension','min_dimension','input_id'];
  const AUDIO_KEYS = ['sample_rate','input_id'];
  const allowedKeys =
    modality === 'text'  ? TEXT_KEYS  :
    modality === 'image' ? IMAGE_KEYS :
                           AUDIO_KEYS;

  const upstreamQuery = new URLSearchParams();
  for (const k of allowedKeys) {
    const v = sp.get(k);
    if (v != null && v !== '') upstreamQuery.set(k, v);
  }

  const upstreamPath =
    modality === 'text'  ? `/v1/pipeline/inspect/text/${tenantId}`  :
    modality === 'image' ? `/v1/pipeline/inspect/image/${tenantId}` :
                           `/v1/pipeline/inspect/audio/${tenantId}`;
  const upstream =
    `${env.UCFP_API_URL.replace(/\/$/, '')}${upstreamPath}` +
    (upstreamQuery.toString() ? `?${upstreamQuery.toString()}` : '');
  const body = await request.arrayBuffer();
  const defaultCt = modality === 'text'
    ? 'text/plain; charset=utf-8'
    : 'application/octet-stream';
  let res: Response;
  try {
    res = await fetch(upstream, {
      method: 'POST',
      headers: {
        'content-type': request.headers.get('content-type') ?? defaultCt,
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
