import type { PageLoad } from './$types';
import type { UsageResponse } from '$lib/types/api';

// `/api/usage` (W3) currently accepts only `?days=N`. The dashboard
// surfaces modality/status/before controls — those filter client-side
// against the rolling event window the API returns.
export const load: PageLoad = async ({ fetch, url }) => {
  const days = url.searchParams.get('days') ?? '30';
  const modality = url.searchParams.get('modality');
  const status = url.searchParams.get('status');
  const before = url.searchParams.get('before');

  let usage: UsageResponse | null = null;
  let usageError: string | null = null;
  try {
    const ctrl = new AbortController();
    const timer = setTimeout(() => ctrl.abort(), 10_000);
    const res = await fetch(`/api/usage?days=${encodeURIComponent(days)}`, { signal: ctrl.signal });
    clearTimeout(timer);
    if (res.ok) usage = (await res.json()) as UsageResponse;
    else usageError = `Usage API returned ${res.status}`;
  } catch (e) {
    usageError = (e as Error).name === 'AbortError'
      ? 'Usage API timed out'
      : `Usage API unreachable: ${(e as Error).message}`;
  }
  return {
    usage,
    usageError,
    filters: { modality, status, before, days }
  };
};
