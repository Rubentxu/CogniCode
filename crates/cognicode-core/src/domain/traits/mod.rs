//! Domain traits module
//!
//! Traits that define the contracts for domain operations.

pub mod code_intelligence;
pub mod dependency_repository;
pub mod file_system;
pub mod graph_store;
pub mod parser;
pub mod refactor_strategy;
pub mod repository;
pub mod search_provider;
#[cfg(feature = "multimodal")]
pub mod source_extractor;

pub use code_intelligence::{
    CodeIntelligenceError, CodeIntelligenceProvider, DocumentSymbol, Reference, ReferenceKind,
    TypeHierarchy,
};
pub use dependency_repository::{DependencyError, DependencyRepository};
pub use file_system::{FileSystem, TextEdit, VfsError, VfsResult};
pub use graph_store::{GraphStore, StoreError};
pub use parser::{AstScanner, ParseError, ParseResult, ParsedTree, Parser, ScannedNode};
pub use refactor_strategy::{RefactorError, RefactorStrategy};
pub use repository::{Repository, RepositoryError};
pub use search_provider::{SearchError, SearchMatch, SearchProvider};
#[cfg(feature = "multimodal")]
pub use source_extractor::{
    ExtractedNode, SourceExtractor, SourceExtractorError, SourceExtractorResult, SourcePath,
};
