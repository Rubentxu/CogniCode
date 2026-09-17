//! PromotionAuthorizationPolicy (e80a — M9).
//!
//! This module adds the **authority** step that e73 deliberately left out. e73
//! answers "is this change technically promotable?" (`PromotionEvaluation`).
//! This module answers "who is allowed to promote it?" (`PromotionAuthorization`).
//!
//! ## Semantic boundary
//!
//! ```text
//! Trial PASS        = technical evidence
//! Policy PASS       = policy accepts
//! Evaluation CLEAN  = the change is promotable (technical readiness / value)
//! External approval = authority
//! PromotionPermit   = effective capability
//! ```
//!
//! Each stage means exactly one thing. Authorship is NEVER folded into
//! [`evaluate_promotion`](super::evaluation::evaluate_promotion): technical
//! readiness and authority are different questions and are decided by
//! different code.
//!
//! ## The invariant this module establishes
//!
//! ```text
//! automated author
//! + perfect trial
//! + PolicyGate::Pass
//! + CleanPromotionReady
//! + forged "human" ActorRef
//! + NO trusted external verification
//! = NO PromotionPermit
//! ```
//!
//! An [`ActorRef`] is public, caller-constructible data — it is an identity
//! *claim*, not proof. Authority is therefore modelled as a sealed artifact
//! ([`VerifiedExternalApproval`], [`PromotionAuthorization`]) that can only be
//! minted after a trusted [`ExternalApprovalVerifier`] confirms the approval.
//!
//! ## Layering
//!
//! ```text
//! authorship / policy
//!        ↓
//! PromotionAuthorization      (sealed)
//!        ↓
//! PromotionPermit             (minted only from an authorization)
//!        ↓
//! apply_with_permit           (knows nothing about authorship)
//! ```
//!
//! ## Purity
//!
//! Everything here is pure and total: no clock, no I/O, no randomness. The
//! verifier port is the only seam to the outside world, and the shipped
//! default, [`RejectAllExternalApprovals`], authorises nothing.
//!
//! ## Audit / event seam (WU8)
//!
//! This module does **not** emit events and does **not** persist approvals.
//! e80a deliberately introduces no event bus and no durable approval store:
//! the module has no event seam today and `issue_promotion_permit` has no
//! production caller, so wiring one now would mean inventing an integration
//! with no consumer.
//!
//! The seam is instead the typed outcomes. A composition-root caller with a
//! real event log can map:
//!
//! * [`ExternalApprovalAuthority::verify`] → `promotion.approval.requested` /
//!   `promotion.approval.verified`
//! * [`PromotionAuthorizationPolicy::authorize`] → `promotion.authorization.granted`
//!   / `promotion.authorization.denied`
//! * [`issue_promotion_permit`](super::permit::issue_promotion_permit) →
//!   `promotion.permit.issued`
//!
//! onto the existing Intelligence Event Log once such a caller exists. This is
//! an explicit adapter seam, not a new architecture.

use crate::application::change_proposal::proposal::{
    ChangeProposal, ChangeProposalId, RequestedBy,
};
use crate::application::promotion_authority::evaluation::{PromotionDryRun, PromotionStatus};
use crate::application::software_world::world::SoftwareWorldId;
use crate::domain::evidence_kernel::ids::SnapshotId;
use crate::domain::execution::actor::{ActorKind, ActorRef};

// ---------------------------------------------------------------------------
// WU1 — the exact promotion attempt an approval is bound to
// ---------------------------------------------------------------------------

/// The exact promotion attempt an external approval authorises.
///
/// An approval must be bound to *this* attempt, not merely to a proposal id:
/// a proposal-only approval could be replayed against a different candidate or
/// a different current world. The target is always derived from the dry-run's
/// lineage ([`PromotionApprovalTarget::from_dry_run`]) and is never
/// caller-supplied independently of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromotionApprovalTarget {
    /// The proposal being promoted.
    pub proposal: ChangeProposalId,
    /// Base world id (when the proposal was created).
    pub base_world: SoftwareWorldId,
    /// Candidate world id (after the trial ran).
    pub candidate_world: SoftwareWorldId,
    /// Current world id (right now).
    pub current_world: SoftwareWorldId,
    /// Base snapshot id.
    pub base_snapshot: SnapshotId,
    /// Current snapshot id.
    pub current_snapshot: SnapshotId,
}

impl PromotionApprovalTarget {
    /// Derive the target from a dry-run's lineage.
    ///
    /// Reuses [`PromotionLineage`](super::evaluation::PromotionLineage) — no
    /// promotional data is duplicated, the target is a view of it.
    pub fn from_dry_run(dry_run: &PromotionDryRun) -> Self {
        Self {
            proposal: dry_run.lineage.proposal.clone(),
            base_world: dry_run.lineage.base_world.clone(),
            candidate_world: dry_run.lineage.candidate_world.clone(),
            current_world: dry_run.lineage.current_world.clone(),
            base_snapshot: dry_run.lineage.base_snapshot,
            current_snapshot: dry_run.lineage.current_snapshot,
        }
    }
}

// ---------------------------------------------------------------------------
// WU2 — external approval verification seam
// ---------------------------------------------------------------------------

/// A request for trusted external verification of a human approval.
///
/// Plain data: the exact target plus the claimed approver. The claimed
/// approver is only a claim; it becomes meaningful only after a verifier
/// confirms that this external authority really approved this exact target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalApprovalRequest {
    /// The exact promotion attempt to be approved.
    pub target: PromotionApprovalTarget,
    /// The approver the caller claims approved it. NOT proof.
    pub claimed_approver: ActorRef,
}

impl ExternalApprovalRequest {
    /// Build a request. Pure.
    pub fn new(target: PromotionApprovalTarget, claimed_approver: ActorRef) -> Self {
        Self {
            target,
            claimed_approver,
        }
    }
}

/// Port: trusted external verification of a human approval.
///
/// The implementation answers exactly one question: *did this external human
/// authority actually approve THIS exact target?* The real implementation
/// (a trusted identity service, an approval record store, a signed
/// attestation) is supplied at the composition root. This cycle ships only a
/// deterministic test double and the fail-closed default.
pub trait ExternalApprovalVerifier {
    /// `true` only when the external authority actually approved the request's
    /// exact target.
    fn verify(&self, request: &ExternalApprovalRequest) -> bool;
}

/// The fail-closed default: verifies nothing.
///
/// There is deliberately no permissive production fallback.
#[derive(Debug, Clone, Copy, Default)]
pub struct RejectAllExternalApprovals;

impl ExternalApprovalVerifier for RejectAllExternalApprovals {
    fn verify(&self, _request: &ExternalApprovalRequest) -> bool {
        false
    }
}

/// Why an external approval could not be verified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExternalApprovalError {
    /// The claimed approver is not human. Only a human can grant the
    /// external authority an automated author requires.
    NotHuman {
        /// The kind the caller claimed.
        actual: ActorKind,
    },
    /// The claimed approver id was empty.
    EmptyApprover,
    /// The verifier did not confirm the approval.
    NotVerified,
}

/// Private seal for [`VerifiedExternalApproval`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ExternalApprovalSeal(());

/// A human approval that a trusted [`ExternalApprovalVerifier`] has confirmed.
///
/// Private fields, private seal, and **no `Serialize` / `Deserialize`**: it
/// cannot be produced by writing
/// `VerifiedExternalApproval { approver: ActorRef::human("alice") }`, and it
/// cannot be reconstructed from serialized bytes. The only creation path is
/// [`ExternalApprovalAuthority::verify`] over a verifier that returns `true`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedExternalApproval {
    target: PromotionApprovalTarget,
    approver: ActorRef,
    _seal: ExternalApprovalSeal,
}

impl VerifiedExternalApproval {
    /// Who approved (read-only audit).
    pub fn approver(&self) -> &ActorRef {
        &self.approver
    }

    /// What exact target was approved (read-only audit).
    pub fn target(&self) -> &PromotionApprovalTarget {
        &self.target
    }
}

/// The only minter of [`VerifiedExternalApproval`]s.
#[derive(Debug, Clone, Copy)]
pub struct ExternalApprovalAuthority;

impl ExternalApprovalAuthority {
    /// Run a verifier over a request; mint a [`VerifiedExternalApproval`] only
    /// on success.
    ///
    /// Structural checks run first (the approver must be a non-empty human),
    /// then the verifier decides.
    pub fn verify(
        verifier: &dyn ExternalApprovalVerifier,
        request: ExternalApprovalRequest,
    ) -> Result<VerifiedExternalApproval, ExternalApprovalError> {
        if request.claimed_approver.kind != ActorKind::Human {
            return Err(ExternalApprovalError::NotHuman {
                actual: request.claimed_approver.kind,
            });
        }
        if request.claimed_approver.id.trim().is_empty() {
            return Err(ExternalApprovalError::EmptyApprover);
        }
        if !verifier.verify(&request) {
            return Err(ExternalApprovalError::NotVerified);
        }
        Ok(VerifiedExternalApproval {
            target: request.target,
            approver: request.claimed_approver,
            _seal: ExternalApprovalSeal(()),
        })
    }
}

// ---------------------------------------------------------------------------
// WU4 — the authority decision
// ---------------------------------------------------------------------------

/// Private seal for [`PromotionAuthorization`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PromotionAuthorizationSeal(());

/// The authority decision: this exact promotion attempt is authorised.
///
/// Private fields, private seal, no serde. It can only be produced by
/// [`PromotionAuthorizationPolicy::authorize`], and
/// [`issue_promotion_permit`](super::permit::issue_promotion_permit) accepts
/// only this type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromotionAuthorization {
    dry_run: PromotionDryRun,
    author: RequestedBy,
    external_approval: Option<VerifiedExternalApproval>,
    _seal: PromotionAuthorizationSeal,
}

impl PromotionAuthorization {
    /// The dry-run this authorization was granted over.
    pub fn dry_run(&self) -> &PromotionDryRun {
        &self.dry_run
    }

    /// The author class the decision was made about (audit).
    pub fn author(&self) -> &RequestedBy {
        &self.author
    }

    /// The verified external approval, when one was required and supplied
    /// (audit).
    pub fn external_approval(&self) -> Option<&VerifiedExternalApproval> {
        self.external_approval.as_ref()
    }

    /// The exact approved target.
    pub fn target(&self) -> PromotionApprovalTarget {
        PromotionApprovalTarget::from_dry_run(&self.dry_run)
    }
}

/// Why a promotion was not authorised.
///
/// Failures are typed and disjoint; they are never collapsed into a bool.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromotionAuthorizationError {
    /// The dry-run is not `CleanPromotionReady`.
    DryRunNotClean {
        /// The actual status.
        actual: PromotionStatus,
    },
    /// The dry-run is for a different proposal than the one supplied.
    ProposalMismatch {
        /// The proposal the caller authorised.
        expected: ChangeProposalId,
        /// The proposal the dry-run belongs to.
        actual: ChangeProposalId,
    },
    /// The proposal's author is automated (plugin or LLM agent) and no
    /// verified external approval was supplied.
    AutomatedAuthorRequiresExternalApproval {
        /// The automated author class.
        author: RequestedBy,
    },
    /// A verified external approval was supplied, but it is bound to a
    /// different promotion attempt (wrong proposal / candidate / current world
    /// / stale snapshot). Boxed to keep the error type small.
    ApprovalTargetMismatch {
        /// The target derived from the dry-run.
        expected: Box<PromotionApprovalTarget>,
        /// The target the approval is bound to.
        actual: Box<PromotionApprovalTarget>,
    },
}

/// The canonical authority step between a [`PromotionDryRun`] and a
/// [`PromotionPermit`](super::permit::PromotionPermit).
#[derive(Debug, Clone, Copy)]
pub struct PromotionAuthorizationPolicy;

impl PromotionAuthorizationPolicy {
    /// Decide whether a promotion attempt is authorised.
    ///
    /// Rules, in order (fail-closed on the first violation):
    ///
    /// 1. The dry-run must be `CleanPromotionReady`. Otherwise
    ///    [`PromotionAuthorizationError::DryRunNotClean`].
    /// 2. The dry-run must belong to `proposal`. Otherwise
    ///    [`PromotionAuthorizationError::ProposalMismatch`].
    /// 3. If the author is automated, a [`VerifiedExternalApproval`] bound to
    ///    the exact target is required. Missing →
    ///    [`PromotionAuthorizationError::AutomatedAuthorRequiresExternalApproval`];
    ///    mismatched → [`PromotionAuthorizationError::ApprovalTargetMismatch`].
    /// 4. Otherwise (a human author) the existing e73 semantics are preserved:
    ///    a clean dry-run is authorised with no *new* co-approval requirement.
    ///    If an approval happens to be supplied anyway, it must still match the
    ///    exact target — a mismatched approval is never silently recorded.
    pub fn authorize(
        proposal: &ChangeProposal,
        dry_run: PromotionDryRun,
        external_approval: Option<VerifiedExternalApproval>,
    ) -> Result<PromotionAuthorization, PromotionAuthorizationError> {
        // 1. Clean dry-run.
        if dry_run.status != PromotionStatus::CleanPromotionReady {
            return Err(PromotionAuthorizationError::DryRunNotClean {
                actual: dry_run.status.clone(),
            });
        }

        // 2. The dry-run must be for this proposal.
        if dry_run.lineage.proposal != proposal.id {
            return Err(PromotionAuthorizationError::ProposalMismatch {
                expected: proposal.id.clone(),
                actual: dry_run.lineage.proposal.clone(),
            });
        }

        let target = PromotionApprovalTarget::from_dry_run(&dry_run);

        // 3. Automated author: verified external approval bound to the target.
        if proposal.is_automated() {
            let approval = match external_approval {
                Some(approval) => approval,
                None => {
                    return Err(
                        PromotionAuthorizationError::AutomatedAuthorRequiresExternalApproval {
                            author: proposal.requested_by.clone(),
                        },
                    );
                }
            };
            if approval.target() != &target {
                return Err(PromotionAuthorizationError::ApprovalTargetMismatch {
                    expected: Box::new(target),
                    actual: Box::new(approval.target().clone()),
                });
            }
            return Ok(PromotionAuthorization {
                dry_run,
                author: proposal.requested_by.clone(),
                external_approval: Some(approval),
                _seal: PromotionAuthorizationSeal(()),
            });
        }

        // 4. Human author: preserve e73 semantics; validate an approval only if
        //    one was supplied.
        if let Some(approval) = &external_approval
            && approval.target() != &target
        {
            return Err(PromotionAuthorizationError::ApprovalTargetMismatch {
                expected: Box::new(target),
                actual: Box::new(approval.target().clone()),
            });
        }

        Ok(PromotionAuthorization {
            dry_run,
            author: proposal.requested_by.clone(),
            external_approval,
            _seal: PromotionAuthorizationSeal(()),
        })
    }
}
