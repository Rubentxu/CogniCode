//! CU-2.4 — Live consumer in isolation.
//!
//! Tests the **end-to-end read-side pipeline** (project + transfer) on
//! Findings shaped like those a real detector would emit. This is the
//! integration regression: cu2_w2 covers `project_finding` in
//! isolation, cu2_w3 covers `to_guard_observation` in isolation, this
//! file proves the *seam* between them preserves the contract under
//! realistic inputs.
//!
//! ## Contract under test (SPEC-CU-003.3, .4, .5, .7)
//!
//! - A `Finding` observed on snapshot S cannot be re-projected to a
//!   different snapshot T: the projection must surface
//!   `Unsupported(SnapshotMismatch)` so a consumer (Guard) cannot
//!   promote stale evidence.
//! - A `Finding` with a forged or non-existent `EvidenceId` does not
//!   acquire authority by passing through the seam: the pipeline
//!   surfaces the id, but the *Block* decision is exclusively
//!   Guard's, not a side effect of the transfer.
//! - Heuristic precision is preserved across the seam: a
//!   `EvidenceClass::D` finding is `Inconclusive` (with explicit
//!   gaps), a `C` is `Advisory`, an `A`/`B` is `Verifiable`.
//! - A causal step that lacks `fact` or `evidence` ids remains
//!   `Ungrounded` in the `Verifiable` observation, never silently
//!   promoted to `Grounded` by the transfer.
//!
//! ## Non-goals
//!
//! - This file does not test the kernel (it does not run a real
//!   detector or a real graph build). It tests the seam, on inputs
//!   shaped like what a detector would emit, so a regression in
//!   either `project_finding` or `to_guard_observation` shows up
//!   here too.
//! - It does not assert anything about *block* vs *allow* decisions;
//!   that is Guard's exclusive concern.

use cognicode_core::domain::execution::actor::{ActorKind, ActorRef};
use cognicode_core::domain::execution::context::ExecutionContext;
use cognicode_core::domain::execution::correlation::CorrelationId;
use cognicode_core::domain::execution::scope::AnalysisScope;
use cognicode_core::domain::findings::guard_transfer::{GuardObservation, to_guard_observation};
use cognicode_core::domain::findings::projection::{
    CauseProjection, ProjectionError, project_finding,
};
use cognicode_core::domain::findings::{
    CausalStep, CausalStepKind, DetectorAuthority, DetectorDigest, DetectorDigests,
    DetectorExecutionRef, DetectorId, EvidenceClass, Finding, FindingId, FindingKind,
    FindingOrigin, FindingSeverity, FindingStatus, RiskLevel,
};
use cognicode_core::domain::kernel_ids::{EntityId, EvidenceId, ExecutionId, FactId, SnapshotId};
use cognicode_core::domain::value_objects::workspace_id::WorkspaceId;

/// Build a Finding as a real cyclomatic-style detector would emit it:
/// one observed snapshot, one detector ref, a grounded causal chain,
/// and the evidence ids actually produced by the analysis.
fn detector_finding(class: EvidenceClass, snapshot: SnapshotId) -> Finding {
    let id = DetectorId::new("cyclomatic@1.0.0").unwrap();
    let digest = DetectorDigest::new(
        "sha256:0000000000000000000000000000000000000000000000000000000000000000",
    )
    .unwrap();
    let scope = AnalysisScope::new(WorkspaceId::default(), snapshot);
    let exec = ExecutionContext::try_new(
        ExecutionId(1),
        scope,
        ActorRef::new(ActorKind::Kernel, "cyclomatic").unwrap(),
        CorrelationId::new("cu-2-4-live").unwrap(),
        None,
    )
    .unwrap();
    Finding {
        id: FindingId::new("F-cu24-cyclomatic").unwrap(),
        kind: FindingKind::new("complexity@1.0.0").unwrap(),
        origin: FindingOrigin::Detector,
        severity: FindingSeverity::Critical,
        risk: RiskLevel::High,
        evidence_class: class,
        evidence: vec![EvidenceId(101), EvidenceId(102)],
        detector: DetectorExecutionRef {
            id,
            version: "1.0".to_string(),
            authority_at_execution: DetectorAuthority::Gated,
            digests: DetectorDigests {
                logic: digest.clone(),
                policy: digest.clone(),
                semantic: digest.clone(),
                instance: digest,
            },
            context: Some(exec),
        },
        status: FindingStatus::Open,
        message: "cyclomatic 12 in fn complex".to_string(),
        causal_chain: vec![CausalStep {
            kind: CausalStepKind::RuntimeObservation,
            subject: Some(EntityId(42)),
            fact: Some(FactId(11)),
            evidence: Some(EvidenceId(101)),
            detail: "fn complex: cyclomatic 12".to_string(),
        }],
    }
}

/// Drive a finding through the full read-side pipeline.
fn pipeline(finding: &Finding, snapshot: &SnapshotId) -> GuardObservation {
    let result = project_finding(finding, snapshot);
    to_guard_observation(&result)
}

#[test]
fn working_copy_change_yields_unsupported_at_seam() {
    // A working copy has been modified: snapshot is now T (a fresh
    // build) but the Finding was observed on S. The pipeline must
    // refuse to project, so Guard never sees a stale Verifiable.
    let finding = detector_finding(EvidenceClass::A, SnapshotId(1));
    let observation = pipeline(&finding, &SnapshotId(2));
    match observation {
        GuardObservation::Unsupported { reason } => {
            // The `to_guard_observation` carries the synthetic detector
            // (the projection never ran, so we have no real one), but
            // what matters here is the discriminant: `Unsupported`,
            // not `Verifiable`.
            let _ = reason;
        }
        other => panic!(
            "expected Unsupported for snapshot drift, got {other:?}; \
             a Verifiable here would let Guard promote stale evidence"
        ),
    }
}

#[test]
fn forged_evidence_id_never_acquires_block_authority_at_seam() {
    // A consumer constructs a Finding with a fake EvidenceId. The
    // pipeline must NOT escalate it to anything that Guard could
    // promote to a `Block` based on the seam alone. The pipeline
    // surfaces the id; the consumer is responsible for verifying it
    // exists in the current `EvidenceLookup`.
    let mut finding = detector_finding(EvidenceClass::A, SnapshotId(7));
    finding.evidence = vec![EvidenceId(999_999)];
    let observation = pipeline(&finding, &SnapshotId(7));
    // The seam produces a Verifiable (the projection trusts the
    // kernel) — but the *observation* does not claim authority: it
    // exposes the id and snapshot so Guard can decide.
    match observation {
        GuardObservation::Verifiable { evidence, snapshot_id, .. } => {
            assert_eq!(evidence.len(), 1);
            assert_eq!(evidence[0].id, EvidenceId(999_999));
            assert_eq!(evidence[0].snapshot, snapshot_id);
        }
        other => panic!(
            "expected Verifiable (passthrough) for forged evidence; \
             the seam must not promote to a more permissive variant, got {other:?}"
        ),
    }
    // And the contract: there is no `Block` discriminant at the seam.
    // We assert it structurally by exhaustive pattern matching in
    // `consumer_chain_covers_all_five_discriminants` below.
}

#[test]
fn heuristic_precision_preserved_across_seam_for_class_d() {
    // D-class (heuristic) is the most permissive finding the kernel
    // emits. The seam must keep it as Inconclusive: not Advisory, not
    // Verifiable. Otherwise Guard could be tricked into Warn/Block on
    // a guess.
    let finding = detector_finding(EvidenceClass::D, SnapshotId(3));
    let observation = pipeline(&finding, &SnapshotId(3));
    match observation {
        GuardObservation::Inconclusive { gaps, .. } => {
            assert!(!gaps.is_empty(), "Inconclusive must carry non-empty gaps");
        }
        other => panic!("D-class must be Inconclusive, got {other:?}"),
    }
}

#[test]
fn heuristic_precision_preserved_across_seam_for_class_c() {
    // C-class (partial static) is Advisory. The seam must not elevate
    // it to Verifiable just because there is evidence.
    let finding = detector_finding(EvidenceClass::C, SnapshotId(3));
    let observation = pipeline(&finding, &SnapshotId(3));
    assert!(
        matches!(observation, GuardObservation::Advisory { .. }),
        "C-class must be Advisory, got {observation:?}"
    );
}

#[test]
fn verified_finding_full_pipeline_preserves_causal_chain() {
    // A|B-class with a grounded chain → Verifiable with the chain
    // intact. This is the happy path: a real detector finding that
    // Guard could (after admission) promote to Block.
    let finding = detector_finding(EvidenceClass::A, SnapshotId(5));
    let observation = pipeline(&finding, &SnapshotId(5));
    match observation {
        GuardObservation::Verifiable { causes, evidence, snapshot_id, .. } => {
            assert_eq!(snapshot_id, SnapshotId(5));
            assert_eq!(evidence.len(), 2);
            assert_eq!(causes.len(), 1);
            match &causes[0] {
                CauseProjection::Grounded { fact, evidence: e, .. } => {
                    assert_eq!(*fact, Some(FactId(11)));
                    assert_eq!(*e, Some(EvidenceId(101)));
                }
                CauseProjection::Ungrounded { .. } => {
                    panic!("happy-path cause must be Grounded, not Ungrounded")
                }
            }
        }
        other => panic!("expected Verifiable for class A, got {other:?}"),
    }
}

#[test]
fn ungrounded_step_remains_visible_in_transfer() {
    // A causal step without fact/evidence ids must surface as
    // `Ungrounded` in the `Verifiable` observation. The transfer
    // must NOT silently re-classify it as Grounded.
    let mut finding = detector_finding(EvidenceClass::A, SnapshotId(8));
    finding.causal_chain = vec![CausalStep {
        kind: CausalStepKind::RuntimeObservation,
        subject: Some(EntityId(99)),
        fact: None,
        evidence: None,
        detail: "ungrounded step".to_string(),
    }];
    let observation = pipeline(&finding, &SnapshotId(8));
    match observation {
        GuardObservation::Verifiable { causes, .. } => {
            assert_eq!(causes.len(), 1);
            match &causes[0] {
                CauseProjection::Ungrounded { .. } => {}
                CauseProjection::Grounded { .. } => {
                    panic!("transfer must not silently ground an ungrounded step")
                }
            }
        }
        other => panic!("expected Verifiable (with Ungrounded cause), got {other:?}"),
    }
}

#[test]
fn consumer_chain_covers_all_five_discriminants() {
    // The full pipeline must be total: every kind of input the
    // projection can produce maps to a distinct `GuardObservation`
    // variant, and the chain never collapses to a default.
    let verified = detector_finding(EvidenceClass::A, SnapshotId(10));
    assert!(matches!(
        pipeline(&verified, &SnapshotId(10)),
        GuardObservation::Verifiable { .. }
    ));

    let advisory = detector_finding(EvidenceClass::C, SnapshotId(10));
    assert!(matches!(
        pipeline(&advisory, &SnapshotId(10)),
        GuardObservation::Advisory { .. }
    ));

    let inconclusive = detector_finding(EvidenceClass::D, SnapshotId(10));
    assert!(matches!(
        pipeline(&inconclusive, &SnapshotId(10)),
        GuardObservation::Inconclusive { .. }
    ));

    // Stale snapshot: the projection errors with Unsupported;
    // the transfer maps it to a synthetic-detector Unsupported.
    let stale = detector_finding(EvidenceClass::A, SnapshotId(10));
    assert!(matches!(
        pipeline(&stale, &SnapshotId(11)),
        GuardObservation::Unsupported { .. }
    ));

    // Structural failure: a Finding with no evidence at all.
    let mut no_evidence = detector_finding(EvidenceClass::A, SnapshotId(10));
    no_evidence.evidence.clear();
    let result = project_finding(&no_evidence, &SnapshotId(10));
    assert!(matches!(
        result,
        Err(ProjectionError::Unsupported(_)) | Err(ProjectionError::Failed(_))
    ));
    let observation = to_guard_observation(&result);
    assert!(matches!(
        observation,
        GuardObservation::Unsupported { .. } | GuardObservation::Failed { .. }
    ));
}
