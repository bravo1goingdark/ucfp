// GET /healthz — edge liveness probe. Reports 200 if upstream UCFP API
// returns 200 within 1s, else 503. Used by Cloudflare's status page +
// any external uptime monitor pointed at the SvelteKit edge.

import type { RequestHandler } from './$types';

const TIMEOUT_MS = 1000;

export const GET: RequestHandler = async ({ platform }) => {
  const env = platform?.env;
  const upstreamUrl = env?.UCFP_API_URL;
  if (!upstreamUrl) {
    return new Response(
      JSON.stringify({ ok: false, reason: 'UCFP_API_URL unset' }),
      { status: 503, headers: jsonHeaders() }
    );
  }
  const ctrl = new AbortController();
  const timer = setTimeout(() => ctrl.abort(), TIMEOUT_MS);
  const t0 = Date.now();
  try {
    const res = await fetch(`${upstreamUrl.replace(/\/$/, '')}/healthz`, {
      method: 'GET',
      signal: ctrl.signal
    });
    const elapsed = Date.now() - t0;
    if (!res.ok) {
      return new Response(
        JSON.stringify({ ok: false, upstream_status: res.status, elapsed_ms: elapsed }),
        { status: 503, headers: { ...jsonHeaders(), 'retry-after': '5' } }
      );
    }
    return new Response(
      JSON.stringify({ ok: true, upstream_status: res.status, elapsed_ms: elapsed }),
      { status: 200, headers: jsonHeaders() }
    );
  } catch (e) {
    return new Response(
      JSON.stringify({
        ok: false,
        reason: (e as Error).name === 'AbortError' ? 'timeout' : (e as Error).message,
        elapsed_ms: Date.now() - t0
      }),
      { status: 503, headers: { ...jsonHeaders(), 'retry-after': '5' } }
    );
  } finally {
    clearTimeout(timer);
  }
};

function jsonHeaders(): Record<string, string> {
  return {
    'content-type': 'application/json',
    'cache-control': 'no-store'
  };
}
