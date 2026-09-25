//! Canonical CogniCode architecture constraints.
//!
//! The three rules below are the **canonical CogniCode architecture
//! rules** declared in
//! `docs/analysis/e77-architecture-ownership-map.md` and now wired into
//! both the self-hosting E2E test and (from E2.W1) the production
//! `ControlQueryService`.
//!
//! ## Why a single source of truth
//!
//! Before E2.W1 these constraints were inlined in
//! `crates/cognicode-core/tests/architecture_self_host_e2e.rs::canonical_constraints`.
//! That kept the test self-contained but meant the production wiring
//! had no canonical data to bootstrap from — the `ControlQueryService`
//! was always empty in production, returning `status: "incomplete"`
//! from `query_architecture`.
//!
//! Centralising the constraint list here lets:
//!
//! * the self-host test re-use the same data without drift,
//! * the production wiring helper build a `ControlQueryService` with
//!   real constraints on startup (CP1 first consumer),
//! * future ADRs amend the rule set in one place — the architecture
//!   working group changes the data, not the wiring code.
//!
//! ## Rule families
//!
//! Two `LayerDependency` rules (domain must not reach infrastructure,
//! domain must not reach application) plus one `NamespaceBoundary`
//! rule (`domain::evidence_kernel` must not drive UI / apps). All
//! three are admitted by a single `HumanPromoter`-class admitter;
//! admission by a non-promoted admitter is rejected upstream, which
//! is the load-bearing property of the admission flow (see
//! [`ArchitectureAdmissionService::admit`]).
//!
//! ## Stability
//!
//! Each constraint id is a string literal in the form
//! `architecture.<snake_case>`. Adding a rule is a non-breaking change
//! (the registry accepts more); removing or renaming a rule is a
//! **breaking change** for any consumer that pins the rule id (e.g.
//! CI gate rules). Renames must go through ADR-011.

use crate::domain::architecture::{
    Admitter, AdmitterRole, ArchitectureConstraintId, ArchitectureConstraintKind, ConstraintCandidate,
    LayerDependencyRule, LayerId, NamespaceBoundaryRule,
};

/// Build the canonical CogniCode architecture constraints.
///
/// Order is preserved (admission is order-stable; the
/// `ArchitectureReadModel.constraints` array is built from the
/// admitted set in insertion order).
pub fn canonical_constraints() -> Vec<ConstraintCandidate> {
    vec![
        ConstraintCandidate {
            id: ArchitectureConstraintId::new("architecture.domain_no_infrastructure")
                .expect("static id is well-formed"),
            kind: ArchitectureConstraintKind::LayerDependency(LayerDependencyRule {
                from_layer: LayerId::Domain,
                forbidden_targets: vec![LayerId::Infrastructure],
                rationale: "domain has no I/O and must not import infrastructure".into(),
            }),
            adr_ref: Some("ADR-046".into()),
            proposed_by: "human:cognicode-architecture-wg".into(),
        },
        ConstraintCandidate {
            id: ArchitectureConstraintId::new("architecture.domain_no_application")
                .expect("static id is well-formed"),
            kind: ArchitectureConstraintKind::LayerDependency(LayerDependencyRule {
                from_layer: LayerId::Domain,
                forbidden_targets: vec![LayerId::Application],
                rationale: "domain must not depend on orchestration".into(),
            }),
            adr_ref: Some("ADR-046".into()),
            proposed_by: "human:cognicode-architecture-wg".into(),
        },
        ConstraintCandidate {
            id: ArchitectureConstraintId::new("architecture.evidence_kernel_no_presentation")
                .expect("static id is well-formed"),
            kind: ArchitectureConstraintKind::NamespaceBoundary(NamespaceBoundaryRule {
                caller_namespace: "domain::evidence_kernel".into(),
                forbidden_targets: vec!["presentation".into(), "apps".into()],
                rationale: "evidence_kernel must not drive UI".into(),
            }),
            adr_ref: Some("ADR-046".into()),
            proposed_by: "human:cognicode-architecture-wg".into(),
        },
    ]
}

/// The single promoted admitter under which the canonical constraints
/// are admitted.
///
/// Identity is stable: changing this string is a breaking change for
/// audit trails that pin the admitter. Keep it pinned to the
/// architecture working group.
pub fn canonical_promoted_admitter() -> Admitter {
    Admitter {
        id: "human:cognicode-architecture-wg".into(),
        role: AdmitterRole::HumanPromoter,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The canonical list is order-stable and well-formed; any future
    /// change must be intentional (this test fails loudly).
    #[test]
    fn canonical_constraints_have_expected_ids_and_kinds() {
        let cs = canonical_constraints();
        assert_eq!(cs.len(), 3, "expected exactly 3 canonical constraints");

        let ids: Vec<&str> = cs.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(
            ids,
            vec![
                "architecture.domain_no_infrastructure",
                "architecture.domain_no_application",
                "architecture.evidence_kernel_no_presentation",
            ],
            "constraint ids and order must not drift"
        );

        let promoted = canonical_promoted_admitter();
        assert!(
            promoted.may_admit(),
            "canonical admitter must be promoted"
        );
        assert_eq!(promoted.id, "human:cognicode-architecture-wg");
    }
}
