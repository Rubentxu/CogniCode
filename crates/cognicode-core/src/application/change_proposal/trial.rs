//! Trial assembly (e72 WU2 — M9).
//!
//! See [`crate::application::change_proposal`] for the umbrella
//! rationale. This module defines:
//!
//! - [`TrialId`]: stable identifier of a trial run.
//! - [`TrialInput`]: the inputs to a trial (proposal, world, snapshots,
//!   facts, work results, evidence bundle, gate decision).
//! - [`TrialEvidence`]: the assembled lineage-labelled envelope.
//! - [`assemble_trial_evidence`]: pure function that composes the
//!   envelope.
//!
//! ## What this module does NOT do
//!
//! - It does not compute a verdict. The verdict lives in
//!   [`GateDecision`] from e69.
//! - It does not recompute the diff. The diff lives in e68.
//! - It does not run the work. The work ran during the trial; this
//!   module receives the [`PerWorkReport`]s.
//! - It does not mint timestamps. Audit timing belongs to e73's
//!   promotion record.
//!
//! This is **composition only**.

use crate::application::change_proposal::proposal::{ChangeProposal, ChangeProposalId};
use crate::application::evidence_bundle::EvidenceBundle;
use crate::application::local_ci::PerWorkReport;
use crate::application::policy_gate::PolicyDecision;
use crate::application::software_world::world::{SoftwareWorld, SoftwareWorldId};
use crate::domain::evidence_kernel::fact::Fact;
use crate::domain::evidence_kernel::ids::SnapshotId;

/// Stable identifier of a trial run.
///
/// Caller-supplied; this module never mints ids at construction time.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TrialId(String);

impl TrialId {
    /// Construct from any string-shaped identifier.
    pub fn from_string(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// Borrow the underlying string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for TrialId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Input envelope for a trial run.
///
/// The trial is a pure composition; the caller has already extracted
/// the facts, run the work, and computed the evidence bundle + gate
/// decision. The trial's only job is to assemble these pieces under
/// one lineage header.
#[derive(Debug, Clone)]
pub struct TrialInput {
    /// The proposal being evaluated. Its id is carried forward into
    /// the [`TrialEvidence`] for lineage.
    pub proposal: ChangeProposal,
    /// The candidate world under evaluation.
    pub world: SoftwareWorld,
    /// Snapshot of the canonical base (the world the candidate is
    /// measured against).
    pub base_snapshot: SnapshotId,
    /// Snapshot of the candidate (the world after the proposed change
    /// has been applied to the source).
    pub candidate_snapshot: SnapshotId,
    /// Facts observed against the candidate's snapshot. These may be
    /// empty for trials that only inspect source-level changes (e.g.
    /// a config-only proposal).
    pub candidate_facts: Vec<Fact>,
    /// Work results from running the affected-work plan over the
    /// candidate.
    pub work_results: Vec<PerWorkReport>,
    /// Evidence bundle assembled during the trial.
    pub evidence_bundle: EvidenceBundle,
    /// Gate decision produced by e69's PolicyGate over the bundle.
    pub gate: PolicyDecision,
}

/// Lineage-labelled envelope of a trial run.
///
/// This is what e73's promotion evaluation reads. It carries:
///
/// - The trial's identity and the proposal it evaluated.
/// - The world under evaluation.
/// - The two snapshots (base and candidate).
/// - The candidate's facts.
/// - The work results.
/// - The evidence bundle.
/// - The gate decision.
///
/// **No separate verdict model.** The verdict is the gate decision.
/// The trial just carries it.
#[derive(Debug, Clone, PartialEq)]
pub struct TrialEvidence {
    /// Stable identifier of this trial.
    pub trial_id: TrialId,
    /// Proposal that was evaluated.
    pub proposal_id: ChangeProposalId,
    /// World under evaluation.
    pub world_id: SoftwareWorldId,
    /// Base snapshot.
    pub base_snapshot: SnapshotId,
    /// Candidate snapshot.
    pub candidate_snapshot: SnapshotId,
    /// Facts observed against the candidate.
    pub candidate_facts: Vec<Fact>,
    /// Work results from the affected-work plan.
    pub work_results: Vec<PerWorkReport>,
    /// Evidence bundle.
    pub evidence_bundle: EvidenceBundle,
    /// Gate decision (the verdict).
    pub gate: PolicyDecision,
}

/// Pure composition: assemble a [`TrialEvidence`] from a [`TrialInput`]
/// and a caller-supplied [`TrialId`].
///
/// This is the entire "executor" responsibility for WU2: there is no
/// effectful behaviour, no clock access, no graph store. The function
/// is total and deterministic; the only inputs are the proposal, the
/// world, the snapshots, the facts, the work results, the bundle, the
/// gate, and the trial id.
pub fn assemble_trial_evidence(trial_id: TrialId, input: TrialInput) -> TrialEvidence {
    TrialEvidence {
        trial_id,
        proposal_id: input.proposal.id,
        world_id: input.world.id,
        base_snapshot: input.base_snapshot,
        candidate_snapshot: input.candidate_snapshot,
        candidate_facts: input.candidate_facts,
        work_results: input.work_results,
        evidence_bundle: input.evidence_bundle,
        gate: input.gate,
    }
}
