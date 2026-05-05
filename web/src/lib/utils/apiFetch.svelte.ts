// Browser-side fetch wrapper for the SvelteKit `/api/*` routes.
// Handles the two cross-cutting concerns the playground / dashboard
// pages otherwise duplicate inline:
//
//   1. Auth expiry — when an API route returns 401 we surface a
//      "session expired" toast and redirect to /login?next=<current>
//      so the user lands back where they were after re-auth.
//   2. Rate limit — surfaces 429 + Retry-After as an info toast so
//      users don't see a generic "Request failed" when they hit the
//      bucket.
//
// Use in any browser-only context. Server (+server.ts / load) keeps
// using bare fetch + the upstreamFetch helper.

import { goto } from '$app/navigation';
import { pushToast } from '$lib/stores/toasts.svelte';

let redirectingForAuth = false;

export interface ApiFetchOpts extends RequestInit {
  /** When true, suppress the auto-redirect on 401 — the caller wants to
   *  handle it manually (login form etc). Toast still fires. */
  suppressRedirect?: boolean;
}

export async function apiFetch(
  input: string | URL | Request,
  init: ApiFetchOpts = {},
): Promise<Response> {
  const { suppressRedirect, ...rest } = init;
  const res = await fetch(input, rest);

  if (res.status === 401) {
    if (!redirectingForAuth) {
      redirectingForAuth = true;
      pushToast({
        kind: 'error',
        message: 'Session expired — please sign in again.',
        ttl: 6000,
      });
      if (!suppressRedirect && typeof window !== 'undefined') {
        const next = encodeURIComponent(window.location.pathname + window.location.search);
        // Fire after a tick so the toast has time to mount before nav.
        setTimeout(() => {
          goto(`/login?next=${next}`).finally(() => {
            redirectingForAuth = false;
          });
        }, 60);
      } else {
        // Reset the latch even when we don't redirect, otherwise a second
        // 401 in the same session never re-triggers.
        setTimeout(() => (redirectingForAuth = false), 1000);
      }
    }
  } else if (res.status === 429) {
    const retry = res.headers.get('retry-after');
    pushToast({
      kind: 'info',
      message: retry
        ? `Rate limited — try again in ${retry}s.`
        : 'Rate limited — slow down a moment.',
      ttl: 5000,
    });
  }

  return res;
}
