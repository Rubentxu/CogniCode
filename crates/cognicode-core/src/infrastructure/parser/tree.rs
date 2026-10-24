//! Where the trees of the product are built.
//!
//! Reading a file means walking a tree of it, and a tree costs a parse. A
//! file that is read twice costs two trees, and a reader that builds its own
//! tree pays for it again without learning anything the first tree did not
//! already say. Every tree the product builds comes from here, so the number
//! of trees a file costs is a number this module can answer.

use std::cell::Cell;

thread_local! {
    /// The trees built on this thread.
    ///
    /// The count is per thread because the product parses files in parallel
    /// and a shared count would make the answer depend on what the other
    /// threads happened to be doing.
    static TREES_BUILT: Cell<usize> = const { Cell::new(0) };
}

/// Builds the tree of a source with a parser that already holds its grammar.
///
/// The parser is reused by the caller, so the grammar is bound once and the
/// tree is the only thing this call pays for.
pub fn parse_with(parser: &mut tree_sitter::Parser, source: &str) -> Option<tree_sitter::Tree> {
    let tree = parser.parse(source, None);
    #[cfg(test)]
    if tree.is_some() {
        TREES_BUILT.with(|built| built.set(built.get() + 1));
    }
    tree
}

/// Builds the tree of a source with a grammar.
///
/// The grammar is bound to a parser that is thrown away afterwards, so this
/// is the way to read a file once and look at the tree it produced.
pub fn build_tree(language: &tree_sitter::Language, source: &str) -> Option<tree_sitter::Tree> {
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(language).ok()?;
    parse_with(&mut parser, source)
}

/// How many trees have been built on this thread.
///
/// The answer is what tells a file that was read once from a file that was
/// read twice, and a product that cannot tell them apart cannot promise that
/// a file is read once.
#[cfg(test)]
pub fn trees_built_here() -> usize {
    TREES_BUILT.with(Cell::get)
}

/// How many trees have been built on this thread since the last reset.
#[cfg(test)]
pub fn reset_trees_built_here() {
    TREES_BUILT.with(|built| built.set(0));
}
