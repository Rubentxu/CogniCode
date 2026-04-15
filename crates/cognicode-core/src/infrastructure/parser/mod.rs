//! Parser module - Tree-sitter based parsing

mod ast_scanner;
mod tree_sitter_parser;

pub use ast_scanner::TreeSitterAstScanner;
pub use tree_sitter_parser::{IdentifierOccurrence, Language, TreeSitterParser};
