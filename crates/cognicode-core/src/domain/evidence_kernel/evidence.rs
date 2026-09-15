//! Kernel evidence — graded support/refutation attached to facts.
//!
//! Evidence never replaces a [`Fact`](super::fact::Fact); it grades one.
//! Unlike facts, evidence carries no LLM-producer exclusion here: the
//! type-level contract (design D3) blocks LLM output from extracted facts,
//! while evidence such as agent claims may legitimately carry
//! `ProducerKind::LlmAgent` provenance.

use serde::{Deserialize, Serialize};

use super::fact::ProvenanceRecord;
use super::ids::{EvidenceId, FactId};

/// Re-export shim (cycle e62.4): the canonical definition moved to the
/// ungated [`crate::domain::kernel_ids::EvidenceGrade`] so the findings domain
/// can reason about it without the kernel feature.
pub use crate::domain::kernel_ids::EvidenceGrade;

/// A graded statement about one fact.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Evidence {
    /// Producer-assigned evidence id.
    pub id: EvidenceId,
    /// The fact this evidence grades.
    pub fact: FactId,
    /// Support / refute / corroborate.
    pub grade: EvidenceGrade,
    /// How this evidence was obtained.
    pub provenance: ProvenanceRecord,
}

#[cfg(test)]
mod tests {
    use crate::domain::value_objects::Provenance;

    use super::super::fact::ProvenanceRecord;
    use super::super::ids::{EvidenceId, FactId};
    use super::*;

    /// Every `EvidenceGrade` variant must round-trip losslessly through
    /// bincode and JSON (exhaustive-variant house pattern).
    #[test]
    fn evidence_grade_round_trip_exhaustive() {
        for grade in [
            EvidenceGrade::Supports,
            EvidenceGrade::Refutes,
            EvidenceGrade::Corroborates,
        ] {
            let bytes =
                bincode::serde::encode_to_vec(grade, bincode::config::standard()).expect("encode");
            let (decoded, _): (EvidenceGrade, usize) =
                bincode::serde::decode_from_slice(&bytes, bincode::config::standard())
                    .expect("decode");
            assert_eq!(decoded, grade);

            let json = serde_json::to_string(&grade).expect("json serialize");
            let parsed: EvidenceGrade = serde_json::from_str(&json).expect("json deserialize");
            assert_eq!(parsed, grade);
        }
    }

    /// A full `Evidence` record must round-trip with fact, grade and
    /// provenance identity preserved.
    #[test]
    fn evidence_round_trip_preserves_identity() {
        let evidence = Evidence {
            id: EvidenceId::new(11),
            fact: FactId::new(3),
            grade: EvidenceGrade::Corroborates,
            provenance: ProvenanceRecord::new(
                Provenance::Tested,
                crate::domain::evidence_kernel::fact::ProducerKind::DeterministicAnalyzer,
                Some("cargo test output".to_string()),
            ),
        };

        let bytes =
            bincode::serde::encode_to_vec(&evidence, bincode::config::standard()).expect("encode");
        let (decoded, _): (Evidence, usize) =
            bincode::serde::decode_from_slice(&bytes, bincode::config::standard()).expect("decode");
        assert_eq!(decoded, evidence);

        let json = serde_json::to_string(&evidence).expect("json serialize");
        let parsed: Evidence = serde_json::from_str(&json).expect("json deserialize");
        assert_eq!(parsed, evidence);
    }

    /// `EvidenceGrade` must render stable Display strings for diagnostics.
    #[test]
    fn evidence_grade_display() {
        assert_eq!(EvidenceGrade::Supports.to_string(), "Supports");
        assert_eq!(EvidenceGrade::Refutes.to_string(), "Refutes");
        assert_eq!(EvidenceGrade::Corroborates.to_string(), "Corroborates");
    }
}
