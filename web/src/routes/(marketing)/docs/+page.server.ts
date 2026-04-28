import type { PageServerLoad } from './$types';
import { loadDocs } from '$lib/server/docs';

// Prerendered at build time. The layout above already sets
// `prerender = true`, but we set it locally too for clarity.
export const prerender = true;

export const load: PageServerLoad = async () => {
  const docs = await loadDocs();
  // Strip the heavy `html`/`body`/`headings` fields — the index page only
  // needs metadata.
  const summary = docs.map(({ slug, title, order, description }) => ({
    slug,
    title,
    order,
    description
  }));
  return { docs: summary };
};
