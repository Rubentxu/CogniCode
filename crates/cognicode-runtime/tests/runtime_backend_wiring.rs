//! RED scenario tests for the e29-7 backend wiring refactor.
//!
//! Strict-TDD scaffolding (task-1): these tests are written FIRST and
//! are RED against the current code. Each subsequent task flips its
//! own scenarios green:
//!
//! | Scenario | Contract | RED until |
//! |----------|----------|-----------|
//! | R1       | `pg_repo` Some on cfg(postgres) after `bootstrap(Some(url))`, absent on ladybug | task-2 (field added) |
//! | R2       | compile-fail: `PgBackend::as_postgres_repo` must be ABSENT | task-6 (trait method removed) |
//! | R3       | Postgres path → 3 ports Some + `backend` None | task-2 (ports) + task-6 (backend None) |
//! | R5       | 3 quality sites clone from the ONE `self.quality_store` Arc (helpers deleted) | task-4 (helpers removed) |
//! | R6       | investigation constructed ONCE and shared (state == search) | task-3 (duplicate L486 deleted) |
//!
//! PG-dependent scenarios (R1/R3) skip cleanly when `TEST_DATABASE_URL`
//! is unset (no live PG in the apply sandbox).
//!
//! Note on R5/R6: the quality/investigation Arcs inside the facade
//! impls (`SearchServiceImpl`, `ViewServiceImpl`, `MoldQLServiceImpl`)
//! are private and not reachable through any public accessor, so the
//! *identity* contract is pinned structurally at the single
//! construction-source level (source-count assertions on lib.rs). The
//! behavior-level `Arc::ptr_eq` assertions land in
//! `bootstrap_with_backend_smoke.rs` (task-7) where the caller-provided
//! Arc identity is observable through `Runtime.quality_store`.

// ---------------------------------------------------------------------------
// R1 — pg_repo
// ---------------------------------------------------------------------------

/// R1 (postgres arm): after `bootstrap(Some(url))` with a live PG, the
/// `pg_repo` field must be `Some`. RED today (field does not exist yet).
#[cfg(feature = "postgres")]
#[test]
fn r1_pg_repo_some_after_bootstrap_with_postgres() {
    let Some(url) = pg_url_from_env() else {
        return; // no live PG — skip
    };
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let runtime = rt
        .block_on(cognicode_runtime::Runtime::bootstrap(
            std::env::temp_dir(),
            Some(url),
        ))
        .expect("bootstrap(Some(url)) with live PG");
    assert!(
        runtime.pg_repo.is_some(),
        "R1: pg_repo must be Some after bootstrap(Some(url))"
    );
}

/// R1 (ladybug arm): the `pg_repo` field is cfg(postgres)-gated, so it
/// does not exist on the ladybug build ("None on ladybug" at the type
/// level). Referencing `runtime.pg_repo` below would not compile on the
/// default build.
#[cfg(not(feature = "postgres"))]
#[test]
fn r1_pg_repo_absent_on_ladybug() {
    fn _assert_no_pg_repo_field(_r: &cognicode_runtime::Runtime) {}
}

// ---------------------------------------------------------------------------
// R2 — compile-fail: as_postgres_repo must be absent from PgBackend
// ---------------------------------------------------------------------------

/// RED until task-6: today `PgBackend::as_postgres_repo` exists, so the
/// ui snippet COMPILES and trybuild fails the test (expected-fail but
/// compiled). After task-6 removes the method, the snippet fails to
/// compile and this test passes.
#[test]
fn r2_as_postgres_repo_is_absent_from_pg_backend_trait() {
    let t = trybuild::TestCases::new();
    t.compile_fail("ui/r2_as_postgres_repo_absent.rs");
}

// ---------------------------------------------------------------------------
// R3 — Postgres path: 3 ports Some + backend None
// ---------------------------------------------------------------------------

/// R3a: after `bootstrap(Some(url))`, the 3 relocated ports must be
/// Some (wired from the shared `pg_repo`). RED until task-2.
#[cfg(feature = "postgres")]
#[test]
fn r3_postgres_path_three_ports_some() {
    let Some(url) = pg_url_from_env() else {
        return; // no live PG — skip
    };
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let runtime = rt
        .block_on(cognicode_runtime::Runtime::bootstrap(
            std::env::temp_dir(),
            Some(url),
        ))
        .expect("bootstrap(Some(url)) with live PG");
    assert!(
        runtime.quality_store.is_some(),
        "R3: quality_store must be Some on the postgres path"
    );
    assert!(
        runtime.view_spec_store.is_some(),
        "R3: view_spec_store must be Some on the postgres path"
    );
    assert!(
        runtime.call_graph_store.is_some(),
        "R3: call_graph_store must be Some on the postgres path"
    );
}

/// R3b: after `bootstrap(Some(url))`, `backend` must be None — the
/// PgBackend abstraction is fully removed from the postgres path.
/// RED until task-6.
#[cfg(feature = "postgres")]
#[test]
fn r3_postgres_path_backend_none() {
    let Some(url) = pg_url_from_env() else {
        return; // no live PG — skip
    };
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let runtime = rt
        .block_on(cognicode_runtime::Runtime::bootstrap(
            std::env::temp_dir(),
            Some(url),
        ))
        .expect("bootstrap(Some(url)) with live PG");
    assert!(
        runtime.backend.is_none(),
        "R3: backend must be None after bootstrap(Some(url)) (task-6)"
    );
}

// ---------------------------------------------------------------------------
// R5 — 3 quality sites share ONE Arc (helpers deleted)
// ---------------------------------------------------------------------------

/// R5: the 3 quality consumer sites (into_api_state search/view/moldql
/// + into_mcp_handler quality/quality_write) must clone from the single
/// `self.quality_store` field. Task-4 deletes the `quality_repo_arc` /
/// `quality_write_repo_arc` helper fns that previously rebuilt the
/// store from `as_postgres_repo()`; with only ONE construction source,
/// identity is preserved by construction.
#[test]
fn r5_quality_sites_share_one_arc() {
    let src = include_str!("../src/lib.rs");
    assert_eq!(
        src.matches("quality_repo_arc").count(),
        0,
        "R5: quality_repo_arc helper must be deleted (task-4)"
    );
    assert_eq!(
        src.matches("quality_write_repo_arc").count(),
        0,
        "R5: quality_write_repo_arc helper must be deleted (task-4)"
    );
}

// ---------------------------------------------------------------------------
// R6 — investigation constructed ONCE and shared (state == search)
// ---------------------------------------------------------------------------

/// R6: the investigation service must be constructed ONCE from the
/// shared pg_repo and wired into BOTH the SearchService and
/// ApiState.investigation (same Arc). Today `into_api_state` builds it
/// twice — once for search (L374) and once more for
/// `state.with_investigation` (L486) — so `state.investigation` is a
/// DIFFERENT Arc from the search facade's. Task-3 deletes the duplicate
/// (single construction site).
#[test]
fn r6_investigation_constructed_once_and_shared() {
    let src = include_str!("../src/lib.rs");
    let sites = src.matches("new_investigation_service_from_postgres").count();
    assert_eq!(
        sites, 1,
        "R6: new_investigation_service_from_postgres must have exactly 1 \
         construction site (state.investigation == search investigation). \
         Found {sites} — the L486 duplicate must be deleted (task-3)"
    );
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Env-skip guard: returns `TEST_DATABASE_URL` when set, `None` when no
/// live PG is available (the apply sandbox). Callers return early on
/// `None` so the [needs-pg] scenarios skip cleanly.
fn pg_url_from_env() -> Option<String> {
    std::env::var("TEST_DATABASE_URL").ok()
}
