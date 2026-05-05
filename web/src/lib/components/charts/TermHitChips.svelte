<!--
  Stacked chips visualizing BM25 term hits per result.

  Each chip is sized by the term's contribution to the BM25 score for
  this hit (idf · normalised tf component). Capped at the top-N most
  contributory terms so the chip row stays scannable.
-->
<script lang="ts">
  interface TermHit {
    /** The matched term as it appears in the query / index. */
    term: string;
    /** Inverse document frequency at index time. */
    idf?: number;
    /** Term frequency in this document. */
    tf?: number;
    /** Final BM25 contribution for this term in this document. */
    contribution?: number;
  }

  interface Props {
    hits: TermHit[];
    /** Cap how many terms to show. */
    max?: number;
  }

  let { hits, max = 8 }: Props = $props();

  let sorted = $derived(
    [...hits].sort((a, b) => (b.contribution ?? b.idf ?? 0) - (a.contribution ?? a.idf ?? 0)).slice(0, max),
  );
  let maxContribution = $derived.by(() => {
    let m = 0;
    for (const h of sorted) {
      const v = h.contribution ?? h.idf ?? 0;
      if (v > m) m = v;
    }
    return m || 1;
  });
</script>

{#if sorted.length > 0}
  <div class="thc" role="list">
    {#each sorted as h (h.term)}
      {@const v = h.contribution ?? h.idf ?? 0}
      {@const intensity = 0.45 + 0.55 * (v / maxContribution)}
      <span
        class="chip"
        role="listitem"
        style:--intensity={intensity}
        title="{h.term} · idf {h.idf?.toFixed(2) ?? '—'} · tf {h.tf ?? '—'} · contrib {v.toFixed(3)}"
      >{h.term}</span>
    {/each}
    {#if hits.length > max}
      <span class="chip more">+{hits.length - max}</span>
    {/if}
  </div>
{/if}

<style>
  .thc {
    display: flex;
    gap: 4px;
    flex-wrap: wrap;
    align-items: center;
  }
  .chip {
    font-family: var(--mono);
    font-size: 10.5px;
    letter-spacing: 0.04em;
    padding: 3px 8px;
    border: 1px solid var(--line-strong);
    background: oklch(0.85 0.05 130 / var(--intensity, 0.6));
    color: var(--ink);
    border-radius: 1px;
    white-space: nowrap;
  }
  .chip.more {
    background: var(--bg-2);
    color: var(--muted);
  }
</style>
