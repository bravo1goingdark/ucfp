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
  const decoded = await decodeResampleAudio(file, algorithm);
  const form = new FormData();
  // TS strict mode treats Uint8Array<ArrayBufferLike> as incompatible
  // with the new BlobPart shape; copy through an ArrayBuffer to land on
  // the strictly-narrower variant.
  const ab = decoded.samplesLE.buffer.slice(
    decoded.samplesLE.byteOffset,
    decoded.samplesLE.byteOffset + decoded.samplesLE.byteLength,
  ) as ArrayBuffer;
  form.set('file', new File([ab], 'audio.f32le', { type: 'audio/x-f32le' }));
  form.set('sample_rate', String(decoded.sampleRate));
  return { form, sampleRate: decoded.sampleRate, bytes: decoded.samplesLE.byteLength };
}

/**
 * Decode + resample an audio file once and return the raw f32 LE byte
 * buffer along with the chosen sample rate. Used by both the regular
 * fingerprint upload (which wraps the bytes in a FormData) and the
 * pipeline inspector (which posts them directly as the request body).
 */
export async function decodeResampleAudio(
  file: File,
  algorithm: string
): Promise<{ samplesLE: Uint8Array; sampleRate: number }> {
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
    const samplesLE = new Uint8Array(ch.length * 4);
    const dv = new DataView(samplesLE.buffer);
    for (let i = 0; i < ch.length; i++) dv.setFloat32(i * 4, ch[i], true);
    return { samplesLE, sampleRate };
  } finally {
    try { await ac.close(); } catch { /* */ }
  }
}
