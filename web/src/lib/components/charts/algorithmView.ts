// Predicate for `<AlgorithmView>` — returns true when the dispatcher
// has a specialised render for this (algorithm, length) pair, so callers
// can skip the wrapping section entirely instead of printing an empty
// "Algorithm structure" header.

export function hasAlgorithmView(algorithm: string, byteLen: number): boolean {
  // MultiHashFingerprint = 32 (bundle exact) + 168×3 (ahash, phash, dhash) = 536.
  // Verified against the `assert!(size_of::<MultiHashFingerprint>() == 536)` in
  // imgfprint-0.4.1/src/core/fingerprint.rs.
  if (algorithm === 'imgfprint-multihash-v1') return byteLen === 536;
  if (
    algorithm === 'imgfprint-phash-v1' ||
    algorithm === 'imgfprint-dhash-v1' ||
    algorithm === 'imgfprint-ahash-v1'
  ) {
    return byteLen === 168;
  }
  if (algorithm === 'simhash-b64-tf' || algorithm === 'simhash-b64-idf') return byteLen === 8;
  // MinHash<128>: 128 u64 slots × 8 bytes = 1024 bytes (txtfp::MinHashSig is repr(C) Pod).
  if (algorithm === 'minhash-h128')      return byteLen === 1024;
  if (algorithm === 'audiofp-wang-v1')   return byteLen > 0 && byteLen % 8 === 0;
  if (algorithm === 'audiofp-panako-v1') return byteLen > 0 && byteLen % 16 === 0;
  return false;
}
