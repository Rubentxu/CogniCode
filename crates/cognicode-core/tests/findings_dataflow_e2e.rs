//! Dataflow backend end-to-end (M6, cycle e60).
//!
//! With AST, Graph **and** Dataflow registered, the planner must pick the
//! dataflow backend for a `{Dataflow}` detector, and the run must traverse the
//! *unchanged* core:
//!
//! ```text
//! DetectorIr → admission → planner → M5DataflowBackend
//!            → ProgramAnalysisService::taint_flow → M5 taint
//!            → DetectorOutcome → FindingAssembler → Finding (B)
//!            → FindingVerifier → FindingGate
//! ```
//!
//! No branch is added to `DetectorExecutor`, `FindingAssembler` or
//! `FindingVerifier`.

use std::collections::BTreeSet;

use cognicode_core::application::findings::M5DataflowBackend;
use cognicode_core::application::program_analysis::ProgramAnalysisService;
use cognicode_core::domain::findings::{
    AnalysisCapability, AnalysisInput, AnalysisScope, AstBackend, BackendRegistry, CausalStepKind,
    DataflowFunction, DataflowInput, DataflowLocation, DataflowStatement, DetectorAdmission,
    DetectorAuthority, DetectorBackend, DetectorExecutor, DetectorFindingPolicy, DetectorId,
    DetectorIr, DetectorStep, EvidenceClass, ExecutionError, ExecutionRecord, FindingGate,
    FindingKind, FindingVerifier, GraphBackend, PromotionAuthority, PromotionRequest, RiskLevel,
    SubjectPattern,
};
use cognicode_core::domain::kernel_ids::ExecutionId;
use cognicode_core::infrastructure::findings::in_memory_evidence::InMemoryEvidenceStore;

fn stmt(id: u64, subjects: &[&str], line: u32, defs: &[&str], uses: &[&str]) -> DataflowStatement {
    DataflowStatement {
        id,
        defs: defs.iter().map(|v| v.to_string()).collect(),
        uses: uses.iter().map(|v| v.to_string()).collect(),
        subjects: subjects
            .iter()
            .map(|s| SubjectPattern::new(*s).unwrap())
            .collect(),
        location: DataflowLocation {
            path: "src/handler.rs".to_string(),
            line,
        },
    }
}

fn sql_injection_ir() -> DetectorIr {
    DetectorIr {
        id: DetectorId::new("security.sql_injection").unwrap(),
        name: "sql injection".to_string(),
        policy: DetectorFindingPolicy::default(),
        requires: [AnalysisCapability::Dataflow].into_iter().collect(),
        authority: DetectorAuthority::Candidate,
        steps: vec![
            DetectorStep::Match {
                subject: SubjectPattern::new("security.user_input").unwrap(),
            },
            DetectorStep::Flow {
                source: SubjectPattern::new("security.user_input").unwrap(),
                sink: SubjectPattern::new("security.sql_execution").unwrap(),
                max_hops: None,
            },
            DetectorStep::Exclude {
                path_contains: SubjectPattern::new("security.sanitizer").unwrap(),
            },
            DetectorStep::Produce {
                kind: FindingKind::new("security.sql_injection").unwrap(),
            },
        ],
    }
}

/// `a = input(); b = f(a); sink(b)` — an unsanitised chain.
fn tainted_function() -> DataflowFunction {
    DataflowFunction {
        id: "handler".to_string(),
        statements: vec![
            stmt(1, &["security.user_input"], 10, &["a"], &[]),
            stmt(2, &[], 11, &["b"], &["a"]),
            stmt(3, &["security.sql_execution"], 12, &[], &["b"]),
        ],
    }
}

/// The same chain, but with a sanitizer between input and sink.
fn sanitized_function() -> DataflowFunction {
    DataflowFunction {
        id: "handler".to_string(),
        statements: vec![
            stmt(1, &["security.user_input"], 10, &["a"], &[]),
            stmt(2, &["security.sanitizer"], 11, &["b"], &["a"]),
            stmt(3, &["security.sql_execution"], 12, &[], &["b"]),
        ],
    }
}

fn input(function: DataflowFunction) -> AnalysisInput {
    AnalysisInput {
        scope: Some(scope()),
        ast: None,
        graph: None,
        dataflow: Some(DataflowInput {
            functions: vec![function],
        }),
    }
}

fn registry() -> BackendRegistry {
    let mut registry = BackendRegistry::new();
    registry.register(Box::new(AstBackend));
    registry.register(Box::new(GraphBackend));
    registry.register(Box::new(M5DataflowBackend::new(
        ProgramAnalysisService::new(),
    )));
    registry
}

fn gate() -> FindingGate {
    FindingGate::new(EvidenceClass::B, RiskLevel::Low)
}

fn run(
    permit: &cognicode_core::domain::findings::ExecutionPermit,
    function: DataflowFunction,
) -> (InMemoryEvidenceStore, ExecutionRecord) {
    let registry = registry();
    let executor = DetectorExecutor::new(&registry);
    let mut store = InMemoryEvidenceStore::with_scope(scope());
    let record = executor
        .execute(permit, &input(function), &mut store, ExecutionId::new(11))
        .expect("execution must succeed");
    (store, record)
}

#[test]
fn u41_dataflow_flows_through_the_unchanged_seam() {
    let permit = DetectorAdmission::admit(
        sql_injection_ir(),
        "1.0.0",
        cognicode_core::domain::findings::AdmissionSource::HumanCurated,
    )
    .unwrap();

    // The planner must pick the dataflow backend for `{Dataflow}`.
    assert_eq!(
        registry()
            .plan(&permit.admitted().definition.requires)
            .unwrap()
            .name(),
        "m5_dataflow"
    );

    let (store, record) = run(&permit, tainted_function());
    assert_eq!(record.backend, "m5_dataflow");
    assert_eq!(record.findings.len(), 1);

    let finding = &record.findings[0];
    assert_eq!(finding.kind.as_str(), "security.sql_injection");
    assert_eq!(
        finding.evidence_class,
        EvidenceClass::B,
        "a taint path is class B evidence"
    );

    // Source -> Flow -> Sink, all attributed to the same evidence.
    let kinds: Vec<CausalStepKind> = finding.causal_chain.iter().map(|c| c.kind).collect();
    assert_eq!(
        kinds,
        vec![
            CausalStepKind::Source,
            CausalStepKind::Flow,
            CausalStepKind::Sink
        ]
    );
    assert!(finding.causal_chain.iter().all(|c| c.evidence.is_some()));

    let verifier = FindingVerifier::new(&store);
    assert!(verifier.verify_for_gate(finding).is_ok());
    assert!(
        !verifier.can_block(finding, &gate()),
        "Candidate detector: found, but never blocking"
    );
}

#[test]
fn u41_dataflow_promoted_detector_blocks() {
    let candidate = DetectorAdmission::admit(
        sql_injection_ir(),
        "1.0.0",
        cognicode_core::domain::findings::AdmissionSource::HumanCurated,
    )
    .unwrap();
    let request = PromotionRequest::for_permit(&candidate, "security-team").unwrap();
    let verified = PromotionAuthority::verify(
        &cognicode_core::domain::findings::EligibleSourceVerifier,
        request,
    )
    .unwrap();
    let gated = DetectorAdmission::promote(&candidate, verified).unwrap();

    let (store, record) = run(&gated, tainted_function());
    let verifier = FindingVerifier::new(&store);
    assert!(verifier.can_block(&record.findings[0], &gate()));
}

#[test]
fn u41_sanitized_path_cannot_reappear() {
    // M5 eliminates the only path (the sanitizer is an untaint site). M6 must
    // report nothing: it does not recompute reachability.
    let permit = DetectorAdmission::admit(
        sql_injection_ir(),
        "1.0.0",
        cognicode_core::domain::findings::AdmissionSource::HumanCurated,
    )
    .unwrap();
    let (_store, record) = run(&permit, sanitized_function());
    assert!(
        record.findings.is_empty(),
        "a sanitized path must not produce a finding"
    );
}

#[test]
fn u41_dataflow_backend_advertises_only_dataflow() {
    let backend = M5DataflowBackend::new(ProgramAnalysisService::new());
    let caps: BTreeSet<AnalysisCapability> = backend.capabilities();
    assert_eq!(
        caps,
        [AnalysisCapability::Dataflow].into_iter().collect(),
        "it must not advertise GraphQuery to win planning"
    );
    assert_eq!(backend.evidence_ceiling(), EvidenceClass::B);
}

#[test]
fn u41_multi_capability_detector_fails_loud() {
    // No single registered backend provides both Dataflow and
    // SymbolicFeasibility: fail loud rather than silently degrade.
    let mut ir = sql_injection_ir();
    ir.requires = [
        AnalysisCapability::Dataflow,
        AnalysisCapability::SymbolicFeasibility,
    ]
    .into_iter()
    .collect();
    let permit = DetectorAdmission::admit(
        ir,
        "1.0.0",
        cognicode_core::domain::findings::AdmissionSource::Builtin,
    )
    .unwrap();

    let registry = registry();
    let executor = DetectorExecutor::new(&registry);
    let mut store = InMemoryEvidenceStore::with_scope(scope());
    let err = executor
        .execute(
            &permit,
            &input(tainted_function()),
            &mut store,
            ExecutionId::new(1),
        )
        .expect_err("no single backend covers both capabilities");
    match err {
        ExecutionError::Plan(plan) => {
            assert!(
                plan.missing
                    .contains(&AnalysisCapability::SymbolicFeasibility)
            );
        }
        other => panic!("expected a planning error, got {other:?}"),
    }
}

#[test]
fn u41_unrelated_match_subject_cannot_seed_a_traversal() {
    // MATCH security.cookie is an observation; FLOW.source is
    // security.user_input. The cookie chain reaches the sink, no user_input
    // site does ⇒ nothing is reported.
    let mut ir = sql_injection_ir();
    ir.steps.insert(
        1,
        DetectorStep::Match {
            subject: SubjectPattern::new("security.cookie").unwrap(),
        },
    );
    let permit = DetectorAdmission::admit(
        ir,
        "1.0.0",
        cognicode_core::domain::findings::AdmissionSource::HumanCurated,
    )
    .unwrap();

    let function = DataflowFunction {
        id: "handler".to_string(),
        statements: vec![
            stmt(1, &["security.cookie"], 10, &["c"], &[]),
            stmt(2, &[], 11, &["d"], &["c"]),
            stmt(3, &["security.sql_execution"], 12, &[], &["d"]),
            stmt(5, &["security.user_input"], 20, &["u"], &[]),
        ],
    };
    let (_store, record) = run(&permit, function);
    assert!(
        record.findings.is_empty(),
        "a cookie-originated path must never be reported as user_input reaching the sink"
    );
}

#[test]
fn u41_exceeded_limits_stay_execution_errors() {
    // max_path_count = 1 with two paths ⇒ AnalyticsError::LimitExceeded ⇒
    // BackendError::Analysis ⇒ ExecutionError::Backend. Never truncated, never
    // silently empty.
    let limits = cognicode_core::domain::plan::limits::PlanLimits {
        time_ms: Some(30_000),
        cancellation: None,
        max_depth: None,
        max_hops: None,
        max_visited_nodes: Some(1_000_000),
        max_visited_edges: None,
        max_result_rows: None,
        max_path_count: Some(1),
        max_memory_bytes: Some(512 * 1024 * 1024),
    };
    let mut registry = BackendRegistry::new();
    registry.register(Box::new(M5DataflowBackend::with_limits(
        ProgramAnalysisService::new(),
        limits,
    )));

    let permit = DetectorAdmission::admit(
        sql_injection_ir(),
        "1.0.0",
        cognicode_core::domain::findings::AdmissionSource::HumanCurated,
    )
    .unwrap();

    // Two sinks reachable from the single source ⇒ two paths.
    let function = DataflowFunction {
        id: "handler".to_string(),
        statements: vec![
            stmt(1, &["security.user_input"], 10, &["a"], &[]),
            stmt(2, &[], 11, &["b"], &["a"]),
            stmt(3, &["security.sql_execution"], 12, &[], &["b"]),
            stmt(4, &["security.sql_execution"], 13, &[], &["b"]),
        ],
    };

    let executor = DetectorExecutor::new(&registry);
    let mut store = InMemoryEvidenceStore::with_scope(scope());
    let err = executor
        .execute(&permit, &input(function), &mut store, ExecutionId::new(1))
        .expect_err("an exceeded limit must fail the execution");
    match err {
        ExecutionError::Backend(cognicode_core::domain::findings::BackendError::Analysis(
            message,
        )) => {
            assert!(
                message.contains("limit exceeded"),
                "expected a limit error, got: {message}"
            );
        }
        other => panic!("expected ExecutionError::Backend(Analysis), got {other:?}"),
    }
}

fn scope() -> AnalysisScope {
    AnalysisScope::new(
        cognicode_core::domain::value_objects::WorkspaceId::try_new("workspace").unwrap(),
        cognicode_core::domain::kernel_ids::SnapshotId::new(1),
    )
}
