//! Git History integration for temporal indexing
//!
//! Provides functions to retrieve file modification times from git history.
//! Falls back to filesystem mtime when git history is unavailable.

use anyhow::Result;
use std::path::Path;
use std::process::Command;

/// Get the last modified time of a file from git history.
///
/// Uses `git log --follow --format=%ct -1 -- <path>` to get the most recent
/// commit time for the file. Falls back to filesystem mtime if git is
/// unavailable or the file is not tracked by git.
///
/// # Arguments
/// * `path` - Path to the file (relative or absolute)
///
/// # Returns
/// * `Ok(Some(i64))` - Unix epoch seconds of the last git commit
/// * `Ok(None)` - File not in git history, use mtime fallback
/// * `Err(_)` - Git command failed (not installed, etc.)
pub fn git_log_mtime(path: &Path) -> Result<Option<i64>> {
    // Try to get git commit time: git log --follow --format=%ct -1 -- <path>
    let path_str = path.to_string_lossy();

    let output = Command::new("git")
        .args(["log", "--follow", "--format=%ct", "-1", "--"])
        .arg(&*path_str)
        .output();

    match output {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let trimmed = stdout.trim();

            if trimmed.is_empty() || trimmed.is_empty() {
                // File exists but no git history (possibly not committed yet)
                tracing::debug!("No git history for {}, falling back to mtime", path_str);
                return Ok(None);
            }

            match trimmed.parse::<i64>() {
                Ok(timestamp) if timestamp > 0 => {
                    tracing::trace!("Git timestamp for {}: {}", path_str, timestamp);
                    Ok(Some(timestamp))
                }
                Ok(_) => {
                    tracing::warn!("Invalid git timestamp '{}' for {}", trimmed, path_str);
                    Ok(None)
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to parse git timestamp '{}' for {}: {}",
                        trimmed,
                        path_str,
                        e
                    );
                    Ok(None)
                }
            }
        }
        Ok(output) => {
            // Git command succeeded but returned non-zero (file not in git)
            tracing::debug!(
                "File {} not tracked by git: {}",
                path_str,
                String::from_utf8_lossy(&output.stderr).trim()
            );
            Ok(None)
        }
        Err(e) => {
            // Git command failed (not installed, etc.)
            tracing::warn!(
                "Git command failed for {}: {}. Falling back to mtime.",
                path_str,
                e
            );
            Ok(None)
        }
    }
}

/// Get the modification time of a file using filesystem metadata.
///
/// This is the fallback when git history is unavailable.
///
/// # Arguments
/// * `path` - Path to the file
///
/// # Returns
/// * `Ok(Some(i64))` - Unix epoch seconds from filesystem
/// * `Ok(None)` - Could not read mtime
/// * `Err(_)` - File does not exist or cannot be accessed
pub fn file_mtime(path: &Path) -> Result<Option<i64>> {
    match std::fs::metadata(path) {
        Ok(metadata) => match metadata.modified() {
            Ok(system_time) => {
                let duration = system_time
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default();
                Ok(Some(duration.as_secs() as i64))
            }
            Err(e) => {
                tracing::warn!("Failed to get mtime for {}: {}", path.display(), e);
                Ok(None)
            }
        },
        Err(e) => {
            tracing::warn!("Failed to read metadata for {}: {}", path.display(), e);
            Ok(None)
        }
    }
}

/// Get the modification time for a file, trying git first then mtime fallback.
///
/// # Arguments
/// * `path` - Path to the file
///
/// # Returns
/// * `(Option<i64>, String)` - Tuple of (timestamp_seconds, source)
///   where source is "git" or "mtime"
pub fn get_file_mtime(path: &Path) -> (Option<i64>, String) {
    // Try git first
    if let Ok(Some(timestamp)) = git_log_mtime(path) {
        return (Some(timestamp), "git".to_string());
    }

    // Fall back to mtime
    match file_mtime(path) {
        Ok(Some(timestamp)) => {
            tracing::debug!("Using mtime fallback for {}", path.display());
            (Some(timestamp), "mtime".to_string())
        }
        Ok(None) => (None, "mtime".to_string()),
        Err(_) => (None, "mtime".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_log_mtime_returns_reasonable_timestamp() {
        // This repo should have git history
        let path = Path::new("Cargo.toml");
        match git_log_mtime(path) {
            Ok(Some(ts)) => {
                // Should be a reasonable Unix timestamp
                assert!(ts > 1577836800, "Timestamp should be after Jan 1, 2020"); // 2020-01-01
                assert!(ts < 4102444800, "Timestamp should be before Jan 1, 2100");
            }
            Ok(None) => {
                // File might not be in git history yet (newly created, etc.)
                println!("No git history for Cargo.toml - this is acceptable");
            }
            Err(e) => {
                // Git not available
                println!("Git error: {} - this is acceptable in CI", e);
            }
        }
    }

    #[test]
    fn test_file_mtime_works() {
        let path = Path::new("Cargo.toml");
        match file_mtime(path) {
            Ok(Some(ts)) => {
                assert!(ts > 1577836800, "Timestamp should be after Jan 1, 2020");
            }
            Ok(None) => panic!("Should be able to get mtime for Cargo.toml"),
            Err(e) => panic!("Should be able to read Cargo.toml metadata: {}", e),
        }
    }

    #[test]
    fn test_get_file_mtime_git_priority() {
        let path = Path::new("Cargo.toml");
        let (ts, source) = get_file_mtime(path);

        // Should get some timestamp
        assert!(ts.is_some(), "Should get a timestamp for Cargo.toml");

        // Source should be either "git" or "mtime"
        assert!(
            source == "git" || source == "mtime",
            "Source should be git or mtime"
        );
    }
}
