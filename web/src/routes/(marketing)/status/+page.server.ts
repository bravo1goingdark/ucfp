import type { PageServerLoad } from './$types';
import type { InfoResponse } from '$lib/types/api';

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
      checkedAt,
      info: null as InfoResponse | null
    };
  }

  const base = apiUrl.replace(/\/$/, '');
  const t0 = Date.now();

  // Probe healthz + info in parallel — fold failures so the page always
  // renders something useful even if the upstream is partially broken.
  const [healthRes, infoRes] = await Promise.allSettled([
    fetch(`${base}/healthz`, { method: 'GET', signal: AbortSignal.timeout(2000), cache: 'no-store' }),
    fetch(`${base}/v1/info`, { method: 'GET', signal: AbortSignal.timeout(2000), cache: 'no-store' })
  ]);
  const latencyMs = Date.now() - t0;

  const reachable =
    healthRes.status === 'fulfilled' && healthRes.value.ok;

  let info: InfoResponse | null = null;
  if (infoRes.status === 'fulfilled' && infoRes.value.ok) {
    try { info = await infoRes.value.json() as InfoResponse; } catch { info = null; }
  }

  return {
    configured: true,
    reachable,
    latencyMs,
    colo: getColo(request),
    checkedAt,
    info
  };
};

function getColo(request: Request): string | null {
  // request.cf is populated on Cloudflare Workers; safe-cast.
  const cf = (request as unknown as { cf?: { colo?: string } }).cf;
  return cf?.colo ?? null;
}
