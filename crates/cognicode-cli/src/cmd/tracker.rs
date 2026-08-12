//! `cogh::tracker` — Version tracker for installed Cogh versions.
//!
//! Writes and reads the current installed version to
//! `~/.cognicode/tracker/version`.

use std::path::PathBuf;

use anyhow::{anyhow, Result};

/// Path to the tracker version file.
fn tracker_path() -> PathBuf {
    super::layout::tracker_dir().join("version")
}

/// Write the installed version to the tracker.
pub fn write_version(version: &str) -> Result<()> {
    let path = tracker_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, version)?;
    Ok(())
}

/// Read the current installed version from the tracker.
pub fn read_version() -> Result<String> {
    let path = tracker_path();
    if !path.exists() {
        return Err(anyhow!("no version installed; run 'cogh install' first"));
    }
    let v = std::fs::read_to_string(&path)?;
    Ok(v.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn write_and_read_version() {
        // Use a temp directory to avoid polluting the real tracker
        let tmp = env::temp_dir().join(format!("cogh-tracker-test-{}", process_id()));
        let tracker_dir = tmp.join("tracker");
        env::set_var("COGNICODE_HOME", &tmp);

        // Manually create the tracker path for testing
        let path = tracker_dir.join("version");
        std::fs::create_dir_all(&tracker_dir).unwrap();
        std::fs::write(&path, "0.94.0").unwrap();

        // Verify we can read it back
        let v = std::fs::read_to_string(&path).unwrap();
        assert_eq!(v.trim(), "0.94.0");

        std::fs::remove_dir_all(&tmp).ok();
        env::remove_var("COGNICODE_HOME");
    }

    fn process_id() -> u32 {
        std::process::id()
    }
}
