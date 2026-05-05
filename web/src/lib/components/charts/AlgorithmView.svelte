<script lang="ts">
  // Dispatch on the upstream algorithm tag (see ALGORITHM_* in
  // src/modality/*.rs) and render an algorithm-aware visualisation.
  // Each branch is wrapped in `ChartFrame` so every algorithm view inherits
  // the production toolbar (download PNG/SVG, fullscreen) and a responsive
  // container — the visualization fills its parent and can occupy the screen
  // on demand.
  //
  // Falls back to nothing when the algorithm has no specialised view —
  // the generic byte grid in the playground already covers that case.

  import BitGrid8x8 from './BitGrid8x8.svelte';
  import ImageHashView from './ImageHashView.svelte';
  import LandmarkScatter from './LandmarkScatter.svelte';
  import SimHashBitWheel from './SimHashBitWheel.svelte';
  import MinHashSlotHeatmap from './MinHashSlotHeatmap.svelte';
  import { ChartFrame } from './_primitives';

  type Props = {
    /** Upstream algorithm tag, e.g. "imgfprint-multihash-v1". */
    algorithm: string;
    /** Full fingerprint bytes (decoded from `fingerprint_hex`). */
    bytes: Uint8Array;
    /** Optional second buffer for compare-mode diff highlight. */
    diffAgainst?: Uint8Array;
  };

  let { algorithm, bytes, diffAgainst }: Props = $props();

  // MultiHashFingerprint = 32 (bundle exact) + 168×3 (ahash, phash, dhash) = 536.
  // Skip the leading 32 bytes (BLAKE3 of the source image — surfaced inside
  // each ImageFingerprint already, no need to render twice) then read the
  // three 168-byte slots in declaration order.
  const MULTI_BUNDLE_SIZE = 536;
  const MULTI_OFFSET_AHASH = 32;
  const MULTI_OFFSET_PHASH = 32 + 168;
  const MULTI_OFFSET_DHASH = 32 + 168 * 2;
  function sliceImageFp(buf: Uint8Array, off: number): Uint8Array {
    return buf.subarray(off, off + 168);
  }

  function shortName(algo: string): string {
    return algo.replace(/-v\d+$/, '').replace(/^(imgfprint|audiofp)-/, '');
  }
</script>

{#if algorithm === 'imgfprint-multihash-v1' && bytes.length === MULTI_BUNDLE_SIZE}
  <ChartFrame
    label="image multihash · ahash + phash + dhash"
    aspect={2.4}
    minHeight={260}
    downloadName="multihash"
  >
    {#snippet children()}
      <div class="av-multi">
        <ImageHashView label="AHash" bytes={sliceImageFp(bytes, MULTI_OFFSET_AHASH)}
          diffAgainst={diffAgainst && diffAgainst.length === MULTI_BUNDLE_SIZE ? sliceImageFp(diffAgainst, MULTI_OFFSET_AHASH) : undefined} />
        <ImageHashView label="PHash" bytes={sliceImageFp(bytes, MULTI_OFFSET_PHASH)}
          diffAgainst={diffAgainst && diffAgainst.length === MULTI_BUNDLE_SIZE ? sliceImageFp(diffAgainst, MULTI_OFFSET_PHASH) : undefined} />
        <ImageHashView label="DHash" bytes={sliceImageFp(bytes, MULTI_OFFSET_DHASH)}
          diffAgainst={diffAgainst && diffAgainst.length === MULTI_BUNDLE_SIZE ? sliceImageFp(diffAgainst, MULTI_OFFSET_DHASH) : undefined} />
      </div>
    {/snippet}
  </ChartFrame>

{:else if (algorithm === 'imgfprint-phash-v1' || algorithm === 'imgfprint-dhash-v1' || algorithm === 'imgfprint-ahash-v1') && bytes.length === 168}
  {@const niceLabel = algorithm.replace('imgfprint-', '').replace('-v1', '').toUpperCase()}
  <ChartFrame
    label="image hash · {niceLabel.toLowerCase()}"
    aspect={2.4}
    minHeight={220}
    downloadName="image-{niceLabel.toLowerCase()}"
  >
    {#snippet children()}
      <div class="av-single">
        <ImageHashView label={niceLabel} bytes={bytes}
          diffAgainst={diffAgainst && diffAgainst.length === 168 ? diffAgainst : undefined} />
      </div>
    {/snippet}
  </ChartFrame>

{:else if (algorithm === 'simhash-b64-tf' || algorithm === 'simhash-b64-idf') && bytes.length === 8}
  <ChartFrame
    label="simhash · 64-bit polarity wheel"
    aspect={1.6}
    minHeight={300}
    downloadName="simhash-wheel"
  >
    {#snippet children({ width, height, isFullscreen })}
      {@const wheelSize = Math.min(height, width * 0.6, isFullscreen ? 720 : 360)}
      {@const gridSize = Math.min(wheelSize * 0.55, 220)}
      <div class="av-simhash" class:fs={isFullscreen}>
        <SimHashBitWheel hashBytes={bytes}
          diffAgainst={diffAgainst && diffAgainst.length === 8 ? diffAgainst : undefined}
          label="SimHash · 64 bits"
          size={Math.max(160, wheelSize)} />
        <BitGrid8x8 hashBytes={bytes}
          diffAgainst={diffAgainst && diffAgainst.length === 8 ? diffAgainst : undefined}
          label="bit grid"
          size={Math.max(96, gridSize)} />
      </div>
    {/snippet}
  </ChartFrame>

{:else if algorithm === 'minhash-h128' && bytes.length === 1032}
  <ChartFrame
    label="minhash · 128 slots × u64"
    aspect={4.5}
    minHeight={140}
    downloadName="minhash-heatmap"
  >
    {#snippet children({ width, height, isFullscreen })}
      <MinHashSlotHeatmap bytes={bytes.subarray(8)}
        diffAgainst={diffAgainst && diffAgainst.length === 1032 ? diffAgainst.subarray(8) : undefined}
        label={isFullscreen ? 'MinHash · 128 slots × u64 (each cell = top 16 bits)' : 'MinHash · 128 slots × u64'}
        width={width}
        height={Math.max(56, Math.min(height, isFullscreen ? 360 : 96))} />
    {/snippet}
  </ChartFrame>

{:else if algorithm === 'audiofp-wang-v1' && bytes.length > 0 && bytes.length % 8 === 0}
  <ChartFrame
    label="audio · wang landmark constellation"
    aspect={2.6}
    minHeight={220}
    downloadName="wang-landmarks"
  >
    {#snippet children({ height, isFullscreen })}
      <LandmarkScatter bytes={bytes} algo="wang" framesPerSec={62.5}
        height={Math.max(160, isFullscreen ? Math.min(height, 800) : Math.min(height, 320))} />
    {/snippet}
  </ChartFrame>

{:else if algorithm === 'audiofp-panako-v1' && bytes.length > 0 && bytes.length % 16 === 0}
  <ChartFrame
    label="audio · panako landmark constellation"
    aspect={2.6}
    minHeight={220}
    downloadName="panako-landmarks"
  >
    {#snippet children({ height, isFullscreen })}
      <LandmarkScatter bytes={bytes} algo="panako" framesPerSec={62.5}
        height={Math.max(160, isFullscreen ? Math.min(height, 800) : Math.min(height, 320))} />
    {/snippet}
  </ChartFrame>
{/if}

<style>
  .av-multi {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
    gap: 0.75rem;
    width: 100%;
    align-content: start;
  }
  .av-single {
    display: grid;
    place-items: center;
    width: 100%;
  }
  .av-simhash {
    display: flex;
    justify-content: center;
    align-items: center;
    gap: 1.25rem;
    flex-wrap: wrap;
    width: 100%;
    height: 100%;
  }
  .av-simhash.fs {
    align-content: center;
    gap: 2.5rem;
  }
</style>
