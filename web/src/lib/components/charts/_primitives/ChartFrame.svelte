<script lang="ts">
  import { onMount } from 'svelte';
  import type { Snippet } from 'svelte';
  import { createFullscreen } from './useFullscreen.svelte';
  import { svgToPng, downloadBlob, downloadSvg } from './useExport';

  interface Props {
    /** Visible label for screen readers + the toolbar title slot. */
    label?: string;
    /** Aspect ratio (width / height) when the container has no explicit height. */
    aspect?: number;
    /** Minimum height in px. */
    minHeight?: number;
    /** Whether to render the default toolbar. Set to false for embedded
     *  thumbnails that should stay chrome-free. */
    toolbar?: boolean;
    /** Allow per-chart filename override for downloads. */
    downloadName?: string;
    /** Custom toolbar content rendered next to the default action buttons. */
    actions?: Snippet;
    /** Body — receives `width` and `height` derived from the container. */
    children: Snippet<[{ width: number; height: number; isFullscreen: boolean }]>;
  }

  let {
    label = 'visualization',
    aspect = 16 / 9,
    minHeight = 240,
    toolbar = true,
    downloadName = 'chart',
    actions,
    children,
  }: Props = $props();

  let host: HTMLDivElement | null = $state(null);
  let bodyEl: HTMLDivElement | null = $state(null);
  let width = $state(640);
  let height = $state(360);
  let isFullscreen = $state(false);

  const fs = createFullscreen();

  $effect(() => {
    isFullscreen = fs.active;
  });

  onMount(() => {
    if (!host) return;
    const ro = new ResizeObserver((entries) => {
      for (const e of entries) {
        const w = Math.max(80, Math.floor(e.contentRect.width));
        let h: number;
        if (isFullscreen) {
          h = Math.max(minHeight, Math.floor(e.contentRect.height));
        } else {
          // Honor explicit container height if set, otherwise derive from aspect.
          const derived = Math.floor(w / aspect);
          h = Math.max(minHeight, derived);
        }
        width = w;
        height = h;
      }
    });
    ro.observe(host);
    return () => {
      ro.disconnect();
      fs.destroy();
    };
  });

  async function onMaximize() {
    if (!host) return;
    await fs.toggle(host);
  }

  async function onDownloadPng() {
    if (!bodyEl) return;
    const svg = bodyEl.querySelector('svg');
    if (!svg) return;
    const blob = await svgToPng(svg as SVGElement, { scale: 2 });
    downloadBlob(blob, `${downloadName}.png`);
  }

  function onDownloadSvg() {
    if (!bodyEl) return;
    const svg = bodyEl.querySelector('svg');
    if (!svg) return;
    downloadSvg(svg as SVGElement, `${downloadName}.svg`);
  }
</script>

<div
  bind:this={host}
  class="chart-host"
  class:fs={isFullscreen}
  role="figure"
  aria-label={label}
>
  {#if toolbar}
    <div class="chart-host-toolbar">
      <span class="chart-host-title">{label}</span>
      <div class="chart-host-actions">
        {#if actions}{@render actions()}{/if}
        <button
          type="button"
          class="chart-host-btn"
          aria-label="Download PNG"
          title="Download PNG"
          onclick={onDownloadPng}
        >PNG</button>
        <button
          type="button"
          class="chart-host-btn"
          aria-label="Download SVG"
          title="Download SVG"
          onclick={onDownloadSvg}
        >SVG</button>
        <button
          type="button"
          class="chart-host-btn"
          aria-label={isFullscreen ? 'Exit fullscreen' : 'Maximize'}
          title={isFullscreen ? 'Exit fullscreen' : 'Maximize'}
          onclick={onMaximize}
        >{isFullscreen ? 'Exit' : 'Max'}</button>
      </div>
    </div>
  {/if}
  <div bind:this={bodyEl} class="chart-host-body">
    {@render children({ width, height, isFullscreen })}
  </div>
</div>

<style>
  .chart-host {
    position: relative;
    display: flex;
    flex-direction: column;
    width: 100%;
    border: 1px solid var(--line-strong);
    background: rgba(255, 255, 255, 0.25);
    container-type: inline-size;
  }
  :global([data-theme='ink']) .chart-host {
    background: rgba(236, 231, 220, 0.03);
  }
  .chart-host.fs,
  .chart-host:fullscreen,
  .chart-host:-webkit-full-screen,
  :global(.chart-fs-fallback) {
    background: var(--bg) !important;
    border: 0;
    z-index: 9999;
  }
  :global(.chart-fs-fallback) {
    position: fixed !important;
    inset: 0 !important;
    width: 100vw !important;
    height: 100vh !important;
  }
  .chart-host-toolbar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 12px;
    padding: 10px 14px;
    border-bottom: 1px solid var(--line);
    font-family: var(--mono);
    font-size: 11px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--muted);
  }
  .chart-host-title {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .chart-host-actions {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .chart-host-btn {
    background: transparent;
    border: 1px solid var(--line-strong);
    color: var(--ink-2);
    font-family: var(--mono);
    font-size: 10px;
    letter-spacing: 0.08em;
    padding: 4px 8px;
    cursor: pointer;
    transition:
      background 0.12s ease,
      color 0.12s ease;
  }
  .chart-host-btn:hover {
    background: var(--ink);
    color: var(--bg);
    border-color: var(--ink);
  }
  .chart-host-btn:focus-visible {
    outline: 2px solid var(--accent-ink);
    outline-offset: 2px;
  }
  .chart-host-body {
    flex: 1;
    min-height: 0;
    display: flex;
    align-items: stretch;
    justify-content: stretch;
    padding: 14px 18px;
    overflow: hidden;
  }
  .chart-host.fs .chart-host-body,
  :global(.chart-fs-fallback) .chart-host-body {
    padding: 24px 32px;
  }
</style>
