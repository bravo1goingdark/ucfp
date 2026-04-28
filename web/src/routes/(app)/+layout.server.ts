import type { LayoutServerLoad } from './$types';
import type { UsageResponse } from '$lib/types/api';

// Layout-level loader: hooks already gate `(app)/*` so `locals.user` is
// guaranteed non-null here.
export const load: LayoutServerLoad = async ({ locals, fetch }) => {
  let summary: UsageResponse | null = null;
  try {
    const res = await fetch('/api/usage?days=7');
    if (res.ok) summary = (await res.json()) as UsageResponse;
  } catch {
    summary = null;
  }
  return {
    user: locals.user!,
    summary
  };
};
