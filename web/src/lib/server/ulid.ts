// Minimal ULID-style identifier generator (Workers-safe; no deps).
//
// Two flavours are exposed:
//   • `ulid()`         — Crockford-base32 ULID, 26 chars, time-ordered.
//                        Use for human-facing record ids in URLs/logs.
//   • `ulidU64()`      — u64-decimal-string (48-bit time + 16-bit random).
//                        Use when calling the upstream Rust API which
//                        currently parses `record_id` as `u64` in the path
//                        (`POST /v1/ingest/{modality}/{tid}/{rid}`). When
//                        the Rust side switches to string ids, drop this.
//
// Both use Web Crypto's `getRandomValues`; both monotonic-enough at demo
// rates (sub-ms collision odds: ~1 / 2^16 within the same ms for u64).
// 80 random bits in `ulid()` make duplicates astronomically unlikely.

const CROCKFORD = '0123456789ABCDEFGHJKMNPQRSTVWXYZ';

function encodeCrockford(value: bigint, length: number): string {
  let out = '';
  let v = value;
  for (let i = 0; i < length; i++) {
    out = CROCKFORD[Number(v & 31n)] + out;
    v >>= 5n;
  }
  return out;
}

/** Standard ULID — 48-bit ms timestamp + 80-bit randomness, base32. */
export function ulid(): string {
  const ts = BigInt(Date.now()) & ((1n << 48n) - 1n);
  const rnd = new Uint8Array(10);
  crypto.getRandomValues(rnd);
  let r = 0n;
  for (let i = 0; i < 10; i++) r = (r << 8n) | BigInt(rnd[i]);
  return encodeCrockford(ts, 10) + encodeCrockford(r, 16);
}

/** u64-safe decimal id — 48-bit ms time + 16-bit randomness. */
export function ulidU64(): string {
  const ts = BigInt(Date.now()) & ((1n << 48n) - 1n);
  const rnd = new Uint8Array(2);
  crypto.getRandomValues(rnd);
  const r = (BigInt(rnd[0]) << 8n) | BigInt(rnd[1]);
  return ((ts << 16n) | r).toString();
}
