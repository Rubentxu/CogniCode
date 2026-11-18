//! The port that measures the native metrics of a source file.
//!
//! The rules that measure a language belong to the adapter: the
//! application asks for the measurement of a file and gets an outcome it
//! can reason about, without knowing which parser or which rule table
//! answered.

use crate::domain::metrics::MetricsOutcome;

/// Measures the native metrics of one source file.
pub trait SourceMetrics: Send + Sync {
    /// The metrics of `source`, whose language is decided by `path`.
    ///
    /// The outcome says what was measured and what was not: a language
    /// without rules, a file that could not be parsed and a file that was
    /// measured are three different answers, and the caller has to be able
    /// to tell them apart.
    fn measure_by_extension(&self, path: &str, source: &str) -> MetricsOutcome;
}
