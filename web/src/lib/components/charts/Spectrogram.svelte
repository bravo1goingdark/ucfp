<!--
  Spectrogram viewer with interactive landmark/peak overlays.

  The Rust `inspect` audio endpoint returns the spectrogram and mel
  spectrogram as base64-encoded PNGs (viridis colour map). This component
  blits the PNG to a container, draws frequency × time axes around it,
  and overlays Wang/Panako peak picks and landmark pairs on top so users
  can see exactly which time/frequency bins drove the fingerprint.
-->
<script lang="ts">
  import Tooltip from './_primitives/Tooltip.svelte';

  type Peak = { t_ms: number; freq_hz: number; db: number };
  type Pair = { t1_ms: number; f1_hz: number; t2_ms: number; f2_hz: number };

  interface Props {
    /** base64-encoded PNG (viridis log-magnitude). */
    pngB64: string;
    /** PNG pixel dimensions, used purely for label/info — actual <img> sizes via CSS. */
    pngWidth: number;
    pngHeight: number;
    /** Frequency axis bounds in Hz. */
    fminHz: number;
    fmaxHz: number;
    /** Audio duration in seconds (time axis). */
    durationSec: number;
    /** Optional peaks drawn as dots over the PNG. */
    peaks?: Peak[];
    /** Optional landmark pairs drawn as anchor → target line segments. */
    pairs?: Pair[];
    /** Show peak dots? */
    showPeaks?: boolean;
    /** Show landmark pairs? */
    showPairs?: boolean;
  }

  let {
    pngB64,
    pngWidth,
    pngHeight,
    fminHz,
    fmaxHz,
    durationSec,
    peaks = [],
    pairs = [],
    showPeaks = $bindable(true),
    showPairs = $bindable(true),
  }: Props = $props();

  let host: HTMLDivElement | null = $state(null);
  let plotW = $state(800);
  let plotH = $state(400);

  // Margins reserve room for axes.
  const M = { top: 12, right: 12, bottom: 36, left: 60 };

  $effect(() => {
    if (!host) return;
    const ro = new ResizeObserver((entries) => {
      for (const e of entries) {
        plotW = Math.max(200, Math.floor(e.contentRect.width));
        plotH = Math.max(160, Math.floor(e.contentRect.height));
      }
    });
    ro.observe(host);
    return () => ro.disconnect();
  });

  // Inner plot area excludes axis margins.
  let innerW = $derived(Math.max(1, plotW - M.left - M.right));
  let innerH = $derived(Math.max(1, plotH - M.top - M.bottom));

  function tToX(t_ms: number): number {
    if (durationSec <= 0) return 0;
    return (t_ms / 1000 / durationSec) * innerW;
  }
  function fToY(f_hz: number): number {
    if (fmaxHz <= fminHz) return innerH;
    const t = (f_hz - fminHz) / (fmaxHz - fminHz);
    return innerH - t * innerH;
  }

  function niceTicks(min: number, max: number, count = 5): number[] {
    if (!isFinite(min) || !isFinite(max) || min === max) return [min];
    const span = max - min;
    const rawStep = span / Math.max(1, count);
    const mag = Math.pow(10, Math.floor(Math.log10(rawStep)));
    const norm = rawStep / mag;
    const step = (norm < 1.5 ? 1 : norm < 3 ? 2 : norm < 7 ? 5 : 10) * mag;
    const start = Math.ceil(min / step) * step;
    const out: number[] = [];
    for (let v = start; v <= max + step / 2; v += step) {
      out.push(Math.round(v / step) * step);
    }
    return out;
  }

  function fmtFreq(hz: number): string {
    if (hz >= 1000) return `${(hz / 1000).toFixed(1)}k`;
    return `${Math.round(hz)}`;
  }
  function fmtTime(s: number): string {
    if (s >= 60) return `${Math.floor(s / 60)}:${String(Math.floor(s % 60)).padStart(2, '0')}`;
    return `${s.toFixed(2)}`;
  }

  let xTicks = $derived(niceTicks(0, durationSec, 6));
  let yTicks = $derived(niceTicks(fminHz, fmaxHz, 5));

  // Hover state for peaks.
  let hover = $state<{ x: number; y: number; text: string } | null>(null);

  function onPeakEnter(p: Peak, ev: MouseEvent) {
    hover = {
      x: ev.clientX,
      y: ev.clientY,
      text: `t = ${fmtTime(p.t_ms / 1000)} s\nf = ${fmtFreq(p.freq_hz)} Hz\n${p.db.toFixed(1)} dB`,
    };
  }
  function onPeakLeave() {
    hover = null;
  }
</script>

<div bind:this={host} class="spec-host">
  <div class="spec-controls">
    <label class="spec-toggle">
      <input type="checkbox" bind:checked={showPeaks} />
      <span>peaks ({peaks.length})</span>
    </label>
    <label class="spec-toggle">
      <input type="checkbox" bind:checked={showPairs} />
      <span>landmarks ({pairs.length})</span>
    </label>
  </div>
  <svg viewBox="0 0 {plotW} {plotH}" preserveAspectRatio="none" class="spec-svg" role="img"
    aria-label="Spectrogram with {peaks.length} peaks and {pairs.length} landmark pairs">
    <g transform="translate({M.left},{M.top})">
      <!-- spectrogram PNG, stretched to fill the inner plot area -->
      <image
        href="data:image/png;base64,{pngB64}"
        x="0" y="0" width={innerW} height={innerH}
        preserveAspectRatio="none"
        style="image-rendering: pixelated;" />

      <!-- axes -->
      <g class="axis x">
        <line x1="0" y1={innerH} x2={innerW} y2={innerH} />
        {#each xTicks as t (t)}
          <g transform="translate({tToX(t * 1000)},{innerH})">
            <line y2={4} />
            <text y={16} text-anchor="middle">{fmtTime(t)}s</text>
          </g>
        {/each}
        <text x={innerW / 2} y={innerH + 32} class="axis-label" text-anchor="middle">time</text>
      </g>
      <g class="axis y">
        <line x1="0" y1="0" x2="0" y2={innerH} />
        {#each yTicks as f (f)}
          <g transform="translate(0,{fToY(f)})">
            <line x2={-4} />
            <text x={-8} dy="0.32em" text-anchor="end">{fmtFreq(f)}Hz</text>
          </g>
        {/each}
        <text transform="translate({-46},{innerH / 2}) rotate(-90)" class="axis-label" text-anchor="middle">frequency</text>
      </g>

      <!-- landmark pair lines (drawn under peaks for layering) -->
      {#if showPairs}
        <g class="pairs">
          {#each pairs as p, i (i)}
            <line
              x1={tToX(p.t1_ms)} y1={fToY(p.f1_hz)}
              x2={tToX(p.t2_ms)} y2={fToY(p.f2_hz)} />
          {/each}
        </g>
      {/if}

      <!-- picked peaks (dots) -->
      {#if showPeaks}
        <g class="peaks">
          {#each peaks as p, i (i)}
            <circle
              cx={tToX(p.t_ms)} cy={fToY(p.freq_hz)} r="2.5"
              onmouseenter={(e: MouseEvent) => onPeakEnter(p, e)}
              onmouseleave={onPeakLeave}
              onmousemove={(e: MouseEvent) => onPeakEnter(p, e)}
              role="img"
              aria-label="peak at {fmtTime(p.t_ms / 1000)}s, {fmtFreq(p.freq_hz)}Hz" />
          {/each}
        </g>
      {/if}
    </g>
  </svg>
  {#if hover}
    {@const h = hover}
    <Tooltip x={h.x} y={h.y} container={host}>
      {#snippet children()}{h.text}{/snippet}
    </Tooltip>
  {/if}
  <div class="spec-meta">
    <span><strong>img</strong> {pngWidth}×{pngHeight}</span>
    <span><strong>span</strong> {durationSec.toFixed(2)}s</span>
    <span><strong>freq</strong> {fmtFreq(fminHz)}–{fmtFreq(fmaxHz)} Hz</span>
  </div>
</div>

<style>
  .spec-host {
    position: relative;
    display: flex;
    flex-direction: column;
    width: 100%;
    height: 100%;
    gap: 0.5rem;
  }
  .spec-controls {
    display: flex;
    gap: 1rem;
    flex-wrap: wrap;
    font-family: var(--mono);
    font-size: 11px;
    color: var(--ink-2);
  }
  .spec-toggle {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    cursor: pointer;
    user-select: none;
  }
  .spec-toggle input {
    accent-color: var(--accent-ink);
  }
  .spec-svg {
    flex: 1;
    width: 100%;
    height: auto;
    min-height: 200px;
    background: #000;
    border: 1px solid var(--line-strong);
  }
  .axis line {
    stroke: var(--line-strong);
    stroke-width: 1;
  }
  .axis text {
    font-family: var(--mono);
    font-size: 10px;
    fill: var(--muted);
  }
  .axis-label {
    font-size: 10.5px;
    fill: var(--ink-2);
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }
  .pairs line {
    stroke: oklch(0.85 0.16 90);
    stroke-width: 1;
    opacity: 0.55;
    pointer-events: none;
  }
  .peaks circle {
    fill: oklch(0.92 0.18 50);
    stroke: oklch(0.45 0.15 50);
    stroke-width: 0.5;
    cursor: crosshair;
    transition: r 0.1s ease;
  }
  .peaks circle:hover {
    r: 4;
    fill: oklch(0.98 0.2 50);
  }
  .spec-meta {
    display: flex;
    gap: 1rem;
    flex-wrap: wrap;
    font-family: var(--mono);
    font-size: 11px;
    color: var(--ink-2);
  }
  .spec-meta strong {
    font-weight: 400;
    color: var(--muted);
    text-transform: uppercase;
    letter-spacing: 0.06em;
    margin-right: 4px;
  }
</style>
