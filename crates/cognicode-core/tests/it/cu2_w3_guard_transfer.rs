//! CU-2.3 — Contrato de transferencia a Guard.
//!
//! Estos tests pinan el seam CogniCode → Guard. La funcion
//! `to_guard_observation` no firma nada, no fabrica `admitted_at`,
//! no inventa `AdmitterRole::HumanPromoter`, y nunca devuelve un
//! payload que Guard pueda promover a `Block` sin verificar
//! admission contra su propio trust root.
//!
//! Spec: SPEC-CU-003.3, .4, .5.

use cognicode_core::domain::execution::actor::{ActorKind, ActorRef};
use cognicode_core::domain::execution::context::ExecutionContext;
use cognicode_core::domain::execution::correlation::CorrelationId;
use cognicode_core::domain::execution::scope::AnalysisScope;
use cognicode_core::domain::findings::guard_transfer::{GuardObservation, to_guard_observation};
use cognicode_core::domain::findings::projection::{
    FindingProjection, TypedEvidenceHandle, UnknownReason, project_finding,
};
use cognicode_core::domain::findings::{
    CausalStep, CausalStepKind, DetectorAuthority, DetectorDigest, DetectorDigests,
    DetectorExecutionRef, DetectorId, EvidenceClass, Finding, FindingId, FindingKind,
    FindingOrigin, FindingSeverity, FindingStatus, RiskLevel,
};
use cognicode_core::domain::kernel_ids::{EntityId, EvidenceId, ExecutionId, FactId, SnapshotId};
use cognicode_core::domain::value_objects::workspace_id::WorkspaceId;

fn test_finding(class: EvidenceClass, snapshot: SnapshotId, with_cause: bool) -> Finding {
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
    let causal_chain = if with_cause {
        vec![CausalStep {
            kind: CausalStepKind::RuntimeObservation,
            subject: Some(EntityId(42)),
            fact: Some(FactId(11)),
            evidence: Some(EvidenceId(7)),
            detail: "fn foo: cyclomatic 12".to_string(),
        }]
    } else {
        vec![]
    };
    Finding {
        id: FindingId::new("F-cu23").unwrap(),
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
        causal_chain,
    }
}

#[test]
fn verified_finding_transfers_to_verifiable() {
    let snapshot = SnapshotId(42);
    let finding = test_finding(EvidenceClass::A, snapshot, true);
    let projection = project_finding(&finding, &snapshot).expect("verified projects");
    let observation = to_guard_observation(&Ok(projection));
    match observation {
        GuardObservation::Verifiable {
            snapshot_id,
            evidence,
            ..
        } => {
            assert_eq!(snapshot_id, snapshot);
            assert_eq!(evidence.len(), 2);
        }
        other => panic!("expected Verifiable, got {other:?}"),
    }
}

#[test]
fn advisory_finding_transfers_to_advisory_never_verifiable() {
    let snapshot = SnapshotId(42);
    let finding = test_finding(EvidenceClass::C, snapshot, true);
    let projection = project_finding(&finding, &snapshot).expect("advisory projects");
    let observation = to_guard_observation(&Ok(projection));
    assert!(
        matches!(observation, GuardObservation::Advisory { .. }),
        "class C must be Advisory, never Verifiable; got {observation:?}"
    );
}

#[test]
fn inconclusive_finding_transfers_to_inconclusive() {
    let snapshot = SnapshotId(42);
    let finding = test_finding(EvidenceClass::D, snapshot, true);
    // A class-D finding that carries its provenance projects to
    // `Ok(Inconclusive { gaps })`; `Err(Inconclusive)` is reserved for a
    // finding whose context the projection cannot recover. Either way the
    // Guard must observe Inconclusive — that is what this test is about:
    // an inconclusive verdict never silently becomes Verifiable.
    let projection = project_finding(&finding, &snapshot).expect("class D projects");
    let observation = to_guard_observation(&Ok(projection));
    match observation {
        GuardObservation::Inconclusive { gaps, .. } => {
            assert!(!gaps.is_empty(), "Inconclusive must carry non-empty gaps");
        }
        other => panic!("expected Inconclusive, got {other:?}"),
    }
}

#[test]
fn snapshot_mismatch_projects_to_unsupported() {
    let observed = SnapshotId(42);
    let requested = SnapshotId(99);
    let finding = test_finding(EvidenceClass::A, observed, true);
    let projection_err = project_finding(&finding, &requested).expect_err("mismatch errors");
    let observation = to_guard_observation(&Err(projection_err));
    assert!(
        matches!(
            observation,
            GuardObservation::Unsupported {
                reason: UnknownReason::SnapshotMismatch { .. }
            }
        ),
        "expected Unsupported(SnapshotMismatch), got {observation:?}"
    );
}

#[test]
fn evidence_handle_carries_snapshot_into_guard_observation() {
    let snapshot = SnapshotId(42);
    let finding = test_finding(EvidenceClass::A, snapshot, true);
    let projection = project_finding(&finding, &snapshot).expect("verified projects");
    let projection_clone = projection.clone();
    if let FindingProjection::Verified { evidence, .. } = &projection {
        let observation = to_guard_observation(&Ok(projection_clone));
        if let GuardObservation::Verifiable {
            evidence: obs_evidence,
            ..
        } = observation
        {
            for h in &obs_evidence {
                assert_eq!(h.snapshot, snapshot);
            }
            assert_eq!(obs_evidence.len(), evidence.len());
        } else {
            panic!("expected Verifiable");
        }
    } else {
        panic!("expected Verified projection");
    }
}

#[test]
fn adversarial_fabricated_evidence_id_does_not_promote_to_advisory() {
    // El transfer es un passthrough: si el projection dice Verified,
    // el transfer dice Verifiable. La defensa contra evidence forjada
    // vive en el projection (cognicode-core) y en el admission verifier
    // de Guard, NO en el transfer. Aqui verificamos que el transfer
    // no INTRODUCE un AdmitterRole ni nada parecido al "fabricar" un
    // Verifiable.
    let snapshot = SnapshotId(42);
    let finding = test_finding(EvidenceClass::A, snapshot, true);
    let projection = project_finding(&finding, &snapshot).expect("verified projects");
    let detector = match &projection {
        FindingProjection::Verified { detector, .. } => detector.clone(),
        _ => unreachable!(),
    };
    let forged = FindingProjection::Verified {
        snapshot_id: snapshot,
        detector,
        severity: FindingSeverity::Critical,
        evidence: vec![TypedEvidenceHandle {
            id: EvidenceId(999_999),
            snapshot,
        }],
        causes: vec![],
        message: "forged".to_string(),
    };
    let observation = to_guard_observation(&Ok(forged));
    // El transfer dice Verifiable, pero la defensa contra
    // admission es responsabilidad de Guard. Este test solo asegura
    // que el transfer no escala el hallazgo a un estado mas
    // permisivo que el projection original.
    match observation {
        GuardObservation::Verifiable { evidence, .. } => {
            assert_eq!(evidence.len(), 1);
            assert_eq!(evidence[0].id, EvidenceId(999_999));
        }
        other => panic!("transfer must be passthrough (Verifiable); got {other:?}"),
    }
}

#[test]
fn adversarial_signature_bytes_never_carry_into_observation() {
    let snapshot = SnapshotId(42);
    let finding = test_finding(EvidenceClass::A, snapshot, true);
    let projection = project_finding(&finding, &snapshot).expect("verified projects");
    let observation = to_guard_observation(&Ok(projection));
    let debug = format!("{observation:?}");
    assert!(
        !debug.contains("signature"),
        "GuardObservation must not carry signature field; debug: {debug}"
    );
    assert!(
        !debug.contains("admitted_at"),
        "GuardObservation must not carry admitted_at; debug: {debug}"
    );
    assert!(
        !debug.contains("AdmitterRole"),
        "GuardObservation must not carry AdmitterRole; debug: {debug}"
    );
}

#[test]
fn adversarial_ungrounded_step_is_observable_in_verifiable_causes() {
    // Un causal step sin fact ni evidence debe ser visible en la
    // observacion Verifiable como Ungrounded, no debe esconderse.
    // Esto protege contra "fabricar" grounded chains en el transfer.
    let snapshot = SnapshotId(42);
    let mut finding = test_finding(EvidenceClass::A, snapshot, false);
    finding.causal_chain = vec![CausalStep {
        kind: CausalStepKind::RuntimeObservation,
        subject: Some(EntityId(42)),
        fact: None,
        evidence: None,
        detail: "ungrounded step".to_string(),
    }];
    let projection = project_finding(&finding, &snapshot).expect("projects (with gap in cause)");
    let observation = to_guard_observation(&Ok(projection));
    match observation {
        GuardObservation::Verifiable { causes, .. } => {
            // Debe haber al menos un cause visible (el transfer
            // preserva la cadena; el consumer es quien decide
            // si el Ungrounded bloquea o no).
            assert_eq!(causes.len(), 1);
        }
        other => panic!("expected Verifiable with visible causes; got {other:?}"),
    }
}
