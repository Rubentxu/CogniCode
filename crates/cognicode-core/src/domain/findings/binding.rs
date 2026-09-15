//! Evidence bindings — what each produced evidence item became once persisted
//! (M6, cycle e62.4, umbrella U42).
//!
//! A backend produces evidence in an order, and its matches refer to that
//! evidence **by index** ([`ProducedEvidence`](super::ProducedEvidence)). Once
//! the run's evidence is persisted, every item either
//!
//! ```text
//! Grounded    → got a kernel EvidenceId, and names the canonical FactId it grades
//! Ungrounded  → got no canonical fact, with a reason why
//! ```
//!
//! The binding list is **index-aligned** with the produced evidence, and that
//! is deliberate: compacting away the ungrounded entries would renumber the
//! items the backend already referenced, silently repointing a match at another
//! piece of evidence.
//!
//! An ungrounded item is not an error. A route that could not be grounded is
//! still worth explaining — it just cannot open the gate:
//!
//! ```text
//! correct incomplete > fabricated complete
//! ```
//!
//! Pure domain: no I/O.

use serde::{Deserialize, Serialize};

use crate::domain::kernel_ids::{EntityId, EvidenceId, FactId};

/// Why a produced evidence item could not be bound to a canonical fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GroundingFailure {
    /// The backend projected the item without naming a canonical fact.
    NoFact,
    /// The assigned evidence id does not resolve in the scope.
    MissingEvidence {
        /// The id that failed to resolve.
        evidence: EvidenceId,
    },
    /// The fact the item claims does not exist in the scope.
    MissingFact {
        /// The fact that failed to resolve.
        fact: FactId,
    },
    /// The item's entity hint contradicts the canonical fact's subject.
    EntityFactMismatch {
        /// Entity the projection named.
        entity: EntityId,
        /// Subject the canonical fact actually carries.
        subject: Option<EntityId>,
    },
    /// Two or more parallel relations witness the same hop with *different*
    /// facts, so no single fact can be said to ground it. Fail closed rather
    /// than pick one arbitrarily.
    AmbiguousRelation {
        /// Hop source node.
        from: u64,
        /// Hop target node.
        to: u64,
    },
}

impl GroundingFailure {
    /// Stable code for diagnostics.
    pub fn code(self) -> &'static str {
        match self {
            Self::NoFact => "no_fact",
            Self::MissingEvidence { .. } => "missing_evidence",
            Self::MissingFact { .. } => "missing_fact",
            Self::EntityFactMismatch { .. } => "entity_fact_mismatch",
            Self::AmbiguousRelation { .. } => "ambiguous_relation",
        }
    }
}

impl std::fmt::Display for GroundingFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoFact => f.write_str("no canonical fact was recorded for this evidence"),
            Self::MissingEvidence { evidence } => {
                write!(f, "evidence {evidence} does not resolve in this scope")
            }
            Self::MissingFact { fact } => {
                write!(f, "fact {fact} does not resolve in this scope")
            }
            Self::EntityFactMismatch { entity, subject } => write!(
                f,
                "entity hint {entity} contradicts the canonical fact's subject {}",
                subject
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "<none>".to_string())
            ),
            Self::AmbiguousRelation { from, to } => write!(
                f,
                "the relation {from} -> {to} is witnessed by more than one fact"
            ),
        }
    }
}

/// What one produced evidence item became.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum EvidenceBinding {
    /// The item was persisted and bound to a canonical fact.
    Grounded {
        /// The kernel id the store assigned.
        id: EvidenceId,
        /// The canonical fact the item grades.
        fact: FactId,
    },
    /// The item exists but carries no canonical truth.
    Ungrounded {
        /// Why it could not be grounded.
        reason: GroundingFailure,
    },
}

impl EvidenceBinding {
    /// Construct a grounded binding.
    pub fn grounded(id: EvidenceId, fact: FactId) -> Self {
        Self::Grounded { id, fact }
    }

    /// Construct an ungrounded binding.
    pub fn ungrounded(reason: GroundingFailure) -> Self {
        Self::Ungrounded { reason }
    }

    /// The evidence id, whether or not the item is grounded.
    pub fn id(&self) -> Option<EvidenceId> {
        match self {
            Self::Grounded { id, .. } => Some(*id),
            Self::Ungrounded { .. } => None,
        }
    }

    /// The canonical fact, if grounded.
    pub fn fact(&self) -> Option<FactId> {
        match self {
            Self::Grounded { fact, .. } => Some(*fact),
            Self::Ungrounded { .. } => None,
        }
    }

    /// Whether the item carries canonical truth.
    pub fn is_grounded(&self) -> bool {
        matches!(self, Self::Grounded { .. })
    }

    /// Why the item is ungrounded, if it is.
    pub fn reason(&self) -> Option<GroundingFailure> {
        match self {
            Self::Ungrounded { reason } => Some(*reason),
            Self::Grounded { .. } => None,
        }
    }
}

/// Index-aligned bindings for a run's produced evidence.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceBindings {
    entries: Vec<EvidenceBinding>,
}

impl EvidenceBindings {
    /// Wrap bindings that are index-aligned with the produced evidence.
    pub fn new(entries: Vec<EvidenceBinding>) -> Self {
        Self { entries }
    }

    /// Number of bindings (== number of produced evidence items).
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether there is no binding at all.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The binding at `index`, if in range.
    pub fn get(&self, index: usize) -> Option<&EvidenceBinding> {
        self.entries.get(index)
    }

    /// All bindings, in produced order.
    pub fn entries(&self) -> &[EvidenceBinding] {
        &self.entries
    }

    /// Whether the item at `index` is grounded.
    pub fn is_grounded(&self, index: usize) -> bool {
        self.entries.get(index).is_some_and(|b| b.is_grounded())
    }

    /// The evidence id at `index`, if the item was persisted.
    pub fn id(&self, index: usize) -> Option<EvidenceId> {
        self.entries.get(index).and_then(|b| b.id())
    }

    /// The canonical fact at `index`, if grounded.
    pub fn fact(&self, index: usize) -> Option<FactId> {
        self.entries.get(index).and_then(|b| b.fact())
    }

    /// The grounded evidence ids, in produced order, de-duplicated.
    ///
    /// This is what a finding claims as its evidence: only the items that carry
    /// canonical truth.
    pub fn grounded_ids(&self) -> Vec<EvidenceId> {
        let mut ids: Vec<EvidenceId> = Vec::new();
        for entry in &self.entries {
            if let Some(id) = entry.id() {
                if !ids.contains(&id) {
                    ids.push(id);
                }
            }
        }
        ids
    }

    /// The indices of every ungrounded item, with its reason.
    pub fn ungrounded(&self) -> Vec<(usize, GroundingFailure)> {
        self.entries
            .iter()
            .enumerate()
            .filter_map(|(i, b)| b.reason().map(|r| (i, r)))
            .collect()
    }

    /// Whether every item is grounded.
    pub fn all_grounded(&self) -> bool {
        self.entries.iter().all(|b| b.is_grounded())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bindings_keep_the_produced_index() {
        let bindings = EvidenceBindings::new(vec![
            EvidenceBinding::grounded(EvidenceId::new(1), FactId::new(7)),
            EvidenceBinding::ungrounded(GroundingFailure::NoFact),
            EvidenceBinding::grounded(EvidenceId::new(2), FactId::new(8)),
        ]);
        assert_eq!(bindings.len(), 3);
        assert_eq!(bindings.fact(0), Some(FactId::new(7)));
        assert_eq!(bindings.fact(1), None);
        assert_eq!(bindings.id(1), None, "an ungrounded item has no id either");
        assert_eq!(bindings.fact(2), Some(FactId::new(8)));
        assert_eq!(
            bindings.grounded_ids(),
            vec![EvidenceId::new(1), EvidenceId::new(2)],
            "only grounded items back a finding, but the indices are preserved"
        );
        assert!(!bindings.all_grounded());
    }

    #[test]
    fn out_of_range_bindings_are_not_grounded() {
        let bindings =
            EvidenceBindings::new(vec![EvidenceBinding::ungrounded(GroundingFailure::NoFact)]);
        assert!(!bindings.is_grounded(9));
        assert_eq!(bindings.fact(9), None);
        assert_eq!(bindings.get(9), None);
    }

    #[test]
    fn grounded_ids_are_de_duplicated() {
        let bindings = EvidenceBindings::new(vec![
            EvidenceBinding::grounded(EvidenceId::new(1), FactId::new(7)),
            EvidenceBinding::grounded(EvidenceId::new(1), FactId::new(7)),
        ]);
        assert_eq!(bindings.grounded_ids(), vec![EvidenceId::new(1)]);
    }

    #[test]
    fn failures_are_named_and_described() {
        assert_eq!(GroundingFailure::NoFact.code(), "no_fact");
        assert_eq!(
            GroundingFailure::AmbiguousRelation { from: 1, to: 4 }.code(),
            "ambiguous_relation"
        );
        assert!(
            GroundingFailure::EntityFactMismatch {
                entity: EntityId::new(3),
                subject: Some(EntityId::new(9)),
            }
            .to_string()
            .contains("contradicts")
        );
    }

    #[test]
    fn bindings_round_trip() {
        let bindings = EvidenceBindings::new(vec![
            EvidenceBinding::grounded(EvidenceId::new(1), FactId::new(7)),
            EvidenceBinding::ungrounded(GroundingFailure::MissingFact {
                fact: FactId::new(9),
            }),
        ]);
        let json = serde_json::to_string(&bindings).unwrap();
        assert_eq!(
            serde_json::from_str::<EvidenceBindings>(&json).unwrap(),
            bindings
        );
    }
}
