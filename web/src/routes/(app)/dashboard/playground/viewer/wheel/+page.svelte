<!--
  Fullbleed SimHash 64-bit polarity wheel. Renders the wheel + bit grid
  side-by-side at viewport size.
-->
<script lang="ts">
  import { page } from '$app/stores';
  import SimHashBitWheel from '$components/charts/SimHashBitWheel.svelte';
  import BitGrid8x8 from '$components/charts/BitGrid8x8.svelte';
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

  let wheelSize = $state(560);
  $effect(() => {
    if (typeof window !== 'undefined') {
      wheelSize = Math.min(720, Math.floor(Math.min(window.innerWidth * 0.45, window.innerHeight * 0.7)));
    }
  });
</script>

<div class="wv-viewer">
  {#if loading}
    <div class="wv-state">loading…</div>
  {:else if err}
    <div class="wv-state error"><strong>error:</strong> <pre>{err}</pre></div>
  {:else if data && data.fingerprintBytes.length === 8}
    <div class="wv-meta-bar">
      <span><strong>algorithm</strong> {data.algorithm}</span>
      <span><strong>hex</strong> <code>{data.fingerprintHex}</code></span>
    </div>
    <div class="wv-stage">
      <SimHashBitWheel
        hashBytes={data.fingerprintBytes}
        label="SimHash · 64-bit polarity wheel"
        size={wheelSize}
      />
      <BitGrid8x8
        hashBytes={data.fingerprintBytes}
        label="bit grid"
        size={Math.max(180, Math.floor(wheelSize * 0.55))}
      />
    </div>
  {:else}
    <div class="wv-state">expected 8-byte SimHash; got {data?.fingerprintBytes.length ?? 0}</div>
  {/if}
</div>

<style>
  .wv-viewer {
    display: flex;
    flex-direction: column;
    height: 100%;
    width: 100%;
    min-height: 0;
  }
  .wv-meta-bar {
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
  .wv-meta-bar strong {
    font-weight: 400;
    color: var(--muted);
    text-transform: uppercase;
    letter-spacing: 0.08em;
    margin-right: 6px;
  }
  .wv-meta-bar code {
    background: var(--bg-2);
    padding: 1px 6px;
    border: 1px solid var(--line);
  }
  .wv-stage {
    flex: 1;
    min-height: 0;
    padding: 32px;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 48px;
    flex-wrap: wrap;
  }
  .wv-state {
    margin: auto;
    font-family: var(--mono);
    font-size: 13px;
    color: var(--muted);
  }
  .wv-state.error {
    color: oklch(0.5 0.16 25);
  }
  .wv-state pre {
    margin: 6px 0 0;
    white-space: pre-wrap;
  }
</style>
