// Records page is browser-only — the source of truth is localStorage,
// and lookups hit /api/records/[id] from the client.
export const ssr = false;
export const prerender = false;
