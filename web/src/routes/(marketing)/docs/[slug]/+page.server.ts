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
  const docsList = all.map(({ slug, title, order, description }) => ({
    slug,
    title,
    order,
    description
  }));

  return {
    doc: {
      slug: doc.slug,
      title: doc.title,
      description: doc.description,
      html: doc.html,
      headings: doc.headings
    },
    docs: docsList
  };
};
