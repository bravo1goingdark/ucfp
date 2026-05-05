<!--
  Fullbleed dense-embedding viewer. Renders all dimensions of a semantic
  algorithm's embedding (no 128-bar cap) at viewport scale.
-->
<script lang="ts">
  import { page } from '$app/stores';
  import EmbeddingBars from '$components/charts/EmbeddingBars.svelte';
  import { loadFingerprint, type LoadedFingerprint } from '$lib/utils/viewerLoader.svelte';

  let data = $state<LoadedFingerprint | null>(null);
  let loading = $state(true);
  let err = $state<string | null>(null);

  $effect(() => {
    const params = new URLSearchParams($page.url.searchParams);
    if (!params.has('modality')) params.set('modality', 'text');
    void load(params);
  });

  async function load(p: URLSearchParams) {
    loading = true;
    err = null;
    try {
      data = await loadFingerprint(p);
    } catch (e) {
      err = (e as Error).message;
    } finally {
      loading = false;
    }
  }

  let stats = $derived.by(() => {
    if (!data?.embedding) return null;
    let sumSq = 0;
    let min = Infinity;
    let max = -Infinity;
    for (const v of data.embedding) {
      sumSq += v * v;
      if (v < min) min = v;
      if (v > max) max = v;
    }
    return {
      dim: data.embedding.length,
      l2: Math.sqrt(sumSq),
      min,
      max,
      mean: data.embedding.reduce((a, b) => a + b, 0) / data.embedding.length,
    };
  });
</script>

<div class="ev-viewer">
  {#if loading}
    <div class="ev-state">loading…</div>
  {:else if err}
    <div class="ev-state error"><strong>error:</strong> <pre>{err}</pre></div>
  {:else if data?.embedding && data.embedding.length > 0 && stats}
    <div class="ev-meta-bar">
      <span><strong>algorithm</strong> {data.algorithm}</span>
      <span><strong>dim</strong> {stats.dim}</span>
      <span><strong>L2</strong> {stats.l2.toFixed(3)}</span>
      <span><strong>min</strong> {stats.min.toFixed(3)}</span>
      <span><strong>max</strong> {stats.max.toFixed(3)}</span>
      <span><strong>mean</strong> {stats.mean.toFixed(3)}</span>
    </div>
    <div class="ev-stage">
      <EmbeddingBars
        vector={data.embedding}
        maxBars={data.embedding.length}
        height={Math.floor((typeof window !== 'undefined' ? window.innerHeight : 600) * 0.7)}
      />
    </div>
  {:else}
    <div class="ev-state">algorithm produced no embedding</div>
  {/if}
</div>

<style>
  .ev-viewer {
    display: flex;
    flex-direction: column;
    height: 100%;
    width: 100%;
    min-height: 0;
  }
  .ev-meta-bar {
    display: flex;
    flex-wrap: wrap;
    gap: 18px;
    padding: 10px 24px;
    border-bottom: 1px solid var(--line);
    font-family: var(--mono);
    font-size: 11px;
    color: var(--ink-2);
    background: rgba(255, 255, 255, 0.18);
  }
  .ev-meta-bar strong {
    font-weight: 400;
    color: var(--muted);
    text-transform: uppercase;
    letter-spacing: 0.08em;
    margin-right: 6px;
  }
  .ev-stage {
    flex: 1;
    min-height: 0;
    padding: 28px 32px;
    display: flex;
    align-items: stretch;
    justify-content: stretch;
  }
  .ev-state {
    margin: auto;
    font-family: var(--mono);
    font-size: 13px;
    color: var(--muted);
  }
  .ev-state.error {
    color: oklch(0.5 0.16 25);
  }
  .ev-state pre {
    margin: 6px 0 0;
    white-space: pre-wrap;
  }
</style>
