<script lang="ts">
  import { onMount } from 'svelte';
  import { fingerprintLocal, hammingDistance, bytesToHex } from '$lib/utils/fingerprint';

  type Mode = 'text' | 'json' | 'bytes' | 'file';

  /** Mirrors the upstream Rust IngestResponse (server/dto.rs). */
  interface IngestResponse {
    tenant_id: number;
    record_id: number | string;
    modality: 'text' | 'image' | 'audio';
    format_version: number;
    algorithm: string;
    config_hash: number | string;
    fingerprint_bytes: number;
    has_embedding: boolean;
  }

  type Badge =
    | { kind: 'live'; label: string }
    | { kind: 'fallback'; label: string }
    | { kind: 'rate-limited'; label: string };

  const SAMPLES: Record<Exclude<Mode, 'file'>, string> = {
    text:
      'A fingerprint is what survives a copy.\n\n' +
      'Paste anything — a paragraph, a JSON blob, a snippet of code — and UCFP collapses it into a single deterministic identifier.',
    json: JSON.stringify(
      { artifact: 'model.safetensors', bytes: 4_123_553_792, modality: 'weights' },
      null,
      2
    ),
    bytes: '0xCAFEBABE 0xDEADBEEF 0x00FF12AB 0x9933EE77 0x10101010 0xABCDEF01'
  };

  // ── state ────────────────────────────────────────────────────────────
  let mode = $state<Mode>('text');
  let value = $state<string>(SAMPLES.text);
  let display = $state<string>('…');
  let bytesLabel = $state<string>('— bytes');
  let entropy = $state<string>('—');
  let distance = $state<string>('—');
  let computeMs = $state<string>('—');
  let algorithmLabel = $state<string>('UCFP-256');
  let configHashLabel = $state<string>('—');
  let badge = $state<Badge>({ kind: 'live', label: 'LIVE · DETERMINISTIC' });
  let cells = $state<{ on: boolean; accent: boolean }[]>(
    Array.from({ length: 40 }, () => ({ on: false, accent: false }))
  );

  // file-mode state
  let file = $state<File | null>(null);
  let fileMeta = $state<string>('Drop an image or audio file');
  let filePreviewUrl = $state<string | null>(null);
  let dragActive = $state<boolean>(false);
  let rateLimitMsg = $state<string>('');
  let rateLimitSec = $state<number>(0);

  let prevBytes: Uint8Array | null = null;
  let debounceHandle: ReturnType<typeof setTimeout> | null = null;
  let inflight: AbortController | null = null;
  let countdownHandle: ReturnType<typeof setInterval> | null = null;

  // ── visualisation ─────────────────────────────────────────────────────
  function updateVisuals(bytes: Uint8Array, hex: string, displayHex: string, bytesIn: number) {
    cells = Array.from({ length: 40 }, (_, i) => {
      const byte = bytes[i % bytes.length];
      return { on: byte > 110, accent: byte > 210 };
    });
    display = displayHex;
    bytesLabel = `${bytesIn.toLocaleString()} bytes`;
    entropy = `${entropyBits(bytes).toFixed(3)} b/B`;
    if (prevBytes) {
      distance = `${hammingDistance(bytes, prevBytes)} / 256`;
    } else {
      distance = '0 / 256';
    }
    prevBytes = bytes;
    void hex; // kept for future copy-to-clipboard
  }

  function entropyBits(b: Uint8Array): number {
    const counts = new Array(256).fill(0);
    for (let i = 0; i < b.length; i++) counts[b[i]]++;
    let H = 0;
    for (let i = 0; i < 256; i++) {
      if (!counts[i]) continue;
      const p = counts[i] / b.length;
      H -= p * Math.log2(p);
    }
    return H;
  }

  function setBadgeFromResponse(latencyMs: number) {
    badge = { kind: 'live', label: `LIVE · SERVER · ${latencyMs.toFixed(0)} ms` };
  }

  function setBadgeFallback() {
    badge = { kind: 'fallback', label: 'FALLBACK · LOCAL FNV-1a' };
  }

  function setBadgeRateLimited(seconds: number) {
    badge = { kind: 'rate-limited', label: `RATE LIMITED · ${seconds}s` };
  }

  // ── compute paths ────────────────────────────────────────────────────
  function localCompute(input: string, bytesIn?: number) {
    const t0 = perfNow();
    const result = fingerprintLocal(input);
    updateVisuals(result.bytes, result.hex, result.display, bytesIn ?? result.bytesLen);
    algorithmLabel = 'fnv-1a (local)';
    configHashLabel = '—';
    computeMs = `${(perfNow() - t0).toFixed(2)} ms`;
  }

  function localComputeFromBytes(bytes: Uint8Array, label: string) {
    // Use FNV-1a-on-bytes as a deterministic visualisation seed.
    const t0 = perfNow();
    let s = '';
    // Sample first 4096 bytes for the FNV seed; keeps large files snappy.
    const slice = bytes.subarray(0, Math.min(bytes.length, 4096));
    for (let i = 0; i < slice.length; i++) s += String.fromCharCode(slice[i]);
    const result = fingerprintLocal(s + '|' + label);
    updateVisuals(result.bytes, result.hex, result.display, bytes.length);
    algorithmLabel = 'fnv-1a (local)';
    configHashLabel = '—';
    computeMs = `${(perfNow() - t0).toFixed(2)} ms`;
  }

  function applyServerVisualisation(seedInput: string | Uint8Array, bytesIn: number, resp: IngestResponse) {
    // Plan-sanctioned shortcut: use local FNV-1a result for the visual
    // (grid + hex), but show the real algorithm + byte count + config_hash
    // in the stats. Upstream returns no raw bytes today.
    const seed = typeof seedInput === 'string' ? seedInput : bytesToString(seedInput);
    const local = fingerprintLocal(seed + '|' + resp.record_id + '|' + resp.algorithm);
    updateVisuals(local.bytes, local.hex, local.display, bytesIn);
    algorithmLabel = resp.algorithm;
    configHashLabel = String(resp.config_hash);
    bytesLabel = `${resp.fingerprint_bytes.toLocaleString()} fp bytes`;
  }

  function bytesToString(b: Uint8Array): string {
    // Tiny sample only — we just need a deterministic seed for the FNV viz.
    const slice = b.subarray(0, Math.min(b.length, 1024));
    let s = '';
    for (let i = 0; i < slice.length; i++) s += String.fromCharCode(slice[i]);
    return bytesToHex(slice).slice(0, 32) + s;
  }

  function perfNow(): number {
    return typeof performance !== 'undefined' ? performance.now() : Date.now();
  }

  async function postToProxy(body: BodyInit, contentType?: string): Promise<Response> {
    if (inflight) inflight.abort();
    inflight = new AbortController();
    const headers: Record<string, string> = {};
    if (contentType) headers['content-type'] = contentType;
    return fetch('/api/fingerprint', {
      method: 'POST',
      headers,
      body,
      signal: inflight.signal
    });
  }

  function handleRateLimited(retryAfterSec: number) {
    rateLimitSec = Math.max(1, retryAfterSec);
    rateLimitMsg = `Rate limit reached. Retrying in ${rateLimitSec}s…`;
    setBadgeRateLimited(rateLimitSec);
    if (countdownHandle) clearInterval(countdownHandle);
    countdownHandle = setInterval(() => {
      rateLimitSec -= 1;
      if (rateLimitSec <= 0) {
        rateLimitMsg = '';
        if (countdownHandle) clearInterval(countdownHandle);
        countdownHandle = null;
        compute();
      } else {
        rateLimitMsg = `Rate limit reached. Retrying in ${rateLimitSec}s…`;
        setBadgeRateLimited(rateLimitSec);
      }
    }, 1000);
  }

  async function compute() {
    if (mode === 'file') {
      await computeFile();
      return;
    }
    const t0 = perfNow();
    const inputText = value;
    const bytesIn = new Blob([inputText]).size;

    let res: Response;
    try {
      res = await postToProxy(inputText, 'text/plain; charset=utf-8');
    } catch (e) {
      if ((e as DOMException).name === 'AbortError') return;
      console.warn('[demo] /api/fingerprint network error, using local FNV-1a:', e);
      setBadgeFallback();
      localCompute(inputText, bytesIn);
      return;
    }

    if (res.status === 503) {
      console.warn('[demo] /api/fingerprint not configured, using local FNV-1a');
      setBadgeFallback();
      localCompute(inputText, bytesIn);
      return;
    }
    if (res.status === 429) {
      let retry = parseInt(res.headers.get('retry-after') ?? '60', 10);
      if (!Number.isFinite(retry)) retry = 60;
      handleRateLimited(retry);
      return;
    }
    if (!res.ok) {
      console.warn('[demo] /api/fingerprint status', res.status, '— falling back');
      setBadgeFallback();
      localCompute(inputText, bytesIn);
      return;
    }

    const latency = parseFloat(res.headers.get('x-proxied-latency') ?? `${perfNow() - t0}`);
    const data = (await res.json()) as IngestResponse;
    applyServerVisualisation(inputText, bytesIn, data);
    setBadgeFromResponse(latency);
    computeMs = `${(perfNow() - t0).toFixed(2)} ms`;
  }

  async function computeFile() {
    if (!file) {
      // No file yet — show empty viz.
      cells = Array.from({ length: 40 }, () => ({ on: false, accent: false }));
      display = '…';
      bytesLabel = '— bytes';
      entropy = '—';
      distance = '—';
      computeMs = '—';
      algorithmLabel = 'awaiting file';
      configHashLabel = '—';
      return;
    }
    const t0 = perfNow();
    const bytesIn = file.size;

    // For audio files, decode via WebAudio so the upstream gets raw f32 LE
    // samples (the only format it accepts today). Fall back to raw bytes if
    // decoding fails — the user will see the upstream error in that case.
    let formBody: FormData;
    try {
      formBody = await buildFileForm(file);
    } catch (e) {
      console.warn('[demo] file prepare failed:', e);
      setBadgeFallback();
      const raw = new Uint8Array(await file.arrayBuffer());
      localComputeFromBytes(raw, file.name);
      computeMs = `${(perfNow() - t0).toFixed(2)} ms`;
      return;
    }

    let res: Response;
    try {
      res = await postToProxy(formBody);
    } catch (e) {
      if ((e as DOMException).name === 'AbortError') return;
      console.warn('[demo] /api/fingerprint network error (file):', e);
      setBadgeFallback();
      const raw = new Uint8Array(await file.arrayBuffer());
      localComputeFromBytes(raw, file.name);
      computeMs = `${(perfNow() - t0).toFixed(2)} ms`;
      return;
    }

    if (res.status === 503) {
      setBadgeFallback();
      const raw = new Uint8Array(await file.arrayBuffer());
      localComputeFromBytes(raw, file.name);
      computeMs = `${(perfNow() - t0).toFixed(2)} ms`;
      return;
    }
    if (res.status === 429) {
      let retry = parseInt(res.headers.get('retry-after') ?? '60', 10);
      if (!Number.isFinite(retry)) retry = 60;
      handleRateLimited(retry);
      return;
    }
    if (!res.ok) {
      console.warn('[demo] /api/fingerprint status (file)', res.status);
      setBadgeFallback();
      const raw = new Uint8Array(await file.arrayBuffer());
      localComputeFromBytes(raw, file.name);
      computeMs = `${(perfNow() - t0).toFixed(2)} ms`;
      return;
    }

    const latency = parseFloat(res.headers.get('x-proxied-latency') ?? `${perfNow() - t0}`);
    const data = (await res.json()) as IngestResponse;
    const seedBytes = new Uint8Array(await file.arrayBuffer());
    applyServerVisualisation(seedBytes, bytesIn, data);
    setBadgeFromResponse(latency);
    computeMs = `${(perfNow() - t0).toFixed(2)} ms`;
  }

  /**
   * Audio: decode → mono Float32Array → little-endian bytes.
   * Image: pass-through.
   */
  async function buildFileForm(f: File): Promise<FormData> {
    const fd = new FormData();
    const isAudio = (f.type || '').toLowerCase().startsWith('audio/');
    if (isAudio && typeof window !== 'undefined' && 'AudioContext' in window) {
      const arrayBuf = await f.arrayBuffer();
      const Ctor = (window.AudioContext ||
        (window as unknown as { webkitAudioContext: typeof AudioContext }).webkitAudioContext);
      const ac = new Ctor();
      try {
        const decoded = await ac.decodeAudioData(arrayBuf.slice(0));
        const channel = decoded.getChannelData(0);
        // f32 LE bytes for the upstream's `body.chunks_exact(4)` parser.
        const bytes = new Uint8Array(channel.byteLength);
        const dv = new DataView(bytes.buffer);
        for (let i = 0; i < channel.length; i++) {
          dv.setFloat32(i * 4, channel[i], true);
        }
        const wrapped = new File([bytes], f.name + '.f32le.raw', { type: 'audio/x-f32le' });
        fd.set('file', wrapped);
        fd.set('sample_rate', String(decoded.sampleRate));
        return fd;
      } finally {
        if ('close' in ac) {
          try { await (ac as AudioContext).close(); } catch { /* ignore */ }
        }
      }
    }
    fd.set('file', f);
    return fd;
  }

  // ── debounced text input ──────────────────────────────────────────────
  function onInput() {
    if (debounceHandle) clearTimeout(debounceHandle);
    debounceHandle = setTimeout(compute, 250);
  }

  function setMode(m: Mode) {
    mode = m;
    if (debounceHandle) clearTimeout(debounceHandle);
    if (m === 'file') {
      // Don't auto-compute on entering file mode — wait for a drop.
      computeFile();
    } else {
      value = SAMPLES[m];
      compute();
    }
  }

  // ── file handlers ────────────────────────────────────────────────────
  function attachFile(f: File) {
    file = f;
    fileMeta = `${f.name} · ${formatSize(f.size)} · ${f.type || 'unknown'}`;
    if (filePreviewUrl) URL.revokeObjectURL(filePreviewUrl);
    filePreviewUrl = URL.createObjectURL(f);
    compute();
  }

  function onFileChange(ev: Event) {
    const target = ev.target as HTMLInputElement;
    if (target.files && target.files[0]) attachFile(target.files[0]);
  }

  function onDrop(ev: DragEvent) {
    ev.preventDefault();
    dragActive = false;
    const f = ev.dataTransfer?.files?.[0];
    if (f) attachFile(f);
  }

  function onDragOver(ev: DragEvent) {
    ev.preventDefault();
    dragActive = true;
  }

  function onDragLeave() {
    dragActive = false;
  }

  function formatSize(n: number): string {
    if (n < 1024) return `${n} B`;
    if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
    return `${(n / (1024 * 1024)).toFixed(2)} MB`;
  }

  function isImageFile(f: File | null): boolean {
    return !!f && (f.type || '').toLowerCase().startsWith('image/');
  }
  function isAudioFile(f: File | null): boolean {
    return !!f && (f.type || '').toLowerCase().startsWith('audio/');
  }

  onMount(() => {
    compute();
    return () => {
      if (debounceHandle) clearTimeout(debounceHandle);
      if (countdownHandle) clearInterval(countdownHandle);
      if (inflight) inflight.abort();
      if (filePreviewUrl) URL.revokeObjectURL(filePreviewUrl);
    };
  });
</script>

<section class="demo" id="demo">
  <div class="pane left">
    <div class="pane-head">
      <span class="tag">◐ INPUT</span>
      <span class="dotline"></span>
      <span class="seg" role="tablist" aria-label="Input mode">
        <button class:on={mode === 'text'} onclick={() => setMode('text')} role="tab" aria-selected={mode === 'text'}>TEXT</button>
        <button class:on={mode === 'json'} onclick={() => setMode('json')} role="tab" aria-selected={mode === 'json'}>JSON</button>
        <button class:on={mode === 'bytes'} onclick={() => setMode('bytes')} role="tab" aria-selected={mode === 'bytes'}>BYTES</button>
        <button class:on={mode === 'file'} onclick={() => setMode('file')} role="tab" aria-selected={mode === 'file'}>FILE</button>
      </span>
    </div>

    {#if mode !== 'file'}
      <textarea
        class="input"
        spellcheck="false"
        bind:value
        oninput={onInput}
        aria-label="Content to fingerprint"
      ></textarea>
    {:else}
      <label
        class="input file-drop"
        class:dragging={dragActive}
        ondrop={onDrop}
        ondragover={onDragOver}
        ondragleave={onDragLeave}
        aria-label="Drop an image or audio file"
      >
        <input
          type="file"
          accept="image/*,audio/*"
          onchange={onFileChange}
          style="position:absolute; width:1px; height:1px; opacity:0; pointer-events:none;"
        />
        {#if file && isImageFile(file) && filePreviewUrl}
          <img src={filePreviewUrl} alt={file.name} class="file-preview-img" />
        {:else if file && isAudioFile(file) && filePreviewUrl}
          <audio controls src={filePreviewUrl} class="file-preview-audio"></audio>
        {:else}
          <div class="file-drop-hint">
            <div class="file-drop-icon" aria-hidden="true">⤓</div>
            <div>Drop an image or audio file</div>
            <div class="file-drop-sub">or click to choose</div>
          </div>
        {/if}
      </label>
    {/if}

    <div class="input-foot">
      <span
        class="pulse"
        class:pulse-fallback={badge.kind === 'fallback'}
        class:pulse-warn={badge.kind === 'rate-limited'}
      >{badge.label}</span>
      <span>{mode === 'file' ? fileMeta : bytesLabel}</span>
    </div>
    {#if rateLimitMsg}
      <div class="rate-notice" role="status">{rateLimitMsg}</div>
    {/if}
  </div>

  <div class="pane right">
    <div class="pane-head">
      <span class="tag">◑ FINGERPRINT</span>
      <span class="dotline"></span>
      <span style="font-family: var(--mono); font-size: 10px; letter-spacing: 0.14em;">{algorithmLabel.toUpperCase()}</span>
    </div>
    <div class="fp-stage">
      <div class="fp-grid" aria-hidden="true">
        {#each cells as cell, i (i)}
          <div class="fp-cell" class:on={cell.on} class:accent={cell.accent}></div>
        {/each}
      </div>
      <div class="fp-hash">
        <span class="pre">ucfp1·</span><span>{display}</span>
      </div>
      <div class="fp-stats">
        <div class="stat"><div class="k">Algorithm</div><div class="v">{algorithmLabel}</div></div>
        <div class="stat"><div class="k">Config hash</div><div class="v">{configHashLabel}</div></div>
        <div class="stat"><div class="k">Compute</div><div class="v">{computeMs}</div></div>
      </div>
      <div class="fp-stats">
        <div class="stat"><div class="k">Entropy</div><div class="v">{entropy}</div></div>
        <div class="stat"><div class="k">Distance</div><div class="v">{distance}</div></div>
        <div class="stat"><div class="k">Bytes</div><div class="v">{bytesLabel}</div></div>
      </div>
    </div>
  </div>
</section>

<style>
  /* ── badge variants (inline so we don't touch global app.css) ────── */
  :global(.input-foot .pulse.pulse-fallback)::before {
    background: #d97706 !important; /* amber */
    box-shadow: 0 0 0 0 rgba(217, 119, 6, 0.6) !important;
  }
  :global(.input-foot .pulse.pulse-warn)::before {
    background: #b91c1c !important; /* red */
    box-shadow: 0 0 0 0 rgba(185, 28, 28, 0.6) !important;
  }

  .file-drop {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    min-height: 200px;
    border: 1.5px dashed var(--line-strong);
    background: transparent;
    cursor: pointer;
    transition: background 0.2s ease, border-color 0.2s ease;
  }
  .file-drop.dragging {
    background: var(--bg-2);
    border-color: var(--accent-ink);
  }
  .file-drop-hint {
    text-align: center;
    font-family: var(--mono);
    font-size: 12px;
    color: var(--muted);
    letter-spacing: 0.04em;
  }
  .file-drop-icon {
    font-size: 28px;
    margin-bottom: 8px;
    color: var(--ink-2);
  }
  .file-drop-sub {
    margin-top: 6px;
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.14em;
  }
  .file-preview-img {
    max-width: 100%;
    max-height: 240px;
    object-fit: contain;
  }
  .file-preview-audio {
    width: 80%;
  }
  .rate-notice {
    margin-top: 10px;
    padding: 8px 12px;
    border: 1px solid #b91c1c33;
    background: #b91c1c0d;
    color: #7f1d1d;
    font-family: var(--mono);
    font-size: 11px;
    letter-spacing: 0.06em;
  }
</style>
