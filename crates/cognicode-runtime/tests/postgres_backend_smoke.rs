//! Smoke test for `PostgresBackend` — the postgres-feature adapter
//! for the `PgBackend` trait. Complements the lbug-feature adapter
//! (`LadybugPgBackend`, PR #205) and the entry point
//! (`bootstrap_with_backend`, PR #206).
//!
//! v1: the runtime still uses `PostgresRepository` directly in its
//! bootstrap. `PostgresBackend::new(repo)` is the bridge that
//! wraps the existing repo for the `PgBackend` trait. The full
//! migration of bootstrap to use `&dyn PgBackend` is a follow-up
//! PR.

#![cfg(feature = "postgres")]

use std::sync::Arc;

use cognicode_runtime::{PgBackend, PostgresBackend};

#[test]
fn postgres_backend_struct_compiles_with_pg_repo_field() {
    // Verify that PostgresBackend can wrap a PostgresRepository and
    // implement PgBackend. This catches API drift between the
    // existing PG types and the new trait surface.
    fn _assert_pg_backend(b: &dyn PgBackend) {
        let _ = b.quality_store();
        let _ = b.view_spec_store();
        let _ = b.call_graph_store();
    }
    // PostgresBackend implements PgBackend — verify via trait
    // object coercion.
    let _check: fn(&PostgresBackend) = |b| {
        _assert_pg_backend(b);
    };
}

#[test]
fn postgres_backend_returns_none_for_v1_ports() {
    // v1 of PostgresBackend returns None for the 3 port accessors
    // (the ports are still constructed by the bootstrap directly).
    // This test pins that behavior — a future PR that populates
    // the ports will update this test.
    let repo = match create_test_pg_repo() {
        Some(r) => r,
        None => return, // skip if no PG live
    };
    let backend = PostgresBackend::new(repo);
    assert!(backend.quality_store().is_none());
    assert!(backend.view_spec_store().is_none());
    assert!(backend.call_graph_store().is_none());
}

/// Try to create a test `PostgresRepository` against a local PG.
/// Returns `None` if the PG is not reachable (so this test skips
/// cleanly in sandbox / non-PG environments).
fn create_test_pg_repo(
) -> Option<Arc<cognicode_core::infrastructure::persistence::PostgresRepository>> {
    let url = std::env::var("TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://invalid:9999/nonexistent".to_string());
    futures_block_on(async move {
        cognicode_core::infrastructure::persistence::PostgresRepository::new(&url)
            .await
            .ok()
            .map(Arc::new)
    })
    .flatten()
}

/// Tiny sync helper for the async `PostgresRepository::connect`.
fn futures_block_on<F: std::future::Future>(future: F) -> Option<F::Output> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .ok()?;
    Some(rt.block_on(future))
}
