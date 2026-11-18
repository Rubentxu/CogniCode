//! The context pack: the symbols that answer a request, with the evidence
//! that backs each one and a budget that holds.
//!
//! See [`service::ContextPackService`] for the contract. The module reads
//! through ports and owns no index, no store and no graph: a pack is an
//! ephemeral projection over the frozen snapshot it is given.

pub mod adaptive;
pub mod change;
pub mod detail;
pub mod expansion;
pub mod measure;
pub mod policy;
pub mod ports;
pub mod render;
pub mod rerank;
pub mod selection;
pub mod service;

#[cfg(test)]
mod tests;

pub use adaptive::{Adaptive, AdaptiveReport};
pub use detail::{Representation, Segment};
pub use expansion::{
    DEFAULT_MAX_VISITS, ExpansionReport, ExpansionStatus, Interrupt, NoInterrupt, Stop, WorkBudget,
};
pub use measure::{LexicalV1, MeasureKind, Measured, TokenMeasure, Utf8UpperBound, measure_named};
pub use policy::{Detail, Expansion, POLICY_VERSION, Purpose, Role, SelectionPolicy, Weights};
pub use ports::{
    DeclarationReader, DeclarationSpan, NoRelevance, Ranked, Relevance, SnapshotView, SourceReader,
};
pub use selection::{
    Ambiguity, Candidate, ContextSelection, MAX_CANDIDATES, Reason, SelectionRequest, select,
};
pub use service::{
    AmbiguousAnchor, BudgetReport, ContextPack, ContextPackError, ContextPackService, ItemRange,
    ItemSelection, OMISSIONS_LISTED, Omission, PackItem, PackRequest,
};
