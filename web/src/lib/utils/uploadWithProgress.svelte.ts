// XMLHttpRequest-based upload that streams progress to a callback.
// Mirrors the apiFetch error-handling: surfaces 401 / 429 / 5xx through
// a Response-like result, and pushes toasts for auth expiry +
// rate-limit so the playground doesn't have to repeat that wiring.
//
// Use this for any /api/* call whose body is a multi-MB image or audio
// blob. Falls back to fetch transparently for tiny bodies (<32 KiB)
// where progress would only flicker.

import { goto } from '$app/navigation';
import { pushToast } from '$lib/stores/toasts.svelte';

export interface UploadOpts {
  method?: string;
  headers?: Record<string, string>;
  body: BodyInit;
  /** 0..1 monotonically increasing during upload. Fires once at the
   *  end with 1 even for non-progressable bodies. */
  onProgress?: (fraction: number) => void;
  signal?: AbortSignal;
  /** Suppress 401 → /login redirect (caller handles it). */
  suppressRedirect?: boolean;
}

export interface UploadResult {
  ok: boolean;
  status: number;
  statusText: string;
  headers: Headers;
  text: () => Promise<string>;
  json: <T = unknown>() => Promise<T>;
}

const SMALL_BODY_THRESHOLD = 32 * 1024;
let redirectingForAuth = false;

export async function uploadWithProgress(url: string, opts: UploadOpts): Promise<UploadResult> {
  const bodyLen = inferBodyLength(opts.body);

  // Tiny bodies: skip XHR — fetch is cheaper and progress is a one-frame
  // flicker anyway. Still emit progress=1 at the end for caller parity.
  if (bodyLen >= 0 && bodyLen < SMALL_BODY_THRESHOLD) {
    const res = await fetch(url, {
      method: opts.method ?? 'POST',
      headers: opts.headers,
      body: opts.body,
      signal: opts.signal ?? null,
    });
    opts.onProgress?.(1);
    return wrapFetchResponse(res, opts);
  }

  return new Promise<UploadResult>((resolve, reject) => {
    const xhr = new XMLHttpRequest();
    xhr.open(opts.method ?? 'POST', url, true);
    if (opts.headers) {
      for (const [k, v] of Object.entries(opts.headers)) {
        xhr.setRequestHeader(k, v);
      }
    }
    xhr.responseType = 'text';

    if (opts.signal) {
      const onAbort = () => {
        xhr.abort();
        reject(new DOMException('aborted', 'AbortError'));
      };
      if (opts.signal.aborted) {
        onAbort();
        return;
      }
      opts.signal.addEventListener('abort', onAbort, { once: true });
    }

    xhr.upload.onprogress = (ev) => {
      if (!opts.onProgress) return;
      const total = ev.lengthComputable ? ev.total : bodyLen;
      if (total > 0) {
        opts.onProgress(Math.min(1, ev.loaded / total));
      }
    };
    xhr.upload.onload = () => opts.onProgress?.(1);

    xhr.onerror = () => reject(new Error('network error'));
    xhr.ontimeout = () => reject(new Error('timeout'));
    xhr.onload = () => {
      const headers = parseHeaders(xhr.getAllResponseHeaders());
      handleAuthEdgeCases(xhr.status, headers, opts.suppressRedirect ?? false);
      resolve({
        ok: xhr.status >= 200 && xhr.status < 300,
        status: xhr.status,
        statusText: xhr.statusText,
        headers,
        text: async () => xhr.responseText,
        json: async <T,>() => JSON.parse(xhr.responseText) as T,
      });
    };
    xhr.send(opts.body as Document | XMLHttpRequestBodyInit | null);
  });
}

function inferBodyLength(body: BodyInit): number {
  if (typeof body === 'string') return body.length;
  if (body instanceof Blob) return body.size;
  if (body instanceof ArrayBuffer) return body.byteLength;
  if (ArrayBuffer.isView(body)) return body.byteLength;
  if (body instanceof FormData || body instanceof URLSearchParams) return -1;
  return -1;
}

function parseHeaders(raw: string): Headers {
  const h = new Headers();
  for (const line of raw.split('\r\n')) {
    if (!line) continue;
    const idx = line.indexOf(':');
    if (idx < 0) continue;
    h.append(line.slice(0, idx).trim(), line.slice(idx + 1).trim());
  }
  return h;
}

async function wrapFetchResponse(res: Response, opts: UploadOpts): Promise<UploadResult> {
  handleAuthEdgeCases(res.status, res.headers, opts.suppressRedirect ?? false);
  // Buffer the body once so .text() and .json() are both replayable.
  const text = await res.text();
  return {
    ok: res.ok,
    status: res.status,
    statusText: res.statusText,
    headers: res.headers,
    text: async () => text,
    json: async <T,>() => JSON.parse(text) as T,
  };
}

function handleAuthEdgeCases(status: number, headers: Headers, suppressRedirect: boolean) {
  if (status === 401 && !redirectingForAuth) {
    redirectingForAuth = true;
    pushToast({ kind: 'error', message: 'Session expired — please sign in again.', ttl: 6000 });
    if (!suppressRedirect && typeof window !== 'undefined') {
      const next = encodeURIComponent(window.location.pathname + window.location.search);
      setTimeout(() => {
        goto(`/login?next=${next}`).finally(() => {
          redirectingForAuth = false;
        });
      }, 60);
    } else {
      setTimeout(() => (redirectingForAuth = false), 1000);
    }
  } else if (status === 429) {
    const retry = headers.get('retry-after');
    pushToast({
      kind: 'info',
      message: retry ? `Rate limited — try again in ${retry}s.` : 'Rate limited — slow down a moment.',
      ttl: 5000,
    });
  }
}
