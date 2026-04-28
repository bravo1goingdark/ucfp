<script lang="ts">
  import type { PageData } from './$types';
  import Seo from '$lib/components/Seo.svelte';
  import DocsSidebar from '$lib/components/DocsSidebar.svelte';
  import { breadcrumbJsonLd } from '$lib/seo';

  let { data }: { data: PageData } = $props();
</script>

<Seo
  title="Documentation"
  description="Guides, API references, and SDK quickstarts for the Universal Content Fingerprinting platform."
  canonical="/docs"
  jsonLd={breadcrumbJsonLd([{ name: 'Docs', url: '/docs' }])}
/>

<div class="docs-shell">
  <DocsSidebar docs={data.docs} />

  <main class="docs-content">
    <div class="section-label">Documentation</div>
    <h1 class="docs-title">
      Build with <span class="it">UCFP.</span>
    </h1>
    <p class="lede">
      Guides, references, and recipes for every modality. Pick a starting
      point — most teams begin with <a href="/docs/getting-started">Getting started</a>.
    </p>

    <ul class="docs-grid">
      {#each data.docs as doc (doc.slug)}
        <li class="doc-card">
          <a href={`/docs/${doc.slug}`}>
            <div class="num">{String(doc.order).padStart(2, '0')}</div>
            <h2>{doc.title}</h2>
            <p>{doc.description}</p>
            <span class="arrow" aria-hidden="true">→</span>
          </a>
        </li>
      {/each}
    </ul>
  </main>

  <div class="docs-toc-spacer" aria-hidden="true"></div>
</div>

<style>
  .docs-shell {
    display: grid;
    grid-template-columns: 220px 1fr 200px;
    gap: 24px;
    margin-top: 24px;
    align-items: start;
  }
  .docs-content { min-width: 0; }
  .docs-title {
    font-family: var(--sans);
    font-weight: 400;
    font-size: clamp(40px, 5vw, 64px);
    letter-spacing: -0.03em;
    line-height: 1.05;
    margin: 0 0 18px;
  }
  .docs-title .it {
    font-family: var(--serif);
    font-style: italic;
  }
  .docs-grid {
    list-style: none;
    padding: 0;
    margin: 36px 0 0;
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: 0;
    border-top: 1px solid var(--line-strong);
    border-left: 1px solid var(--line-strong);
  }
  .doc-card {
    border-right: 1px solid var(--line-strong);
    border-bottom: 1px solid var(--line-strong);
    background: rgba(255, 255, 255, 0.25);
  }
  .doc-card a {
    display: block;
    padding: 24px 24px 28px;
    text-decoration: none;
    color: inherit;
    position: relative;
    min-height: 180px;
    transition: background 0.15s ease;
  }
  .doc-card a:hover { background: rgba(255, 255, 255, 0.45); }
  .doc-card .num {
    font-family: var(--mono);
    font-size: 11px;
    color: var(--muted);
    letter-spacing: 0.14em;
  }
  .doc-card h2 {
    font-family: var(--sans);
    font-weight: 500;
    font-size: 19px;
    letter-spacing: -0.01em;
    margin: 14px 0 8px;
    color: var(--ink);
  }
  .doc-card p {
    font-size: 13.5px;
    color: var(--ink-2);
    margin: 0;
    line-height: 1.55;
  }
  .doc-card .arrow {
    position: absolute;
    bottom: 16px;
    right: 24px;
    font-family: var(--mono);
    font-size: 14px;
    color: var(--accent-ink);
    transition: transform 0.15s ease;
  }
  .doc-card a:hover .arrow { transform: translateX(4px); }

  @media (max-width: 1100px) {
    .docs-shell { grid-template-columns: 200px 1fr; }
    .docs-toc-spacer { display: none; }
  }
  @media (max-width: 800px) {
    .docs-shell { grid-template-columns: 1fr; gap: 24px; }
    .docs-grid { grid-template-columns: 1fr; }
  }
</style>
