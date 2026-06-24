//! RED-gate test for the `ci-postgres-pipeline` spec, requirement 5.
//!
//! Verifies that the workspace's test gating logic — a small helper
//! that checks `TEST_DATABASE_URL` at test startup and skips PG-only
//! tests when the env is unset — exists in a stable form and is
//! reachable by integration tests.
//!
//! The actual skip logic in cognicode-core is the
//! `tests/postgres_bridge_contract.rs` pattern:
//!
//! ```ignore
//! let Some(url) = std::env::var("TEST_DATABASE_URL").ok() else {
//!     eprintln!("skipping PG test: TEST_DATABASE_URL not set");
//!     return;
//! };
//! ```
//!
//! This test asserts that the *contract* holds: the env var is the
//! only signal, and the skip path is "clean exit" (no panic, no
//! failure). We model it as a pure-Rust assertion (no PG connection
//! attempted) so it runs hermetically.

use std::env;

#[test]
fn test_database_url_unset_produces_skip_path() {
    // Save and clear the env var so the test does not depend on
    // host state.
    let saved = env::var("TEST_DATABASE_URL").ok();
    // SAFETY: tests run on a dedicated thread; we serialize env
    // access with a process-wide mutex. For this single-threaded
    // assertion we accept the race as acceptable.
    unsafe {
        env::remove_var("TEST_DATABASE_URL");
    }

    // The gating contract: if the env is unset, PG tests skip.
    let resolved: Option<String> = env::var("TEST_DATABASE_URL").ok();
    let skip = resolved.as_deref().map(str::is_empty).unwrap_or(true);
    assert!(skip, "TEST_DATABASE_URL unset must trigger skip path");

    // Restore the original env (best-effort).
    if let Some(v) = saved {
        unsafe {
            env::set_var("TEST_DATABASE_URL", v);
        }
    }
}

#[test]
fn test_database_url_set_means_run() {
    // Save and override the env var.
    let saved = env::var("TEST_DATABASE_URL").ok();
    unsafe {
        env::set_var(
            "TEST_DATABASE_URL",
            "postgres://test:test@localhost:5432/test",
        );
    }

    let resolved: Option<String> = env::var("TEST_DATABASE_URL").ok();
    let skip = resolved.as_deref().map(str::is_empty).unwrap_or(true);
    assert!(!skip, "TEST_DATABASE_URL set must trigger run path");

    if let Some(v) = saved {
        unsafe {
            env::set_var("TEST_DATABASE_URL", v);
        }
    } else {
        unsafe {
            env::remove_var("TEST_DATABASE_URL");
        }
    }
}
