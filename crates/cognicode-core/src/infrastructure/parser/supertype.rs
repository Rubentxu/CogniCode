//! The parents a declaration names, and the declaration that names them.
//!
//! A declaration that names a parent —a class that extends another, a type
//! that implements a trait— states a relation between two declarations. The
//! reader of the language says which node kinds mean that, and this module
//! carries what the reader said to the routes that build the graph.
//!
//! The relation hangs on a declaration, and the declaration is not always
//! the node the reader was given: Rust's `impl Trait for Type` names the
//! trait in a block that declares nothing, so the relation belongs to the
//! type that implements the trait. The site says so by naming the owner
//! instead of pointing at it.

use crate::domain::aggregates::call_graph::SymbolId;
use crate::domain::aggregates::symbol::Symbol;
use crate::domain::value_objects::Language;

/// A parent a declaration names.
#[derive(Debug, Clone)]
pub struct SupertypeSite {
    /// The declaration that owns the relation.
    pub source: SupertypeSource,
    /// The parent, as it is written.
    pub parent: String,
    /// The line the parent is written on, counted from one.
    pub line: u32,
}

/// The declaration that owns a parent relation.
#[derive(Debug, Clone)]
pub enum SupertypeSource {
    /// The declaration the reader was given: the class that names the
    /// parent declares it.
    Declaration(SymbolId),
    /// A type the reader named: the declaration that owns the relation is
    /// named, not pointed at.
    Named(String),
}

/// The declaration of a file that carries `name`, when the file carries
/// exactly one.
///
/// A name two declarations of the same file carry does not name either one:
/// the relation cannot be hung on a declaration the file does not single
/// out, so the answer is `None` and the relation is not published.
///
/// The spelling is the language's: a language that tells `Describe` from
/// `describe` does not answer for the other one.
pub fn declaration_named(symbols: &[Symbol], name: &str, folds_case: bool) -> Option<SymbolId> {
    let mut found = symbols
        .iter()
        .filter(|symbol| Language::answers_to_in(symbol.name(), name, folds_case));
    let first = found.next()?;
    if found.next().is_some() {
        return None;
    }
    Some(SymbolId::new(first.fully_qualified_name()))
}
