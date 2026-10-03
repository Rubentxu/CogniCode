//! Parser module - Tree-sitter based parsing

mod ast_scanner;
pub mod language_config;
pub mod syntax_analysis;
mod tree_sitter_parser;
pub mod type_ref_walkers;

pub use ansible_handler::interpret_ansible;
pub use ast_scanner::TreeSitterAstScanner;
pub use language_config::LanguageConfig;
pub use terraform_handler::interpret_terraform;
pub use tree_sitter_parser::{IdentifierOccurrence, TreeSitterParser};

// `Language` is a domain value object since ST-02 slice 2; this is a
// re-export of the one definition in `domain::value_objects`, not a second
// enum. The adapters below this line kept the path they already imported, and
// the application layer now imports it from `domain` where it belongs.
pub use crate::domain::value_objects::Language;
pub use type_ref_walkers::{
    walk_c_type_refs, walk_cpp_type_refs, walk_csharp_type_refs, walk_go_type_refs,
    walk_java_type_refs, walk_php_type_refs, walk_python_type_refs, walk_ruby_type_refs,
    walk_rust_type_refs, walk_swift_type_refs, walk_typescript_type_refs,
};
pub mod ansible_handler;
pub mod terraform_handler;
