<script lang="ts">
  const types = [
    'plain text',
    'markdown',
    'json / yaml',
    'source code',
    'png / jpg / webp',
    'svg',
    'mp3 / wav / flac',
    'mp4 / mov / webm',
    'pdf',
    'parquet / arrow',
    'safetensors',
    'onnx / gguf',
    'embeddings (f32, f16, i8)',
    'arbitrary bytes'
  ];
</script>

<section class="types" id="types">
  <div class="section-label">04 · Inputs</div>
  <h2 class="h">If it's bytes, <span class="it">it has a fingerprint.</span></h2>
  <div class="type-row">
    {#each types as t}
      <span class="type-chip"><span class="dot"></span>{t}</span>
    {/each}
  </div>
</section>
