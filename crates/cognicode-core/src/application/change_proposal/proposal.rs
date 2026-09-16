//! ChangeProposal types (e72 WU1 — M9).
//!
//! See [`crate::application::change_proposal`] for the umbrella
//! rationale. This file defines the data shape only.

use serde::{Deserialize, Serialize};

use crate::application::software_world::world::SoftwareWorldId;

/// Stable identifier of a [`ChangeProposal`].
///
/// Caller-supplied; this module never mints ids at construction time,
/// in line with the pure / deterministic contract.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ChangeProposalId(String);

impl ChangeProposalId {
    /// Construct from any string-shaped identifier. Caller owns the
    /// scheme (UUID v4, ULID, monotonic counter — whatever the
    /// upstream wants).
    pub fn from_string(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// Borrow the underlying string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ChangeProposalId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// What kind of change is being proposed.
///
/// The envelope distinguishes three concrete flavours:
///
/// - `SourcePatch`: a source-file change (typically from an AI agent or
///   human editing the code).
/// - `ConfigChange`: a configuration change (e.g. extractor config,
///   policy thresholds).
/// - `DetectorChange`: a change to a detector definition.
///
/// New variants may be added; the order is part of the test surface
/// (see tests). Existing variants MUST NOT be removed (extend-never-
/// mutate), because persisted proposals carry the discriminant.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProposalKind {
    /// A source-file patch is proposed.
    SourcePatch {
        /// Logical identifier of the patch (e.g. commit SHA, patch id).
        patch_ref: String,
    },
    /// A configuration change is proposed.
    ConfigChange {
        /// Logical identifier of the config artefact being changed.
        config_ref: String,
    },
    /// A change to a detector definition is proposed.
    DetectorChange {
        /// Logical identifier of the detector being changed.
        detector_ref: String,
    },
}

impl ProposalKind {
    /// Short tag identifying the proposal flavour (for logs / audit).
    pub fn kind_tag(&self) -> &'static str {
        match self {
            ProposalKind::SourcePatch { .. } => "source-patch",
            ProposalKind::ConfigChange { .. } => "config-change",
            ProposalKind::DetectorChange { .. } => "detector-change",
        }
    }
}

/// Who or what authored the proposal.
///
/// The classification is essential for e73's adversarial gate: AI /
/// plugin authors must not be able to skip the trial + policy stages
/// by claiming "I authored the proposal, therefore I may apply it".
/// The author class is informational here; the authority decision
/// belongs to e73.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RequestedBy {
    /// A human curator.
    Human {
        /// Logical identifier of the human (e.g. username).
        user_ref: String,
    },
    /// A named plugin.
    Plugin {
        /// Plugin identifier.
        plugin_ref: String,
    },
    /// An LLM agent. Per ADR-040 / design D3, LLM output stays a
    /// hypothesis at the kernel level; here the proposal can exist as
    /// an intent to act, but promotion is a separate authority.
    LlmAgent {
        /// Logical identifier of the agent.
        agent_ref: String,
    },
}

impl RequestedBy {
    /// True iff the author is an automated producer (plugin or LLM).
    /// Used by e73 to enforce "AI/plugin asks for direct apply →
    /// reject".
    pub fn is_automated(&self) -> bool {
        matches!(
            self,
            RequestedBy::Plugin { .. } | RequestedBy::LlmAgent { .. }
        )
    }

    /// Short tag identifying the author class.
    pub fn class_tag(&self) -> &'static str {
        match self {
            RequestedBy::Human { .. } => "human",
            RequestedBy::Plugin { .. } => "plugin",
            RequestedBy::LlmAgent { .. } => "llm-agent",
        }
    }
}

/// Intent to apply a change against a derivation context.
///
/// The proposal does **not** grant authority to apply the change. The
/// shape is intentionally minimal: id, target world, the change
/// itself, and the author class. Authority is granted separately by
/// e73's [`PromotionPermit`](crate::application::promotion_authority::PromotionPermit).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChangeProposal {
    /// Stable identifier of this proposal.
    pub id: ChangeProposalId,
    /// The [`SoftwareWorldId`] the change targets. The proposal
    /// describes a change against this world's derivation context; it
    /// does not pick the candidate world — that is decided at trial
    /// time (e72 WU2).
    pub base_world: SoftwareWorldId,
    /// What kind of change is proposed.
    pub proposed_change: ProposalKind,
    /// Who or what authored the proposal.
    pub requested_by: RequestedBy,
}

impl ChangeProposal {
    /// Construct a proposal. Pure: no clock, no UUID mint.
    pub fn new(
        id: ChangeProposalId,
        base_world: SoftwareWorldId,
        proposed_change: ProposalKind,
        requested_by: RequestedBy,
    ) -> Self {
        Self {
            id,
            base_world,
            proposed_change,
            requested_by,
        }
    }

    /// True iff the proposal was authored by an automated producer
    /// (plugin or LLM). Convenience wrapper over
    /// [`RequestedBy::is_automated`].
    pub fn is_automated(&self) -> bool {
        self.requested_by.is_automated()
    }
}
