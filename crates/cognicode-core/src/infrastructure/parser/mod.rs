//! Parser module - Tree-sitter based parsing

mod ast_scanner;
pub mod language_config;
mod tree_sitter_parser;

pub use ast_scanner::TreeSitterAstScanner;
pub use language_config::LanguageConfig;
pub use tree_sitter_parser::{IdentifierOccurrence, Language, TreeSitterParser};
