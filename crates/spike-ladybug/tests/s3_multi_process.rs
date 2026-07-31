//! E29 S3 multi-process lock tests — validates OS-level file locking.
//!
//! Three test functions for the lock matrix:
//!   RW+RW → second process fails with lock error
//!   RW+RO → both succeed (read-only can coexist with write)
//!   RO+RO → both succeed (multiple read-only can coexist)
//!
//! Uses s3_lock_holder via std::process::Command.

use std::process::{Command, Stdio};
use std::time::Duration;
use tempfile::TempDir;

/// Returns the path to the spike-ladybug Cargo.toml
fn manifest_path() -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("Cargo.toml")
        .to_str().unwrap()
        .to_string()
}

/// Spawn s3_lock_holder in background and immediately return.
/// Use this when you need the process to stay running.
fn spawn_lock_holder_background(
    path: &std::path::Path,
    mode: &str,
    hold_secs: u64,
) -> std::process::Child {
    let manifest = manifest_path();
    Command::new("cargo")
        .args([
            "run",
            "--quiet",
            "--manifest-path",
            &manifest,
            "--example",
            "s3_lock_holder",
            "--",
            "--mode",
            mode,
            "--path",
            path.to_str().unwrap(),
            "--hold-secs",
            &hold_secs.to_string(),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to spawn s3_lock_holder")
}

/// Spawn s3_lock_holder and wait for it to complete.
fn run_lock_holder(path: &std::path::Path, mode: &str, hold_secs: u64) -> std::process::Output {
    let manifest = manifest_path();
    Command::new("cargo")
        .args([
            "run",
            "--quiet",
            "--manifest-path",
            &manifest,
            "--example",
            "s3_lock_holder",
            "--",
            "--mode",
            mode,
            "--path",
            path.to_str().unwrap(),
            "--hold-secs",
            &hold_secs.to_string(),
        ])
        .output()
        .expect("Failed to run s3_lock_holder")
}

// =============================================================================
// Criterion 5: Cross-process file lock — RW+RW
// =============================================================================

#[test]
fn rw_rw_second_process_fails() {
    let tmp = TempDir::new().expect("tempdir");
    let db_path = tmp.path().join("s3_mp_rw_rw.lbdb");
    let path_str = db_path.to_str().unwrap();

    // First, create the database with RW mode (brief, just to create it)
    let manifest = manifest_path();
    let create_output = Command::new("cargo")
        .args([
            "run",
            "--manifest-path",
            &manifest,
            "--example",
            "s3_lock_holder",
            "--",
            "--mode",
            "rw",
            "--path",
            path_str,
            "--hold-secs",
            "1",
        ])
        .output()
        .expect("Failed to create initial DB");
    assert!(
        create_output.status.success(),
        "First RW open should succeed: {}",
        String::from_utf8_lossy(&create_output.stderr)
    );

    // Now spawn a long-running RW holder in background
    let mut holder = spawn_lock_holder_background(&db_path, "rw", 10);

    // Give it a moment to start and acquire the lock
    std::thread::sleep(Duration::from_millis(500));

    // Try to open the same DB as RW from another process
    // This should FAIL with a lock error
    let second_output = run_lock_holder(&db_path, "rw", 1);

    // Clean up background holder
    let _ = holder.kill();
    let _ = holder.wait();

    // Second RW open should FAIL
    assert!(
        !second_output.status.success(),
        "Second RW process should fail with lock error, got: {:?}",
        second_output.status
    );

    let stderr = String::from_utf8_lossy(&second_output.stderr);
    assert!(
        stderr.contains("Could not set lock"),
        "Expected 'Could not set lock' error, got: {}",
        stderr
    );
}

// =============================================================================
// Criterion 5: Cross-process file lock — RW+RO (coexist)
// =============================================================================

#[test]
fn rw_ro_both_succeed() {
    let tmp = TempDir::new().expect("tempdir");
    let db_path = tmp.path().join("s3_mp_rw_ro.lbdb");
    let path_str = db_path.to_str().unwrap();

    // First, create the database as RW
    let manifest = manifest_path();
    let output = Command::new("cargo")
        .args([
            "run",
            "--manifest-path",
            &manifest,
            "--example",
            "s3_lock_holder",
            "--",
            "--mode",
            "rw",
            "--path",
            path_str,
            "--hold-secs",
            "1",
        ])
        .output()
        .expect("Failed to create initial DB");
    assert!(
        output.status.success(),
        "First RW open should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Now spawn a long-running RW holder in background
    let mut holder = spawn_lock_holder_background(&db_path, "rw", 10);

    // Give it a moment to start
    std::thread::sleep(Duration::from_millis(500));

    // Try to open as RO while RW is held
    // This should SUCCEED because read-only can coexist with write
    let ro_output = run_lock_holder(&db_path, "ro", 1);

    // Clean up background holder
    let _ = holder.kill();
    let _ = holder.wait();

    assert!(
        ro_output.status.success(),
        "RO process should succeed even while RW is held: {}",
        String::from_utf8_lossy(&ro_output.stderr)
    );
}

// =============================================================================
// Criterion 5: Cross-process file lock — RO+RO (coexist)
// =============================================================================

#[test]
fn ro_ro_both_succeed() {
    let tmp = TempDir::new().expect("tempdir");
    let db_path = tmp.path().join("s3_mp_ro_ro.lbdb");
    let path_str = db_path.to_str().unwrap();

    // First, create the database as RW (can't create new DB in RO mode)
    let manifest = manifest_path();
    let output = Command::new("cargo")
        .args([
            "run",
            "--manifest-path",
            &manifest,
            "--example",
            "s3_lock_holder",
            "--",
            "--mode",
            "rw",
            "--path",
            path_str,
            "--hold-secs",
            "1",
        ])
        .output()
        .expect("Failed to create initial DB");
    assert!(
        output.status.success(),
        "First RW open should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Now spawn a long-running RO holder in background
    let mut holder = spawn_lock_holder_background(&db_path, "ro", 10);

    // Give it a moment to start
    std::thread::sleep(Duration::from_millis(500));

    // Try to open as RO again
    // Both should SUCCEED because multiple read-only can coexist
    let second_output = run_lock_holder(&db_path, "ro", 1);

    // Clean up background holder
    let _ = holder.kill();
    let _ = holder.wait();

    assert!(
        second_output.status.success(),
        "Second RO process should succeed: {}",
        String::from_utf8_lossy(&second_output.stderr)
    );
}
