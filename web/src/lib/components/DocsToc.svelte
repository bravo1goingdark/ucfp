<script lang="ts">
  type Props = {
    headings: { id: string; text: string }[];
  };
  let { headings }: Props = $props();
</script>

{#if headings.length > 0}
  <aside class="docs-toc" aria-label="On this page">
    <div class="rail-label">On this page</div>
    <nav>
      <ul>
        {#each headings as h (h.id)}
          <li>
            <a href={`#${h.id}`}>{h.text}</a>
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
    color: var(--ink-2);
    font-family: var(--sans);
    font-size: 12.5px;
    line-height: 1.4;
    padding: 4px 0;
    display: inline-block;
    border-bottom: 1px solid transparent;
    transition: color 0.12s ease, border-color 0.12s ease;
  }
  a:hover {
    color: var(--ink);
    border-bottom-color: var(--accent-ink);
  }
</style>
