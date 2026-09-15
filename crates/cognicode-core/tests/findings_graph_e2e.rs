//! Graph backend end-to-end (M6, cycle e59, U41).
//!
//! The point of this test is **not** "GraphBackend works". It is that the seam
//! defined in e57–e58.2 generalises to a *second analysis paradigm* without a
//! single special case:
//!
//! ```text
//! MATCH endpoint.http
//! FLOW  endpoint.http -> persistence.write
//! EXCLUDE security.sanitizer
//! PRODUCE architecture.direct_db_access
//! ```
//!
//! flows through the same `admission → plan → backend → outcome → evidence →
//! assembler → finding → verifier → gate` path, producing `GraphPath`
//! (class B) evidence and a `Source → Flow → Sink` causal chain.

use std::collections::BTreeSet;

use cognicode_core::domain::findings::{
    AdmissionSource, AnalysisCapability, AnalysisInput, AstBackend, BackendRegistry,
    DetectorAdmission, DetectorAuthority, DetectorBackend, DetectorExecutor, DetectorFindingPolicy,
    DetectorId, DetectorIr, DetectorStep, EvidenceClass, ExecutionError, ExecutionRecord,
    FindingGate, FindingKind, FindingVerifier, GraphBackend, GraphEdge, GraphInput, GraphNode,
    PromotionAuthority, PromotionRequest, RiskLevel, SubjectPattern,
};
use cognicode_core::domain::kernel_ids::ExecutionId;
use cognicode_core::infrastructure::findings::in_memory_evidence::InMemoryEvidenceStore;

fn node(id: u64, subject: &str, line: u32) -> GraphNode {
    GraphNode {
        id,
        subject: SubjectPattern::new(subject).unwrap(),
        path: "src/app.rs".to_string(),
        line,
    }
}

fn direct_db_access_ir() -> DetectorIr {
    DetectorIr {
        id: DetectorId::new("architecture.direct_db_access").unwrap(),
        name: "direct db access".to_string(),
        policy: DetectorFindingPolicy::default(),
        requires: [AnalysisCapability::GraphQuery].into_iter().collect(),
        authority: DetectorAuthority::Candidate,
        steps: vec![
            DetectorStep::Match {
                subject: SubjectPattern::new("endpoint.http").unwrap(),
            },
            DetectorStep::Flow {
                source: SubjectPattern::new("endpoint.http").unwrap(),
                sink: SubjectPattern::new("persistence.write").unwrap(),
                max_hops: None,
            },
            DetectorStep::Exclude {
                path_contains: SubjectPattern::new("security.sanitizer").unwrap(),
            },
            DetectorStep::Produce {
                kind: FindingKind::new("architecture.direct_db_access").unwrap(),
            },
        ],
    }
}

fn clean_graph() -> GraphInput {
    GraphInput {
        nodes: vec![
            node(1, "endpoint.http", 10),
            node(2, "service.handler", 20),
            node(3, "persistence.write", 30),
        ],
        edges: vec![GraphEdge { from: 1, to: 2 }, GraphEdge { from: 2, to: 3 }],
    }
}

fn sanitized_graph() -> GraphInput {
    GraphInput {
        nodes: vec![
            node(1, "endpoint.http", 10),
            node(2, "security.sanitizer", 20),
            node(3, "persistence.write", 30),
        ],
        edges: vec![GraphEdge { from: 1, to: 2 }, GraphEdge { from: 2, to: 3 }],
    }
}

fn input(graph: GraphInput) -> AnalysisInput {
    AnalysisInput {
        dataflow: None,
        ast: None,
        graph: Some(graph),
    }
}

fn registry() -> BackendRegistry {
    let mut registry = BackendRegistry::new();
    // Both backends registered: the planner must pick `graph` for GraphQuery.
    registry.register(Box::new(AstBackend));
    registry.register(Box::new(GraphBackend));
    registry
}

fn graph_gate() -> FindingGate {
    FindingGate::new(EvidenceClass::B, RiskLevel::Low)
}

fn run(
    permit: &cognicode_core::domain::findings::ExecutionPermit,
    graph: GraphInput,
) -> (InMemoryEvidenceStore, ExecutionRecord) {
    let registry = registry();
    let executor = DetectorExecutor::new(&registry);
    let mut store = InMemoryEvidenceStore::new();
    let record = executor
        .execute(permit, &input(graph), &mut store, ExecutionId::new(7))
        .expect("execution must succeed");
    (store, record)
}

#[test]
fn u41_graph_flow_through_the_unchanged_seam() {
    let permit = DetectorAdmission::admit(
        direct_db_access_ir(),
        "1.0.0",
        AdmissionSource::HumanCurated,
    )
    .unwrap();

    // The planner must choose the graph backend because of `GraphQuery`.
    assert_eq!(
        registry()
            .plan(&permit.admitted().definition.requires)
            .unwrap()
            .name(),
        "graph"
    );

    let (store, record) = run(&permit, clean_graph());
    assert_eq!(record.backend, "graph");
    assert_eq!(record.findings.len(), 1);

    let finding = &record.findings[0];
    assert_eq!(finding.kind.as_str(), "architecture.direct_db_access");
    assert_eq!(
        finding.evidence_class,
        EvidenceClass::B,
        "a graph path is class B evidence"
    );

    // The causal chain is source -> flow -> sink, each attributed to evidence.
    let kinds: Vec<_> = finding.causal_chain.iter().map(|c| c.kind).collect();
    assert_eq!(
        kinds,
        vec![
            cognicode_core::domain::findings::CausalStepKind::Source,
            cognicode_core::domain::findings::CausalStepKind::Flow,
            cognicode_core::domain::findings::CausalStepKind::Sink,
        ]
    );
    assert!(finding.causal_chain.iter().all(|c| c.evidence.is_some()));

    let verifier = FindingVerifier::new(&store);
    assert!(verifier.verify_for_gate(finding).is_ok());
    // Candidate (the raw IR claimed nothing here, but authority is still
    // Candidate until promoted).
    assert!(!verifier.can_block(finding, &graph_gate()));
}

#[test]
fn u41_graph_promoted_detector_blocks() {
    let candidate = DetectorAdmission::admit(
        direct_db_access_ir(),
        "1.0.0",
        AdmissionSource::HumanCurated,
    )
    .unwrap();
    let request = PromotionRequest::for_permit(&candidate, "architecture-team").unwrap();
    let verified = PromotionAuthority::verify(
        &cognicode_core::domain::findings::EligibleSourceVerifier,
        request,
    )
    .unwrap();
    let gated = DetectorAdmission::promote(&candidate, verified).unwrap();

    let (store, record) = run(&gated, clean_graph());
    let verifier = FindingVerifier::new(&store);
    assert!(verifier.can_block(&record.findings[0], &graph_gate()));
}

#[test]
fn u41_excluded_path_produces_no_finding() {
    let permit = DetectorAdmission::admit(
        direct_db_access_ir(),
        "1.0.0",
        AdmissionSource::HumanCurated,
    )
    .unwrap();
    let (_store, record) = run(&permit, sanitized_graph());
    assert!(
        record.findings.is_empty(),
        "a path through a sanitizer must not produce a finding"
    );
    assert!(record.diagnostics.iter().any(|d| d.code == "path_excluded"));
}

#[test]
fn u41_planning_rejects_a_detector_the_graph_backend_cannot_run() {
    // A detector requiring SymbolicFeasibility cannot be planned with the
    // registered backends: fail loud, do not silently use a weaker backend.
    let mut ir = direct_db_access_ir();
    ir.requires = [
        AnalysisCapability::GraphQuery,
        AnalysisCapability::SymbolicFeasibility,
    ]
    .into_iter()
    .collect();
    let permit = DetectorAdmission::admit(ir, "1.0.0", AdmissionSource::Builtin).unwrap();

    let registry = registry();
    let executor = DetectorExecutor::new(&registry);
    let mut store = InMemoryEvidenceStore::new();
    let err = executor
        .execute(
            &permit,
            &input(clean_graph()),
            &mut store,
            ExecutionId::new(1),
        )
        .expect_err("no backend provides SymbolicFeasibility");
    match err {
        ExecutionError::Plan(plan) => {
            assert!(
                plan.missing
                    .contains(&AnalysisCapability::SymbolicFeasibility)
            );
        }
        other => panic!("expected a planning error, got {other:?}"),
    }

    // Sanity: the graph backend advertises exactly GraphQuery — it must not
    // claim Dataflow/Symbolic just to win planning.
    let caps: BTreeSet<AnalysisCapability> = GraphBackend.capabilities();
    assert_eq!(
        caps,
        [AnalysisCapability::GraphQuery].into_iter().collect(),
        "GraphBackend must advertise only GraphQuery"
    );
    assert_eq!(GraphBackend.evidence_ceiling(), EvidenceClass::B);
}
