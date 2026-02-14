/// In-place L2 normalization helper to keep allocations down during hot paths.
/// Uses f32 throughout for better SIMD auto-vectorization.
pub(crate) fn l2_normalize_in_place(v: &mut [f32]) {
    let norm_sq: f32 = v.iter().map(|x| x * x).sum();
    if norm_sq > 0.0 {
        let inv_norm = norm_sq.sqrt().recip();
        for x in v.iter_mut() {
            *x *= inv_norm;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l2_normalize_unit_vector() {
        let mut v = vec![1.0f32, 0.0, 0.0];
        l2_normalize_in_place(&mut v);
        // Already unit length, should stay the same
        assert!((v[0] - 1.0).abs() < 1e-6);
        assert!((v[1]).abs() < 1e-6);
        assert!((v[2]).abs() < 1e-6);
    }

    #[test]
    fn l2_normalize_simple_vector() {
        let mut v = vec![3.0f32, 4.0];
        l2_normalize_in_place(&mut v);
        // L2 norm of [3, 4] is 5, so normalized should be [0.6, 0.8]
        assert!((v[0] - 0.6).abs() < 1e-6);
        assert!((v[1] - 0.8).abs() < 1e-6);
    }

    #[test]
    fn l2_normalize_maintains_unit_length() {
        let mut v = vec![1.0f32, 2.0, 3.0, 4.0, 5.0];
        l2_normalize_in_place(&mut v);
        // Check that the result has unit length
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-5);
    }

    #[test]
    fn l2_normalize_zero_vector() {
        let mut v = vec![0.0f32, 0.0, 0.0];
        l2_normalize_in_place(&mut v);
        // Zero vector should remain zero (no division by zero)
        assert_eq!(v, vec![0.0, 0.0, 0.0]);
    }

    #[test]
    fn l2_normalize_near_zero_vector() {
        let mut v = vec![1e-10f32, 1e-10, 1e-10];
        l2_normalize_in_place(&mut v);
        // Very small values should still normalize correctly
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-5);
    }

    #[test]
    fn l2_normalize_single_element() {
        let mut v = vec![5.0f32];
        l2_normalize_in_place(&mut v);
        // Single element should become [1.0] or [-1.0] depending on sign
        assert!((v[0] - 1.0).abs() < 1e-6 || (v[0] + 1.0).abs() < 1e-6);
    }

    #[test]
    fn l2_normalize_large_vector() {
        let mut v: Vec<f32> = (0..1000).map(|i| i as f32).collect();
        l2_normalize_in_place(&mut v);
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-4);
    }

    #[test]
    fn l2_normalize_negative_values() {
        let mut v = vec![-3.0f32, -4.0];
        l2_normalize_in_place(&mut v);
        // Should handle negative values correctly
        assert!((v[0] + 0.6).abs() < 1e-6);
        assert!((v[1] + 0.8).abs() < 1e-6);
    }

    #[test]
    fn l2_normalize_mixed_signs() {
        let mut v = vec![-1.0f32, 2.0, -3.0, 4.0];
        l2_normalize_in_place(&mut v);
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-5);
    }

    #[test]
    fn l2_normalize_empty_slice() {
        let mut v: Vec<f32> = vec![];
        l2_normalize_in_place(&mut v);
        // Empty slice should remain empty
        assert!(v.is_empty());
    }

    #[test]
    fn l2_normalize_preserves_direction() {
        let mut v1 = vec![1.0f32, 2.0, 3.0];
        let v1_original = v1.clone();
        l2_normalize_in_place(&mut v1);

        // The direction should be preserved (ratios between components)
        let ratio1 = v1[1] / v1[0];
        let ratio2 = v1[2] / v1[0];
        let expected_ratio1 = v1_original[1] / v1_original[0];
        let expected_ratio2 = v1_original[2] / v1_original[0];

        assert!((ratio1 - expected_ratio1).abs() < 1e-5);
        assert!((ratio2 - expected_ratio2).abs() < 1e-5);
    }

    #[test]
    fn l2_normalize_idempotent() {
        let mut v = vec![1.0f32, 2.0, 3.0];
        l2_normalize_in_place(&mut v);
        let first_result = v.clone();

        // Normalizing again should not change the vector significantly
        l2_normalize_in_place(&mut v);
        // Use approximate equality due to floating point precision
        for (a, b) in v.iter().zip(first_result.iter()) {
            assert!(
                (a - b).abs() < 1e-6,
                "Values should be approximately equal: {a} vs {b}"
            );
        }
    }
}
