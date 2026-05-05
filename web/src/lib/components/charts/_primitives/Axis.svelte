<script lang="ts">
  interface Props {
    orientation: 'top' | 'right' | 'bottom' | 'left';
    /** Plot area extent on this axis (px). */
    extent: number;
    /** Domain min/max. */
    domain: [number, number];
    /** Approximate tick count. */
    ticks?: number;
    /** Format value to label string. */
    format?: (v: number) => string;
    /** Inline label. */
    label?: string;
    /** Scale. Currently 'linear' only. */
    scale?: 'linear';
  }

  let {
    orientation,
    extent,
    domain,
    ticks = 5,
    format = (v) => formatNumber(v),
    label,
    scale = 'linear',
  }: Props = $props();

  function niceTicks(min: number, max: number, count: number): number[] {
    if (!isFinite(min) || !isFinite(max) || min === max) return [min];
    const span = max - min;
    const rawStep = span / Math.max(1, count);
    const mag = Math.pow(10, Math.floor(Math.log10(rawStep)));
    const norm = rawStep / mag;
    const step = (norm < 1.5 ? 1 : norm < 3 ? 2 : norm < 7 ? 5 : 10) * mag;
    const start = Math.ceil(min / step) * step;
    const out: number[] = [];
    for (let v = start; v <= max + step / 2; v += step) {
      // Round-trip to clamp floating-point drift.
      out.push(Math.round(v / step) * step);
    }
    return out;
  }

  function formatNumber(v: number): string {
    const a = Math.abs(v);
    if (a === 0) return '0';
    if (a >= 1000) return v.toExponential(1);
    if (a < 0.01) return v.toExponential(1);
    return Number.isInteger(v) ? v.toString() : v.toFixed(2);
  }

  let tickValues = $derived(niceTicks(domain[0], domain[1], ticks));

  function project(v: number): number {
    const [d0, d1] = domain;
    if (d0 === d1) return 0;
    const t = (v - d0) / (d1 - d0);
    if (orientation === 'left' || orientation === 'right') {
      return extent - t * extent;
    }
    return t * extent;
  }

  let isHorizontal = $derived(orientation === 'top' || orientation === 'bottom');
</script>

<g class="axis" data-orientation={orientation}>
  {#if isHorizontal}
    <line x1="0" y1="0" x2={extent} y2="0" />
    {#each tickValues as t (t)}
      <g transform="translate({project(t)},0)">
        <line y2={orientation === 'bottom' ? 4 : -4} />
        <text
          y={orientation === 'bottom' ? 14 : -8}
          text-anchor="middle">{format(t)}</text>
      </g>
    {/each}
    {#if label}
      <text x={extent / 2} y={orientation === 'bottom' ? 32 : -22} text-anchor="middle" class="axis-label">{label}</text>
    {/if}
  {:else}
    <line x1="0" y1="0" x2="0" y2={extent} />
    {#each tickValues as t (t)}
      <g transform="translate(0,{project(t)})">
        <line x2={orientation === 'left' ? -4 : 4} />
        <text
          x={orientation === 'left' ? -8 : 8}
          dy="0.32em"
          text-anchor={orientation === 'left' ? 'end' : 'start'}>{format(t)}</text>
      </g>
    {/each}
    {#if label}
      <text
        transform="translate({orientation === 'left' ? -38 : 38},{extent / 2}) rotate({orientation === 'left' ? -90 : 90})"
        text-anchor="middle"
        class="axis-label">{label}</text>
    {/if}
  {/if}
</g>

<style>
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
  }
</style>
