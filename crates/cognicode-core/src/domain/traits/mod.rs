//! Domain traits module
//!
//! Traits that define the contracts for domain operations.

pub mod code_intelligence;
pub mod dependency_repository;
pub mod file_system;
pub mod graph_store;
pub mod parser;
pub mod refactor_strategy;
pub mod search_provider;

pub use code_intelligence::{CodeIntelligenceProvider, CodeIntelligenceError, Reference, ReferenceKind, TypeHierarchy, DocumentSymbol};
pub use dependency_repository::{DependencyRepository, DependencyError};
pub use file_system::{FileSystem, TextEdit, VfsError, VfsResult};
pub use graph_store::{GraphStore, StoreError};
pub use parser::{AstScanner, ParseError, ParseResult, Parser, ParsedTree, ScannedNode};
pub use refactor_strategy::{RefactorStrategy, RefactorError};
pub use search_provider::{SearchProvider, SearchMatch, SearchError};
