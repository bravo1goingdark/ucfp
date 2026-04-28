<script lang="ts">
  import { onMount } from 'svelte';
  import { fingerprintLocal, bytesEntropy } from '$lib/utils/fingerprint';
  import FpFlow from '$components/FpFlow.svelte';

  // ── algorithm registry ────────────────────────────────────────────────────
  const ALGORITHMS: Record<string, string[]> = {
    text:  ['minhash','simhash-tf','simhash-idf','lsh','tlsh',
            'semantic-openai','semantic-voyage','semantic-cohere','semantic-local'],
    image: ['multi','phash','dhash','ahash','semantic'],
    audio: ['wang','panako','haitsma','neural','watermark'],
  };
  const DEFAULT_ALG: Record<string, string> = {
    text: 'minhash', image: 'multi', audio: 'wang'
  };
  // sample rate per audio algorithm (Hz)
  const AUDIO_RATES: Record<string, number> = {
    wang: 8000, panako: 8000, haitsma: 5000, neural: 16000, watermark: 16000
  };
  const ALG_LABELS: Record<string, string> = {
    'minhash':          'MinHash',
    'simhash-tf':       'SimHash TF',
    'simhash-idf':      'SimHash IDF',
    'lsh':              'LSH',
    'tlsh':             'TLSH',
    'multi':            'Multi (P+D+A)',
    'phash':            'PHash',
    'dhash':            'DHash',
    'ahash':            'AHash',
    'semantic':         'Semantic (CLIP)',
    'wang':             'Wang',
    'panako':           'Panako',
    'haitsma':          'Haitsma',
    'neural':           'Neural (ONNX)',
    'watermark':        'Watermark',
    'semantic-openai':  'Semantic (OpenAI)',
    'semantic-voyage':  'Semantic (Voyage)',
    'semantic-cohere':  'Semantic (Cohere)',
    'semantic-local':   'Semantic (Local)',
  };

  // algorithms that require an API key
  const NEEDS_API_KEY = new Set(['semantic-openai','semantic-voyage','semantic-cohere']);
  // algorithms that require a model path / model ID
  const NEEDS_MODEL   = new Set(['semantic-local','semantic','neural','watermark']);

  // ── state ─────────────────────────────────────────────────────────────────
  type Modality = 'text' | 'image' | 'audio';

  let modality     = $state<Modality>('text');
  let algorithm    = $state('minhash');
  let textInput    = $state('The quick brown fox jumps over the lazy dog. Sphinx of black quartz, judge my vow.');
  let file         = $state<File | null>(null);
  let filePreviewUrl = $state<string | null>(null);
  let dragActive   = $state(false);
  let running      = $state(false);
  let errorMsg     = $state<string | null>(null);
  let rateLimitSec = $state(0);
  let showPipeline = $state(false);

  // Advanced options
  let modelId      = $state('');
  let apiKey       = $state('');

  // Fingerprint result metrics
  let cells      = $state<{ color: string }[]>([]);
  let algLabel   = $state('—');
  let cfgHash    = $state('—');
  let entropy    = $state('—');
  let fpBytes    = $state('—');
  let latencyMs  = $state<string>('—');
  let hexStr     = $state('');
  let hasResult  = $state(false);
  let isLocal    = $state(false);

  // Watermark result
  let isWatermark  = $state(false);
  let wmDetected   = $state(false);
  let wmConfidence = $state(0);
  let wmPayload    = $state<string | null>(null);

  // derived: does the selected algorithm need extra inputs?
  const needsAdvanced = $derived(NEEDS_API_KEY.has(algorithm) || NEEDS_MODEL.has(algorithm));

  // ── localStorage persistence ──────────────────────────────────────────────
  onMount(() => {
    try { showPipeline = localStorage.getItem('ucfp:pg:pipeline') === '1'; } catch { /* */ }
  });
  $effect(() => {
    try { localStorage.setItem('ucfp:pg:pipeline', showPipeline ? '1' : '0'); } catch { /* */ }
  });

  // ── rate-limit countdown ──────────────────────────────────────────────────
  let rlInterval: ReturnType<typeof setInterval> | null = null;
  function startRlCountdown(secs: number) {
    rateLimitSec = secs;
    if (rlInterval) clearInterval(rlInterval);
    rlInterval = setInterval(() => {
      rateLimitSec = Math.max(0, rateLimitSec - 1);
      if (rateLimitSec === 0 && rlInterval) { clearInterval(rlInterval); rlInterval = null; }
    }, 1000);
  }

  // ── modality switch ───────────────────────────────────────────────────────
  function switchModality(m: Modality) {
    modality     = m;
    algorithm    = DEFAULT_ALG[m];
    file         = null;
    if (filePreviewUrl) { URL.revokeObjectURL(filePreviewUrl); filePreviewUrl = null; }
    errorMsg     = null;
    hasResult    = false;
    isWatermark  = false;
    cells        = [];
  }

  // ── audio resampling + FormData builder ──────────────────────────────────
  async function buildFileForm(f: File, alg: string): Promise<FormData> {
    const fd = new FormData();
    const isAudio = (f.type || '').toLowerCase().startsWith('audio/');

    if (isAudio) {
      const targetRate = AUDIO_RATES[alg] ?? 8000;
      const arrayBuf = await f.arrayBuffer();
      const ACtx = (window.AudioContext ||
        (window as unknown as { webkitAudioContext: typeof AudioContext }).webkitAudioContext);
      const ac = new ACtx();
      try {
        const decoded = await ac.decodeAudioData(arrayBuf.slice(0));
        const sampleCount = Math.ceil(decoded.duration * targetRate);
        const offline = new OfflineAudioContext(1, sampleCount, targetRate);
        const src = offline.createBufferSource();
        src.buffer = decoded;
        src.connect(offline.destination);
        src.start(0);
        const resampled = await offline.startRendering();
        const ch = resampled.getChannelData(0);
        const bytes = new Uint8Array(ch.length * 4);
        const dv = new DataView(bytes.buffer);
        for (let i = 0; i < ch.length; i++) dv.setFloat32(i * 4, ch[i], true);
        fd.set('file', new File([bytes], 'audio.f32le', { type: 'audio/x-f32le' }));
        fd.set('sample_rate', String(targetRate));
      } finally {
        try { await ac.close(); } catch { /* */ }
      }
    } else {
      fd.set('file', f);
    }
    return fd;
  }

  // ── compute ───────────────────────────────────────────────────────────────
  async function compute() {
    if (running) return;
    errorMsg = null;

    // Validation
    if (NEEDS_API_KEY.has(algorithm) && !apiKey.trim()) {
      errorMsg = `${ALG_LABELS[algorithm]} requires an API key — open Advanced options.`;
      return;
    }
    if (NEEDS_MODEL.has(algorithm) && !modelId.trim()) {
      errorMsg = `${ALG_LABELS[algorithm]} requires a model path / ID — open Advanced options.`;
      return;
    }

    const t0 = performance.now();

    let body: BodyInit;
    let contentType: string | null = null;

    if (modality === 'text') {
      if (!textInput.trim()) { errorMsg = 'Enter some text first.'; return; }
      body = textInput;
      contentType = 'text/plain; charset=utf-8';
    } else {
      if (!file) { errorMsg = 'Drop a file first.'; return; }
      body = await buildFileForm(file, algorithm);
    }

    running = true;
    try {
      let url = `/api/fingerprint?algorithm=${encodeURIComponent(algorithm)}`;
      if (modelId.trim()) url += `&model_id=${encodeURIComponent(modelId.trim())}`;
      if (apiKey.trim())  url += `&api_key=${encodeURIComponent(apiKey.trim())}`;

      const init: RequestInit = { method: 'POST', body };
      if (contentType) init.headers = { 'content-type': contentType };

      const res = await fetch(url, init);
      const elapsed = Math.round(performance.now() - t0);

      if (res.status === 429) {
        const ra = Number(res.headers.get('retry-after') ?? '60');
        startRlCountdown(ra);
        errorMsg = `Rate limited — try again in ${ra}s.`;
        return;
      }
      if (res.status === 503) {
        // Backend not wired — local FNV-1a fallback
        const seed = modality === 'text' ? textInput : (file?.name ?? 'file');
        const local = fingerprintLocal(seed + '|' + algorithm);
        latencyMs   = `${elapsed} ms`;
        algLabel    = `${ALG_LABELS[algorithm] ?? algorithm} (local FNV-1a)`;
        cfgHash     = '—';
        fpBytes     = `${local.bytes.length} bytes`;
        hexStr      = local.hex;
        entropy     = `${bytesEntropy(local.bytes).toFixed(2)} bits`;
        cells       = Array.from(local.bytes).map(b => ({
          color: `oklch(0.55 0.15 ${Math.round((b / 255) * 360)}deg)`
        }));
        hasResult   = true;
        isLocal     = true;
        isWatermark = false;
        return;
      }
      if (res.status === 501) {
        errorMsg = `"${ALG_LABELS[algorithm] ?? algorithm}" is not compiled in this build. ` +
                   `Restart the server with the matching feature flag enabled.`;
        return;
      }
      if (!res.ok) {
        const msg = await res.text().catch(() => String(res.status));
        errorMsg = `Request failed (${res.status}): ${msg.slice(0, 300)}`;
        return;
      }

      const data = await res.json() as Record<string, unknown>;

      // Watermark response
      if (data.watermark === true) {
        isWatermark  = true;
        wmDetected   = Boolean(data.detected);
        wmConfidence = Number(data.confidence ?? 0);
        wmPayload    = typeof data.payload === 'string' ? data.payload : null;
        latencyMs    = `${elapsed} ms`;
        hasResult    = true;
        isLocal      = false;
        return;
      }

      // Normal fingerprint response
      isWatermark = false;
      latencyMs   = `${elapsed} ms`;
      algLabel    = String(data.algorithm ?? algorithm);
      cfgHash     = data.config_hash != null ? `0x${Number(data.config_hash).toString(16)}` : '—';
      fpBytes     = data.fingerprint_bytes != null ? `${data.fingerprint_bytes} bytes` : '—';

      // hex-grid: derive from local hash keyed on input + algorithm (server doesn't return raw bytes)
      const seed  = modality === 'text' ? textInput : (file?.name ?? 'file');
      const local = fingerprintLocal(seed + '|' + String(data.algorithm ?? algorithm));
      hexStr  = local.hex;
      entropy = `${bytesEntropy(local.bytes).toFixed(2)} bits`;
      cells   = Array.from(local.bytes).map(b => ({
        color: `oklch(0.55 0.15 ${Math.round((b / 255) * 360)}deg)`
      }));
      hasResult = true;
      isLocal   = false;
    } catch (e) {
      errorMsg = `Network error: ${(e as Error).message}`;
    } finally {
      running = false;
    }
  }

  // ── file drop ─────────────────────────────────────────────────────────────
  function attachFile(f: File) {
    file = f;
    if (filePreviewUrl) URL.revokeObjectURL(filePreviewUrl);
    filePreviewUrl = URL.createObjectURL(f);
    errorMsg    = null;
    hasResult   = false;
    isWatermark = false;
    cells       = [];
  }
  function onDrop(e: DragEvent) {
    e.preventDefault(); dragActive = false;
    const f = e.dataTransfer?.files[0];
    if (f) attachFile(f);
  }
  function onFileInput(e: Event) {
    const f = (e.currentTarget as HTMLInputElement).files?.[0];
    if (f) attachFile(f);
  }

  const ACCEPT: Record<Modality, string> = {
    text: '', image: 'image/*', audio: 'audio/*,.wav,.mp3,.ogg,.flac,.m4a'
  };
</script>

<div class="pg-wrap">
  <!-- ── header ────────────────────────────────────────────────────────── -->
  <div class="pg-head">
    <div>
      <h1 class="pg-title">Fingerprint Playground</h1>
      <p class="pg-sub">Run any algorithm against text, images, or audio and inspect the result.</p>
    </div>
    <label class="pipeline-toggle" title="Show algorithm pipeline graph">
      <input type="checkbox" bind:checked={showPipeline} />
      <span class="toggle-track"><span class="toggle-thumb"></span></span>
      <span class="toggle-label">Pipeline graph</span>
    </label>
  </div>

  <!-- ── modality tabs ─────────────────────────────────────────────────── -->
  <div class="mod-tabs" role="tablist" aria-label="Modality">
    {#each (['text','image','audio'] as const) as m}
      <button
        role="tab"
        aria-selected={modality === m}
        class="mod-tab"
        class:active={modality === m}
        onclick={() => switchModality(m)}
      >
        {m === 'text' ? '⟨T⟩ Text' : m === 'image' ? '⬡ Image' : '♪ Audio'}
      </button>
    {/each}
  </div>

  <!-- ── main grid ─────────────────────────────────────────────────────── -->
  <div class="pg-grid">
    <!-- Left pane: input + algorithm selector + advanced + run -->
    <div class="pg-pane">
      {#if modality === 'text'}
        <label class="pane-label" for="pg-text">Input text</label>
        <textarea
          id="pg-text"
          class="pg-textarea"
          bind:value={textInput}
          rows={6}
          placeholder="Enter text to fingerprint…"
        ></textarea>
      {:else}
        <div
          class="drop-zone"
          class:drag-over={dragActive}
          role="button"
          tabindex="0"
          aria-label="Drop {modality} file here or click to browse"
          ondragover={(e) => { e.preventDefault(); dragActive = true; }}
          ondragleave={() => { dragActive = false; }}
          ondrop={onDrop}
          onclick={() => document.getElementById('pg-file-input')?.click()}
          onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') document.getElementById('pg-file-input')?.click(); }}
        >
          {#if file && filePreviewUrl}
            {#if modality === 'image'}
              <img src={filePreviewUrl} alt={file.name} class="file-preview-img" />
            {:else}
              <div class="file-preview-audio">
                <span class="audio-icon">♪</span>
                <span class="file-name">{file.name}</span>
                <audio controls src={filePreviewUrl} class="audio-ctrl"></audio>
              </div>
            {/if}
          {:else}
            <div class="drop-hint">
              <span class="drop-icon">{modality === 'image' ? '⬡' : '♪'}</span>
              <span>Drop {modality} file here</span>
              <span class="drop-sub">or click to browse</span>
            </div>
          {/if}
        </div>
        <input
          id="pg-file-input"
          type="file"
          accept={ACCEPT[modality]}
          class="sr-only"
          onchange={onFileInput}
        />
      {/if}

      <div class="pane-label" id="algo-label">Algorithm</div>
      <div class="algo-grid" role="group" aria-labelledby="algo-label">
        {#each ALGORITHMS[modality] as alg}
          <button
            class="algo-btn"
            class:selected={algorithm === alg}
            class:needs-input={NEEDS_API_KEY.has(alg) || NEEDS_MODEL.has(alg)}
            onclick={() => { algorithm = alg; }}
            aria-pressed={algorithm === alg}
            title={NEEDS_API_KEY.has(alg) ? 'Requires API key' : NEEDS_MODEL.has(alg) ? 'Requires model path' : ''}
          >
            {ALG_LABELS[alg] ?? alg}
          </button>
        {/each}
      </div>

      <!-- Advanced options (model path / API key) -->
      {#if needsAdvanced}
        <details class="adv-opts">
          <summary class="adv-summary">
            Advanced options
            {#if (NEEDS_MODEL.has(algorithm) && !modelId.trim()) || (NEEDS_API_KEY.has(algorithm) && !apiKey.trim())}
              <span class="adv-required">required ↓</span>
            {/if}
          </summary>
          <div class="adv-body">
            {#if NEEDS_MODEL.has(algorithm)}
              <label class="adv-label">
                Model path / ID
                <input
                  class="adv-input"
                  type="text"
                  bind:value={modelId}
                  placeholder={algorithm === 'watermark' || algorithm === 'neural'
                    ? '/models/audio.onnx'
                    : algorithm === 'semantic' ? '/models/clip.onnx'
                    : 'sentence-transformers/all-MiniLM-L6-v2'}
                />
              </label>
            {/if}
            {#if NEEDS_API_KEY.has(algorithm)}
              <label class="adv-label">
                API key
                <input class="adv-input" type="password" bind:value={apiKey} placeholder="sk-…" />
              </label>
            {/if}
          </div>
        </details>
      {/if}

      {#if errorMsg}
        <p class="pg-error" role="alert">{errorMsg}</p>
      {/if}
      {#if rateLimitSec > 0}
        <p class="pg-warn">Rate limited — retry in {rateLimitSec}s</p>
      {/if}

      <button
        class="run-btn"
        onclick={compute}
        disabled={running || rateLimitSec > 0}
        aria-busy={running}
      >
        {running ? 'Running…' : 'Run fingerprint'}
      </button>
    </div>

    <!-- Right pane: results -->
    <div class="pg-pane result-pane">
      {#if isWatermark && hasResult}
        <!-- Watermark detection result -->
        <div class="pane-label">Watermark detection result</div>
        <div class="wm-result">
          <span class="wm-pill" class:detected={wmDetected}>
            {wmDetected ? '✓ Watermark detected' : '✗ No watermark detected'}
          </span>
          <div class="metrics-grid">
            <div class="metric-card">
              <span class="metric-k">Confidence</span>
              <span class="metric-v">{(wmConfidence * 100).toFixed(1)}%</span>
            </div>
            <div class="metric-card">
              <span class="metric-k">Latency</span>
              <span class="metric-v">{latencyMs}</span>
            </div>
            {#if wmPayload}
              <div class="metric-card" style="grid-column:1/-1">
                <span class="metric-k">Payload</span>
                <span class="metric-v mono">{wmPayload}</span>
              </div>
            {/if}
          </div>
        </div>
      {:else}
        <!-- Normal fingerprint / embedding result -->
        <div class="pane-label">Fingerprint visualization</div>
        {#if hasResult}
          <div class="hex-grid" aria-label="Fingerprint byte visualization">
            {#each cells as cell}
              <div class="hex-cell" style="background:{cell.color}"></div>
            {/each}
          </div>
          <div class="hex-str" title={hexStr}>{hexStr.slice(0, 64)}{hexStr.length > 64 ? '…' : ''}</div>
        {:else}
          <div class="result-empty">
            <span class="empty-icon">⬡</span>
            <span>Run a fingerprint to see the visualization</span>
          </div>
        {/if}

        {#if isLocal}
          <p class="local-notice" role="status">
            FALLBACK · LOCAL FNV-1a — backend not connected (set <code>UCFP_API_URL</code> to enable real fingerprinting)
          </p>
        {/if}

        <div class="metrics-grid">
          <div class="metric-card">
            <span class="metric-k">Algorithm</span>
            <span class="metric-v">{algLabel}</span>
          </div>
          <div class="metric-card">
            <span class="metric-k">Config hash</span>
            <span class="metric-v mono">{cfgHash}</span>
          </div>
          <div class="metric-card">
            <span class="metric-k">Entropy</span>
            <span class="metric-v">{entropy}</span>
          </div>
          <div class="metric-card">
            <span class="metric-k">FP size</span>
            <span class="metric-v">{fpBytes}</span>
          </div>
          <div class="metric-card">
            <span class="metric-k">Latency</span>
            <span class="metric-v">{latencyMs}</span>
          </div>
        </div>
      {/if}
    </div>
  </div>

  <!-- ── optional pipeline graph ────────────────────────────────────────── -->
  {#if showPipeline}
    <div class="pipeline-section">
      <div class="pane-label">How {ALG_LABELS[algorithm] ?? algorithm} works</div>
      <FpFlow {modality} {algorithm} />
    </div>
  {/if}
</div>

<style>
  /* ── Advanced options ───────────────────────────────────────────────── */
  .adv-opts { border: 1px solid var(--ink); border-radius: 4px; overflow: hidden; }
  .adv-summary {
    font-family: var(--mono); font-size: 0.72rem; cursor: pointer;
    padding: 0.4rem 0.65rem; color: var(--ink-2);
    display: flex; align-items: center; gap: 0.5rem;
    list-style: none;
  }
  .adv-summary::-webkit-details-marker { display: none; }
  .adv-summary::before { content: '▶'; font-size: 0.55rem; transition: transform 0.15s; }
  details[open] .adv-summary::before { transform: rotate(90deg); }
  .adv-required {
    font-size: 0.65rem; color: #b03030;
    background: color-mix(in srgb, #b03030 10%, transparent);
    padding: 1px 5px; border-radius: 3px;
  }
  .adv-body {
    padding: 0.6rem 0.65rem; display: flex; flex-direction: column; gap: 0.5rem;
    background: var(--bg-2);
  }
  .adv-label {
    display: flex; flex-direction: column; gap: 3px;
    font-family: var(--mono); font-size: 0.7rem; color: var(--ink-2);
  }
  .adv-input {
    font-family: var(--mono); font-size: 0.72rem;
    padding: 5px 8px; border: 1px solid var(--ink);
    border-radius: 3px; background: var(--bg); color: var(--ink);
    width: 100%; box-sizing: border-box;
  }
  .adv-input:focus { outline: 2px solid var(--accent-ink); outline-offset: 1px; }

  /* algo buttons that need extra input get a subtle dot indicator */
  .algo-btn.needs-input:not(.selected)::after {
    content: '·'; margin-left: 2px; color: var(--ink-2); opacity: 0.6;
  }

  /* ── Watermark result ───────────────────────────────────────────────── */
  .wm-result { display: flex; flex-direction: column; gap: 1rem; padding: 0.5rem 0; }
  .wm-pill {
    display: inline-flex; align-items: center;
    font-family: var(--mono); font-size: 0.82rem; font-weight: 600;
    padding: 7px 16px; border-radius: 20px;
    background: color-mix(in oklch, var(--ink) 6%, transparent);
    color: var(--ink-2); border: 1px solid var(--ink);
    align-self: flex-start;
  }
  .wm-pill.detected {
    background: color-mix(in oklch, oklch(0.58 0.18 145) 15%, transparent);
    color: oklch(0.38 0.15 145);
    border-color: oklch(0.58 0.18 145);
  }

  /* ── Local notice ───────────────────────────────────────────────────── */
  .local-notice {
    font-family: var(--mono); font-size: 11px;
    padding: 6px 10px; border-radius: 4px;
    background: color-mix(in oklch, var(--accent-ink) 10%, var(--bg));
    color: var(--ink-2);
    border: 1px solid color-mix(in oklch, var(--accent-ink) 30%, transparent);
    margin: 0 0 0.75rem;
  }
  .local-notice code {
    font-family: inherit;
    background: color-mix(in oklch, var(--ink) 8%, transparent);
    padding: 1px 4px; border-radius: 3px;
  }

  /* ── Layout ─────────────────────────────────────────────────────────── */
  .pg-wrap { display: flex; flex-direction: column; gap: 1.5rem; }

  .pg-head {
    display: flex; align-items: flex-start;
    justify-content: space-between; gap: 1rem; flex-wrap: wrap;
  }
  .pg-title { font-size: 1.25rem; font-weight: 700; margin: 0 0 0.25rem; }
  .pg-sub { margin: 0; color: var(--ink-2); font-size: 0.85rem; }

  /* Pipeline toggle */
  .pipeline-toggle {
    display: flex; align-items: center; gap: 0.5rem;
    cursor: pointer; user-select: none;
    font-size: 0.8rem; color: var(--ink-2); margin-top: 4px;
  }
  .pipeline-toggle input { display: none; }
  .toggle-track {
    width: 36px; height: 20px; border-radius: 10px;
    background: var(--bg-2); border: 1px solid var(--ink);
    position: relative; transition: background 0.15s; flex-shrink: 0;
  }
  .toggle-thumb {
    position: absolute; top: 2px; left: 2px;
    width: 14px; height: 14px; border-radius: 50%;
    background: var(--ink-2); transition: transform 0.15s, background 0.15s;
  }
  .pipeline-toggle:has(input:checked) .toggle-track { background: var(--accent-ink); }
  .pipeline-toggle:has(input:checked) .toggle-thumb { transform: translateX(16px); background: var(--bg); }
  .toggle-label { color: var(--ink); }

  /* Modality tabs */
  .mod-tabs { display: flex; gap: 0.5rem; }
  .mod-tab {
    font-family: var(--mono); font-size: 0.8rem;
    padding: 0.35rem 0.9rem; border: 1px solid var(--ink);
    background: transparent; color: var(--ink);
    border-radius: 3px; cursor: pointer;
    transition: background 0.1s, color 0.1s;
  }
  .mod-tab:hover { background: var(--bg-2); }
  .mod-tab.active { background: var(--ink); color: var(--bg); }

  /* Two-column grid */
  .pg-grid {
    display: grid; grid-template-columns: 1.1fr 0.9fr;
    gap: 1.5rem; align-items: start;
  }
  @media (max-width: 700px) { .pg-grid { grid-template-columns: 1fr; } }

  .pg-pane { display: flex; flex-direction: column; gap: 0.75rem; }
  .pane-label {
    font-family: var(--mono); font-size: 0.7rem;
    text-transform: uppercase; letter-spacing: 0.08em; color: var(--ink-2);
  }

  /* Text input */
  .pg-textarea {
    font-family: var(--mono); font-size: 0.78rem;
    border: 1px solid var(--ink); background: var(--bg-2); color: var(--ink);
    border-radius: 4px; padding: 0.6rem 0.75rem;
    resize: vertical; min-height: 120px; line-height: 1.5;
  }

  /* Drop zone */
  .drop-zone {
    border: 1px dashed var(--ink); border-radius: 6px;
    background: var(--bg-2); min-height: 160px;
    display: flex; align-items: center; justify-content: center;
    cursor: pointer; transition: border-color 0.15s, background 0.15s; overflow: hidden;
  }
  .drop-zone.drag-over { border-color: var(--accent-ink); background: var(--bg); }
  .drop-zone:focus-visible { outline: 2px solid var(--accent-ink); outline-offset: 2px; }
  .drop-hint {
    display: flex; flex-direction: column; align-items: center;
    gap: 0.35rem; color: var(--ink-2); font-size: 0.82rem;
  }
  .drop-icon { font-size: 2rem; }
  .drop-sub { font-size: 0.72rem; opacity: 0.7; }
  .file-preview-img { max-width: 100%; max-height: 200px; object-fit: contain; display: block; }
  .file-preview-audio {
    display: flex; flex-direction: column; align-items: center; gap: 0.5rem; padding: 1rem;
  }
  .audio-icon { font-size: 2rem; }
  .file-name { font-family: var(--mono); font-size: 0.72rem; word-break: break-all; }
  .audio-ctrl { width: 100%; max-width: 260px; }

  .sr-only {
    position: absolute; width: 1px; height: 1px;
    padding: 0; margin: -1px; overflow: hidden;
    clip: rect(0,0,0,0); border: 0;
  }

  /* Algorithm grid */
  .algo-grid {
    display: grid; grid-template-columns: repeat(auto-fill, minmax(100px, 1fr)); gap: 0.4rem;
  }
  .algo-btn {
    font-family: var(--mono); font-size: 0.7rem;
    padding: 0.3rem 0.5rem; border: 1px solid var(--ink);
    background: transparent; color: var(--ink-2);
    border-radius: 3px; cursor: pointer;
    transition: background 0.1s, color 0.1s, border-color 0.1s;
    text-align: center; line-height: 1.3;
  }
  .algo-btn:hover { background: var(--bg-2); color: var(--ink); }
  .algo-btn.selected { background: var(--ink); color: var(--bg); border-color: var(--ink); }

  /* Error / warn */
  .pg-error {
    font-family: var(--mono); font-size: 0.75rem; color: #b03030; margin: 0;
    padding: 0.4rem 0.6rem; border: 1px solid currentColor; border-radius: 3px;
    background: color-mix(in srgb, #b03030 8%, transparent);
  }
  .pg-warn { font-family: var(--mono); font-size: 0.75rem; color: #8a6000; margin: 0; }

  /* Run button */
  .run-btn {
    font-family: var(--mono); font-size: 0.82rem;
    padding: 0.55rem 1.2rem; border: 1px solid var(--ink);
    background: var(--ink); color: var(--bg);
    border-radius: 3px; cursor: pointer;
    transition: opacity 0.15s; align-self: flex-start;
  }
  .run-btn:disabled { opacity: 0.45; cursor: not-allowed; }
  .run-btn:not(:disabled):hover { opacity: 0.85; }

  /* Result pane */
  .result-pane {
    padding: 0.75rem; background: var(--bg-2);
    border-radius: 6px; border: 1px solid var(--ink);
  }
  .result-empty {
    display: flex; flex-direction: column; align-items: center; justify-content: center;
    gap: 0.4rem; min-height: 100px; color: var(--ink-2); font-size: 0.8rem;
  }
  .empty-icon { font-size: 1.8rem; opacity: 0.4; }

  /* Hex grid */
  .hex-grid {
    display: grid; grid-template-columns: repeat(16, 14px);
    gap: 2px; margin-bottom: 0.5rem;
  }
  .hex-cell {
    width: 14px; height: 14px; border-radius: 2px;
    animation: pop-in 0.2s ease both;
  }
  @keyframes pop-in {
    from { transform: scale(0); opacity: 0; }
    to   { transform: scale(1); opacity: 1; }
  }
  .hex-str {
    font-family: var(--mono); font-size: 0.6rem; color: var(--ink-2);
    word-break: break-all; margin-bottom: 0.75rem;
  }

  /* Metrics */
  .metrics-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 0.5rem; }
  .metric-card {
    display: flex; flex-direction: column; gap: 2px;
    padding: 0.4rem 0.6rem; background: var(--bg);
    border-radius: 4px; border: 1px solid var(--ink);
  }
  .metric-card:last-child:nth-child(odd) { grid-column: 1 / -1; }
  .metric-k {
    font-family: var(--mono); font-size: 0.62rem;
    text-transform: uppercase; letter-spacing: 0.06em; color: var(--ink-2);
  }
  .metric-v { font-family: var(--mono); font-size: 0.8rem; color: var(--ink); font-weight: 600; }
  .mono { word-break: break-all; }

  /* Pipeline section */
  .pipeline-section { display: flex; flex-direction: column; gap: 0.5rem; }
</style>
