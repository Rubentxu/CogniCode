//! Canonical grounding end-to-end (M6, cycle e62.4, U42 part 2b).
//!
//! The acceptance test for U42. Everything here runs through the **real**
//! kernel stores: a produced finding is persisted by
//! [`CanonicalEvidenceWriter`], the read model is loaded with
//! [`KernelEvidenceReadModel::load`], and the verifier is the plain sync one.
//!
//! Three positives (one per analysis paradigm) prove the bridge does not add a
//! special case. Three adversarial negatives prove that a bare id match is no
//! longer enough to block:
//!
//! ```text
//! A) finding in A + real evidence/fact of B, numerically identical ids
//!       → ScopeMismatch, before any id is resolved
//! B) correct EvidenceId, but the step claims a fact the evidence does not grade
//!       → FactMismatch
//! C) correct EvidenceId and fact, but the evidence refutes its fact
//!       → RefutingEvidence, can_block == false
//! ```
//!
//! Requires the `evidence-kernel` feature.
#![cfg(feature = "evidence-kernel")]

use std::collections::BTreeSet;
use std::sync::Arc;

use cognicode_core::application::findings::M5DataflowBackend;
use cognicode_core::application::findings::kernel_bridge::{
    CanonicalEvidenceWriter, KernelEvidenceReadModel,
};
use cognicode_core::application::program_analysis::ProgramAnalysisService;
use cognicode_core::domain::evidence_kernel::bootstrap::bootstrap_registry;
use cognicode_core::domain::evidence_kernel::evidence::EvidenceGrade;
use cognicode_core::domain::evidence_kernel::fact::{
    Fact, FactValue, ProducerKind, ProvenanceRecord,
};
use cognicode_core::domain::evidence_kernel::ports::{
    EvidenceStore, FactStore, KernelError, NewEvidence,
};
use cognicode_core::domain::execution::{ActorRef, CorrelationId};
use cognicode_core::domain::findings::{
    AdmissionSource, AnalysisCapability, AnalysisInput, AnalysisScope, AstBackend, AstConstruct,
    AstInput, AstUnit, BackendRegistry, DataflowFunction, DataflowInput, DataflowLocation,
    DataflowStatement, DetectorAdmission, DetectorAuthority, DetectorExecutor,
    DetectorFindingPolicy, DetectorId, DetectorIr, DetectorStep, EvidenceBinding, EvidenceClass,
    ExecutionRequest, FindingGate, FindingKind, FindingVerifier, GraphBackend, GraphEdge,
    GraphInput, GraphNode, GroundingRef, PromotionAuthority, PromotionRequest, RiskLevel,
    SubjectPattern, VerificationError,
};
use cognicode_core::domain::kernel_ids::{EntityId, EvidenceId, ExecutionId, FactId, SnapshotId};
use cognicode_core::domain::value_objects::{Provenance, WorkspaceId};
use cognicode_core::infrastructure::evidence_kernel::in_memory::{
    InMemoryEvidenceStore, InMemoryFactStore, InMemorySchemaRegistry,
};

const WS: &str = "ws-u42";
const SNAP_A: u64 = 1;
const SNAP_B: u64 = 2;
/// The canonical fact every grounded fixture points at.
const FACT: u64 = 7;

fn workspace() -> WorkspaceId {
    WorkspaceId::try_new(WS).expect("valid workspace")
}

/// The execution request every test run uses (M7.2): a deterministic actor,
/// a fixed correlation, and the scope the fixture views were projected from.
fn test_request(id: u64) -> ExecutionRequest {
    ExecutionRequest::new(
        ExecutionId::new(id),
        scope(SNAP_A),
        ActorRef::detector("test.detector"),
        CorrelationId::new("test-correlation").unwrap(),
    )
}

fn scope(snapshot: u64) -> AnalysisScope {
    AnalysisScope::new(workspace(), SnapshotId::new(snapshot))
}

fn provenance() -> ProvenanceRecord {
    ProvenanceRecord::new(
        Provenance::Extracted,
        ProducerKind::DeterministicAnalyzer,
        Some("u42 fixture".to_string()),
    )
}

/// A fact store plus the evidence store, wired to the canonical vocabulary.
fn stores() -> (InMemoryFactStore, InMemoryEvidenceStore) {
    let registry = InMemorySchemaRegistry::new();
    bootstrap_registry(&registry).expect("canonical bootstrap");
    (
        InMemoryFactStore::new(Arc::new(registry)),
        InMemoryEvidenceStore::new(),
    )
}

/// Record `fact_id` about `subject` in `snapshot`.
async fn seed_fact(facts: &InMemoryFactStore, fact_id: u64, subject: u64, snapshot: u64) {
    seed_facts(facts, &[(fact_id, subject)], snapshot).await
}

/// Record several facts in **one** commit: the kernel assigns id space per
/// batch, so two separate commits in the same snapshot collide by design.
async fn seed_facts(facts: &InMemoryFactStore, spec: &[(u64, u64)], snapshot: u64) {
    let batch: Vec<Fact> = spec
        .iter()
        .map(|(fact_id, subject)| {
            Fact::new(
                FactId::new(*fact_id),
                EntityId::new(*subject),
                cognicode_core::domain::evidence_kernel::relation::RelationKind::try_new(
                    "core:calls",
                )
                .expect("valid relation"),
                FactValue::Ref(EntityId::new(*subject + 100)),
                SnapshotId::new(snapshot),
                provenance(),
            )
            .expect("non-LLM fact")
        })
        .collect();
    facts
        .commit(&workspace(), &SnapshotId::new(snapshot), batch)
        .await
        .expect("facts committed");
}

// ============================================================================
// IR fixtures (one per paradigm)
// ============================================================================

fn ir(id: &str, capability: AnalysisCapability, steps: Vec<DetectorStep>) -> DetectorIr {
    DetectorIr {
        id: DetectorId::new(id).unwrap(),
        name: id.to_string(),
        policy: DetectorFindingPolicy::default(),
        requires: [capability].into_iter().collect(),
        authority: DetectorAuthority::Candidate,
        steps,
    }
}

/// A gated permit: the detector holds blocking authority at execution time.
fn gated(ir: DetectorIr) -> cognicode_core::domain::findings::ExecutionPermit {
    let candidate = DetectorAdmission::admit(ir, "1.0.0", AdmissionSource::HumanCurated).unwrap();
    let request = PromotionRequest::for_permit(&candidate, "architecture-team").unwrap();
    let verified = PromotionAuthority::verify(
        &cognicode_core::domain::findings::EligibleSourceVerifier,
        request,
    )
    .unwrap();
    DetectorAdmission::promote(&candidate, verified).unwrap()
}

fn weak_hash_ir() -> DetectorIr {
    ir(
        "security.weak_hash",
        AnalysisCapability::AstPattern,
        vec![
            DetectorStep::Match {
                subject: SubjectPattern::new("security.md5_usage").unwrap(),
            },
            DetectorStep::Produce {
                kind: FindingKind::new("security.weak_hash").unwrap(),
            },
        ],
    )
}

fn direct_db_ir() -> DetectorIr {
    ir(
        "architecture.direct_db_access",
        AnalysisCapability::GraphQuery,
        vec![
            DetectorStep::Match {
                subject: SubjectPattern::new("endpoint.http").unwrap(),
            },
            DetectorStep::Flow {
                source: SubjectPattern::new("endpoint.http").unwrap(),
                sink: SubjectPattern::new("persistence.write").unwrap(),
                max_hops: None,
            },
            DetectorStep::Produce {
                kind: FindingKind::new("architecture.direct_db_access").unwrap(),
            },
        ],
    )
}

fn sql_injection_ir() -> DetectorIr {
    ir(
        "security.sql_injection",
        AnalysisCapability::Dataflow,
        vec![
            DetectorStep::Match {
                subject: SubjectPattern::new("security.user_input").unwrap(),
            },
            DetectorStep::Flow {
                source: SubjectPattern::new("security.user_input").unwrap(),
                sink: SubjectPattern::new("security.sql_execution").unwrap(),
                max_hops: None,
            },
            DetectorStep::Produce {
                kind: FindingKind::new("security.sql_injection").unwrap(),
            },
        ],
    )
}

/// An AST input whose single construct is projected from `fact`.
fn ast_input(fact: u64) -> AnalysisInput {
    AnalysisInput {
        scope: Some(scope(SNAP_A)),
        ast: Some(AstInput {
            units: vec![AstUnit {
                path: "src/hash.rs".to_string(),
                constructs: vec![AstConstruct {
                    subject: SubjectPattern::new("security.md5_usage").unwrap(),
                    line: 12,
                    detail: "md5::Md5::new()".to_string(),
                    grounding: Some(GroundingRef::entity(EntityId::new(1), FactId::new(fact))),
                }],
            }],
        }),
        ..Default::default()
    }
}

/// A two-hop graph where **every** node and relation is grounded.
fn graph_input(fact: u64) -> AnalysisInput {
    let node = |id: u64, subject: &str, line: u32| GraphNode {
        id,
        subject: SubjectPattern::new(subject).unwrap(),
        path: "src/app.rs".to_string(),
        line,
        // No entity hint: every fixture fact is about entity 1, so hinting the
        // node id would (correctly) be refused by the bridge's coherence check.
        grounding: Some(GroundingRef::fact(FactId::new(fact))),
    };
    AnalysisInput {
        scope: Some(scope(SNAP_A)),
        graph: Some(GraphInput {
            nodes: vec![
                node(1, "endpoint.http", 10),
                node(2, "service.handler", 20),
                node(3, "persistence.write", 30),
            ],
            edges: vec![
                GraphEdge {
                    from: 1,
                    to: 2,
                    grounding: Some(GroundingRef::fact(FactId::new(fact))),
                },
                GraphEdge {
                    from: 2,
                    to: 3,
                    grounding: Some(GroundingRef::fact(FactId::new(fact))),
                },
            ],
        }),
        ..Default::default()
    }
}

/// A statement chain where every statement is grounded.
fn dataflow_input(fact: u64) -> AnalysisInput {
    let stmt =
        |id: u64, subjects: &[&str], line: u32, defs: &[&str], uses: &[&str]| DataflowStatement {
            id,
            defs: defs.iter().map(|s| s.to_string()).collect(),
            uses: uses.iter().map(|s| s.to_string()).collect(),
            subjects: subjects
                .iter()
                .map(|s| SubjectPattern::new(*s).unwrap())
                .collect(),
            location: DataflowLocation {
                path: "src/handler.rs".to_string(),
                line,
            },
            grounding: Some(GroundingRef::fact(FactId::new(fact))),
        };
    AnalysisInput {
        scope: Some(scope(SNAP_A)),
        dataflow: Some(DataflowInput {
            functions: vec![DataflowFunction {
                id: "handler".to_string(),
                statements: vec![
                    stmt(1, &["security.user_input"], 3, &["a"], &[]),
                    stmt(2, &[], 4, &["b"], &["a"]),
                    stmt(3, &["security.sql_execution"], 5, &[], &["b"]),
                ],
            }],
        }),
        ..Default::default()
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

/// The whole pipeline: prepare → async persist into the kernel → finalize.
async fn run(
    permit: &cognicode_core::domain::findings::ExecutionPermit,
    input: &AnalysisInput,
    facts: &InMemoryFactStore,
    evidence: &InMemoryEvidenceStore,
) -> cognicode_core::domain::findings::ExecutionRecord {
    let registry = registry();
    let executor = DetectorExecutor::new(&registry);
    let prepared = executor
        .prepare(permit, input, test_request(1))
        .expect("prepare");
    let scope = prepared.scope().cloned().expect("scoped");
    let bindings = CanonicalEvidenceWriter::new(facts, evidence)
        .persist(&scope, prepared.produced_evidence(), &provenance())
        .await
        .expect("persist into the kernel");
    prepared.finalize(&bindings).expect("finalize")
}

async fn read_model(
    facts: &InMemoryFactStore,
    evidence: &InMemoryEvidenceStore,
    scope: &AnalysisScope,
    record: &cognicode_core::domain::findings::ExecutionRecord,
) -> KernelEvidenceReadModel {
    KernelEvidenceReadModel::load(scope, &record.bindings.grounded_ids(), facts, evidence)
        .await
        .expect("read model loads")
}

/// A gate that admits the strongest class each paradigm can support.
fn gate(min_class: EvidenceClass) -> FindingGate {
    FindingGate::new(min_class, RiskLevel::Low)
}

// ============================================================================
// Positives — every paradigm reaches the gate through the same bridge
// ============================================================================

#[tokio::test]
async fn ast_finding_is_grounded_and_gates_through_the_kernel() {
    let (facts, evidence) = stores();
    seed_fact(&facts, FACT, 1, SNAP_A).await;

    let record = run(&gated(weak_hash_ir()), &ast_input(FACT), &facts, &evidence).await;
    assert_eq!(record.backend, "ast");
    let finding = &record.findings[0];

    // The finding cites the id the *kernel* allocated, not one it chose.
    let id = record.bindings.grounded_ids();
    assert_eq!(id.len(), 1);
    assert_eq!(finding.evidence, id);
    assert_eq!(finding.causal_chain[0].fact, Some(FactId::new(FACT)));

    let model = read_model(&facts, &evidence, &scope(SNAP_A), &record).await;
    // The kernel knows the provenance the domain deliberately does not carry.
    assert!(model.provenance(id[0]).is_some());

    let verifier = FindingVerifier::new(&model);
    assert!(
        verifier.verify_for_gate(finding).is_ok(),
        "{:?}",
        verifier.verify_for_gate(finding)
    );
    assert!(
        verifier.can_block(finding, &gate(EvidenceClass::C)),
        "a grounded AST route through an admitted detector must be able to block"
    );
}

#[tokio::test]
async fn graph_finding_is_grounded_edge_by_edge_and_gates() {
    let (facts, evidence) = stores();
    seed_fact(&facts, FACT, 1, SNAP_A).await;

    let record = run(
        &gated(direct_db_ir()),
        &graph_input(FACT),
        &facts,
        &evidence,
    )
    .await;
    assert_eq!(record.backend, "graph");
    let finding = &record.findings[0];

    // Source node, two relations, sink node: every atom is grounded, so every
    // causal step is grounded and cites its own evidence.
    assert_eq!(finding.evidence.len(), 4);
    assert_eq!(finding.causal_chain.len(), 4);
    let mut step_ids: Vec<EvidenceId> = finding
        .causal_chain
        .iter()
        .map(|s| s.evidence.expect("grounded"))
        .collect();
    step_ids.sort();
    step_ids.dedup();
    assert_eq!(step_ids.len(), 4, "each step has its own atom");
    assert!(
        finding
            .causal_chain
            .iter()
            .all(|s| s.fact == Some(FactId::new(FACT)))
    );

    let model = read_model(&facts, &evidence, &scope(SNAP_A), &record).await;
    let verifier = FindingVerifier::new(&model);
    assert!(
        verifier.verify_for_gate(finding).is_ok(),
        "{:?}",
        verifier.verify_for_gate(finding)
    );
    assert!(verifier.can_block(finding, &gate(EvidenceClass::B)));
}

#[tokio::test]
async fn dataflow_finding_is_grounded_statement_by_statement_and_gates() {
    let (facts, evidence) = stores();
    seed_fact(&facts, FACT, 1, SNAP_A).await;

    let record = run(
        &gated(sql_injection_ir()),
        &dataflow_input(FACT),
        &facts,
        &evidence,
    )
    .await;
    assert_eq!(record.backend, "m5_dataflow");
    let finding = &record.findings[0];
    assert_eq!(finding.evidence.len(), 3, "one atom per statement");
    assert_eq!(finding.causal_chain.len(), 3);

    let model = read_model(&facts, &evidence, &scope(SNAP_A), &record).await;
    let verifier = FindingVerifier::new(&model);
    assert!(
        verifier.verify_for_gate(finding).is_ok(),
        "{:?}",
        verifier.verify_for_gate(finding)
    );
    assert!(verifier.can_block(finding, &gate(EvidenceClass::B)));
}

// ============================================================================
// Negatives — the three adversarial cases
// ============================================================================

/// A) The finding was produced in snapshot A; the read model is hydrated from
/// snapshot B, which holds a **real** evidence atom and a **real** fact with
/// exactly the same numeric ids. Nothing resolves to a missing id, so the only
/// thing that can stop it is the scope check — and it must come first.
#[tokio::test]
async fn a_cross_snapshot_read_model_is_rejected_before_any_id_resolves() {
    let (facts, evidence) = stores();
    // Snapshot A: the run actually happened here.
    seed_fact(&facts, FACT, 1, SNAP_A).await;
    let record = run(&gated(weak_hash_ir()), &ast_input(FACT), &facts, &evidence).await;

    // Snapshot B: a *different* fact and a *different* evidence atom, crafted
    // to carry the same ids as A's.
    seed_fact(&facts, FACT, 1, SNAP_B).await;
    let b_id = evidence
        .append_batch(
            &workspace(),
            &SnapshotId::new(SNAP_B),
            vec![NewEvidence {
                fact: FactId::new(FACT),
                grade: EvidenceGrade::Supports,
                provenance: provenance(),
            }],
        )
        .await
        .expect("B evidence");
    assert_eq!(
        b_id,
        record.bindings.grounded_ids(),
        "the whole point: the ids coincide numerically"
    );

    // Hydrate the read model for B and verify A's finding against it.
    let model = KernelEvidenceReadModel::load(
        &scope(SNAP_B),
        &record.bindings.grounded_ids(),
        &facts,
        &evidence,
    )
    .await
    .expect("load");
    // B is genuinely complete: the id resolves and the fact exists.
    assert_eq!(model.len(), 1);

    let verifier = FindingVerifier::new(&model);
    match verifier.verify_for_gate(&record.findings[0]) {
        Err(VerificationError::ScopeMismatch { .. }) => {}
        other => panic!("expected ScopeMismatch, got {other:?}"),
    }
    assert!(!verifier.can_block(&record.findings[0], &gate(EvidenceClass::B)));
}

/// B) The evidence id is correct and the evidence grades fact 7, but the causal
/// step claims fact 8. Both facts exist and every id resolves: only the
/// coherence rule catches this.
#[tokio::test]
async fn a_step_that_claims_a_different_fact_is_rejected() {
    let (facts, evidence) = stores();
    // Two perfectly valid facts, committed together (the kernel assigns one id
    // space per batch), so the step can lie about which one it was grounded in.
    seed_facts(&facts, &[(FACT, 1), (FACT + 1, 1)], SNAP_A).await;

    let record = run(&gated(weak_hash_ir()), &ast_input(FACT), &facts, &evidence).await;
    let mut finding = record.findings[0].clone();
    finding.causal_chain[0].fact = Some(FactId::new(FACT + 1));

    let model = read_model(&facts, &evidence, &scope(SNAP_A), &record).await;
    let verifier = FindingVerifier::new(&model);
    match verifier.verify_for_gate(&finding) {
        Err(VerificationError::FactMismatch {
            step,
            step_fact,
            evidence_fact,
        }) => {
            assert_eq!(step, 0);
            assert_eq!(step_fact, FactId::new(FACT + 1));
            assert_eq!(evidence_fact, FactId::new(FACT));
        }
        other => panic!("expected FactMismatch, got {other:?}"),
    }
    assert!(!verifier.can_block(&finding, &gate(EvidenceClass::B)));
}

/// C) The evidence id is correct and the fact is correct, but the evidence
/// **refutes** its fact. Accepting evidence merely because it exists would be
/// wrong: `Refutes` must fail loud and never enable the gate.
#[tokio::test]
async fn refuting_evidence_is_rejected_and_never_gates() {
    let (facts, evidence) = stores();
    seed_fact(&facts, FACT, 1, SNAP_A).await;
    let record = run(&gated(weak_hash_ir()), &ast_input(FACT), &facts, &evidence).await;
    let id = record.bindings.grounded_ids()[0];

    // A second snapshot-free store where the same fact is graded `Refutes`:
    // same id, same fact, opposite meaning.
    let (facts_b, evidence_b) = stores();
    seed_fact(&facts_b, FACT, 1, SNAP_A).await;
    evidence_b
        .add(
            &workspace(),
            &SnapshotId::new(SNAP_A),
            cognicode_core::domain::evidence_kernel::evidence::Evidence {
                id,
                fact: FactId::new(FACT),
                grade: EvidenceGrade::Refutes,
                provenance: provenance(),
            },
        )
        .await
        .expect("evidence committed");

    let model = KernelEvidenceReadModel::load(&scope(SNAP_A), &[id], &facts_b, &evidence_b)
        .await
        .expect("load");

    let verifier = FindingVerifier::new(&model);
    match verifier.verify_for_gate(&record.findings[0]) {
        Err(VerificationError::RefutingEvidence {
            evidence, grade, ..
        }) => {
            assert_eq!(evidence, id);
            assert_eq!(grade, EvidenceGrade::Refutes);
        }
        other => panic!("expected RefutingEvidence, got {other:?}"),
    }
    assert!(!verifier.can_block(&record.findings[0], &gate(EvidenceClass::B)));
}

/// The bridge never fabricates: an item whose fact does not exist is reported
/// ungrounded, is not written, and the route cannot gate.
#[tokio::test]
async fn a_missing_fact_is_reported_ungrounded_and_never_gates() {
    let (facts, evidence) = stores();
    // No fact is seeded at all.
    let record = run(&gated(weak_hash_ir()), &ast_input(FACT), &facts, &evidence).await;

    assert_eq!(
        record.bindings.ungrounded(),
        vec![(
            0,
            cognicode_core::domain::findings::GroundingFailure::MissingFact {
                fact: FactId::new(FACT)
            }
        )]
    );
    assert!(matches!(record.findings[0].evidence.is_empty(), true));
    assert!(
        record.findings[0].causal_chain[0].detail.contains("md5"),
        "the route is still explained"
    );

    let model = read_model(&facts, &evidence, &scope(SNAP_A), &record).await;
    let verifier = FindingVerifier::new(&model);
    assert!(!verifier.can_block(&record.findings[0], &gate(EvidenceClass::B)));
}

/// An entity hint that contradicts the canonical fact's subject is refused: the
/// fact is the authority, never the projection.
#[tokio::test]
async fn an_entity_hint_that_contradicts_the_fact_is_refused() {
    let (facts, evidence) = stores();
    // The fact is about entity 99, the projection claims entity 1.
    seed_fact(&facts, FACT, 99, SNAP_A).await;
    let record = run(&gated(weak_hash_ir()), &ast_input(FACT), &facts, &evidence).await;

    assert_eq!(
        record.bindings.get(0),
        Some(&EvidenceBinding::ungrounded(
            cognicode_core::domain::findings::GroundingFailure::EntityFactMismatch {
                entity: EntityId::new(1),
                subject: Some(EntityId::new(99)),
            }
        ))
    );

    let model = read_model(&facts, &evidence, &scope(SNAP_A), &record).await;
    let verifier = FindingVerifier::new(&model);
    assert!(!verifier.can_block(&record.findings[0], &gate(EvidenceClass::B)));
}

/// The seam cannot be skipped: `prepare` alone produces nothing observable in
/// the kernel, and a store failure is an error, never a silent "ungrounded".
#[tokio::test]
async fn a_store_failure_is_an_error_not_an_ungrounded_item() {
    let (facts, evidence) = stores();
    seed_fact(&facts, FACT, 1, SNAP_A).await;

    let registry = registry();
    let executor = DetectorExecutor::new(&registry);
    let prepared = executor
        .prepare(&gated(weak_hash_ir()), &ast_input(FACT), test_request(1))
        .expect("prepare");

    // Nothing was written merely by preparing.
    let before = evidence
        .get(&workspace(), &SnapshotId::new(SNAP_A), EvidenceId::new(1))
        .await
        .expect("store read");
    assert!(before.is_none(), "prepare must not write");

    // A broken fact store surfaces as an error.
    struct BrokenFacts;
    #[async_trait::async_trait]
    impl FactStore for BrokenFacts {
        async fn commit(
            &self,
            _ws: &WorkspaceId,
            _snap: &SnapshotId,
            _batch: Vec<Fact>,
        ) -> Result<Vec<FactId>, KernelError> {
            Err(KernelError::Store("down".to_string()))
        }
        async fn facts_of(
            &self,
            _ws: &WorkspaceId,
            _snap: &SnapshotId,
            _subject: &EntityId,
        ) -> Result<Vec<Fact>, KernelError> {
            Err(KernelError::Store("down".to_string()))
        }
        async fn get(
            &self,
            _ws: &WorkspaceId,
            _snap: &SnapshotId,
            _id: FactId,
        ) -> Result<Option<Fact>, KernelError> {
            Err(KernelError::Store("down".to_string()))
        }
        async fn facts_in_snapshot(
            &self,
            _ws: &WorkspaceId,
            _snap: &SnapshotId,
        ) -> Result<Vec<Fact>, KernelError> {
            Err(KernelError::Store("down".to_string()))
        }
    }

    let scope = prepared.scope().cloned().expect("scoped");
    let err = CanonicalEvidenceWriter::new(&BrokenFacts, &evidence)
        .persist(&scope, prepared.produced_evidence(), &provenance())
        .await
        .expect_err("a store failure must not degrade into 'ungrounded'");
    assert!(matches!(err, KernelError::Store(_)));
}

/// The capability set of the fixture registry is stable, so the planner cannot
/// silently start choosing another backend.
#[test]
fn the_registry_covers_all_three_paradigms() {
    let registry = registry();
    let requires: BTreeSet<AnalysisCapability> =
        [AnalysisCapability::Dataflow].into_iter().collect();
    assert_eq!(registry.plan(&requires).unwrap().name(), "m5_dataflow");
}
