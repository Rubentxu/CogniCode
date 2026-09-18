//! Governed candidate binding and freeze (e83 — M13 task 13.5).
//!
//! ```text
//! GovernedCandidateBinding   this proposal + this base/candidate world + these
//!                            two analyzer revisions are ONE attempt
//!         ↓  (after OPTIMIZE)
//! CandidateFreeze            the candidate identity is now fixed
//! ```
//!
//! ## What the binding proves, and what it does not
//!
//! It proves **identity and lineage inside the governed workflow**: the proposal
//! targets this base world, the candidate was forked from it, and it was
//! measured against the same canonical base snapshot.
//!
//! It does **not** attest that arbitrary machine code really corresponds to the
//! analyzer descriptor. There is no trusted candidate-materialization or
//! binary-attestation layer, and e83 does not invent one. That remains a future
//! adapter seam.
//!
//! ## The freeze is not authority
//!
//! It proves only that a candidate identity was fixed after OPTIMIZE ran.

use crate::application::change_proposal::proposal::{
    ChangeProposal, ChangeProposalId, ProposalKind,
};
use crate::application::historical_replay::split::DatasetRole;
use crate::application::portable_execution::content_digest;
use crate::application::shadow_evaluation::analyzer::AnalyzerDescriptor;
use crate::application::shadow_evaluation::report::ShadowRoleReport;
use crate::application::software_world::world::{SoftwareWorld, SoftwareWorldId};

/// One governed-improvement attempt, bound to its proposal and worlds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GovernedCandidateBinding {
    proposal_id: ChangeProposalId,
    proposal_kind: ProposalKind,
    base_world: SoftwareWorldId,
    candidate_world: SoftwareWorldId,
    current: AnalyzerDescriptor,
    candidate: AnalyzerDescriptor,
    evaluation_digest: String,
}

impl GovernedCandidateBinding {
    /// Bind a proposal, its worlds, and both analyzer revisions.
    ///
    /// Validates:
    ///
    /// ```text
    /// proposal.base_world == base.id
    /// candidate.parent_world == Some(base.id)
    /// candidate.base_snapshot == base.base_snapshot
    /// ```
    pub fn bind(
        proposal: &ChangeProposal,
        base: &SoftwareWorld,
        candidate: &SoftwareWorld,
        current: AnalyzerDescriptor,
        candidate_descriptor: AnalyzerDescriptor,
        evaluation_digest: impl Into<String>,
    ) -> Result<Self, GovernedBindingError> {
        if proposal.base_world != base.id {
            return Err(GovernedBindingError::ProposalBaseWorldMismatch {
                proposal_world: proposal.base_world.clone(),
                base_world: base.id.clone(),
            });
        }
        if candidate.parent_world.as_ref() != Some(&base.id) {
            return Err(GovernedBindingError::CandidateNotDerivedFromBase {
                candidate_world: candidate.id.clone(),
                actual_parent: candidate.parent_world.clone(),
                base_world: base.id.clone(),
            });
        }
        if candidate.base_snapshot != base.base_snapshot {
            return Err(GovernedBindingError::CandidateBaseSnapshotMismatch {
                candidate_snapshot: candidate.base_snapshot,
                base_snapshot: base.base_snapshot,
            });
        }
        Ok(Self {
            proposal_id: proposal.id.clone(),
            proposal_kind: proposal.proposed_change.clone(),
            base_world: base.id.clone(),
            candidate_world: candidate.id.clone(),
            current,
            candidate: candidate_descriptor,
            evaluation_digest: evaluation_digest.into(),
        })
    }

    /// The proposal id.
    pub fn proposal_id(&self) -> &ChangeProposalId {
        &self.proposal_id
    }

    /// The proposal kind / artifact reference.
    pub fn proposal_kind(&self) -> &ProposalKind {
        &self.proposal_kind
    }

    /// The base world.
    pub fn base_world(&self) -> &SoftwareWorldId {
        &self.base_world
    }

    /// The candidate world.
    pub fn candidate_world(&self) -> &SoftwareWorldId {
        &self.candidate_world
    }

    /// The current analyzer revision.
    pub fn current_descriptor(&self) -> &AnalyzerDescriptor {
        &self.current
    }

    /// The candidate analyzer revision.
    pub fn candidate_descriptor(&self) -> &AnalyzerDescriptor {
        &self.candidate
    }

    /// The e82 evaluation digest this attempt is pinned to.
    pub fn evaluation_digest(&self) -> &str {
        &self.evaluation_digest
    }
}

/// Why a binding could not be constructed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GovernedBindingError {
    /// The proposal targets a different base world.
    ProposalBaseWorldMismatch {
        /// The proposal's target.
        proposal_world: SoftwareWorldId,
        /// The supplied base world.
        base_world: SoftwareWorldId,
    },
    /// The candidate world was not forked from the base.
    CandidateNotDerivedFromBase {
        /// The candidate world.
        candidate_world: SoftwareWorldId,
        /// Its actual parent, if any.
        actual_parent: Option<SoftwareWorldId>,
        /// The base it should have been forked from.
        base_world: SoftwareWorldId,
    },
    /// The candidate was measured against a different canonical base.
    CandidateBaseSnapshotMismatch {
        /// The candidate's base snapshot.
        candidate_snapshot: crate::domain::evidence_kernel::ids::SnapshotId,
        /// The base world's snapshot.
        base_snapshot: crate::domain::evidence_kernel::ids::SnapshotId,
    },
}

impl std::fmt::Display for GovernedBindingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ProposalBaseWorldMismatch {
                proposal_world,
                base_world,
            } => write!(
                f,
                "proposal targets world {proposal_world} but the base world is {base_world}"
            ),
            Self::CandidateNotDerivedFromBase {
                candidate_world,
                actual_parent,
                base_world,
            } => write!(
                f,
                "candidate {candidate_world} has parent {actual_parent:?}, not the base {base_world}"
            ),
            Self::CandidateBaseSnapshotMismatch {
                candidate_snapshot,
                base_snapshot,
            } => write!(
                f,
                "candidate base snapshot {candidate_snapshot:?} != base snapshot {base_snapshot:?}"
            ),
        }
    }
}

impl std::error::Error for GovernedBindingError {}

/// Private seal for [`CandidateFreeze`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FreezeSeal(());

/// A candidate identity frozen after OPTIMIZE.
///
/// Private fields, a private seal, and no serde: it cannot be fabricated by
/// literal or reconstructed from bytes. The only creation path is
/// [`CandidateFreeze::freeze`] over a binding and an OPTIMIZE report that agree.
///
/// This is **not** authority. It proves the candidate identity was fixed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateFreeze {
    proposal_id: ChangeProposalId,
    base_world: SoftwareWorldId,
    candidate_world: SoftwareWorldId,
    current: AnalyzerDescriptor,
    candidate: AnalyzerDescriptor,
    evaluation_digest: String,
    optimize_report_digest: String,
    _seal: FreezeSeal,
}

impl CandidateFreeze {
    /// Freeze the candidate after OPTIMIZE.
    ///
    /// The OPTIMIZE report must be for the `Optimize` role, must carry the same
    /// analyzer descriptors as the binding, and must belong to the same
    /// evaluation digest.
    pub fn freeze(
        binding: &GovernedCandidateBinding,
        optimize: &ShadowRoleReport,
    ) -> Result<Self, FreezeError> {
        if optimize.role() != DatasetRole::Optimize {
            return Err(FreezeError::WrongDatasetRole {
                expected: DatasetRole::Optimize,
                actual: optimize.role(),
            });
        }
        if optimize.current_descriptor() != binding.current_descriptor() {
            return Err(FreezeError::CurrentAnalyzerMismatch);
        }
        if optimize.candidate_descriptor() != binding.candidate_descriptor() {
            return Err(FreezeError::CandidateAnalyzerMismatch);
        }
        if optimize.evaluation_digest() != binding.evaluation_digest() {
            return Err(FreezeError::EvaluationDigestMismatch {
                expected: binding.evaluation_digest().to_string(),
                actual: optimize.evaluation_digest().to_string(),
            });
        }
        Ok(Self {
            proposal_id: binding.proposal_id().clone(),
            base_world: binding.base_world().clone(),
            candidate_world: binding.candidate_world().clone(),
            current: binding.current_descriptor().clone(),
            candidate: binding.candidate_descriptor().clone(),
            evaluation_digest: binding.evaluation_digest().to_string(),
            optimize_report_digest: report_digest(optimize),
            _seal: FreezeSeal(()),
        })
    }

    /// The frozen proposal id.
    pub fn proposal_id(&self) -> &ChangeProposalId {
        &self.proposal_id
    }

    /// The frozen base world.
    pub fn base_world(&self) -> &SoftwareWorldId {
        &self.base_world
    }

    /// The frozen candidate world.
    pub fn candidate_world(&self) -> &SoftwareWorldId {
        &self.candidate_world
    }

    /// The frozen current analyzer revision.
    pub fn current_descriptor(&self) -> &AnalyzerDescriptor {
        &self.current
    }

    /// The frozen candidate analyzer revision.
    pub fn candidate_descriptor(&self) -> &AnalyzerDescriptor {
        &self.candidate
    }

    /// The frozen evaluation digest.
    pub fn evaluation_digest(&self) -> &str {
        &self.evaluation_digest
    }

    /// Audit digest of the OPTIMIZE report that justified the freeze.
    pub fn optimize_report_digest(&self) -> &str {
        &self.optimize_report_digest
    }
}

/// Why a candidate could not be frozen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FreezeError {
    /// The report was not an OPTIMIZE report.
    WrongDatasetRole {
        /// The role required.
        expected: DatasetRole,
        /// The role supplied.
        actual: DatasetRole,
    },
    /// The report's current analyzer revision differs from the binding.
    CurrentAnalyzerMismatch,
    /// The report's candidate analyzer revision differs from the binding.
    CandidateAnalyzerMismatch,
    /// The report belongs to a different evaluation.
    EvaluationDigestMismatch {
        /// The binding's digest.
        expected: String,
        /// The report's digest.
        actual: String,
    },
}

impl std::fmt::Display for FreezeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WrongDatasetRole { expected, actual } => write!(
                f,
                "expected an {} report, got {}",
                expected.name(),
                actual.name()
            ),
            Self::CurrentAnalyzerMismatch => {
                f.write_str("report current analyzer does not match the binding")
            }
            Self::CandidateAnalyzerMismatch => {
                f.write_str("report candidate analyzer does not match the binding")
            }
            Self::EvaluationDigestMismatch { expected, actual } => write!(
                f,
                "evaluation digest mismatch: binding {expected}, report {actual}"
            ),
        }
    }
}

impl std::error::Error for FreezeError {}

/// Deterministic digest over a role report's semantic content.
pub fn report_digest(report: &ShadowRoleReport) -> String {
    let mut canonical = String::new();
    canonical.push_str("shadow-role-report.v1\n");
    canonical.push_str(report.role().name());
    canonical.push('\n');
    canonical.push_str(report.evaluation_digest());
    canonical.push('\n');
    canonical.push_str(report.current_descriptor().analyzer_id());
    canonical.push('\n');
    canonical.push_str(report.current_descriptor().revision_digest());
    canonical.push('\n');
    canonical.push_str(report.candidate_descriptor().analyzer_id());
    canonical.push('\n');
    canonical.push_str(report.candidate_descriptor().revision_digest());
    canonical.push('\n');
    for case in report.case_comparisons() {
        canonical.push_str(case.case_id.as_str());
        canonical.push('\n');
        canonical.push_str(&format!("{:?}", case.current));
        canonical.push('\n');
        canonical.push_str(&format!("{:?}", case.candidate));
        canonical.push('\n');
    }
    content_digest(canonical.as_bytes()).as_str().to_string()
}
