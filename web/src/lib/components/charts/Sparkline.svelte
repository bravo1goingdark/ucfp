<script lang="ts">
  // Tiny single-series SVG sparkline. ~80×24 by default, no axes.
  // SSR-safe: pure data → path string. Hover the chart to surface a
  // crosshair + the value at that index.

  import Tooltip from './_primitives/Tooltip.svelte';

  interface Props {
    values: number[];
    width?: number;
    height?: number;
    label?: string;
    /** Optional ISO-or-similar timestamps aligned with `values`. When
     *  supplied the hover tooltip shows the timestamp instead of the
     *  bare index. */
    timestamps?: string[];
  }

  let { values, width = 80, height = 24, label, timestamps = [] }: Props = $props();

  let host: HTMLDivElement | null = $state(null);
  let hover = $state<{ idx: number; x: number; y: number; clientX: number; clientY: number } | null>(null);

  const stats = $derived.by(() => {
    if (values.length === 0) return { min: 0, max: 1, range: 1 };
    const max = Math.max(...values, 1);
    const min = Math.min(...values, 0);
    return { min, max, range: max - min || 1 };
  });

  const path = $derived.by(() => {
    if (values.length === 0) return '';
    const stepX = values.length > 1 ? width / (values.length - 1) : 0;
    return values
      .map((v, i) => {
        const x = i * stepX;
        const y = height - ((v - stats.min) / stats.range) * height;
        return `${i === 0 ? 'M' : 'L'}${x.toFixed(2)} ${y.toFixed(2)}`;
      })
      .join(' ');
  });

  function onMove(e: MouseEvent) {
    if (values.length === 0) return;
    const svg = e.currentTarget as SVGSVGElement;
    const rect = svg.getBoundingClientRect();
    const xRatio = (e.clientX - rect.left) / rect.width;
    const idx = Math.max(0, Math.min(values.length - 1, Math.round(xRatio * (values.length - 1))));
    const stepX = values.length > 1 ? width / (values.length - 1) : 0;
    const v = values[idx];
    const x = idx * stepX;
    const y = height - ((v - stats.min) / stats.range) * height;
    hover = { idx, x, y, clientX: e.clientX, clientY: e.clientY };
  }
</script>

<div class="spk-wrap" bind:this={host}>
  <svg
    class="chart-svg sparkline"
    viewBox="0 0 {width} {height}"
    width={width}
    height={height}
    role="img"
    aria-label={label ?? 'Trend sparkline'}
    preserveAspectRatio="none"
    onmousemove={onMove}
    onmouseleave={() => (hover = null)}
  >
    {#if path}
      <path d={path} />
      {#if hover}
        <line
          x1={hover.x} y1={0}
          x2={hover.x} y2={height}
          stroke="var(--ink)" stroke-width="0.5" opacity="0.4"
          vector-effect="non-scaling-stroke" />
        <circle cx={hover.x} cy={hover.y} r="2"
          fill="var(--accent-ink)" stroke="var(--bg)" stroke-width="1"
          vector-effect="non-scaling-stroke" />
      {/if}
    {/if}
  </svg>
  {#if hover}
    {@const ho = hover}
    {@const ts = timestamps[ho.idx]}
    <Tooltip x={ho.clientX} y={ho.clientY} container={host}>
      {#snippet children()}{ts ? `${ts}\n${values[ho.idx]}` : `idx ${ho.idx}\nvalue ${values[ho.idx]}`}{/snippet}
    </Tooltip>
  {/if}
</div>

<style>
  .spk-wrap {
    position: relative;
    display: inline-block;
    line-height: 0;
  }
  .chart-svg.sparkline {
    cursor: crosshair;
  }
</style>
