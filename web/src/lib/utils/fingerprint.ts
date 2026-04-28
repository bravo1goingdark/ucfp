// Deterministic in-browser FNV-1a → 32-byte stretch.
// Used by the live demo when the proxy isn't configured.

export function fnv1a(str: string): number {
  let h = 0x811c9dc5;
  for (let i = 0; i < str.length; i++) {
    h ^= str.charCodeAt(i);
    h = (h + ((h << 1) + (h << 4) + (h << 7) + (h << 8) + (h << 24))) >>> 0;
  }
  return h >>> 0;
}

export function stretch(seed: number): Uint8Array {
  const out = new Uint8Array(32);
  let s = seed >>> 0;
  for (let i = 0; i < 32; i++) {
    s = (s * 1664525 + 1013904223) >>> 0;
    out[i] = s & 0xff;
  }
  return out;
}

export function bytesToHex(b: Uint8Array): string {
  let s = '';
  for (let i = 0; i < b.length; i++) s += b[i].toString(16).padStart(2, '0');
  return s;
}

export function bytesEntropy(b: Uint8Array): number {
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

export function hammingDistance(a: Uint8Array, b: Uint8Array): number {
  let d = 0;
  const len = Math.min(a.length, b.length);
  for (let i = 0; i < len; i++) {
    let x = a[i] ^ b[i];
    while (x) {
      d += x & 1;
      x >>= 1;
    }
  }
  return d;
}

export function fingerprintLocal(input: string): {
  bytes: Uint8Array;
  hex: string;
  display: string;
  bytesLen: number;
} {
  const bytes = stretch(fnv1a(input));
  const hex = bytesToHex(bytes);
  const display = (hex.match(/.{1,4}/g) || []).slice(0, 12).join('·');
  const bytesLen = new Blob([input]).size;
  return { bytes, hex, display, bytesLen };
}
