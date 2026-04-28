import type { RequestHandler } from './$types';
import { listSlugs } from '$lib/server/docs';
import { SITE_URL } from '$lib/seo';

export const prerender = true;

const STATIC_PATHS = [
  '/',
  '/docs',
  '/legal/privacy',
  '/legal/terms',
  '/legal/accessibility',
  '/status'
];

export const GET: RequestHandler = async () => {
  const slugs = listSlugs();
  const urls = [
    ...STATIC_PATHS,
    ...slugs.map((s) => `/docs/${s}`)
  ];
  const today = new Date().toISOString().slice(0, 10);

  const body =
    `<?xml version="1.0" encoding="UTF-8"?>\n` +
    `<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n` +
    urls
      .map(
        (path) =>
          `  <url>\n` +
          `    <loc>${SITE_URL}${path}</loc>\n` +
          `    <lastmod>${today}</lastmod>\n` +
          `    <changefreq>${path.startsWith('/docs') ? 'weekly' : 'monthly'}</changefreq>\n` +
          `    <priority>${path === '/' ? '1.0' : path.startsWith('/docs') ? '0.8' : '0.5'}</priority>\n` +
          `  </url>`
      )
      .join('\n') +
    `\n</urlset>\n`;

  return new Response(body, {
    headers: {
      'Content-Type': 'application/xml; charset=utf-8',
      'Cache-Control': 'public, s-maxage=3600, max-age=3600'
    }
  });
};
