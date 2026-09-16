//! Fact bridge (E37 WU-2) — deterministic adapters that turn extractor
//! output and code-intelligence observations into canonical kernel `Fact`s.
//!
//! Everything here is feature-gated behind `evidence-kernel` (off by
//! default). Adapters never talk to a store: they feed a
//! [`FactBatchBuilder`], whose `finish()` canonicalizes the batch (design
//! D3) so the caller commits one deterministic fact set per snapshot.
//!
//! # Canonical subject grammar (E38.1 CP-3)
//!
//! Every observation SUBJECT that denotes a symbol is that symbol's
//! 1-BASED fact-side FQN `"{file}:{name}:{line}"` (typed grammar:
//! `domain::evidence_kernel::SymbolFqn::from_fact_side`), byte-identical to
//! the `core:defines` fact the DeterministicAnalyzer emits for the same
//! symbol. RuntimeObserver observations therefore JOIN the tree-sitter
//! entities in the snapshot view:
//!
//! - [`tree_sitter_facts`] subjects are symbol FQNs (defines/calls/
//!   references) or file paths (contains/imports) — the fact-side grammar.
//! - [`lsp_facts`] container references resolve the reported container
//!   name against the extraction context (the `get_symbols` symbols of the
//!   walked files): exact name match, `SymbolKind::File` excluded,
//!   duplicate names tie-break on the lexicographically smallest FQN (the
//!   shared-resolver rule). A resolved container becomes the enclosing
//!   symbol's 1-based fact-side FQN.
//! - [`lsp_facts`] hierarchy subjects are the queried symbol itself —
//!   always resolvable, so always the fact-side FQN.
//! - DETERMINISTIC FALLBACK (never invented entities): when no container
//!   is reported, or the container matches no extraction-context symbol,
//!   the subject is the reference site's FILE PATH. File paths are never
//!   `core:defines` subjects, so a fallback subject can never fabricate or
//!   mis-join an entity — it stays a documented non-joinable subject form.
//!
//! The `Symbol` aggregate stores the ZERO-BASED tree-sitter `start.row`
//! (legacy-side grammar, `SymbolFqn::from_legacy_side`); the extraction
//! context re-bases it onto the fact side with the declared `+1` step —
//! the same convention alignment the equivalence harness normalizes
//! (`normalize_legacy_fqn`).
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
pub mod production_grounding;
pub mod tree_sitter_facts;

use crate::domain::evidence_kernel::relation::RelationKind;

pub use batch_builder::{FactBatchBuilder, UnresolvedRecord};
pub use entity_table::EntityIdTable;

/// Errors raised while assembling a fact batch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum FactBridgeError {
    /// LLM-agent output cannot be recorded as an extracted-fact
    /// observation (design D3 / ADR-040): it stays a Hypothesis or
    /// AgentEvidence and is rejected before it can enter a batch.
    #[error("LLM-agent output cannot be recorded as an extracted-fact observation")]
    LlmProvenance,
    /// An observation's provenance class contradicts the serving precision
    /// tier it declares (design D4, spec `provider-tier-provenance`):
    /// e.g. a tree-sitter (`S0`) heuristic stamped `Extracted` instead of
    /// `Ambiguous`. Batch construction fails rather than silently accepting
    /// the mis-attributed fact.
    #[error("provenance class contradicts the observation's declared precision tier")]
    TierProvenanceContradiction,
}

/// Constructs a canonical `core:*` relation kind.
///
/// Only compile-time-constant canonical names reach this helper; a
/// malformed shape is a programming error, not a runtime condition.
fn relation(name: &'static str) -> RelationKind {
    RelationKind::try_new(name).expect("canonical predicate is a valid 'ns:name'")
}
