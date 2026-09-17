//! UAT + adversarial matrix for the architecture evaluator (WU6).
//!
//! This test file is the **executable counterpart** to the
//! "adversarial matrix" declared in `openspec/changes/e77-lsi-executable-architecture/proposal.md`.
//! Each test is one row of that matrix. Running the file is the
//! proof that the property holds; reading the file is the proof that
//! we thought through the cases.
//!
//! The matrix is intentionally adversarial: the tests are designed
//! to defeat easy implementations, not to congratulate them.
//!
//! The load-bearing case is `adr_text_alone_produces_zero_findings`:
//! an ADR string in the candidate field is **not enough** to produce
//! findings. Only an admitted constraint can.

use cognicode_core::application::architecture::{
    ArchitectureRegistry, ArchitectureSource, SourceFile,
};
use cognicode_core::application::architecture::admission::{ArchitectureClock, SystemArchitectureClock};
use cognicode_core::domain::architecture::{
    Admitter, AdmitterRole, ArchitectureConstraintId, ArchitectureConstraintKind,
    ConstraintCandidate, ForbiddenDependencyRule, LayerDependencyRule, LayerId,
    NamespaceBoundaryRule,
};

// ============================================================================
// Helpers
// ============================================================================

struct FixedClock(&'static str);
impl ArchitectureClock for FixedClock {
    fn now(&self) -> String {
        self.0.into()
    }
}

fn promoted_human() -> Admitter {
    Admitter {
        id: "human:test".into(),
        role: AdmitterRole::HumanPromoter,
    }
}

fn ci_promoter() -> Admitter {
    Admitter {
        id: "ci:test".into(),
        role: AdmitterRole::CiPromoter,
    }
}

fn non_promoted() -> Admitter {
    Admitter {
        id: "ai:test".into(),
        role: AdmitterRole::Other,
    }
}

fn layer_candidate(id: &str) -> ConstraintCandidate {
    ConstraintCandidate {
        id: ArchitectureConstraintId::new(id).unwrap(),
        kind: ArchitectureConstraintKind::LayerDependency(LayerDependencyRule {
            from_layer: LayerId::Domain,
            forbidden_targets: vec![LayerId::Infrastructure],
            rationale: "test".into(),
        }),
        adr_ref: Some("ADR-046".into()),
        proposed_by: "human:test".into(),
    }
}

fn forbidden_candidate(id: &str, forbidden_path: &str) -> ConstraintCandidate {
    ConstraintCandidate {
        id: ArchitectureConstraintId::new(id).unwrap(),
        kind: ArchitectureConstraintKind::ForbiddenDependency(ForbiddenDependencyRule {
            from_layer: LayerId::Domain,
            forbidden_paths: vec![forbidden_path.into()],
            rationale: "test".into(),
        }),
        adr_ref: Some("ADR-046".into()),
        proposed_by: "human:test".into(),
    }
}

fn boundary_candidate(id: &str) -> ConstraintCandidate {
    ConstraintCandidate {
        id: ArchitectureConstraintId::new(id).unwrap(),
        kind: ArchitectureConstraintKind::NamespaceBoundary(NamespaceBoundaryRule {
            caller_namespace: "domain::evidence_kernel".into(),
            forbidden_targets: vec!["presentation".into()],
            rationale: "evidence_kernel must not drive UI".into(),
        }),
        adr_ref: Some("ADR-046".into()),
        proposed_by: "human:test".into(),
    }
}

// ============================================================================
// Adversarial matrix
// ============================================================================

/// **Row 1 — load-bearing.** An ADR string in `adr_ref` is **not**
/// enough to produce findings. Only an admitted constraint can.
///
/// This is the property that distinguishes e77 from "we wrote an ADR
/// and called it executable". If this test ever fails, the whole
/// premise of e77 collapses.
#[test]
fn adr_text_alone_produces_zero_findings() {
    let registry = ArchitectureRegistry::new();
    // We *propose* a candidate with a strong normative ADR ref but
    // never admit it.
    let candidate = layer_candidate("architecture.adr_text_alone");
    assert!(!registry.is_admitted(&candidate));

    // Even when the evaluator is fed a source with an obvious
    // violation, the registry rejects the candidate before the
    // evaluator is consulted.
    let source = ArchitectureSource {
        files: vec![SourceFile {
            file_path: "src/domain/foo.rs".into(),
            module_path: Some("domain::foo".into()),
            source: "use crate::infrastructure::db;\n".into(),
        }],
    };
    let report = registry.evaluate_candidate(&candidate, &source);
    assert!(report.is_err(), "registry must reject non-admitted candidates");
}

/// **Row 2.** Once admitted, the constraint produces findings.
#[test]
fn admitted_constraint_produces_findings() {
    let mut registry = ArchitectureRegistry::new();
    let out = registry
        .admission
        .admit(layer_candidate("architecture.admitted_emits"), &promoted_human(), &FixedClock("t0"));
    assert!(out.result.is_ok());
    let constraint = registry.admission.admitted().first().unwrap().clone();

    let source = ArchitectureSource {
        files: vec![SourceFile {
            file_path: "src/domain/foo.rs".into(),
            module_path: Some("domain::foo".into()),
            source: "use crate::infrastructure::db;\n".into(),
        }],
    };
    let report = registry.evaluate(&constraint, &source).unwrap();
    assert_eq!(report.findings.len(), 1);
}

/// **Row 3.** A `Candidate` admitter (`AdmitterRole::Other`) cannot
/// admit — even with the right id and the right admitter text.
#[test]
fn non_promoted_admitter_cannot_admit() {
    let mut registry = ArchitectureRegistry::new();
    let out = registry.admission.admit(
        layer_candidate("architecture.non_promoted"),
        &non_promoted(),
        &FixedClock("t0"),
    );
    assert!(out.result.is_err());
    assert!(registry.admission.admitted().is_empty());
}

/// **Row 4.** A `CiPromoter` can admit — the rule is "promoted",
/// not "human".
#[test]
fn ci_promoter_can_admit() {
    let mut registry = ArchitectureRegistry::new();
    let out = registry
        .admission
        .admit(layer_candidate("architecture.ci_admit"), &ci_promoter(), &FixedClock("t0"));
    assert!(out.result.is_ok());
}

/// **Row 5.** Re-admitting an already-admitted id is rejected
/// (idempotency).
#[test]
fn double_admission_is_rejected() {
    let mut registry = ArchitectureRegistry::new();
    let out = registry
        .admission
        .admit(layer_candidate("architecture.double"), &promoted_human(), &FixedClock("t0"));
    assert!(out.result.is_ok());
    let out2 = registry
        .admission
        .admit(layer_candidate("architecture.double"), &promoted_human(), &FixedClock("t1"));
    assert!(out2.result.is_err());
    assert_eq!(registry.admission.admitted().len(), 1);
}

/// **Row 6.** The three rule kinds behave independently: admitting
/// one does not silence the others.
#[test]
fn rule_kinds_are_independent() {
    let mut registry = ArchitectureRegistry::new();
    registry.admission.admit(
        layer_candidate("architecture.rule_independence_layer"),
        &promoted_human(),
        &FixedClock("t0"),
    );
    registry.admission.admit(
        forbidden_candidate("architecture.no_sqlx", "sqlx"),
        &promoted_human(),
        &FixedClock("t0"),
    );
    registry.admission.admit(
        boundary_candidate("architecture.ek_no_presentation"),
        &promoted_human(),
        &FixedClock("t0"),
    );
    assert_eq!(registry.admission.admitted().len(), 3);

    // A source that violates only the layer rule does not trigger
    // the forbidden/boundary rules.
    let source = ArchitectureSource {
        files: vec![SourceFile {
            file_path: "src/domain/foo.rs".into(),
            module_path: Some("domain::foo".into()),
            source: "use crate::infrastructure::db;\n".into(),
        }],
    };
    let mut total = 0;
    for c in registry.admission.admitted() {
        let report = registry.evaluate(c, &source).unwrap();
        total += report.findings.len();
    }
    // Only the layer rule fires on this source.
    assert_eq!(total, 1);
}

/// **Row 7.** Source with no `use` statements yields zero findings
/// (positive control).
#[test]
fn empty_source_yields_zero_findings() {
    let mut registry = ArchitectureRegistry::new();
    registry
        .admission
        .admit(layer_candidate("architecture.empty_source"), &promoted_human(), &FixedClock("t0"));
    let constraint = registry.admission.admitted().first().unwrap().clone();
    let report = registry
        .evaluate(&constraint, &ArchitectureSource::default())
        .unwrap();
    assert!(report.findings.is_empty());
}

/// **Row 8.** The system clock works (smoke test for the time
/// surface).
#[test]
fn system_clock_produces_non_empty_string() {
    let s = SystemArchitectureClock.now();
    assert!(!s.is_empty());
}

/// **Row 9.** The evaluator is stateless: two evaluations on the
/// same input yield identical reports (property: stable evidence
/// ids, same findings, same messages).
#[test]
fn evaluation_is_deterministic() {
    let mut registry = ArchitectureRegistry::new();
    registry
        .admission
        .admit(layer_candidate("architecture.deterministic"), &promoted_human(), &FixedClock("t0"));
    let constraint = registry.admission.admitted().first().unwrap().clone();
    let source = ArchitectureSource {
        files: vec![SourceFile {
            file_path: "src/domain/foo.rs".into(),
            module_path: Some("domain::foo".into()),
            source: "use crate::infrastructure::db;\n".into(),
        }],
    };
    let a = registry.evaluate(&constraint, &source).unwrap();
    let b = registry.evaluate(&constraint, &source).unwrap();
    assert_eq!(a.findings.len(), b.findings.len());
    assert_eq!(a.findings[0].id, b.findings[0].id);
    assert_eq!(a.findings[0].evidence, b.findings[0].evidence);
}

/// **Row 10.** An empty rule body is rejected (admission-level
/// validation).
#[test]
fn empty_rule_body_is_rejected() {
    let mut registry = ArchitectureRegistry::new();
    let candidate = ConstraintCandidate {
        id: ArchitectureConstraintId::new("architecture.empty_body").unwrap(),
        kind: ArchitectureConstraintKind::LayerDependency(LayerDependencyRule {
            from_layer: LayerId::Domain,
            forbidden_targets: vec![], // empty -> invalid
            rationale: "test".into(),
        }),
        adr_ref: Some("ADR-046".into()),
        proposed_by: "human:test".into(),
    };
    let out = registry
        .admission
        .admit(candidate, &promoted_human(), &FixedClock("t0"));
    assert!(out.result.is_err());
}
