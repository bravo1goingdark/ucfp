<!--
  PipelineInspector — surfaces every intermediate stage of the
  fingerprinting pipeline so users can see what each step produced.

  Text:  raw → canonicalized → tokens → shingles → final hash.
  Image: original → 32×32 grayscale → 8×8 grayscale + AHash mean → final hex.
  Audio: amplitude envelope → log-magnitude spectrogram → picked peaks
         → final Wang fingerprint hex.

  Triggered explicitly via the "Inspect pipeline" button — never on
  every keystroke or slider tick. Caches the last result client-side.
-->
<script lang="ts">
  import { decodeResampleAudio } from '$lib/utils/audioResample';

  type Props = {
    modality: 'text' | 'image' | 'audio';
    /** UTF-8 text body (text modality). */
    text: string;
    /** File handle (image / audio modality). */
    file?: File | null;
    /** Raw f32 LE samples (audio modality, when uploaded via WebAudio). */
    audioBytes?: Uint8Array | null;
    /** Audio sample rate (audio modality, required). */
    audioSampleRate?: number | null;
    /** Cached input id for live-tune; sent instead of a body when present. */
    inputId?: number | null;
    /** Live tunables forwarded as query params (subset relevant to inspect). */
    opts?: Record<string, unknown>;
  };
  let {
    modality, text, file = null,
    audioBytes = null, audioSampleRate = null,
    inputId = null, opts = {},
  }: Props = $props();

  type TextStages = {
    kind: 'text';
    algorithm: string;
    raw: string;
    canonicalized: string;
    tokens: string[];
    total_tokens: number;
    shingles: string[];
    total_shingles: number;
    fingerprint_hex: string;
    fingerprint_bytes: number;
    config_hash: number;
  };
  type ImageStages = {
    kind: 'image';
    algorithm: string;
    width: number;
    height: number;
    original_png_b64: string;
    gray32_png_b64: string;
    gray8_png_b64: string;
    ahash_mean: number;
    fingerprint_hex: string;
    fingerprint_bytes: number;
    config_hash: number;
  };
  type AudioPeak = { t_ms: number; freq_hz: number; db: number };
  type AudioStages = {
    kind: 'audio';
    algorithm: string;
    sample_rate: number;
    duration_secs: number;
    envelope: number[];
    spectrogram_png_b64: string;
    spec_width: number;
    spec_height: number;
    peaks: AudioPeak[];
    total_peaks: number;
    fingerprint_hex: string;
    fingerprint_bytes: number;
  };
  type Stages = TextStages | ImageStages | AudioStages;

  let result = $state<Stages | null>(null);
  let loading = $state(false);
  let errMsg = $state<string | null>(null);
  let openStage = $state<string | null>('canonicalized');

  const TEXT_OPT_KEYS = [
    'k','h','tokenizer','preprocess',
    'canon_normalization','canon_case_fold','canon_strip_bidi',
    'canon_strip_format','canon_apply_confusable',
  ];
  const IMAGE_OPT_KEYS = ['max_input_bytes','max_dimension','min_dimension'];
  const AUDIO_OPT_KEYS = ['sample_rate'];

  async function run(): Promise<void> {
    if (loading) return;
    if (modality === 'image' && !file && inputId == null) {
      errMsg = `Drop an image file first, then click Inspect.`;
      return;
    }
    if (modality === 'audio' && !file && audioBytes == null && inputId == null) {
      errMsg = `Drop an audio file first, then click Inspect.`;
      return;
    }
    errMsg = null;
    loading = true;
    try {
      const sp = new URLSearchParams({ modality });
      if (inputId != null) sp.set('input_id', String(inputId));
      const optKeys =
        modality === 'text'  ? TEXT_OPT_KEYS  :
        modality === 'image' ? IMAGE_OPT_KEYS :
                               AUDIO_OPT_KEYS;
      for (const k of optKeys) {
        const v = opts[k];
        if (v == null || v === '') continue;
        sp.set(k, String(v));
      }

      let body: BodyInit;
      let contentType: string;
      if (modality === 'text') {
        body = inputId != null ? '' : text;
        contentType = 'text/plain; charset=utf-8';
      } else if (modality === 'image') {
        body = inputId != null ? new ArrayBuffer(0) : await file!.arrayBuffer();
        contentType = 'application/octet-stream';
      } else {
        // Audio: prefer caller-supplied bytes; fall back to decoding the
        // file ourselves through the same WebAudio path the regular
        // upload uses.
        contentType = 'application/octet-stream';
        if (inputId != null) {
          body = new ArrayBuffer(0);
          if (audioSampleRate) sp.set('sample_rate', String(audioSampleRate));
        } else {
          // Use caller-supplied bytes if present, otherwise decode the
          // file ourselves through the same WebAudio path the regular
          // upload uses. Wang is the inspect target → 8 kHz canonical
          // rate so picked peaks match `inspect_audio`'s view.
          let samplesLE: Uint8Array;
          let sr: number;
          if (audioBytes != null && audioSampleRate) {
            samplesLE = audioBytes;
            sr = audioSampleRate;
          } else {
            const dec = await decodeResampleAudio(file!, 'wang');
            samplesLE = dec.samplesLE;
            sr = dec.sampleRate;
          }
          // Copy through ArrayBuffer for the same TS-strict reason as
          // audioResample.ts — Uint8Array<ArrayBufferLike> doesn't
          // satisfy the modern BodyInit shape.
          body = samplesLE.buffer.slice(
            samplesLE.byteOffset,
            samplesLE.byteOffset + samplesLE.byteLength,
          ) as ArrayBuffer;
          sp.set('sample_rate', String(sr));
        }
      }
      const res = await fetch(`/api/pipeline/inspect?${sp.toString()}`, {
        method: 'POST',
        headers: { 'content-type': contentType },
        body,
      });
      if (!res.ok) {
        const detail = await res.text().catch(() => String(res.status));
        errMsg = `Inspect failed (${res.status}): ${detail.slice(0, 200)}`;
        return;
      }
      const parsed = await res.json() as Record<string, unknown>;
      // Discriminate by a field that's unique to each modality response.
      if ('canonicalized' in parsed) {
        result = { kind: 'text', ...(parsed as Omit<TextStages, 'kind'>) };
        openStage ??= 'canonicalized';
      } else if ('original_png_b64' in parsed) {
        result = { kind: 'image', ...(parsed as Omit<ImageStages, 'kind'>) };
        openStage ??= 'gray8';
      } else if ('spectrogram_png_b64' in parsed) {
        result = { kind: 'audio', ...(parsed as Omit<AudioStages, 'kind'>) };
        openStage ??= 'spectrogram';
      }
    } catch (e) {
      errMsg = `Inspect error: ${(e as Error).message}`;
    } finally {
      loading = false;
    }
  }

  // Highlight characters that differ between raw and canonicalized so
  // the user can see e.g. "é" → "é" (NFKC) or zero-widths being stripped.
  type Span = { text: string; changed: boolean };
  function diffSpans(raw: string, canon: string): Span[] {
    // Cheap two-pointer character walk — not a true diff, just "did this
    // codepoint change vs the corresponding position?". Good enough for
    // a glance; bails to a single-span when lengths differ enough.
    if (Math.abs(raw.length - canon.length) > Math.min(raw.length, canon.length) / 2) {
      return [{ text: canon, changed: false }];
    }
    const out: Span[] = [];
    let buf = '';
    let bufChanged = false;
    const n = Math.min(raw.length, canon.length);
    for (let i = 0; i < n; i++) {
      const same = raw[i] === canon[i];
      if (buf.length === 0) {
        buf = canon[i];
        bufChanged = !same;
      } else if (bufChanged === !same) {
        buf += canon[i];
      } else {
        out.push({ text: buf, changed: bufChanged });
        buf = canon[i];
        bufChanged = !same;
      }
    }
    if (buf.length > 0) out.push({ text: buf, changed: bufChanged });
    if (canon.length > n) out.push({ text: canon.slice(n), changed: true });
    return out;
  }

  function toggle(stage: string) {
    openStage = openStage === stage ? null : stage;
  }
</script>

<div class="inspector">
  <header class="inspector-head">
    <span class="inspector-title">Pipeline inspector</span>
    <button type="button" class="inspect-btn" onclick={run} disabled={loading}>
      {loading ? 'Inspecting…' : (result ? 'Re-inspect' : 'Inspect pipeline')}
    </button>
  </header>

  {#if errMsg}
    <p class="err" role="alert">{errMsg}</p>
  {/if}

  {#if result && result.kind === 'image'}
    <section class="stage" class:open={openStage === 'original'}>
      <button type="button" class="stage-head" onclick={() => toggle('original')}>
        <span class="step-num">1</span>
        <span class="stage-label">Original</span>
        <span class="stage-meta">{result.width} × {result.height} px</span>
      </button>
      {#if openStage === 'original'}
        <div class="stage-body img-stage">
          <img class="img-original" src="data:image/png;base64,{result.original_png_b64}" alt="original (thumbnail)" />
        </div>
      {/if}
    </section>

    <section class="stage" class:open={openStage === 'gray32'}>
      <button type="button" class="stage-head" onclick={() => toggle('gray32')}>
        <span class="step-num">2</span>
        <span class="stage-label">32 × 32 grayscale</span>
        <span class="stage-meta">PHash DCT input</span>
      </button>
      {#if openStage === 'gray32'}
        <div class="stage-body img-stage">
          <img class="img-pixel img-32" src="data:image/png;base64,{result.gray32_png_b64}" alt="32×32 grayscale" />
        </div>
      {/if}
    </section>

    <section class="stage" class:open={openStage === 'gray8'}>
      <button type="button" class="stage-head" onclick={() => toggle('gray8')}>
        <span class="step-num">3</span>
        <span class="stage-label">8 × 8 grayscale</span>
        <span class="stage-meta">AHash input · mean = {result.ahash_mean}</span>
      </button>
      {#if openStage === 'gray8'}
        <div class="stage-body img-stage">
          <img class="img-pixel img-8" src="data:image/png;base64,{result.gray8_png_b64}" alt="8×8 grayscale" />
          <p class="caption">Each cell is one input pixel for AHash. Pixels above the mean ({result.ahash_mean}) become a 1 bit; below, a 0.</p>
        </div>
      {/if}
    </section>

    <section class="stage" class:open={openStage === 'fingerprint'}>
      <button type="button" class="stage-head" onclick={() => toggle('fingerprint')}>
        <span class="step-num">4</span>
        <span class="stage-label">Fingerprint</span>
        <span class="stage-meta">
          {result.algorithm} · {result.fingerprint_bytes} bytes
        </span>
      </button>
      {#if openStage === 'fingerprint'}
        <div class="stage-body">
          <div class="fp-meta mono">config_hash 0x{result.config_hash.toString(16)}</div>
          <pre class="fp-hex mono">{result.fingerprint_hex.slice(0, 256)}{result.fingerprint_hex.length > 256 ? '…' : ''}</pre>
        </div>
      {/if}
    </section>

  {:else if result && result.kind === 'audio'}
    {@const env = result.envelope}
    {@const envMax = Math.max(0.001, ...env)}
    {@const envPolyline = env.map((v, i) => `${i},${(50 - (v / envMax) * 48).toFixed(2)}`).join(' ')}

    <section class="stage" class:open={openStage === 'envelope'}>
      <button type="button" class="stage-head" onclick={() => toggle('envelope')}>
        <span class="step-num">1</span>
        <span class="stage-label">Waveform envelope</span>
        <span class="stage-meta">{result.duration_secs.toFixed(2)} s · {result.sample_rate.toLocaleString()} Hz</span>
      </button>
      {#if openStage === 'envelope'}
        <div class="stage-body">
          <svg viewBox="0 0 {env.length} 50" preserveAspectRatio="none" class="env-svg" role="img" aria-label="amplitude envelope">
            <polyline points={envPolyline} class="env-line" />
            <line x1="0" y1="50" x2={env.length} y2="50" class="env-axis" />
          </svg>
          <p class="caption">Max-abs sample magnitude per bucket — a quick read on overall loudness shape.</p>
        </div>
      {/if}
    </section>

    <section class="stage" class:open={openStage === 'spectrogram'}>
      <button type="button" class="stage-head" onclick={() => toggle('spectrogram')}>
        <span class="step-num">2</span>
        <span class="stage-label">Log-magnitude spectrogram</span>
        <span class="stage-meta">{result.spec_width} frames × {result.spec_height} bins</span>
      </button>
      {#if openStage === 'spectrogram'}
        <div class="stage-body img-stage">
          <img class="spec-img" src="data:image/png;base64,{result.spectrogram_png_b64}" alt="log-magnitude spectrogram" />
          <p class="caption">STFT magnitudes in dB (-60 floor → 0 max). Brighter = louder; vertical axis is frequency, low frequencies at the bottom.</p>
        </div>
      {/if}
    </section>

    <section class="stage" class:open={openStage === 'peaks'}>
      <button type="button" class="stage-head" onclick={() => toggle('peaks')}>
        <span class="step-num">3</span>
        <span class="stage-label">Picked peaks</span>
        <span class="stage-meta">
          {result.peaks.length} of {result.total_peaks}
          {result.peaks.length < result.total_peaks ? '(truncated)' : ''}
        </span>
      </button>
      {#if openStage === 'peaks'}
        <div class="stage-body">
          {#if result.peaks.length === 0}
            <p class="caption">No peaks above the magnitude floor — input may be silent or below the noise threshold.</p>
          {:else}
            {@const tMax = Math.max(1, ...result.peaks.map(p => p.t_ms))}
            {@const fMax = Math.max(1, ...result.peaks.map(p => p.freq_hz))}
            <svg viewBox="0 0 {result.spec_width} {result.spec_height}" preserveAspectRatio="none" class="peaks-svg" role="img" aria-label="picked peaks">
              {#each result.peaks as p, i (i)}
                <circle
                  cx={(p.t_ms / tMax) * result.spec_width}
                  cy={result.spec_height - (p.freq_hz / fMax) * result.spec_height}
                  r="1.2"
                  class="peak-dot" />
              {/each}
            </svg>
            <p class="caption">Each dot is a (time, frequency) landmark Wang pairs into hashes. The first 256 are shown.</p>
          {/if}
        </div>
      {/if}
    </section>

    <section class="stage" class:open={openStage === 'fingerprint'}>
      <button type="button" class="stage-head" onclick={() => toggle('fingerprint')}>
        <span class="step-num">4</span>
        <span class="stage-label">Fingerprint</span>
        <span class="stage-meta">
          {result.algorithm} · {result.fingerprint_bytes} bytes
        </span>
      </button>
      {#if openStage === 'fingerprint'}
        <div class="stage-body">
          {#if result.fingerprint_bytes === 0}
            <p class="caption">Wang produced no hashes — typical when the clip is below ~2 s or has no spectral peaks.</p>
          {:else}
            <pre class="fp-hex mono">{result.fingerprint_hex.slice(0, 256)}{result.fingerprint_hex.length > 256 ? '…' : ''}</pre>
          {/if}
        </div>
      {/if}
    </section>

  {:else if result && result.kind === 'text'}
    {@const spans = diffSpans(result.raw, result.canonicalized)}
    {@const changedCount = spans.filter(s => s.changed).reduce((a, s) => a + s.text.length, 0)}

    <section class="stage" class:open={openStage === 'raw'}>
      <button type="button" class="stage-head" onclick={() => toggle('raw')}>
        <span class="step-num">1</span>
        <span class="stage-label">Raw input</span>
        <span class="stage-meta">{result.raw.length} chars</span>
      </button>
      {#if openStage === 'raw'}
        <pre class="stage-body mono">{result.raw}</pre>
      {/if}
    </section>

    <section class="stage" class:open={openStage === 'canonicalized'}>
      <button type="button" class="stage-head" onclick={() => toggle('canonicalized')}>
        <span class="step-num">2</span>
        <span class="stage-label">Canonicalized</span>
        <span class="stage-meta">
          {result.canonicalized.length} chars · {changedCount} changed
        </span>
      </button>
      {#if openStage === 'canonicalized'}
        <pre class="stage-body mono">{#each spans as s, i (i)}<span class:diff={s.changed}>{s.text}</span>{/each}</pre>
      {/if}
    </section>

    <section class="stage" class:open={openStage === 'tokens'}>
      <button type="button" class="stage-head" onclick={() => toggle('tokens')}>
        <span class="step-num">3</span>
        <span class="stage-label">Tokens</span>
        <span class="stage-meta">
          {result.tokens.length} of {result.total_tokens}
          {result.tokens.length < result.total_tokens ? '(truncated)' : ''}
        </span>
      </button>
      {#if openStage === 'tokens'}
        <div class="stage-body chips">
          {#each result.tokens as t, i (i)}<span class="chip mono">{t}</span>{/each}
        </div>
      {/if}
    </section>

    <section class="stage" class:open={openStage === 'shingles'}>
      <button type="button" class="stage-head" onclick={() => toggle('shingles')}>
        <span class="step-num">4</span>
        <span class="stage-label">k-shingles</span>
        <span class="stage-meta">
          {result.shingles.length} of {result.total_shingles}
          {result.shingles.length < result.total_shingles ? '(truncated)' : ''}
        </span>
      </button>
      {#if openStage === 'shingles'}
        <div class="stage-body chips">
          {#each result.shingles as s, i (i)}<span class="chip mono shingle">{s}</span>{/each}
        </div>
      {/if}
    </section>

    <section class="stage" class:open={openStage === 'fingerprint'}>
      <button type="button" class="stage-head" onclick={() => toggle('fingerprint')}>
        <span class="step-num">5</span>
        <span class="stage-label">Fingerprint</span>
        <span class="stage-meta">
          {result.algorithm} · {result.fingerprint_bytes} bytes
        </span>
      </button>
      {#if openStage === 'fingerprint'}
        <div class="stage-body">
          <div class="fp-meta mono">config_hash 0x{result.config_hash.toString(16)}</div>
          <pre class="fp-hex mono">{result.fingerprint_hex.slice(0, 256)}{result.fingerprint_hex.length > 256 ? '…' : ''}</pre>
        </div>
      {/if}
    </section>
  {:else if !loading && !errMsg}
    <p class="hint">
      {#if modality === 'text'}
        Click <strong>Inspect pipeline</strong> to see each stage —
        raw → canonicalized → tokens → shingles → final hash.
      {:else if modality === 'image'}
        Click <strong>Inspect pipeline</strong> to see each stage —
        original → 32×32 grayscale → 8×8 grayscale (AHash input) → final hash.
      {:else}
        Click <strong>Inspect pipeline</strong> to see each stage —
        waveform envelope → log-magnitude spectrogram → picked peaks → final hash.
      {/if}
    </p>
  {/if}
</div>

<style>
  .inspector {
    display: flex; flex-direction: column; gap: 0.4rem;
    padding: 0.75rem 0.85rem;
    background: var(--bg-2, rgba(255,255,255,0.03));
    border: 1px solid var(--border, rgba(255,255,255,0.08));
    border-radius: 0.55rem;
  }
  .inspector-head {
    display: flex; align-items: center; justify-content: space-between;
    gap: 0.6rem;
  }
  .inspector-title {
    font-size: 0.78rem; opacity: 0.85;
    text-transform: uppercase; letter-spacing: 0.06em;
  }
  .inspect-btn {
    appearance: none; cursor: pointer;
    border: 1px solid var(--border, rgba(255,255,255,0.18));
    background: var(--surface, rgba(255,255,255,0.04));
    color: inherit;
    padding: 0.32rem 0.7rem; border-radius: 0.4rem;
    font: inherit; font-size: 0.8rem;
  }
  .inspect-btn:hover:not(:disabled) { background: var(--surface-hover, rgba(255,255,255,0.08)); }
  .inspect-btn:disabled { opacity: 0.55; cursor: progress; }
  .err {
    margin: 0; padding: 0.4rem 0.55rem; border-radius: 0.35rem;
    background: oklch(0.32 0.18 30 / 0.18);
    color: oklch(0.85 0.18 30);
    font-size: 0.78rem;
  }
  .hint {
    margin: 0.25rem 0 0;
    font-size: 0.78rem; opacity: 0.7;
    line-height: 1.4;
  }
  .stage {
    border: 1px solid var(--border, rgba(255,255,255,0.06));
    border-radius: 0.4rem;
    background: var(--bg, rgba(255,255,255,0.02));
    overflow: hidden;
  }
  .stage.open { background: var(--bg-3, rgba(255,255,255,0.045)); }
  .stage-head {
    display: grid;
    grid-template-columns: auto 1fr auto;
    gap: 0.6rem; align-items: center;
    width: 100%;
    appearance: none; border: 0; background: transparent; color: inherit;
    padding: 0.45rem 0.7rem; cursor: pointer;
    font: inherit; text-align: left;
  }
  .stage-head:hover { background: var(--surface-hover, rgba(255,255,255,0.04)); }
  .step-num {
    display: inline-flex; align-items: center; justify-content: center;
    width: 1.4rem; height: 1.4rem;
    font-family: var(--mono, monospace); font-size: 0.7rem;
    border-radius: 999px;
    background: var(--accent, oklch(0.55 0.18 240));
    color: var(--accent-ink, #fff);
  }
  .stage-label { font-size: 0.85rem; }
  .stage-meta {
    font-family: var(--mono, monospace); font-size: 0.7rem;
    opacity: 0.65;
  }
  .stage-body {
    padding: 0.6rem 0.7rem 0.7rem;
    border-top: 1px dashed var(--border, rgba(255,255,255,0.06));
  }
  .mono { font-family: var(--mono, monospace); font-size: 0.78rem; }
  pre.stage-body, pre.fp-hex {
    margin: 0; white-space: pre-wrap; word-break: break-word;
    max-height: 180px; overflow-y: auto;
  }
  pre.fp-hex {
    background: var(--bg, rgba(0,0,0,0.25));
    padding: 0.5rem 0.6rem; border-radius: 0.3rem;
    color: oklch(0.85 0.04 240);
  }
  .fp-meta { margin-bottom: 0.4rem; opacity: 0.7; }
  .diff {
    background: oklch(0.55 0.18 90 / 0.32);
    border-radius: 2px;
    padding: 0 1px;
  }
  .chips {
    display: flex; flex-wrap: wrap; gap: 0.25rem;
    max-height: 180px; overflow-y: auto;
  }
  .chip {
    display: inline-block;
    padding: 0.1rem 0.45rem;
    background: var(--bg, rgba(255,255,255,0.04));
    border: 1px solid var(--border, rgba(255,255,255,0.08));
    border-radius: 0.4rem;
    font-size: 0.72rem;
  }
  .chip.shingle {
    background: oklch(0.5 0.12 240 / 0.16);
    border-color: oklch(0.5 0.12 240 / 0.3);
  }
  /* Image-stage rendering — pixelated upscale for the 8×8 / 32×32 panes
     so users can see the discrete bit cells without antialiasing fuzz. */
  .img-stage { display: flex; flex-direction: column; align-items: flex-start; gap: 0.5rem; }
  .img-original {
    max-width: 100%;
    max-height: 256px;
    border-radius: 0.3rem;
    border: 1px solid var(--border, rgba(255,255,255,0.1));
  }
  .img-pixel {
    image-rendering: pixelated;
    border-radius: 0.3rem;
    border: 1px solid var(--border, rgba(255,255,255,0.1));
  }
  .img-32 { width: 192px; height: 192px; }
  .img-8  { width: 192px; height: 192px; }
  .caption {
    margin: 0;
    font-size: 0.74rem;
    opacity: 0.7;
    line-height: 1.4;
    max-width: 480px;
  }
  /* Audio-stage rendering. */
  .env-svg {
    width: 100%;
    height: 50px;
    background: var(--bg, rgba(0,0,0,0.25));
    border-radius: 0.3rem;
    border: 1px solid var(--border, rgba(255,255,255,0.08));
  }
  .env-line {
    fill: none;
    stroke: oklch(0.7 0.15 150);
    stroke-width: 0.6;
    vector-effect: non-scaling-stroke;
  }
  .env-axis {
    stroke: var(--ink-2, rgba(255,255,255,0.15));
    stroke-width: 0.4;
    vector-effect: non-scaling-stroke;
  }
  .spec-img {
    image-rendering: pixelated;
    width: 100%;
    max-width: 512px;
    height: 192px;
    border-radius: 0.3rem;
    border: 1px solid var(--border, rgba(255,255,255,0.1));
    background: var(--bg, #000);
  }
  .peaks-svg {
    width: 100%;
    max-width: 512px;
    height: 192px;
    background: var(--bg, rgba(0,0,0,0.4));
    border-radius: 0.3rem;
    border: 1px solid var(--border, rgba(255,255,255,0.1));
  }
  .peak-dot {
    fill: oklch(0.8 0.18 90);
    opacity: 0.85;
  }
</style>
