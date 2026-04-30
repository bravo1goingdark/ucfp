<script lang="ts">
  // Centered horizontal bars for a dense embedding vector. Bars rise
  // upward for positive components and downward for negative, with bar
  // colour tracking sign. Designed for ≤256 dims at a glance — for
  // larger vectors we sample uniformly to keep the SVG tractable.

  type Props = {
    vector: number[];
    /** Maximum bars to draw — over this we sample uniformly. */
    maxBars?: number;
    /** Height in CSS px of the chart area (excluding axis line). */
    height?: number;
    /** Colour for positive components (defaults to --accent-ink). */
    posColor?: string;
    /** Colour for negative components. */
    negColor?: string;
  };

  let {
    vector,
    maxBars = 128,
    height = 80,
    posColor = 'var(--accent-ink, oklch(0.55 0.18 240))',
    negColor = 'oklch(0.55 0.18 30)'
  }: Props = $props();

  // Subsample if the vector is longer than maxBars.
  const sampled = $derived.by(() => {
    if (vector.length <= maxBars) return vector;
    const step = vector.length / maxBars;
    const out: number[] = new Array(maxBars);
    for (let i = 0; i < maxBars; i++) out[i] = vector[Math.floor(i * step)];
    return out;
  });

  // Symmetric range so positive and negative bars share a scale.
  const peak = $derived.by(() => {
    let m = 0;
    for (const v of sampled) {
      const a = Math.abs(v);
      if (a > m) m = a;
    }
    return m || 1;
  });

  const halfH = $derived(height / 2);
  const barW = $derived(100 / sampled.length); // viewBox is 100 wide
</script>

<div class="emb-wrap" aria-label="Embedding visualization">
  <svg viewBox="0 0 100 {height}" preserveAspectRatio="none" class="emb-svg" role="img">
    <line x1="0" y1={halfH} x2="100" y2={halfH}
          stroke="var(--ink)" stroke-width="0.15" opacity="0.4" />
    {#each sampled as v, i}
      {@const h = (Math.abs(v) / peak) * (halfH - 1)}
      {@const x = i * barW}
      {@const y = v >= 0 ? halfH - h : halfH}
      <rect x={x + barW * 0.08} y={y}
            width={barW * 0.84} height={Math.max(0.4, h)}
            fill={v >= 0 ? posColor : negColor} />
    {/each}
  </svg>
  <div class="emb-meta">
    <span><strong>dim</strong> {vector.length}{sampled.length < vector.length ? ` (sampled to ${sampled.length})` : ''}</span>
    <span><strong>peak</strong> {peak.toFixed(3)}</span>
    <span><strong>L2</strong> {Math.sqrt(vector.reduce((a, v) => a + v * v, 0)).toFixed(3)}</span>
  </div>
</div>

<style>
  .emb-wrap { display: flex; flex-direction: column; gap: 0.4rem; }
  .emb-svg {
    width: 100%; display: block;
    background: var(--bg);
    border: 1px solid var(--ink); border-radius: 4px;
  }
  .emb-meta {
    display: flex; gap: 0.75rem; flex-wrap: wrap;
    font-family: var(--mono); font-size: 0.65rem; color: var(--ink-2);
  }
  .emb-meta strong {
    color: var(--ink-2); font-weight: 400;
    text-transform: uppercase; letter-spacing: 0.06em;
    margin-right: 4px;
  }
</style>
