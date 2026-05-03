<script lang="ts">
  import type { PageData } from './$types';
  import Seo from '$lib/components/Seo.svelte';
  import DocsSidebar from '$lib/components/DocsSidebar.svelte';
  import { breadcrumbJsonLd } from '$lib/seo';

  let { data }: { data: PageData } = $props();

  // Group docs by category, preserving the order in which categories
  // first appear (driven by frontmatter `order`).
  const groups = $derived.by(() => {
    const map = new Map<string, typeof data.docs>();
    for (const doc of data.docs) {
      const list = map.get(doc.category);
      if (list) list.push(doc);
      else map.set(doc.category, [doc]);
    }
    return Array.from(map.entries()).map(([name, items]) => ({ name, items }));
  });
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

    {#each groups as group (group.name)}
      <section class="group">
        <h2 class="group-heading">{group.name}</h2>
        <ul class="docs-grid">
          {#each group.items as doc (doc.slug)}
            <li class="doc-card">
              <a href={`/docs/${doc.slug}`}>
                <h3>{doc.title}</h3>
                <p>{doc.description}</p>
                <span class="arrow" aria-hidden="true">→</span>
              </a>
            </li>
          {/each}
        </ul>
      </section>
    {/each}
  </main>

  <div class="docs-toc-spacer" aria-hidden="true"></div>
</div>

<style>
  .docs-shell {
    display: grid;
    grid-template-columns: 240px 1fr 200px;
    gap: 32px;
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
  .group {
    margin-top: 44px;
  }
  .group-heading {
    font-family: var(--mono);
    font-size: 11px;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: var(--muted);
    font-weight: 500;
    margin: 0 0 14px;
    padding-bottom: 10px;
    border-bottom: 1px solid var(--line);
  }
  .docs-grid {
    list-style: none;
    padding: 0;
    margin: 0;
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
    padding: 20px 22px 24px;
    text-decoration: none;
    color: inherit;
    position: relative;
    min-height: 130px;
    transition: background 0.15s ease;
  }
  .doc-card a:hover { background: rgba(255, 255, 255, 0.5); }
  .doc-card h3 {
    font-family: var(--sans);
    font-weight: 500;
    font-size: 18px;
    letter-spacing: -0.01em;
    margin: 0 0 8px;
    color: var(--ink);
  }
  .doc-card p {
    font-size: 13.5px;
    color: var(--ink-2);
    margin: 0;
    line-height: 1.55;
    max-width: 48ch;
  }
  .doc-card .arrow {
    position: absolute;
    bottom: 14px;
    right: 22px;
    font-family: var(--mono);
    font-size: 14px;
    color: var(--accent-ink);
    transition: transform 0.15s ease;
  }
  .doc-card a:hover .arrow { transform: translateX(4px); }

  @media (max-width: 1100px) {
    .docs-shell { grid-template-columns: 220px 1fr; gap: 24px; }
    .docs-toc-spacer { display: none; }
  }
  @media (max-width: 800px) {
    .docs-shell { grid-template-columns: 1fr; gap: 24px; }
    .docs-grid { grid-template-columns: 1fr; }
  }
</style>
