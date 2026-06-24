//! Error types for graph algorithms.

use thiserror::Error;

/// Errors that can occur during graph analytics operations.
#[derive(Debug, Error, PartialEq)]
pub enum AnalyticsError {
    /// Percentile value was outside the valid range [0.0, 1.0].
    #[error("percentile must be in [0.0, 1.0], got {0}")]
    InvalidPercentile(f64),
    /// The adjacency structure was malformed.
    #[error("input adjacency is malformed: {0}")]
    MalformedAdjacency(&'static str),
}
