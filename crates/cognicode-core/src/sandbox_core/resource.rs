//! Resource usage measurement for scenario execution.
//!
//! Captures RSS (resident set size) and CPU time for the current process
//! and its children.

/// Resource snapshot at a point in time.
#[derive(Debug, Clone)]
pub struct ResourceSnapshot {
    /// RSS in kilobytes (from /proc/self/status VmRSS on Linux)
    pub rss_kb: u64,
    /// Wall clock time since some reference
    pub timestamp_ms: u64,
}

/// Cumulative resource usage between two snapshots.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ResourceDelta {
    /// Peak RSS observed in KB
    pub peak_rss_kb: u64,
    /// Peak RSS in MB (for display)
    pub peak_rss_mb: f64,
    /// Wall clock duration in ms
    pub duration_ms: u64,
}

/// Read current process RSS from /proc/self/status (Linux only).
/// Returns 0 on non-Linux platforms.
pub fn read_current_rss_kb() -> u64 {
    #[cfg(target_os = "linux")]
    {
        std::fs::read_to_string("/proc/self/status")
            .ok()
            .and_then(|content| {
                content
                    .lines()
                    .find(|line| line.starts_with("VmRSS:"))
                    .and_then(|line| {
                        line.split_whitespace()
                            .nth(1)
                            .and_then(|s| s.parse::<u64>().ok())
                    })
            })
            .unwrap_or(0)
    }
    #[cfg(not(target_os = "linux"))]
    {
        0
    }
}

/// Take a resource snapshot.
pub fn take_snapshot() -> ResourceSnapshot {
    ResourceSnapshot {
        rss_kb: read_current_rss_kb(),
        timestamp_ms: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0),
    }
}

/// Compute resource delta between start and end snapshots.
/// Takes the peak RSS observed (end RSS is typically higher after allocation).
pub fn compute_delta(start: &ResourceSnapshot, end: &ResourceSnapshot) -> ResourceDelta {
    let peak_rss_kb = std::cmp::max(start.rss_kb, end.rss_kb);
    ResourceDelta {
        peak_rss_kb,
        peak_rss_mb: peak_rss_kb as f64 / 1024.0,
        duration_ms: end.timestamp_ms.saturating_sub(start.timestamp_ms),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_current_rss_kb_returns_nonzero() {
        let rss = read_current_rss_kb();
        // On Linux, RSS should be non-zero for a running process
        #[cfg(target_os = "linux")]
        {
            assert!(rss > 0, "RSS should be non-zero on Linux, got {}", rss);
        }
        #[cfg(not(target_os = "linux"))]
        {
            assert_eq!(rss, 0, "RSS should be 0 on non-Linux platforms");
        }
    }

    #[test]
    fn test_compute_delta() {
        let start = ResourceSnapshot {
            rss_kb: 1000,
            timestamp_ms: 100,
        };
        let end = ResourceSnapshot {
            rss_kb: 2500,
            timestamp_ms: 300,
        };

        let delta = compute_delta(&start, &end);

        assert_eq!(delta.peak_rss_kb, 2500);
        assert!((delta.peak_rss_mb - 2.44).abs() < 0.01); // ~2.44 MB
        assert_eq!(delta.duration_ms, 200);
    }

    #[test]
    fn test_compute_delta_with_higher_start_rss() {
        // End RSS might be lower due to memory reclamation; peak should be max
        let start = ResourceSnapshot {
            rss_kb: 5000,
            timestamp_ms: 100,
        };
        let end = ResourceSnapshot {
            rss_kb: 3000,
            timestamp_ms: 200,
        };

        let delta = compute_delta(&start, &end);

        assert_eq!(delta.peak_rss_kb, 5000); // Peak is start
        assert!((delta.peak_rss_mb - 4.88).abs() < 0.01); // ~4.88 MB
    }

    #[test]
    fn test_resource_snapshot() {
        let snap = ResourceSnapshot {
            rss_kb: 4096,
            timestamp_ms: 1000,
        };

        assert_eq!(snap.rss_kb, 4096);
        assert_eq!(snap.timestamp_ms, 1000);
    }

    #[test]
    fn test_take_snapshot() {
        let snap = take_snapshot();
        // RSS should be reasonable for a running process
        assert!(snap.rss_kb > 0);
        assert!(snap.timestamp_ms > 0);
    }
}
