<script lang="ts">
  import { onMount } from 'svelte';
  import type { PageData } from './$types';
  import Seo from '$lib/components/Seo.svelte';
  import DocsSidebar from '$lib/components/DocsSidebar.svelte';
  import DocsToc from '$lib/components/DocsToc.svelte';
  import { breadcrumbJsonLd } from '$lib/seo';
  import { tweaks, setTweak } from '$lib/stores/tweaks.svelte';

  let { data }: { data: PageData } = $props();

  const jsonLd = $derived(
    breadcrumbJsonLd([
      { name: 'Docs', url: '/docs' },
      { name: data.doc.title, url: `/docs/${data.doc.slug}` }
    ])
  );

  let article: HTMLElement | undefined = $state();

  // Wire copy-to-clipboard on every code-block button injected by the
  // markdown renderer. Re-runs whenever the article HTML changes (i.e.
  // on client-side navigation between docs).
  $effect(() => {
    if (!article) return;
    void data.doc.html;

    const buttons = article.querySelectorAll<HTMLButtonElement>('.copy-btn');
    const cleanups: Array<() => void> = [];
    for (const btn of buttons) {
      const handler = async () => {
        const block = btn.closest('.code-block');
        const pre = block?.querySelector('pre');
        if (!pre) return;
        try {
          await navigator.clipboard.writeText(pre.textContent ?? '');
          const prev = btn.textContent;
          btn.textContent = 'Copied';
          btn.classList.add('copied');
          setTimeout(() => {
            btn.textContent = prev;
            btn.classList.remove('copied');
          }, 1400);
        } catch {
          // Clipboard write failed — leave the button alone.
        }
      };
      btn.addEventListener('click', handler);
      cleanups.push(() => btn.removeEventListener('click', handler));
    }
    return () => {
      for (const c of cleanups) c();
    };
  });

  function toggleDark() {
    setTweak('theme', tweaks.theme === 'ink' ? 'paper' : 'ink');
  }
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

  <article class="doc-article" bind:this={article}>
    <header class="article-head">
      <div class="crumbs">
        <a href="/docs">Docs</a>
        <span aria-hidden="true">/</span>
        <span class="crumb-cat">{data.doc.category}</span>
        <span aria-hidden="true">/</span>
        <span>{data.doc.title}</span>
      </div>
      <button
        type="button"
        class="theme-toggle"
        onclick={toggleDark}
        aria-label={tweaks.theme === 'ink' ? 'Switch to light mode' : 'Switch to dark mode'}
        title={tweaks.theme === 'ink' ? 'Switch to light mode' : 'Switch to dark mode'}
      >
        {tweaks.theme === 'ink' ? '☀' : '☾'}
      </button>
    </header>

    {@html data.doc.html}

    <nav class="page-nav" aria-label="Page navigation">
      {#if data.prev}
        <a class="page-nav-link prev" href={`/docs/${data.prev.slug}`}>
          <span class="dir">← Previous</span>
          <span class="title">{data.prev.title}</span>
        </a>
      {:else}
        <span></span>
      {/if}
      {#if data.next}
        <a class="page-nav-link next" href={`/docs/${data.next.slug}`}>
          <span class="dir">Next →</span>
          <span class="title">{data.next.title}</span>
        </a>
      {/if}
    </nav>
  </article>

  <DocsToc headings={data.doc.headings} />
</div>

<style>
  .docs-shell {
    display: grid;
    grid-template-columns: 240px minmax(0, 1fr) 200px;
    gap: 32px;
    margin-top: 24px;
    align-items: start;
  }
  .article-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 28px;
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
    flex-wrap: wrap;
  }
  .crumbs a {
    color: var(--muted);
    text-decoration: none;
  }
  .crumbs a:hover { color: var(--ink); }
  .crumb-cat { color: var(--ink-2); }
  .theme-toggle {
    background: rgba(255, 255, 255, 0.4);
    border: 1px solid var(--line);
    color: var(--ink-2);
    width: 30px;
    height: 30px;
    border-radius: 50%;
    cursor: pointer;
    font-size: 14px;
    line-height: 1;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    transition: background 0.12s ease, border-color 0.12s ease;
  }
  .theme-toggle:hover {
    background: rgba(255, 255, 255, 0.7);
    border-color: var(--line-strong);
    color: var(--ink);
  }

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

  /* code-block wrapper (lang label + copy button) */
  .doc-article :global(.code-block) {
    margin: 18px 0;
    border: 1px solid var(--line-strong);
    box-shadow: var(--paper-shadow);
    background: #15140F;
    overflow: hidden;
  }
  .doc-article :global(.code-block-header) {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 14px;
    background: rgba(255, 255, 255, 0.04);
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
  }
  .doc-article :global(.code-block-lang) {
    font-family: var(--mono);
    font-size: 10.5px;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: rgba(232, 227, 214, 0.55);
  }
  .doc-article :global(.copy-btn) {
    background: transparent;
    border: 1px solid rgba(232, 227, 214, 0.18);
    color: rgba(232, 227, 214, 0.75);
    font-family: var(--mono);
    font-size: 11px;
    letter-spacing: 0.06em;
    padding: 3px 10px;
    cursor: pointer;
    transition: background 0.12s ease, color 0.12s ease, border-color 0.12s ease;
  }
  .doc-article :global(.copy-btn:hover) {
    background: rgba(232, 227, 214, 0.08);
    color: #E8E3D6;
    border-color: rgba(232, 227, 214, 0.35);
  }
  .doc-article :global(.copy-btn.copied) {
    color: var(--accent);
    border-color: var(--accent);
  }

  /* shiki code blocks */
  .doc-article :global(.code-block pre.shiki),
  .doc-article :global(pre.shiki) {
    border: 0;
    padding: 16px 18px;
    overflow-x: auto;
    margin: 0;
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

  /* blockquote (non-callout) */
  .doc-article :global(blockquote) {
    border-left: 3px solid var(--accent);
    margin: 18px 0;
    padding: 4px 0 4px 18px;
    color: var(--ink-2);
    font-style: italic;
  }

  /* callouts (admonitions) */
  .doc-article :global(.callout) {
    margin: 20px 0;
    padding: 14px 18px 14px 18px;
    border-left: 3px solid;
    background: rgba(255, 255, 255, 0.4);
    border-radius: 0 4px 4px 0;
  }
  .doc-article :global(.callout-title) {
    font-family: var(--mono);
    font-size: 11px;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    margin-bottom: 6px;
    font-weight: 600;
  }
  .doc-article :global(.callout-body > p:first-child) { margin-top: 0; }
  .doc-article :global(.callout-body > p:last-child) { margin-bottom: 0; }
  .doc-article :global(.callout-note) {
    border-left-color: #4a86c5;
    background: rgba(74, 134, 197, 0.08);
  }
  .doc-article :global(.callout-note .callout-title) { color: #2f6aab; }
  .doc-article :global(.callout-tip) {
    border-left-color: var(--accent);
    background: rgba(120, 160, 100, 0.1);
  }
  .doc-article :global(.callout-tip .callout-title) { color: var(--accent-ink); }
  .doc-article :global(.callout-info) {
    border-left-color: #6e6a60;
    background: rgba(110, 106, 96, 0.08);
  }
  .doc-article :global(.callout-info .callout-title) { color: var(--ink-2); }
  .doc-article :global(.callout-warning),
  .doc-article :global(.callout-caution) {
    border-left-color: #c08a3a;
    background: rgba(192, 138, 58, 0.1);
  }
  .doc-article :global(.callout-warning .callout-title),
  .doc-article :global(.callout-caution .callout-title) { color: #8c5e1a; }
  .doc-article :global(.callout-important) {
    border-left-color: #b8484c;
    background: rgba(184, 72, 76, 0.08);
  }
  .doc-article :global(.callout-important .callout-title) { color: #913238; }

  /* hr */
  .doc-article :global(hr) {
    border: 0;
    border-top: 1px solid var(--line);
    margin: 36px 0;
  }

  /* prev/next page nav */
  .page-nav {
    margin-top: 56px;
    padding-top: 24px;
    border-top: 1px solid var(--line);
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 16px;
  }
  .page-nav-link {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 14px 16px;
    border: 1px solid var(--line);
    text-decoration: none;
    background: rgba(255, 255, 255, 0.3);
    transition: background 0.12s ease, border-color 0.12s ease;
    min-width: 0;
  }
  .page-nav-link:hover {
    background: rgba(255, 255, 255, 0.55);
    border-color: var(--line-strong);
  }
  .page-nav-link.next {
    text-align: right;
    align-items: flex-end;
  }
  .page-nav-link .dir {
    font-family: var(--mono);
    font-size: 10.5px;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: var(--muted);
  }
  .page-nav-link .title {
    font-family: var(--sans);
    font-size: 15px;
    color: var(--ink);
    font-weight: 500;
  }

  @media (max-width: 1100px) {
    .docs-shell { grid-template-columns: 220px minmax(0, 1fr); gap: 24px; }
  }
  @media (max-width: 800px) {
    .docs-shell { grid-template-columns: 1fr; gap: 24px; }
    .page-nav { grid-template-columns: 1fr; }
    .page-nav-link.next { text-align: left; align-items: flex-start; }
  }
</style>
