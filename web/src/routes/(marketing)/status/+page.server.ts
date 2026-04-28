import type { PageServerLoad } from './$types';

export const prerender = false;

export const load: PageServerLoad = async ({ platform, request }) => {
  const env = platform?.env;
  const apiUrl = env?.UCFP_API_URL;
  const checkedAt = Date.now();

  if (!apiUrl) {
    return {
      configured: false,
      reachable: false,
      latencyMs: null,
      colo: getColo(request),
      checkedAt
    };
  }

  const t0 = Date.now();
  let reachable = false;
  try {
    const res = await fetch(`${apiUrl.replace(/\/$/, '')}/healthz`, {
      method: 'GET',
      signal: AbortSignal.timeout(2000),
      cache: 'no-store'
    });
    reachable = res.ok;
  } catch {
    reachable = false;
  }
  const latencyMs = Date.now() - t0;

  return {
    configured: true,
    reachable,
    latencyMs,
    colo: getColo(request),
    checkedAt
  };
};

function getColo(request: Request): string | null {
  // request.cf is populated on Cloudflare Workers; safe-cast.
  const cf = (request as unknown as { cf?: { colo?: string } }).cf;
  return cf?.colo ?? null;
}
