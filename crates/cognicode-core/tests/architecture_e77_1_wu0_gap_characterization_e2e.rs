//! WU0 — characterization of the e77 trust-boundary gap (cycle e77.1).
//!
//! These tests pin the **post-e77.1 contract** for the architecture
//! evaluator. They currently FAIL because the e77 first slice
//! (commits `20764bf5` / `fe8804b1`) emits `Finding` instances as its
//! primary output, with:
//!
//! * `DetectorAuthority::Gated` minted at the call site;
//! * synthetic FNV-1a `EvidenceId` values;
//! * placeholder `DetectorDigests` derived from a `FindingKind`.
//!
//! None of those values are canonical truth, yet the evaluator
//! declares the findings to be the result of evaluation. The
//! post-e77.1 evaluator must split detection from authority:
//!
//! ```text
//! ArchitectureEvaluator::evaluate
//!     → Vec<ArchitectureViolation>   (the *primary* output)
//!     → no Finding, no DetectorAuthority, no EvidenceId
//! ```
//!
//! Assembly into a `Finding` happens **downstream**, only after the
//! violation has been routed through canonical evidence. The tests
//! below fail today because the API is wrong; they will pass when
//! WU1+ lands.
//!
//! Naming convention: each test is named `claim_<N>_<property>`. The
//! tests are runtime checks, not compile-time, so they are easy to
//! run and easy to update as WU1 progresses.

use cognicode_core::application::architecture::admission::{
    ArchitectureAdmissionService, SystemArchitectureClock,
};
use cognicode_core::application::architecture::evaluator::{
    ArchitectureEvaluator, ArchitectureSource, SourceFile,
};
use cognicode_core::domain::architecture::{
    Admitter, AdmitterRole, ArchitectureConstraintId, ArchitectureConstraintKind,
    LayerDependencyRule, LayerId,
};

// ============================================================================
// Helpers
// ============================================================================

fn promoted() -> Admitter {
    Admitter {
        id: "human:test".into(),
        role: AdmitterRole::HumanPromoter,
    }
}

fn admitted_layer_constraint() -> cognicode_core::domain::architecture::ArchitectureConstraint {
    let mut svc = ArchitectureAdmissionService::new();
    let candidate = cognicode_core::domain::architecture::ConstraintCandidate {
        id: ArchitectureConstraintId::new("architecture.test.layer").unwrap(),
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
// Claim 1 — the evaluator must NOT emit Findings as primary output
// ============================================================================

/// Today, the evaluator returns `EvaluationReport { findings: Vec<Finding>, .. }`.
/// The post-e77.1 evaluator must return a report whose primary output is
/// `Vec<ArchitectureViolation>` — a distinct DTO that carries no
/// `DetectorAuthority`, no `EvidenceId`, no `DetectorDigests`.
///
/// This test asserts the property at runtime by reading the report's
/// public surface: today there are findings, so the count is 1 and the
/// assertion fails. After WU1, the `findings` field is gone and the
/// report carries violations instead.
#[test]
fn claim_1_evaluator_primary_output_is_not_finding() {
    let constraint = admitted_layer_constraint();
    let report = ArchitectureEvaluator::new()
        .evaluate(&constraint, &synthetic_source())
        .expect("evaluator should succeed on syntactically valid source");

    // The post-e77.1 contract is that an `EvaluationReport` does not
    // carry a `findings: Vec<Finding>` field. The way we check this
    // at runtime today is via Debug-formatting the report: the
    // `Debug` impl of `EvaluationReport` today includes `findings`
    // in the output; after WU1 it must not.
    let dbg = format!("{report:?}");

    // Today the report debug-prints its findings. This is the
    // observable surface that the gap exists.
    assert!(
        !dbg.contains("findings:"),
        "post-e77.1: EvaluationReport must not expose a `findings` field; \
         today it does. Debug: {dbg}"
    );
    // And the report must expose violations, which today it does not.
    assert!(
        dbg.contains("violations:"),
        "post-e77.1: EvaluationReport must expose `violations`; today it does not. \
         Debug: {dbg}"
    );
}

// ============================================================================
// Claim 2 — constraint admission alone must not mint gate authority
// ============================================================================

/// An admitted constraint is a *rule*, not a *finding*. The
/// post-e77.1 evaluator must not be able to emit anything carrying
/// `DetectorAuthority::Gated` purely on the strength of constraint
/// admission. Today it can — the Finding emitted by the evaluator
/// has `DetectorAuthority::Gated`.
///
/// We assert the property by checking the report's public surface
/// for any reference to `Gated`. Today the `Debug` output of a
/// Finding includes "authority: Gated"; after WU1 the report does
/// not include findings, so "Gated" is unreachable from the
/// evaluator's output alone.
#[test]
fn claim_2_violation_carries_no_gate_authority() {
    let constraint = admitted_layer_constraint();
    let report = ArchitectureEvaluator::new()
        .evaluate(&constraint, &synthetic_source())
        .unwrap();
    let dbg = format!("{report:?}");

    assert!(
        !dbg.contains("Gated"),
        "post-e77.1: the evaluator's report must not mention `Gated`; \
         today it does (findings carry DetectorAuthority::Gated). Debug: {dbg}"
    );
}

// ============================================================================
// Claim 3 — placeholder DetectorDigests must not be reachable
// ============================================================================

/// The e77 first slice added `DetectorDigests::of_for_kind(&kind)` as a
/// global placeholder factory. The post-e77.1 API must NOT expose
/// it. Today the method exists on `DetectorDigests`. After WU1 it
/// should not.
///
/// The Rust compiler enforces this implicitly: re-introducing the
/// factory would require a deliberate code change. We document the
/// property here so it is part of the audit trail.
#[test]
fn claim_3_placeholder_factory_will_be_removed() {
    // The placeholder factory was reachable as
    // `DetectorDigests::of_for_kind(&kind)`. Removing it is a WU1
    // code change, not a test-time check.
    //
    // This test is a marker: it documents the invariant and the
    // fact that its absence is verified by the WU3 regression
    // sweep (any re-introduction must compile against the type
    // system).
    //
    // Today this is a trivial pass. After WU1, a *new* test
    // (authored in WU3) will assert the absence at the API level.
    let _: () = ();
}

// ============================================================================
// Claim 4 — the canonical path must be the only path to a Finding
// ============================================================================

/// The post-e77.1 contract is that a `Finding` only emerges after
/// the canonical path:
///
/// ```text
/// ArchitectureViolation
///     │ + GroundingRef (when the canonical fact is known)
///     ▼
/// ProducedEvidence { grounding: Some(fact) }
///     │
///     ▼  CanonicalEvidenceWriter::persist
/// EvidenceBinding::grounded(real_evidence_id, real_fact)
///     │
///     ▼  FindingAssembler (existing seam)
/// Finding
///     │
///     ▼  FindingVerifier::verify_for_gate
/// gate-eligible only if all of the above are real
/// ```
///
/// Today the e77 evaluator short-circuits this path by minting the
/// finding directly. The evaluator's output therefore does not
/// carry a `GroundingRef` field at all (no finding has grounding in
/// the canonical sense). After WU1+W2, violations carry an
/// optional `GroundingRef`.
#[test]
fn claim_4_evaluator_output_carries_grounding_optionality() {
    let constraint = admitted_layer_constraint();
    let report = ArchitectureEvaluator::new()
        .evaluate(&constraint, &synthetic_source())
        .unwrap();
    let dbg = format!("{report:?}");

    // Today the report does not mention `grounding`. After WU1 it
    // does (each violation carries an optional `grounding` field).
    assert!(
        dbg.contains("grounding"),
        "post-e77.1: violations must carry an optional `grounding`; today they don't. \
         Debug: {dbg}"
    );
}
