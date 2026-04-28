<script lang="ts">
  import { onMount } from 'svelte';
  import { fingerprintLocal, bytesEntropy, hammingDistance } from '$lib/utils/fingerprint';
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
  const ALG_DESCRIPTIONS: Record<string, string> = {
    'minhash':
      'Estimates Jaccard similarity between documents. Generates H independent min-hashes over k-shingles (overlapping word sequences). Fast, storage-compact, and scales to billions of records.',
    'simhash-tf':
      'Assigns a 64-bit fingerprint by summing term-frequency-weighted token hashes. Near-duplicate text produces signatures with small Hamming distance — finding matches is a bit-comparison.',
    'simhash-idf':
      'Like SimHash-TF but downweights common terms via IDF. Better at discriminating semantically distinct documents that share common vocabulary.',
    'lsh':
      'Band-partitions the MinHash signature so near-duplicate pairs collide in the same bucket with high probability. Optimised for high-recall retrieval in large corpora.',
    'tlsh':
      'Trend Micro Locality-Sensitive Hash — byte-histogram-based digest that supports numeric distance queries. Requires ≥50 bytes. Used for malware clustering and file similarity.',
    'multi':
      'Bundles PHash + DHash + AHash into one record. Best default for images — simultaneously tolerates crop, resize, JPEG re-encoding, and brightness/contrast shifts.',
    'phash':
      'DCT-based 64-bit perceptual hash. Robust to resize and JPEG artefacts; stable across colour-space conversions. The gold standard for photo deduplication.',
    'dhash':
      'Gradient-difference 64-bit hash. Extremely fast; captures directional edge patterns rather than absolute colour. Good for detecting rotation and local edits.',
    'ahash':
      'Mean-threshold 64-bit hash — fastest perceptual hash. Works well for graphics and cartoons; less robust than PHash to natural photo variation.',
    'semantic':
      'CLIP-style ONNX vision embedding (512-d). Encodes visual semantics rather than pixel similarity — matches images that are conceptually related, not just visually similar.',
    'wang':
      'Shazam-style landmark hash. Hashes peak-pair constellations from the audio spectrogram into (f₁, f₂, Δt) triplets — highly robust to noise, pitch shift, and lossy re-encoding.',
    'panako':
      'Three-point frequency constellation hashes. Invariant to playback speed changes up to ±20%, making it ideal for time-stretched or sped-up copies.',
    'haitsma':
      'Philips robust hash (Haitsma–Kalker). Sub-band energy-ratio hashing at 5 kHz. Extremely robust to codec re-encoding and transmission noise.',
    'neural':
      'ONNX log-mel neural audio embedding. Encodes audio semantics rather than acoustic structure — matches speech or music across recording conditions.',
    'watermark':
      'Detect AudioSeal-style invisible neural watermarks embedded in audio at ingest time. Returns a detection confidence and optionally decoded payload bits.',
    'semantic-openai':
      'Dense text embedding via OpenAI\'s Embeddings API. Captures deep semantic meaning for cross-paraphrase and multilingual matching. Requires an OpenAI API key.',
    'semantic-voyage':
      'High-quality retrieval-optimised embedding via Voyage AI\'s API. Strong on technical and domain-specific content.',
    'semantic-cohere':
      'Multilingual dense embedding via Cohere\'s Embed API. Strong for cross-language deduplication across 100+ languages.',
    'semantic-local':
      'Run a local ONNX text encoder (BGE, E5, MiniLM) — no API key needed. Specify the model file path. Zero data leaves your server.',
  };

  const NEEDS_API_KEY = new Set(['semantic-openai','semantic-voyage','semantic-cohere']);
  const NEEDS_MODEL   = new Set(['semantic-local','semantic','neural','watermark']);

  // ── state ─────────────────────────────────────────────────────────────────
  type Modality = 'text' | 'image' | 'audio';

  let modality       = $state<Modality>('text');
  let algorithm      = $state('minhash');
  let textInput      = $state('The quick brown fox jumps over the lazy dog. Sphinx of black quartz, judge my vow.');
  let file           = $state<File | null>(null);
  let filePreviewUrl = $state<string | null>(null);
  let dragActive     = $state(false);
  let running        = $state(false);
  let errorMsg       = $state<string | null>(null);
  let rateLimitSec   = $state(0);
  let showPipeline   = $state(true);
  let compareMode    = $state(false);

  // Advanced options
  let modelId = $state('');
  let apiKey  = $state('');

  // ── result A ──────────────────────────────────────────────────────────────
  let cells      = $state<{ color: string }[]>([]);
  let hexBytesA  = $state<Uint8Array | null>(null);
  let algLabel   = $state('—');
  let cfgHash    = $state('—');
  let entropy    = $state('—');
  let fpBytes    = $state('—');
  let latencyMs  = $state<string>('—');
  let hexStr     = $state('');
  let hasResult  = $state(false);
  let isLocal    = $state(false);

  // Watermark A
  let isWatermark  = $state(false);
  let wmDetected   = $state(false);
  let wmConfidence = $state(0);
  let wmPayload    = $state<string | null>(null);

  // ── compare mode state ────────────────────────────────────────────────────
  let textInputB      = $state('The quick brown fox leaps over the sleepy cat. Sphinx of black quartz, judge my vow!');
  let fileB           = $state<File | null>(null);
  let fileBPreviewUrl = $state<string | null>(null);
  let dragActiveB     = $state(false);
  let runningB        = $state(false);
  let errorMsgB       = $state<string | null>(null);

  // result B
  let cellsB     = $state<{ color: string }[]>([]);
  let hexBytesB  = $state<Uint8Array | null>(null);
  let algLabelB  = $state('—');
  let entropyB   = $state('—');
  let fpBytesB   = $state('—');
  let latencyMsB = $state<string>('—');
  let hexStrB    = $state('');
  let hasResultB = $state(false);
  let isLocalB   = $state(false);

  // ── similarity (derived) ──────────────────────────────────────────────────
  const hammingBits = $derived.by(() => {
    if (!hexBytesA || !hexBytesB) return null;
    const len = Math.min(hexBytesA.length, hexBytesB.length, 128);
    return hammingDistance(hexBytesA.slice(0, len), hexBytesB.slice(0, len));
  });
  const totalBits = $derived.by(() => {
    if (!hexBytesA || !hexBytesB) return null;
    return Math.min(hexBytesA.length, hexBytesB.length, 128) * 8;
  });
  const similarityPct = $derived.by(() => {
    if (hammingBits === null || totalBits === null || totalBits === 0) return null;
    return ((1 - hammingBits / totalBits) * 100).toFixed(1);
  });
  // byte-level diff mask for grid highlighting (first 128 bytes)
  const diffMask = $derived.by(() => {
    if (!hexBytesA || !hexBytesB || !compareMode) return null;
    const len = Math.min(hexBytesA.length, hexBytesB.length, 128);
    const mask: boolean[] = new Array(len);
    for (let i = 0; i < len; i++) mask[i] = hexBytesA[i] !== hexBytesB[i];
    return mask;
  });

  const needsAdvanced = $derived(NEEDS_API_KEY.has(algorithm) || NEEDS_MODEL.has(algorithm));

  // ── localStorage persistence ──────────────────────────────────────────────
  onMount(() => {
    try {
      const s = localStorage.getItem('ucfp:pg:pipeline');
      if (s !== null) showPipeline = s === '1';
    } catch { /* */ }
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
    modality      = m;
    algorithm     = DEFAULT_ALG[m];
    file          = null; fileB = null;
    if (filePreviewUrl) { URL.revokeObjectURL(filePreviewUrl); filePreviewUrl = null; }
    if (fileBPreviewUrl) { URL.revokeObjectURL(fileBPreviewUrl); fileBPreviewUrl = null; }
    errorMsg      = null; errorMsgB = null;
    hasResult     = false; hasResultB = false;
    isWatermark   = false;
    cells = []; cellsB = [];
    hexBytesA = null; hexBytesB = null;
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
        src.buffer = decoded; src.connect(offline.destination); src.start(0);
        const resampled = await offline.startRendering();
        const ch = resampled.getChannelData(0);
        const bytes = new Uint8Array(ch.length * 4);
        const dv = new DataView(bytes.buffer);
        for (let i = 0; i < ch.length; i++) dv.setFloat32(i * 4, ch[i], true);
        fd.set('file', new File([bytes], 'audio.f32le', { type: 'audio/x-f32le' }));
        fd.set('sample_rate', String(targetRate));
      } finally { try { await ac.close(); } catch { /* */ } }
    } else { fd.set('file', f); }
    return fd;
  }

  // ── hex helpers ──────────────────────────────────────────────────────────
  function hexToBytes(hex: string, maxBytes = 128): Uint8Array {
    const byteCount = Math.min(Math.floor(hex.length / 2), maxBytes);
    const b = new Uint8Array(byteCount);
    for (let i = 0; i < byteCount; i++) b[i] = parseInt(hex.slice(i * 2, i * 2 + 2), 16);
    return b;
  }

  function bytesToCells(bytes: Uint8Array): { color: string }[] {
    return Array.from(bytes).map(b => ({
      color: `oklch(0.55 0.15 ${Math.round((b / 255) * 360)}deg)`
    }));
  }

  // ── core fingerprint runner ───────────────────────────────────────────────
  type FpRunOk = {
    kind: 'ok';
    cells: { color: string }[];
    hexBytes: Uint8Array | null;
    hexStr: string;
    algLabel: string;
    cfgHash: string;
    fpBytes: string;
    entropy: string;
    latencyMs: string;
    isLocal: boolean;
  };
  type FpRunWatermark = {
    kind: 'watermark';
    detected: boolean; confidence: number; payload: string | null;
    latencyMs: string;
  };
  type FpRunError    = { kind: 'error'; msg: string };
  type FpRunRateLimit = { kind: 'rate-limit'; retryAfter: number };
  type FpRunResult = FpRunOk | FpRunWatermark | FpRunError | FpRunRateLimit;

  async function runFingerprint(
    textIn: string,
    fileIn: File | null,
    seedFallback: string,
  ): Promise<FpRunResult> {
    if (NEEDS_API_KEY.has(algorithm) && !apiKey.trim())
      return { kind: 'error', msg: `${ALG_LABELS[algorithm]} requires an API key — open Advanced options.` };
    if (NEEDS_MODEL.has(algorithm) && !modelId.trim())
      return { kind: 'error', msg: `${ALG_LABELS[algorithm]} requires a model path — open Advanced options.` };

    const t0 = performance.now();
    let body: BodyInit;
    let contentType: string | null = null;

    if (modality === 'text') {
      if (!textIn.trim()) return { kind: 'error', msg: 'Enter some text first.' };
      body = textIn; contentType = 'text/plain; charset=utf-8';
    } else {
      if (!fileIn) return { kind: 'error', msg: 'Drop a file first.' };
      body = await buildFileForm(fileIn, algorithm);
    }

    let url = `/api/fingerprint?algorithm=${encodeURIComponent(algorithm)}`;
    if (modelId.trim()) url += `&model_id=${encodeURIComponent(modelId.trim())}`;
    if (apiKey.trim())  url += `&api_key=${encodeURIComponent(apiKey.trim())}`;

    const init: RequestInit = { method: 'POST', body };
    if (contentType) init.headers = { 'content-type': contentType };

    const res = await fetch(url, init);
    const elapsed = Math.round(performance.now() - t0);

    if (res.status === 429) {
      const ra = Number(res.headers.get('retry-after') ?? '60');
      return { kind: 'rate-limit', retryAfter: ra };
    }
    if (res.status === 503) {
      const local = fingerprintLocal(seedFallback + '|' + algorithm);
      return {
        kind: 'ok',
        cells: bytesToCells(local.bytes),
        hexBytes: local.bytes,
        hexStr: local.hex,
        algLabel: `${ALG_LABELS[algorithm] ?? algorithm} (local FNV-1a)`,
        cfgHash: '—',
        fpBytes: `${local.bytes.length} bytes`,
        entropy: `${bytesEntropy(local.bytes).toFixed(2)} bits`,
        latencyMs: `${elapsed} ms`,
        isLocal: true,
      };
    }
    if (res.status === 501) {
      return { kind: 'error', msg: `"${ALG_LABELS[algorithm] ?? algorithm}" is not compiled in this build. Restart the server with the matching feature flag enabled.` };
    }
    if (!res.ok) {
      const msg = await res.text().catch(() => String(res.status));
      return { kind: 'error', msg: `Request failed (${res.status}): ${msg.slice(0, 300)}` };
    }

    const data = await res.json() as Record<string, unknown>;

    if (data.watermark === true) {
      return {
        kind: 'watermark',
        detected: Boolean(data.detected),
        confidence: Number(data.confidence ?? 0),
        payload: typeof data.payload === 'string' ? data.payload : null,
        latencyMs: `${elapsed} ms`,
      };
    }

    // Use real fingerprint_hex from server; fall back to FNV-1a if absent
    let displayBytes: Uint8Array;
    let displayHex: string;
    if (typeof data.fingerprint_hex === 'string' && data.fingerprint_hex.length > 0) {
      displayBytes = hexToBytes(data.fingerprint_hex);
      displayHex   = data.fingerprint_hex;
    } else {
      const local = fingerprintLocal(seedFallback + '|' + String(data.algorithm ?? algorithm));
      displayBytes = local.bytes;
      displayHex   = local.hex;
    }

    return {
      kind: 'ok',
      cells: bytesToCells(displayBytes),
      hexBytes: displayBytes,
      hexStr: displayHex,
      algLabel: String(data.algorithm ?? algorithm),
      cfgHash: data.config_hash != null ? `0x${Number(data.config_hash).toString(16)}` : '—',
      fpBytes: data.fingerprint_bytes != null ? `${data.fingerprint_bytes} bytes` : '—',
      entropy: `${bytesEntropy(displayBytes).toFixed(2)} bits`,
      latencyMs: `${elapsed} ms`,
      isLocal: false,
    };
  }

  // ── compute A ─────────────────────────────────────────────────────────────
  async function compute() {
    if (running) return;
    errorMsg = null;
    running  = true;
    try {
      const seed = modality === 'text' ? textInput : (file?.name ?? 'file');
      const result = await runFingerprint(textInput, file, seed);
      if (result.kind === 'rate-limit') {
        startRlCountdown(result.retryAfter);
        errorMsg = `Rate limited — try again in ${result.retryAfter}s.`;
        return;
      }
      if (result.kind === 'error') { errorMsg = result.msg; return; }
      if (result.kind === 'watermark') {
        isWatermark  = true;
        wmDetected   = result.detected;
        wmConfidence = result.confidence;
        wmPayload    = result.payload;
        latencyMs    = result.latencyMs;
        hasResult    = true; isLocal = false;
        hexBytesA    = null;
        return;
      }
      isWatermark = false;
      cells      = result.cells;
      hexBytesA  = result.hexBytes;
      hexStr     = result.hexStr;
      algLabel   = result.algLabel;
      cfgHash    = result.cfgHash;
      fpBytes    = result.fpBytes;
      entropy    = result.entropy;
      latencyMs  = result.latencyMs;
      isLocal    = result.isLocal;
      hasResult  = true;
    } catch (e) {
      errorMsg = `Network error: ${(e as Error).message}`;
    } finally {
      running = false;
    }
  }

  // ── compute B ─────────────────────────────────────────────────────────────
  async function computeB() {
    if (runningB) return;
    errorMsgB = null;
    runningB  = true;
    try {
      const seed = modality === 'text' ? textInputB : (fileB?.name ?? 'file-b');
      const result = await runFingerprint(textInputB, fileB, seed);
      if (result.kind === 'rate-limit') {
        startRlCountdown(result.retryAfter);
        errorMsgB = `Rate limited — try again in ${result.retryAfter}s.`;
        return;
      }
      if (result.kind === 'error') { errorMsgB = result.msg; return; }
      if (result.kind === 'watermark') {
        hasResultB = true; isLocalB = false; hexBytesB = null;
        return;
      }
      cellsB     = result.cells;
      hexBytesB  = result.hexBytes;
      hexStrB    = result.hexStr;
      algLabelB  = result.algLabel;
      fpBytesB   = result.fpBytes;
      entropyB   = result.entropy;
      latencyMsB = result.latencyMs;
      isLocalB   = result.isLocal;
      hasResultB = true;
    } catch (e) {
      errorMsgB = `Network error: ${(e as Error).message}`;
    } finally {
      runningB = false;
    }
  }

  // ── run both (compare mode) ───────────────────────────────────────────────
  async function runBoth() {
    if (running || runningB) return;
    errorMsg = null; errorMsgB = null;
    running = true; runningB = true;
    try {
      const seedA = modality === 'text' ? textInput  : (file?.name  ?? 'file-a');
      const seedB = modality === 'text' ? textInputB : (fileB?.name ?? 'file-b');
      const [ra, rb] = await Promise.all([
        runFingerprint(textInput,  file,  seedA),
        runFingerprint(textInputB, fileB, seedB),
      ]);
      // apply A
      if (ra.kind === 'rate-limit') startRlCountdown(ra.retryAfter);
      if (ra.kind === 'error') errorMsg = ra.msg;
      if (ra.kind === 'ok') {
        isWatermark = false; cells = ra.cells; hexBytesA = ra.hexBytes;
        hexStr = ra.hexStr; algLabel = ra.algLabel; cfgHash = ra.cfgHash;
        fpBytes = ra.fpBytes; entropy = ra.entropy; latencyMs = ra.latencyMs;
        isLocal = ra.isLocal; hasResult = true;
      }
      // apply B
      if (rb.kind === 'error') errorMsgB = rb.msg;
      if (rb.kind === 'ok') {
        cellsB = rb.cells; hexBytesB = rb.hexBytes;
        hexStrB = rb.hexStr; algLabelB = rb.algLabel;
        fpBytesB = rb.fpBytes; entropyB = rb.entropy; latencyMsB = rb.latencyMs;
        isLocalB = rb.isLocal; hasResultB = true;
      }
    } catch (e) {
      errorMsg = `Network error: ${(e as Error).message}`;
    } finally {
      running = false; runningB = false;
    }
  }

  // ── file drop ─────────────────────────────────────────────────────────────
  function attachFile(f: File) {
    file = f;
    if (filePreviewUrl) URL.revokeObjectURL(filePreviewUrl);
    filePreviewUrl = URL.createObjectURL(f);
    errorMsg = null; hasResult = false; isWatermark = false; cells = []; hexBytesA = null;
  }
  function attachFileB(f: File) {
    fileB = f;
    if (fileBPreviewUrl) URL.revokeObjectURL(fileBPreviewUrl);
    fileBPreviewUrl = URL.createObjectURL(f);
    errorMsgB = null; hasResultB = false; cellsB = []; hexBytesB = null;
  }
  function onDrop(e: DragEvent) {
    e.preventDefault(); dragActive = false;
    const f = e.dataTransfer?.files[0]; if (f) attachFile(f);
  }
  function onDropB(e: DragEvent) {
    e.preventDefault(); dragActiveB = false;
    const f = e.dataTransfer?.files[0]; if (f) attachFileB(f);
  }
  function onFileInput(e: Event) {
    const f = (e.currentTarget as HTMLInputElement).files?.[0]; if (f) attachFile(f);
  }
  function onFileInputB(e: Event) {
    const f = (e.currentTarget as HTMLInputElement).files?.[0]; if (f) attachFileB(f);
  }

  const ACCEPT: Record<Modality, string> = {
    text: '', image: 'image/*', audio: 'audio/*,.wav,.mp3,.ogg,.flac,.m4a'
  };

  // similarity bar width (0–100)
  const simBarWidth = $derived(similarityPct !== null ? Number(similarityPct) : 0);
  const simColor = $derived(
    simBarWidth >= 90 ? 'oklch(0.55 0.18 145)' :
    simBarWidth >= 60 ? 'oklch(0.55 0.18 80)'  :
                        'oklch(0.55 0.18 20)'
  );
</script>

<div class="pg-wrap">
  <!-- ── header ────────────────────────────────────────────────────────── -->
  <div class="pg-head">
    <div>
      <h1 class="pg-title">Fingerprint Playground</h1>
      <p class="pg-sub">Run any algorithm against text, images, or audio and inspect the result.</p>
    </div>
    <div class="pg-head-controls">
      <label class="toggle-control" title="Compare two inputs side by side">
        <input type="checkbox" bind:checked={compareMode} />
        <span class="toggle-track"><span class="toggle-thumb"></span></span>
        <span class="toggle-label">Compare</span>
      </label>
      <label class="toggle-control" title="Show algorithm pipeline graph">
        <input type="checkbox" bind:checked={showPipeline} />
        <span class="toggle-track"><span class="toggle-thumb"></span></span>
        <span class="toggle-label">Pipeline</span>
      </label>
    </div>
  </div>

  <!-- ── modality tabs ─────────────────────────────────────────────────── -->
  <div class="mod-tabs" role="tablist" aria-label="Modality">
    {#each (['text','image','audio'] as const) as m}
      <button role="tab" aria-selected={modality === m}
        class="mod-tab" class:active={modality === m}
        onclick={() => switchModality(m)}>
        {m === 'text' ? '⟨T⟩ Text' : m === 'image' ? '⬡ Image' : '♪ Audio'}
      </button>
    {/each}
  </div>

  {#if !compareMode}
    <!-- ── single mode ───────────────────────────────────────────────────── -->
    <div class="pg-grid">
      <!-- Left: input + controls -->
      <div class="pg-pane">
        {#if modality === 'text'}
          <label class="pane-label" for="pg-text">Input text</label>
          <textarea id="pg-text" class="pg-textarea" bind:value={textInput}
            rows={6} placeholder="Enter text to fingerprint…"></textarea>
        {:else}
          <div class="pane-label">Input {modality}</div>
          {#snippet dropZone(fileState: File | null, previewUrl: string | null, isActive: boolean, onDropFn: (e: DragEvent) => void, onLeaveFn: () => void, onDragFn: (e: DragEvent) => void, onInputId: string)}
            <div class="drop-zone" class:drag-over={isActive}
              role="button" tabindex="0"
              aria-label="Drop {modality} file here or click to browse"
              ondragover={(e) => { e.preventDefault(); dragActive = true; }}
              ondragleave={onLeaveFn} ondrop={onDropFn}
              onclick={() => document.getElementById(onInputId)?.click()}
              onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') document.getElementById(onInputId)?.click(); }}>
              {#if fileState && previewUrl}
                {#if modality === 'image'}
                  <img src={previewUrl} alt={fileState.name} class="file-preview-img" />
                {:else}
                  <div class="file-preview-audio">
                    <span class="audio-icon">♪</span>
                    <span class="file-name">{fileState.name}</span>
                    <audio controls src={previewUrl} class="audio-ctrl"></audio>
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
          {/snippet}
          {@render dropZone(file, filePreviewUrl, dragActive, onDrop, () => { dragActive = false; }, (e) => { e.preventDefault(); dragActive = true; }, 'pg-file-input')}
          <input id="pg-file-input" type="file" accept={ACCEPT[modality]} class="sr-only" onchange={onFileInput} />
        {/if}

        <div class="pane-label" id="algo-label">Algorithm</div>
        <div class="algo-grid" role="group" aria-labelledby="algo-label">
          {#each ALGORITHMS[modality] as alg}
            <button class="algo-btn" class:selected={algorithm === alg}
              class:needs-input={NEEDS_API_KEY.has(alg) || NEEDS_MODEL.has(alg)}
              onclick={() => { algorithm = alg; }}
              aria-pressed={algorithm === alg}
              title={NEEDS_API_KEY.has(alg) ? 'Requires API key' : NEEDS_MODEL.has(alg) ? 'Requires model path' : ''}>
              {ALG_LABELS[alg] ?? alg}
            </button>
          {/each}
        </div>

        {#if ALG_DESCRIPTIONS[algorithm]}
          <p class="alg-desc">{ALG_DESCRIPTIONS[algorithm]}</p>
        {/if}

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
                <label class="adv-label">Model path / ID
                  <input class="adv-input" type="text" bind:value={modelId}
                    placeholder={algorithm === 'watermark' || algorithm === 'neural'
                      ? '/models/audio.onnx' : algorithm === 'semantic'
                      ? '/models/clip.onnx' : 'sentence-transformers/all-MiniLM-L6-v2'} />
                </label>
              {/if}
              {#if NEEDS_API_KEY.has(algorithm)}
                <label class="adv-label">API key
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

        <button class="run-btn" onclick={compute}
          disabled={running || rateLimitSec > 0} aria-busy={running}>
          {running ? 'Running…' : 'Run fingerprint'}
        </button>
      </div>

      <!-- Right: result -->
      <div class="pg-pane result-pane">
        {#if isWatermark && hasResult}
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
              FALLBACK · LOCAL FNV-1a — backend not connected
              (set <code>UCFP_API_URL</code> to enable real fingerprinting)
            </p>
          {/if}
          <div class="metrics-grid">
            <div class="metric-card"><span class="metric-k">Algorithm</span><span class="metric-v">{algLabel}</span></div>
            <div class="metric-card"><span class="metric-k">Config hash</span><span class="metric-v mono">{cfgHash}</span></div>
            <div class="metric-card"><span class="metric-k">Entropy</span><span class="metric-v">{entropy}</span></div>
            <div class="metric-card"><span class="metric-k">FP size</span><span class="metric-v">{fpBytes}</span></div>
            <div class="metric-card"><span class="metric-k">Latency</span><span class="metric-v">{latencyMs}</span></div>
          </div>
        {/if}
      </div>
    </div>

  {:else}
    <!-- ── compare mode ───────────────────────────────────────────────────── -->
    <div class="pg-grid">
      <!-- Input A -->
      <div class="pg-pane">
        <div class="compare-col-label">Sample A</div>
        {#if modality === 'text'}
          <textarea class="pg-textarea" bind:value={textInput} rows={5} placeholder="Sample A text…"></textarea>
        {:else}
          <div class="drop-zone" class:drag-over={dragActive}
            role="button" tabindex="0"
            aria-label="Drop file A"
            ondragover={(e) => { e.preventDefault(); dragActive = true; }}
            ondragleave={() => { dragActive = false; }} ondrop={onDrop}
            onclick={() => document.getElementById('pg-file-input')?.click()}
            onkeydown={(e) => { if (e.key==='Enter'||e.key===' ') document.getElementById('pg-file-input')?.click(); }}>
            {#if file && filePreviewUrl}
              {#if modality === 'image'}
                <img src={filePreviewUrl} alt={file.name} class="file-preview-img" />
              {:else}
                <div class="file-preview-audio">
                  <span class="audio-icon">♪</span>
                  <span class="file-name">{file.name}</span>
                </div>
              {/if}
            {:else}
              <div class="drop-hint">
                <span class="drop-icon">{modality === 'image' ? '⬡' : '♪'}</span>
                <span>Drop A here</span>
              </div>
            {/if}
          </div>
          <input id="pg-file-input" type="file" accept={ACCEPT[modality]} class="sr-only" onchange={onFileInput} />
        {/if}
      </div>

      <!-- Input B -->
      <div class="pg-pane">
        <div class="compare-col-label">Sample B</div>
        {#if modality === 'text'}
          <textarea class="pg-textarea" bind:value={textInputB} rows={5} placeholder="Sample B text…"></textarea>
        {:else}
          <div class="drop-zone" class:drag-over={dragActiveB}
            role="button" tabindex="0"
            aria-label="Drop file B"
            ondragover={(e) => { e.preventDefault(); dragActiveB = true; }}
            ondragleave={() => { dragActiveB = false; }} ondrop={onDropB}
            onclick={() => document.getElementById('pg-file-input-b')?.click()}
            onkeydown={(e) => { if (e.key==='Enter'||e.key===' ') document.getElementById('pg-file-input-b')?.click(); }}>
            {#if fileB && fileBPreviewUrl}
              {#if modality === 'image'}
                <img src={fileBPreviewUrl} alt={fileB.name} class="file-preview-img" />
              {:else}
                <div class="file-preview-audio">
                  <span class="audio-icon">♪</span>
                  <span class="file-name">{fileB.name}</span>
                </div>
              {/if}
            {:else}
              <div class="drop-hint">
                <span class="drop-icon">{modality === 'image' ? '⬡' : '♪'}</span>
                <span>Drop B here</span>
              </div>
            {/if}
          </div>
          <input id="pg-file-input-b" type="file" accept={ACCEPT[modality]} class="sr-only" onchange={onFileInputB} />
        {/if}
      </div>
    </div>

    <!-- Shared algo selector + run -->
    <div class="compare-controls">
      <div class="pane-label" id="algo-label-cmp">Algorithm (applied to both)</div>
      <div class="algo-grid" role="group" aria-labelledby="algo-label-cmp">
        {#each ALGORITHMS[modality] as alg}
          <button class="algo-btn" class:selected={algorithm === alg}
            class:needs-input={NEEDS_API_KEY.has(alg) || NEEDS_MODEL.has(alg)}
            onclick={() => { algorithm = alg; }}
            aria-pressed={algorithm === alg}>
            {ALG_LABELS[alg] ?? alg}
          </button>
        {/each}
      </div>

      {#if ALG_DESCRIPTIONS[algorithm]}
        <p class="alg-desc">{ALG_DESCRIPTIONS[algorithm]}</p>
      {/if}

      {#if needsAdvanced}
        <details class="adv-opts">
          <summary class="adv-summary">Advanced options</summary>
          <div class="adv-body">
            {#if NEEDS_MODEL.has(algorithm)}
              <label class="adv-label">Model path / ID
                <input class="adv-input" type="text" bind:value={modelId} placeholder="/models/…" />
              </label>
            {/if}
            {#if NEEDS_API_KEY.has(algorithm)}
              <label class="adv-label">API key
                <input class="adv-input" type="password" bind:value={apiKey} placeholder="sk-…" />
              </label>
            {/if}
          </div>
        </details>
      {/if}

      {#if errorMsg}
        <p class="pg-error" role="alert">{errorMsg}</p>
      {/if}
      {#if errorMsgB}
        <p class="pg-error" role="alert">{errorMsgB}</p>
      {/if}
      {#if rateLimitSec > 0}
        <p class="pg-warn">Rate limited — retry in {rateLimitSec}s</p>
      {/if}

      <button class="run-btn" onclick={runBoth}
        disabled={running || runningB || rateLimitSec > 0}
        aria-busy={running || runningB}>
        {(running || runningB) ? 'Running…' : 'Run A + B'}
      </button>
    </div>

    <!-- Comparison results -->
    {#if hasResult || hasResultB}
      <div class="compare-results">
        <!-- Result A -->
        <div class="compare-result-pane">
          <div class="pane-label">Sample A</div>
          {#if hasResult}
            <div class="hex-grid" aria-label="Fingerprint A">
              {#each cells as cell, i}
                <div class="hex-cell"
                  class:diff-cell={diffMask && diffMask[i]}
                  style="background:{cell.color}"></div>
              {/each}
            </div>
            <div class="hex-str" title={hexStr}>{hexStr.slice(0, 48)}{hexStr.length > 48 ? '…' : ''}</div>
            {#if isLocal}
              <p class="local-notice-sm">LOCAL FNV-1a fallback</p>
            {/if}
            <div class="compare-metrics">
              <span class="cmp-metric"><span class="cmp-k">Entropy</span><span class="cmp-v">{entropy}</span></span>
              <span class="cmp-metric"><span class="cmp-k">Size</span><span class="cmp-v">{fpBytes}</span></span>
              <span class="cmp-metric"><span class="cmp-k">Latency</span><span class="cmp-v">{latencyMs}</span></span>
            </div>
          {:else}
            <div class="result-empty"><span class="empty-icon">⬡</span><span>—</span></div>
          {/if}
        </div>

        <!-- Similarity panel -->
        <div class="similarity-panel">
          <div class="sim-label">Similarity</div>
          {#if similarityPct !== null}
            <div class="sim-pct" style="color:{simColor}">{similarityPct}%</div>
            <div class="sim-bar-track">
              <div class="sim-bar-fill" style="width:{simBarWidth}%;background:{simColor}"></div>
            </div>
            <div class="sim-detail">
              <span>{hammingBits} bit{hammingBits === 1 ? '' : 's'} differ</span>
              <span>/ {totalBits} total</span>
            </div>
            <div class="sim-algo">{ALG_LABELS[algorithm] ?? algorithm}</div>
          {:else}
            <div class="sim-empty">Run both</div>
          {/if}
        </div>

        <!-- Result B -->
        <div class="compare-result-pane">
          <div class="pane-label">Sample B</div>
          {#if hasResultB}
            <div class="hex-grid" aria-label="Fingerprint B">
              {#each cellsB as cell, i}
                <div class="hex-cell"
                  class:diff-cell={diffMask && diffMask[i]}
                  style="background:{cell.color}"></div>
              {/each}
            </div>
            <div class="hex-str" title={hexStrB}>{hexStrB.slice(0, 48)}{hexStrB.length > 48 ? '…' : ''}</div>
            {#if isLocalB}
              <p class="local-notice-sm">LOCAL FNV-1a fallback</p>
            {/if}
            <div class="compare-metrics">
              <span class="cmp-metric"><span class="cmp-k">Entropy</span><span class="cmp-v">{entropyB}</span></span>
              <span class="cmp-metric"><span class="cmp-k">Size</span><span class="cmp-v">{fpBytesB}</span></span>
              <span class="cmp-metric"><span class="cmp-k">Latency</span><span class="cmp-v">{latencyMsB}</span></span>
            </div>
          {:else}
            <div class="result-empty"><span class="empty-icon">⬡</span><span>—</span></div>
          {/if}
        </div>
      </div>
    {/if}
  {/if}

  <!-- ── pipeline graph ────────────────────────────────────────────────── -->
  {#if showPipeline}
    <div class="pipeline-section">
      <div class="pane-label">How {ALG_LABELS[algorithm] ?? algorithm} works — hover any step</div>
      <FpFlow {modality} {algorithm} />
    </div>
  {/if}
</div>

<style>
  /* ── Layout ─────────────────────────────────────────────────────────── */
  .pg-wrap { display: flex; flex-direction: column; gap: 1.5rem; }

  .pg-head {
    display: flex; align-items: flex-start;
    justify-content: space-between; gap: 1rem; flex-wrap: wrap;
  }
  .pg-title { font-size: 1.25rem; font-weight: 700; margin: 0 0 0.25rem; }
  .pg-sub { margin: 0; color: var(--ink-2); font-size: 0.85rem; }

  .pg-head-controls { display: flex; gap: 0.75rem; flex-wrap: wrap; margin-top: 4px; }

  /* Toggle control */
  .toggle-control {
    display: flex; align-items: center; gap: 0.4rem;
    cursor: pointer; user-select: none;
    font-size: 0.8rem; color: var(--ink-2);
  }
  .toggle-control input { display: none; }
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
  .toggle-control:has(input:checked) .toggle-track { background: var(--accent-ink); }
  .toggle-control:has(input:checked) .toggle-thumb { transform: translateX(16px); background: var(--bg); }
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
    resize: vertical; min-height: 100px; line-height: 1.5;
  }

  /* Drop zone */
  .drop-zone {
    border: 1px dashed var(--ink); border-radius: 6px;
    background: var(--bg-2); min-height: 130px;
    display: flex; align-items: center; justify-content: center;
    cursor: pointer; transition: border-color 0.15s, background 0.15s; overflow: hidden;
  }
  .drop-zone.drag-over { border-color: var(--accent-ink); background: var(--bg); }
  .drop-zone:focus-visible { outline: 2px solid var(--accent-ink); outline-offset: 2px; }
  .drop-hint { display: flex; flex-direction: column; align-items: center; gap: 0.35rem; color: var(--ink-2); font-size: 0.82rem; }
  .drop-icon { font-size: 2rem; }
  .drop-sub { font-size: 0.72rem; opacity: 0.7; }
  .file-preview-img { max-width: 100%; max-height: 180px; object-fit: contain; display: block; }
  .file-preview-audio { display: flex; flex-direction: column; align-items: center; gap: 0.5rem; padding: 1rem; }
  .audio-icon { font-size: 2rem; }
  .file-name { font-family: var(--mono); font-size: 0.72rem; word-break: break-all; }
  .audio-ctrl { width: 100%; max-width: 260px; }
  .sr-only { position: absolute; width: 1px; height: 1px; padding: 0; margin: -1px; overflow: hidden; clip: rect(0,0,0,0); border: 0; }

  /* Algorithm grid */
  .algo-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(100px, 1fr)); gap: 0.4rem; }
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
  .algo-btn.needs-input:not(.selected)::after { content: '·'; margin-left: 2px; color: var(--ink-2); opacity: 0.6; }

  /* Algorithm description */
  .alg-desc {
    font-family: var(--mono); font-size: 0.72rem;
    line-height: 1.6; color: var(--ink-2); margin: 0;
    padding: 0.5rem 0.75rem;
    background: var(--bg-2); border-radius: 4px;
    border: 1px solid var(--ink);
    border-left: 3px solid var(--accent-ink);
  }

  /* Advanced options */
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
  .adv-required { font-size: 0.65rem; color: #b03030; background: color-mix(in srgb, #b03030 10%, transparent); padding: 1px 5px; border-radius: 3px; }
  .adv-body { padding: 0.6rem 0.65rem; display: flex; flex-direction: column; gap: 0.5rem; background: var(--bg-2); }
  .adv-label { display: flex; flex-direction: column; gap: 3px; font-family: var(--mono); font-size: 0.7rem; color: var(--ink-2); }
  .adv-input { font-family: var(--mono); font-size: 0.72rem; padding: 5px 8px; border: 1px solid var(--ink); border-radius: 3px; background: var(--bg); color: var(--ink); width: 100%; box-sizing: border-box; }
  .adv-input:focus { outline: 2px solid var(--accent-ink); outline-offset: 1px; }

  /* Error / warn */
  .pg-error { font-family: var(--mono); font-size: 0.75rem; color: #b03030; margin: 0; padding: 0.4rem 0.6rem; border: 1px solid currentColor; border-radius: 3px; background: color-mix(in srgb, #b03030 8%, transparent); }
  .pg-warn { font-family: var(--mono); font-size: 0.75rem; color: #8a6000; margin: 0; }

  /* Run button */
  .run-btn { font-family: var(--mono); font-size: 0.82rem; padding: 0.55rem 1.2rem; border: 1px solid var(--ink); background: var(--ink); color: var(--bg); border-radius: 3px; cursor: pointer; transition: opacity 0.15s; align-self: flex-start; }
  .run-btn:disabled { opacity: 0.45; cursor: not-allowed; }
  .run-btn:not(:disabled):hover { opacity: 0.85; }

  /* Result pane */
  .result-pane { padding: 0.75rem; background: var(--bg-2); border-radius: 6px; border: 1px solid var(--ink); }
  .result-empty { display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 0.4rem; min-height: 100px; color: var(--ink-2); font-size: 0.8rem; }
  .empty-icon { font-size: 1.8rem; opacity: 0.4; }

  /* Hex grid */
  .hex-grid { display: grid; grid-template-columns: repeat(16, 14px); gap: 2px; margin-bottom: 0.5rem; }
  .hex-cell { width: 14px; height: 14px; border-radius: 2px; animation: pop-in 0.2s ease both; }
  .hex-cell.diff-cell { outline: 2px solid rgba(180,30,30,0.7); outline-offset: -1px; }
  @keyframes pop-in { from { transform: scale(0); opacity: 0; } to { transform: scale(1); opacity: 1; } }
  .hex-str { font-family: var(--mono); font-size: 0.6rem; color: var(--ink-2); word-break: break-all; margin-bottom: 0.75rem; }

  /* Metrics */
  .metrics-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 0.5rem; }
  .metric-card { display: flex; flex-direction: column; gap: 2px; padding: 0.4rem 0.6rem; background: var(--bg); border-radius: 4px; border: 1px solid var(--ink); }
  .metric-card:last-child:nth-child(odd) { grid-column: 1 / -1; }
  .metric-k { font-family: var(--mono); font-size: 0.62rem; text-transform: uppercase; letter-spacing: 0.06em; color: var(--ink-2); }
  .metric-v { font-family: var(--mono); font-size: 0.8rem; color: var(--ink); font-weight: 600; }
  .mono { word-break: break-all; }

  /* Watermark result */
  .wm-result { display: flex; flex-direction: column; gap: 1rem; padding: 0.5rem 0; }
  .wm-pill { display: inline-flex; align-items: center; font-family: var(--mono); font-size: 0.82rem; font-weight: 600; padding: 7px 16px; border-radius: 20px; background: color-mix(in oklch, var(--ink) 6%, transparent); color: var(--ink-2); border: 1px solid var(--ink); align-self: flex-start; }
  .wm-pill.detected { background: color-mix(in oklch, oklch(0.58 0.18 145) 15%, transparent); color: oklch(0.38 0.15 145); border-color: oklch(0.58 0.18 145); }

  /* Local notice */
  .local-notice { font-family: var(--mono); font-size: 11px; padding: 6px 10px; border-radius: 4px; background: color-mix(in oklch, var(--accent-ink) 10%, var(--bg)); color: var(--ink-2); border: 1px solid color-mix(in oklch, var(--accent-ink) 30%, transparent); margin: 0 0 0.75rem; }
  .local-notice code { font-family: inherit; background: color-mix(in oklch, var(--ink) 8%, transparent); padding: 1px 4px; border-radius: 3px; }
  .local-notice-sm { font-family: var(--mono); font-size: 10px; color: var(--ink-2); margin: 0; opacity: 0.7; }

  /* ── Compare mode styles ─────────────────────────────────────────────── */
  .compare-col-label {
    font-family: var(--mono); font-size: 0.72rem; font-weight: 700;
    text-transform: uppercase; letter-spacing: 0.1em;
    padding: 3px 8px; border-radius: 3px;
    background: var(--ink); color: var(--bg);
    align-self: flex-start;
  }

  .compare-controls {
    display: flex; flex-direction: column; gap: 0.75rem;
    padding: 1rem; background: var(--bg-2);
    border-radius: 6px; border: 1px solid var(--ink);
  }

  .compare-results {
    display: grid;
    grid-template-columns: 1fr 160px 1fr;
    gap: 1rem; align-items: start;
  }
  @media (max-width: 800px) {
    .compare-results { grid-template-columns: 1fr; }
  }

  .compare-result-pane {
    display: flex; flex-direction: column; gap: 0.5rem;
    padding: 0.75rem; background: var(--bg-2);
    border-radius: 6px; border: 1px solid var(--ink);
  }

  /* Similarity panel */
  .similarity-panel {
    display: flex; flex-direction: column; align-items: center;
    justify-content: center; gap: 0.6rem;
    padding: 1rem 0.75rem;
    background: var(--bg-2); border-radius: 6px;
    border: 1px solid var(--ink); text-align: center;
  }
  .sim-label { font-family: var(--mono); font-size: 0.62rem; text-transform: uppercase; letter-spacing: 0.08em; color: var(--ink-2); }
  .sim-pct { font-family: var(--mono); font-size: 2rem; font-weight: 700; line-height: 1; }
  .sim-bar-track { width: 100%; height: 6px; background: var(--ink); border-radius: 3px; overflow: hidden; }
  .sim-bar-fill { height: 100%; border-radius: 3px; transition: width 0.4s ease; }
  .sim-detail { font-family: var(--mono); font-size: 0.62rem; color: var(--ink-2); }
  .sim-detail span { display: block; }
  .sim-algo { font-family: var(--mono); font-size: 0.65rem; color: var(--ink-2); background: var(--bg); padding: 2px 6px; border-radius: 2px; border: 1px solid var(--ink); }
  .sim-empty { font-family: var(--mono); font-size: 0.75rem; color: var(--ink-2); opacity: 0.5; }

  .compare-metrics {
    display: flex; flex-direction: column; gap: 3px;
  }
  .cmp-metric {
    display: flex; justify-content: space-between; align-items: baseline;
    font-family: var(--mono); font-size: 0.7rem;
  }
  .cmp-k { color: var(--ink-2); font-size: 0.62rem; text-transform: uppercase; letter-spacing: 0.05em; }
  .cmp-v { color: var(--ink); font-weight: 600; }

  /* Pipeline section */
  .pipeline-section { display: flex; flex-direction: column; gap: 0.5rem; }
</style>
