// Metrics hooks for the `ufp_match` crate.
//
// Callers install a global `MatchMetrics` implementation via [`set_match_metrics`],
// then `DefaultMatcher` will report per-request latency and hit counts for each
// call to [`Matcher::match_document`]. This keeps instrumentation decoupled from
// any specific metrics backend.
use std::sync::{Arc, RwLock};
use std::time::Duration;

use once_cell::sync::OnceCell;

use crate::types::MatchMode;

/// Metrics observer for match operations.
pub trait MatchMetrics: Send + Sync {
    /// Record the outcome of a match.
    ///
    /// `tenant_id` is the logical tenant that issued the request, `mode` is the
    /// effective [`MatchMode`], `latency` is the wall-clock duration between the
    /// start and end of the match, and `hit_count` is the number of results
    /// returned to the caller after all filtering.
    fn record_match(&self, tenant_id: &str, mode: &MatchMode, latency: Duration, hit_count: usize);
}

fn metrics_lock() -> &'static RwLock<Option<Arc<dyn MatchMetrics>>> {
    static METRICS: OnceCell<RwLock<Option<Arc<dyn MatchMetrics>>>> = OnceCell::new();
    METRICS.get_or_init(|| RwLock::new(None))
}

pub(crate) fn metrics_recorder() -> Option<Arc<dyn MatchMetrics>> {
    let guard = metrics_lock()
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    guard.clone()
}

/// Install or clear the global match metrics recorder.
///
/// This is typically called once during service startup so all `DefaultMatcher`
/// instances share the same metrics backend.
pub fn set_match_metrics(recorder: Option<Arc<dyn MatchMetrics>>) {
    let lock = metrics_lock();
    let mut guard = lock.write().expect("match metrics lock poisoned");
    *guard = recorder;
}
