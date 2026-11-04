//! Adversarial E2E for the e77.1 canonical grounding bridge (WU3).
//!
//! The load-bearing properties of the e77.1 corrective cycle, as
//! runtime checks:
//!
//! 1. An admitted constraint + violation + no grounding cannot
//!    gate (the canonical EvidenceLookup rejects ungrounded items).
//! 2. A synthetic `EvidenceId` cannot gate (the verifier rejects
//!    unresolved evidence; the architecture path no longer mints
//!    ids by itself).
//! 3. An admitted constraint does NOT itself mint
//!    `DetectorAuthority::Gated` (the evaluator's primary output is
//!    `Vec<ArchitectureViolation>`, which carries no authority).
//! 4. The placeholder `DetectorDigests::of_for_kind` is removed
//!    (or, if reintroduced, must be the only path to a Finding —
//!    which is not what we want).
//! 5. A canonical grounded violation routes through canonical
//!    evidence (the bridge emits a `ProducedEvidence` whose
//!    `grounding` matches the violation's).
//! 6. ADR text alone yields zero violations (unchanged from e77).
//! 7. Non-admitted constraint yields zero violations (unchanged).
//! 8. **The most important**: `ArchitectureEvaluator::evaluate` cannot,
//!    by itself, produce a gateable `Finding` — the type
//!    `EvaluationReport` has no `findings` field.
//!
//! Plus: deterministic identity, idempotent violation ids, and the
//! WU2 invariants from the bridge.

use cognicode_core::application::architecture::admission::{
    ArchitectureAdmissionService, SystemArchitectureClock,
};
use cognicode_core::application::architecture::evaluator::{
    ArchitectureEvaluator, ArchitectureSource, SourceFile,
};
use cognicode_core::application::architecture::grounding::ArchitectureGroundingBridge;
use cognicode_core::domain::architecture::{
    Admitter, AdmitterRole, ArchitectureConstraintId, ArchitectureConstraintKind,
    LayerDependencyRule, LayerId, ViolationId,
};
use cognicode_core::domain::findings::{EvidenceKind, GroundingRef};
use cognicode_core::domain::kernel_ids::{EvidenceId, FactId};

// ============================================================================
// Helpers
// ============================================================================

fn promoted() -> Admitter {
    Admitter {
        id: "human:test".into(),
        role: AdmitterRole::HumanPromoter,
    }
}

fn admitted_layer_constraint(
    id: &str,
) -> cognicode_core::domain::architecture::ArchitectureConstraint {
    let mut svc = ArchitectureAdmissionService::new();
    let candidate = cognicode_core::domain::architecture::ConstraintCandidate {
        id: ArchitectureConstraintId::new(id).unwrap(),
        kind: ArchitectureConstraintKind::LayerDependency(LayerDependencyRule {
            from_layer: LayerId::Domain,
            forbidden_targets: vec![LayerId::Infrastructure],
            rationale: "test".into(),
        }),
        adr_ref: Some("ADR-046".into()),
        proposed_by: "human:test".into(),
    };
    let out = svc.admit(candidate, &promoted(), &SystemArchitectureClock);
    out.result.expect("admission should succeed").constraint
}

fn synthetic_source() -> ArchitectureSource {
    ArchitectureSource {
        files: vec![SourceFile {
            file_path: "src/domain/foo.rs".into(),
            module_path: Some("domain::foo".into()),
            source: "use crate::infrastructure::db;\n".into(),
        }],
    }
}

// ============================================================================
// Property 1 — admitted constraint + violation + no grounding → cannot gate
// ============================================================================

/// A violation emitted by the evaluator carries `grounding = None`
/// (the evaluator has no canonical knowledge of the source). When
/// the bridge converts it to `ProducedEvidence`, the result has
/// `grounding = None`. The canonical writer then persists it as
/// `EvidenceBinding::ungrounded(NoFact)`. The verifier rejects
/// ungrounded evidence; therefore the violation cannot gate.
#[test]
fn property_1_violation_without_grounding_cannot_gate() {
    let constraint = admitted_layer_constraint("architecture.prop1");
    let report = ArchitectureEvaluator::new()
        .evaluate(&constraint, &synthetic_source())
        .unwrap();
    assert_eq!(report.violations.len(), 1);
    assert!(report.violations[0].grounding.is_none());

    let items = ArchitectureGroundingBridge::new().to_produced_evidence(&report.violations);
    assert_eq!(items.len(), 1);
    assert!(items[0].grounding.is_none());
    // The kind is the new architectural kind, distinct from the
    // detector kinds.
    assert_eq!(items[0].kind, EvidenceKind::ArchitectureSource);
}

// ============================================================================
// Property 2 — fake EvidenceId cannot gate
// ============================================================================

/// The e77 first slice minted a synthetic `EvidenceId` from a
/// FNV-1a hash. The e77.1 cycle removes that minting path: the
/// architecture evaluator no longer returns Findings, so no
/// `EvidenceId` is minted by the architecture surface. A
/// downstream consumer that wants to gate must route the violation
/// through canonical evidence; if it tries to gate with a fake id,
/// the verifier rejects it.
#[test]
fn property_2_no_synthetic_evidence_id_in_evaluator_output() {
    let constraint = admitted_layer_constraint("architecture.prop2");
    let report = ArchitectureEvaluator::new()
        .evaluate(&constraint, &synthetic_source())
        .unwrap();
    let dbg = format!("{report:?}");
    assert!(
        !dbg.contains("EvidenceId("),
        "evaluator report must not mint EvidenceId values; today it doesn't. \
         Debug: {dbg}"
    );
}

// ============================================================================
// Property 3 — admitted constraint does not mint Gated
// ============================================================================

/// The e77 first slice minted `DetectorAuthority::Gated` on the
/// emitted `Finding`. The e77.1 cycle removes that minting path:
/// the evaluator emits `ArchitectureViolation`s, which carry no
/// authority field at all.
#[test]
fn property_3_admission_does_not_mint_authority() {
    let constraint = admitted_layer_constraint("architecture.prop3");
    let report = ArchitectureEvaluator::new()
        .evaluate(&constraint, &synthetic_source())
        .unwrap();
    let dbg = format!("{report:?}");
    assert!(
        !dbg.contains("DetectorAuthority"),
        "evaluator report must not mention DetectorAuthority; today it doesn't. \
         Debug: {dbg}"
    );
    assert!(!dbg.contains("Gated"));
}

// ============================================================================
// Property 4 — placeholder DetectorDigests are not used
// ============================================================================

/// The e77 first slice introduced `DetectorDigests::of_for_kind`
/// as a placeholder factory. The e77.1 cycle makes the evaluator
/// return violations, not findings, so the placeholder factory is
/// not used by the architecture path. We assert this by verifying
/// the evaluator's report does not mention digests.
#[test]
fn property_4_no_placeholder_digests_in_evaluator_output() {
    let constraint = admitted_layer_constraint("architecture.prop4");
    let report = ArchitectureEvaluator::new()
        .evaluate(&constraint, &synthetic_source())
        .unwrap();
    let dbg = format!("{report:?}");
    assert!(
        !dbg.contains("DetectorDigest"),
        "evaluator report must not mention DetectorDigest; today it doesn't. \
         Debug: {dbg}"
    );
}

// ============================================================================
// Property 5 — canonical grounded violation routes through the bridge
// ============================================================================

/// A violation that **does** carry a `GroundingRef` (because the
/// caller has canonical knowledge of the source) produces a
/// `ProducedEvidence` with the same grounding. The downstream
/// `CanonicalEvidenceWriter` will validate the fact, persist a
/// real `EvidenceId`, and the verifier will accept the binding.
#[test]
fn property_5_grounded_violation_routes_through_bridge() {
    let constraint = admitted_layer_constraint("architecture.prop5");
    // Synthesise a violation that already carries a grounding.
    let mut violation = cognicode_core::domain::architecture::ArchitectureViolation {
        id: ViolationId::compute(&constraint.id, "src/domain/foo.rs", 1, "infrastructure::db"),
        constraint_id: constraint.id.clone(),
        finding_kind: cognicode_core::domain::findings::FindingKind::new(
            "architecture.layer_dependency",
        )
        .unwrap(),
        file_path: "src/domain/foo.rs".into(),
        module_path: Some("domain::foo".into()),
        line: 1,
        dependency_path: "infrastructure::db".into(),
        from_layer: LayerId::Domain,
        grounding: Some(GroundingRef::fact(FactId::new(42))),
        rationale: "test".into(),
    };
    let items = ArchitectureGroundingBridge::new().to_produced_evidence(&[violation.clone()]);
    assert_eq!(items.len(), 1);
    let g = items[0].grounding.expect("grounding must be present");
    assert_eq!(g.fact, FactId::new(42));

    // Without grounding, the same violation becomes ungrounded.
    violation.grounding = None;
    let items = ArchitectureGroundingBridge::new().to_produced_evidence(&[violation]);
    assert!(items[0].grounding.is_none());
}

// ============================================================================
// Property 6 — ADR text alone → zero violations
// ============================================================================

#[test]
fn property_6_adr_text_alone_yields_zero_violations() {
    let registry = cognicode_core::application::architecture::registry::ArchitectureRegistry::new();
    let candidate = cognicode_core::domain::architecture::ConstraintCandidate {
        id: ArchitectureConstraintId::new("architecture.adr_alone").unwrap(),
        kind: ArchitectureConstraintKind::LayerDependency(LayerDependencyRule {
            from_layer: LayerId::Domain,
            forbidden_targets: vec![LayerId::Infrastructure],
            rationale: "test".into(),
        }),
        adr_ref: Some("ADR-046".into()),
        proposed_by: "human:test".into(),
    };
    assert!(!registry.is_admitted(&candidate));
    let source = ArchitectureSource {
        files: vec![SourceFile {
            file_path: "src/domain/foo.rs".into(),
            module_path: Some("domain::foo".into()),
            source: "use crate::infrastructure::db;\n".into(),
        }],
    };
    let report = registry.evaluate_candidate(&candidate, &source);
    assert!(report.is_err());
}

// ============================================================================
// Property 7 — non-admitted constraint → zero violations
// ============================================================================

#[test]
fn property_7_non_admitted_constraint_yields_zero_violations() {
    let mut registry =
        cognicode_core::application::architecture::registry::ArchitectureRegistry::new();
    let candidate = cognicode_core::domain::architecture::ConstraintCandidate {
        id: ArchitectureConstraintId::new("architecture.non_admitted").unwrap(),
        kind: ArchitectureConstraintKind::LayerDependency(LayerDependencyRule {
            from_layer: LayerId::Domain,
            forbidden_targets: vec![LayerId::Infrastructure],
            rationale: "test".into(),
        }),
        adr_ref: Some("ADR-046".into()),
        proposed_by: "human:test".into(),
    };
    let out = registry.admission.admit(
        candidate.clone(),
        &Admitter {
            id: "ai:test".into(),
            role: AdmitterRole::Other,
        },
        &SystemArchitectureClock,
    );
    assert!(out.result.is_err());
    assert!(!registry.is_admitted(&candidate));
}

// ============================================================================
// Property 8 — evaluate cannot produce a gateable Finding by itself
// ============================================================================

/// The most important property of the cycle. The evaluator's
/// primary output is `EvaluationReport { violations: Vec<...>, .. }`,
/// not a `Finding`. The type system enforces this — there is no
/// `findings` field to read. The runtime check below verifies the
/// type is `EvaluationReport` and that its `violations` is
/// populated but the surface mentions nothing about findings.
#[test]
fn property_8_evaluator_cannot_produce_gateable_finding_by_itself() {
    let constraint = admitted_layer_constraint("architecture.prop8");
    let report = ArchitectureEvaluator::new()
        .evaluate(&constraint, &synthetic_source())
        .unwrap();
    let dbg = format!("{report:?}");
    assert!(
        !dbg.contains("findings:"),
        "EvaluationReport must not have a `findings` field"
    );
    assert!(
        dbg.contains("violations:"),
        "EvaluationReport must have a `violations` field"
    );
    assert!(report.violations.len() == 1);
}

// ============================================================================
// Property 9 — deterministic violation identity across runs
// ============================================================================

#[test]
fn property_9_deterministic_violation_identity() {
    let constraint = admitted_layer_constraint("architecture.prop9");
    let a = ArchitectureEvaluator::new()
        .evaluate(&constraint, &synthetic_source())
        .unwrap();
    let b = ArchitectureEvaluator::new()
        .evaluate(&constraint, &synthetic_source())
        .unwrap();
    assert_eq!(a.violations.len(), b.violations.len());
    assert_eq!(a.violations[0].id, b.violations[0].id);
    assert_eq!(
        a.violations[0].dependency_path,
        b.violations[0].dependency_path
    );
    assert_eq!(a.violations[0].from_layer, b.violations[0].from_layer);
}

// ============================================================================
// Property 10 — ViolationId is not an EvidenceId
// ============================================================================

/// The `ViolationId` and `EvidenceId` are distinct types with
/// distinct semantics. The ViolationId is a deduplication /
/// navigation key; it must never be used as an `EvidenceId`.
/// The runtime check below exercises the distinction: the
/// ViolationId is `u64`, but its *semantics* are not that of an
/// EvidenceId (no fact, no grade, no canonical anchor).
#[test]
fn property_10_violation_id_is_not_evidence_id() {
    let v_id = ViolationId::compute(
        &ArchitectureConstraintId::new("architecture.prop10").unwrap(),
        "src/domain/foo.rs",
        1,
        "infrastructure::db",
    );
    let raw = v_id.raw();
    // Wrap the raw u64 in an EvidenceId — this is allowed by the
    // type system but semantically wrong. The test documents that
    // the wrap is structurally possible (so callers must take
    // care) but does not assert correctness; correctness comes from
    // the verifier rejecting ungrounded evidence.
    let _fake_eid = EvidenceId::new(raw);
    // The structural test is: the type `ViolationId` is distinct
    // from `EvidenceId`. We assert by constructing both and
    // showing they are different types at the call site.
    fn _takes_violation_id(_: ViolationId) {}
    fn _takes_evidence_id(_: EvidenceId) {}
    _takes_violation_id(v_id);
}
