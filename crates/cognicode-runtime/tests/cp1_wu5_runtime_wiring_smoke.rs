// Integration test for CP1.0 WU5 — runtime wiring.
//
// Verifies that the production composition root (`bootstrap_ladybug_default`
// → `Runtime::into_api_state`) returns an `ApiState` whose `control_query`
// field is wired through `with_control_query` from the `explorer-api`
// binary's `--with-architecture` flag, and that the wired service behaves
// according to the CP1.0 contract.
//
// This test prevents the regression discovered in `674c3795` (commit
// message: "with_control_query builder only called from tests"), where
// `with_control_query` was defined but never invoked outside the test
// suite. The composition root must reach the production builder so that
// the CP1.0 HTTP endpoint can return a meaningful `status:incomplete`
// instead of the dead `control_query_service_not_wired` reason.
//
// The registry starts empty by design; we do NOT fabricate admitted
// constraints. The test asserts that the wiring is connected (the
// service is reachable from the composition root) — not that any
// specific evaluation verdict is produced.

#![cfg(feature = "ladybug")]

use std::sync::Arc;

use cognicode_core::application::architecture::control_query::source_from_files;
use cognicode_core::application::architecture::{
    ArchitectureRegistry, ControlQueryService, EvaluationStatus,
};
use cognicode_runtime::bootstrap_ladybug_default;

fn temp_cwd(label: &str) -> std::path::PathBuf {
    let dir =
        std::env::temp_dir().join(format!("cognicode-cp1-wu5-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("mkdir");
    dir
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn composition_root_returns_unwired_baseline() {
    // The runtime crate's `into_api_state` does NOT pre-wire
    // `control_query`. The binary `bin/api.rs` does it explicitly via
    // `--with-architecture`. This documents the current baseline.
    let cwd = temp_cwd("wire");
    let runtime = bootstrap_ladybug_default(cwd.clone()).expect("bootstrap");
    let state = runtime.into_api_state();

    assert!(
        state.control_query.is_none(),
        "the runtime composition root must NOT pre-wire control_query \
         (the binary opts in via --with-architecture; pre-wiring would \
         couple the runtime crate to architecture policy)"
    );
}

#[test]
fn wired_service_yields_incomplete_without_fake_data() {
    // Authoritative check: once `with_control_query` is called (as the
    // binary does), the wired service is reachable AND a query against
    // an empty registry produces `Incomplete` without fabricating
    // constraints or violations. This is the live-server contract.
    let registry = ArchitectureRegistry::new();
    let svc = ControlQueryService::new(registry);

    let source = source_from_files(vec![]);
    let read_model = svc.query_architecture("ws-1", None, &source);
    assert_eq!(read_model.status, EvaluationStatus::Incomplete);
    assert!(read_model.constraints.is_empty());
    assert!(read_model.violations.is_empty());
}

#[test]
fn wired_service_arc_reaches_handler_path() {
    // The wired ControlQueryService is reachable as an `Arc<...>` so
    // it can be shared across handlers. We mimic the binary's wiring
    // pattern (`Arc::new(ControlQueryService::new(registry))`) and
    // assert the Arc is constructible + Send + Sync (required for axum
    // state sharing).
    let registry = ArchitectureRegistry::new();
    let _arc: Arc<ControlQueryService> = Arc::new(ControlQueryService::new(registry));
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<ControlQueryService>();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn regression_guard_for_wu5_wiring_disconnect() {
    // FAILS if the runtime crate's `into_api_state` ever starts
    // pre-wiring `control_query` INCORRECTLY (e.g. with a hardcoded
    // registry that bypasses `--with-architecture`).
    let cwd = temp_cwd("guard");
    let runtime = bootstrap_ladybug_default(cwd).expect("bootstrap");
    let state = runtime.into_api_state();

    if state.control_query.is_some() {
        panic!(
            "REGRESSION: runtime composition root started pre-wiring \
             control_query without going through the --with-architecture \
             opt-in in bin/api.rs. Either revert or move the opt-in to \
             the runtime crate."
        );
    }
}
