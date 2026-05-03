<script lang="ts">
  import { onMount } from 'svelte';

  type Props = {
    headings: { id: string; text: string }[];
  };
  let { headings }: Props = $props();

  let activeId = $state<string | null>(null);

  // Scroll-spy: an IntersectionObserver tracks which <h2> is currently
  // closest to the top of the viewport. We pick the topmost intersecting
  // heading; if none intersect, we keep the most-recently-seen one.
  onMount(() => {
    if (headings.length === 0) return;

    const targets = headings
      .map((h) => document.getElementById(h.id))
      .filter((el): el is HTMLElement => el != null);
    if (targets.length === 0) return;

    activeId = headings[0].id;

    const visible = new Set<string>();

    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting) visible.add(entry.target.id);
          else visible.delete(entry.target.id);
        }
        if (visible.size === 0) return;
        // Pick the heading nearest the top of the viewport.
        let best: string | null = null;
        let bestTop = Infinity;
        for (const id of visible) {
          const el = document.getElementById(id);
          if (!el) continue;
          const top = el.getBoundingClientRect().top;
          if (top < bestTop) {
            bestTop = top;
            best = id;
          }
        }
        if (best) activeId = best;
      },
      { rootMargin: '0px 0px -70% 0px', threshold: [0, 1] }
    );

    for (const el of targets) observer.observe(el);
    return () => observer.disconnect();
  });
</script>

{#if headings.length > 0}
  <aside class="docs-toc" aria-label="On this page">
    <div class="rail-label">On this page</div>
    <nav>
      <ul>
        {#each headings as h (h.id)}
          <li>
            <a href={`#${h.id}`} class:active={activeId === h.id}>{h.text}</a>
          </li>
        {/each}
      </ul>
    </nav>
  </aside>
{/if}

<style>
  .docs-toc {
    position: sticky;
    top: 24px;
    align-self: start;
    padding-left: 16px;
    border-left: 1px solid var(--line);
    min-height: 200px;
  }
  .rail-label {
    font-family: var(--mono);
    font-size: 10px;
    letter-spacing: 0.18em;
    text-transform: uppercase;
    color: var(--muted);
    margin-bottom: 14px;
  }
  ul {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  a {
    text-decoration: none;
    color: var(--muted);
    font-family: var(--sans);
    font-size: 12.5px;
    line-height: 1.4;
    padding: 4px 0;
    display: inline-block;
    border-left: 2px solid transparent;
    padding-left: 10px;
    margin-left: -12px;
    transition: color 0.12s ease, border-color 0.12s ease;
  }
  a:hover {
    color: var(--ink);
  }
  a.active {
    color: var(--ink);
    border-left-color: var(--accent-ink);
  }
</style>
