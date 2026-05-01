<!--
  MinHashSlotHeatmap — H × 1 colour-coded grid over a MinHash signature.

  txtfp ships `MinHashSig<H>` as `repr(C)` + `bytemuck::Pod`, so the
  fingerprint bytes are exactly H × 8 little-endian u64 slot values
  (H = 128 by default — 1024 bytes). Each slot is rendered as a tiny
  cell whose hue encodes the high bits of the slot value: visually
  similar fingerprints look like similar mosaics; collisions stand out
  as identically-coloured cells in the same column when diff'ed.

  When `diffAgainst` is supplied with the same H, cells where the slot
  value matches are dimmed and matching cells are highlighted (the LSH
  banded-collision intuition made visible).
-->
<script lang="ts">
  type Props = {
    /** MinHash signature bytes — must be a positive multiple of 8. */
    bytes: Uint8Array;
    /** Optional second signature for compare-mode diff highlight. */
    diffAgainst?: Uint8Array;
    /** Caption shown beneath. */
    label?: string;
    /** CSS px width of the whole strip. */
    width?: number;
    /** CSS px height of the whole strip. */
    height?: number;
  };
  let { bytes, diffAgainst, label, width = 384, height = 56 }: Props = $props();

  const slotCount = $derived(Math.floor(bytes.length / 8));
  const ok = $derived(slotCount > 0 && bytes.length % 8 === 0);
  const diffOk = $derived(!diffAgainst || diffAgainst.length === bytes.length);

  /**
   * Read one u64 slot as a 16-bit hue seed (top 16 bits of the LE u64).
   * 16 bits is enough to make near-identical signatures look near-identical
   * and keep the colour palette stable across reloads.
   */
  function slotHue(buf: Uint8Array, i: number): number {
    const off = i * 8;
    // bytes[off+7] is the high byte (LE); take top 16 bits.
    return (buf[off + 7] << 8) | buf[off + 6];
  }

  type Cell = { hue: number; sat: number; light: number; match: boolean };
  const cells = $derived.by<Cell[]>(() => {
    if (!ok) return [];
    const out: Cell[] = new Array(slotCount);
    for (let i = 0; i < slotCount; i++) {
      const h = (slotHue(bytes, i) / 65536) * 360; // 0..360
      let match = false;
      if (diffAgainst && diffOk) {
        // Cheap byte equality over the 8-byte slot; OK because the
        // slots are repr(C) Pod and identical bytes ⇔ identical u64.
        match = true;
        const off = i * 8;
        for (let k = 0; k < 8; k++) {
          if (bytes[off + k] !== diffAgainst[off + k]) { match = false; break; }
        }
      }
      out[i] = {
        hue: h,
        sat: match ? 92 : 70,
        light: match ? 50 : 55,
        match,
      };
    }
    return out;
  });

  // Jaccard estimate (matches / H) when in diff mode — same statistic the
  // SDK uses to compare two MinHash signatures.
  const jaccardEst = $derived.by(() => {
    if (!diffAgainst || !diffOk || !ok) return null;
    const matches = cells.filter(c => c.match).length;
    return matches / slotCount;
  });

  // Lay out: rows × cols computed to keep cells roughly square.
  const cols = $derived(Math.min(slotCount, 32));
  const rows = $derived(Math.ceil(slotCount / cols));
</script>

{#if ok}
  <div class="mh-wrap" style="width:{width}px">
    {#if label}<div class="mh-label">{label}</div>{/if}
    <div class="mh-grid"
      style="--cols:{cols}; --rows:{rows}; height:{height}px"
      role="img" aria-label="{slotCount}-slot MinHash heatmap">
      {#each cells as c, i (i)}
        <span class="mh-cell"
          class:match={c.match}
          style="background: oklch({c.light}% 0.18 {c.hue})"
          title={c.match ? `slot ${i}: collision` : `slot ${i}: 0x${slotHue(bytes, i).toString(16).padStart(4, '0')}`}>
        </span>
      {/each}
    </div>
    <div class="mh-meta">
      {#if jaccardEst != null}
        <span><strong>Ĵ</strong> {jaccardEst.toFixed(3)}</span>
        <span><strong>matching slots</strong> {cells.filter(c => c.match).length}/{slotCount}</span>
      {:else}
        <span><strong>slots</strong> {slotCount}</span>
        <span><strong>bytes</strong> {bytes.length}</span>
      {/if}
    </div>
  </div>
{/if}

<style>
  .mh-wrap { display: flex; flex-direction: column; gap: 0.3rem; }
  .mh-label {
    font-family: var(--mono, monospace);
    font-size: 0.62rem;
    color: var(--ink-2, #888);
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }
  .mh-grid {
    display: grid;
    grid-template-columns: repeat(var(--cols), 1fr);
    grid-template-rows: repeat(var(--rows), 1fr);
    gap: 1px;
    padding: 2px;
    background: var(--ink, #111);
    border-radius: 3px;
  }
  .mh-cell {
    border-radius: 1px;
    transition: filter 0.12s, outline-color 0.12s;
    cursor: help;
  }
  .mh-cell.match {
    outline: 1px solid oklch(0.85 0.18 90);
    outline-offset: -1px;
    filter: brightness(1.15);
  }
  .mh-cell:hover {
    filter: brightness(1.25);
  }
  .mh-meta {
    display: flex; gap: 0.8rem;
    font-family: var(--mono, monospace);
    font-size: 0.6rem;
    color: var(--ink-2, #888);
  }
  .mh-meta strong { font-weight: 400; margin-right: 4px; }
</style>
