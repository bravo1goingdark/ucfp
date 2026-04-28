// JSON-LD helpers + SEO defaults.
//
// These are pure functions — no platform deps — so they work in both
// `+page.ts` (universal) and `+page.server.ts` (server-only) loaders.

export const SITE_NAME = 'UCFP';
export const SITE_URL = 'https://ucfp.dev';
export const DEFAULT_OG_IMAGE = '/og-default.svg';
export const DEFAULT_DESCRIPTION =
  'Universal Content Fingerprinting — turn text, images, and audio into compact, comparable digests. Production-grade SDKs for Node, Python, and Rust.';

/** schema.org `Organization` — landing page. */
export function organizationJsonLd(): Record<string, unknown> {
  return {
    '@context': 'https://schema.org',
    '@type': 'Organization',
    name: SITE_NAME,
    url: SITE_URL,
    logo: `${SITE_URL}/og-default.svg`,
    description: DEFAULT_DESCRIPTION,
    sameAs: ['https://github.com/bravo1goingdark/ucfp']
  };
}

/** schema.org `FAQPage` — FAQ section + /docs/* Q&A blocks. */
export function faqPageJsonLd(items: { q: string; a: string }[]): Record<string, unknown> {
  return {
    '@context': 'https://schema.org',
    '@type': 'FAQPage',
    mainEntity: items.map((item) => ({
      '@type': 'Question',
      name: item.q,
      acceptedAnswer: { '@type': 'Answer', text: item.a }
    }))
  };
}

/** schema.org `BreadcrumbList` — docs hierarchy. */
export function breadcrumbJsonLd(crumbs: { name: string; url: string }[]): Record<string, unknown> {
  return {
    '@context': 'https://schema.org',
    '@type': 'BreadcrumbList',
    itemListElement: crumbs.map((c, i) => ({
      '@type': 'ListItem',
      position: i + 1,
      name: c.name,
      item: c.url.startsWith('http') ? c.url : `${SITE_URL}${c.url}`
    }))
  };
}

/** Absolutize a path against SITE_URL for canonical / og:url. */
export function absoluteUrl(path: string): string {
  if (path.startsWith('http')) return path;
  return `${SITE_URL}${path.startsWith('/') ? path : `/${path}`}`;
}
