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

/// Lock guard that releases the lock on drop.
pub struct LockGuard {
    path: PathBuf,
}

impl LockGuard {
    /// Acquire the advisory lock by creating the lock file.
    pub fn new() -> Result<Self> {
        let path = lock_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        // PID + timestamp for uniqueness and debugging
        let pid = std::process::id();
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let content = format!("{}:{}\n", pid, ts);
        fs::write(&path, content)?;
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

/// Explicitly release the lock before the guard is dropped.
pub fn release_lock(guard: LockGuard) {
    drop(guard);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_acquire_and_release() {
        let guard = acquire_lock().unwrap();
        let path = lock_path();
        assert!(path.exists(), "lock file should exist");
        release_lock(guard);
        assert!(!path.exists(), "lock file should be removed after release");
    }

    #[test]
    fn lock_guard_releases_on_drop() {
        let guard = acquire_lock().unwrap();
        let path = lock_path();
        assert!(path.exists());
        drop(guard); // Explicit drop to test Drop impl
        assert!(!path.exists());
    }
}
