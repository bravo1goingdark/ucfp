<!--
  Token → weight bars for SimHash TF / TF-IDF inspect output.

  Each token's contribution is rendered as a horizontal bar whose width
  is proportional to the weight. Negative weights (tf-idf can produce
  these for some implementations) extend leftward.
-->
<script lang="ts">
  interface TokenWeight {
    token: string;
    weight: number;
  }

  interface Props {
    weights: TokenWeight[];
    /** Cap rows shown. */
    max?: number;
  }

  let { weights, max = 32 }: Props = $props();

  let sorted = $derived(
    [...weights].sort((a, b) => Math.abs(b.weight) - Math.abs(a.weight)).slice(0, max),
  );
  let peak = $derived.by(() => {
    let m = 0;
    for (const w of sorted) {
      const a = Math.abs(w.weight);
      if (a > m) m = a;
    }
    return m || 1;
  });
</script>

{#if sorted.length > 0}
  <div class="tfidf">
    {#each sorted as w (w.token)}
      {@const pct = (Math.abs(w.weight) / peak) * 100}
      {@const positive = w.weight >= 0}
      <div class="row">
        <span class="token mono" title={w.token}>{w.token}</span>
        <div class="track">
          <div
            class="fill"
            class:neg={!positive}
            style:width="{pct}%"
            aria-label="{w.token} weight {w.weight.toFixed(3)}"></div>
        </div>
        <span class="val mono">{w.weight.toFixed(3)}</span>
      </div>
    {/each}
    {#if weights.length > max}
      <p class="more">…and {weights.length - max} more</p>
    {/if}
  </div>
{/if}

<style>
  .tfidf {
    display: flex;
    flex-direction: column;
    gap: 4px;
    width: 100%;
  }
  .row {
    display: grid;
    grid-template-columns: minmax(80px, 14ch) 1fr 8ch;
    gap: 8px;
    align-items: center;
    font-size: 11px;
  }
  .token {
    color: var(--ink-2);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .track {
    height: 6px;
    background: var(--bg-2);
    border: 1px solid var(--line);
    overflow: hidden;
  }
  .fill {
    height: 100%;
    background: var(--accent-ink);
    transition: width 0.18s ease;
  }
  .fill.neg {
    background: oklch(0.55 0.18 30);
  }
  .val {
    color: var(--muted);
    text-align: right;
    font-variant-numeric: tabular-nums;
  }
  .mono {
    font-family: var(--mono);
  }
  .more {
    font-family: var(--mono);
    font-size: 10.5px;
    color: var(--muted);
    margin: 4px 0 0;
  }
</style>
