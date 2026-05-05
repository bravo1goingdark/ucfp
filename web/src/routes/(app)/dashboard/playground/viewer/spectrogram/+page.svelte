<!--
  Fullbleed audio spectrogram viewer.

  URL params:
    ?input_id=N         (required) — cached upstream input handle
    ?algorithm=wang     (optional) — affects landmark overlay
    ?sample_rate=8000   (optional) — needed when input was uploaded as raw f32 LE

  Calls /api/pipeline/inspect with `?modality=audio&input_id=…` to fetch
  the spectrogram PNG + peak picks + landmark pairs, then renders them
  through the new Spectrogram chart at full viewport size.
-->
<script lang="ts">
  import { page } from '$app/stores';
  import Spectrogram from '$components/charts/Spectrogram.svelte';

  type AudioPeak = { t_ms: number; freq_hz: number; db: number };
  type AudioPair = { t1_ms: number; f1_hz: number; t2_ms: number; f2_hz: number };
  type AudioInspect = {
    algorithm: string;
    sample_rate: number;
    duration_secs: number;
    spectrogram_png_b64: string;
    spec_width: number;
    spec_height: number;
    mel_spec_png_b64: string;
    mel_spec_width: number;
    mel_spec_height: number;
    mel_fmin_hz: number;
    mel_fmax_hz: number;
    peaks: AudioPeak[];
    total_peaks: number;
    landmark_pairs: AudioPair[];
    total_landmarks: number;
    fingerprint_hex: string;
  };

  let data = $state<AudioInspect | null>(null);
  let loading = $state(true);
  let err = $state<string | null>(null);
  let useMel = $state(false);

  $effect(() => {
    const inputId = $page.url.searchParams.get('input_id');
    if (!inputId) {
      err = 'no input_id in URL — open a viewer from the playground';
      loading = false;
      return;
    }
    const algo = $page.url.searchParams.get('algorithm') ?? 'wang';
    const sampleRate = $page.url.searchParams.get('sample_rate') ?? '8000';
    const qs = new URLSearchParams();
    qs.set('input_id', inputId);
    qs.set('algorithm', algo);
    qs.set('sample_rate', sampleRate);
    void load(qs);
  });

  async function load(qs: URLSearchParams) {
    loading = true;
    err = null;
    try {
      const res = await fetch(`/api/pipeline/inspect?modality=audio&${qs.toString()}`, {
        method: 'POST',
        // body deliberately empty — input_id tells the upstream what to use.
        headers: { 'content-type': 'application/octet-stream' },
        body: new Uint8Array(),
      });
      if (!res.ok) {
        err = `${res.status} ${res.statusText}: ${await res.text()}`;
        return;
      }
      data = (await res.json()) as AudioInspect;
    } catch (e) {
      err = (e as Error).message;
    } finally {
      loading = false;
    }
  }

  let png = $derived(data ? (useMel ? data.mel_spec_png_b64 : data.spectrogram_png_b64) : '');
  let pngW = $derived(data ? (useMel ? data.mel_spec_width : data.spec_width) : 0);
  let pngH = $derived(data ? (useMel ? data.mel_spec_height : data.spec_height) : 0);
  let fmin = $derived(data ? (useMel ? data.mel_fmin_hz : 0) : 0);
  let fmax = $derived(
    data ? (useMel ? data.mel_fmax_hz : (data.sample_rate / 2)) : 1,
  );
</script>

<div class="spec-viewer">
  <div class="spec-meta-bar">
    {#if data}
      <span><strong>algorithm</strong> {data.algorithm}</span>
      <span><strong>duration</strong> {data.duration_secs.toFixed(2)}s</span>
      <span><strong>sample rate</strong> {data.sample_rate} Hz</span>
      <span><strong>peaks</strong> {data.total_peaks}</span>
      <span><strong>landmarks</strong> {data.total_landmarks}</span>
      <span class="spec-mode-toggle">
        <button class:on={!useMel} onclick={() => (useMel = false)} type="button">Linear</button>
        <button class:on={useMel} onclick={() => (useMel = true)} type="button">Mel</button>
      </span>
    {/if}
  </div>

  <div class="spec-stage">
    {#if loading}
      <div class="spec-state">loading inspect…</div>
    {:else if err}
      <div class="spec-state error">
        <strong>could not load:</strong>
        <pre>{err}</pre>
      </div>
    {:else if data}
      <Spectrogram
        pngB64={png}
        pngWidth={pngW}
        pngHeight={pngH}
        fminHz={fmin}
        fmaxHz={fmax}
        durationSec={data.duration_secs}
        peaks={data.peaks}
        pairs={data.landmark_pairs}
      />
    {/if}
  </div>
</div>

<style>
  .spec-viewer {
    display: flex;
    flex-direction: column;
    height: 100%;
    width: 100%;
    min-height: 0;
  }
  .spec-meta-bar {
    display: flex;
    flex-wrap: wrap;
    gap: 18px;
    align-items: center;
    padding: 10px 24px;
    border-bottom: 1px solid var(--line);
    font-family: var(--mono);
    font-size: 11px;
    color: var(--ink-2);
    background: rgba(255, 255, 255, 0.18);
  }
  .spec-meta-bar strong {
    font-weight: 400;
    color: var(--muted);
    text-transform: uppercase;
    letter-spacing: 0.08em;
    margin-right: 6px;
  }
  .spec-mode-toggle {
    display: inline-flex;
    margin-left: auto;
    border: 1px solid var(--line-strong);
  }
  .spec-mode-toggle button {
    background: transparent;
    border: 0;
    border-right: 1px solid var(--line);
    padding: 4px 12px;
    font-family: var(--mono);
    font-size: 10px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--muted);
    cursor: pointer;
  }
  .spec-mode-toggle button:last-child {
    border-right: 0;
  }
  .spec-mode-toggle button.on {
    background: var(--ink);
    color: var(--bg);
  }
  .spec-stage {
    flex: 1;
    min-height: 0;
    padding: 20px 24px 24px;
    display: flex;
    align-items: stretch;
    justify-content: stretch;
  }
  .spec-state {
    margin: auto;
    font-family: var(--mono);
    font-size: 13px;
    color: var(--muted);
  }
  .spec-state.error {
    color: oklch(0.5 0.16 25);
  }
  .spec-state pre {
    margin: 8px 0 0;
    white-space: pre-wrap;
    color: var(--ink-2);
  }
</style>
