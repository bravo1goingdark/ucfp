<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    heading: string;
    description?: string;
    icon?: Snippet;
    cta?: Snippet;
  }

  let { heading, description, icon, cta }: Props = $props();
</script>

<div class="empty-state" role="status">
  {#if icon}
    <div class="empty-icon" aria-hidden="true">{@render icon()}</div>
  {/if}
  <h3>{heading}</h3>
  {#if description}<p>{description}</p>{/if}
  {#if cta}<div class="empty-cta">{@render cta()}</div>{/if}
</div>
