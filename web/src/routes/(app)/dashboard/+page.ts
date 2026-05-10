import type { PageLoad } from './$types';
import type { UsageResponse } from '$lib/types/api';

export const load: PageLoad = async ({ fetch }) => {
  let usage: UsageResponse | null = null;
  let usageError: string | null = null;
  try {
    const ctrl = new AbortController();
    const timer = setTimeout(() => ctrl.abort(), 10_000);
    const res = await fetch('/api/usage?days=30', { signal: ctrl.signal });
    clearTimeout(timer);
    if (res.ok) usage = (await res.json()) as UsageResponse;
    else usageError = `Usage API returned ${res.status}`;
  } catch (e) {
    usageError = (e as Error).name === 'AbortError'
      ? 'Usage API timed out'
      : `Usage API unreachable: ${(e as Error).message}`;
  }
  return { usage, usageError };
};
