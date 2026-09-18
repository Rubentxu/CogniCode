//! Governed improvement permit and receipt (e83 — M13 task 13.6).
//!
//! ```text
//! HeldOutGatePass          "CONFIRM did not block"
//! PromotionAuthorization   "the authority permits"      (e80a)
//! PromotionPermit          "effective apply capability" (e73)
//! GovernedImprovementPermit = HeldOutGatePass + PromotionPermit
//! ```
//!
//! The three concepts stay distinct. The governed permit is the only thing that
//! ties a held-out pass to an effective promotion permit, and it can only be
//! built from a sealed pass **and** a real permit that describe the same attempt.
//!
//! ## Apply is fail-closed
//!
//! [`apply_governed_improvement`] delegates to e73's `apply_with_permit`; the
//! world-drift checks are NOT reimplemented.

use serde::{Deserialize, Serialize};

use crate::application::change_proposal::proposal::ChangeProposalId;
use crate::application::governed_improvement::policy::HeldOutGatePass;
use crate::application::promotion_authority::permit::{
    PromotionApplyOutcome, PromotionPermit, apply_with_permit,
};
use crate::application::software_world::world::{SoftwareWorld, SoftwareWorldId};
use crate::domain::evidence_kernel::ids::SnapshotId;

/// Private seal for [`GovernedImprovementPermit`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GovernedSeal(());

/// The governed path's effective capability: held-out pass **and** promotion
/// permit for the same attempt.
///
/// Private fields, a private seal, no public constructor, and no owned
/// `PromotionPermit` accessor: the governed path cannot discard the held-out
/// requirement. Read-only accessors are exposed for audit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GovernedImprovementPermit {
    heldout: HeldOutGatePass,
    permit: PromotionPermit,
    _seal: GovernedSeal,
}

impl GovernedImprovementPermit {
    /// Compose the governed permit.
    ///
    /// Rejects a pass and a permit that do not describe the same attempt:
    ///
    /// ```text
    /// heldout.proposal_id    == permit.proposal
    /// heldout.candidate_world == permit.dry_run().lineage.candidate_world
    /// ```
    pub fn compose(
        heldout: HeldOutGatePass,
        permit: PromotionPermit,
    ) -> Result<Self, GovernedPermitError> {
        if heldout.proposal_id() != &permit.proposal {
            return Err(GovernedPermitError::ProposalMismatch {
                heldout: heldout.proposal_id().as_str().to_string(),
                permit: permit.proposal.as_str().to_string(),
            });
        }
        let permit_candidate = permit.dry_run().lineage.candidate_world.clone();
        if heldout.candidate_world() != &permit_candidate {
            return Err(GovernedPermitError::CandidateWorldMismatch {
                heldout: heldout.candidate_world().as_str().to_string(),
                permit: permit_candidate.as_str().to_string(),
            });
        }
        Ok(Self {
            heldout,
            permit,
            _seal: GovernedSeal(()),
        })
    }

    /// The held-out pass (read-only).
    pub fn heldout(&self) -> &HeldOutGatePass {
        &self.heldout
    }

    /// The promotion permit (read-only). Deliberately a reference: an owned
    /// permit would let the governed path bypass the held-out requirement.
    pub fn promotion_permit(&self) -> &PromotionPermit {
        &self.permit
    }

    /// The proposal.
    pub fn proposal_id(&self) -> &ChangeProposalId {
        &self.permit.proposal
    }

    /// The candidate world.
    pub fn candidate_world(&self) -> &SoftwareWorldId {
        &self.permit.dry_run().lineage.candidate_world
    }
}

/// Why a governed permit could not be composed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GovernedPermitError {
    /// The pass and the permit are about different proposals.
    ProposalMismatch {
        /// The pass's proposal.
        heldout: String,
        /// The permit's proposal.
        permit: String,
    },
    /// The pass and the permit are about different candidate worlds.
    CandidateWorldMismatch {
        /// The pass's candidate world.
        heldout: String,
        /// The permit's candidate world.
        permit: String,
    },
}

impl std::fmt::Display for GovernedPermitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ProposalMismatch { heldout, permit } => write!(
                f,
                "held-out pass is for proposal {heldout} but the permit is for {permit}"
            ),
            Self::CandidateWorldMismatch { heldout, permit } => write!(
                f,
                "held-out pass is for candidate world {heldout} but the permit is for {permit}"
            ),
        }
    }
}

impl std::error::Error for GovernedPermitError {}

/// Apply a governed improvement.
///
/// Fail-closed: delegates to e73's `apply_with_permit`, so the existing
/// world-drift checks are preserved exactly.
pub fn apply_governed_improvement(
    current: &SoftwareWorld,
    permit: &GovernedImprovementPermit,
) -> PromotionApplyOutcome {
    apply_with_permit(current, permit.promotion_permit())
}

/// Deterministic audit record for a completed governed improvement.
///
/// This is **audit data, not authority**: it is serializable, contains no seals,
/// and cannot reconstruct a permit. It answers: what candidate, what historical
/// data, what policy, what trial, what human approval, what permit, what world,
/// and what was applied.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernedImprovementReceipt {
    /// The proposal.
    pub proposal_id: String,
    /// The current analyzer revision, as `id@revision`.
    pub current_analyzer: String,
    /// The candidate analyzer revision, as `id@revision`.
    pub candidate_analyzer: String,
    /// The candidate world.
    pub candidate_world: String,
    /// The e82 evaluation digest.
    pub evaluation_digest: String,
    /// The held-out policy digest.
    pub heldout_policy_digest: String,
    /// The trial's identifier, when the caller supplies it.
    pub trial_id: Option<String>,
    /// The trial's gate outcome, when the caller supplies it.
    pub trial_gate_outcome: Option<String>,
    /// The promotion lineage target, as `base -> candidate`.
    pub promotion_target: String,
    /// The verified external human approver, when one was required.
    pub external_approver: Option<String>,
    /// The promotion permit id.
    pub permit_id: String,
    /// The snapshot the apply was performed against.
    pub applied_to_snapshot: u64,
}

impl GovernedImprovementReceipt {
    /// Record the applied outcome.
    ///
    /// Returns `None` when the apply was rejected: a rejected apply is not a
    /// governed improvement and must not produce a success receipt.
    pub fn record(
        permit: &GovernedImprovementPermit,
        outcome: &PromotionApplyOutcome,
        trial_id: Option<String>,
        trial_gate_outcome: Option<String>,
    ) -> Option<Self> {
        let applied_to_snapshot = match outcome {
            PromotionApplyOutcome::Applied {
                applied_to_snapshot,
                ..
            } => *applied_to_snapshot,
            PromotionApplyOutcome::Rejected(_) => return None,
        };
        let lineage = &permit.promotion_permit().dry_run().lineage;
        Some(Self {
            proposal_id: permit.proposal_id().as_str().to_string(),
            current_analyzer: describe(permit.heldout().current_descriptor()),
            candidate_analyzer: describe(permit.heldout().candidate_descriptor()),
            candidate_world: lineage.candidate_world.as_str().to_string(),
            evaluation_digest: permit.heldout().evaluation_digest().to_string(),
            heldout_policy_digest: permit.heldout().policy_digest().to_string(),
            trial_id,
            trial_gate_outcome,
            promotion_target: format!("{} -> {}", lineage.base_world, lineage.candidate_world),
            external_approver: permit
                .promotion_permit()
                .external_approver()
                .map(|actor| actor.id.clone()),
            permit_id: permit.promotion_permit().id.as_str().to_string(),
            applied_to_snapshot: applied_to_snapshot.get(),
        })
    }
}

fn describe(
    descriptor: &crate::application::shadow_evaluation::analyzer::AnalyzerDescriptor,
) -> String {
    format!(
        "{}@{}",
        descriptor.analyzer_id(),
        descriptor.revision_digest()
    )
}

/// Re-export for callers that need the snapshot type in the receipt's shape.
pub type ReceiptSnapshot = SnapshotId;
