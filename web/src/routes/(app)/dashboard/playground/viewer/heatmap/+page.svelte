<!--
  Fullbleed MinHash slot heatmap. Loads the fingerprint via input_id
  and renders the H × u64 heatmap occupying the full viewport.
-->
<script lang="ts">
  import { page } from '$app/stores';
  import MinHashSlotHeatmap from '$components/charts/MinHashSlotHeatmap.svelte';
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

  // MinHash signature is `repr(C) { schema:u16, _pad:[u8;6], hashes:[u64;128] }`
  // — strip the 8-byte header before rendering.
  let slotBytes = $derived(
    data && data.fingerprintBytes.length > 8
      ? data.fingerprintBytes.subarray(8)
      : new Uint8Array(0),
  );
</script>

<div class="hm-viewer">
  {#if loading}
    <div class="hm-state">loading…</div>
  {:else if err}
    <div class="hm-state error"><strong>error:</strong> <pre>{err}</pre></div>
  {:else if data && slotBytes.length > 0}
    <div class="hm-meta-bar">
      <span><strong>algorithm</strong> {data.algorithm}</span>
      <span><strong>slots</strong> {Math.floor(slotBytes.length / 8)}</span>
      <span><strong>bytes</strong> {data.fingerprintBytes.length}</span>
    </div>
    <div class="hm-stage">
      <MinHashSlotHeatmap
        bytes={slotBytes}
        label="MinHash · 128 slots × u64 · top-16 bits → hue"
        height={Math.floor(window.innerHeight * 0.7)}
      />
    </div>
  {:else}
    <div class="hm-state">no data</div>
  {/if}
</div>

<style>
  .hm-viewer {
    display: flex;
    flex-direction: column;
    height: 100%;
    width: 100%;
    min-height: 0;
  }
  .hm-meta-bar {
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
  .hm-meta-bar strong {
    font-weight: 400;
    color: var(--muted);
    text-transform: uppercase;
    letter-spacing: 0.08em;
    margin-right: 6px;
  }
  .hm-stage {
    flex: 1;
    min-height: 0;
    padding: 24px 32px 32px;
    display: flex;
    flex-direction: column;
    align-items: stretch;
    justify-content: center;
  }
  .hm-state {
    margin: auto;
    font-family: var(--mono);
    font-size: 13px;
    color: var(--muted);
  }
  .hm-state.error {
    color: oklch(0.5 0.16 25);
  }
  .hm-state pre {
    margin: 6px 0 0;
    white-space: pre-wrap;
    color: var(--ink-2);
  }
</style>
