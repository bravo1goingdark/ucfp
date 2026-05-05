<!--
  Fullbleed image fingerprint viewer.

  Supports both single-hash (PHash / DHash / AHash, 168 bytes) and
  multihash (PHash + DHash + AHash bundle, 536 bytes). Each hash gets
  an ImageHashView panel rendered side-by-side at viewport scale.
-->
<script lang="ts">
  import { page } from '$app/stores';
  import ImageHashView from '$components/charts/ImageHashView.svelte';
  import { loadFingerprint, type LoadedFingerprint } from '$lib/utils/viewerLoader.svelte';

  // MultiHashFingerprint = 32 (bundle exact) + 168×3 = 536. Asserted
  // upstream in imgfprint-0.4.1/src/core/fingerprint.rs.
  const MULTI_BUNDLE_SIZE = 536;
  const MULTI_OFFSET_AHASH = 32;
  const MULTI_OFFSET_PHASH = 32 + 168;
  const MULTI_OFFSET_DHASH = 32 + 168 * 2;

  let data = $state<LoadedFingerprint | null>(null);
  let loading = $state(true);
  let err = $state<string | null>(null);

  $effect(() => {
    const params = new URLSearchParams($page.url.searchParams);
    if (!params.has('modality')) params.set('modality', 'image');
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

  function slice(b: Uint8Array, off: number): Uint8Array {
    return b.subarray(off, off + 168);
  }
</script>

<div class="ihv-viewer">
  {#if loading}
    <div class="ihv-state">loading…</div>
  {:else if err}
    <div class="ihv-state error"><strong>error:</strong> <pre>{err}</pre></div>
  {:else if data}
    <div class="ihv-meta-bar">
      <span><strong>algorithm</strong> {data.algorithm}</span>
      <span><strong>bytes</strong> {data.fingerprintBytes.length}</span>
    </div>
    <div class="ihv-stage">
      {#if data.fingerprintBytes.length === MULTI_BUNDLE_SIZE}
        <ImageHashView label="AHash" bytes={slice(data.fingerprintBytes, MULTI_OFFSET_AHASH)} />
        <ImageHashView label="PHash" bytes={slice(data.fingerprintBytes, MULTI_OFFSET_PHASH)} />
        <ImageHashView label="DHash" bytes={slice(data.fingerprintBytes, MULTI_OFFSET_DHASH)} />
      {:else if data.fingerprintBytes.length === 168}
        <ImageHashView
          label={data.algorithm.replace('imgfprint-', '').replace('-v1', '').toUpperCase()}
          bytes={data.fingerprintBytes}
        />
      {:else}
        <p class="ihv-state">unsupported byte length: {data.fingerprintBytes.length}</p>
      {/if}
    </div>
  {/if}
</div>

<style>
  .ihv-viewer {
    display: flex;
    flex-direction: column;
    height: 100%;
    width: 100%;
    min-height: 0;
  }
  .ihv-meta-bar {
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
  .ihv-meta-bar strong {
    font-weight: 400;
    color: var(--muted);
    text-transform: uppercase;
    letter-spacing: 0.08em;
    margin-right: 6px;
  }
  .ihv-stage {
    flex: 1;
    min-height: 0;
    padding: 32px;
    display: grid;
    gap: 32px;
    grid-template-columns: repeat(auto-fit, minmax(360px, 1fr));
    align-content: center;
    justify-content: center;
    overflow: auto;
  }
  .ihv-state {
    margin: auto;
    font-family: var(--mono);
    font-size: 13px;
    color: var(--muted);
  }
  .ihv-state.error {
    color: oklch(0.5 0.16 25);
  }
  .ihv-state pre {
    margin: 6px 0 0;
    white-space: pre-wrap;
  }
</style>
