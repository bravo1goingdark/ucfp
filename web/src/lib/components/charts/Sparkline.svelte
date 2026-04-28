<script lang="ts">
  // Tiny single-series SVG sparkline. ~80×24 by default, no axes.
  // SSR-safe: pure data → path string.

  interface Props {
    values: number[];
    width?: number;
    height?: number;
    label?: string;
  }

  let { values, width = 80, height = 24, label }: Props = $props();

  const path = $derived.by(() => {
    if (values.length === 0) return '';
    const max = Math.max(...values, 1);
    const min = Math.min(...values, 0);
    const range = max - min || 1;
    const stepX = values.length > 1 ? width / (values.length - 1) : 0;
    return values
      .map((v, i) => {
        const x = i * stepX;
        const y = height - ((v - min) / range) * height;
        return `${i === 0 ? 'M' : 'L'}${x.toFixed(2)} ${y.toFixed(2)}`;
      })
      .join(' ');
  });
</script>

<svg
  class="chart-svg sparkline"
  viewBox="0 0 {width} {height}"
  width={width}
  height={height}
  role="img"
  aria-label={label ?? 'Trend sparkline'}
  preserveAspectRatio="none"
>
  {#if path}
    <path d={path} />
  {/if}
</svg>
