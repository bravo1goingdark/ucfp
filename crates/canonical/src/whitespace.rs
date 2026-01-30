/// Collapses repeated whitespace, trims edges, and normalizes newlines to
/// single spaces.
///
/// This utility is deterministic and primarily useful for callers that need
/// whitespace-normalized text without running the full canonical pipeline.
pub fn collapse_whitespace(s: &str) -> String {
    let mut normalized = String::with_capacity(s.len());
    for segment in s.split_whitespace() {
        if !normalized.is_empty() {
            normalized.push(' ');
        }
        normalized.push_str(segment);
    }
    normalized
}
