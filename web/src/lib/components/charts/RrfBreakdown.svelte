<!--
  RRF (Reciprocal Rank Fusion) per-hit breakdown bar.

  Hybrid search runs vector kNN and BM25 in parallel and fuses with
  RRF (rrf_k=60). The Rust matcher computes per-source ranks and scores
  but a non-explainable client only sees the fused score. This component
  surfaces the contribution split as a stacked horizontal bar so users
  can see _why_ a record ranked where it did.
-->
<script lang="ts">
  interface Props {
    /** Vector-source contribution to the fused RRF score (1/(rrf_k + rank_v)). */
    vectorScore?: number | null;
    /** BM25-source contribution to the fused RRF score (1/(rrf_k + rank_b)). */
    bm25Score?: number | null;
    /** Ranks from each source (1-indexed); shown as a label. */
    vectorRank?: number | null;
    bm25Rank?: number | null;
    /** Total fused score (vector + bm25 contributions); used for max scaling. */
    fusedScore?: number;
    /** Absolute max of fused scores in this result page — for shared horizontal scale. */
    pageMax?: number;
  }

  let {
    vectorScore = null,
    bm25Score = null,
    vectorRank = null,
    bm25Rank = null,
    fusedScore = (vectorScore ?? 0) + (bm25Score ?? 0),
    pageMax,
  }: Props = $props();

  let total = $derived(fusedScore || ((vectorScore ?? 0) + (bm25Score ?? 0)));
  let scale = $derived(pageMax && pageMax > 0 ? pageMax : Math.max(total, 1e-6));
  let vPct = $derived(((vectorScore ?? 0) / scale) * 100);
  let bPct = $derived(((bm25Score ?? 0) / scale) * 100);

  function fmtScore(v: number | null): string {
    if (v == null) return '—';
    if (v < 0.001) return v.toExponential(1);
    return v.toFixed(4);
  }
</script>

<div class="rrf">
  <div class="rrf-bar" role="img"
    aria-label="RRF contribution: vector {fmtScore(vectorScore)}, bm25 {fmtScore(bm25Score)}">
    {#if vPct > 0}
      <div class="rrf-seg rrf-vec" style:width="{vPct}%" title="vector · {fmtScore(vectorScore)}{vectorRank ? ` (rank ${vectorRank})` : ''}"></div>
    {/if}
    {#if bPct > 0}
      <div class="rrf-seg rrf-bm" style:width="{bPct}%" title="bm25 · {fmtScore(bm25Score)}{bm25Rank ? ` (rank ${bm25Rank})` : ''}"></div>
    {/if}
  </div>
  <div class="rrf-meta">
    <span class="dot vec"></span>
    <span class="lbl">vec</span>
    <span class="val">{fmtScore(vectorScore)}{vectorRank ? ` · #${vectorRank}` : ''}</span>
    <span class="sep">·</span>
    <span class="dot bm"></span>
    <span class="lbl">bm25</span>
    <span class="val">{fmtScore(bm25Score)}{bm25Rank ? ` · #${bm25Rank}` : ''}</span>
  </div>
</div>

<style>
  .rrf {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }
  .rrf-bar {
    display: flex;
    height: 8px;
    background: var(--bg-2);
    border: 1px solid var(--line-strong);
    overflow: hidden;
  }
  .rrf-seg {
    height: 100%;
    transition: width 0.18s ease;
  }
  .rrf-vec {
    background: var(--accent-ink);
  }
  .rrf-bm {
    background: oklch(0.62 0.16 50);
  }
  .rrf-meta {
    display: flex;
    align-items: center;
    gap: 6px;
    font-family: var(--mono);
    font-size: 10.5px;
    color: var(--ink-2);
    flex-wrap: wrap;
  }
  .rrf-meta .lbl {
    color: var(--muted);
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }
  .rrf-meta .val {
    font-variant-numeric: tabular-nums;
  }
  .rrf-meta .sep {
    color: var(--line-strong);
  }
  .dot {
    display: inline-block;
    width: 8px;
    height: 8px;
    border-radius: 1px;
  }
  .dot.vec {
    background: var(--accent-ink);
  }
  .dot.bm {
    background: oklch(0.62 0.16 50);
  }
</style>
