//! Execution ports for findings (M6, cycle e57).
//!
//! Narrow, sync ports the executor uses to persist and later resolve
//! evidence. They are deliberately **separate** from the gated async kernel
//! `EvidenceStore`: the kernel port's contract is flagged for renegotiation
//! (RETIREMENT-LEDGER C1), and the findings surface is ungated. The
//! production adapter binds this sink to the kernel store in the wiring
//! cycle; the in-memory adapter lives in `infrastructure::findings`.
//!
//! Pure domain: no I/O.

use super::binding::EvidenceBindings;
use super::outcome::ProducedEvidence;
use crate::domain::kernel_ids::{EntityId, EvidenceGrade, EvidenceId, FactId, SnapshotId};

/// Why recording evidence failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceError {
    /// Human-readable reason.
    pub reason: String,
}

impl EvidenceError {
    /// Construct an error with a reason.
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }
}

impl std::fmt::Display for EvidenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "evidence sink error: {}", self.reason)
    }
}

impl std::error::Error for EvidenceError {}

/// Persists a run's produced evidence and reports how each item was bound.
///
/// The whole run is persisted in **one call**, so an adapter can commit it
/// atomically: writing items one at a time would leave orphaned evidence behind
/// if a later item failed, evidence no finding would ever cite.
///
/// The returned [`EvidenceBindings`] are index-aligned with `produced`, and an
/// item the sink cannot ground is reported as ungrounded rather than dropped.
pub trait EvidenceSink {
    /// Persist the run's evidence, in order.
    fn persist(&mut self, produced: &[ProducedEvidence])
    -> Result<EvidenceBindings, EvidenceError>;
}

/// A canonical fact, as the findings domain sees it.
///
/// Deliberately small: only what verification reasons about. The kernel's
/// `Fact` carries predicate/object/provenance, which live in the kernel read
/// model, not in this port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FactDescriptor {
    /// The canonical fact id.
    pub id: FactId,
    /// The entity the fact is about, when the lookup can attest to it.
    ///
    /// `None` means "this lookup cannot vouch for the subject" (a plain
    /// existence store). The verification rule is fail-closed: a causal step
    /// that names a subject requires the fact to agree.
    pub subject: Option<EntityId>,
    /// The snapshot the fact belongs to.
    pub snapshot: SnapshotId,
}

/// Where the canonical fact behind an evidence atom is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FactSlot {
    /// The fact exists and its identity is known.
    Resolved(FactDescriptor),
    /// The evidence points at a fact that does not exist in this scope.
    Dangling {
        /// The fact it points at.
        id: FactId,
    },
}

/// A canonical evidence atom, as the findings domain sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceDescriptor {
    /// The evidence id.
    pub id: EvidenceId,
    /// How the evidence relates to its fact.
    pub grade: EvidenceGrade,
    /// The fact it grades.
    pub fact: FactSlot,
}

/// What a lookup knows about an evidence id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvidenceResolution<'a> {
    /// The id resolves and its canonical truth was loaded.
    Known(&'a EvidenceDescriptor),
    /// The id does not resolve in this scope.
    Unknown,
}

/// Resolves evidence ids to their canonical truth, **in the scope it was
/// hydrated for**.
///
/// Used by [`FindingVerifier`](super::FindingVerifier) to check causal
/// coherence (U42) before a finding may block. `EvidenceId` is canonical per
/// snapshot, so a lookup must declare the scope it was loaded from: otherwise
/// a caller could hydrate snapshot B's evidence for a finding produced in A and
/// every id would still resolve.
///
/// A lookup that only knows ids exist (not what they mean) is a legitimate but
/// weaker implementation: it must report the truth it can and no more. The
/// strict verifier is what turns "the fact is not attested" into a refusal.
pub trait EvidenceLookup {
    /// Resolve `id` to its canonical truth, if known.
    fn resolve(&self, id: EvidenceId) -> EvidenceResolution<'_>;

    /// Whether `id` is known to the store at all.
    fn contains(&self, id: EvidenceId) -> bool {
        matches!(self.resolve(id), EvidenceResolution::Known(_))
    }

    /// The `(workspace, snapshot)` this lookup was hydrated for.
    ///
    /// `None` means "unscoped": verification then requires the finding to be
    /// unscoped too (the legacy QualityIssue projection).
    fn scope(&self) -> Option<&super::scope::AnalysisScope>;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tiny in-test sink/lookup to exercise the traits.
    struct VecStore {
        items: Vec<ProducedEvidence>,
        descriptors: Vec<EvidenceDescriptor>,
    }

    impl EvidenceSink for VecStore {
        fn persist(
            &mut self,
            produced: &[ProducedEvidence],
        ) -> Result<EvidenceBindings, EvidenceError> {
            let mut bindings = Vec::with_capacity(produced.len());
            for item in produced {
                self.items.push(item.clone());
                let id = EvidenceId::new(self.items.len() as u64);
                bindings.push(match item.grounding {
                    Some(grounding) => super::super::EvidenceBinding::grounded(id, grounding.fact),
                    None => super::super::EvidenceBinding::ungrounded(
                        super::super::GroundingFailure::NoFact,
                    ),
                });
            }
            for item in produced {
                if let Some(grounding) = item.grounding {
                    let id = EvidenceId::new(self.descriptors.len() as u64 + 1);
                    self.descriptors.push(EvidenceDescriptor {
                        id,
                        grade: EvidenceGrade::Supports,
                        fact: FactSlot::Resolved(FactDescriptor {
                            id: grounding.fact,
                            subject: grounding.entity,
                            snapshot: SnapshotId::new(1),
                        }),
                    });
                }
            }
            Ok(EvidenceBindings::new(bindings))
        }
    }

    impl EvidenceLookup for VecStore {
        fn resolve(&self, id: EvidenceId) -> EvidenceResolution<'_> {
            let raw = id.get();
            if raw == 0 || (raw as usize) > self.items.len() {
                return EvidenceResolution::Unknown;
            }
            // A pure existence store: it can attest the id and the grade it
            // recorded, but not the subject of the fact.
            self.descriptors
                .get(raw as usize - 1)
                .map(EvidenceResolution::Known)
                .unwrap_or(EvidenceResolution::Unknown)
        }

        fn scope(&self) -> Option<&crate::domain::findings::AnalysisScope> {
            None
        }
    }

    #[test]
    fn sink_assigns_ids_and_lookup_resolves_them() {
        let mut store = VecStore {
            items: Vec::new(),
            descriptors: Vec::new(),
        };
        let bindings = store
            .persist(&[
                ProducedEvidence {
                    kind: super::super::EvidenceKind::AstMatch,
                    detail: "grounded".to_string(),
                    subject: None,
                    grounding: Some(super::super::GroundingRef::fact(
                        crate::domain::kernel_ids::FactId::new(7),
                    )),
                },
                ProducedEvidence {
                    kind: super::super::EvidenceKind::AstMatch,
                    detail: "ungrounded".to_string(),
                    subject: None,
                    grounding: None,
                },
            ])
            .unwrap();
        assert_eq!(bindings.id(0), Some(EvidenceId::new(1)));
        assert_eq!(
            bindings.fact(0),
            Some(crate::domain::kernel_ids::FactId::new(7))
        );
        assert_eq!(
            bindings.id(1),
            None,
            "an ungrounded item carries no claim, not even an id: the finding must not cite it"
        );
        assert_eq!(bindings.fact(1), None, "no fact, no grounding");
        assert_eq!(
            bindings.ungrounded(),
            vec![(1, super::super::GroundingFailure::NoFact)]
        );
        assert!(store.contains(EvidenceId::new(1)));
        assert!(!store.contains(EvidenceId::new(3)));
    }

    #[test]
    fn evidence_error_displays_reason() {
        assert_eq!(
            EvidenceError::new("boom").to_string(),
            "evidence sink error: boom"
        );
    }
}
