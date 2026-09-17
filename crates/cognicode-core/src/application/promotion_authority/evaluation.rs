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

use crate::application::change_proposal::proposal::ChangeProposalId;
use crate::application::change_proposal::trial::TrialEvidence;
use crate::application::policy_gate::PolicyOutcome;
use crate::application::software_world::world::SoftwareWorld;
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
    let lineage = PromotionLineage {
        proposal: input.proposal.clone(),
        base_world: input.base.id.clone(),
        candidate_world: input.candidate.id.clone(),
        current_world: input.current.id.clone(),
        base_snapshot: input.base.base_snapshot,
        current_snapshot: input.current.base_snapshot,
        base_matches_current: input.base.base_snapshot == input.current.base_snapshot,
    };

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
