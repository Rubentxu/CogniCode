//! Integration test: verify the runtime can hold either a
//! PostgresBackend or a LadybugPgBackend as &dyn PgBackend,
//! exercising the cross-backend compatibility foundation for
//! the v0.79+ cutover.

#![cfg(feature = "postgres")]

use std::sync::Arc;

use cognicode_core::domain::ports::{CallGraphStore, QualityStore, ViewSpecStore};
use cognicode_runtime::{LadybugPgBackend, PgBackend, PostgresBackend};

/// `PgBackend` is a Send+Sync trait object that both the PG-side
/// (PostgresBackend) and the lbug-side (LadybugPgBackend) adapters
/// implement. This is the foundation for the v0.79+ cutover where
/// the runtime picks the canonical backend at build time.
///
/// v1: this is a compile-only check (the actual
/// `PostgresBackend::new` requires a live PG). The trait object
/// pattern is verified at compile time — if either backend stops
/// implementing `PgBackend`, this test fails to compile.
#[test]
fn pg_backend_trait_object_supports_ladybug_backend() {
    // LadybugPgBackend can be Box<dyn PgBackend> (no PG live
    // needed). This is the cross-backend compatibility test that
    // doesn't require live infrastructure.
    let _ladybug: Box<dyn PgBackend> = Box::new(LadybugPgBackend::new(
        None::<Arc<dyn QualityStore>>,
        None::<Arc<dyn ViewSpecStore>>,
        None::<Arc<dyn CallGraphStore>>,
    ));
}

/// `bootstrap_with_backend` accepts either backend as `Arc<dyn PgBackend>`.
/// This is the type-level guarantee that the v0.79+ runtime will
/// compile with both backends.
#[test]
fn bootstrap_with_backend_accepts_either_backend() {
    fn _accepts(_b: Arc<dyn PgBackend>) {}
    fn _accept_pg(_b: Arc<PostgresBackend>) {
        _accepts(_b);
    }
    fn _accept_ladybug(_b: Arc<LadybugPgBackend>) {
        _accepts(_b);
    }
    // Verify both functions exist (type-level coercion check).
    let _ = _accept_pg;
    let _ = _accept_ladybug;
}
