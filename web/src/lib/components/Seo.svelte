<script lang="ts">
  // Centralised <head> management. Drop one of these into any page; the
  // `<svelte:head>` underneath emits the OG/Twitter/canonical/JSON-LD
  // tags. SvelteKit dedupes <title> across blocks; meta tags are NOT
  // deduped, so don't double-up `description` on the same page.

  import {
    DEFAULT_DESCRIPTION,
    DEFAULT_OG_IMAGE,
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

  <!-- Twitter -->
  <meta name="twitter:card" content="summary_large_image" />
  <meta name="twitter:title" content={fullTitle} />
  <meta name="twitter:description" content={description} />
  <meta name="twitter:image" content={ogImageUrl} />

  {#if jsonLdJson}
    {@html `<script type="application/ld+json">${jsonLdJson}</` + `script>`}
  {/if}
</svelte:head>
