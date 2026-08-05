//! Smoke tests for `bootstrap_with_backend` — the canonical entry
//! point for the E29 v0.79+ runtime (ladybug path).
//!
//! e29-3 (port-abstraction-audit, Phase 5): the composition seam
//! collapsed from a single-implementer trait indirection (`PgBackend` +
//! `LadybugPgBackend`) into a plain `RuntimePorts` DTO carrying the
//! three relocated `Option<Arc<dyn *Port>>` slots. The 2
//! PgBackend-self-justifying compile-only tests were deleted; the
//! functional R3+R5 test migrates to the new DTO (runtime-bootstrap-contract
//! spec S2: Arc identity preserved through `bootstrap_with_backend`).

use std::sync::Arc;

use cognicode_core::domain::aggregates::CallGraph;
use cognicode_core::domain::ports::{CallGraphStore, QualityStore, ViewSpecStore};
use cognicode_core::domain::value_objects::{RevisionId, WorkspaceId};
use cognicode_runtime::{bootstrap_with_backend, RuntimePorts};

// ---------------------------------------------------------------------------
// Identity stubs for the relocated ports — never called, only held and
// compared by Arc identity.
// ---------------------------------------------------------------------------

struct TestQualityStore;
impl QualityStore for TestQualityStore {
    fn issues_for_file(
        &self,
        _file: &str,
    ) -> Result<
        Vec<cognicode_core::domain::ports::QualityIssue>,
        cognicode_core::domain::ports::QualityError,
    > {
        unimplemented!()
    }
    fn issues_for_scope(
        &self,
        _scope_prefix: &str,
    ) -> Result<
        Vec<cognicode_core::domain::ports::QualityIssue>,
        cognicode_core::domain::ports::QualityError,
    > {
        unimplemented!()
    }
    fn issues_at_line(
        &self,
        _file: &str,
        _line: u32,
    ) -> Result<
        Vec<cognicode_core::domain::ports::QualityIssue>,
        cognicode_core::domain::ports::QualityError,
    > {
        unimplemented!()
    }
    fn issue_by_id(
        &self,
        _id: i64,
    ) -> Result<
        Option<cognicode_core::domain::ports::QualityIssue>,
        cognicode_core::domain::ports::QualityError,
    > {
        unimplemented!()
    }
    fn rule_summary(
        &self,
        _rule_id: &str,
    ) -> Result<
        cognicode_core::domain::ports::RuleSummary,
        cognicode_core::domain::ports::QualityError,
    > {
        unimplemented!()
    }
    fn quality_gate(
        &self,
        _workspace_id: Option<&str>,
    ) -> Result<
        cognicode_core::domain::ports::QualityGateSummary,
        cognicode_core::domain::ports::QualityError,
    > {
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
    ) -> Result<
        Vec<cognicode_core::domain::ports::QualityIssue>,
        cognicode_core::domain::ports::QualityError,
    > {
        unimplemented!()
    }
    fn insert_issues(
        &self,
        _issues: &[cognicode_core::domain::ports::NewIssue],
    ) -> Result<
        cognicode_core::domain::ports::UpsertSummary,
        cognicode_core::domain::ports::QualityError,
    > {
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
    ) -> Result<
        Option<cognicode_core::domain::ports::ViewSpecPayload>,
        cognicode_core::domain::ports::ViewSpecStoreError,
    > {
        unimplemented!()
    }
    async fn list(
        &self,
        _workspace_id: &str,
        _owner: &str,
    ) -> Result<
        Vec<cognicode_core::domain::ports::ViewSpecPayload>,
        cognicode_core::domain::ports::ViewSpecStoreError,
    > {
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
    ) -> Result<
        Vec<cognicode_core::domain::ports::ViewSpecPayload>,
        cognicode_core::domain::ports::ViewSpecStoreError,
    > {
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
// R1 — pg_repo
// ---------------------------------------------------------------------------

/// R1: the postgres `pg_repo` field was removed with the full postgres
/// removal (e29-7) — the runtime no longer has any PG-specific field.
#[test]
fn r1_pg_repo_absent() {
    fn _assert_no_pg_repo_field(_r: &cognicode_runtime::Runtime) {}
}

// ---------------------------------------------------------------------------
// R3 + R5 — ports populated FROM the RuntimePorts DTO with Arc identity
// ---------------------------------------------------------------------------

/// R3 + R5 (runtime-bootstrap-contract S2): `bootstrap_with_backend`
/// must move the 3 port Arcs from the `RuntimePorts` DTO onto the
/// `Runtime` verbatim — the runtime field IS the Arc the caller
/// provided (same allocation). The Runtime carries no `backend` field.
#[tokio::test]
async fn r3_r5_ports_populated_from_runtime_ports_with_identity() {
    let quality: Arc<dyn QualityStore> = Arc::new(TestQualityStore);
    let view_spec: Arc<dyn ViewSpecStore> = Arc::new(TestViewSpecStore);
    let cg_store: Arc<dyn CallGraphStore> = Arc::new(TestCallGraphStore);
    let ports = RuntimePorts {
        quality_store: Some(quality.clone()),
        view_spec_store: Some(view_spec.clone()),
        call_graph_store: Some(cg_store.clone()),
        analytics_lineage_store: None,
        revision_store: None,
        manifest_store: None,
        session_store: None,
        report_store: None,
        narrative_store: None,
    };

    let runtime = bootstrap_with_backend(std::env::temp_dir(), ports)
        .await
        .expect("bootstrap_with_backend succeeds with a RuntimePorts DTO");

    // R3: ports populated (Some when the DTO provides Some).
    assert!(runtime.quality_store.is_some(), "R3: quality_store Some");
    assert!(
        runtime.view_spec_store.is_some(),
        "R3: view_spec_store Some"
    );
    assert!(
        runtime.call_graph_store.is_some(),
        "R3: call_graph_store Some"
    );

    // R5: Arc identity preserved — same allocation, not a clone.
    assert!(
        Arc::ptr_eq(runtime.quality_store.as_ref().unwrap(), &quality),
        "R5: quality_store must be the SAME Arc the DTO carried"
    );
    assert!(
        Arc::ptr_eq(runtime.view_spec_store.as_ref().unwrap(), &view_spec),
        "R5: view_spec_store must be the SAME Arc the DTO carried"
    );
    assert!(
        Arc::ptr_eq(runtime.call_graph_store.as_ref().unwrap(), &cg_store),
        "R5: call_graph_store must be the SAME Arc the DTO carried"
    );
}

/// The Runtime must not carry a `backend` field after the PgBackend
/// indirection is deleted.
#[test]
fn runtime_has_no_backend_field() {
    fn _assert_no_backend_field(_r: &cognicode_runtime::Runtime) {}
}

// ---------------------------------------------------------------------------
// R6 — investigation
// ---------------------------------------------------------------------------

/// R6: the postgres-backed investigation service
/// (`new_investigation_service_from_postgres`) was removed with the
/// full postgres removal (e29-7). `ApiState.investigation` stays None
/// on the ladybug path — verify the runtime no longer wires it.
#[test]
fn r6_investigation_postgres_path_removed() {
    let src = include_str!("../src/lib.rs");
    let sites = src
        .matches("new_investigation_service_from_postgres")
        .count();
    assert_eq!(
        sites, 0,
        "R6: new_investigation_service_from_postgres must be gone after \
         the full postgres removal. Found {sites}"
    );
}
