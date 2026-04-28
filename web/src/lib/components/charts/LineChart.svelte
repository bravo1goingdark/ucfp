<script module lang="ts">
  export interface Series {
    label: string;
    /** One value per x-tick. Length must match `xLabels.length`. */
    values: number[];
    /** CSS color string. Defaults to var(--accent-ink) cycled by index. */
    color?: string;
  }
</script>

<script lang="ts">
  // Multi-series SVG line chart with axes + hover tooltip.
  //
  // Tooltip approach (no library): each data point is a small <circle>
  // with onmouseenter setting `hovered` state, onmouseleave clearing it.
  // A tooltip <g> renders conditionally inside the SVG, positioned at
  // the hovered point. `pointer-events: none` on the tooltip prevents
  // it from stealing mouse events. SSR-safe — `hovered` starts null.

  interface Props {
    series: Series[];
    xLabels: string[];
    width?: number;
    height?: number;
    yAxisLabel?: string;
  }

  let {
    series,
    xLabels,
    width = 720,
    height = 240,
    yAxisLabel
  }: Props = $props();

  // Layout — inner plot area inside padding for axes + labels.
  const pad = { top: 12, right: 16, bottom: 28, left: 40 };
  const innerW = $derived(width - pad.left - pad.right);
  const innerH = $derived(height - pad.top - pad.bottom);

  const max = $derived.by(() => {
    let m = 0;
    for (const s of series) for (const v of s.values) if (v > m) m = v;
    return m === 0 ? 1 : m;
  });

  function fallbackColor(i: number): string {
    // Three muted hues; reuse --accent-ink as the primary stroke.
    const palette = ['var(--accent-ink)', 'var(--ink-2)', 'var(--muted)'];
    return palette[i % palette.length];
  }

  function pointX(i: number, n: number): number {
    if (n <= 1) return pad.left + innerW / 2;
    return pad.left + (i / (n - 1)) * innerW;
  }

  function pointY(v: number): number {
    return pad.top + (1 - v / max) * innerH;
  }

  function pathFor(values: number[]): string {
    return values
      .map((v, i) => `${i === 0 ? 'M' : 'L'}${pointX(i, values.length).toFixed(2)} ${pointY(v).toFixed(2)}`)
      .join(' ');
  }

  // Y-axis ticks (4 evenly spaced).
  const yTicks = $derived.by(() => {
    const ticks: { y: number; v: number }[] = [];
    for (let i = 0; i <= 4; i++) {
      const v = (max * i) / 4;
      ticks.push({ v, y: pointY(v) });
    }
    return ticks;
  });

  // Hover state — null on SSR + before any mouseenter fires.
  type Hover = {
    seriesIndex: number;
    pointIndex: number;
    x: number;
    y: number;
    label: string;
    value: number;
    xLabel: string;
  };
  let hovered = $state<Hover | null>(null);

  function fmtNumber(n: number): string {
    if (n >= 1000) return (n / 1000).toFixed(1) + 'k';
    return Math.round(n).toString();
  }
</script>

<div class="linechart-wrap">
  <svg
    class="chart-svg linechart"
    viewBox="0 0 {width} {height}"
    width="100%"
    height={height}
    role="img"
    aria-label={yAxisLabel ?? 'Time series chart'}
  >
    <!-- y axis grid + labels -->
    <g class="axis y-axis" aria-hidden="true">
      {#each yTicks as t (t.v)}
        <line x1={pad.left} x2={width - pad.right} y1={t.y} y2={t.y} />
        <text x={pad.left - 6} y={t.y + 3} text-anchor="end">{fmtNumber(t.v)}</text>
      {/each}
    </g>

    <!-- x axis labels (subset for readability) -->
    <g class="axis x-axis" aria-hidden="true">
      {#each xLabels as lbl, i (i)}
        {#if i === 0 || i === xLabels.length - 1 || i % Math.max(1, Math.ceil(xLabels.length / 6)) === 0}
          <text
            x={pointX(i, xLabels.length)}
            y={height - pad.bottom + 16}
            text-anchor="middle"
          >{lbl}</text>
        {/if}
      {/each}
    </g>

    <!-- series paths -->
    {#each series as s, si (s.label)}
      <path
        d={pathFor(s.values)}
        stroke={s.color ?? fallbackColor(si)}
        fill="none"
      />
    {/each}

    <!-- data dots (hover targets) -->
    {#each series as s, si (s.label)}
      {#each s.values as v, pi (pi)}
        <circle
          class="dot"
          role="img"
          aria-label="{s.label} {xLabels[pi] ?? ''}: {v}"
          cx={pointX(pi, s.values.length)}
          cy={pointY(v)}
          r="4"
          fill={s.color ?? fallbackColor(si)}
          onmouseenter={() => {
            hovered = {
              seriesIndex: si,
              pointIndex: pi,
              x: pointX(pi, s.values.length),
              y: pointY(v),
              label: s.label,
              value: v,
              xLabel: xLabels[pi] ?? ''
            };
          }}
          onmouseleave={() => {
            hovered = null;
          }}
        ></circle>
      {/each}
    {/each}

    <!-- tooltip -->
    {#if hovered}
      {@const tw = 130}
      {@const th = 38}
      {@const tx = Math.min(Math.max(hovered.x - tw / 2, pad.left), width - pad.right - tw)}
      {@const ty = Math.max(hovered.y - th - 10, pad.top)}
      <g class="tooltip" pointer-events="none" transform="translate({tx} {ty})">
        <rect width={tw} height={th} rx="2"></rect>
        <text x="8" y="14">{hovered.label} · {hovered.xLabel}</text>
        <text x="8" y="30" class="val">{fmtNumber(hovered.value)} requests</text>
      </g>
    {/if}
  </svg>

  {#if series.length > 1}
    <ul class="chart-legend">
      {#each series as s, si (s.label)}
        <li>
          <span class="swatch" style="background:{s.color ?? fallbackColor(si)}"></span>
          <span>{s.label}</span>
        </li>
      {/each}
    </ul>
  {/if}
</div>
