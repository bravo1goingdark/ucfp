<script lang="ts">
  // Constellation scatter for Wang / Panako audio landmark fingerprints.
  //
  // Wang   = 8 bytes per hash: u32 packed `f_a_q(9)|f_b_q(9)|Δt(14)`, then u32 t_anchor.
  // Panako = 16 bytes per hash: u32 hash + u32 t_anchor + u32 t_b + u32 t_c.
  //
  // We plot (t_anchor, anchor_freq) so users see the iconic Shazam-style
  // peak constellation that drove the original landmark idea. Frame index
  // is converted to seconds using the algorithm's known frame rate (62.5
  // for wang-v1 / panako-v2).

  type Algo = 'wang' | 'panako';

  type Props = {
    bytes: Uint8Array;
    algo: Algo;
    height?: number;
    framesPerSec?: number;
  };

  let { bytes, algo, height = 140, framesPerSec = 62.5 }: Props = $props();

  const STRIDE = $derived(algo === 'wang' ? 8 : 16);
  const FREQ_BUCKETS = 512;

  // Read the 32-bit anchor-frequency field MSB-first from the packed hash.
  function freqOf(view: DataView, off: number): number {
    const hash = view.getUint32(off, true); // little-endian
    return (hash >>> 23) & 0x1FF;
  }
  function tOf(view: DataView, off: number): number {
    return view.getUint32(off + 4, true);
  }

  const points = $derived.by(() => {
    if (bytes.length === 0 || bytes.length % STRIDE !== 0) return [];
    const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    const n = Math.min(bytes.length / STRIDE, 4096); // cap render
    const out: { t: number; f: number }[] = new Array(n);
    for (let i = 0; i < n; i++) {
      const off = i * STRIDE;
      out[i] = { t: tOf(view, off), f: freqOf(view, off) };
    }
    return out;
  });

  const tMax = $derived.by(() => {
    let m = 0;
    for (const p of points) if (p.t > m) m = p.t;
    return m || 1;
  });
  const durationSec = $derived(tMax / framesPerSec);
</script>

{#if points.length > 0}
  <div class="ls-wrap">
    <svg viewBox="0 0 100 {height}" preserveAspectRatio="none" class="ls-svg" role="img"
         aria-label="{points.length} landmarks across {durationSec.toFixed(1)}s">
      <!-- horizontal grid lines for frequency quartiles -->
      {#each [0.25, 0.5, 0.75] as q}
        <line x1="0" y1={height * q} x2="100" y2={height * q}
              stroke="var(--ink)" stroke-width="0.1" opacity="0.2" />
      {/each}
      {#each points as p}
        {@const x = (p.t / tMax) * 100}
        {@const y = (1 - p.f / FREQ_BUCKETS) * height}
        <circle cx={x} cy={y} r="0.45"
                fill="var(--accent-ink, oklch(0.55 0.18 240))"
                opacity="0.65" />
      {/each}
    </svg>
    <div class="ls-meta">
      <span><strong>landmarks</strong> {points.length}</span>
      <span><strong>span</strong> {durationSec.toFixed(2)}s</span>
      <span><strong>density</strong> {(points.length / Math.max(0.1, durationSec)).toFixed(1)}/s</span>
      <span class="ls-axis">freq ↑ · time →</span>
    </div>
  </div>
{:else}
  <p class="ls-warn">expected stride-{STRIDE} bytes; got {bytes.length}</p>
{/if}

<style>
  .ls-wrap { display: flex; flex-direction: column; gap: 0.3rem; }
  .ls-svg {
    width: 100%; display: block;
    background: var(--bg);
    border: 1px solid var(--ink); border-radius: 4px;
    /* Light grid lines visible in both themes. */
  }
  .ls-meta {
    display: flex; gap: 0.85rem; flex-wrap: wrap;
    font-family: var(--mono); font-size: 0.62rem; color: var(--ink-2);
  }
  .ls-meta strong { font-weight: 400; text-transform: uppercase; letter-spacing: 0.06em; margin-right: 4px; }
  .ls-axis { margin-left: auto; opacity: 0.7; }
  .ls-warn { font-family: var(--mono); font-size: 0.7rem; color: #b03030; margin: 0; }
</style>
