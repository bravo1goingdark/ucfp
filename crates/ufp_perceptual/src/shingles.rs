//! Shingling and winnowing for UCFP perceptual fingerprinting.
//!
//! This module implements deterministic k‑shingling over a canonical token
//! stream, followed by winnowing to select representative shingles. Both
//! operations run in O(n) over the number of tokens or shingles.

use std::collections::VecDeque;

use xxhash_rust::xxh3::xxh3_64_with_seed;

use crate::fingerprint::WinnowedShingle;
use crate::minhash::splitmix64;

/// Compute rolling‑hash shingles deterministically in O(n).
///
/// The caller must provide **canonical tokens in order**. This function makes
/// no attempt to normalize or tokenize raw text.
pub fn make_shingles_rolling<S: AsRef<str>>(tokens: &[S], k: usize, seed: u64) -> Vec<u64> {
    let n = tokens.len();
    if k == 0 || n < k {
        return Vec::new();
    }
    // Hash each token individually first.
    let th: Vec<u64> = tokens
        .iter()
        .map(|t| xxh3_64_with_seed(t.as_ref().as_bytes(), seed))
        .collect();

    // A large prime used as the base for the polynomial hash.
    // It's XORed with a seed-derived value to make the base unpredictable.
    const BASE: u64 = 1_000_003;
    let base = BASE ^ splitmix64(seed);

    // Precompute base^(k-1) for efficient removal of the oldest element in the window.
    let mut base_km1 = 1u64;
    for _ in 1..k {
        base_km1 = base_km1.wrapping_mul(base);
    }

    let mut out = Vec::with_capacity(n - k + 1);
    let mut h = 0u64;
    // Calculate the hash of the first window.
    for &val in th.iter().take(k) {
        h = h.wrapping_mul(base).wrapping_add(val);
    }
    out.push(h);

    // Slide the window over the rest of the tokens, updating the hash in O(1) at each step.
    for (&old, &new) in th.iter().zip(th.iter().skip(k)) {
        h = h.wrapping_sub(old.wrapping_mul(base_km1)); // Remove old token
        h = h.wrapping_mul(base).wrapping_add(new); // Add new token
        out.push(h);
    }
    out
}

/// Winnowing via monotonic deque, O(n).
///
/// This selects the minimum hash in each window of shingles, with rightmost
/// tie-breaking. It is deterministic for a given sequence of shingle hashes
/// and window size.
pub fn winnow_minq(shingles: &[u64], w: usize) -> Vec<WinnowedShingle> {
    let n = shingles.len();
    if n == 0 {
        return Vec::new();
    }

    // Ensure the window size is at least 1 and not larger than the number of shingles.
    let window = w.max(1);
    let window_span = window.min(n);
    let window_count = if window >= n { 1 } else { n - window + 1 };
    let mut out = Vec::with_capacity(window_count);
    // The deque stores indices of shingles in the current window, in increasing order of their hash values.
    let mut dq: VecDeque<usize> = VecDeque::with_capacity(window_span);
    let mut last_picked: Option<usize> = None;

    // Helper to push a new index onto the deque, maintaining the monotonic property.
    let push = |dq: &mut VecDeque<usize>, i: usize, vals: &[u64]| {
        // Remove elements from the back of the deque that are greater than or equal to the new element.
        // This ensures the deque is monotonically increasing and handles rightmost tie-breaking.
        while let Some(&j) = dq.back() {
            if vals[i] <= vals[j] {
                dq.pop_back();
            } else {
                break;
            }
        }
        dq.push_back(i);
    };

    // Initialize the first window.
    for i in 0..window_span {
        push(&mut dq, i, shingles);
    }

    // Helper to emit the minimum hash in the current window.
    let emit = |dq: &VecDeque<usize>,
                out: &mut Vec<WinnowedShingle>,
                last: &mut Option<usize>,
                vals: &[u64]| {
        // The front of the deque always holds the index of the minimum hash in the window.
        if let Some(&idx) = dq.front() {
            // Only emit if it's a new minimum.
            if *last != Some(idx) {
                out.push(WinnowedShingle {
                    hash: vals[idx],
                    start_idx: idx,
                });
                *last = Some(idx);
            }
        }
    };

    emit(&dq, &mut out, &mut last_picked, shingles);

    // Slide the window over the rest of the shingles.
    for i in window..n {
        // Remove indices that are no longer in the window.
        let left = i - window + 1;
        while let Some(&j) = dq.front() {
            if j < left {
                dq.pop_front();
            } else {
                break;
            }
        }
        // Push the new index and emit the new minimum if it has changed.
        push(&mut dq, i, shingles);
        emit(&dq, &mut out, &mut last_picked, shingles);
    }

    out
}
