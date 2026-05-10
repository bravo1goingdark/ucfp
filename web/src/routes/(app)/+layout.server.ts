import type { LayoutServerLoad } from './$types';
import type { UsageResponse } from '$lib/types/api';

// Layout-level loader: hooks already gate `(app)/*` so `locals.user` is
// guaranteed non-null here.
export const load: LayoutServerLoad = async ({ locals, fetch }) => {
  let summary: UsageResponse | null = null;
  try {
    const ctrl = new AbortController();
    const timer = setTimeout(() => ctrl.abort(), 8_000);
    const res = await fetch('/api/usage?days=7', { signal: ctrl.signal });
    clearTimeout(timer);
    if (res.ok) summary = (await res.json()) as UsageResponse;
  } catch {
    summary = null;
  }
  return {
    user: locals.user!,
    summary
  };
};
