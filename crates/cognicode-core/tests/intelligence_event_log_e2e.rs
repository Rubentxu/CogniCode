//! U50 — the causal slice, end to end (M7, cycle e63).
//!
//! The ADR-043 spike. Nothing here is a synthetic event stream: the facts are
//! committed to the **real** evidence kernel, the analysis runs through the
//! **real** detector executor and the M6 canonical bridge, and the events are
//! recorded at those real boundaries by the application recorder.
//!
//! ```text
//! SourceDelta ──caused_by──► FactBatchCommitted ──► AnalysisCompleted ──► FindingProduced
//! ```
//!
//! The exit gate for e63:
//!
//! ```text
//! Given   SourceDelta D
//! When    D causes FactBatch F, F causes Analysis A, A causes Finding X
//! Then    causal_chain(X) == [D, F, A, X]
//! And     replay produces the same ordered immutable events
//! And     a large fact batch is one bounded event referencing a digest
//! ```
//!
//! Requires the `evidence-kernel` feature (the slice crosses the kernel).

#![cfg(feature = "evidence-kernel")]

use std::sync::Arc;

use cognicode_core::application::findings::kernel_bridge::{
    CanonicalEvidenceWriter, KernelEvidenceReadModel,
};
use cognicode_core::application::intelligence_log::{CausalRecorder, counted_payload};
use cognicode_core::domain::evidence_kernel::bootstrap::bootstrap_registry;
use cognicode_core::domain::evidence_kernel::fact::{
    Fact, FactValue, ProducerKind, ProvenanceRecord,
};
use cognicode_core::domain::evidence_kernel::ports::FactStore;
use cognicode_core::domain::evidence_kernel::relation::RelationKind;
use cognicode_core::domain::findings::{
    AdmissionSource, AnalysisCapability, AnalysisInput, AnalysisScope, AstBackend, AstConstruct,
    AstInput, AstUnit, BackendRegistry, DetectorAdmission, DetectorAuthority, DetectorExecutor,
    DetectorFindingPolicy, DetectorId, DetectorIr, DetectorStep, EvidenceClass, ExecutionRequest,
    FindingGate, FindingKind, FindingVerifier, GroundingRef, PromotionAuthority, PromotionRequest,
    RiskLevel, SubjectPattern,
};
use cognicode_core::domain::intelligence_log::IntelligenceEventStore;
use cognicode_core::domain::intelligence_log::event::EventTime;
use cognicode_core::domain::intelligence_log::ids::CorrelationId;
use cognicode_core::domain::intelligence_log::kind::{ActorRef, EventKinds};
use cognicode_core::domain::intelligence_log::payload::{ContentDigest, EventPayloadRef};
use cognicode_core::domain::kernel_ids::{EntityId, ExecutionId, FactId, SnapshotId};
use cognicode_core::domain::value_objects::{Provenance, WorkspaceId};
use cognicode_core::infrastructure::evidence_kernel::in_memory::{
    InMemoryEvidenceStore, InMemoryFactStore, InMemorySchemaRegistry,
};
use cognicode_core::infrastructure::intelligence_log::InMemoryEventLog;

const SNAP: u64 = 1;
const FACT: u64 = 7;
/// Large enough that "one event per fact" would be visibly wrong.
const BATCH_SIZE: u64 = 500;

fn workspace() -> WorkspaceId {
    WorkspaceId::try_new("ws-u50").unwrap()
}

/// The execution request every test run uses (M7.2): a deterministic actor,
/// a fixed correlation, and the scope the fixture views were projected from.
fn test_request(id: u64) -> ExecutionRequest {
    ExecutionRequest::new(
        ExecutionId::new(id),
        scope(),
        ActorRef::detector("test.detector"),
        CorrelationId::new("test-correlation").unwrap(),
    )
}

fn scope() -> AnalysisScope {
    AnalysisScope::new(workspace(), SnapshotId::new(SNAP))
}

fn provenance() -> ProvenanceRecord {
    ProvenanceRecord::new(
        Provenance::Extracted,
        ProducerKind::DeterministicAnalyzer,
        Some("u50 fixture".to_string()),
    )
}

/// The facts of one batch. `FACT` is inside it, so the analysis has something
/// real to be grounded in.
fn fact_batch() -> Vec<Fact> {
    // Compile-time invariant: `FACT` must be within `BATCH_SIZE` so the
    // fixture exercises a batch that actually contains the target fact.
    // Lifted to a `const` block so the check runs at compile time; a
    // plain `assert!` here trips `clippy::assertions_on_constants`.
    const {
        assert!(
            FACT <= BATCH_SIZE,
            "the fixture's fact must belong to the committed batch"
        )
    };
    (1..=BATCH_SIZE)
        .map(|i| {
            Fact::new(
                FactId::new(i),
                EntityId::new(i),
                RelationKind::try_new("core:calls").expect("valid relation"),
                FactValue::Ref(EntityId::new(i + 1000)),
                SnapshotId::new(SNAP),
                provenance(),
            )
            .expect("non-LLM fact")
        })
        .collect()
}

fn weak_hash_ir() -> DetectorIr {
    DetectorIr {
        id: DetectorId::new("security.weak_hash").unwrap(),
        name: "weak hash".to_string(),
        policy: DetectorFindingPolicy::default(),
        requires: [AnalysisCapability::AstPattern].into_iter().collect(),
        authority: DetectorAuthority::Candidate,
        steps: vec![
            DetectorStep::Match {
                subject: SubjectPattern::new("security.md5_usage").unwrap(),
            },
            DetectorStep::Produce {
                kind: FindingKind::new("security.weak_hash").unwrap(),
            },
        ],
    }
}

fn gated_detector() -> cognicode_core::domain::findings::ExecutionPermit {
    let candidate =
        DetectorAdmission::admit(weak_hash_ir(), "1.0.0", AdmissionSource::HumanCurated).unwrap();
    let request = PromotionRequest::for_permit(&candidate, "security-team").unwrap();
    let verified = PromotionAuthority::verify(
        &cognicode_core::domain::findings::EligibleSourceVerifier,
        request,
    )
    .unwrap();
    DetectorAdmission::promote(&candidate, verified).unwrap()
}

fn ast_input() -> AnalysisInput {
    AnalysisInput {
        scope: Some(scope()),
        ast: Some(AstInput {
            units: vec![AstUnit {
                path: "src/hash.rs".to_string(),
                constructs: vec![AstConstruct {
                    subject: SubjectPattern::new("security.md5_usage").unwrap(),
                    line: 12,
                    detail: "md5::Md5::new()".to_string(),
                    grounding: Some(GroundingRef::fact(FactId::new(FACT))),
                }],
            }],
        }),
        ..Default::default()
    }
}

fn registry() -> BackendRegistry {
    let mut registry = BackendRegistry::new();
    registry.register(Box::new(AstBackend));
    registry
}

/// Run the whole slice and return the log plus the finding event id.
async fn run_slice(
    log: &InMemoryEventLog,
    facts: &InMemoryFactStore,
    evidence: &InMemoryEvidenceStore,
) -> cognicode_core::domain::kernel_ids::EventId {
    let correlation = CorrelationId::new("corr-u50").unwrap();
    let mut recorder = CausalRecorder::new(
        log,
        workspace(),
        ActorRef::kernel(),
        correlation.clone(),
        EventTime::from_millis(1_700_000_000_000),
    );

    // ── 1. A source delta was observed: the root of the chain ──────────────
    let delta = recorder
        .record_root(
            scope(),
            EventKinds::source_delta(),
            counted_payload(
                "source delta observed",
                &[("files", "1".to_string()), ("kind", "edit".to_string())],
            )
            .unwrap(),
        )
        .await
        .expect("root event");

    // ── 2. The batch is committed to the REAL kernel ───────────────────────
    let batch = fact_batch();
    // The digest is taken over the batch that actually landed, so the event
    // points at real bytes and not at a description of them.
    let bytes = serde_json::to_vec(&batch).expect("facts serialize");
    let digest = ContentDigest::of(&bytes);
    facts
        .commit(&workspace(), &SnapshotId::new(SNAP), batch)
        .await
        .expect("fact batch committed");

    // ONE event for the whole batch, referencing the bytes by digest: this is
    // the "a million facts do not produce a million events" rule, enforced by
    // the payload model rather than by convention.
    let batch_event = recorder
        .record_next(
            scope(),
            EventKinds::fact_batch_committed(),
            EventPayloadRef::artifact(digest.clone(), "application/json", bytes.len() as u64)
                .unwrap(),
        )
        .await
        .expect("fact batch event");

    // ── 3. The analysis runs through the real executor (M6) ────────────────
    let registry = registry();
    let executor = DetectorExecutor::new(&registry);
    let prepared = executor
        .prepare(&gated_detector(), &ast_input(), test_request(1))
        .expect("prepare");
    let bindings = CanonicalEvidenceWriter::new(facts, evidence)
        .persist(&scope(), prepared.produced_evidence(), &provenance())
        .await
        .expect("canonical persist");
    let record = prepared.finalize(&bindings).expect("finalize");

    let completed = recorder
        .record_next(
            scope(),
            EventKinds::analysis_completed(),
            counted_payload(
                "analysis completed",
                &[
                    ("backend", record.backend.clone()),
                    ("findings", record.findings.len().to_string()),
                ],
            )
            .unwrap(),
        )
        .await
        .expect("analysis event");

    // ── 4. The finding is produced ─────────────────────────────────────────
    let mut produces = CausalRecorder::new(
        log,
        workspace(),
        ActorRef::detector("security.weak_hash"),
        correlation,
        EventTime::from_millis(1_700_000_000_100),
    );
    let finding_event = produces
        .record_caused_by(
            completed,
            scope(),
            EventKinds::finding_produced(),
            counted_payload(
                "finding produced",
                &[("kind", "security.weak_hash".to_string())],
            )
            .unwrap(),
        )
        .await
        .expect("finding event");

    // The events are real because the finding is real: it verifies against the
    // kernel and it gates. A log of events about nothing would be worthless.
    let model =
        KernelEvidenceReadModel::load(&scope(), &record.bindings.grounded_ids(), facts, evidence)
            .await
            .expect("read model");
    let verifier = FindingVerifier::new(&model);
    assert!(
        verifier.can_block(
            &record.findings[0],
            &FindingGate::new(EvidenceClass::C, RiskLevel::Low)
        ),
        "the analysed finding must be the one the log ends at"
    );

    assert_eq!(delta.get(), 1);
    assert_eq!(batch_event.get(), 2);
    finding_event
}

// ============================================================================
// The exit gate
// ============================================================================

#[tokio::test]
async fn u50_the_causal_chain_reads_source_delta_factbatch_analysis_finding() {
    let registry = InMemorySchemaRegistry::new();
    bootstrap_registry(&registry).expect("canonical bootstrap");
    let facts = InMemoryFactStore::new(Arc::new(registry));
    let evidence = InMemoryEvidenceStore::new();
    let log = InMemoryEventLog::new();

    let finding_event = run_slice(&log, &facts, &evidence).await;

    let chain = log
        .causal_chain(&workspace(), finding_event)
        .await
        .expect("chain");
    let kinds: Vec<&str> = chain.iter().map(|e| e.kind.as_str()).collect();
    assert_eq!(
        kinds,
        vec![
            "kernel.source_delta",
            "kernel.fact_batch_committed",
            "analysis.completed",
            "finding.produced",
        ],
        "the chain must read in the order things happened"
    );

    // And the causal edges are exactly the ones claimed, not merely the order.
    assert_eq!(chain[0].caused_by, None);
    for window in chain.windows(2) {
        assert_eq!(
            window[1].caused_by,
            Some(window[0].id),
            "each event must be caused by the one before it"
        );
    }
    assert_eq!(chain[3].id, finding_event);
}

#[tokio::test]
async fn u50_replay_is_ordered_and_immutable() {
    let registry = InMemorySchemaRegistry::new();
    bootstrap_registry(&registry).expect("canonical bootstrap");
    let facts = InMemoryFactStore::new(Arc::new(registry));
    let evidence = InMemoryEvidenceStore::new();
    let log = InMemoryEventLog::new();

    run_slice(&log, &facts, &evidence).await;

    let first = log.replay(&workspace(), None).await.expect("replay");
    let second = log.replay(&workspace(), None).await.expect("replay again");
    assert_eq!(first, second, "replay must be reproducible");
    assert_eq!(first.len(), 4, "one event per boundary, not per fact");

    let kinds: Vec<&str> = first.iter().map(|e| e.kind.as_str()).collect();
    assert_eq!(
        kinds,
        vec![
            "kernel.source_delta",
            "kernel.fact_batch_committed",
            "analysis.completed",
            "finding.produced"
        ]
    );

    // Immutability: an appended-then-read event never changes, and appending
    // more does not rewrite history.
    let replayed_again = log.replay(&workspace(), None).await.unwrap();
    assert_eq!(replayed_again, first);

    // Incremental replay is a suffix, and the union is the whole log.
    let tail = log
        .replay(&workspace(), Some(first[1].id))
        .await
        .expect("incremental replay");
    assert_eq!(tail.len(), 2);
    assert_eq!(tail[0].id, first[2].id);
}

#[tokio::test]
async fn u50_a_large_batch_is_one_bounded_event_referencing_a_digest() {
    let registry = InMemorySchemaRegistry::new();
    bootstrap_registry(&registry).expect("canonical bootstrap");
    let facts = InMemoryFactStore::new(Arc::new(registry));
    let evidence = InMemoryEvidenceStore::new();
    let log = InMemoryEventLog::new();

    run_slice(&log, &facts, &evidence).await;

    let all = log.replay(&workspace(), None).await.unwrap();
    assert_eq!(
        all.len(),
        4,
        "{BATCH_SIZE} facts must not produce {BATCH_SIZE} events"
    );

    let batch_event = &all[1];
    assert_eq!(batch_event.kind.as_str(), "kernel.fact_batch_committed");
    assert!(
        !batch_event.payload.is_inline(),
        "the batch is referenced, not embedded"
    );
    let digest = batch_event.payload.digest().expect("artifact digest");
    assert!(digest.as_str().starts_with("sha256:"));

    // And the digest actually identifies the committed batch.
    let expected = ContentDigest::of(&serde_json::to_vec(&fact_batch()).unwrap());
    assert_eq!(digest, &expected);
}

#[tokio::test]
async fn u50_the_chain_is_correlated_and_actor_attributed() {
    let registry = InMemorySchemaRegistry::new();
    bootstrap_registry(&registry).expect("canonical bootstrap");
    let facts = InMemoryFactStore::new(Arc::new(registry));
    let evidence = InMemoryEvidenceStore::new();
    let log = InMemoryEventLog::new();

    run_slice(&log, &facts, &evidence).await;

    let correlation = CorrelationId::new("corr-u50").unwrap();
    let correlated = log
        .by_correlation(&workspace(), &correlation)
        .await
        .expect("correlation");
    assert_eq!(
        correlated.len(),
        4,
        "the whole slice is one correlated operation, even across two actors"
    );

    // The kernel committed the facts; a detector produced the finding.
    assert_eq!(
        correlated[1].actor,
        ActorRef::kernel(),
        "a fact batch is the kernel's act"
    );
    assert_eq!(
        correlated[3].actor,
        ActorRef::detector("security.weak_hash"),
        "a finding is the detector's act"
    );
    assert_eq!(correlated[0].actor, ActorRef::kernel());
    assert_eq!(correlated[2].actor, ActorRef::kernel());
}
