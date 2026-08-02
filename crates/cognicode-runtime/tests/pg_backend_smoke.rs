//! Smoke test for the `PgBackend` trait + `LadybugPgBackend` adapter
//! (first step of `e29-2-remove-pg`).

#![cfg(feature = "postgres")]

use std::sync::Arc;

use cognicode_core::domain::ports::{CallGraphStore, QualityStore, ViewSpecStore};
use cognicode_runtime::{LadybugPgBackend, PgBackend};

#[test]
fn pg_backend_trait_object_supports_ladybug_pg_backend() {
    // Verify that `LadybugPgBackend` can be used as a `&dyn PgBackend`
    // (the runtime will eventually use it through this trait object).
    let backend: Box<dyn PgBackend> = Box::new(LadybugPgBackend::new(
        None::<Arc<dyn QualityStore>>,
        None::<Arc<dyn ViewSpecStore>>,
        None::<Arc<dyn CallGraphStore>>,
    ));
    // All 3 port accessors return None (no ports wired in v1 — the
    // adapter is just a placeholder for the trait object).
    assert!(backend.quality_store().is_none());
    assert!(backend.view_spec_store().is_none());
    assert!(backend.call_graph_store().is_none());
}

#[test]
fn ladybug_pg_backend_implements_send_sync() {
    // Verify the trait bounds (required by the runtime's Arc<dyn ...>
    // usage).
    fn _assert_send<T: Send + Sync>() {}
    _assert::<LadybugPgBackend>();
    fn _assert<T: Send + Sync>() {}
    _assert::<Box<dyn PgBackend>>();
    let _ = _assert_send::<LadybugPgBackend>;
    let _ = _assert::<Box<dyn PgBackend>>;
}
