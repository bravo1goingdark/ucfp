<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    x: number;
    y: number;
    /** Anchor relative to the nearest containing block. Defaults to viewport. */
    container?: HTMLElement | null;
    /** Show or hide. */
    visible?: boolean;
    children: Snippet;
  }

  let { x, y, container = null, visible = true, children }: Props = $props();

  let tipEl: HTMLDivElement | null = $state(null);
  let placement = $state<{ left: number; top: number }>({ left: 0, top: 0 });

  $effect(() => {
    if (!visible || !tipEl) return;
    const tipRect = tipEl.getBoundingClientRect();
    const bounds = container?.getBoundingClientRect();
    const offsetX = bounds?.left ?? 0;
    const offsetY = bounds?.top ?? 0;
    const vw = bounds?.width ?? window.innerWidth;
    const vh = bounds?.height ?? window.innerHeight;

    let left = x + 12;
    let top = y + 12;
    if (left + tipRect.width > offsetX + vw - 8) {
      left = x - tipRect.width - 12;
    }
    if (top + tipRect.height > offsetY + vh - 8) {
      top = y - tipRect.height - 12;
    }
    placement = { left, top };
  });
</script>

{#if visible}
  <div
    bind:this={tipEl}
    class="chart-tooltip"
    role="status"
    aria-live="polite"
    style:left="{placement.left}px"
    style:top="{placement.top}px"
  >
    {@render children()}
  </div>
{/if}

<style>
  .chart-tooltip {
    position: fixed;
    z-index: 50;
    pointer-events: none;
    background: var(--ink);
    color: var(--bg);
    font-family: var(--mono);
    font-size: 11px;
    line-height: 1.4;
    letter-spacing: 0.04em;
    padding: 6px 10px;
    border: 1px solid var(--ink);
    box-shadow: 0 8px 24px -12px rgba(0, 0, 0, 0.4);
    max-width: 320px;
    white-space: pre-wrap;
  }
</style>
