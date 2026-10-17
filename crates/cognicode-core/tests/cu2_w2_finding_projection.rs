//! CU-2.2 — Read-side projection of verified findings.
//!
//! Contract: a function in `domain::findings::projection` takes a slice
//! of `Finding`s and emits a `FindingProjection` ADT that preserves the
//! typed kernel ids (SnapshotId, EvidenceId, FactId) and discriminates
//! Verified / Advisory / Inconclusive / Unsupported / Failed per
//! SPEC-CU-003.3.
//!
//! This is the seam between CogniCode (technical authority) and Guard
//! (admission authority). Guard never sees a `Finding` directly; it
//! sees a `FindingProjection` and must decide based on that.

use cognicode_core::domain::execution::actor::{ActorKind, ActorRef};
use cognicode_core::domain::execution::context::ExecutionContext;
use cognicode_core::domain::execution::correlation::CorrelationId;
use cognicode_core::domain::execution::scope::AnalysisScope;
use cognicode_core::domain::findings::projection::{
    CauseProjection, FindingProjection, Gap, ProjectionError, UnknownReason, project_finding,
};
use cognicode_core::domain::findings::{
    CausalStep, CausalStepKind, DetectorAuthority, DetectorDigest, DetectorDigests,
    DetectorExecutionRef, DetectorId, EvidenceClass, Finding, FindingId, FindingKind,
    FindingOrigin, FindingSeverity, FindingStatus, RiskLevel,
};
use cognicode_core::domain::kernel_ids::{EntityId, EvidenceId, ExecutionId, FactId, SnapshotId};
use cognicode_core::domain::value_objects::workspace_id::WorkspaceId;

fn test_finding_with_class_and_snapshot(class: EvidenceClass, snapshot: SnapshotId) -> Finding {
    let id = DetectorId::new("cyclomatic@1.0.0").unwrap();
    let digest = DetectorDigest::new(
        "sha256:0000000000000000000000000000000000000000000000000000000000000000",
    )
    .unwrap();
    let scope = AnalysisScope::new(WorkspaceId::default(), snapshot);
    let exec = ExecutionContext::try_new(
        ExecutionId(1),
        scope,
        ActorRef::new(ActorKind::Kernel, "test").unwrap(),
        CorrelationId::new("test-correlation").unwrap(),
        None,
    )
    .unwrap();
    Finding {
        id: FindingId::new("F-1").unwrap(),
        kind: FindingKind::new("complexity@1.0.0").unwrap(),
        origin: FindingOrigin::Detector,
        severity: FindingSeverity::Critical,
        risk: RiskLevel::High,
        evidence_class: class,
        evidence: vec![EvidenceId(7), EvidenceId(8)],
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
        message: "complexity too high".to_string(),
        causal_chain: vec![CausalStep {
            kind: CausalStepKind::RuntimeObservation,
            subject: Some(EntityId(42)),
            fact: Some(FactId(11)),
            evidence: Some(EvidenceId(7)),
            detail: "fn foo: cyclomatic 12".to_string(),
        }],
    }
}

fn fixed_finding() -> Finding {
    test_finding_with_class_and_snapshot(EvidenceClass::A, SnapshotId(42))
}

#[test]
fn verified_finding_projects_to_verified() {
    let finding = fixed_finding();
    let snapshot = SnapshotId(42);
    let p = project_finding(&finding, &snapshot).expect("verified finding projects");
    match p {
        FindingProjection::Verified {
            snapshot_id,
            evidence,
            causes,
            ..
        } => {
            assert_eq!(snapshot_id, SnapshotId(42));
            assert_eq!(evidence.len(), 2);
            assert!(evidence.iter().any(|h| h.id == EvidenceId(7)));
            assert!(evidence.iter().any(|h| h.id == EvidenceId(8)));
            assert_eq!(causes.len(), 1);
        }
        other => panic!("expected Verified, got {other:?}"),
    }
}

#[test]
fn advisory_observation_projects_to_advisory() {
    // Class C with evidence is the realistic advisory shape: partial
    // static evidence, no causal grounding required.
    let f = test_finding_with_class_and_snapshot(EvidenceClass::C, SnapshotId(1));
    let p = project_finding(&f, &SnapshotId(1)).expect("class C projects");
    assert!(
        matches!(p, FindingProjection::Advisory { .. }),
        "class C (partial) must be Advisory, got {p:?}"
    );
}

#[test]
fn missing_snapshot_makes_finding_unsupported() {
    // Finding observed on snapshot 42; asked about snapshot 99.
    let f = test_finding_with_class_and_snapshot(EvidenceClass::A, SnapshotId(42));
    let p = project_finding(&f, &SnapshotId(99));
    match p {
        Err(ProjectionError::Unsupported(reason)) => {
            // A finding made on snapshot != current cannot be claimed.
            assert!(matches!(reason, UnknownReason::SnapshotMismatch { .. }));
        }
        other => panic!("expected Unsupported, got {other:?}"),
    }
}

#[test]
fn unknown_evidence_class_makes_finding_inconclusive() {
    // Class D (heuristic) projects to Inconclusive with a gap list.
    let f = test_finding_with_class_and_snapshot(EvidenceClass::D, SnapshotId(42));
    let p = project_finding(&f, &SnapshotId(42));
    match p {
        Err(ProjectionError::Inconclusive(gaps)) => {
            assert!(
                gaps.iter()
                    .any(|g| matches!(g, Gap::EvidenceClassBelowThreshold { .. }))
            );
        }
        other => panic!("expected Inconclusive, got {other:?}"),
    }
}

#[test]
fn evidence_handle_carries_snapshot_bound() {
    // The projection must bind every EvidenceId to the snapshot it was
    // observed under. A consumer (Guard) that wants to use an evidence
    // id needs the snapshot to verify it; without that binding the
    // consumer could promote a snapshot from another workspace.
    let f = fixed_finding();
    let snapshot = SnapshotId(42);
    let p = project_finding(&f, &snapshot).unwrap();
    if let FindingProjection::Verified { evidence, .. } = p {
        for h in &evidence {
            assert_eq!(
                h.snapshot, snapshot,
                "evidence handle must carry its snapshot"
            );
        }
    } else {
        panic!("expected Verified");
    }
}

#[test]
fn cause_preserves_fact_id_and_evidence_id() {
    let f = fixed_finding();
    let p = project_finding(&f, &SnapshotId(42)).unwrap();
    if let FindingProjection::Verified { causes, .. } = p {
        match &causes[0] {
            CauseProjection::Grounded { fact, evidence, .. } => {
                assert_eq!(*fact, Some(FactId(11)));
                assert_eq!(*evidence, Some(EvidenceId(7)));
            }
            CauseProjection::Ungrounded { .. } => panic!("causal step is grounded"),
        }
    } else {
        panic!("expected Verified");
    }
}
