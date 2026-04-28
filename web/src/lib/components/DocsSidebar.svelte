<script lang="ts">
  import { page } from '$app/stores';

  type Props = {
    docs: { slug: string; title: string; order: number }[];
  };
  let { docs }: Props = $props();

  const currentPath = $derived($page.url.pathname);
</script>

<aside class="docs-sidebar" aria-label="Documentation navigation">
  <div class="rail-label">Docs</div>
  <nav>
    <ul>
      <li>
        <a
          href="/docs"
          class="doc-link"
          class:active={currentPath === '/docs' || currentPath === '/docs/'}
        >
          Overview
        </a>
      </li>
      {#each docs as doc (doc.slug)}
        <li>
          <a
            href={`/docs/${doc.slug}`}
            class="doc-link"
            class:active={currentPath === `/docs/${doc.slug}`}
          >
            <span class="num">{String(doc.order).padStart(2, '0')}</span>
            {doc.title}
          </a>
        </li>
      {/each}
    </ul>
  </nav>
</aside>

<style>
  .docs-sidebar {
    position: sticky;
    top: 24px;
    align-self: start;
    padding-right: 12px;
    border-right: 1px solid var(--line);
    min-height: 200px;
  }
  .rail-label {
    font-family: var(--mono);
    font-size: 10px;
    letter-spacing: 0.18em;
    text-transform: uppercase;
    color: var(--muted);
    margin-bottom: 14px;
    padding: 4px 8px;
  }
  ul {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .doc-link {
    display: flex;
    align-items: baseline;
    gap: 10px;
    text-decoration: none;
    color: var(--ink-2);
    font-family: var(--sans);
    font-size: 13.5px;
    line-height: 1.4;
    padding: 7px 8px;
    border-left: 2px solid transparent;
    transition: background 0.12s ease, color 0.12s ease, border-color 0.12s ease;
  }
  .doc-link:hover {
    color: var(--ink);
    background: rgba(255, 255, 255, 0.35);
  }
  .doc-link.active {
    color: var(--ink);
    border-left-color: var(--accent-ink);
    background: rgba(255, 255, 255, 0.5);
    font-weight: 500;
  }
  .num {
    font-family: var(--mono);
    font-size: 10px;
    color: var(--muted);
    letter-spacing: 0.08em;
    flex-shrink: 0;
    width: 18px;
  }
  .doc-link.active .num {
    color: var(--accent-ink);
  }
</style>
