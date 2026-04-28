<script module lang="ts">
  export interface NavItem {
    label: string;
    href: string;
    /** External or full-document nav (logout). */
    external?: boolean;
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
</script>

<nav class="sidebar" aria-label="Dashboard">
  <ul>
    {#each items as item (item.href)}
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
</nav>
