<script lang="ts">
  const items = [
    {
      n: '/01',
      h: 'Deterministic',
      p: 'Same bytes in, same fingerprint out — for the next twenty years, on any machine.'
    },
    {
      n: '/02',
      h: 'Modality-agnostic',
      p: 'Text, images, audio, video, embeddings, weights — all collapse to one ID space.'
    },
    {
      n: '/03',
      h: 'Perceptual mode',
      p: 'Opt into Hamming-comparable IDs that survive resizing, re-encoding, and minor edits.'
    },
    {
      n: '/04',
      h: 'Streaming',
      p: 'Constant memory. Fingerprint a 40 GB shard the same way you fingerprint a tweet.'
    },
    {
      n: '/05',
      h: 'Content-addressed',
      p: 'Use UCFPs as keys in any KV, blob store, or graph. Provenance becomes a join.'
    },
    {
      n: '/06',
      h: 'Bindings',
      p: 'TypeScript, Python, Go, Rust, Swift, JVM. WASM build for the browser.'
    },
    {
      n: '/07',
      h: 'Open spec',
      p: 'UCFP-1 is a public, versioned protocol. No vendor, no rotation, no lock-in.'
    },
    {
      n: '/08',
      h: 'Self-describing',
      p: 'Every ID carries its modality, version, and bit-width. Parse without context.'
    }
  ];
</script>

<section class="features" id="features">
  <div class="feat-head">
    <div>
      <div class="section-label">03 · Capabilities</div>
      <h2 class="h">A flat namespace<br />for <span class="it">all content.</span></h2>
    </div>
    <p>
      Every UCFP is comparable to every other UCFP — across modality, source, and time. Build dedup,
      provenance, and search on a single primitive.
    </p>
  </div>
  <div class="feat-grid">
    {#each items as item}
      <div class="feat">
        <span class="num">{item.n}</span>
        <div>
          <h3>{item.h}</h3>
          <p>{item.p}</p>
        </div>
      </div>
    {/each}
  </div>
</section>
