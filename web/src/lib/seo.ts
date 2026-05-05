// JSON-LD helpers + SEO defaults.
//
// These are pure functions — no platform deps — so they work in both
// `+page.ts` (universal) and `+page.server.ts` (server-only) loaders.

export const SITE_NAME = 'UCFP';
export const SITE_URL = 'https://ucfp.dev';
/* PNG over SVG for the default OG: Twitter, Slack, iMessage, Discord all
   render PNG previews reliably; many treat SVG as `image/xml` and skip
   the preview entirely. The SVG is kept as a fallback for inline use. */
export const DEFAULT_OG_IMAGE = '/og-default.png';
export const OG_IMAGE_WIDTH = 1200;
export const OG_IMAGE_HEIGHT = 630;
export const DEFAULT_DESCRIPTION =
  'Universal Content Fingerprinting — turn text, images, and audio into compact, comparable digests. Production-grade SDKs for Node, Python, and Rust.';
export const DEFAULT_KEYWORDS = [
  'fingerprinting', 'content addressing', 'minhash', 'simhash', 'lsh', 'tlsh',
  'phash', 'dhash', 'perceptual hash', 'audio fingerprint', 'wang', 'panako',
  'deduplication', 'similarity search', 'rust sdk', 'cloudflare workers',
];

/** schema.org `Organization` — landing page. */
export function organizationJsonLd(): Record<string, unknown> {
  return {
    '@context': 'https://schema.org',
    '@type': 'Organization',
    name: SITE_NAME,
    url: SITE_URL,
    logo: `${SITE_URL}/og-default.png`,
    description: DEFAULT_DESCRIPTION,
    sameAs: ['https://github.com/bravo1goingdark/ucfp']
  };
}

/** schema.org `WebSite` with sitelinks search box. */
export function websiteJsonLd(): Record<string, unknown> {
  return {
    '@context': 'https://schema.org',
    '@type': 'WebSite',
    name: SITE_NAME,
    url: SITE_URL,
    description: DEFAULT_DESCRIPTION,
    potentialAction: {
      '@type': 'SearchAction',
      target: `${SITE_URL}/docs?q={search_term_string}`,
      'query-input': 'required name=search_term_string',
    },
  };
}

/** schema.org `SoftwareApplication` for the SDK / playground product. */
export function softwareApplicationJsonLd(): Record<string, unknown> {
  return {
    '@context': 'https://schema.org',
    '@type': 'SoftwareApplication',
    name: SITE_NAME,
    applicationCategory: 'DeveloperApplication',
    operatingSystem: 'Cross-platform',
    description: DEFAULT_DESCRIPTION,
    url: SITE_URL,
    image: `${SITE_URL}/og-default.png`,
    offers: { '@type': 'Offer', price: '0', priceCurrency: 'USD' },
    softwareVersion: '0.4.1',
    author: { '@type': 'Organization', name: SITE_NAME, url: SITE_URL },
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

/** schema.org `TechArticle` — for individual /docs/* pages. The Google
 *  rich-results parser specifically rewards `TechArticle` for technical
 *  content (vs the generic `Article`), giving developer docs a small
 *  ranking nudge for query intents like "how to <X>". */
export function techArticleJsonLd(input: {
  slug: string;
  title: string;
  description: string;
  /** ISO date string. Optional but improves snippet quality. */
  datePublished?: string;
  dateModified?: string;
}): Record<string, unknown> {
  const url = `${SITE_URL}/docs/${input.slug}`;
  const body: Record<string, unknown> = {
    '@context': 'https://schema.org',
    '@type': 'TechArticle',
    headline: input.title,
    description: input.description,
    mainEntityOfPage: { '@type': 'WebPage', '@id': url },
    url,
    image: `${SITE_URL}/og-default.png`,
    author: { '@type': 'Organization', name: SITE_NAME, url: SITE_URL },
    publisher: {
      '@type': 'Organization',
      name: SITE_NAME,
      logo: { '@type': 'ImageObject', url: `${SITE_URL}/og-default.png` },
    },
  };
  if (input.datePublished) body.datePublished = input.datePublished;
  if (input.dateModified) body.dateModified = input.dateModified;
  return body;
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
