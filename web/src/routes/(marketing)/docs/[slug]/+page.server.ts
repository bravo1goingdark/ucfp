import type { EntryGenerator, PageServerLoad } from './$types';
import { error } from '@sveltejs/kit';
import { getDoc, listSlugs, loadDocs } from '$lib/server/docs';

// All docs prerender at build time. The plan called for `+page.ts` so
// `entries()` would be a universal export, but `loadDocs()` lives under
// `$lib/server/*` (forbidden in universal load) AND we don't want
// `marked` + `shiki` shipped to the client. `+page.server.ts` is
// equivalent for prerender-with-entries and keeps the heavy deps off the
// browser bundle.
export const prerender = true;

export const entries: EntryGenerator = () => {
  return listSlugs().map((slug) => ({ slug }));
};

export const load: PageServerLoad = async ({ params }) => {
  const doc = await getDoc(params.slug);
  if (!doc) throw error(404, `No doc named "${params.slug}"`);

  // Sidebar needs the whole list; trim to metadata only.
  const all = await loadDocs();
  const docsList = all.map(({ slug, title, order, description, category }) => ({
    slug,
    title,
    order,
    description,
    category
  }));

  // Prev/next siblings in the global order — ignores category boundaries
  // so the reader can walk the docs front-to-back.
  const idx = all.findIndex((d) => d.slug === doc.slug);
  const sibling = (other: (typeof all)[number] | undefined) =>
    other ? { slug: other.slug, title: other.title, category: other.category } : null;
  const prev = sibling(idx > 0 ? all[idx - 1] : undefined);
  const next = sibling(idx >= 0 && idx < all.length - 1 ? all[idx + 1] : undefined);

  return {
    doc: {
      slug: doc.slug,
      title: doc.title,
      description: doc.description,
      category: doc.category,
      html: doc.html,
      headings: doc.headings
    },
    docs: docsList,
    prev,
    next
  };
};
