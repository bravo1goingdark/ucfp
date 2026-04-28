import type { PageLoad } from './$types';

// SvelteFlow is browser-only — disable SSR for this route.
export const ssr = false;
export const prerender = false;

export const load: PageLoad = () => ({});
