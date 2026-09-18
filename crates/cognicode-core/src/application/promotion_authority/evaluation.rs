//! PromotionEvaluation (e73 WU1 — M9).
//!
//! Given a [`ChangeProposal`](crate::application::change_proposal::proposal::ChangeProposal),
//! the world that existed when the proposal was created (`base A`),
//! the world after the trial (`candidate B`), and the world as it is
//! right now (`current C`), produce a [`PromotionDryRun`] describing
//! whether promotion is safe, requires re-evaluation, or is blocked.
//!
//! ## Three-way diff strategy
//!
//! - If `base.base_snapshot == current.base_snapshot` → C has not
//!   diverged from A in terms of canonical base. The candidate may be
//!   promoted as long as the trial passed.
//! - If `base.base_snapshot != current.base_snapshot` → C diverged.
//!   Promotion is not safe; re-evaluation is required before any
//!   apply. We do NOT silently re-run the trial or "best-effort"
//!   promote; the envelope is explicit about this.
//!
//! ## Trial-evidence gating
//!
//! A promotion without trial evidence is blocked. A promotion whose
//! trial evidence has a non-pass gate is blocked. A promotion whose
//! trial evidence is for a different proposal is blocked. These are
//! fail-closed conditions; no fall-through to "best-effort promote"
//! exists.
//!
//! ## Pure / deterministic / no I/O
//!
//! See [`crate::application::promotion_authority`] for the umbrella
//! rationale. The evaluation is total and pure; it does not run the
//! trial, does not touch the filesystem, and does not record an
//! audit trail (the audit trail lives in e73 WU2).

use crate::application::change_proposal::proposal::ChangeProposal;
use crate::application::change_proposal::proposal::ChangeProposalId;
use crate::application::change_proposal::trial::TrialEvidence;
use crate::application::policy_gate::PolicyOutcome;
use crate::application::software_world::world::SoftwareWorld;
use crate::application::software_world::world::SoftwareWorldId;
use crate::domain::evidence_kernel::ids::SnapshotId;

/// Three-way promotion input.
///
/// The base/candidate/current worlds together describe the timeline:
/// A → trial produced B → C is now. The
/// [`evaluate_promotion`](super::evaluate_promotion) function compares
/// them and produces a [`PromotionDryRun`].
#[derive(Debug, Clone)]
pub struct PromotionEvaluationInput<'a> {
    /// Proposal being promoted.
    pub proposal: ChangeProposalId,
    /// Base world — the world that existed when the proposal was
    /// created.
    pub base: &'a SoftwareWorld,
    /// Candidate world — the world produced by the trial.
    pub candidate: &'a SoftwareWorld,
    /// Current world — the world as it is right now.
    pub current: &'a SoftwareWorld,
}

/// Status of a three-way promotion evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromotionStatus {
    /// `base == current` in canonical base; trial passed. Promotion
    /// may proceed (still subject to a `PromotionPermit` — see e73
    /// WU2).
    CleanPromotionReady,
    /// `current` has diverged from `base`. Re-evaluation is required
    /// before any promotion.
    ConflictRequiresReevaluation,
    /// Promotion is blocked by a precondition failure.
    Blocked(PromotionBlockReason),
}

/// Why a promotion is blocked.
///
/// The variants are deliberately disjoint so that callers (audit
/// reports, CI dashboards) can distinguish them at a glance.
///
/// ## Authorship is not a technical block (e80a)
///
/// This enum used to carry an `AutomatedAuthorWithoutCoAuth` variant that was
/// never constructed. Automated-author enforcement is an *authority* decision,
/// not a technical-readiness one, so it now lives in
/// [`PromotionAuthorizationPolicy`](super::authorization::PromotionAuthorizationPolicy).
/// The variant was removed rather than rewired, so there is exactly one
/// automated-author rule in the codebase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromotionBlockReason {
    /// No trial evidence was supplied. Promotion requires trial.
    NoTrialEvidence,
    /// Trial evidence's gate decision was not Pass. Pass is the only
    /// outcome that lets promotion proceed.
    TrialGateNotPassing { actual: PolicyOutcome },
    /// Trial evidence is for a different proposal than the one being
    /// promoted.
    TrialProposalMismatch {
        expected: ChangeProposalId,
        actual: ChangeProposalId,
    },
    /// (e83) The proposal targets a different base world than the one
    /// supplied. The tested candidate would not be evidence for this
    /// proposal.
    ProposalBaseWorldMismatch {
        /// The proposal's `base_world`.
        proposal_world: SoftwareWorldId,
        /// The base world supplied for evaluation.
        base_world: SoftwareWorldId,
    },
    /// (e83) The candidate world was not derived from the base world.
    CandidateNotDerivedFromBase {
        /// The candidate world.
        candidate_world: SoftwareWorldId,
        /// Its actual parent, if any.
        actual_parent: Option<SoftwareWorldId>,
        /// The base world it should have been forked from.
        base_world: SoftwareWorldId,
    },
    /// (e83) The candidate was measured against a different canonical base
    /// snapshot than the base world offers.
    CandidateBaseSnapshotMismatch {
        /// The candidate's `base_snapshot`.
        candidate_snapshot: SnapshotId,
        /// The base world's `base_snapshot`.
        base_snapshot: SnapshotId,
    },
    /// (e83) The trial was run against a different world than the candidate
    /// being promoted.
    TrialWorldMismatch {
        /// The candidate world under promotion.
        expected: SoftwareWorldId,
        /// The world the trial recorded.
        actual: SoftwareWorldId,
    },
    /// (e83) The trial was run against a different canonical base snapshot
    /// than the base world offers.
    TrialBaseSnapshotMismatch {
        /// The base world's `base_snapshot`.
        expected: SnapshotId,
        /// The snapshot the trial recorded.
        actual: SnapshotId,
    },
}

/// Lineage metadata attached to a promotion dry-run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromotionLineage {
    /// Proposal being promoted.
    pub proposal: ChangeProposalId,
    /// Base world id.
    pub base_world: crate::application::software_world::world::SoftwareWorldId,
    /// Candidate world id.
    pub candidate_world: crate::application::software_world::world::SoftwareWorldId,
    /// Current world id.
    pub current_world: crate::application::software_world::world::SoftwareWorldId,
    /// Base snapshot id (when the proposal was created).
    pub base_snapshot: SnapshotId,
    /// Current snapshot id (right now).
    pub current_snapshot: SnapshotId,
    /// Whether `base.base_snapshot == current.base_snapshot` (C has
    /// not diverged from A).
    pub base_matches_current: bool,
}

/// Dry-run outcome of a three-way promotion evaluation.
///
/// This is a *value*: it describes the situation but does not
/// authorise an apply. Authority is granted separately by
/// [`PromotionPermit`](super::permit::PromotionPermit).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromotionDryRun {
    /// Status of the dry-run.
    pub status: PromotionStatus,
    /// Lineage metadata for audit.
    pub lineage: PromotionLineage,
}

/// Three-way promotion evaluation.
///
/// Pure function. Same inputs → same output. No IO, no clock.
///
/// Rules (in order of evaluation, fail-closed on first blocking
/// condition):
///
/// 1. If no trial evidence → [`PromotionStatus::Blocked`] with
///    [`PromotionBlockReason::NoTrialEvidence`].
/// 2. If trial's gate outcome is not `Pass` → blocked with
///    [`PromotionBlockReason::TrialGateNotPassing`].
/// 3. If trial's proposal id does not match the promotion's proposal
///    id → blocked with [`PromotionBlockReason::TrialProposalMismatch`].
/// 4. If `base.base_snapshot != current.base_snapshot` → conflict;
///    [`PromotionStatus::ConflictRequiresReevaluation`].
/// 5. Otherwise → [`PromotionStatus::CleanPromotionReady`].
///
/// Rule 4 is the key "do not promote against a drifted base" check.
/// The trial's verdict is valid only against the base snapshot; if
/// the world has moved on since the trial, the verdict is stale.
pub fn evaluate_promotion(
    input: PromotionEvaluationInput,
    trial: Option<&TrialEvidence>,
) -> PromotionDryRun {
    let lineage = build_lineage(
        input.proposal.clone(),
        input.base,
        input.candidate,
        input.current,
    );

    // 1. Trial required.
    let trial = match trial {
        Some(t) => t,
        None => {
            return PromotionDryRun {
                status: PromotionStatus::Blocked(PromotionBlockReason::NoTrialEvidence),
                lineage,
            };
        }
    };

    // 2. Trial gate must be Pass.
    if trial.gate.outcome != PolicyOutcome::Pass {
        return PromotionDryRun {
            status: PromotionStatus::Blocked(PromotionBlockReason::TrialGateNotPassing {
                actual: trial.gate.outcome,
            }),
            lineage,
        };
    }

    // 3. Trial must be for the same proposal.
    if trial.proposal_id != input.proposal {
        return PromotionDryRun {
            status: PromotionStatus::Blocked(PromotionBlockReason::TrialProposalMismatch {
                expected: input.proposal.clone(),
                actual: trial.proposal_id.clone(),
            }),
            lineage,
        };
    }

    // 4. C must not have diverged from A.
    if !lineage.base_matches_current {
        return PromotionDryRun {
            status: PromotionStatus::ConflictRequiresReevaluation,
            lineage,
        };
    }

    // 5. Clean.
    PromotionDryRun {
        status: PromotionStatus::CleanPromotionReady,
        lineage,
    }
}

/// Build the lineage view shared by both evaluators.
fn build_lineage(
    proposal: ChangeProposalId,
    base: &SoftwareWorld,
    candidate: &SoftwareWorld,
    current: &SoftwareWorld,
) -> PromotionLineage {
    PromotionLineage {
        proposal,
        base_world: base.id.clone(),
        candidate_world: candidate.id.clone(),
        current_world: current.id.clone(),
        base_snapshot: base.base_snapshot,
        current_snapshot: current.base_snapshot,
        base_matches_current: base.base_snapshot == current.base_snapshot,
    }
}

// ============================================================================
// e83 — authority-bearing promotion evaluation
// ============================================================================

/// Authority-bearing promotion input (e83).
///
/// Unlike [`PromotionEvaluationInput`], this carries the **proposal itself**, so
/// the evaluator can validate full lineage: proposal target, candidate
/// derivation, candidate base snapshot, and the trial's world/snapshot identity.
///
/// The generic M9 path stays generic on purpose (e83 WU10): a `ChangeProposal`
/// that is not a continuous-improvement candidate does not need historical
/// replay lineage. Any promotion performed **through the governed-improvement
/// API** uses this evaluator structurally.
pub struct GovernedPromotionInput<'a> {
    /// The proposal being promoted.
    pub proposal: &'a ChangeProposal,
    /// The world the proposal was authored against.
    pub base: &'a SoftwareWorld,
    /// The candidate world the trial measured.
    pub candidate: &'a SoftwareWorld,
    /// The world as it is right now.
    pub current: &'a SoftwareWorld,
}

/// Evaluate a promotion with full lineage validation (e83).
///
/// Rules, in order (fail-closed on the first violation):
///
/// 1. `proposal.base_world == base.id`.
/// 2. `candidate.parent_world == Some(base.id)`.
/// 3. `candidate.base_snapshot == base.base_snapshot`.
/// 4. Trial evidence is required.
/// 5. Trial gate outcome is `Pass`.
/// 6. `trial.proposal_id == proposal.id`.
/// 7. `trial.world_id == candidate.id`.
/// 8. `trial.base_snapshot == base.base_snapshot`.
/// 9. `base.base_snapshot == current.base_snapshot`.
/// 10. Otherwise: clean.
///
/// Only after identity and lineage are valid may the trial gate be trusted as
/// evidence for **this** candidate.
pub fn evaluate_promotion_lineage(
    input: GovernedPromotionInput<'_>,
    trial: Option<&TrialEvidence>,
) -> PromotionDryRun {
    let lineage = build_lineage(
        input.proposal.id.clone(),
        input.base,
        input.candidate,
        input.current,
    );

    // 1. The proposal must target the base world we are evaluating against.
    if input.proposal.base_world != input.base.id {
        return PromotionDryRun {
            status: PromotionStatus::Blocked(PromotionBlockReason::ProposalBaseWorldMismatch {
                proposal_world: input.proposal.base_world.clone(),
                base_world: input.base.id.clone(),
            }),
            lineage,
        };
    }

    // 2. The candidate must be derived from that base.
    if input.candidate.parent_world.as_ref() != Some(&input.base.id) {
        return PromotionDryRun {
            status: PromotionStatus::Blocked(PromotionBlockReason::CandidateNotDerivedFromBase {
                candidate_world: input.candidate.id.clone(),
                actual_parent: input.candidate.parent_world.clone(),
                base_world: input.base.id.clone(),
            }),
            lineage,
        };
    }

    // 3. The candidate must have been measured against the same canonical base.
    if input.candidate.base_snapshot != input.base.base_snapshot {
        return PromotionDryRun {
            status: PromotionStatus::Blocked(PromotionBlockReason::CandidateBaseSnapshotMismatch {
                candidate_snapshot: input.candidate.base_snapshot,
                base_snapshot: input.base.base_snapshot,
            }),
            lineage,
        };
    }

    // 4. Trial required.
    let trial = match trial {
        Some(t) => t,
        None => {
            return PromotionDryRun {
                status: PromotionStatus::Blocked(PromotionBlockReason::NoTrialEvidence),
                lineage,
            };
        }
    };

    // 5. Trial gate must be Pass.
    if trial.gate.outcome != PolicyOutcome::Pass {
        return PromotionDryRun {
            status: PromotionStatus::Blocked(PromotionBlockReason::TrialGateNotPassing {
                actual: trial.gate.outcome,
            }),
            lineage,
        };
    }

    // 6. The trial must be for this proposal.
    if trial.proposal_id != input.proposal.id {
        return PromotionDryRun {
            status: PromotionStatus::Blocked(PromotionBlockReason::TrialProposalMismatch {
                expected: input.proposal.id.clone(),
                actual: trial.proposal_id.clone(),
            }),
            lineage,
        };
    }

    // 7. The trial must have measured THIS candidate world.
    if trial.world_id != input.candidate.id {
        return PromotionDryRun {
            status: PromotionStatus::Blocked(PromotionBlockReason::TrialWorldMismatch {
                expected: input.candidate.id.clone(),
                actual: trial.world_id.clone(),
            }),
            lineage,
        };
    }

    // 8. ... against THIS canonical base snapshot.
    if trial.base_snapshot != input.base.base_snapshot {
        return PromotionDryRun {
            status: PromotionStatus::Blocked(PromotionBlockReason::TrialBaseSnapshotMismatch {
                expected: input.base.base_snapshot,
                actual: trial.base_snapshot,
            }),
            lineage,
        };
    }

    // 9. C must not have diverged from A.
    if !lineage.base_matches_current {
        return PromotionDryRun {
            status: PromotionStatus::ConflictRequiresReevaluation,
            lineage,
        };
    }

    // 10. Clean.
    PromotionDryRun {
        status: PromotionStatus::CleanPromotionReady,
        lineage,
    }
}
