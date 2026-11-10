//! Characterization of the boundary between durable graph identity and context.
//!
//! The graph snapshot is keyed by the analyzer and the source manifest. A
//! consumer profile, an execution budget, and a findings scope are different
//! questions. This test keeps those questions separate before any cache-key
//! migration is considered.

use cognicode_core::application::analysis_basis::{
    AnalysisBasis, AnalysisOp, AnalysisValue, Analyze, Budget, ConsumerProfile, GraphSource,
    OutcomeState,
};
use cognicode_core::application::consumer_analysis::ServiceAnalyzer;
use cognicode_core::domain::execution::AnalysisScope;
use cognicode_core::domain::kernel_ids::SnapshotId;
use cognicode_core::domain::value_objects::{
    AnalyzerBuild, SNAPSHOT_SCHEMA, SnapshotKey, WorkspaceId,
};
use tempfile::TempDir;

fn fixture() -> TempDir {
    let root = tempfile::tempdir().expect("fixture tempdir");
    std::fs::create_dir_all(root.path().join("src")).expect("fixture src");
    std::fs::write(
        root.path().join("src/lib.rs"),
        "pub fn answer() -> u32 { helper() }\nfn helper() -> u32 { 42 }\n",
    )
    .expect("fixture source");
    root
}

fn graph_for(
    basis: &AnalysisBasis,
) -> (
    cognicode_core::domain::aggregates::CallGraph,
    OutcomeState,
    Option<GraphSource>,
) {
    let analyzer = ServiceAnalyzer::new();
    let outcome = analyzer.execute(
        basis,
        AnalysisOp::GraphFull {
            path: basis.workspace_root().to_path_buf(),
            rebuild: true,
        },
    );
    let graph = analyzer.service().get_project_graph().as_ref().clone();
    let source = outcome.provenance().graph_source;
    let state = outcome.state();
    assert!(matches!(outcome.value(), Some(AnalysisValue::Graph(_))));
    (graph, state, source)
}

#[test]
fn consumer_profile_and_budget_do_not_change_the_durable_graph() {
    let root = fixture();
    let human = AnalysisBasis::for_workspace(root.path().to_path_buf());
    let agent_limited = human
        .clone()
        .with_profile(ConsumerProfile::Agent)
        .with_budget(Budget {
            max_elapsed: None,
            max_entities: Some(1),
        });

    let (human_graph, human_state, human_source) = graph_for(&human);
    let (agent_graph, agent_state, agent_source) = graph_for(&agent_limited);

    assert_eq!(human_graph, agent_graph);
    assert_eq!(human_state, OutcomeState::Complete);
    assert_eq!(agent_state, OutcomeState::Complete);
    assert_eq!(human_source, Some(GraphSource::Built));
    assert_eq!(agent_source, Some(GraphSource::Built));

    let key_a = SnapshotKey::new(
        root.path().to_string_lossy(),
        AnalyzerBuild::current(),
        SNAPSHOT_SCHEMA,
    );
    let key_b = SnapshotKey::new(
        root.path().to_string_lossy(),
        AnalyzerBuild::current(),
        SNAPSHOT_SCHEMA,
    );
    assert_eq!(key_a, key_b);
}

#[test]
fn findings_scope_is_not_graph_identity() {
    let root = fixture();
    let basis = AnalysisBasis::for_workspace(root.path().to_path_buf());
    let (graph_a, state_a, source_a) = graph_for(&basis);
    let (graph_b, state_b, source_b) = graph_for(&basis);

    let workspace = WorkspaceId::try_new("fixture").expect("workspace id");
    let scope_a = AnalysisScope::new(workspace.clone(), SnapshotId::new(1));
    let scope_b = AnalysisScope::new(workspace, SnapshotId::new(2));
    assert_ne!(scope_a, scope_b);

    assert_eq!(graph_a, graph_b);
    assert_eq!(state_a, state_b);
    assert_eq!(source_a, source_b);
}
