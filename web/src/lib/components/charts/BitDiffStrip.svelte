<script lang="ts">
  // Bit-level XOR view of two byte buffers. Renders a wide strip with
  // 8 cells per byte; each cell is dark if A and B agree on that bit and
  // bright (in the diff colour) if they differ. Surfaces patterns that
  // a byte-level diff hides — e.g. a SimHash with one bit flipped looks
  // like one differing byte at the byte level but shows clearly here.

  type Props = {
    a: Uint8Array;
    b: Uint8Array;
    /** Cap bytes considered (avoid huge strips for streaming hashes). */
    maxBytes?: number;
  };

  let { a, b, maxBytes = 64 }: Props = $props();

  const n = $derived(Math.min(a.length, b.length, maxBytes));
  // Pre-compute the differ flags once per (a,b) pair.
  const flags = $derived.by(() => {
    const out: boolean[] = new Array(n * 8);
    for (let i = 0; i < n; i++) {
      const x = a[i] ^ b[i];
      for (let j = 0; j < 8; j++) out[i * 8 + j] = (x >> (7 - j) & 1) === 1;
    }
    return out;
  });

  const bitsTotal = $derived(n * 8);
  const bitsDiff  = $derived(flags.filter(Boolean).length);
  const matchPct  = $derived(bitsTotal === 0 ? 0 : ((1 - bitsDiff / bitsTotal) * 100));
</script>

<div class="bd-wrap" aria-label="Bit-level XOR diff">
  <div class="bd-strip" role="img" aria-label="{bitsDiff} of {bitsTotal} bits differ">
    {#each flags as differ}
      <span class="bd-cell" class:diff={differ}></span>
    {/each}
  </div>
  <div class="bd-meta">
    <span><strong>bits</strong> {bitsDiff} / {bitsTotal} differ</span>
    <span><strong>match</strong> {matchPct.toFixed(2)}%</span>
  </div>
</div>

<style>
  .bd-wrap { display: flex; flex-direction: column; gap: 0.3rem; }
  .bd-strip {
    display: grid;
    /* 8 cells per byte → autofill keeps row height stable across sizes */
    grid-template-columns: repeat(64, 1fr);
    gap: 1px;
    padding: 3px;
    background: var(--bg);
    border: 1px solid var(--ink);
    border-radius: 3px;
  }
  .bd-cell {
    display: block;
    aspect-ratio: 1;
    background: var(--bg-2);
    border-radius: 1px;
  }
  .bd-cell.diff { background: oklch(0.55 0.18 30); }
  .bd-meta {
    display: flex; justify-content: space-between;
    font-family: var(--mono); font-size: 0.62rem; color: var(--ink-2);
  }
  .bd-meta strong { color: var(--ink-2); font-weight: 400; text-transform: uppercase; letter-spacing: 0.06em; margin-right: 4px; }
  /* Smaller strips on narrow screens look better with fewer columns. */
  @media (max-width: 600px) {
    .bd-strip { grid-template-columns: repeat(32, 1fr); }
  }
</style>
