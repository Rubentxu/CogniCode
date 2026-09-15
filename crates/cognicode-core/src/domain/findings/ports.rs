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

use super::outcome::ProducedEvidence;
use crate::domain::kernel_ids::EvidenceId;

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

/// Records produced evidence and assigns it a kernel [`EvidenceId`].
pub trait EvidenceSink {
    /// Persist one piece of evidence, returning its assigned id.
    fn record(&mut self, evidence: ProducedEvidence) -> Result<EvidenceId, EvidenceError>;
}

/// Resolves whether an evidence id exists.
///
/// Used by [`FindingVerifier`](super::FindingVerifier) to check referential
/// truth (U42) before a finding may block.
pub trait EvidenceLookup {
    /// Whether `id` is known to the store.
    fn contains(&self, id: EvidenceId) -> bool;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tiny in-test sink/lookup to exercise the traits.
    struct VecStore {
        items: Vec<ProducedEvidence>,
    }

    impl EvidenceSink for VecStore {
        fn record(&mut self, evidence: ProducedEvidence) -> Result<EvidenceId, EvidenceError> {
            self.items.push(evidence);
            Ok(EvidenceId::new(self.items.len() as u64))
        }
    }

    impl EvidenceLookup for VecStore {
        fn contains(&self, id: EvidenceId) -> bool {
            id.get() >= 1 && (id.get() as usize) <= self.items.len()
        }
    }

    #[test]
    fn sink_assigns_ids_and_lookup_resolves_them() {
        let mut store = VecStore { items: Vec::new() };
        let id = store
            .record(ProducedEvidence {
                kind: super::super::EvidenceKind::AstMatch,
                detail: "x".to_string(),
                subject: None,
                fact: None,
            })
            .unwrap();
        assert_eq!(id, EvidenceId::new(1));
        assert!(store.contains(id));
        assert!(!store.contains(EvidenceId::new(2)));
    }

    #[test]
    fn evidence_error_displays_reason() {
        assert_eq!(
            EvidenceError::new("boom").to_string(),
            "evidence sink error: boom"
        );
    }
}
