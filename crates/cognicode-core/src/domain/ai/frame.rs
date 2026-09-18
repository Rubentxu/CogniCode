//! M11 (AI Foundation, cycle e79) — read-only AI support.
//!
//! The domain vocabulary for an AI investigation. The shape is bounded
//! by design:
//!
//! - The frame says **what** the agent was allowed to inspect, **what**
//!   it actually read, and **what question** it is answering.
//! - It references existing identities (`FactId`, `EvidenceId`,
//!   `ArchitectureConstraintId`, `ReadSet`) rather than copying
//!   knowledge.
//! - It carries a budget and a digest. The digest is the
//!   `InvestigationFrame::content_digest()` and is what the `LlmPort`
//!   records as the `request_identity` of the response.
//!
//! Critical invariants:
//!
//! - `InvestigationFrame` is **immutable**: every field is private and
//!   there is no public mutation API. Frames are constructed once and
//!   passed by reference.
//! - The frame **does not embed** canonical facts, evidence, or graph
//!   nodes. It references them by id.
//! - `Hypothesis` is **not** a `Fact`, **not** a `Finding` authority,
//!   and **not** an `Evidence` proof. It is an advisory observation
//!   the platform may or may not route into a downstream record.
//!
//! These invariants are enforced by the types, not by convention:
//! the public surface offers no constructor that could violate them.

use serde::{Deserialize, Serialize};

use crate::domain::architecture::ArchitectureConstraintId;
use crate::domain::execution::ExecutionContext;
use crate::domain::findings::{FindingError, FindingId};
use crate::domain::kernel_ids::{EntityId, EvidenceId, FactId, SnapshotId};
use crate::domain::readset::{InMemoryReadSetRecorder, ReadSet, ReadSetConfig};
use crate::domain::value_objects::WorkspaceId;

// ============================================================================
// InvestigationFrameId
// ============================================================================

/// Identifies one concrete `InvestigationFrame`.
///
/// Constructed deterministically from the frame's content digest via
/// [`Self::from_content_digest`]. Two frames with the same content have
/// the same id. The id is a `u64` for the same reasons other kernel ids
/// are `u64`: cheap to compare, cheap to log, sufficient for dedup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct InvestigationFrameId(pub u64);

impl InvestigationFrameId {
    /// Construct from a content digest (`u64`). The digest is whatever
    /// stable hash the caller computes over the frame's content; this
    /// type only stores the value.
    pub const fn from_content_digest(digest: u64) -> Self {
        Self(digest)
    }

    /// The raw value.
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for InvestigationFrameId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "frame:{}", self.0)
    }
}

// ============================================================================
// InvestigationBudget
// ============================================================================

/// What an investigation may consume.
///
/// The budget is **declarative** — it does not enforce itself. The
/// `LlmPort` and the runtime are expected to read it and obey it; the
/// type system does not prevent an over-budget call from being made
/// (because there is no live I/O in the domain). What the type system
/// *does* prevent is a budget that exceeds `u32::MAX` worth of anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvestigationBudget {
    /// Maximum number of tool/read calls the agent may make.
    pub max_tool_calls: u32,
    /// Maximum bytes of structured context the request may carry.
    pub max_context_bytes: u32,
    /// Maximum bytes of textual output the response may carry.
    pub max_response_bytes: u32,
}

impl Default for InvestigationBudget {
    /// A small default. Tests and the deterministic fake use this.
    fn default() -> Self {
        Self {
            max_tool_calls: 16,
            max_context_bytes: 32 * 1024,
            max_response_bytes: 4 * 1024,
        }
    }
}

impl InvestigationBudget {
    /// A bounded budget.
    pub fn new(max_tool_calls: u32, max_context_bytes: u32, max_response_bytes: u32) -> Self {
        Self {
            max_tool_calls,
            max_context_bytes,
            max_response_bytes,
        }
    }
}

// ============================================================================
// InvestigationScope
// ============================================================================

/// What the agent was allowed to inspect.
///
/// All ids in the scope are *declared intent*: the agent's prompt may
/// see these references but no others. The actual reads are recorded
/// by `LlmResponse.observed_read_set`.
///
/// Not `Serialize`/`Deserialize`: the `ReadSet` field is intentionally
/// not serde-serializable. Construct via `InvestigationScope::empty`
/// or reconstruct via `InMemoryReadSetRecorder`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvestigationScope {
    /// The workspace the investigation is anchored to.
    pub workspace: WorkspaceId,
    /// Snapshot id within the workspace. Must be a real snapshot
    /// (the same restriction `ExecutionContext` enforces — `None`
    /// is not allowed for a real investigation).
    pub snapshot: SnapshotId,
    /// Optional list of canonical references the agent may ground
    /// its answer in. Empty means "may inspect all canonical
    /// knowledge visible at this snapshot" — but the agent MUST
    /// declare which it used (via `LlmResponse.observed_read_set`).
    pub facts: Vec<FactId>,
    pub evidence: Vec<EvidenceId>,
    pub entities: Vec<EntityId>,
    pub findings: Vec<FindingId>,
    pub architecture_constraints: Vec<ArchitectureConstraintId>,
    /// The read set the agent *intends* to consult. Declared up front;
    /// the actual reads (`LlmResponse.observed_read_set`) must be a
    /// subset.
    pub declared_read_set: ReadSet,
}

impl InvestigationScope {
    /// Construct an empty scope (no declared references). The agent
    /// may not, by default, see any canonical knowledge; it can only
    /// answer from the question itself.
    pub fn empty(workspace: WorkspaceId, snapshot: SnapshotId) -> Self {
        let recorder = InMemoryReadSetRecorder::new(ReadSetConfig { max_records: None });
        let declared_read_set = recorder.finalize();
        Self {
            workspace,
            snapshot,
            facts: Vec::new(),
            evidence: Vec::new(),
            entities: Vec::new(),
            findings: Vec::new(),
            architecture_constraints: Vec::new(),
            declared_read_set,
        }
    }

    /// The number of declared canonical references.
    pub fn declared_ref_count(&self) -> usize {
        self.facts.len()
            + self.evidence.len()
            + self.entities.len()
            + self.findings.len()
            + self.architecture_constraints.len()
    }

    /// Whether the scope names any canonical reference.
    pub fn has_declarations(&self) -> bool {
        self.declared_ref_count() > 0 || !self.declared_read_set.is_empty()
    }
}

// ============================================================================
// InvestigationFrame
// ============================================================================

/// A bounded immutable input to one AI investigation.
///
/// Constructed via [`InvestigationFrame::try_new`] which rejects:
/// - unanchored scopes (workspace must be non-empty, snapshot must be a
///   real snapshot),
/// - an unpinned execution context,
/// - empty questions (the agent must have an objective).
///
/// The frame is read-only after construction. The `content_digest`
/// method computes a stable digest over every declared field; this
/// is the value the `LlmPort` records as the request identity of the
/// response, so the lineage trail is reconstructable from the response
/// alone.
///
/// Note: `InvestigationFrame` is NOT `Serialize`/`Deserialize` because
/// it embeds a `ReadSet` (the canonical lineage model), which is
/// intentionally not serde-serializable to keep the canonical store
/// the single source of truth. The frame can be reconstructed via the
/// `try_new` constructor + `InMemoryReadSetRecorder` if needed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvestigationFrame {
    id: InvestigationFrameId,
    execution: ExecutionContext,
    question: String,
    scope: InvestigationScope,
    budget: InvestigationBudget,
}

impl InvestigationFrame {
    /// Construct a frame, rejecting unanchored scopes and empty
    /// questions.
    pub fn try_new(
        execution: ExecutionContext,
        question: impl Into<String>,
        scope: InvestigationScope,
        budget: InvestigationBudget,
    ) -> Result<Self, InvestigationFrameError> {
        if !execution.is_valid() {
            return Err(InvestigationFrameError::InvalidExecutionContext);
        }
        if scope.workspace.as_str().is_empty() {
            return Err(InvestigationFrameError::UnanchoredWorkspace);
        }
        if scope.snapshot == SnapshotId::NONE {
            return Err(InvestigationFrameError::InvalidSnapshot);
        }
        let q = question.into();
        if q.trim().is_empty() {
            return Err(InvestigationFrameError::EmptyQuestion);
        }
        let id = Self::compute_id(&execution, &q, &scope, budget);
        Ok(Self {
            id,
            execution,
            question: q,
            scope,
            budget,
        })
    }

    /// The frame id (deterministic from content).
    pub fn id(&self) -> InvestigationFrameId {
        self.id
    }

    /// The execution this frame belongs to.
    pub fn execution(&self) -> &ExecutionContext {
        &self.execution
    }

    /// The bounded question.
    pub fn question(&self) -> &str {
        &self.question
    }

    /// The scope of canonical references the agent is allowed to see.
    pub fn scope(&self) -> &InvestigationScope {
        &self.scope
    }

    /// The declarative budget.
    pub fn budget(&self) -> InvestigationBudget {
        self.budget
    }

    /// Stable content digest over the frame's inputs. This is the
    /// value the `LlmPort` records as `request_identity` so the
    /// response can be paired with the request even if the frame
    /// object itself is later dropped.
    pub fn content_digest(&self) -> u64 {
        let mut h: u64 = 0xcbf29ce484222325; // FNV-1a 64 offset basis
        let prime: u64 = 0x100000001b3;
        let mix = |acc: &mut u64, bytes: &[u8]| {
            for b in bytes {
                *acc ^= *b as u64;
                *acc = acc.wrapping_mul(prime);
            }
        };
        mix(&mut h, &self.execution.execution_id.get().to_le_bytes());
        mix(&mut h, self.execution.scope.workspace.as_str().as_bytes());
        mix(&mut h, &self.execution.scope.snapshot.get().to_le_bytes());
        mix(&mut h, self.question.as_bytes());
        for f in &self.scope.facts {
            mix(&mut h, &f.get().to_le_bytes());
        }
        for e in &self.scope.evidence {
            mix(&mut h, &e.get().to_le_bytes());
        }
        for ent in &self.scope.entities {
            mix(&mut h, &ent.get().to_le_bytes());
        }
        for fi in &self.scope.findings {
            mix(&mut h, fi.as_str().as_bytes());
        }
        for ac in &self.scope.architecture_constraints {
            mix(&mut h, ac.as_str().as_bytes());
        }
        mix(&mut h, &self.budget.max_tool_calls.to_le_bytes());
        mix(&mut h, &self.budget.max_context_bytes.to_le_bytes());
        mix(&mut h, &self.budget.max_response_bytes.to_le_bytes());
        h
    }

    fn compute_id(
        execution: &ExecutionContext,
        question: &str,
        scope: &InvestigationScope,
        budget: InvestigationBudget,
    ) -> InvestigationFrameId {
        let mut h: u64 = 0xcbf29ce484222325;
        let prime: u64 = 0x100000001b3;
        let mix = |acc: &mut u64, bytes: &[u8]| {
            for b in bytes {
                *acc ^= *b as u64;
                *acc = acc.wrapping_mul(prime);
            }
        };
        mix(&mut h, &execution.execution_id.get().to_le_bytes());
        mix(&mut h, execution.scope.workspace.as_str().as_bytes());
        mix(&mut h, &execution.scope.snapshot.get().to_le_bytes());
        mix(&mut h, question.as_bytes());
        for f in &scope.facts {
            mix(&mut h, &f.get().to_le_bytes());
        }
        for e in &scope.evidence {
            mix(&mut h, &e.get().to_le_bytes());
        }
        for ent in &scope.entities {
            mix(&mut h, &ent.get().to_le_bytes());
        }
        for fi in &scope.findings {
            mix(&mut h, fi.as_str().as_bytes());
        }
        for ac in &scope.architecture_constraints {
            mix(&mut h, ac.as_str().as_bytes());
        }
        mix(&mut h, &budget.max_tool_calls.to_le_bytes());
        mix(&mut h, &budget.max_context_bytes.to_le_bytes());
        mix(&mut h, &budget.max_response_bytes.to_le_bytes());
        InvestigationFrameId::from_content_digest(h)
    }

    /// Convenience for tests / fake adapters: build a frame with the
    /// given finding id list. Returns the first error encountered.
    pub fn findings_iter(&self) -> impl Iterator<Item = &FindingId> {
        self.scope.findings.iter()
    }
}

/// Why a frame could not be constructed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvestigationFrameError {
    /// The execution context is not valid (e.g. unpinned scope).
    InvalidExecutionContext,
    /// The workspace is empty.
    UnanchoredWorkspace,
    /// The snapshot is the invalid sentinel.
    InvalidSnapshot,
    /// The question is empty or whitespace.
    EmptyQuestion,
    /// A finding id is empty.
    InvalidFindingId(FindingError),
}

impl std::fmt::Display for InvestigationFrameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidExecutionContext => f.write_str("execution context is not valid"),
            Self::UnanchoredWorkspace => f.write_str("workspace id is empty"),
            Self::InvalidSnapshot => f.write_str("snapshot is the invalid sentinel"),
            Self::EmptyQuestion => f.write_str("investigation question is empty"),
            Self::InvalidFindingId(e) => write!(f, "invalid finding id: {e}"),
        }
    }
}

impl std::error::Error for InvestigationFrameError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::execution::{AnalysisScope, CorrelationId};
    use crate::domain::kernel_ids::{ExecutionId, SnapshotId};
    use crate::domain::value_objects::WorkspaceId;

    fn ctx() -> ExecutionContext {
        ExecutionContext::try_new(
            ExecutionId::new(1),
            AnalysisScope::new(WorkspaceId::try_new("ws").unwrap(), SnapshotId::new(7)),
            crate::domain::execution::ActorRef::agent("explorer"),
            CorrelationId::new("c-1").unwrap(),
            None,
        )
        .unwrap()
    }

    fn scope() -> InvestigationScope {
        InvestigationScope::empty(WorkspaceId::try_new("ws").unwrap(), SnapshotId::new(7))
    }

    #[test]
    fn a_well_formed_frame_is_accepted() {
        let f = InvestigationFrame::try_new(
            ctx(),
            "what architectural constraints should this snapshot have?",
            scope(),
            InvestigationBudget::default(),
        )
        .unwrap();
        assert!(f.id().get() != 0);
        assert_eq!(
            f.question(),
            "what architectural constraints should this snapshot have?"
        );
        assert_eq!(f.scope().workspace.as_str(), "ws");
        assert_eq!(f.scope().snapshot.get(), 7);
    }

    #[test]
    fn an_empty_question_is_rejected() {
        let err =
            InvestigationFrame::try_new(ctx(), "   ", scope(), InvestigationBudget::default())
                .unwrap_err();
        assert_eq!(err, InvestigationFrameError::EmptyQuestion);
    }

    #[test]
    fn an_invalid_snapshot_is_rejected() {
        let s = InvestigationScope::empty(WorkspaceId::try_new("ws").unwrap(), SnapshotId::NONE);
        let err =
            InvestigationFrame::try_new(ctx(), "q", s, InvestigationBudget::default()).unwrap_err();
        assert_eq!(err, InvestigationFrameError::InvalidSnapshot);
    }

    #[test]
    fn the_content_digest_is_stable() {
        let f1 = InvestigationFrame::try_new(
            ctx(),
            "same question",
            scope(),
            InvestigationBudget::default(),
        )
        .unwrap();
        let f2 = InvestigationFrame::try_new(
            ctx(),
            "same question",
            scope(),
            InvestigationBudget::default(),
        )
        .unwrap();
        assert_eq!(f1.content_digest(), f2.content_digest());
        assert_eq!(f1.id(), f2.id());
    }

    #[test]
    fn the_content_digest_changes_with_the_question() {
        let f1 = InvestigationFrame::try_new(
            ctx(),
            "question one",
            scope(),
            InvestigationBudget::default(),
        )
        .unwrap();
        let f2 = InvestigationFrame::try_new(
            ctx(),
            "question two",
            scope(),
            InvestigationBudget::default(),
        )
        .unwrap();
        assert_ne!(f1.content_digest(), f2.content_digest());
        assert_ne!(f1.id(), f2.id());
    }

    #[test]
    fn declared_ref_count_is_zero_for_an_empty_scope() {
        let s = scope();
        assert_eq!(s.declared_ref_count(), 0);
        assert!(!s.has_declarations());
    }

    #[test]
    fn a_scope_with_findings_reports_the_count() {
        let mut s = scope();
        s.findings.push(FindingId::new("f-1").unwrap());
        s.findings.push(FindingId::new("f-2").unwrap());
        assert_eq!(s.declared_ref_count(), 2);
        assert!(s.has_declarations());
    }
}
