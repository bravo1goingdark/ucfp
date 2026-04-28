<script lang="ts">
  const integrations = [
    { name: 'PostgreSQL', kind: 'STORAGE' },
    { name: 'Redis', kind: 'CACHE' },
    { name: 'S3 / R2', kind: 'BLOBS' },
    { name: 'DuckDB', kind: 'ANALYTICS' },
    { name: 'Parquet', kind: 'COLUMNAR' },
    { name: 'Arrow', kind: 'INTEROP' },
    { name: 'Qdrant', kind: 'VECTOR' },
    { name: 'Vectorize', kind: 'VECTOR' },
    { name: 'OpenAI', kind: 'EMBEDDINGS' },
    { name: 'Cohere', kind: 'EMBEDDINGS' },
    { name: 'HuggingFace', kind: 'MODELS' },
    { name: 'Triton', kind: 'INFERENCE' }
  ];
</script>

<section class="integ" id="integrations">
  <div class="section-label">07 · Integrations</div>
  <h2 class="h">Drop in to <span class="it">what you already run.</span></h2>

  <div class="integ-grid">
    {#each integrations as it}
      <div class="integ-cell">
        <span class="name">{it.name}</span>
        <span class="kind">{it.kind}</span>
      </div>
    {/each}
  </div>
</section>
