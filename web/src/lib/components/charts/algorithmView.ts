// Predicate for `<AlgorithmView>` — returns true when the dispatcher
// has a specialised render for this (algorithm, length) pair, so callers
// can skip the wrapping section entirely instead of printing an empty
// "Algorithm structure" header.

export function hasAlgorithmView(algorithm: string, byteLen: number): boolean {
  if (algorithm === 'imgfprint-multihash-v1') return byteLen === 504;
  if (
    algorithm === 'imgfprint-phash-v1' ||
    algorithm === 'imgfprint-dhash-v1' ||
    algorithm === 'imgfprint-ahash-v1'
  ) {
    return byteLen === 168;
  }
  if (algorithm === 'simhash-b64-tf' || algorithm === 'simhash-b64-idf') return byteLen === 8;
  if (algorithm === 'audiofp-wang-v1')   return byteLen > 0 && byteLen % 8 === 0;
  if (algorithm === 'audiofp-panako-v1') return byteLen > 0 && byteLen % 16 === 0;
  return false;
}
