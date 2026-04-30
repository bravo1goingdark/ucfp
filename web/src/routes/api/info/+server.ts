// GET /api/info — public passthrough to upstream `/v1/info`. Cached 30s.
import { json } from '@sveltejs/kit';
import type { RequestHandler } from './$types';
import { getInfo } from '$lib/server/upstream';

export const GET: RequestHandler = async ({ platform }) => {
  const env = platform?.env;
  if (!env || !env.UCFP_API_URL) {
    return json({ configured: false }, { status: 503 });
  }
  try {
    const out = await getInfo({ apiUrl: env.UCFP_API_URL });
    if (!out.info) return json({ configured: true, ok: false, status: out.status }, { status: out.status });
    return json(out.info, {
      status: 200,
      headers: { 'cache-control': 'public, max-age=30, s-maxage=30' }
    });
  } catch (e) {
    return json({ configured: true, ok: false, error: (e as Error).message }, { status: 502 });
  }
};
