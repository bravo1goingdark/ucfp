<script lang="ts">
  // Centralised <head> management. Drop one of these into any page; the
  // `<svelte:head>` underneath emits the OG/Twitter/canonical/JSON-LD
  // tags. SvelteKit dedupes <title> across blocks; meta tags are NOT
  // deduped, so don't double-up `description` on the same page.

  import {
    DEFAULT_DESCRIPTION,
    DEFAULT_OG_IMAGE,
    OG_IMAGE_HEIGHT,
    OG_IMAGE_WIDTH,
    SITE_NAME,
    SITE_URL,
    absoluteUrl
  } from '$lib/seo';

  type Props = {
    title?: string;
    description?: string;
    canonical?: string;
    ogImage?: string;
    /** A pre-built JSON-LD object (or array of objects). */
    jsonLd?: Record<string, unknown> | Record<string, unknown>[];
    /** Override the OG type (default `website`, docs use `article`). */
    ogType?: 'website' | 'article';
    /** Mark this page as noindex. */
    noindex?: boolean;
  };

  let {
    title,
    description = DEFAULT_DESCRIPTION,
    canonical,
    ogImage = DEFAULT_OG_IMAGE,
    jsonLd,
    ogType = 'website',
    noindex = false
  }: Props = $props();

  const fullTitle = $derived(
    title && title !== SITE_NAME ? `${title} — ${SITE_NAME}` : SITE_NAME
  );
  const canonicalUrl = $derived(canonical ? absoluteUrl(canonical) : SITE_URL);
  const ogImageUrl = $derived(absoluteUrl(ogImage));
  /* Mime type lets Facebook/LinkedIn cache the right asset; without it
     they sometimes guess wrong on the first crawl and never refresh. */
  const ogImageMime = $derived(
    ogImageUrl.endsWith('.png')
      ? 'image/png'
      : ogImageUrl.endsWith('.svg')
        ? 'image/svg+xml'
        : 'image/jpeg'
  );

  const jsonLdJson = $derived(
    jsonLd ? JSON.stringify(jsonLd) : null
  );
</script>

<svelte:head>
  <title>{fullTitle}</title>
  <meta name="description" content={description} />
  <link rel="canonical" href={canonicalUrl} />
  {#if noindex}
    <meta name="robots" content="noindex, nofollow" />
  {/if}

  <!-- Open Graph -->
  <meta property="og:site_name" content={SITE_NAME} />
  <meta property="og:type" content={ogType} />
  <meta property="og:title" content={fullTitle} />
  <meta property="og:description" content={description} />
  <meta property="og:url" content={canonicalUrl} />
  <meta property="og:image" content={ogImageUrl} />
  <meta property="og:image:secure_url" content={ogImageUrl} />
  <meta property="og:image:type" content={ogImageMime} />
  <meta property="og:image:width" content={String(OG_IMAGE_WIDTH)} />
  <meta property="og:image:height" content={String(OG_IMAGE_HEIGHT)} />
  <meta property="og:image:alt" content={fullTitle} />
  <meta property="og:locale" content="en_US" />

  <!-- Twitter -->
  <meta name="twitter:card" content="summary_large_image" />
  <meta name="twitter:title" content={fullTitle} />
  <meta name="twitter:description" content={description} />
  <meta name="twitter:image" content={ogImageUrl} />
  <meta name="twitter:image:alt" content={fullTitle} />

  {#if jsonLdJson}
    {@html `<script type="application/ld+json">${jsonLdJson}</` + `script>`}
  {/if}
</svelte:head>
