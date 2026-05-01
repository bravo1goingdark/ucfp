<!--
  SimHashBitWheel — radial 64-bit polarity wheel for SimHash bytes.

  Visual structure differs sharply from BitGrid8x8: each of the 64 bits
  is a wedge laid around a circle, set bits in accent colour, unset in
  ink-dim. When `diffAgainst` is supplied, flipped bits glow red on the
  outer ring. Pure SVG — zero deps, sharp at any zoom.
-->
<script lang="ts">
  type Props = {
    /** 8 bytes of LE-encoded SimHash. */
    hashBytes: Uint8Array;
    /** Optional second hash for compare-mode diff highlight. */
    diffAgainst?: Uint8Array;
    /** Edge length in CSS px. */
    size?: number;
    /** Caption shown beneath the wheel. */
    label?: string;
  };
  let { hashBytes, diffAgainst, size = 200, label }: Props = $props();

  const ok = $derived(hashBytes.length === 8);
  const diffOk = $derived(!diffAgainst || diffAgainst.length === 8);

  function unpack(bytes: Uint8Array): boolean[] {
    const out = new Array<boolean>(64);
    for (let i = 0; i < 8; i++) {
      const b = bytes[i];
      for (let j = 0; j < 8; j++) out[i * 8 + j] = ((b >> (7 - j)) & 1) === 1;
    }
    return out;
  }

  const bits = $derived(ok ? unpack(hashBytes) : ([] as boolean[]));
  const diffBits = $derived.by(() => {
    if (!diffAgainst || !diffOk || !ok) return null;
    const xor = new Uint8Array(8);
    for (let i = 0; i < 8; i++) xor[i] = hashBytes[i] ^ diffAgainst[i];
    return unpack(xor);
  });
  const popcount = $derived(bits.filter(Boolean).length);
  const flipCount = $derived(diffBits ? diffBits.filter(Boolean).length : 0);

  const N = 64;
  const center = $derived(size / 2);
  const rOuter = $derived(size / 2 - 4);
  const rInner = $derived(size * 0.30);
  const rRing = $derived(size / 2 - 1);

  type Wedge = { d: string; on: boolean; flip: boolean; idx: number };
  const wedges = $derived.by<Wedge[]>(() => {
    if (!ok) return [];
    const w: Wedge[] = [];
    const c = center;
    const r1 = rOuter;
    const r0 = rInner;
    const sweep = (Math.PI * 2) / N;
    // Start angle at -π/2 so bit 0 sits at 12 o'clock and the wheel
    // walks clockwise — matches reading hex left-to-right.
    const start0 = -Math.PI / 2;
    for (let i = 0; i < N; i++) {
      const a0 = start0 + i * sweep;
      const a1 = a0 + sweep;
      // Outer arc points
      const x0o = c + r1 * Math.cos(a0);
      const y0o = c + r1 * Math.sin(a0);
      const x1o = c + r1 * Math.cos(a1);
      const y1o = c + r1 * Math.sin(a1);
      // Inner arc points
      const x1i = c + r0 * Math.cos(a1);
      const y1i = c + r0 * Math.sin(a1);
      const x0i = c + r0 * Math.cos(a0);
      const y0i = c + r0 * Math.sin(a0);
      const d = [
        `M${x0o.toFixed(2)} ${y0o.toFixed(2)}`,
        `A${r1.toFixed(2)} ${r1.toFixed(2)} 0 0 1 ${x1o.toFixed(2)} ${y1o.toFixed(2)}`,
        `L${x1i.toFixed(2)} ${y1i.toFixed(2)}`,
        `A${r0.toFixed(2)} ${r0.toFixed(2)} 0 0 0 ${x0i.toFixed(2)} ${y0i.toFixed(2)}`,
        'Z',
      ].join(' ');
      w.push({ d, on: bits[i], flip: !!(diffBits && diffBits[i]), idx: i });
    }
    return w;
  });

  // Tick marks at every 8th bit so the byte boundaries are visible.
  type Tick = { x1: number; y1: number; x2: number; y2: number };
  const ticks = $derived.by<Tick[]>(() => {
    const t: Tick[] = [];
    const c = center;
    const sweep = (Math.PI * 2) / 8;
    const r0 = rOuter + 1;
    const r1 = rRing + 2;
    for (let i = 0; i < 8; i++) {
      const a = -Math.PI / 2 + i * sweep;
      t.push({
        x1: c + r0 * Math.cos(a),
        y1: c + r0 * Math.sin(a),
        x2: c + r1 * Math.cos(a),
        y2: c + r1 * Math.sin(a),
      });
    }
    return t;
  });
</script>

{#if ok}
  <div class="wheel-wrap" style="width:{size}px">
    <svg viewBox="0 0 {size} {size}" width={size} height={size} role="img"
      aria-label={label ?? `SimHash 64-bit polarity wheel, ${popcount}/64 bits set`}>
      {#each wedges as w (w.idx)}
        <path d={w.d} class="wedge" class:on={w.on} class:flip={w.flip} />
      {/each}
      <!-- byte-boundary ticks -->
      {#each ticks as t, i (i)}
        <line x1={t.x1} y1={t.y1} x2={t.x2} y2={t.y2} class="tick" />
      {/each}
      <!-- centre badge: popcount or flip count -->
      <g>
        <circle cx={center} cy={center} r={rInner - 2} class="hub" />
        <text x={center} y={center} class="hub-num" text-anchor="middle" dominant-baseline="central">
          {diffBits ? flipCount : popcount}
        </text>
        <text x={center} y={center + rInner * 0.55} class="hub-sub" text-anchor="middle">
          {diffBits ? 'flips/64' : 'bits/64'}
        </text>
      </g>
    </svg>
    {#if label}<div class="wheel-label">{label}</div>{/if}
  </div>
{/if}

<style>
  .wheel-wrap { display: flex; flex-direction: column; gap: 0.25rem; align-items: center; }
  .wedge {
    fill: var(--bg-2, rgba(255,255,255,0.04));
    stroke: var(--ink, rgba(0,0,0,0.7));
    stroke-width: 0.5;
    transition: fill 0.12s;
  }
  .wedge.on  { fill: var(--accent-ink, oklch(0.55 0.18 240)); }
  .wedge.flip {
    fill: oklch(0.65 0.22 30);
    stroke: oklch(0.4 0.18 30);
    stroke-width: 0.8;
  }
  .tick {
    stroke: var(--ink-2, rgba(0,0,0,0.5));
    stroke-width: 1;
  }
  .hub {
    fill: var(--bg, #fff);
    stroke: var(--ink-2, rgba(0,0,0,0.3));
    stroke-width: 0.5;
  }
  .hub-num {
    font-family: var(--mono, monospace);
    font-size: 18px;
    font-weight: 600;
    fill: var(--ink, #111);
  }
  .hub-sub {
    font-family: var(--mono, monospace);
    font-size: 7.5px;
    fill: var(--ink-2, #888);
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }
  .wheel-label {
    font-family: var(--mono, monospace);
    font-size: 0.62rem;
    color: var(--ink-2, #888);
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }
</style>
