<script lang="ts">
  const cases = [
    {
      label: 'TRAINING DATA',
      title: 'Dedup at <span class="it">corpus scale.</span>',
      body:
        'Strip exact and near-duplicate documents from a 12B-token pretraining set in a single MapReduce pass. UCFPs are stable across shards, so the join is just a hash compare — no re-tokenization, no per-modality glue.',
      meta: ['~840 MB/s · single core', 'O(1) compare', 'streaming']
    },
    {
      label: 'CONTENT PROVENANCE',
      title: 'Watermark-free <span class="it">attribution.</span>',
      body:
        'Sign every artifact your pipeline produces — checkpoints, generated images, synthetic audio — with its UCFP. Trace any downstream output back to the exact bytes it came from, without modifying the content.',
      meta: ['256-bit · multibase', 'self-describing', 'cross-modal']
    },
    {
      label: 'VECTOR INDEX',
      title: 'A <span class="it">flat namespace</span> for embeddings.',
      body:
        'Use UCFPs as the primary key in your vector store. Same fingerprint, same row — across f32, f16, and i8 quantizations of the same model. Eliminates an entire class of dedup bugs in retrieval.',
      meta: ['embedding-aware', 'quantization-stable', 'KV-friendly']
    },
    {
      label: 'CDN + EDGE',
      title: 'Cache by <span class="it">content,</span> not by URL.',
      body:
        'Compute a UCFP at the edge for any uploaded asset. Two uploads of the same image — different filenames, different MIME hints — collapse to one cache entry. Bandwidth bills go down; hit rates go up.',
      meta: ['<1 ms p99', 'WASM build', 'no central authority']
    }
  ];
</script>

<section class="cases" id="cases">
  <div class="feat-head">
    <div>
      <div class="section-label">06 · Use cases</div>
      <h2 class="h">Where teams ship<br />UCFP <span class="it">today.</span></h2>
    </div>
    <p>
      These aren't abstract — every shape below is a workload UCFP was designed for. Pick the one
      that matches your bottleneck.
    </p>
  </div>

  <div class="case-grid">
    {#each cases as c}
      <article class="case">
        <span class="label">{c.label}</span>
        <h3>{@html c.title}</h3>
        <p>{c.body}</p>
        <div class="meta">
          {#each c.meta as m}<span>· {m}</span>{/each}
        </div>
      </article>
    {/each}
  </div>
</section>
