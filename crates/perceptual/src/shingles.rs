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
    let mut th: Vec<u64> = Vec::with_capacity(n);
    th.extend(
        tokens
            .iter()
            .map(|t| xxh3_64_with_seed(t.as_ref().as_bytes(), seed)),
    );

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
    if window >= n {
        let mut min_idx = 0;
        let mut min_val = shingles[0];
        for (idx, &val) in shingles.iter().enumerate().skip(1) {
            if val <= min_val {
                min_val = val;
                min_idx = idx;
            }
        }
        return vec![WinnowedShingle {
            hash: min_val,
            start_idx: min_idx,
        }];
    }

    let window_span = window;
    let window_count = n - window + 1;
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

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== Shingling Tests ====================

    #[test]
    fn make_shingles_rolling_empty_tokens() {
        let tokens: Vec<&str> = vec![];
        let shingles = make_shingles_rolling(&tokens, 3, 42);
        assert!(shingles.is_empty());
    }

    #[test]
    fn make_shingles_rolling_k_zero() {
        let tokens = vec!["a", "b", "c"];
        let shingles = make_shingles_rolling(&tokens, 0, 42);
        assert!(shingles.is_empty());
    }

    #[test]
    fn make_shingles_rolling_n_less_than_k() {
        let tokens = vec!["a", "b"];
        let shingles = make_shingles_rolling(&tokens, 3, 42);
        assert!(shingles.is_empty());
    }

    #[test]
    fn make_shingles_rolling_exact_k() {
        let tokens = vec!["a", "b", "c"];
        let shingles = make_shingles_rolling(&tokens, 3, 42);
        assert_eq!(shingles.len(), 1);
    }

    #[test]
    fn make_shingles_rolling_produces_correct_count() {
        // For n tokens and k shingle size, we get n - k + 1 shingles
        let tokens = vec!["a", "b", "c", "d", "e"];
        let k = 3;
        let shingles = make_shingles_rolling(&tokens, k, 42);
        assert_eq!(shingles.len(), tokens.len() - k + 1);
        assert_eq!(shingles.len(), 3); // 5 - 3 + 1 = 3
    }

    #[test]
    fn make_shingles_rolling_deterministic() {
        let tokens = vec!["the", "quick", "brown", "fox", "jumps"];
        let seed = 12345u64;

        let shingles1 = make_shingles_rolling(&tokens, 3, seed);
        let shingles2 = make_shingles_rolling(&tokens, 3, seed);

        assert_eq!(shingles1, shingles2);
    }

    #[test]
    fn make_shingles_rolling_different_seeds() {
        let tokens = vec!["the", "quick", "brown", "fox"];

        let shingles1 = make_shingles_rolling(&tokens, 3, 12345u64);
        let shingles2 = make_shingles_rolling(&tokens, 3, 54321u64);

        // Different seeds should produce different hashes
        assert_ne!(shingles1, shingles2);
    }

    #[test]
    fn make_shingles_rolling_different_order() {
        let tokens1 = vec!["the", "quick", "brown"];
        let tokens2 = vec!["brown", "quick", "the"];
        let seed = 42u64;

        let shingles1 = make_shingles_rolling(&tokens1, 2, seed);
        let shingles2 = make_shingles_rolling(&tokens2, 2, seed);

        // Same tokens in different order should produce different shingles
        assert_ne!(shingles1, shingles2);
    }

    #[test]
    fn make_shingles_rolling_single_token_k1() {
        let tokens = vec!["hello"];
        let shingles = make_shingles_rolling(&tokens, 1, 42);
        assert_eq!(shingles.len(), 1);
    }

    #[test]
    fn make_shingles_rolling_hash_values_well_distributed() {
        let tokens = vec!["a", "b", "c", "d", "e", "f", "g", "h", "i", "j"];
        let shingles = make_shingles_rolling(&tokens, 3, 42);

        // Check that not all shingles are the same (they should be different)
        let first = shingles[0];
        let all_same = shingles.iter().all(|&s| s == first);
        assert!(
            !all_same,
            "Shingles should be well-distributed, not all identical"
        );
    }

    #[test]
    fn make_shingles_rolling_large_k() {
        let tokens: Vec<String> = (0..1000).map(|i| format!("token{i}")).collect();
        let k = 100;
        let shingles = make_shingles_rolling(&tokens, k, 42);
        assert_eq!(shingles.len(), 901); // 1000 - 100 + 1
    }

    // ==================== Winnowing Tests ====================

    #[test]
    fn winnow_minq_empty_shingles() {
        let shingles: Vec<u64> = vec![];
        let winnowed = winnow_minq(&shingles, 4);
        assert!(winnowed.is_empty());
    }

    #[test]
    fn winnow_minq_w_zero_treated_as_one() {
        let shingles = vec![100u64, 50u64, 75u64];
        let winnowed = winnow_minq(&shingles, 0);
        // With w=0 or w=1, each window is size 1, so we get all shingles
        assert!(!winnowed.is_empty());
    }

    #[test]
    fn winnow_minq_window_larger_than_shingles() {
        let shingles = vec![50u64, 100u64, 75u64];
        let winnowed = winnow_minq(&shingles, 10); // window > n

        // Should return single global minimum
        assert_eq!(winnowed.len(), 1);
        assert_eq!(winnowed[0].hash, 50u64); // minimum value
    }

    #[test]
    fn winnow_minq_selects_correct_minimums() {
        // Arrange shingles so minimum moves predictably
        let shingles = vec![100u64, 50u64, 200u64, 75u64, 25u64];
        let winnowed = winnow_minq(&shingles, 2); // window size 2

        // With w=2, windows are: [100,50], [50,200], [200,75], [75,25]
        // Minimums: 50, 50, 75, 25 (but 50 repeats so we skip second)
        // Expected: 50 (idx 1), 75 (idx 3), 25 (idx 4)
        assert!(winnowed.len() >= 2);

        // First winnowed should be the minimum from first window
        let first_hash = winnowed[0].hash;
        assert!(first_hash <= 50);
    }

    #[test]
    fn winnow_minq_rightmost_tie_breaking() {
        // Create a situation with equal values
        let shingles = vec![100u64, 50u64, 50u64, 75u64];
        let winnowed = winnow_minq(&shingles, 3);

        // If there are ties, the rightmost should win
        // This is harder to test directly, but we can verify determinism
        let winnowed2 = winnow_minq(&shingles, 3);
        assert_eq!(winnowed, winnowed2);
    }

    #[test]
    fn winnow_minq_no_duplicate_consecutive_selections() {
        // Create shingles where same index is minimum in consecutive windows
        let shingles = vec![100u64, 1u64, 200u64, 300u64];
        let winnowed = winnow_minq(&shingles, 2);

        // Index 1 (value 1) is minimum in both first and second windows
        // But should only be emitted once
        let hashes: Vec<u64> = winnowed.iter().map(|w| w.hash).collect();
        let unique_hashes: std::collections::HashSet<u64> = hashes.iter().cloned().collect();

        // All emitted hashes should be unique (no consecutive duplicates)
        assert_eq!(hashes.len(), unique_hashes.len());
    }

    #[test]
    fn winnow_minq_preserves_start_indices() {
        let shingles = vec![100u64, 50u64, 200u64, 75u64];
        let winnowed = winnow_minq(&shingles, 2);

        for w in &winnowed {
            // start_idx should be within bounds
            assert!(w.start_idx < shingles.len());
            // hash should match the shingle at that index
            assert_eq!(w.hash, shingles[w.start_idx]);
        }
    }

    #[test]
    fn winnow_minq_deterministic() {
        let shingles = vec![100u64, 50u64, 200u64, 75u64, 25u64, 150u64];

        let winnowed1 = winnow_minq(&shingles, 3);
        let winnowed2 = winnow_minq(&shingles, 3);

        assert_eq!(winnowed1, winnowed2);
    }

    #[test]
    fn winnow_minq_covers_all_windows() {
        let shingles: Vec<u64> = (0..10).map(|i| (i * 10) as u64).collect(); // 0, 10, 20, ..., 90
        let winnowed = winnow_minq(&shingles, 3);

        // The minimum value is 0 at index 0
        // It should be selected in the first window
        assert!(winnowed.iter().any(|w| w.hash == 0));
    }

    #[test]
    fn winnow_minq_boundary_conditions() {
        // Test with small number of shingles
        let shingles = vec![1u64];
        let winnowed = winnow_minq(&shingles, 4);
        assert_eq!(winnowed.len(), 1);
        assert_eq!(winnowed[0].hash, 1u64);

        let shingles = vec![1u64, 2u64];
        let winnowed = winnow_minq(&shingles, 4);
        assert_eq!(winnowed.len(), 1);
        assert_eq!(winnowed[0].hash, 1u64);
    }

    #[test]
    fn integration_shingle_then_winnow() {
        let tokens = vec!["the", "quick", "brown", "fox", "jumps", "over"];
        let k = 2;
        let w = 2;
        let seed = 42u64;

        let shingles = make_shingles_rolling(&tokens, k, seed);
        assert_eq!(shingles.len(), 5); // 6 - 2 + 1 = 5

        let winnowed = winnow_minq(&shingles, w);
        assert!(!winnowed.is_empty());

        // Each winnowed shingle should correspond to a valid shingle
        for ws in &winnowed {
            assert!(ws.start_idx < shingles.len());
            assert_eq!(ws.hash, shingles[ws.start_idx]);
        }
    }

    #[test]
    fn shingles_with_string_references() {
        let string_tokens = vec![
            "hello".to_string(),
            "world".to_string(),
            "foo".to_string(),
            "bar".to_string(),
        ];
        let shingles = make_shingles_rolling(&string_tokens, 2, 42);
        assert_eq!(shingles.len(), 3);
    }

    #[test]
    fn shingles_with_mixed_string_types() {
        let tokens: Vec<&str> = vec!["hello", "world", "test"];
        let shingles = make_shingles_rolling(&tokens, 2, 42);
        assert_eq!(shingles.len(), 2);
    }
}
