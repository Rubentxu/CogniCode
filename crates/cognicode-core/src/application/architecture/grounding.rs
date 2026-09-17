//! Canonical grounding bridge (e77.1 WU2).
//!
//! This module is the **seam** between the architecture evaluator
//! (which emits [`ArchitectureViolation`]s) and the canonical
//! evidence pipeline (which produces real `EvidenceId`s and
//! [`crate::domain::findings::binding::EvidenceBinding`]s).
//!
//! The bridge does **not** touch the kernel stores. It is a
//! pure-domain transformation: a violation is converted into a
//! [`ProducedEvidence`](crate::domain::findings::outcome::ProducedEvidence)
//! whose `grounding` is the violation's optional
//! [`GroundingRef`]. The actual write step — `FactStore::get`,
//! `EvidenceStore::append_batch` — is delegated to the existing
//! [`crate::application::findings::kernel_bridge::CanonicalEvidenceWriter`],
//! which is the only writer that touches the kernel.
//!
//! ## Invariants
//!
//! 1. **A violation without grounding produces an ungrounded
//!    `ProducedEvidence`.** The downstream writer then persists it
//!    as `EvidenceBinding::ungrounded(NoFact)`. The
//!    [`crate::domain::findings::verifier::FindingVerifier`] rejects
//!    any finding whose evidence is ungrounded. Therefore, an
//!    architecture observation that cannot be grounded in the
//!    canonical kernel **cannot gate**.
//!
//! 2. **A violation with grounding produces a grounded
//!    `ProducedEvidence`.** The downstream writer then validates
//!    the fact, persists a real `EvidenceId`, and returns a
//!    grounded `EvidenceBinding`. From that point on, the
//!    observation follows the same path as any other detector
//!    finding (assembly, verifier, gate).
//!
//! 3. **The bridge never invents grounding.** If the caller wants
//!    a violation to gate, the caller must supply a real
//!    `GroundingRef` derived from canonical knowledge. There is no
//!    shortcut that "guesses" a fact id. This is the structural
//!    reason the bridge does not produce canonical evidence by
//!    itself.

use crate::domain::architecture::ArchitectureViolation;
use crate::domain::findings::outcome::{EvidenceKind, ProducedEvidence};

/// The grounding bridge. Stateless; construct once, call many
/// times.
#[derive(Debug, Default, Clone)]
pub struct ArchitectureGroundingBridge;

impl ArchitectureGroundingBridge {
    pub fn new() -> Self {
        Self
    }

    /// Convert a slice of violations into a vector of
    /// `ProducedEvidence` items, one per violation, in order.
    ///
    /// The resulting `ProducedEvidence` items are ready to be fed
    /// to [`crate::application::findings::kernel_bridge::CanonicalEvidenceWriter::persist`].
    /// Items with `grounding = None` will be persisted as
    /// `EvidenceBinding::ungrounded(NoFact)` and cannot gate.
    pub fn to_produced_evidence(
        &self,
        violations: &[ArchitectureViolation],
    ) -> Vec<ProducedEvidence> {
        violations
            .iter()
            .map(|v| ProducedEvidence {
                kind: EvidenceKind::ArchitectureSource,
                detail: format!(
                    "{} @ {}:{} -> {}",
                    v.finding_kind.as_str(),
                    v.file_path,
                    v.line,
                    v.dependency_path,
                ),
                subject: None,
                grounding: v.grounding,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::architecture::{
        Admitter, AdmitterRole, ArchitectureConstraintId, ArchitectureConstraintKind,
        LayerDependencyRule, LayerId, ViolationId,
    };
    use crate::domain::findings::GroundingRef;
    use crate::domain::findings::FindingKind;
    use crate::domain::kernel_ids::FactId;

    fn violation(grounding: Option<GroundingRef>) -> ArchitectureViolation {
        ArchitectureViolation {
            id: ViolationId::compute(
                &ArchitectureConstraintId::new("architecture.test.rule").unwrap(),
                "src/domain/foo.rs",
                1,
                "infrastructure::db",
            ),
            constraint_id: ArchitectureConstraintId::new("architecture.test.rule").unwrap(),
            finding_kind: FindingKind::new("architecture.layer_dependency").unwrap(),
            file_path: "src/domain/foo.rs".into(),
            module_path: Some("domain::foo".into()),
            line: 1,
            dependency_path: "infrastructure::db".into(),
            from_layer: LayerId::Domain,
            grounding,
            rationale: "test".into(),
        }
    }

    #[test]
    fn ungrounded_violation_becomes_ungrounded_produced_evidence() {
        let v = violation(None);
        let items = ArchitectureGroundingBridge::new()
            .to_produced_evidence(&[v]);
        assert_eq!(items.len(), 1);
        assert!(items[0].grounding.is_none());
        assert_eq!(items[0].kind, EvidenceKind::ArchitectureSource);
    }

    #[test]
    fn grounded_violation_becomes_grounded_produced_evidence() {
        let g = GroundingRef::fact(FactId::new(42));
        let v = violation(Some(g));
        let items = ArchitectureGroundingBridge::new()
            .to_produced_evidence(&[v]);
        assert_eq!(items.len(), 1);
        assert!(items[0].grounding.is_some());
        assert_eq!(items[0].grounding.unwrap().fact, FactId::new(42));
    }

    #[test]
    fn empty_violations_yield_empty_produced_evidence() {
        let items = ArchitectureGroundingBridge::new()
            .to_produced_evidence(&[]);
        assert!(items.is_empty());
    }
}
