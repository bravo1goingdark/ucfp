<script lang="ts">
  import type { PageData } from './$types';
  import Seo from '$lib/components/Seo.svelte';
  import DocsSidebar from '$lib/components/DocsSidebar.svelte';
  import DocsToc from '$lib/components/DocsToc.svelte';
  import { breadcrumbJsonLd } from '$lib/seo';

  let { data }: { data: PageData } = $props();

  const jsonLd = $derived(
    breadcrumbJsonLd([
      { name: 'Docs', url: '/docs' },
      { name: data.doc.title, url: `/docs/${data.doc.slug}` }
    ])
  );
</script>

<Seo
  title={data.doc.title}
  description={data.doc.description}
  canonical={`/docs/${data.doc.slug}`}
  ogType="article"
  {jsonLd}
/>

<div class="docs-shell">
  <DocsSidebar docs={data.docs} />

  <article class="doc-article">
    <div class="crumbs">
      <a href="/docs">Docs</a>
      <span aria-hidden="true">/</span>
      <span>{data.doc.title}</span>
    </div>
    {@html data.doc.html}
  </article>

  <DocsToc headings={data.doc.headings} />
</div>

<style>
  .docs-shell {
    display: grid;
    grid-template-columns: 220px minmax(0, 1fr) 200px;
    gap: 24px;
    margin-top: 24px;
    align-items: start;
  }
  .crumbs {
    font-family: var(--mono);
    font-size: 11px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--muted);
    display: flex;
    gap: 8px;
    align-items: center;
    margin-bottom: 28px;
  }
  .crumbs a {
    color: var(--muted);
    text-decoration: none;
  }
  .crumbs a:hover { color: var(--ink); }

  .doc-article {
    min-width: 0;
    color: var(--ink);
    font-family: var(--sans);
    font-size: 16px;
    line-height: 1.65;
    letter-spacing: -0.003em;
  }

  .doc-article :global(h1) {
    font-family: var(--sans);
    font-weight: 400;
    font-size: clamp(36px, 4.4vw, 56px);
    letter-spacing: -0.03em;
    line-height: 1.05;
    margin: 0 0 24px;
    color: var(--ink);
  }
  .doc-article :global(h2) {
    font-family: var(--sans);
    font-weight: 500;
    font-size: 26px;
    letter-spacing: -0.015em;
    margin: 48px 0 16px;
    padding-top: 16px;
    border-top: 1px solid var(--line);
    scroll-margin-top: 24px;
    color: var(--ink);
  }
  .doc-article :global(h3) {
    font-family: var(--sans);
    font-weight: 500;
    font-size: 19px;
    letter-spacing: -0.01em;
    margin: 32px 0 10px;
    color: var(--ink);
  }
  .doc-article :global(p) {
    color: var(--ink-2);
    margin: 14px 0;
    max-width: 70ch;
  }
  .doc-article :global(ul),
  .doc-article :global(ol) {
    color: var(--ink-2);
    padding-left: 24px;
    margin: 14px 0;
    max-width: 70ch;
  }
  .doc-article :global(li) {
    margin: 6px 0;
  }
  .doc-article :global(a) {
    color: var(--accent-ink);
    text-decoration: underline;
    text-decoration-color: var(--line-strong);
    text-underline-offset: 3px;
  }
  .doc-article :global(a:hover) {
    text-decoration-color: var(--accent-ink);
  }
  .doc-article :global(strong) { color: var(--ink); font-weight: 600; }

  /* inline code */
  .doc-article :global(p code),
  .doc-article :global(li code),
  .doc-article :global(td code) {
    font-family: var(--mono);
    font-size: 0.88em;
    background: rgba(20, 20, 20, 0.06);
    border: 1px solid var(--line);
    padding: 1px 6px;
    border-radius: 3px;
    color: var(--ink);
  }

  /* shiki code blocks */
  .doc-article :global(pre.shiki) {
    border: 1px solid var(--line-strong);
    padding: 18px 22px;
    overflow-x: auto;
    box-shadow: var(--paper-shadow);
    margin: 18px 0;
    font-family: var(--mono);
    font-size: 13px;
    line-height: 1.7;
    background: #15140F;
    color: #E8E3D6;
  }
  .doc-article :global(pre.shiki code) {
    font-family: inherit;
    background: transparent;
    border: 0;
    padding: 0;
    color: inherit;
  }
  /* dual-theme: shiki emits both color sets via CSS vars; flip on theme */
  .doc-article :global(pre.shiki .line) { display: block; }

  /* tables */
  .doc-article :global(table) {
    width: 100%;
    border-collapse: collapse;
    margin: 18px 0;
    font-size: 13.5px;
    border-top: 1px solid var(--line-strong);
    border-left: 1px solid var(--line-strong);
  }
  .doc-article :global(th),
  .doc-article :global(td) {
    text-align: left;
    padding: 10px 12px;
    border-right: 1px solid var(--line-strong);
    border-bottom: 1px solid var(--line-strong);
    vertical-align: top;
  }
  .doc-article :global(th) {
    background: rgba(20, 20, 20, 0.04);
    font-family: var(--mono);
    font-size: 11px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--ink);
    font-weight: 500;
  }
  .doc-article :global(td) {
    color: var(--ink-2);
    font-family: var(--sans);
  }
  .doc-article :global(td code) { font-size: 12px; }

  /* blockquote */
  .doc-article :global(blockquote) {
    border-left: 3px solid var(--accent);
    margin: 18px 0;
    padding: 4px 0 4px 18px;
    color: var(--ink-2);
    font-style: italic;
  }

  /* hr */
  .doc-article :global(hr) {
    border: 0;
    border-top: 1px solid var(--line);
    margin: 36px 0;
  }

  @media (max-width: 1100px) {
    .docs-shell { grid-template-columns: 200px minmax(0, 1fr); }
  }
  @media (max-width: 800px) {
    .docs-shell { grid-template-columns: 1fr; gap: 24px; }
  }
</style>
