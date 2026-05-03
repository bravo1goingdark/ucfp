<script lang="ts">
  import { page } from '$app/stores';

  type DocItem = { slug: string; title: string; order: number; category: string };
  type Props = { docs: DocItem[] };
  let { docs }: Props = $props();

  const currentPath = $derived($page.url.pathname);

  // Group docs by category, preserving the order in which categories first
  // appear in the sorted doc list (so "Get started" comes before "API
  // reference" because order=1 lives in "Get started").
  const groups = $derived.by(() => {
    const map = new Map<string, DocItem[]>();
    for (const doc of docs) {
      const list = map.get(doc.category);
      if (list) list.push(doc);
      else map.set(doc.category, [doc]);
    }
    return Array.from(map.entries()).map(([name, items]) => ({ name, items }));
  });
</script>

<aside class="docs-sidebar" aria-label="Documentation navigation">
  <div class="rail-label">Documentation</div>

  <nav>
    <ul class="top">
      <li>
        <a
          href="/docs"
          class="doc-link"
          class:active={currentPath === '/docs' || currentPath === '/docs/'}
        >
          Overview
        </a>
      </li>
    </ul>

    {#each groups as group (group.name)}
      <div class="group-label">{group.name}</div>
      <ul>
        {#each group.items as doc (doc.slug)}
          <li>
            <a
              href={`/docs/${doc.slug}`}
              class="doc-link"
              class:active={currentPath === `/docs/${doc.slug}`}
            >
              {doc.title}
            </a>
          </li>
        {/each}
      </ul>
    {/each}
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
  .group-label {
    font-family: var(--mono);
    font-size: 10px;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: var(--muted);
    margin: 18px 0 6px;
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
  ul.top { margin-bottom: 4px; }
  .doc-link {
    display: block;
    text-decoration: none;
    color: var(--ink-2);
    font-family: var(--sans);
    font-size: 13.5px;
    line-height: 1.4;
    padding: 6px 10px;
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
    background: rgba(255, 255, 255, 0.55);
    font-weight: 500;
  }
</style>
