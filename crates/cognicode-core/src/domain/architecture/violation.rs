//! Architecture violation DTO (e77.1 WU1).
//!
//! An [`ArchitectureViolation`] is the **primary output** of the
//! architecture evaluator. It is a *detection*, not a *finding*:
//!
//! * it carries **no** `DetectorAuthority` — the violation cannot
//!   gate any policy by itself;
//! * it carries **no** `EvidenceId` — evidence, when it exists,
//!   lives in the canonical kernel and is referenced through
//!   [`GroundingRef`];
//! * it carries **no** `DetectorDigests` — there is no detector
//!   behind an architecture observation.
//!
//! The split mirrors the canonical evidence path that every other
//! detector finding already follows:
//!
//! ```text
//! observation
//!    ↓
//! ProducedEvidence { grounding: Option<GroundingRef> }
//!    ↓ CanonicalEvidenceWriter::persist
//! EvidenceBinding (grounded → real EvidenceId; ungrounded → no id)
//!    ↓ FindingAssembler
//! Finding
//!    ↓ FindingVerifier::verify_for_gate
//! gate-eligible only if grounded
//! ```
//!
//! A violation is the "observation" stage. Assembly into a `Finding`
//! happens downstream, **after** the canonical write step. Until
//! then, the violation is explanatory only: a human or an automated
//! tool can act on it, but no gate evaluates it.
//!
//! ## Identity
//!
//! `ArchitectureViolation::id` is a deterministic fingerprint
//! derived from `(constraint_id, file_path, line, dependency_path)`.
//! It is **not** an `EvidenceId` and must never be used as one. It
//! is a *deduplication* and *navigation* key: two evaluations on the
//! same source yield the same id, so the violation can be tracked
//! across runs without flakiness.
//!
//! Pure domain: no I/O, no `sqlx`/`tokio`.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::domain::architecture::{ArchitectureConstraintId, LayerId};
use crate::domain::findings::{FindingKind, GroundingRef};

/// Stable identifier for a single violation: a deterministic
/// fingerprint over the tuple `(constraint_id, file_path, line,
/// dependency_path)`. Used for deduplication and navigation, never
/// for evidence.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ViolationId(u64);

impl ViolationId {
    /// Compute the deterministic fingerprint. The function is the
    /// single source of truth for violation identity: any change to
    /// the hashing is a breaking change for downstream consumers.
    pub fn compute(
        constraint_id: &ArchitectureConstraintId,
        file_path: &str,
        line: u32,
        dependency_path: &str,
    ) -> Self {
        // FNV-1a 64-bit, same algorithm as the (now-removed) FNV in
        // the e77 first slice's `stable_hash_id`. Kept here so the
        // dedup key is stable across the corrective cycle.
        const FNV_OFFSET: u64 = 0xcbf29ce484222325;
        const FNV_PRIME: u64 = 0x100000001b3;
        let mut hash = FNV_OFFSET;
        for byte in constraint_id.as_str().as_bytes() {
            hash ^= *byte as u64;
            hash = hash.wrapping_mul(FNV_PRIME);
        }
        hash = hash.wrapping_mul(FNV_PRIME); // separator
        for byte in file_path.as_bytes() {
            hash ^= *byte as u64;
            hash = hash.wrapping_mul(FNV_PRIME);
        }
        hash = hash.wrapping_mul(FNV_PRIME); // separator
        for byte in line.to_le_bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(FNV_PRIME);
        }
        hash = hash.wrapping_mul(FNV_PRIME); // separator
        for byte in dependency_path.as_bytes() {
            hash ^= *byte as u64;
            hash = hash.wrapping_mul(FNV_PRIME);
        }
        Self(hash)
    }

    /// The raw u64.
    pub fn raw(&self) -> u64 {
        self.0
    }
}

impl fmt::Display for ViolationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "violation:{}", self.0)
    }
}

/// A single architecture observation emitted by the evaluator.
///
/// Carries **no** `DetectorAuthority`, **no** `EvidenceId`, **no**
/// `DetectorDigests`. The optional [`GroundingRef`] is the bridge
/// to canonical evidence; when it is `None`, the violation is
/// *ungrounded* and may not gate (it may still be explanatory).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchitectureViolation {
    /// Deterministic identity (fingerprint of the violation tuple).
    pub id: ViolationId,
    /// The constraint that fired.
    pub constraint_id: ArchitectureConstraintId,
    /// The kind of finding this violation will become *if* it is
    /// ever assembled into a `Finding`. Carried here so that
    /// downstream assembly does not need to re-derive it.
    pub finding_kind: FindingKind,
    /// The source file the violation lives in.
    pub file_path: String,
    /// Module path hint, if the parser supplied one.
    pub module_path: Option<String>,
    /// 1-based line number in `file_path`.
    pub line: u32,
    /// The dependency path that triggered the rule (e.g.
    /// `infrastructure::db` for a layer rule).
    pub dependency_path: String,
    /// The source layer the rule fired on (e.g. `LayerId::Domain`).
    pub from_layer: LayerId,
    /// Optional canonical grounding. When `None`, the violation is
    /// explanatory only — it cannot gate. When `Some`, the violation
    /// *may* gate once it has been routed through canonical evidence
    /// (see e77.1 WU2).
    pub grounding: Option<GroundingRef>,
    /// Human-readable rationale (carried over from the rule body).
    pub rationale: String,
}
