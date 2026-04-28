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
  try {
    const res = await fetch(`/api/usage?days=${encodeURIComponent(days)}`);
    if (res.ok) usage = (await res.json()) as UsageResponse;
  } catch {
    usage = null;
  }
  return {
    usage,
    filters: { modality, status, before, days }
  };
};
