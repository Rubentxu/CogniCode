//! PromotionPermit (e73 WU2 — M9).
//!
//! A [`PromotionPermit`] is the authority to apply a
//! [`ChangeProposal`](crate::application::change_proposal::proposal::ChangeProposal)
//! to the current world. It is **only** issued from a sealed
//! [`PromotionAuthorization`](crate::application::promotion_authority::authorization::PromotionAuthorization),
//! which in turn is only granted over a `CleanPromotionReady`
//! [`PromotionDryRun`](crate::application::promotion_authority::evaluation::PromotionDryRun)
//! by
//! [`PromotionAuthorizationPolicy`](crate::application::promotion_authority::authorization::PromotionAuthorizationPolicy).
//!
//! ## e80a: minting requires authority, not just cleanliness
//!
//! Before e80a, `issue_promotion_permit(id, dry_run)` minted a permit from a
//! clean dry-run alone, so an automated author could self-promote. Now the
//! minting function takes a [`PromotionAuthorization`], which for an automated
//! author can only be obtained with a verified external human approval bound
//! to the exact promotion attempt. The compiler makes
//! `CleanPromotionReady alone → PromotionPermit` impossible.
//!
//! ## Creation is not authority
//!
//! Following the umbrella rule, the [`ChangeProposal`] does NOT
//! carry a permit. The permit is constructed separately, by the
//! [`issue_promotion_permit`] function, and only when a CleanPromotion
//! dry-run justifies it.
//!
//! ## Pure / deterministic / no I/O
//!
//! - Permit construction is total and pure: same dry-run + same world
//!   → same permit.
//! - Permit application is also pure: it validates the permit against
//!   the current world and returns a structured outcome. It does NOT
//!   write to the filesystem, run shell commands, or modify any state.
//!   The actual apply (which writes source files, updates config, etc.)
//!   lives outside M9; M9 establishes the *authority* and the *audit
//!   marker*, not the concrete mutation.
//!
//! ## Audit trail
//!
//! Each `PromotionPermit` carries the full
//! [`PromotionDryRun`](crate::application::promotion_authority::evaluation::PromotionDryRun)
//! it was issued against. This means an auditor can reconstruct
//! *why* the permit was issued: which proposal, which trial verdict,
//! which base/candidate/current worlds, and whether C had diverged.
//!
//! ## `apply_with_permit` is fail-closed
//!
//! Direct apply without a permit →
//! [`PromotionApplyError::NoPermit`]. A permit whose dry-run was not
//! CleanPromotionReady → [`PromotionApplyError::PermitInvalid`].
//! A permit whose base_snapshot no longer matches the current world
//! → [`PromotionApplyError::WorldDriftedSincePermit`].
//!
//! None of these branches "best-effort" applies anything. The audit
//! log records the rejection; no mutation happens.

use crate::application::change_proposal::proposal::ChangeProposalId;
use crate::application::promotion_authority::authorization::PromotionAuthorization;
use crate::application::promotion_authority::evaluation::{PromotionDryRun, PromotionStatus};
use crate::application::software_world::world::SoftwareWorld;
use crate::domain::evidence_kernel::ids::SnapshotId;

/// Stable identifier of a [`PromotionPermit`].
///
/// Caller-supplied; this module never mints ids at construction time.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PromotionPermitId(String);

impl PromotionPermitId {
    /// Construct from any string-shaped identifier.
    pub fn from_string(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// Borrow the underlying string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for PromotionPermitId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Authority to apply a proposal.
///
/// A permit is the only thing that grants apply power. It carries the sealed
/// [`PromotionAuthorization`] that enabled its minting, plus the
/// [`PromotionDryRun`](crate::application::promotion_authority::evaluation::PromotionDryRun)
/// that justified it, so an auditor can reconstruct *why* the permit exists and
/// *who* authorised it.
///
/// The `authorization` field is private, which makes external struct-literal
/// construction impossible: the only way to obtain a `PromotionPermit` is
/// [`issue_promotion_permit`] over a sealed [`PromotionAuthorization`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromotionPermit {
    /// Stable identifier of this permit.
    pub id: PromotionPermitId,
    /// The proposal the permit authorises.
    pub proposal: ChangeProposalId,
    /// Snapshot id the permit was issued against (the `current.base_snapshot`
    /// at issue time). The permit becomes invalid if the world's
    /// base_snapshot changes after issue.
    pub issued_for_snapshot: SnapshotId,
    /// The sealed authority decision that enabled this permit. Private so a
    /// caller cannot construct a permit by literal without authority. Boxed
    /// to keep `PromotionPermit` (and enums that carry it) small.
    authorization: Box<PromotionAuthorization>,
}

impl PromotionPermit {
    /// The dry-run the permit was issued over (audit).
    pub fn dry_run(&self) -> &PromotionDryRun {
        self.authorization.dry_run()
    }

    /// The sealed authority decision behind the permit.
    pub fn authorization(&self) -> &PromotionAuthorization {
        &self.authorization
    }

    /// The author class this permit was authorised for (audit).
    pub fn author(&self) -> &crate::application::change_proposal::proposal::RequestedBy {
        self.authorization.author()
    }

    /// The verified external approver, when one was required (audit).
    pub fn external_approver(&self) -> Option<&crate::domain::execution::actor::ActorRef> {
        self.authorization.external_approval().map(|a| a.approver())
    }
}

/// Why a permit could not be applied.
///
/// Kept for [`apply_with_permit`]'s defensive branch: a permit always carries
/// an authorization, so this should be unreachable through the public API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromotionPermitError {
    /// The permit's authorization does not cover a `CleanPromotionReady`
    /// dry-run. Defensive only.
    DryRunNotClean { actual: PromotionStatus },
}

/// Issue a [`PromotionPermit`] from a sealed [`PromotionAuthorization`].
///
/// Infallible: an authorization already proves the dry-run was
/// `CleanPromotionReady` and, for an automated author, that a verified external
/// approval was bound to the attempt. A `CleanPromotionReady` dry-run alone can
/// no longer mint a permit: the compiler requires the authorization value.
pub fn issue_promotion_permit(
    id: PromotionPermitId,
    authorization: PromotionAuthorization,
) -> PromotionPermit {
    let proposal = authorization.dry_run().lineage.proposal.clone();
    let issued_for_snapshot = authorization.dry_run().lineage.current_snapshot;
    PromotionPermit {
        id,
        proposal,
        issued_for_snapshot,
        authorization: Box::new(authorization),
    }
}

/// Outcome of an `apply_with_permit` call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromotionApplyOutcome {
    /// The permit was valid and the world had not drifted. The
    /// proposal is recorded as applied; the concrete mutation (which
    /// is outside M9) follows in a separate step. This variant
    /// carries the permit as the audit marker.
    Applied {
        permit: PromotionPermit,
        /// Snapshot id of the world the apply was performed against.
        applied_to_snapshot: SnapshotId,
    },
    /// The apply was rejected. No mutation has happened.
    Rejected(PromotionApplyError),
}

/// Why an apply was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromotionApplyError {
    /// No permit was supplied. Direct apply is never permitted.
    NoPermit,
    /// The supplied permit was not issued via
    /// [`issue_promotion_permit`] — it carries an invalid dry-run
    /// status. This should be unreachable through the public API
    /// (the constructor forbids it) but is included as a defensive
    /// variant.
    PermitInvalid { reason: PromotionPermitError },
    /// The current world's base_snapshot does not match the snapshot
    /// the permit was issued against. The world drifted; the permit
    /// is stale.
    WorldDriftedSincePermit {
        permit_snapshot: SnapshotId,
        current_snapshot: SnapshotId,
    },
}

/// Apply with an explicit permit.
///
/// Pure function. Validates the permit and the world state, then
/// returns a [`PromotionApplyOutcome`]. The actual mutation (writing
/// files, updating config) is the responsibility of a downstream
/// adapter; M9 establishes the authority and the audit marker.
///
/// Rules:
/// - If `permit` is structurally invalid (i.e. its dry-run status
///   is not `CleanPromotionReady`) → `Rejected(PermitInvalid)`.
/// - If the world's base_snapshot drifted since the permit was
///   issued → `Rejected(WorldDriftedSincePermit)`.
/// - Otherwise → `Applied { permit, applied_to_snapshot }`.
pub fn apply_with_permit(
    current: &SoftwareWorld,
    permit: &PromotionPermit,
) -> PromotionApplyOutcome {
    // Defensive: a permit must always carry CleanPromotionReady (the
    // constructor forbids anything else), but if a caller bypassed
    // the constructor, we still refuse.
    if permit.dry_run().status != PromotionStatus::CleanPromotionReady {
        return PromotionApplyOutcome::Rejected(PromotionApplyError::PermitInvalid {
            reason: PromotionPermitError::DryRunNotClean {
                actual: permit.dry_run().status.clone(),
            },
        });
    }
    if current.base_snapshot != permit.issued_for_snapshot {
        return PromotionApplyOutcome::Rejected(PromotionApplyError::WorldDriftedSincePermit {
            permit_snapshot: permit.issued_for_snapshot,
            current_snapshot: current.base_snapshot,
        });
    }
    PromotionApplyOutcome::Applied {
        permit: permit.clone(),
        applied_to_snapshot: current.base_snapshot,
    }
}

/// Helper: confirm a world's base_snapshot matches a permit. Returns
/// the snapshot id on success and an [`PromotionApplyError`] on
/// mismatch. Exposed for tests and downstream callers that want to
/// pre-validate before calling [`apply_with_permit`].
pub fn assert_world_matches_permit(
    current: &SoftwareWorld,
    permit: &PromotionPermit,
) -> Result<SnapshotId, PromotionApplyError> {
    if current.base_snapshot != permit.issued_for_snapshot {
        return Err(PromotionApplyError::WorldDriftedSincePermit {
            permit_snapshot: permit.issued_for_snapshot,
            current_snapshot: current.base_snapshot,
        });
    }
    Ok(current.base_snapshot)
}
