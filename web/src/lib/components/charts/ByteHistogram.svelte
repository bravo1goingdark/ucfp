<script lang="ts">
  // 16-bucket histogram of byte values (one bucket per high nibble).
  // Visually surfaces low-entropy patterns: a flat bar profile means
  // the fingerprint distributes evenly; spikes mean structure (or a
  // bug). Cheap one-pass, SSR-safe, no DOM measurement.

  import Tooltip from './_primitives/Tooltip.svelte';

  type Props = {
    bytes: Uint8Array | null;
    height?: number;
    /** Hint label rendered under the bars. Defaults to "byte distribution". */
    label?: string;
  };

  let { bytes, height = 48, label = 'byte distribution' }: Props = $props();

  let host: HTMLDivElement | null = $state(null);
  let hover = $state<{ x: number; y: number; text: string } | null>(null);
  const HEX_DIGITS = '0123456789ABCDEF';

  const buckets = $derived.by(() => {
    const out = new Array<number>(16).fill(0);
    if (!bytes) return out;
    for (const b of bytes) out[b >> 4]++;
    return out;
  });

  const peak = $derived.by(() => {
    let m = 0;
    for (const v of buckets) if (v > m) m = v;
    return m || 1;
  });

  const total = $derived(buckets.reduce((a, v) => a + v, 0));
  // Shannon entropy in bits over the 16 buckets, normalised to [0,1].
  const entropyNorm = $derived.by(() => {
    if (total === 0) return 0;
    let h = 0;
    for (const v of buckets) {
      if (v === 0) continue;
      const p = v / total;
      h -= p * Math.log2(p);
    }
    return h / 4; // log2(16) = 4
  });
</script>

<div class="bh-wrap" bind:this={host} aria-label={label}>
  <svg viewBox="0 0 100 {height}" preserveAspectRatio="none" class="bh-svg" role="img"
       onmouseleave={() => (hover = null)}>
    {#each buckets as v, i (i)}
      {@const h = (v / peak) * (height - 2)}
      <rect
        x={i * (100 / 16) + 0.4}
        y={height - h}
        width={100 / 16 - 0.8}
        height={Math.max(0.5, h)}
        fill="var(--ink)"
        opacity={0.55 + 0.45 * (v / peak)}
        role="img"
        aria-label={`high nibble 0x${HEX_DIGITS[i]}: ${v} bytes (${total > 0 ? ((v / total) * 100).toFixed(1) : 0}%)`}
        onmousemove={(e: MouseEvent) => {
          hover = {
            x: e.clientX,
            y: e.clientY,
            text: `0x${HEX_DIGITS[i]}_ (high nibble)\n${v} bytes (${total > 0 ? ((v / total) * 100).toFixed(1) : 0}%)`,
          };
        }}
      />
    {/each}
    <!-- Bucket labels along the bottom (0–F). Drawn last so they read
         on top of any tall bars. -->
    {#each buckets as _, i (i)}
      <text x={i * (100 / 16) + (100 / 32)} y={height - 1}
            text-anchor="middle"
            class="bh-tick">{HEX_DIGITS[i]}</text>
    {/each}
  </svg>
  <div class="bh-meta">
    <span>{label}</span>
    <span><strong>uniformity</strong> {(entropyNorm * 100).toFixed(0)}%</span>
  </div>
  {#if hover}
    {@const ho = hover}
    <Tooltip x={ho.x} y={ho.y} container={host}>
      {#snippet children()}{ho.text}{/snippet}
    </Tooltip>
  {/if}
</div>

<style>
  .bh-wrap { display: flex; flex-direction: column; gap: 0.25rem; }
  .bh-svg {
    width: 100%; display: block;
    background: var(--bg);
    border: 1px solid var(--ink); border-radius: 4px;
  }
  .bh-svg rect {
    cursor: crosshair;
    transition: opacity 0.1s;
  }
  .bh-svg rect:hover {
    opacity: 1 !important;
  }
  .bh-tick {
    font-family: var(--mono);
    font-size: 3.2px;
    fill: var(--ink-2);
    pointer-events: none;
    opacity: 0.55;
  }
  .bh-meta {
    display: flex; justify-content: space-between;
    font-family: var(--mono); font-size: 0.6rem; color: var(--ink-2);
    text-transform: uppercase; letter-spacing: 0.06em;
  }
  .bh-meta strong { font-weight: 400; margin-right: 4px; }
</style>
