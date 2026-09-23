//! `cogh::tracker` — Version tracker for installed Cogh versions.
//!
//! Writes and reads the current installed version to
//! `~/.cognicode/tracker/version`.

//! Lint policy
//!
//! This module is part of the cogh CLI distribution surface.  Items here
//! are referenced from `cogh` and/or `cognicode-release` bin targets, but
//! each individual bin only links a subset; the items that are unused
//! in a given target are still part of the public API of the cogh
//! version manager and of the release factory.  Without the module-level
//! allow, `cargo clippy --workspace --all-targets -- -D warnings` (the
//! CI gate enforced by PRF-CI-01) would fail on every push.
//!
//! The allow is anchored: the consumers for these items arrive
//! incrementally as H-03/H-04/H-06 land. Until that work lands,
//! the allow documents why these items are exported but not yet
//! fully exercised on every bin. See D34-2 for the historical
//! decision to defer the cleanup out of the PRF program.
//!
//! Audit history: 2026-09-23 confirmed in `728f05a0`/`d0913498` that
//! H-06 (A→B upgrade cycle, JOURNAL §99-§100) did NOT close this
//! allow per-item; the previous "H-06 will add live consumers" claim
//! was outdated. The allow is intentional but its justification
//! reference is updated.
#![allow(dead_code)]
#![allow(unused_imports)]

use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};

/// Default tracker path from the process environment.
///
/// DEPRECATED for production use: `COGNICODE_HOME` ignores `--home`, so
/// callers that hold a [`crate::layout::CognicodeHome`] MUST pass
/// `home.tracker_version()` instead (H-F6-1: a single resolution of home).
/// Kept only for tests that wire the tracker via the env var.
fn default_tracker_path() -> PathBuf {
    super::layout::tracker_dir().join("version")
}

/// Write the installed version to the given tracker file.
pub fn write_version_at(tracker_path: &Path, version: &str) -> Result<()> {
    if let Some(parent) = tracker_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(tracker_path, version)?;
    Ok(())
}

/// Read the current installed version from the given tracker file.
pub fn read_version_at(tracker_path: &Path) -> Result<String> {
    if !tracker_path.exists() {
        return Err(anyhow!("no version installed; run 'cogh install' first"));
    }
    let v = std::fs::read_to_string(tracker_path)?;
    Ok(v.trim().to_string())
}

/// Read the tracker as an `Option`, distinguishing "no tracker" from
/// "tracker with content". Used by the install pipeline to capture the
/// previous pin before overwriting it (e86 lifecycle-journal).
pub fn read_version_optional_at(tracker_path: &Path) -> Option<String> {
    let v = std::fs::read_to_string(tracker_path).ok()?;
    let trimmed = v.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Env-resolved convenience wrappers. Production code holding a
/// `CognicodeHome` must use the `*_at` variants with
/// `home.tracker_version()` (H-F6-1).
#[deprecated(
    since = "0.97.4",
    note = "env-only resolution; ignored `--home`. Use `write_version_at(home.tracker_version(), v)` instead (H-F6-1)."
)]
pub fn write_version(version: &str) -> Result<()> {
    write_version_at(&default_tracker_path(), version)
}

pub fn read_version() -> Result<String> {
    read_version_at(&default_tracker_path())
}

#[deprecated(
    since = "0.97.4",
    note = "env-only resolution; ignored `--home`. Use `read_version_optional_at(&home.tracker_version())` instead (H-F6-1)."
)]
pub fn read_version_optional() -> Option<String> {
    read_version_optional_at(&default_tracker_path())
}

#[cfg(test)]
mod tests {
    use serial_test::serial;
    use std::env;

    #[test]
    #[serial]
    fn write_and_read_version() {
        // Use a temp directory to avoid polluting the real tracker
        let tmp = env::temp_dir().join(format!("cogh-tracker-test-{}", process_id()));
        let tracker_dir = tmp.join("tracker");
        unsafe {
            env::set_var("COGNICODE_HOME", &tmp);
        }

        // Manually create the tracker path for testing
        let path = tracker_dir.join("version");
        std::fs::create_dir_all(&tracker_dir).unwrap();
        std::fs::write(&path, "0.94.0").unwrap();

        // Verify we can read it back
        let v = std::fs::read_to_string(&path).unwrap();
        assert_eq!(v.trim(), "0.94.0");

        std::fs::remove_dir_all(&tmp).ok();
        unsafe {
            env::remove_var("COGNICODE_HOME");
        }
    }

    fn process_id() -> u32 {
        std::process::id()
    }
}

#[cfg(test)]
mod h_f6_1_tests {
    use super::*;
    use crate::layout::CognicodeHome;

    /// H-F6-1 regression: `*_at` functions must resolve strictly via the
    /// path given (the home-resolved one), never via the env. With
    /// COGNICODE_HOME pointing elsewhere, the `*_at` variants must write
    /// and read ONLY the explicit path.
    #[test]
    fn at_functions_ignore_env_and_honour_explicit_path() {
        let tmp = std::env::temp_dir().join(format!("cogh-hf61-{}", std::process::id()));
        let uat = tmp.join("uat");
        let other = tmp.join("other");
        std::fs::create_dir_all(uat.join("tracker")).unwrap();
        std::fs::create_dir_all(other.join("tracker")).unwrap();
        // Poison the env resolution: any env-based write would land here.
        unsafe {
            std::env::set_var("COGNICODE_HOME", &other);
        }

        let home = CognicodeHome::resolve(Some(&uat)).unwrap();
        let path = home.tracker_version();

        write_version_at(&path, "0.97.3").unwrap();
        assert_eq!(read_version_at(&path).unwrap(), "0.97.3");
        assert_eq!(read_version_optional_at(&path).as_deref(), Some("0.97.3"));
        // Nothing leaked to the env-resolved home.
        assert!(!other.join("tracker/version").exists());

        std::fs::remove_dir_all(&tmp).ok();
        unsafe {
            std::env::remove_var("COGNICODE_HOME");
        }
    }

    /// H-F6-1 regression: reading an explicit path that does not exist is
    /// None even when the env-resolved tracker exists.
    #[test]
    fn read_optional_at_is_none_for_missing_explicit_path() {
        let tmp = std::env::temp_dir().join(format!("cogh-hf61b-{}", std::process::id()));
        let uat = tmp.join("uat");
        let other = tmp.join("other");
        std::fs::create_dir_all(other.join("tracker")).unwrap();
        std::fs::write(other.join("tracker/version"), "9.9.9").unwrap();
        unsafe {
            std::env::set_var("COGNICODE_HOME", &other);
        }

        let home = CognicodeHome::resolve(Some(&uat)).unwrap();
        assert!(read_version_optional_at(&home.tracker_version()).is_none());

        std::fs::remove_dir_all(&tmp).ok();
        unsafe {
            std::env::remove_var("COGNICODE_HOME");
        }
    }
}
