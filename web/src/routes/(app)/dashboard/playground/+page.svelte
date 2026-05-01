<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { fingerprintLocal, bytesEntropy, hammingDistance } from '$lib/utils/fingerprint';
  import FpFlow from '$components/FpFlow.svelte';
  import { createRecordHistory } from '$lib/stores/recordHistory.svelte';
  import { buildResampledAudioForm, AUDIO_RATES_BY_ALG } from '$lib/utils/audioResample';
  import EmbeddingBars from '$components/charts/EmbeddingBars.svelte';
  import ByteHistogram from '$components/charts/ByteHistogram.svelte';
  import BitDiffStrip from '$components/charts/BitDiffStrip.svelte';
  import AlgorithmView from '$components/charts/AlgorithmView.svelte';
  import { hasAlgorithmView } from '$components/charts/algorithmView';
  import TuningForm from '$lib/components/playground/TuningForm.svelte';
  import PipelineInspector from '$lib/components/playground/PipelineInspector.svelte';
  import type { RecordHistoryEntry } from '$lib/types/api';

  const history = createRecordHistory();

  // Algorithms that produce a dense embedding vector (eligible for "Find similar").
  const EMBEDDING_ALGS = new Set([
    'semantic-openai','semantic-voyage','semantic-cohere','semantic-local',
    'semantic','neural'
  ]);

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
  const AUDIO_RATES = AUDIO_RATES_BY_ALG;
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

  // ── per-algorithm tunables (all optional; missing = upstream default) ────
  // Text MinHash / SimHash / LSH / TLSH
  let optK         = $state<number | null>(null);            // shingle width
  let optH         = $state<number | null>(null);            // MinHash slots
  let optTokenizer = $state<'word'|'grapheme'|'cjk-jp'|'cjk-ko'|''>('');
  // Text preprocessing pass (HTML/Markdown/PDF) — short-circuits to dedicated endpoint.
  let optPreprocess = $state<''|'html'|'markdown'|'pdf'>('');
  // Audio Wang knobs
  let optFanOut         = $state<number | null>(null);
  let optPeaksPerSec    = $state<number | null>(null);
  let optTargetZoneT    = $state<number | null>(null);
  let optTargetZoneF    = $state<number | null>(null);
  let optMinAnchorMagDb = $state<number | null>(null);
  // Image preprocess
  let optMaxDimension  = $state<number | null>(null);
  let optMinDimension  = $state<number | null>(null);
  let optMaxInputBytes = $state<number | null>(null);

  // Extra knobs sourced from the GET /v1/algorithms manifest. The
  // primary controls above remain hardcoded for back-compat; everything
  // *else* (canonicalizer flags, Panako/Haitsma/Neural/Watermark
  // configs) flows through this map and is splatted onto the request
  // URL below. Adding a knob upstream now only requires updating the
  // manifest in src/server/algorithms_manifest.rs — no UI edit needed.
  let extraOpts = $state<Record<string, unknown>>({});

  // ── live-tune (text + image) ─────────────────────────────────────────────
  // After the first successful compute we upload the current input to
  // /api/inputs and remember the returned input_id. Subsequent slider
  // movements fire a debounced re-fingerprint that quotes input_id —
  // the bytes never traverse the wire again. Audio live-tune is
  // deferred (it would need the same resampling as the upload path).
  let cachedInputIdA = $state<number | null>(null);
  let liveTuneEnabled = $state(true);              // user toggle
  let liveTuneAbortA: AbortController | null = null;
  let liveTuneTimer:  ReturnType<typeof setTimeout> | null = null;
  let liveTuneInflight = $state(false);            // for the spinner badge
  // Snapshot of "every opt that affects the upstream URL" at the last
  // successful compute. The live-tune $effect compares the current
  // opts-key against this and skips when they match — prevents an
  // immediate duplicate retune right after a manual Run click.
  let lastComputedOptsKey = $state<string>('');
  function optsKey(): string {
    return JSON.stringify({
      algorithm, modality, modelId, apiKey,
      extraOpts,
      optK, optH, optTokenizer, optPreprocess,
      optFanOut, optPeaksPerSec, optTargetZoneT, optTargetZoneF, optMinAnchorMagDb,
      optMaxDimension, optMinDimension, optMaxInputBytes,
    });
  }

  // Last response embedding (when ?return_embedding=1 was sent and the
  // algorithm produced one). Used by the "Find similar" handoff.
  let lastEmbedding = $state<number[] | null>(null);
  let lastRecordId  = $state<string | null>(null);
  let lastTenantId  = $state<number>(0);

  // Controlled open state for the three <details> panels — sidesteps the
  // sporadic Chromium issue where a flex/grid summary swallows the click
  // before the native toggle fires.
  let advancedOpen = $state(false);
  let tuningOpen   = $state(false);
  let advancedOpenB = $state(false);

  // ── result A ──────────────────────────────────────────────────────────────
  let cells      = $state<{ color: string }[]>([]);
  let hexBytesA  = $state<Uint8Array | null>(null);
  // Full fingerprint bytes (not truncated). Used by AlgorithmView for
  // structure-aware visualisations that need the whole buffer.
  let fullBytesA = $state<Uint8Array | null>(null);
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
  let fullBytesB = $state<Uint8Array | null>(null);
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
    fullBytesA = null; fullBytesB = null;
  }

  // ── FormData builder (audio resampling delegates to shared helper) ───────
  async function buildFileForm(f: File, alg: string): Promise<FormData> {
    const isAudio = (f.type || '').toLowerCase().startsWith('audio/');
    if (isAudio) {
      const built = await buildResampledAudioForm(f, alg);
      return built.form;
    }
    const fd = new FormData();
    fd.set('file', f);
    return fd;
  }

  // ── hex helpers ──────────────────────────────────────────────────────────
  function hexToBytes(hex: string, maxBytes = 128): Uint8Array {
    const byteCount = Math.min(Math.floor(hex.length / 2), maxBytes);
    const b = new Uint8Array(byteCount);
    for (let i = 0; i < byteCount; i++) b[i] = parseInt(hex.slice(i * 2, i * 2 + 2), 16);
    return b;
  }

  // Copy-to-clipboard with a brief on-screen confirmation. Shared
  // across every hex-string render in the playground.
  let copyToast = $state<string | null>(null);
  let copyToastTimer: ReturnType<typeof setTimeout> | null = null;
  async function copyHex(value: string, what = 'fingerprint hex'): Promise<void> {
    if (!value) return;
    try {
      await navigator.clipboard.writeText(value);
      copyToast = `${what} copied (${value.length} chars)`;
    } catch (e) {
      copyToast = `Copy failed: ${(e as Error).message}`;
    }
    if (copyToastTimer != null) clearTimeout(copyToastTimer);
    copyToastTimer = setTimeout(() => { copyToast = null; }, 1600);
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
    /** Full bytes (uncapped) for algorithm-aware visualisations. */
    fullBytes: Uint8Array | null;
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

    // Per-algorithm tunables — only forward fields meaningful for the
    // current modality so we don't push noise upstream.
    function appendNum(name: string, v: number | null): void {
      if (v != null && Number.isFinite(v)) url += `&${name}=${v}`;
    }
    if (modality === 'text') {
      appendNum('k', optK); appendNum('h', optH);
      if (optTokenizer) url += `&tokenizer=${optTokenizer}`;
      if (optPreprocess) url += `&preprocess=${optPreprocess}`;
    } else if (modality === 'audio' && algorithm === 'wang') {
      appendNum('fan_out', optFanOut);
      appendNum('peaks_per_sec', optPeaksPerSec);
      appendNum('target_zone_t', optTargetZoneT);
      appendNum('target_zone_f', optTargetZoneF);
      appendNum('min_anchor_mag_db', optMinAnchorMagDb);
    } else if (modality === 'image') {
      appendNum('max_dimension', optMaxDimension);
      appendNum('min_dimension', optMinDimension);
      appendNum('max_input_bytes', optMaxInputBytes);
    }
    // Manifest-driven extras (Panako/Haitsma/Neural/Watermark configs,
    // text canonicalizer flags). The /api/fingerprint proxy validates
    // each key against its allowlist before forwarding upstream, so we
    // can splat freely here.
    for (const [k, v] of Object.entries(extraOpts)) {
      if (v == null || v === '') continue;
      url += `&${encodeURIComponent(k)}=${encodeURIComponent(String(v))}`;
    }
    // Request the embedding back when the algorithm produces one — the
    // search "Find similar" path needs it client-side.
    if (EMBEDDING_ALGS.has(algorithm)) url += `&return_embedding=1`;

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
        fullBytes: local.bytes,
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

    // Stash record_id + tenant_id + embedding from the *just-completed*
    // call so the "Save to records" and "Find similar" buttons have
    // something to use.
    lastRecordId = data.record_id != null ? String(data.record_id) : null;
    lastTenantId = typeof data.tenant_id === 'number' ? data.tenant_id : 0;
    lastEmbedding = Array.isArray(data.embedding) ? (data.embedding as number[]) : null;

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
    let fullBytes: Uint8Array;
    let displayHex: string;
    if (typeof data.fingerprint_hex === 'string' && data.fingerprint_hex.length > 0) {
      displayBytes = hexToBytes(data.fingerprint_hex);
      fullBytes    = hexToBytes(data.fingerprint_hex, Number.MAX_SAFE_INTEGER);
      displayHex   = data.fingerprint_hex;
    } else {
      const local = fingerprintLocal(seedFallback + '|' + String(data.algorithm ?? algorithm));
      displayBytes = local.bytes;
      fullBytes    = local.bytes;
      displayHex   = local.hex;
    }

    return {
      kind: 'ok',
      cells: bytesToCells(displayBytes),
      hexBytes: displayBytes,
      fullBytes,
      hexStr: displayHex,
      algLabel: String(data.algorithm ?? algorithm),
      cfgHash: data.config_hash != null ? `0x${Number(data.config_hash).toString(16)}` : '—',
      fpBytes: data.fingerprint_bytes != null ? `${data.fingerprint_bytes} bytes` : '—',
      entropy: `${bytesEntropy(displayBytes).toFixed(2)} bits`,
      latencyMs: `${elapsed} ms`,
      isLocal: false,
    };
  }

  // ── save-to-records / find-similar ────────────────────────────────────────
  let saveToast = $state<string | null>(null);
  function saveToRecords(): void {
    if (!lastRecordId || !hasResult || isWatermark) return;
    const labelSeed = modality === 'text'
      ? textInput.trim().slice(0, 60) || 'untitled text'
      : (file?.name ?? 'untitled file');
    const entry: RecordHistoryEntry = {
      tenantId: lastTenantId,
      recordId: lastRecordId,
      label: labelSeed,
      modality,
      algorithm,
      hasEmbedding: lastEmbedding != null,
      fingerprintHex: hexStr.slice(0, 64),
      createdAt: Math.floor(Date.now() / 1000)
    };
    history.add(entry);
    saveToast = 'Saved to records';
    setTimeout(() => { saveToast = null; }, 1800);
  }

  function findSimilar(): void {
    if (!lastEmbedding) return;
    try {
      sessionStorage.setItem('ucfp:search:handoff', JSON.stringify({
        modality, algorithm, vector: lastEmbedding,
        sourceLabel: modality === 'text'
          ? textInput.trim().slice(0, 60)
          : (file?.name ?? '')
      }));
    } catch { /* quota — proceed anyway */ }
    void goto(`/dashboard/search?modality=${modality}&algorithm=${algorithm}`);
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
      fullBytesA = result.fullBytes;
      hexStr     = result.hexStr;
      algLabel   = result.algLabel;
      cfgHash    = result.cfgHash;
      fpBytes    = result.fpBytes;
      entropy    = result.entropy;
      latencyMs  = result.latencyMs;
      isLocal    = result.isLocal;
      hasResult  = true;
      // Record opts snapshot so the live-tune effect doesn't fire an
      // immediate redundant retune right after this manual compute.
      lastComputedOptsKey = optsKey();
    } catch (e) {
      errorMsg = `Network error: ${(e as Error).message}`;
    } finally {
      running = false;
    }
  }

  // ── live-tune helpers ─────────────────────────────────────────────────────
  // Build the URL for a fingerprint request driven by a cached input_id.
  // Mirrors the URL-build path inside `runFingerprint` but inserts
  // `&input_id=N` and skips the body — bytes are fetched from the
  // process-local cache upstream.
  function buildLiveTuneUrl(inputId: number): string {
    let url = `/api/fingerprint?algorithm=${encodeURIComponent(algorithm)}&input_id=${inputId}`;
    if (modelId.trim()) url += `&model_id=${encodeURIComponent(modelId.trim())}`;
    if (apiKey.trim())  url += `&api_key=${encodeURIComponent(apiKey.trim())}`;
    function appendNum(name: string, v: number | null): void {
      if (v != null && Number.isFinite(v)) url += `&${name}=${v}`;
    }
    if (modality === 'text') {
      appendNum('k', optK); appendNum('h', optH);
      if (optTokenizer) url += `&tokenizer=${optTokenizer}`;
      if (optPreprocess) url += `&preprocess=${optPreprocess}`;
    } else if (modality === 'image') {
      appendNum('max_dimension', optMaxDimension);
      appendNum('min_dimension', optMinDimension);
      appendNum('max_input_bytes', optMaxInputBytes);
    }
    for (const [k, v] of Object.entries(extraOpts)) {
      if (v == null || v === '') continue;
      url += `&${encodeURIComponent(k)}=${encodeURIComponent(String(v))}`;
    }
    if (EMBEDDING_ALGS.has(algorithm)) url += `&return_embedding=1`;
    return url;
  }

  // Cache the current input. Text + image only — audio defers to the
  // existing resampling path. Failures surface as `errorMsg` and
  // disable live-tune (rather than looping silently on every slider tick).
  async function ensureCachedInputIdA(): Promise<number | null> {
    if (cachedInputIdA != null) return cachedInputIdA;
    if (modality === 'audio') return null;
    let body: BodyInit;
    let contentType: string;
    if (modality === 'text') {
      if (!textInput) return null;
      body = textInput;
      contentType = 'text/plain; charset=utf-8';
    } else {
      // image
      if (!file) return null;
      body = await file.arrayBuffer();
      contentType = 'application/octet-stream';
    }
    const sp = new URLSearchParams({ modality });
    let res: Response;
    try {
      res = await fetch(`/api/inputs?${sp.toString()}`, {
        method: 'POST',
        headers: { 'content-type': contentType },
        body,
      });
    } catch (e) {
      errorMsg = `Live-tune disabled — couldn't reach the input cache: ${(e as Error).message}`;
      liveTuneEnabled = false;
      return null;
    }
    if (!res.ok) {
      const detail = await res.text().catch(() => String(res.status));
      errorMsg = `Live-tune disabled — input cache returned ${res.status}: ${detail.slice(0, 200)}`;
      liveTuneEnabled = false;
      return null;
    }
    let parsed: unknown;
    try { parsed = await res.json(); } catch (e) {
      errorMsg = `Live-tune disabled — input cache replied with invalid JSON: ${(e as Error).message}`;
      liveTuneEnabled = false;
      return null;
    }
    const id = (parsed as { input_id?: unknown }).input_id;
    if (typeof id !== 'number' || !Number.isFinite(id)) {
      errorMsg = 'Live-tune disabled — input cache reply missing `input_id`.';
      liveTuneEnabled = false;
      return null;
    }
    cachedInputIdA = id;
    return id;
  }

  // Apply the bytes returned by a live-tune fetch to the result panel.
  async function applyLiveTuneResponse(res: Response, t0: number) {
    const elapsed = Math.round(performance.now() - t0);
    if (!res.ok) return;
    const data = await res.json() as Record<string, unknown>;
    if (!data.fingerprint_hex || typeof data.fingerprint_hex !== 'string') return;
    const hex = data.fingerprint_hex;
    const fullBytes = new Uint8Array(hex.length / 2);
    for (let i = 0; i < fullBytes.length; i++) fullBytes[i] = parseInt(hex.slice(i * 2, i * 2 + 2), 16);
    const trimBytes = fullBytes.subarray(0, Math.min(fullBytes.length, 128));
    fullBytesA = fullBytes;
    hexBytesA  = trimBytes;
    hexStr     = hex;
    fpBytes    = `${data.fingerprint_bytes ?? fullBytes.length} bytes`;
    entropy    = `${bytesEntropy(trimBytes).toFixed(2)} bits`;
    latencyMs  = `${elapsed} ms`;
    cells      = bytesToCells(trimBytes);
    cfgHash    = data.config_hash != null ? `0x${(data.config_hash as number).toString(16)}` : cfgHash;
    if (typeof data.algorithm === 'string') {
      algLabel = `${ALG_LABELS[data.algorithm as string] ?? data.algorithm} · live-tune`;
    }
    if (Array.isArray(data.embedding)) lastEmbedding = data.embedding as number[];
  }

  async function retuneA(): Promise<void> {
    if (modality === 'audio') return;             // not supported
    if (running) return;                           // primary compute in flight
    const id = await ensureCachedInputIdA();
    if (id == null) return;
    if (liveTuneAbortA) liveTuneAbortA.abort();
    liveTuneAbortA = new AbortController();
    liveTuneInflight = true;
    const t0 = performance.now();
    try {
      const res = await fetch(buildLiveTuneUrl(id), {
        method: 'POST',
        body: '',
        signal: liveTuneAbortA.signal,
      });
      await applyLiveTuneResponse(res, t0);
    } catch (e) {
      // Aborts are expected when newer ticks supersede; ignore.
      if ((e as { name?: string }).name !== 'AbortError') {
        errorMsg = `Live-tune error: ${(e as Error).message}`;
      }
    } finally {
      liveTuneInflight = false;
    }
  }

  // Invalidate cached input_id whenever the underlying bytes change.
  $effect(() => { void textInput; cachedInputIdA = null; });
  $effect(() => { void file;      cachedInputIdA = null; });
  $effect(() => { void modality;  cachedInputIdA = null; });

  // Watch `extraOpts` (and all primary tunables that affect the
  // request) and trigger a debounced re-fingerprint via input_id.
  // Only fires after the first manual compute has populated `hasResult`,
  // and only when live-tune is enabled.
  //
  // Critical: skip when the opts haven't actually changed since the
  // last compute / retune. Without this guard, every manual Run flips
  // `hasResult` false→true and re-runs this effect, triggering an
  // immediate redundant retune with the same opts.
  $effect(() => {
    if (!hasResult || !liveTuneEnabled) return;
    if (modality === 'audio') return;
    const k = optsKey();
    if (k === lastComputedOptsKey) return;
    if (liveTuneTimer != null) clearTimeout(liveTuneTimer);
    liveTuneTimer = setTimeout(() => {
      lastComputedOptsKey = k;          // commit before fetch — debounce fold
      void retuneA();
    }, 200);
  });

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
      fullBytesB = result.fullBytes;
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
        fullBytesA = ra.fullBytes;
        hexStr = ra.hexStr; algLabel = ra.algLabel; cfgHash = ra.cfgHash;
        fpBytes = ra.fpBytes; entropy = ra.entropy; latencyMs = ra.latencyMs;
        isLocal = ra.isLocal; hasResult = true;
      }
      // apply B
      if (rb.kind === 'error') errorMsgB = rb.msg;
      if (rb.kind === 'ok') {
        cellsB = rb.cells; hexBytesB = rb.hexBytes;
        fullBytesB = rb.fullBytes;
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
          <details class="adv-opts" bind:open={advancedOpen}>
            <summary class="adv-summary"
              onclick={(e) => { e.preventDefault(); advancedOpen = !advancedOpen; }}>
              <span class="adv-summary-inner">Advanced options</span>
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

        <!-- ── Algorithm tuning (per-modality knobs that map to upstream ── -->
        <!-- ── DTO query params; missing values fall back to defaults).  ── -->
        <details class="adv-opts" bind:open={tuningOpen}>
          <summary class="adv-summary"
            onclick={(e) => { e.preventDefault(); tuningOpen = !tuningOpen; }}>
            <span class="adv-summary-inner">Algorithm tuning</span>
          </summary>
          <div class="adv-body">
            {#if modality === 'text'}
              <div class="adv-row">
                <label class="adv-label">Shingle width (k)
                  <input class="adv-input" type="number" min="1" max="16"
                    bind:value={optK} placeholder="default 5" />
                </label>
                <label class="adv-label">MinHash slots (h)
                  <select class="adv-input" bind:value={optH}>
                    <option value={null}>default (128)</option>
                    <option value={32}>32</option>
                    <option value={64}>64</option>
                    <option value={128}>128</option>
                    <option value={256}>256</option>
                    <option value={512}>512</option>
                  </select>
                </label>
              </div>
              <label class="adv-label">Tokenizer
                <select class="adv-input" bind:value={optTokenizer}>
                  <option value="">default (word)</option>
                  <option value="word">word (UAX #29)</option>
                  <option value="grapheme">grapheme cluster</option>
                  <option value="cjk-jp">CJK Japanese (Lindera + IPADIC)</option>
                  <option value="cjk-ko">CJK Korean (Lindera + ko-dic)</option>
                </select>
              </label>
              <label class="adv-label">Preprocess pass
                <select class="adv-input" bind:value={optPreprocess}>
                  <option value="">none — fingerprint raw text</option>
                  <option value="html">HTML → plain text → MinHash</option>
                  <option value="markdown">Markdown → plain text → MinHash</option>
                  <option value="pdf">PDF → text → MinHash (drop a .pdf above)</option>
                </select>
              </label>
            {:else if modality === 'image'}
              <div class="adv-row">
                <label class="adv-label">Max dimension (px)
                  <input class="adv-input" type="number" min="32" max="8192"
                    bind:value={optMaxDimension} placeholder="upstream default" />
                </label>
                <label class="adv-label">Min dimension (px)
                  <input class="adv-input" type="number" min="1"
                    bind:value={optMinDimension} placeholder="upstream default" />
                </label>
              </div>
              <label class="adv-label">Max input bytes
                <input class="adv-input" type="number" min="1024"
                  bind:value={optMaxInputBytes} placeholder="upstream default" />
              </label>
            {:else if modality === 'audio' && algorithm === 'wang'}
              <div class="adv-row">
                <label class="adv-label">Fan out
                  <input class="adv-input" type="number" min="1" max="64"
                    bind:value={optFanOut} placeholder="upstream default" />
                </label>
                <label class="adv-label">Peaks / sec
                  <input class="adv-input" type="number" min="1" max="200"
                    bind:value={optPeaksPerSec} placeholder="upstream default" />
                </label>
              </div>
              <div class="adv-row">
                <label class="adv-label">Target zone Δt
                  <input class="adv-input" type="number" min="1"
                    bind:value={optTargetZoneT} placeholder="upstream default" />
                </label>
                <label class="adv-label">Target zone Δf
                  <input class="adv-input" type="number" min="1"
                    bind:value={optTargetZoneF} placeholder="upstream default" />
                </label>
              </div>
              <label class="adv-label">Min anchor magnitude (dB)
                <input class="adv-input" type="number"
                  bind:value={optMinAnchorMagDb} placeholder="upstream default" />
              </label>
            {/if}
            <!-- Manifest-driven knobs (canonicalizer flags, Panako/Haitsma/
                 Neural/Watermark configs). The component fetches
                 /api/algorithms once and renders one input per Tunable
                 the upstream binary advertises that the primary controls
                 above don't already cover. -->
            <div class="manifest-extras">
              <TuningForm {modality} {algorithm} bind:opts={extraOpts} />
              {#if hasResult && modality !== 'audio'}
                <label class="live-toggle" title="Re-fingerprint as soon as a knob changes (debounced 200ms). Uses the cached input — no re-upload.">
                  <input type="checkbox" bind:checked={liveTuneEnabled} />
                  <span>Live-tune</span>
                  {#if liveTuneInflight}<span class="live-dot" aria-label="recomputing"></span>{/if}
                </label>
              {/if}
            </div>
          </div>
        </details>

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
            <button type="button" class="hex-str copyable"
              title="Click to copy full hex ({hexStr.length} chars)"
              onclick={() => copyHex(hexStr)}>
              {hexStr.slice(0, 64)}{hexStr.length > 64 ? '…' : ''}
            </button>
            {#if hexBytesA && hexBytesA.length > 0}
              <div class="viz-section">
                <ByteHistogram bytes={hexBytesA} height={42} />
              </div>
            {/if}
            {#if fullBytesA && !isLocal && hasAlgorithmView(algLabel, fullBytesA.length)}
              <div class="viz-section">
                <div class="viz-label">Algorithm structure · {algLabel}</div>
                <AlgorithmView algorithm={algLabel} bytes={fullBytesA} />
              </div>
            {/if}
            {#if lastEmbedding}
              <div class="viz-section">
                <div class="viz-label">Embedding · dense vector</div>
                <EmbeddingBars vector={lastEmbedding} maxBars={128} height={64} />
              </div>
            {/if}
            <!-- Pipeline inspector — text, image, and audio all wired.
                 Audio decodes the dropped file through the existing
                 WebAudio resampler when the user clicks Inspect. -->
            <div class="viz-section">
              <PipelineInspector
                {modality}
                text={textInput}
                {file}
                inputId={cachedInputIdA}
                opts={{
                  k: optK, h: optH, tokenizer: optTokenizer, preprocess: optPreprocess,
                  max_dimension: optMaxDimension, min_dimension: optMinDimension, max_input_bytes: optMaxInputBytes,
                  ...extraOpts,
                }} />
            </div>
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
          {#if hasResult && !isWatermark && lastRecordId && !isLocal}
            <div class="result-actions">
              <button class="action-btn" onclick={saveToRecords}>+ Save to records</button>
              <button class="action-btn"
                disabled={!lastEmbedding}
                title={lastEmbedding ? 'Find similar records via vector kNN' : 'Pick a semantic algorithm to enable similarity search'}
                onclick={findSimilar}>↗ Find similar</button>
              {#if saveToast}<span class="save-toast" role="status">{saveToast}</span>{/if}
            </div>
          {/if}
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
        <details class="adv-opts" bind:open={advancedOpenB}>
          <summary class="adv-summary"
            onclick={(e) => { e.preventDefault(); advancedOpenB = !advancedOpenB; }}>
            <span class="adv-summary-inner">Advanced options</span>
          </summary>
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
            <button type="button" class="hex-str copyable"
              title="Click to copy full hex ({hexStr.length} chars)"
              onclick={() => copyHex(hexStr, 'A hex')}>
              {hexStr.slice(0, 48)}{hexStr.length > 48 ? '…' : ''}
            </button>
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
            <button type="button" class="hex-str copyable"
              title="Click to copy full hex ({hexStrB.length} chars)"
              onclick={() => copyHex(hexStrB, 'B hex')}>
              {hexStrB.slice(0, 48)}{hexStrB.length > 48 ? '…' : ''}
            </button>
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

      <!-- bit-level XOR strip across the pair (richer than the byte diff). -->
      {#if hexBytesA && hexBytesB}
        <div class="bit-diff-section">
          <div class="pane-label">Bit-level diff (XOR · A ⊕ B, first {Math.min(hexBytesA.length, hexBytesB.length, 64)} bytes)</div>
          <BitDiffStrip a={hexBytesA} b={hexBytesB} maxBytes={64} />
        </div>
      {/if}

      <!-- Algorithm-aware structural diff (where the layout permits). -->
      {#if fullBytesA && fullBytesB && !isLocal && !isLocalB && hasAlgorithmView(algLabel, fullBytesA.length) && fullBytesA.length === fullBytesB.length}
        <div class="bit-diff-section">
          <div class="pane-label">Structural diff · {algLabel}</div>
          <AlgorithmView algorithm={algLabel} bytes={fullBytesA} diffAgainst={fullBytesB} />
        </div>
      {/if}
    {/if}
  {/if}

  <!-- ── pipeline graph ────────────────────────────────────────────────── -->
  {#if showPipeline}
    <div class="pipeline-section">
      <div class="pane-label">How {ALG_LABELS[algorithm] ?? algorithm} works — hover any step</div>
      <FpFlow {modality} {algorithm} />
    </div>
  {/if}

  {#if copyToast}
    <div class="copy-toast" role="status" aria-live="polite">{copyToast}</div>
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

  /* Advanced options. Open state is controlled by Svelte (`bind:open`)
     with an explicit onclick that preventDefaults the native toggle —
     this guarantees the panel responds across browsers regardless of
     summary display quirks. */
  .adv-opts { border: 1px solid var(--ink); border-radius: 4px; overflow: hidden; }
  .adv-summary {
    font-family: var(--mono); font-size: 0.72rem; cursor: pointer;
    padding: 0.4rem 0.65rem; color: var(--ink-2);
    list-style: none;
    display: flex; align-items: center; gap: 0.5rem;
  }
  .adv-summary::-webkit-details-marker { display: none; }
  .adv-summary::marker { content: ''; }
  .adv-summary-inner {
    display: inline-flex; align-items: center; gap: 0.5rem;
  }
  .adv-summary-inner::before { content: '▶'; font-size: 0.55rem; transition: transform 0.15s; display: inline-block; }
  /* Use :global() so Svelte's CSS scoper doesn't strip the `details[open]`
     prefix — without it the chevron would render rotated regardless of
     state because the scoper can't see the dynamic open attribute. */
  :global(details[open]) .adv-summary-inner::before { transform: rotate(90deg); }
  .adv-required { font-size: 0.65rem; color: #b03030; background: color-mix(in srgb, #b03030 10%, transparent); padding: 1px 5px; border-radius: 3px; }
  .adv-body { padding: 0.6rem 0.65rem; display: flex; flex-direction: column; gap: 0.5rem; background: var(--bg-2); }
  .adv-label { display: flex; flex-direction: column; gap: 3px; font-family: var(--mono); font-size: 0.7rem; color: var(--ink-2); }
  .adv-input { font-family: var(--mono); font-size: 0.72rem; padding: 5px 8px; border: 1px solid var(--ink); border-radius: 3px; background: var(--bg); color: var(--ink); width: 100%; box-sizing: border-box; }
  .adv-input:focus { outline: 2px solid var(--accent-ink); outline-offset: 1px; }
  .adv-row { display: grid; grid-template-columns: 1fr 1fr; gap: 0.5rem; }
  .manifest-extras {
    margin-top: 0.5rem;
    padding-top: 0.5rem;
    border-top: 1px dashed var(--border, rgba(255,255,255,0.06));
  }
  .live-toggle {
    display: inline-flex;
    align-items: center;
    gap: 0.45rem;
    margin-top: 0.5rem;
    font-size: 0.78rem;
    color: var(--ink-2, #888);
    cursor: pointer;
  }
  .live-toggle input { margin: 0; cursor: pointer; }
  .live-dot {
    display: inline-block;
    width: 0.5rem;
    height: 0.5rem;
    border-radius: 999px;
    background: oklch(0.7 0.18 150);
    box-shadow: 0 0 0 0 oklch(0.7 0.18 150);
    animation: live-pulse 1.2s infinite ease-out;
  }
  @keyframes live-pulse {
    0%   { box-shadow: 0 0 0 0    oklch(0.7 0.18 150 / 0.55); }
    70%  { box-shadow: 0 0 0 6px  oklch(0.7 0.18 150 / 0); }
    100% { box-shadow: 0 0 0 0    oklch(0.7 0.18 150 / 0); }
  }
  /* Hex-string copy affordance — click to copy the full fingerprint hex. */
  .hex-str.copyable {
    appearance: none;
    border: 1px dashed transparent;
    background: transparent;
    color: inherit;
    text-align: left;
    cursor: pointer;
    padding: 0.05rem 0.25rem;
    border-radius: 0.25rem;
    transition: background 0.12s, border-color 0.12s;
  }
  .hex-str.copyable:hover  { background: var(--bg-2, rgba(255,255,255,0.04)); }
  .hex-str.copyable:focus  { outline: 2px solid var(--accent, #6ad); outline-offset: 1px; }
  .hex-str.copyable:active { background: var(--bg-3, rgba(255,255,255,0.08)); }
  /* Floating toast surfaced after a successful (or failed) copy. */
  .copy-toast {
    position: fixed;
    bottom: 1.25rem;
    left: 50%;
    transform: translateX(-50%);
    background: oklch(0.18 0.02 240 / 0.94);
    color: oklch(0.95 0.02 240);
    padding: 0.5rem 0.85rem;
    border-radius: 0.5rem;
    font-family: var(--mono, monospace);
    font-size: 0.78rem;
    box-shadow: 0 6px 20px oklch(0 0 0 / 0.25);
    z-index: 9999;
    pointer-events: none;
    animation: toast-in 160ms ease-out;
  }
  @keyframes toast-in {
    from { opacity: 0; transform: translate(-50%, 6px); }
    to   { opacity: 1; transform: translate(-50%, 0); }
  }

  .result-actions {
    display: flex; gap: 0.5rem; align-items: center; flex-wrap: wrap;
    margin-top: 0.75rem;
  }
  .action-btn {
    font-family: var(--mono); font-size: 0.72rem;
    padding: 0.35rem 0.7rem; border: 1px solid var(--ink);
    background: transparent; color: var(--ink); border-radius: 3px;
    cursor: pointer; transition: background 0.1s, opacity 0.1s;
  }
  .action-btn:not(:disabled):hover { background: var(--bg-2); }
  .action-btn:disabled { opacity: 0.4; cursor: not-allowed; }
  .save-toast {
    font-family: var(--mono); font-size: 0.7rem; color: var(--ink-2);
    padding: 0.25rem 0.5rem; background: var(--bg-2); border-radius: 3px;
  }

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

  /* Inline visualization sections (embedding / histogram / bit-diff). */
  .viz-section { display: flex; flex-direction: column; gap: 0.35rem; margin-top: 0.6rem; }
  .viz-label {
    font-family: var(--mono); font-size: 0.62rem;
    text-transform: uppercase; letter-spacing: 0.06em; color: var(--ink-2);
  }
  .bit-diff-section {
    display: flex; flex-direction: column; gap: 0.4rem;
    padding: 0.75rem; background: var(--bg-2);
    border: 1px solid var(--ink); border-radius: 6px;
    margin-top: 0.5rem;
  }
</style>
