// DELETE /api/inputs/[tenant_id]/[input_id] — proxy explicit eviction
// to upstream `DELETE /v1/inputs/{tenant_id}/{input_id}`.

import { json } from '@sveltejs/kit';
import type { RequestHandler } from './$types';

export const DELETE: RequestHandler = async ({ platform, params }) => {
  const env = platform?.env;
  if (!env || !env.UCFP_API_URL || !env.UCFP_API_TOKEN) {
    return json({ error: 'UCFP_API_URL or UCFP_API_TOKEN not configured.' }, { status: 503 });
  }
  const tenantId = Number(params.tenant_id);
  const inputId = params.input_id;
  if (!Number.isFinite(tenantId) || !inputId) {
    return json({ error: 'invalid path' }, { status: 400 });
  }
  const upstream = `${env.UCFP_API_URL.replace(/\/$/, '')}/v1/inputs/${tenantId}/${inputId}`;
  let res: Response;
  try {
    res = await fetch(upstream, {
      method: 'DELETE',
      headers: {
        authorization: `Bearer ${env.UCFP_API_TOKEN}`,
        'x-ucfp-tenant': String(tenantId),
      },
    });
  } catch (e) {
    return json({ error: `upstream unreachable: ${(e as Error).message}` }, { status: 502 });
  }
  return new Response(null, { status: res.status });
};
