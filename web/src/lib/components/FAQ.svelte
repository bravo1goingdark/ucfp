<script lang="ts">
  const items = [
    {
      q: 'How is UCFP different from a normal cryptographic hash?',
      a: 'SHA-256 changes completely if you re-encode an image or normalize whitespace in a JSON document. UCFP is content-addressed in the perceptual sense: minor lossy transforms produce a Hamming-comparable ID, while tampering still flips far enough bits to be detectable. You opt into either mode per call.'
    },
    {
      q: 'Is the spec really stable?',
      a: 'Yes. UCFP-1 is on draft 04 and has had no breaking changes in twelve months. Every signature carries its schema version inline, so old fingerprints stay parseable forever — and we will refuse to ship a v2 that breaks v1 reads.'
    },
    {
      q: 'Can I run this without sending data to your servers?',
      a: 'The Apache-2.0 SDK is the entire runtime — server, indices, and all. The hosted Cloud tier is a convenience, not a dependency. Nothing in the protocol requires a network round-trip.'
    },
    {
      q: 'What does it cost in practice?',
      a: 'Open tier: the SDK is free forever. Cloud: $0.000004 per fingerprint after the 10M monthly free quota. A team indexing a million records per day lands around $80 / month. Enterprise is a flat annual contract.'
    },
    {
      q: 'How does it handle GDPR / right-to-be-forgotten?',
      a: 'A UCFP is a one-way fingerprint, not a reversible identifier. You delete the underlying record from your storage, the fingerprint stops resolving. We never see your raw bytes on the open or self-host tier.'
    },
    {
      q: 'Why should I trust the throughput numbers?',
      a: 'Every benchmark in the docs is reproducible from a single make bench command in the public repo. CI publishes a fresh number per commit. If your hardware comes back slower, file an issue — we treat regressions as bugs.'
    }
  ];
</script>

<section class="faq" id="faq">
  <div class="section-label">10 · Common questions</div>
  <h2 class="h">No jargon. <span class="it">Just answers.</span></h2>

  <div class="faq-list">
    {#each items as item}
      <details class="faq-item">
        <summary>
          <span>{item.q}</span>
          <span class="marker" aria-hidden="true">+</span>
        </summary>
        <div class="answer">{item.a}</div>
      </details>
    {/each}
  </div>
</section>
