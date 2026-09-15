//! In-memory evidence store (cycle e57).
//!
//! Implements both the [`EvidenceSink`] and [`EvidenceLookup`] findings ports
//! with a simple append-only vector. Ids are assigned 1-based, matching the
//! house `u64` newtype convention. The production adapter binds the sink to
//! the kernel `EvidenceStore` in a later wiring cycle.

use crate::domain::findings::AnalysisScope;
use crate::domain::findings::outcome::ProducedEvidence;
use crate::domain::findings::ports::{EvidenceError, EvidenceLookup, EvidenceSink};
use crate::domain::kernel_ids::EvidenceId;

/// Append-only in-memory evidence store.
#[derive(Debug, Default, Clone)]
pub struct InMemoryEvidenceStore {
    items: Vec<ProducedEvidence>,
    scope: Option<AnalysisScope>,
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
    fn record(&mut self, evidence: ProducedEvidence) -> Result<EvidenceId, EvidenceError> {
        self.items.push(evidence);
        Ok(EvidenceId::new(self.items.len() as u64))
    }
}

impl EvidenceLookup for InMemoryEvidenceStore {
    fn contains(&self, id: EvidenceId) -> bool {
        self.get(id).is_some()
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
            fact: None,
        }
    }

    #[test]
    fn assigns_sequential_ids_and_resolves_them() {
        let mut store = InMemoryEvidenceStore::new();
        assert!(store.is_empty());
        let a = store.record(sample()).unwrap();
        let b = store.record(sample()).unwrap();
        assert_eq!(a, EvidenceId::new(1));
        assert_eq!(b, EvidenceId::new(2));
        assert_eq!(store.len(), 2);
        assert!(store.contains(a));
        assert!(!store.contains(EvidenceId::new(3)));
        assert_eq!(store.get(a).map(|e| e.kind), Some(EvidenceKind::AstMatch));
        assert!(store.get(EvidenceId::new(0)).is_none());
    }
}
