// Client-side error hook. Posts unhandled errors to /api/_errors so they
// land in the Workers Analytics Engine dataset alongside server errors.
//
// Sent via `navigator.sendBeacon` when available (so it survives page
// unload), with a fetch fallback. Best-effort; never throws back into
// SvelteKit because the renderer needs to recover gracefully.

import type { HandleClientError } from '@sveltejs/kit';

interface ClientErrorPayload {
  id: string;
  url: string;
  message: string;
  stack: string;
  ua: string;
}

function send(payload: ClientErrorPayload): void {
  try {
    const json = JSON.stringify(payload);
    if (typeof navigator !== 'undefined' && 'sendBeacon' in navigator) {
      const blob = new Blob([json], { type: 'application/json' });
      navigator.sendBeacon('/api/_errors', blob);
      return;
    }
    void fetch('/api/_errors', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: json,
      keepalive: true,
    });
  } catch {
    /* never let monitoring break the page */
  }
}

export const handleError: HandleClientError = ({ error, event, status }) => {
  const id = (typeof crypto !== 'undefined' && 'randomUUID' in crypto)
    ? crypto.randomUUID()
    : Math.random().toString(36).slice(2);
  const err = error as { name?: string; message?: string; stack?: string };
  // eslint-disable-next-line no-console
  console.error(`[${id}] ${event.url.pathname}:`, err);
  send({
    id,
    url: event.url.pathname,
    message: `${err?.name ?? 'Error'}: ${(err?.message ?? '').slice(0, 256)}`,
    stack: (err?.stack ?? '').slice(0, 1024),
    ua: typeof navigator !== 'undefined' ? navigator.userAgent.slice(0, 200) : '',
  });
  return { message: 'Something went wrong.', id, status };
};
