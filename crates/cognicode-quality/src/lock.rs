//! File-based advisory lock for serializing analysis runs.
//!
//! Uses `flock(2)` semantics via `fs2` for cross-platform support.
//! Lock is automatically released on Drop.

use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use std::time::Duration;
use fs2::FileExt;

/// Lock file name in the cognicode data directory
const LOCK_FILE_NAME: &str = "analysis.lock";

/// Advisory file lock for serializing quality analysis runs.
///
/// Uses exclusive flock semantics to ensure only one analysis
/// process runs at a time per project.
pub struct AnalysisLock {
    /// Path to the lock file (for debugging/error messages)
    lock_path: PathBuf,
    /// The locked file descriptor
    file: File,
}

impl AnalysisLock {
    /// Attempt to acquire an exclusive lock without blocking.
    ///
    /// Returns `Some(Self)` if the lock was acquired,
    /// `None` if the file is already locked by another process.
    pub fn try_acquire(project_root: &Path) -> Option<Self> {
        let lock_path = Self::lock_file_path(project_root);
        
        // Ensure the .cognicode directory exists
        if let Some(parent) = lock_path.parent() {
            fs::create_dir_all(parent).ok()?;
        }
        
        // Open or create the lock file
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .read(true)
            .open(&lock_path)
            .ok()?;
        
        // Try to acquire exclusive lock (non-blocking)
        file.try_lock_exclusive().ok()?;
        
        Some(Self { lock_path, file })
    }
    
    /// Acquire lock with exponential backoff retry.
    ///
    /// Retries up to `max_retries` times with exponential backoff:
    /// 100ms, 200ms, 400ms, 800ms, 1600ms (capped at 1600ms).
    ///
    /// Returns `Some(Self)` if lock acquired, `None` if all retries failed.
    pub fn acquire_with_retry(project_root: &Path, max_retries: u32) -> Option<Self> {
        let base_delay_ms: u64 = 100;
        let max_delay_ms: u64 = 1600;
        
        for attempt in 0..=max_retries {
            if attempt > 0 {
                // Calculate exponential backoff: 100, 200, 400, 800, 1600, ...
                let delay_ms = (base_delay_ms * 2u64.pow(attempt - 1)).min(max_delay_ms);
                std::thread::sleep(Duration::from_millis(delay_ms));
            }
            
            if let Some(lock) = Self::try_acquire(project_root) {
                return Some(lock);
            }
        }
        
        None
    }
    
    /// Get the path to the lock file for a project.
    fn lock_file_path(project_root: &Path) -> PathBuf {
        project_root.join(".cognicode").join(LOCK_FILE_NAME)
    }
    
    /// Get the lock file path (for debugging).
    #[allow(dead_code)]
    pub fn lock_path(&self) -> &Path {
        &self.lock_path
    }
}

impl Drop for AnalysisLock {
    fn drop(&mut self) {
        // Release the exclusive lock
        // Note: fs2 locks are automatically released when the file is closed,
        // but we explicitly unlock for clarity and to handle Drop in non-matching order
        self.file.unlock().ok();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[test]
    fn test_try_acquire_success() {
        let temp_dir = TempDir::new().unwrap();
        
        let lock = AnalysisLock::try_acquire(temp_dir.path());
        assert!(lock.is_some());
        
        // Lock should be dropped here, releasing the lock
    }
    
    #[test]
    fn test_try_acquire_blocked_when_locked() {
        let temp_dir = TempDir::new().unwrap();
        
        // Acquire first lock
        let first = AnalysisLock::try_acquire(temp_dir.path());
        assert!(first.is_some());
        
        // Second attempt should fail (already locked by first)
        let second = AnalysisLock::try_acquire(temp_dir.path());
        assert!(second.is_none());
        
        // Drop first lock
        drop(first);
        
        // Now second should succeed
        let second = AnalysisLock::try_acquire(temp_dir.path());
        assert!(second.is_some());
    }
    
    #[test]
    fn test_lock_released_on_drop() {
        let temp_dir = TempDir::new().unwrap();
        
        let lock = AnalysisLock::try_acquire(temp_dir.path()).unwrap();
        let lock_path = lock.lock_path().to_path_buf();
        drop(lock);
        
        // After drop, another lock should be acquirable
        let lock2 = AnalysisLock::try_acquire(temp_dir.path());
        assert!(lock2.is_some());
        
        // Verify lock file still exists (but is now unlocked)
        assert!(lock_path.exists());
    }
    
    #[test]
    fn test_acquire_with_retry_success_first_try() {
        let temp_dir = TempDir::new().unwrap();
        
        let lock = AnalysisLock::acquire_with_retry(temp_dir.path(), 5);
        assert!(lock.is_some());
    }
    
    #[test]
    fn test_acquire_with_retry_eventual_success() {
        let temp_dir = TempDir::new().unwrap();
        
        // Hold the lock
        let first = AnalysisLock::try_acquire(temp_dir.path()).unwrap();
        
        // Release after a short delay in a separate thread
        let handle = std::thread::spawn(move || {
            drop(first);
        });
        
        // Should eventually get the lock
        let lock = AnalysisLock::acquire_with_retry(temp_dir.path(), 10);
        assert!(lock.is_some());
        
        handle.join().unwrap();
    }
    
    #[test]
    fn test_acquire_with_retry_exhausted() {
        let temp_dir = TempDir::new().unwrap();
        
        // Hold the lock indefinitely
        let _lock = AnalysisLock::try_acquire(temp_dir.path()).unwrap();
        
        // Should fail after exhausting retries
        let lock = AnalysisLock::acquire_with_retry(temp_dir.path(), 2);
        assert!(lock.is_none());
    }
    
    #[test]
    fn test_lock_path_creation() {
        let temp_dir = TempDir::new().unwrap();
        
        let lock = AnalysisLock::try_acquire(temp_dir.path()).unwrap();
        let lock_path = lock.lock_path();
        
        // Lock path should be in .cognicode directory
        assert!(lock_path.to_str().unwrap().ends_with(".cognicode/analysis.lock"));
        assert!(lock_path.exists());
    }
}
