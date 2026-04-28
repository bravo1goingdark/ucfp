<script lang="ts">
  const quotes = [
    {
      body:
        'We replaced four bespoke dedup pipelines — text, images, audio, parquet — with a single UCFP join. Pretraining throughput went up <span class="it">2.4×.</span>',
      name: 'Riya N.',
      role: 'STAFF MLE · HARMONIC LABS'
    },
    {
      body:
        'Content-addressed by default means every artifact in our pipeline is reproducible by construction. We stopped writing provenance code <span class="it">six months ago.</span>',
      name: 'Marko V.',
      role: 'PLATFORM LEAD · NEARFIELD'
    },
    {
      body:
        'The fact that the protocol is open and the spec is short — under fifty pages — is the only reason we shipped it through legal in <span class="it">a single quarter.</span>',
      name: 'Anya O.',
      role: 'PRINCIPAL · CTRL/Z RESEARCH'
    }
  ];
</script>

<section class="testimonials" id="testimonials">
  <div class="section-label">09 · Field reports</div>
  <h2 class="h">Engineers <span class="it">running it.</span></h2>

  <div class="quote-grid">
    {#each quotes as q}
      <figure class="quote">
        <blockquote>"{@html q.body}"</blockquote>
        <figcaption class="who">
          <span class="name">{q.name}</span>
          <span>{q.role}</span>
        </figcaption>
      </figure>
    {/each}
  </div>
</section>
