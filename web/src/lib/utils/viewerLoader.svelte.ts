// Shared loader for the fullbleed viewer routes. Each viewer reads
// `?input_id=N&algorithm=X&modality=Y[&sample_rate=Z]` from the URL,
// posts an empty body to `/api/fingerprint` (the upstream uses the
// cached input bytes when input_id is set), and returns the parsed
// fingerprint hex + decoded byte buffer.

import { apiFetch } from './apiFetch.svelte';

export interface LoadedFingerprint {
  algorithm: string;
  fingerprintHex: string;
  fingerprintBytes: Uint8Array;
  embedding?: number[];
}

export interface LoaderState {
  loading: boolean;
  error: string | null;
  data: LoadedFingerprint | null;
}

function hexToBytes(hex: string): Uint8Array {
  if (hex.length % 2 !== 0) return new Uint8Array(0);
  const out = new Uint8Array(hex.length / 2);
  for (let i = 0; i < out.length; i++) {
    out[i] = parseInt(hex.slice(i * 2, i * 2 + 2), 16);
  }
  return out;
}

export async function loadFingerprint(params: URLSearchParams): Promise<LoadedFingerprint> {
  const inputId = params.get('input_id');
  const algorithm = params.get('algorithm');
  const modality = (params.get('modality') ?? 'text') as 'text' | 'image' | 'audio';
  const sampleRate = params.get('sample_rate');
  if (!inputId) throw new Error('no input_id in URL — open a viewer from the playground');
  if (!algorithm) throw new Error('algorithm query param required');

  const qs = new URLSearchParams();
  qs.set('algorithm', algorithm);
  qs.set('input_id', inputId);
  qs.set('return_embedding', '1');
  if (modality === 'audio' && sampleRate) qs.set('sample_rate', sampleRate);

  // Content-Type drives modality routing in /api/fingerprint.
  const headers: Record<string, string> = {};
  if (modality === 'image') headers['content-type'] = 'image/png';
  else if (modality === 'audio') headers['content-type'] = 'audio/x-raw';
  else headers['content-type'] = 'text/plain; charset=utf-8';

  const res = await apiFetch(`/api/fingerprint?${qs.toString()}`, {
    method: 'POST',
    headers,
    body: new Uint8Array(0),
  });
  if (!res.ok) {
    throw new Error(`${res.status} ${res.statusText}: ${await res.text()}`);
  }
  const body = (await res.json()) as {
    algorithm: string;
    fingerprint_hex: string;
    embedding?: number[];
  };
  return {
    algorithm: body.algorithm,
    fingerprintHex: body.fingerprint_hex,
    fingerprintBytes: hexToBytes(body.fingerprint_hex),
    embedding: body.embedding,
  };
}
