//! Smoke test for `bootstrap_with_backend` — the canonical entry
//! point for the E29 v0.79+ runtime (when the full PG-removal
//! migration lands).
//!
//! v0.78.0 (this PR) only adds the signature + a delegating body.
//! The full implementation (runtime's port construction routed
//! through the backend) is a follow-up PR.

use cognicode_runtime::{bootstrap_with_backend, LadybugPgBackend, PgBackend};

#[test]
fn bootstrap_with_backend_signature_compiles() {
    // Verify the function is callable with the expected types.
    // We can't actually call it (it's async and requires a
    // runtime + filesystem), so we just take a reference.
    let _: fn(std::path::PathBuf, std::sync::Arc<dyn PgBackend>) -> _ = bootstrap_with_backend;
}

#[test]
fn ladybug_pg_backend_implements_pg_backend_for_bootstrap_with_backend() {
    // Verify that a LadybugPgBackend can be passed to
    // bootstrap_with_backend (compile-time check that the trait
    // bound is satisfied).
    let _backend: Box<dyn PgBackend> = Box::new(LadybugPgBackend::new(
        None::<std::sync::Arc<dyn cognicode_core::domain::ports::QualityStore>>,
        None::<std::sync::Arc<dyn cognicode_core::domain::ports::ViewSpecStore>>,
        None::<std::sync::Arc<dyn cognicode_core::domain::ports::CallGraphStore>>,
    ));
    // The dummy assignment above + the type coercion below prove
    // the trait object passes the bound.
    let _f: fn(std::path::PathBuf, std::sync::Arc<dyn PgBackend>) -> _ = bootstrap_with_backend;
}
