<script lang="ts">
  // Bit-level XOR view of two byte buffers. Renders a wide strip with
  // 8 cells per byte; each cell is dark if A and B agree on that bit and
  // bright (in the diff colour) if they differ. Surfaces patterns that
  // a byte-level diff hides — e.g. a SimHash with one bit flipped looks
  // like one differing byte at the byte level but shows clearly here.

  import Tooltip from './_primitives/Tooltip.svelte';

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

  let host: HTMLDivElement | null = $state(null);
  let hover = $state<{ x: number; y: number; text: string } | null>(null);
</script>

<div class="bd-wrap" bind:this={host} aria-label="Bit-level XOR diff">
  <div class="bd-strip" role="img" aria-label="{bitsDiff} of {bitsTotal} bits differ"
       onmouseleave={() => (hover = null)}>
    {#each flags as differ, i (i)}
      {@const byteIdx = Math.floor(i / 8)}
      {@const bitIdx = 7 - (i % 8)}
      {@const isByteBoundary = i % 64 === 0 && i !== 0}
      <span class="bd-cell"
        class:diff={differ}
        class:boundary={isByteBoundary}
        role="img"
        aria-label={`byte ${byteIdx} bit ${bitIdx} ${differ ? 'differs' : 'agrees'}`}
        onmousemove={(e: MouseEvent) => {
          const av = a[byteIdx]?.toString(16).padStart(2, '0') ?? '??';
          const bv = b[byteIdx]?.toString(16).padStart(2, '0') ?? '??';
          hover = {
            x: e.clientX,
            y: e.clientY,
            text: `byte ${byteIdx} · bit ${bitIdx}\nA=0x${av} · B=0x${bv}\n${differ ? '✗ differs' : '✓ agrees'}`,
          };
        }}></span>
    {/each}
  </div>
  <div class="bd-meta">
    <span><strong>bits</strong> {bitsDiff} / {bitsTotal} differ</span>
    <span><strong>match</strong> {matchPct.toFixed(2)}%</span>
  </div>
  {#if hover}
    {@const h = hover}
    <Tooltip x={h.x} y={h.y} container={host}>
      {#snippet children()}{h.text}{/snippet}
    </Tooltip>
  {/if}
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
    cursor: crosshair;
    transition: transform 0.1s;
  }
  .bd-cell:hover {
    transform: scale(1.4);
    z-index: 1;
    position: relative;
  }
  .bd-cell.diff { background: oklch(0.55 0.18 30); }
  /* Visual marker every 8 bytes so users can count offsets at a glance. */
  .bd-cell.boundary {
    box-shadow: -1px 0 0 0 var(--ink);
  }
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
