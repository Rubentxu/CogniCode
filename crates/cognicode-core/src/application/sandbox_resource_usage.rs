//! Resource measurements shared by sandbox artifacts and the orchestrator.

use serde::{Deserialize, Serialize};

/// Resource usage observed during scenario execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsage {
    /// Peak resident set size in MB.
    pub peak_rss_mb: f64,
    /// Total CPU time in seconds.
    pub cpu_time_s: f64,
}
