//! Admission sources — where a definition came from.
//!
//! **Ungated general trust vocabulary (cycle e64).** M6 introduced this for
//! detector definitions; M7.3's behavior admission needs the same trust ladder,
//! which is the proof it was never detector vocabulary. The canonical definition
//! lives here and `findings::admission` re-exports it, so there is still exactly
//! one ladder.
//!
//! Trust is *eligibility*, never authority: an eligible source still needs an
//! approver, and the ladder only decides what a definition may be considered
//! for.

use serde::{Deserialize, Serialize};

/// Where a definition came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdmissionSource {
    /// Shipped with the platform.
    Builtin,
    /// Curated and reviewed by a human.
    HumanCurated,
    /// Proposed by an AI agent.
    AiGenerated,
    /// Imported from an external source (e.g. a pack).
    Imported,
}

impl AdmissionSource {
    /// Whether this source is *eligible* to be trusted to gate.
    ///
    /// Eligibility alone never grants authority: a promotion still needs an
    /// [`ApprovalVerifier`] to accept it.
    pub fn is_trusted_to_gate(self) -> bool {
        matches!(self, Self::Builtin | Self::HumanCurated)
    }
}

impl std::fmt::Display for AdmissionSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Builtin => "Builtin",
            Self::HumanCurated => "HumanCurated",
            Self::AiGenerated => "AiGenerated",
            Self::Imported => "Imported",
        })
    }
}
