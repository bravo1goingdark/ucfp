<script module lang="ts">
  export interface Slice {
    label: string;
    value: number;
    color?: string;
  }
</script>

<script lang="ts">
  // Proportional ring SVG, color-coded with a legend.
  // SSR-safe — pure data → arc strings.

  import Tooltip from './_primitives/Tooltip.svelte';

  interface Props {
    data: Slice[];
    size?: number;
    /** Donut hole radius as a fraction of outer radius. */
    holeFraction?: number;
  }

  let { data, size = 160, holeFraction = 0.6 }: Props = $props();

  const total = $derived(data.reduce((a, s) => a + s.value, 0));

  let host: HTMLDivElement | null = $state(null);
  let hover = $state<{ x: number; y: number; text: string; idx: number } | null>(null);

  function fallbackColor(i: number): string {
    const palette = ['var(--accent-ink)', 'var(--ink-2)', 'var(--muted)', 'var(--accent)'];
    return palette[i % palette.length];
  }

  // Build SVG arc paths. We use a single arc per slice, drawn from the
  // top (12 o'clock) clockwise. Each slice = annular sector.
  type Arc = { d: string; color: string; label: string; value: number; pct: number };

  const arcs = $derived.by<Arc[]>(() => {
    const cx = size / 2;
    const cy = size / 2;
    const r = size / 2;
    const ri = r * holeFraction;
    if (total === 0) return [];

    let acc = 0;
    return data.map((s, i): Arc => {
      const startAngle = (acc / total) * Math.PI * 2 - Math.PI / 2;
      acc += s.value;
      const endAngle = (acc / total) * Math.PI * 2 - Math.PI / 2;
      const large = endAngle - startAngle > Math.PI ? 1 : 0;

      const x1 = cx + r * Math.cos(startAngle);
      const y1 = cy + r * Math.sin(startAngle);
      const x2 = cx + r * Math.cos(endAngle);
      const y2 = cy + r * Math.sin(endAngle);

      const xi1 = cx + ri * Math.cos(endAngle);
      const yi1 = cy + ri * Math.sin(endAngle);
      const xi2 = cx + ri * Math.cos(startAngle);
      const yi2 = cy + ri * Math.sin(startAngle);

      // Full-ring edge case: SVG won't draw a 360° arc with a single path.
      // When a single slice is 100%, draw two halves.
      if (s.value === total) {
        const mxOuter = cx - r;
        const myOuter = cy;
        const d =
          `M${cx} ${cy - r}` +
          `A${r} ${r} 0 0 1 ${mxOuter} ${myOuter}` +
          `A${r} ${r} 0 0 1 ${cx} ${cy - r}` +
          `M${cx} ${cy - ri}` +
          `A${ri} ${ri} 0 0 0 ${cx - ri} ${cy}` +
          `A${ri} ${ri} 0 0 0 ${cx} ${cy - ri}Z`;
        return {
          d,
          color: s.color ?? fallbackColor(i),
          label: s.label,
          value: s.value,
          pct: 1
        };
      }

      const d =
        `M${x1.toFixed(2)} ${y1.toFixed(2)}` +
        `A${r} ${r} 0 ${large} 1 ${x2.toFixed(2)} ${y2.toFixed(2)}` +
        `L${xi1.toFixed(2)} ${yi1.toFixed(2)}` +
        `A${ri} ${ri} 0 ${large} 0 ${xi2.toFixed(2)} ${yi2.toFixed(2)}` +
        `Z`;
      return {
        d,
        color: s.color ?? fallbackColor(i),
        label: s.label,
        value: s.value,
        pct: s.value / total
      };
    });
  });
</script>

<div class="donut-wrap" bind:this={host}>
  <svg
    class="chart-svg donut"
    viewBox="0 0 {size} {size}"
    width={size}
    height={size}
    role="img"
    aria-label="Modality breakdown"
    onmouseleave={() => (hover = null)}
  >
    {#if total === 0}
      <circle
        cx={size / 2}
        cy={size / 2}
        r={size / 2 - 1}
        fill="none"
        stroke="var(--line-strong)"
      />
    {:else}
      {#each arcs as a, i (i)}
        <path d={a.d} fill={a.color} class="donut-slice" class:active={hover?.idx === i}
          role="img"
          aria-label={`${a.label}: ${a.value} (${(a.pct * 100).toFixed(1)}%)`}
          onmousemove={(e: MouseEvent) => {
            hover = {
              x: e.clientX,
              y: e.clientY,
              idx: i,
              text: `${a.label}\n${a.value} · ${(a.pct * 100).toFixed(1)}%`,
            };
          }} />
      {/each}
    {/if}
    <text
      x={size / 2}
      y={size / 2 + 4}
      text-anchor="middle"
      class="donut-total"
    >{hover ? data[hover.idx]?.value ?? total : total}</text>
  </svg>

  <ul class="chart-legend donut-legend">
    {#each data as s, i (s.label)}
      <li class:active={hover?.idx === i}>
        <span class="swatch" style="background:{s.color ?? fallbackColor(i)}"></span>
        <span class="lbl">{s.label}</span>
        <span class="val">{s.value}{total > 0 ? ` · ${((s.value / total) * 100).toFixed(0)}%` : ''}</span>
      </li>
    {/each}
  </ul>

  {#if hover}
    {@const ho = hover}
    <Tooltip x={ho.x} y={ho.y} container={host}>
      {#snippet children()}{ho.text}{/snippet}
    </Tooltip>
  {/if}
</div>

<style>
  .donut-slice {
    cursor: pointer;
    transition: opacity 0.12s, transform 0.12s;
    transform-origin: center;
  }
  .donut-slice:hover,
  .donut-slice.active {
    filter: brightness(1.1);
  }
  .donut-wrap :global(.donut-legend li) {
    transition: opacity 0.1s;
  }
  .donut-wrap :global(.donut-legend li.active) {
    background: rgba(20, 20, 20, 0.05);
    border-radius: 2px;
  }
</style>
