// Smoke test for the runtime's `ladybug` feature.
//
// Verifies that the `ladybug` feature compiles and exposes the
// lbug types via the cognicode-ladybug crate. Catches API drift
// if the runtime's `ladybug` feature is broken or the underlying
// types change.
//
// The v1 switch-default is a compile-time check only — actually
// switching the runtime default to `ladybug` requires migrating
// many call sites that depend on `PostgresRepository` directly.
// That work is tracked as `e29-2-remove-pg`.

#![cfg(feature = "ladybug")]

use std::path::PathBuf;
use std::sync::Arc;

#[test]
fn runtime_ladybug_feature_exposes_lbug_types() {
    let _: Option<Arc<cognicode_ladybug::LadybugStore>> = None;
    let _: Option<Arc<cognicode_ladybug::LadybugGraphExecutor>> = None;
}

#[test]
fn runtime_ladybug_feature_compiles() {
    // The `bootstrap` function should be callable with `ladybug`
    // feature (it's async and requires a runtime, so we just
    // verify the type signature compiles by taking a reference).
    fn _bootstrap_takes_path(p: PathBuf) {
        let _future = cognicode_runtime::Runtime::bootstrap(p, None);
    }
}
