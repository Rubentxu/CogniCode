//! Fact bridge (E37 WU-2) — deterministic adapters that turn extractor
//! output and code-intelligence observations into canonical kernel `Fact`s.
//!
//! Everything here is feature-gated behind `evidence-kernel` (off by
//! default). Adapters never talk to a store: they feed a
//! [`FactBatchBuilder`], whose `finish()` canonicalizes the batch (design
//! D3) so the caller commits one deterministic fact set per snapshot.
//!
//! Module map:
//!
//! - [`entity_table`] — snapshot-scoped sorted `EntityId(1..N)` assignment
//!   (no hashing; design D3).
//! - [`batch_builder`] — canonical batch assembly (`FactBatchBuilder`).
//! - [`tree_sitter_facts`] — `extract_file` output adapter (producer
//!   `DeterministicAnalyzer`).
//! - [`lsp_facts`] — `CodeIntelligenceProvider` observation adapter
//!   (producer `RuntimeObserver`).

pub mod batch_builder;
pub mod entity_table;
pub mod lsp_facts;
pub mod tree_sitter_facts;

use crate::domain::evidence_kernel::relation::RelationKind;

pub use batch_builder::FactBatchBuilder;
pub use entity_table::EntityIdTable;

/// Errors raised while assembling a fact batch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum FactBridgeError {
    /// LLM-agent output cannot be recorded as an extracted-fact
    /// observation (design D3 / ADR-040): it stays a Hypothesis or
    /// AgentEvidence and is rejected before it can enter a batch.
    #[error("LLM-agent output cannot be recorded as an extracted-fact observation")]
    LlmProvenance,
}

/// Constructs a canonical `core:*` relation kind.
///
/// Only compile-time-constant canonical names reach this helper; a
/// malformed shape is a programming error, not a runtime condition.
fn relation(name: &'static str) -> RelationKind {
    RelationKind::try_new(name).expect("canonical predicate is a valid 'ns:name'")
}
