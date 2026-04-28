import type { PageLoad } from './$types';
import type { UsageResponse } from '$lib/types/api';

export const load: PageLoad = async ({ fetch }) => {
  let usage: UsageResponse | null = null;
  try {
    const res = await fetch('/api/usage?days=30');
    if (res.ok) usage = (await res.json()) as UsageResponse;
  } catch {
    usage = null;
  }
  return { usage };
};
