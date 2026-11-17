//! A second opinion on whether a source file is well-formed.
//!
//! The bundled tree-sitter grammars lag behind their languages: some valid
//! constructs end in `ERROR` nodes. A fault of the grammar and a fault of the
//! file look the same in the tree, and only a reader that is not the grammar
//! can tell them apart. Where such a reader exists it is asked here; where it
//! does not, the answer is "not known to be well-formed", which keeps the file
//! a syntax error rather than excusing it.

use crate::domain::value_objects::Language;

/// Whether an independent parser of `language` accepts `source` as well-formed.
///
/// `true` only when a reader other than the bundled grammar parsed the whole
/// file; it never means the file was read by the extractor. `false` covers
/// both a file that is really broken and a language with no independent
/// reader, so a caller must treat it as "unconfirmed", never as "broken".
/// Pure: no I/O.
pub(crate) fn is_independently_well_formed(language: Language, source: &str) -> bool {
    match language {
        Language::Rust => syn::parse_file(source).is_ok(),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_the_independent_parser_accepts_is_well_formed() {
        assert!(is_independently_well_formed(
            Language::Rust,
            "pub fn run<T>() where (): Marker<T> {}\n"
        ));
    }

    #[test]
    fn rust_with_an_unclosed_delimiter_is_not_well_formed() {
        assert!(!is_independently_well_formed(
            Language::Rust,
            "pub fn lost( {{{\n"
        ));
    }

    #[test]
    fn a_language_without_an_independent_reader_is_never_confirmed() {
        assert!(!is_independently_well_formed(
            Language::Go,
            "package demo\nfunc Run() {}\n"
        ));
    }
}
