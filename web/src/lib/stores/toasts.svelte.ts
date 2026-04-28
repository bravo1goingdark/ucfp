// Toast store — Svelte 5 runes-backed module.
//
// Shared `$state` proxy: components that import `toasts` see updates
// because Svelte 5 runes propagate reactivity through module-scope state
// the same way they do inside a component. IDs are generated with the
// Web Crypto API (no extra deps).

export type ToastKind = 'success' | 'error' | 'info';

export interface Toast {
  id: string;
  kind: ToastKind;
  message: string;
  /** Auto-dismiss timeout in ms. `0` keeps the toast until dismissed. */
  ttl: number;
}

export const toasts = $state<Toast[]>([]);

const timers = new Map<string, ReturnType<typeof setTimeout>>();

function makeId(): string {
  if (typeof crypto !== 'undefined' && 'randomUUID' in crypto) {
    return crypto.randomUUID();
  }
  // Fallback (Workers + browsers both have crypto.randomUUID; this only
  // exists for the unlikely SSR-without-crypto case).
  return Math.random().toString(36).slice(2) + Date.now().toString(36);
}

export function pushToast(input: {
  kind?: ToastKind;
  message: string;
  ttl?: number;
}): string {
  const id = makeId();
  const t: Toast = {
    id,
    kind: input.kind ?? 'info',
    message: input.message,
    ttl: input.ttl ?? 4000
  };
  toasts.push(t);

  if (t.ttl > 0 && typeof setTimeout !== 'undefined') {
    const handle = setTimeout(() => dismiss(id), t.ttl);
    timers.set(id, handle);
  }
  return id;
}

export function dismiss(id: string): void {
  const idx = toasts.findIndex((t) => t.id === id);
  if (idx >= 0) toasts.splice(idx, 1);
  const handle = timers.get(id);
  if (handle !== undefined) {
    clearTimeout(handle);
    timers.delete(id);
  }
}
