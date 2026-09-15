//! In-memory evidence store (cycle e57).
//!
//! Implements both the [`EvidenceSink`] and [`EvidenceLookup`] findings ports
//! with a simple append-only vector. Ids are assigned 1-based, matching the
//! house `u64` newtype convention. The production adapter binds the sink to
//! the kernel `EvidenceStore` in a later wiring cycle.

use crate::domain::findings::AnalysisScope;
use crate::domain::findings::binding::{EvidenceBinding, EvidenceBindings, GroundingFailure};
use crate::domain::findings::outcome::ProducedEvidence;
use crate::domain::findings::ports::{
    EvidenceDescriptor, EvidenceError, EvidenceLookup, EvidenceResolution, EvidenceSink,
    FactDescriptor, FactSlot,
};
use crate::domain::kernel_ids::{EvidenceGrade, EvidenceId, FactId, SnapshotId};

/// Append-only in-memory evidence store.
#[derive(Debug, Default, Clone)]
pub struct InMemoryEvidenceStore {
    items: Vec<ProducedEvidence>,
    scope: Option<AnalysisScope>,
    /// Scratch space so `resolve` can hand out a borrowed descriptor.
    descriptors: Vec<EvidenceDescriptor>,
}

impl InMemoryEvidenceStore {
    /// Create an empty, unscoped store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create an empty store hydrated for a specific analysis scope.
    pub fn with_scope(scope: AnalysisScope) -> Self {
        Self {
            items: Vec::new(),
            scope: Some(scope),
            descriptors: Vec::new(),
        }
    }

    /// The scope this store was hydrated for.
    pub fn scope(&self) -> Option<&AnalysisScope> {
        self.scope.as_ref()
    }

    /// Number of recorded evidence items.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Look up evidence by id.
    pub fn get(&self, id: EvidenceId) -> Option<&ProducedEvidence> {
        let raw = id.get();
        if raw == 0 {
            return None;
        }
        self.items.get((raw - 1) as usize)
    }

    /// All recorded evidence, in id order.
    pub fn all(&self) -> &[ProducedEvidence] {
        &self.items
    }
}

impl EvidenceSink for InMemoryEvidenceStore {
    fn persist(
        &mut self,
        produced: &[ProducedEvidence],
    ) -> Result<EvidenceBindings, EvidenceError> {
        // Ids are allocated first, then every item is committed: an adapter
        // failure mid-way must not leave evidence behind for a run that never
        // reached the assembler.
        let start = self.items.len() as u64;
        let snapshot = self
            .scope
            .as_ref()
            .map(|s| s.snapshot)
            .unwrap_or(SnapshotId::NONE);
        let mut entries = Vec::with_capacity(produced.len());
        let mut descriptors = Vec::with_capacity(produced.len());
        for (offset, item) in produced.iter().enumerate() {
            let id = EvidenceId::new(start + offset as u64 + 1);
            // This store holds *projections*, not canonical truth: it can
            // attest the id it assigned and the entity hint the projection
            // carried, and nothing more. It never invents a subject, and it
            // speaks only for the snapshot it was hydrated for.
            descriptors.push(EvidenceDescriptor {
                id,
                grade: EvidenceGrade::Supports,
                fact: match item.grounding {
                    Some(g) => FactSlot::Resolved(FactDescriptor {
                        id: g.fact,
                        subject: g.entity,
                        snapshot,
                    }),
                    None => FactSlot::Dangling { id: FactId::new(0) },
                },
            });
            entries.push(match item.grounding {
                Some(grounding) => EvidenceBinding::grounded(id, grounding.fact),
                None => EvidenceBinding::ungrounded(GroundingFailure::NoFact),
            });
        }
        self.items.extend(produced.iter().cloned());
        self.descriptors.extend(descriptors);
        Ok(EvidenceBindings::new(entries))
    }
}

impl EvidenceLookup for InMemoryEvidenceStore {
    fn resolve(&self, id: EvidenceId) -> EvidenceResolution<'_> {
        let raw = id.get();
        if raw == 0 {
            return EvidenceResolution::Unknown;
        }
        match self.descriptors.get(raw as usize - 1) {
            Some(descriptor) => EvidenceResolution::Known(descriptor),
            None => EvidenceResolution::Unknown,
        }
    }

    fn scope(&self) -> Option<&AnalysisScope> {
        self.scope.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::findings::outcome::EvidenceKind;

    fn sample() -> ProducedEvidence {
        ProducedEvidence {
            kind: EvidenceKind::AstMatch,
            detail: "x".to_string(),
            subject: None,
            grounding: None,
        }
    }

    fn grounded() -> ProducedEvidence {
        ProducedEvidence {
            kind: EvidenceKind::AstMatch,
            detail: "grounded".to_string(),
            subject: None,
            grounding: Some(crate::domain::findings::GroundingRef::fact(
                crate::domain::kernel_ids::FactId::new(7),
            )),
        }
    }

    #[test]
    fn assigns_sequential_ids_and_resolves_them() {
        let mut store = InMemoryEvidenceStore::new();
        assert!(store.is_empty());
        let bindings = store.persist(&[sample(), grounded()]).unwrap();
        assert_eq!(
            bindings.id(0),
            None,
            "an item with no canonical fact is ungrounded, not fabricated"
        );
        assert_eq!(bindings.fact(0), None);
        assert_eq!(bindings.id(1), Some(EvidenceId::new(2)));
        assert_eq!(
            bindings.fact(1),
            Some(crate::domain::kernel_ids::FactId::new(7))
        );
        assert_eq!(
            bindings.grounded_ids(),
            vec![EvidenceId::new(2)],
            "only the grounded item is claimable"
        );
        assert_eq!(store.len(), 2);
        assert!(store.contains(EvidenceId::new(1)));
        assert!(!store.contains(EvidenceId::new(3)));
        assert_eq!(
            store.get(EvidenceId::new(1)).map(|e| e.kind),
            Some(EvidenceKind::AstMatch)
        );
        assert!(store.get(EvidenceId::new(0)).is_none());
    }

    /// A second run in the same store continues the id sequence and keeps
    /// bindings index-aligned with *its* produced evidence.
    #[test]
    fn a_second_run_continues_the_sequence() {
        let mut store = InMemoryEvidenceStore::new();
        store.persist(&[sample()]).unwrap();
        let second = store.persist(&[grounded(), sample()]).unwrap();
        assert_eq!(
            second.len(),
            2,
            "bindings stay aligned with this run's evidence"
        );
        assert_eq!(second.id(0), Some(EvidenceId::new(2)));
        assert_eq!(
            second.id(1),
            None,
            "the second item is ungrounded, so it is not claimable"
        );
        assert_eq!(store.len(), 3, "both runs are persisted");
    }
}
