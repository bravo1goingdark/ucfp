<script module lang="ts">
  export interface Crumb {
    label: string;
    href?: string;
  }
</script>

<script lang="ts">
  interface Props {
    crumbs: Crumb[];
  }

  let { crumbs }: Props = $props();
</script>

<nav class="breadcrumb" aria-label="Breadcrumb">
  <ol>
    {#each crumbs as c, i (i)}
      <li>
        {#if c.href && i < crumbs.length - 1}
          <a href={c.href}>{c.label}</a>
        {:else}
          <span aria-current={i === crumbs.length - 1 ? 'page' : undefined}>{c.label}</span>
        {/if}
        {#if i < crumbs.length - 1}<span class="sep" aria-hidden="true">/</span>{/if}
      </li>
    {/each}
  </ol>
</nav>
