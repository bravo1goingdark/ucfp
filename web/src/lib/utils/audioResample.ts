// Decode a browser-loaded audio file and resample to a target rate, then
// pack into f32 LE bytes wrapped in a FormData ready for /api/fingerprint.
//
// Used by the playground, search, and bulk pages so they all build the
// same upstream-friendly multipart body (the Rust `/v1/ingest/audio/...`
// handler requires raw f32 LE samples; sample_rate goes as a form field).

export const AUDIO_RATES_BY_ALG: Record<string, number> = {
  wang: 8000,
  panako: 8000,
  haitsma: 5000,
  neural: 16000,
  watermark: 16000
};

export function targetSampleRateFor(algorithm: string): number {
  return AUDIO_RATES_BY_ALG[algorithm] ?? 8000;
}

export async function buildResampledAudioForm(
  file: File,
  algorithm: string
): Promise<{ form: FormData; sampleRate: number; bytes: number }> {
  const sampleRate = targetSampleRateFor(algorithm);
  const arrayBuf = await file.arrayBuffer();
  const ACtx = (window.AudioContext ||
    (window as unknown as { webkitAudioContext: typeof AudioContext }).webkitAudioContext);
  const ac = new ACtx();
  try {
    const decoded = await ac.decodeAudioData(arrayBuf.slice(0));
    const sampleCount = Math.ceil(decoded.duration * sampleRate);
    const offline = new OfflineAudioContext(1, sampleCount, sampleRate);
    const src = offline.createBufferSource();
    src.buffer = decoded;
    src.connect(offline.destination);
    src.start(0);
    const resampled = await offline.startRendering();
    const ch = resampled.getChannelData(0);
    const bytes = new Uint8Array(ch.length * 4);
    const dv = new DataView(bytes.buffer);
    for (let i = 0; i < ch.length; i++) dv.setFloat32(i * 4, ch[i], true);

    const form = new FormData();
    form.set('file', new File([bytes], 'audio.f32le', { type: 'audio/x-f32le' }));
    form.set('sample_rate', String(sampleRate));
    return { form, sampleRate, bytes: bytes.byteLength };
  } finally {
    try { await ac.close(); } catch { /* */ }
  }
}
