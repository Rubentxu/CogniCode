//! CogniCode Explorer.
//!
//! Moldable code exploration layer for Spotter search, inspectable objects,
//! contextual views, evidence blocks, exploration paths, and decision artifacts.

pub mod adapters;
pub mod api;
pub mod domain;
pub mod dto;
pub mod error;
pub mod mcp;
pub mod moldql;
pub mod ports;
pub mod service;

pub use domain::lens::{Lens, LensContext, LensRegistry};
pub use domain::ObjectIdentity;
pub use dto::{
    DesignFinding, FindingSeverity, LensDescriptor, LensResult,
};
pub use error::{ExplorerError, ExplorerResult};
pub use mcp::ExplorerMcpHandler;
pub use moldql::{MoldQLExecutor, MoldQLItem, MoldQLResult, MoldQLQuery, ParseError};
pub use ports::{
    QualityGateSummary, QualityIssue, QualityRepository, RelationTarget, ResolvedSymbol, RuleSummary,
    SearchHit, SearchRepository, SourceReader, SymbolRepository,
};
