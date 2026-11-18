//! The context pack: the symbols that answer a request, with the evidence
//! that backs each one and a budget that holds.
//!
//! See [`service::ContextPackService`] for the contract. The module reads
//! through ports and owns no index, no store and no graph: a pack is an
//! ephemeral projection over the frozen snapshot it is given.

pub mod measure;
pub mod ports;
pub mod render;
pub mod selection;
pub mod service;

#[cfg(test)]
mod tests;

pub use measure::{LexicalV1, MeasureKind, Measured, TokenMeasure, Utf8UpperBound, measure_named};
pub use ports::{
    DeclarationReader, DeclarationSpan, Ranked, Relevance, SnapshotView, SourceReader,
};
pub use selection::{
    Ambiguity, Candidate, ContextSelection, MAX_CANDIDATES, Reason, SelectionRequest, select,
};
pub use service::{
    AmbiguousAnchor, BudgetReport, ContextPack, ContextPackService, OMISSIONS_LISTED, Omission,
    PackError, PackItem, PackRequest, SourceRange,
};
