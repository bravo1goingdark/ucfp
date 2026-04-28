import type { PageLoad } from './$types';
import type { KeyRow } from '$lib/types/api';

// W3 ships `GET /api/keys` returning `KeyRow[]` directly.
export const load: PageLoad = async ({ fetch }) => {
  let keys: KeyRow[] = [];
  let error: string | null = null;
  try {
    const res = await fetch('/api/keys');
    if (res.ok) {
      keys = (await res.json()) as KeyRow[];
    } else if (res.status !== 404) {
      error = `Could not load keys (${res.status}).`;
    }
  } catch {
    error = 'Could not reach the keys API.';
  }
  return { keys, error };
};
