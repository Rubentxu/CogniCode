//! The declaration that starts at a position, read from the bytes of a file.
//!
//! [`TreeSitterDeclarations`] answers the [`DeclarationReader`] port of the
//! context pack. It reads the text it is given and never opens a file, so
//! the caller decides which bytes —the ones it verified— are parsed.

use std::ffi::OsStr;
use std::path::Path;

use super::symbol_code::{find_enclosing_symbol, find_node_at_position, is_symbol_node};
use crate::application::context_pack::{DeclarationReader, DeclarationSpan};
use crate::infrastructure::parser::{Language, TreeSitterParser};

/// Reads declarations with the tree-sitter grammar of the file's language.
///
/// A language the product cannot parse, a position that no declaration
/// encloses and a source that does not parse all answer `None`: the caller
/// then publishes the line of the symbol and says that is all it knows.
#[derive(Debug, Default, Clone, Copy)]
pub struct TreeSitterDeclarations;

impl DeclarationReader for TreeSitterDeclarations {
    fn declaration(
        &self,
        file: &str,
        source: &str,
        line: u32,
        column: u32,
    ) -> Option<DeclarationSpan> {
        let extension = Path::new(file).extension()?;
        let language = Language::from_extension(Some(OsStr::new(extension)))?;
        let tree = TreeSitterParser::new(language)
            .ok()?
            .parse_tree(source)
            .ok()?;
        let at = find_node_at_position(tree.root_node(), line, column)?;
        let declaration = if is_symbol_node(at, language) {
            at
        } else {
            find_enclosing_symbol(at, language)
        };
        if !is_symbol_node(declaration, language) {
            return None;
        }
        let start_line = declaration.start_position().row as u32 + 1;
        let end_line = declaration.end_position().row as u32 + 1;
        if line + 1 < start_line || line + 1 > end_line {
            return None;
        }
        let body = declaration.child_by_field_name("body");
        let header_end = body.map_or(declaration.end_byte(), |body| body.start_byte());
        let header = source
            .get(declaration.start_byte()..header_end)?
            .trim_end()
            .to_string();
        Some(DeclarationSpan {
            start_line,
            end_line,
            header,
            body_statements: body.map_or_else(Vec::new, statements_of),
        })
    }
}

/// The line ranges of the direct children of a body block.
fn statements_of(body: tree_sitter::Node<'_>) -> Vec<(u32, u32)> {
    let mut cursor = body.walk();
    body.named_children(&mut cursor)
        .map(|statement| {
            (
                statement.start_position().row as u32 + 1,
                statement.end_position().row as u32 + 1,
            )
        })
        .collect()
}
