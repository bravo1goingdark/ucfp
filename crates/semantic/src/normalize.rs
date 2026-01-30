/// In-place L2 normalization helper to keep allocations down during hot paths.
pub(crate) fn l2_normalize_in_place(v: &mut [f32]) {
    let norm = v.iter().map(|x| (*x as f64).powi(2)).sum::<f64>().sqrt();
    if norm > 0.0 {
        let inv = 1.0 / norm as f32;
        for x in v.iter_mut() {
            *x *= inv;
        }
    }
}
