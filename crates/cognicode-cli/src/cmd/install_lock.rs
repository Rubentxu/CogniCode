//! `cogh::lockfile` — Advisory install lock for atomic installs.
//!
//! Provides a PID-based lock mechanism to prevent concurrent installs
//! from interfering with each other. The lock is acquired by creating
//! a lock file with PID and timestamp, and is released when the
//! LockGuard is dropped.

use std::fs;
use std::path::PathBuf;

use anyhow::Result;

/// Path to the advisory install lock file.
fn lock_path() -> PathBuf {
    super::layout::cognicode_home()
        .join("locks")
        .join("install.lock")
}

fn lock_path_for(home: &super::layout::CognicodeHome) -> PathBuf {
    home.locks().join("install.lock")
}

/// Lock guard that releases the lock on drop.
pub struct LockGuard {
    path: PathBuf,
}

impl LockGuard {
    /// Acquire the advisory lock by creating the lock file.
    pub fn new() -> Result<Self> {
        let home = super::layout::CognicodeHome::resolve(None)?;
        Self::new_at(&home)
    }

    pub fn new_at(home: &super::layout::CognicodeHome) -> Result<Self> {
        use std::io::Write;
        let path = lock_path_for(home);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        // create_new is atomic; fs::write silently overwrote a concurrent
        // install's advisory lock, allowing both processes to mutate shims.
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|e| anyhow::anyhow!(
                "cannot acquire CogniCode install lock at {}: {e}; another installation may be in progress",
                path.display()
            ))?;
        let pid = std::process::id();
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        writeln!(file, "{pid}:{ts}")?;
        Ok(Self { path })
    }
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

/// Acquire the advisory install lock.
/// Returns a LockGuard that will release the lock on drop.
pub fn acquire_lock() -> Result<LockGuard> {
    LockGuard::new()
}

/// Product installation must lock its explicitly selected --home, not the
/// ambient COGNICODE_HOME of an unrelated development or test process.
pub fn acquire_lock_at(home: &super::layout::CognicodeHome) -> Result<LockGuard> {
    LockGuard::new_at(home)
}

/// Explicitly release the lock before the guard is dropped.
pub fn release_lock(guard: LockGuard) {
    drop(guard);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::test_support::TempCognicodeHome;
    use serial_test::serial;

    #[test]
    #[serial]
    fn lock_acquire_and_release() {
        let _home = TempCognicodeHome::new();
        let guard = acquire_lock().unwrap();
        let path = lock_path();
        assert!(path.exists(), "lock file should exist");
        release_lock(guard);
        assert!(!path.exists(), "lock file should be removed after release");
    }

    #[test]
    #[serial]
    fn lock_guard_releases_on_drop() {
        let _home = TempCognicodeHome::new();
        let guard = acquire_lock().unwrap();
        let path = lock_path();
        assert!(path.exists());
        drop(guard); // Explicit drop to test Drop impl
        assert!(!path.exists());
    }
}
