<script lang="ts">
  // Dispatch on the upstream algorithm tag (see ALGORITHM_* in
  // src/modality/*.rs) and render an algorithm-aware visualisation.
  // Falls back to nothing when the algorithm has no specialised view —
  // the generic byte grid in the playground already covers that case.
  //
  // Keep this layer purely structural: each child component owns its
  // own parsing + rendering. This dispatcher just routes.

  import BitGrid8x8 from './BitGrid8x8.svelte';
  import ImageHashView from './ImageHashView.svelte';
  import LandmarkScatter from './LandmarkScatter.svelte';

  type Props = {
    /** Upstream algorithm tag, e.g. "imgfprint-multihash-v1". */
    algorithm: string;
    /** Full fingerprint bytes (decoded from `fingerprint_hex`). */
    bytes: Uint8Array;
    /** Optional second buffer for compare-mode diff highlight. */
    diffAgainst?: Uint8Array;
  };

  let { algorithm, bytes, diffAgainst }: Props = $props();

  // Slice helpers shared across image variants.
  // MultiHashFingerprint is `#[repr(C)]` with declaration-order fields
  // `ahash, phash, dhash` (verified in imgfprint-0.4.1/src/core/fingerprint.rs:235),
  // so byte offsets 0/168/336 map to AHash/PHash/DHash respectively.
  function sliceImageFp(buf: Uint8Array, idx: number): Uint8Array {
    const off = idx * 168;
    return buf.subarray(off, off + 168);
  }

</script>

{#if algorithm === 'imgfprint-multihash-v1' && bytes.length === 504}
  <!-- Layout (from Record::fingerprint via bytemuck::bytes_of(&MultiHashFingerprint)):
       [ahash:168][phash:168][dhash:168] -->
  <div class="av-multi">
    <ImageHashView label="AHash" bytes={sliceImageFp(bytes, 0)}
      diffAgainst={diffAgainst && diffAgainst.length === 504 ? sliceImageFp(diffAgainst, 0) : undefined} />
    <ImageHashView label="PHash" bytes={sliceImageFp(bytes, 1)}
      diffAgainst={diffAgainst && diffAgainst.length === 504 ? sliceImageFp(diffAgainst, 1) : undefined} />
    <ImageHashView label="DHash" bytes={sliceImageFp(bytes, 2)}
      diffAgainst={diffAgainst && diffAgainst.length === 504 ? sliceImageFp(diffAgainst, 2) : undefined} />
  </div>

{:else if (algorithm === 'imgfprint-phash-v1' || algorithm === 'imgfprint-dhash-v1' || algorithm === 'imgfprint-ahash-v1') && bytes.length === 168}
  {@const niceLabel = algorithm.replace('imgfprint-', '').replace('-v1', '').toUpperCase()}
  <ImageHashView label={niceLabel} bytes={bytes}
    diffAgainst={diffAgainst && diffAgainst.length === 168 ? diffAgainst : undefined} />

{:else if (algorithm === 'simhash-b64-tf' || algorithm === 'simhash-b64-idf') && bytes.length === 8}
  <div class="av-simhash">
    <BitGrid8x8 hashBytes={bytes}
      diffAgainst={diffAgainst && diffAgainst.length === 8 ? diffAgainst : undefined}
      label="SimHash · 64 bits" size={120} />
  </div>

{:else if algorithm === 'audiofp-wang-v1' && bytes.length > 0 && bytes.length % 8 === 0}
  <LandmarkScatter bytes={bytes} algo="wang" framesPerSec={62.5} height={140} />

{:else if algorithm === 'audiofp-panako-v1' && bytes.length > 0 && bytes.length % 16 === 0}
  <LandmarkScatter bytes={bytes} algo="panako" framesPerSec={62.5} height={140} />
{/if}

<style>
  .av-multi { display: grid; grid-template-columns: repeat(auto-fit, minmax(220px, 1fr)); gap: 0.5rem; }
  .av-simhash { display: flex; justify-content: flex-start; }
</style>
