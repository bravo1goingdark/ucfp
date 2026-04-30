<script lang="ts">
  // Visualise an `imgfprint::ImageFingerprint` (168 bytes packed, repr(C)):
  //   exact:        [u8; 32]
  //   global_hash:  u64        (8 bytes)
  //   block_hashes: [u64; 16]  (128 bytes — a 4×4 grid of 64-bit hashes)
  //
  // The view shows the 8×8 global bit grid on the left and the 4×4
  // block-hash overview on the right (each cell coloured by popcount of
  // the underlying u64 — light = sparse, dark = dense). Both halves
  // gain diff highlights when a second fingerprint is supplied.

  import BitGrid8x8 from './BitGrid8x8.svelte';

  type Props = {
    /** Exactly 168 bytes (one ImageFingerprint). */
    bytes: Uint8Array;
    /** Optional second fingerprint to diff against. */
    diffAgainst?: Uint8Array;
    /** Subtitle (e.g. "PHash"). */
    label?: string;
  };

  let { bytes, diffAgainst, label }: Props = $props();
  const ok = $derived(bytes.length === 168);

  function sliceGlobal(b: Uint8Array): Uint8Array { return b.subarray(32, 40); }
  function sliceBlock(b: Uint8Array, i: number): Uint8Array {
    const off = 40 + i * 8;
    return b.subarray(off, off + 8);
  }
  function popcount(b: Uint8Array): number {
    let n = 0;
    for (const x of b) {
      let v = x;
      v = v - ((v >> 1) & 0x55);
      v = (v & 0x33) + ((v >> 2) & 0x33);
      n += ((v + (v >> 4)) & 0x0F);
    }
    return n;
  }
  function hammingBits(a: Uint8Array, b: Uint8Array): number {
    let n = 0;
    const m = Math.min(a.length, b.length);
    for (let i = 0; i < m; i++) {
      let v = a[i] ^ b[i];
      v = v - ((v >> 1) & 0x55);
      v = (v & 0x33) + ((v >> 2) & 0x33);
      n += ((v + (v >> 4)) & 0x0F);
    }
    return n;
  }
  function bytesToHex(b: Uint8Array): string {
    let s = '';
    for (const x of b) s += x.toString(16).padStart(2, '0');
    return s;
  }

  const exact = $derived(ok ? bytes.subarray(0, 32) : null);
  const global = $derived(ok ? sliceGlobal(bytes) : null);
  const blockPopcounts = $derived.by(() => {
    if (!ok) return [];
    return Array.from({ length: 16 }, (_, i) => popcount(sliceBlock(bytes, i)));
  });
  const blockDiffs = $derived.by(() => {
    if (!ok || !diffAgainst || diffAgainst.length !== 168) return null;
    return Array.from({ length: 16 }, (_, i) =>
      hammingBits(sliceBlock(bytes, i), sliceBlock(diffAgainst, i))
    );
  });
</script>

{#if ok}
  <div class="ihv-wrap">
    {#if label}<div class="ihv-label">{label}</div>{/if}
    <div class="ihv-row">
      <BitGrid8x8 hashBytes={global!} diffAgainst={diffAgainst ? sliceGlobal(diffAgainst) : undefined} size={88} label="global · 8×8" />

      <div class="ihv-blocks-wrap">
        <div class="ihv-blocks" role="img" aria-label="4×4 block-hash overview">
          {#each blockPopcounts as pc, i}
            {@const intensity = pc / 64}
            {@const diff = blockDiffs ? blockDiffs[i] : 0}
            {@const flipped = diff > 0}
            <div class="ihv-block"
                 class:flipped
                 style="background:oklch(0.95 0 0 / {0.15 + intensity * 0.85})"
                 title={blockDiffs
                   ? `block ${i}: ${pc} bits set · ${diff} differ`
                   : `block ${i}: ${pc} bits set`}>
              <span class="ihv-block-n">{pc}</span>
            </div>
          {/each}
        </div>
        <div class="ihv-blocks-cap">block hashes · 4×4 (popcount)</div>
      </div>
    </div>
    <div class="ihv-exact mono" title={bytesToHex(exact!)}>
      exact: {bytesToHex(exact!).slice(0, 24)}…
    </div>
  </div>
{:else}
  <p class="ihv-warn">expected 168 bytes, got {bytes.length}</p>
{/if}

<style>
  .ihv-wrap {
    display: flex; flex-direction: column; gap: 0.4rem;
    padding: 0.6rem; background: var(--bg-2);
    border: 1px solid var(--ink); border-radius: 5px;
  }
  .ihv-label {
    font-family: var(--mono); font-size: 0.7rem; font-weight: 600;
    text-transform: uppercase; letter-spacing: 0.06em; color: var(--ink);
  }
  .ihv-row { display: flex; gap: 0.75rem; align-items: flex-start; }

  .ihv-blocks-wrap { display: flex; flex-direction: column; gap: 0.25rem; flex: 1; min-width: 0; }
  .ihv-blocks {
    display: grid; grid-template-columns: repeat(4, 1fr); gap: 2px;
    padding: 2px; background: var(--ink); border-radius: 3px;
    aspect-ratio: 1; max-width: 88px;
  }
  .ihv-block {
    display: flex; align-items: center; justify-content: center;
    border-radius: 1px;
    color: oklch(0.2 0 0);
    font-family: var(--mono); font-size: 0.6rem;
    transition: background 0.12s;
  }
  .ihv-block.flipped {
    box-shadow: inset 0 0 0 2px oklch(0.6 0.2 30);
  }
  .ihv-block-n { opacity: 0.7; }
  .ihv-blocks-cap {
    font-family: var(--mono); font-size: 0.58rem;
    color: var(--ink-2); text-transform: uppercase; letter-spacing: 0.06em;
  }
  .ihv-exact {
    font-family: var(--mono); font-size: 0.62rem; color: var(--ink-2);
    word-break: break-all;
  }
  .ihv-warn { font-family: var(--mono); font-size: 0.7rem; color: #b03030; margin: 0; }
  .mono { font-family: var(--mono); }
</style>
