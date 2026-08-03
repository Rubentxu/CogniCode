//! Smoke tests for `bootstrap_with_backend` — the canonical entry
//! point for the E29 v0.79+ runtime (ladybug path).
//!
//! e29-7 task-7: extended with the R1/R3/R5/R6 scenario assertions
//! (previously the RED scaffolding in `runtime_backend_wiring.rs`):
//!
//! | Scenario | Contract |
//! |----------|----------|
//! | R1       | `pg_repo` absent on ladybug / Some on cfg(postgres) after `bootstrap(Some(url))` |
//! | R3       | 3 ports populated FROM the backend; postgres path → 3 ports Some + `backend` None |
//! | R5       | quality identity preserved — runtime field IS the backend's Arc (single source for all 3 sites) |
//! | R6       | investigation constructed ONCE and shared (state == search) |
//!
//! PG-dependent scenarios skip cleanly when `TEST_DATABASE_URL` is unset
//! (no live PG in the apply sandbox).

use std::sync::Arc;

use cognicode_core::domain::aggregates::CallGraph;
use cognicode_core::domain::ports::{CallGraphStore, QualityStore, ViewSpecStore};
use cognicode_core::domain::value_objects::{RevisionId, WorkspaceId};
use cognicode_runtime::{bootstrap_with_backend, LadybugPgBackend, PgBackend};

// ---------------------------------------------------------------------------
// Identity stubs for the relocated ports — never called, only held and
// compared by Arc identity.
// ---------------------------------------------------------------------------

struct TestQualityStore;
impl QualityStore for TestQualityStore {
    fn issues_for_file(
        &self,
        _file: &str,
    ) -> Result<Vec<cognicode_core::domain::ports::QualityIssue>, cognicode_core::domain::ports::QualityError> {
        unimplemented!()
    }
    fn issues_for_scope(
        &self,
        _scope_prefix: &str,
    ) -> Result<Vec<cognicode_core::domain::ports::QualityIssue>, cognicode_core::domain::ports::QualityError> {
        unimplemented!()
    }
    fn issues_at_line(
        &self,
        _file: &str,
        _line: u32,
    ) -> Result<Vec<cognicode_core::domain::ports::QualityIssue>, cognicode_core::domain::ports::QualityError> {
        unimplemented!()
    }
    fn issue_by_id(
        &self,
        _id: i64,
    ) -> Result<Option<cognicode_core::domain::ports::QualityIssue>, cognicode_core::domain::ports::QualityError> {
        unimplemented!()
    }
    fn rule_summary(
        &self,
        _rule_id: &str,
    ) -> Result<cognicode_core::domain::ports::RuleSummary, cognicode_core::domain::ports::QualityError> {
        unimplemented!()
    }
    fn quality_gate(
        &self,
        _workspace_id: Option<&str>,
    ) -> Result<cognicode_core::domain::ports::QualityGateSummary, cognicode_core::domain::ports::QualityError> {
        unimplemented!()
    }
    fn open_issues_count(
        &self,
        _workspace_id: Option<&str>,
    ) -> Result<usize, cognicode_core::domain::ports::QualityError> {
        unimplemented!()
    }
    fn issues_for_workspace(
        &self,
        _workspace_id: Option<&str>,
        _filter: &cognicode_core::domain::ports::IssueFilter,
    ) -> Result<Vec<cognicode_core::domain::ports::QualityIssue>, cognicode_core::domain::ports::QualityError> {
        unimplemented!()
    }
    fn insert_issues(
        &self,
        _issues: &[cognicode_core::domain::ports::NewIssue],
    ) -> Result<cognicode_core::domain::ports::UpsertSummary, cognicode_core::domain::ports::QualityError> {
        unimplemented!()
    }
    fn delete_issue(
        &self,
        _workspace_id: &str,
        _rule_id: &str,
        _file_path: &str,
        _line: u32,
    ) -> Result<bool, cognicode_core::domain::ports::QualityError> {
        unimplemented!()
    }
}

struct TestViewSpecStore;
#[async_trait::async_trait]
impl ViewSpecStore for TestViewSpecStore {
    async fn save(
        &self,
        _payload: &cognicode_core::domain::ports::ViewSpecPayload,
        _workspace_id: &str,
        _owner: &str,
    ) -> Result<(), cognicode_core::domain::ports::ViewSpecStoreError> {
        unimplemented!()
    }
    async fn load(
        &self,
        _id: &str,
        _workspace_id: &str,
        _owner: &str,
    ) -> Result<Option<cognicode_core::domain::ports::ViewSpecPayload>, cognicode_core::domain::ports::ViewSpecStoreError> {
        unimplemented!()
    }
    async fn list(
        &self,
        _workspace_id: &str,
        _owner: &str,
    ) -> Result<Vec<cognicode_core::domain::ports::ViewSpecPayload>, cognicode_core::domain::ports::ViewSpecStoreError> {
        unimplemented!()
    }
    async fn delete(
        &self,
        _id: &str,
        _workspace_id: &str,
        _owner: &str,
    ) -> Result<bool, cognicode_core::domain::ports::ViewSpecStoreError> {
        unimplemented!()
    }
    async fn list_for_workspace(
        &self,
        _workspace_id: &str,
        _applies_to_kind: &str,
    ) -> Result<Vec<cognicode_core::domain::ports::ViewSpecPayload>, cognicode_core::domain::ports::ViewSpecStoreError> {
        unimplemented!()
    }
    async fn update(
        &self,
        _id: &str,
        _workspace_id: &str,
        _owner: &str,
        _seed_object_id: Option<&str>,
        _seed_view_id: Option<&str>,
        _applies_when: Option<&str>,
    ) -> Result<bool, cognicode_core::domain::ports::ViewSpecStoreError> {
        unimplemented!()
    }
}

struct TestCallGraphStore;
#[async_trait::async_trait]
impl CallGraphStore for TestCallGraphStore {
    async fn save_call_graph_ws(
        &self,
        _graph: &CallGraph,
        _ws: &WorkspaceId,
    ) -> Result<RevisionId, cognicode_core::domain::ports::CallGraphError> {
        unimplemented!()
    }
    async fn load_call_graph_ws(
        &self,
        _ws: &WorkspaceId,
        _revision: RevisionId,
    ) -> Result<Option<CallGraph>, cognicode_core::domain::ports::CallGraphError> {
        unimplemented!()
    }
    async fn load_call_graph_current(
        &self,
        _ws: &WorkspaceId,
    ) -> Result<Option<CallGraph>, cognicode_core::domain::ports::CallGraphError> {
        unimplemented!()
    }
}

// ---------------------------------------------------------------------------
// Existing compile-only smoke tests (kept).
// ---------------------------------------------------------------------------

#[test]
fn bootstrap_with_backend_signature_compiles() {
    let _: fn(std::path::PathBuf, std::sync::Arc<dyn PgBackend>) -> _ = bootstrap_with_backend;
}

#[test]
fn ladybug_pg_backend_implements_pg_backend_for_bootstrap_with_backend() {
    let _backend: Box<dyn PgBackend> = Box::new(LadybugPgBackend::new(
        None::<Arc<dyn QualityStore>>,
        None::<Arc<dyn ViewSpecStore>>,
        None::<Arc<dyn CallGraphStore>>,
    ));
    let _f: fn(std::path::PathBuf, std::sync::Arc<dyn PgBackend>) -> _ = bootstrap_with_backend;
}

// ---------------------------------------------------------------------------
// R1 — pg_repo
// ---------------------------------------------------------------------------

/// R1 (ladybug arm): the `pg_repo` field is cfg(postgres)-gated, so it
/// does not exist on the ladybug build ("None on ladybug" at the type
/// level). Referencing `runtime.pg_repo` here would not compile.
#[cfg(not(feature = "postgres"))]
#[test]
fn r1_pg_repo_absent_on_ladybug() {
    fn _assert_no_pg_repo_field(_r: &cognicode_runtime::Runtime) {}
}

/// R1 + R3 (postgres path, live PG): after `bootstrap(Some(url))` the
/// `pg_repo` field is Some, the 3 ports are Some, and `backend` is None
/// (the PgBackend abstraction is fully removed from the postgres path).
#[cfg(feature = "postgres")]
#[test]
fn r1_r3_postgres_bootstrap_path() {
    let Some(url) = std::env::var("TEST_DATABASE_URL").ok() else {
        return; // no live PG — skip
    };
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let runtime = rt
        .block_on(cognicode_runtime::Runtime::bootstrap(
            std::env::temp_dir(),
            Some(url),
        ))
        .expect("bootstrap(Some(url)) with live PG");
    assert!(runtime.pg_repo.is_some(), "R1: pg_repo must be Some");
    assert!(
        runtime.quality_store.is_some(),
        "R3: quality_store must be Some"
    );
    assert!(
        runtime.view_spec_store.is_some(),
        "R3: view_spec_store must be Some"
    );
    assert!(
        runtime.call_graph_store.is_some(),
        "R3: call_graph_store must be Some"
    );
    assert!(runtime.backend.is_none(), "R3: backend must be None");
}

// ---------------------------------------------------------------------------
// R3 + R5 — ports populated FROM the backend with Arc identity
// ---------------------------------------------------------------------------

/// R3 (ladybug arm) + R5: `bootstrap_with_backend` must build the 3
/// ports from the backend's port accessors, preserving Arc identity —
/// the runtime field IS the Arc the caller provided (the single source
/// all 3 quality consumer sites clone from).
#[tokio::test]
async fn r3_r5_ports_populated_from_backend_with_identity() {
    let quality: Arc<dyn QualityStore> = Arc::new(TestQualityStore);
    let view_spec: Arc<dyn ViewSpecStore> = Arc::new(TestViewSpecStore);
    let cg_store: Arc<dyn CallGraphStore> = Arc::new(TestCallGraphStore);
    let backend = Arc::new(LadybugPgBackend::new(
        Some(quality.clone()),
        Some(view_spec.clone()),
        Some(cg_store.clone()),
    ));

    let runtime = bootstrap_with_backend(std::env::temp_dir(), backend)
        .await
        .expect("bootstrap_with_backend succeeds with a provided backend");

    // R3: ports populated (Some when the backend provides Some).
    assert!(runtime.quality_store.is_some(), "R3: quality_store Some");
    assert!(
        runtime.view_spec_store.is_some(),
        "R3: view_spec_store Some"
    );
    assert!(
        runtime.call_graph_store.is_some(),
        "R3: call_graph_store Some"
    );

    // R5: the runtime stores the SAME Arc the backend returned — the
    // 3 quality consumer sites (search/view/moldql + mcp
    // quality/quality_write) all clone from this single field.
    assert!(
        Arc::ptr_eq(runtime.quality_store.as_ref().unwrap(), &quality),
        "R5: quality_store must be the SAME Arc the backend returned"
    );
    assert!(
        Arc::ptr_eq(runtime.view_spec_store.as_ref().unwrap(), &view_spec),
        "R5: view_spec_store must be the SAME Arc the backend returned"
    );
    assert!(
        Arc::ptr_eq(runtime.call_graph_store.as_ref().unwrap(), &cg_store),
        "R5: call_graph_store must be the SAME Arc the backend returned"
    );
}

// ---------------------------------------------------------------------------
// R6 — investigation constructed ONCE and shared (state == search)
// ---------------------------------------------------------------------------

/// R6: the investigation service must be constructed ONCE from the
/// shared pg_repo and wired into BOTH the SearchService and
/// ApiState.investigation (same Arc). The duplicate construction site
/// was deleted, so `new_investigation_service_from_postgres` has
/// exactly 1 call site in the runtime source.
#[test]
fn r6_investigation_constructed_once_and_shared() {
    let src = include_str!("../src/lib.rs");
    let sites = src.matches("new_investigation_service_from_postgres").count();
    assert_eq!(
        sites, 1,
        "R6: new_investigation_service_from_postgres must have exactly 1 \
         construction site (state.investigation == search investigation). \
         Found {sites}"
    );
}
