<script module lang="ts">
  export interface NavItem {
    label: string;
    href: string;
    /** External or full-document nav (logout). */
    external?: boolean;
    /** Optional section header rendered above the FIRST item carrying it.
     *  Items with the same `section` cluster together; items without one
     *  fall into an unlabelled top group (typical for "Dashboard"). */
    section?: string;
  }
</script>

<script lang="ts">
  import { page } from '$app/stores';

  interface Props {
    items: NavItem[];
  }

  let { items }: Props = $props();

  function isActive(href: string, current: string): boolean {
    if (href === '/dashboard') return current === '/dashboard';
    return current === href || current.startsWith(href + '/');
  }

  // Group consecutive items by section, preserving order.
  const groups = $derived.by(() => {
    const out: { section?: string; items: NavItem[] }[] = [];
    for (const it of items) {
      const last = out[out.length - 1];
      if (last && last.section === it.section) last.items.push(it);
      else out.push({ section: it.section, items: [it] });
    }
    return out;
  });
</script>

<nav class="sidebar" aria-label="Dashboard">
  {#each groups as g, gi}
    {#if g.section}
      <div class="nav-section" id="nav-section-{gi}">{g.section}</div>
    {/if}
    <ul aria-labelledby={g.section ? `nav-section-${gi}` : undefined}>
      {#each g.items as item (item.href)}
        {@const active = isActive(item.href, $page.url.pathname)}
        <li>
          <a
            href={item.href}
            class:active
            aria-current={active ? 'page' : undefined}
            data-sveltekit-reload={item.external ? '' : undefined}
          >{item.label}</a>
        </li>
      {/each}
    </ul>
  {/each}
</nav>

<style>
  .nav-section {
    font-family: var(--mono); font-size: 0.6rem;
    text-transform: uppercase; letter-spacing: 0.1em;
    color: var(--ink-2); opacity: 0.7;
    padding: 0.85rem 0.75rem 0.3rem;
    border-top: 1px solid color-mix(in oklch, var(--ink) 12%, transparent);
    margin-top: 0.4rem;
  }
  .nav-section:first-child { border-top: 0; margin-top: 0; padding-top: 0.4rem; }
</style>
