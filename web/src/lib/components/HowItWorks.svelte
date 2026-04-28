<section class="how" id="how">
  <div class="section-label">05 · How it works</div>
  <h2 class="h">Three calls. <span class="it">Zero ceremony.</span></h2>

  <div class="steps">
    <div class="step">
      <span class="badge">/01 · INGEST</span>
      <span class="num">→</span>
      <h3>Hand it bytes.</h3>
      <p>
        Pipe a stream, pass a string, point at a file. The SDK detects the modality and routes to
        the right canonicalizer — NFKC for text, DCT for images, Wang landmarks for audio.
      </p>
    </div>
    <div class="step">
      <span class="badge">/02 · COLLAPSE</span>
      <span class="num">≡</span>
      <h3>Collapse to a vector.</h3>
      <p>
        Each modality has a deterministic algorithm: MinHash for text, perceptual hashes for
        images, Shazam-style landmarks for audio. The result is a single, comparable signature.
      </p>
    </div>
    <div class="step">
      <span class="badge">/03 · INDEX</span>
      <span class="num">∎</span>
      <h3>Index, query, prove.</h3>
      <p>
        Store as a content-addressed key. Query by similarity, dedup by exact match, prove
        provenance by signature. Every operation is O(log n) at scale.
      </p>
    </div>
  </div>
</section>
