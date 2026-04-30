<script lang="ts">
  // 64-bit hash → 8×8 bit grid. Used for SimHash bodies and the
  // `global_hash: u64` slot inside an ImageFingerprint. Optionally
  // diff-shaded against a second hash so MultiHash compare mode can
  // highlight which bits flipped.

  type Props = {
    /** 8 bytes of LE-encoded u64 hash data. Anything else is rejected. */
    hashBytes: Uint8Array;
    /** Optional second hash for diff highlight. */
    diffAgainst?: Uint8Array;
    /** Edge length in CSS px of the entire grid. */
    size?: number;
    /** Caption rendered under the grid. */
    label?: string;
  };

  let { hashBytes, diffAgainst, size = 88, label }: Props = $props();

  const ok = $derived(hashBytes.length === 8);
  const diffOk = $derived(!diffAgainst || diffAgainst.length === 8);

  // Build 64 bits. Bit ordering: byte 0 holds the highest-order 8 bits,
  // and within each byte we walk MSB→LSB so reading left-to-right,
  // top-to-bottom matches the natural u64 hex view.
  const bits = $derived.by(() => {
    if (!ok) return [] as boolean[];
    const out: boolean[] = new Array(64);
    for (let i = 0; i < 8; i++) {
      const b = hashBytes[i];
      for (let j = 0; j < 8; j++) out[i * 8 + j] = ((b >> (7 - j)) & 1) === 1;
    }
    return out;
  });

  const diffBits = $derived.by(() => {
    if (!diffAgainst || !diffOk || !ok) return null;
    const out: boolean[] = new Array(64);
    for (let i = 0; i < 8; i++) {
      const x = hashBytes[i] ^ diffAgainst[i];
      for (let j = 0; j < 8; j++) out[i * 8 + j] = ((x >> (7 - j)) & 1) === 1;
    }
    return out;
  });

  const popcount = $derived(bits.filter(Boolean).length);
  const flipCount = $derived(diffBits ? diffBits.filter(Boolean).length : 0);
</script>

{#if ok}
  <div class="bg-wrap" style="width:{size}px">
    <div class="bg-grid" role="img" aria-label={label ?? `${popcount} bits set`}>
      {#each bits as on, i}
        <span class="bg-cell"
          class:on
          class:flip={diffBits && diffBits[i]}></span>
      {/each}
    </div>
    {#if label}<div class="bg-label">{label}</div>{/if}
    <div class="bg-meta">
      {#if diffBits}
        <span><strong>flips</strong> {flipCount}/64</span>
      {:else}
        <span><strong>set</strong> {popcount}/64</span>
      {/if}
    </div>
  </div>
{/if}

<style>
  .bg-wrap { display: flex; flex-direction: column; gap: 0.25rem; }
  .bg-grid {
    display: grid;
    grid-template-columns: repeat(8, 1fr);
    gap: 1px; padding: 2px;
    background: var(--ink); border-radius: 3px;
    aspect-ratio: 1;
  }
  .bg-cell {
    background: var(--bg);
    border-radius: 1px;
    transition: background 0.12s;
  }
  .bg-cell.on { background: var(--accent-ink, oklch(0.55 0.18 240)); }
  /* Diff overrides plain colour — bright red ring around bits that flipped. */
  .bg-cell.flip {
    background: oklch(0.6 0.2 30);
    box-shadow: inset 0 0 0 1px oklch(0.4 0.18 30);
  }
  .bg-label {
    font-family: var(--mono); font-size: 0.62rem;
    color: var(--ink-2); text-transform: uppercase; letter-spacing: 0.06em;
    text-align: center;
  }
  .bg-meta {
    font-family: var(--mono); font-size: 0.6rem;
    color: var(--ink-2); text-align: center;
  }
  .bg-meta strong { font-weight: 400; margin-right: 4px; }
</style>
