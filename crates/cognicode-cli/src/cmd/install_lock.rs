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
        let acquire = || {
            let file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)?;
            let pid = std::process::id();
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            let mut file = file;
            writeln!(file, "{pid}:{ts}")?;
            Ok::<(), anyhow::Error>(())
        };
        if acquire().is_err() {
            // Lock exists. If the recorded holder is dead, the lock is stale
            // (crash, SIGKILL mid-install, power loss) and MUST NOT block
            // recovery forever (U21). Take over atomically: remove + retry
            // once; if another process recreated it in between, we lose the
            // race honestly and fail.
            if !stale_lock(&path) {
                return Err(anyhow::anyhow!(
                    "cannot acquire CogniCode install lock at {}: another live installation is in progress",
                    path.display()
                ));
            }
            let _ = fs::remove_file(&path);
            acquire().map_err(|e| anyhow::anyhow!(
                "cannot acquire CogniCode install lock at {}: {e}; another installation may be in progress",
                path.display()
            ))?;
        }
        Ok(Self { path })
    }
}

/// A lock is stale when its recorded PID does not exist.
/// The lock format is `pid:unix_ts`; unreadable/corrupt contents are treated
/// as stale (an empty or damaged lock cannot prove a live holder).
fn stale_lock(path: &std::path::Path) -> bool {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return true,
    };
    let pid_str = content.split(':').next().unwrap_or("").trim();
    let pid: u32 = match pid_str.parse() {
        Ok(p) => p,
        Err(_) => return true,
    };
    // A pid of 0 is never a valid holder.
    if pid == 0 {
        return true;
    }
    !proc_exists(pid)
}

#[cfg(target_os = "linux")]
fn proc_exists(pid: u32) -> bool {
    std::path::Path::new("/proc").join(pid.to_string()).exists()
}

#[cfg(not(target_os = "linux"))]
fn proc_exists(_pid: u32) -> bool {
    // Without a liveness probe, treat unknown holders as live: never steal
    // a lock we cannot prove is stale.
    true
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

    #[test]
    #[serial]
    fn stale_lock_with_dead_pid_is_taken_over() {
        let _home = TempCognicodeHome::new();
        let path = lock_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        // PID 1 is not a valid holder sentinel in a container; use a pid that
        // certainly does not exist: 2^30.
        std::fs::write(&path, "1073741824:1700000000\n").unwrap();
        let guard = acquire_lock().expect("stale lock (dead pid) must be taken over");
        let content = std::fs::read_to_string(&path).unwrap();
        let holder = content.split(':').next().unwrap();
        assert_eq!(
            holder,
            std::process::id().to_string(),
            "we must be the new holder"
        );
        release_lock(guard);
        assert!(!path.exists());
    }

    #[test]
    #[serial]
    fn corrupt_lock_contents_are_treated_as_stale() {
        let _home = TempCognicodeHome::new();
        let path = lock_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, "not-a-pid\n").unwrap();
        let guard = acquire_lock().expect("corrupt lock cannot prove a live holder");
        release_lock(guard);
    }

    #[test]
    #[serial]
    fn live_pid_lock_is_refused() {
        let _home = TempCognicodeHome::new();
        let path = lock_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        // Our own pid is definitively alive.
        std::fs::write(&path, format!("{}:1700000000\n", std::process::id())).unwrap();
        let err = match acquire_lock() {
            Ok(_) => panic!("live holder must block acquisition"),
            Err(e) => e,
        };
        assert!(
            err.to_string().contains("live installation"),
            "error must distinguish live holder; got: {err}"
        );
    }
}
