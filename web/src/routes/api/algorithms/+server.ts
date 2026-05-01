// GET /api/algorithms — proxy to upstream `GET /v1/algorithms`.
//
// Returns the manifest of every algorithm the upstream binary supports,
// with a tunable schema per algorithm. The playground's TuningForm reads
// this to render its inputs generically.
//
// Cache for 5 minutes — the manifest changes only on a deploy.

import type { RequestHandler } from './$types';

export const GET: RequestHandler = async ({ platform, fetch: _fetch }) => {
  const env = platform?.env;
  if (!env || !env.UCFP_API_URL) {
    return new Response(
      JSON.stringify({ error: 'UCFP_API_URL not configured' }),
      { status: 503, headers: { 'content-type': 'application/json' } }
    );
  }
  const upstream = `${env.UCFP_API_URL.replace(/\/$/, '')}/v1/algorithms`;
  let res: Response;
  try {
    res = await fetch(upstream, { headers: { accept: 'application/json' } });
  } catch (e) {
    return new Response(
      JSON.stringify({ error: `upstream unreachable: ${(e as Error).message}` }),
      { status: 502, headers: { 'content-type': 'application/json' } }
    );
  }
  const body = await res.text();
  return new Response(body, {
    status: res.status,
    headers: {
      'content-type': res.headers.get('content-type') ?? 'application/json',
      'cache-control': 'public, max-age=300',
    },
  });
};
