//! The reader is the only one that can say what a test is.
//!
//! A Rust file says it with `#[test]`, a Java file with `@Test`, a C# file
//! with `[Fact]`. The mark is in the source, and the source is what this
//! module reads: it looks at the declaration that was just read, at the
//! mark that sits next to it, and hands the mark to the domain, which
//! knows what the mark of that language means.
//!
//! The languages that NAME their tests (Python, Go, JavaScript and
//! TypeScript) have no mark to look for: there the convention is the
//! answer, and the domain applies it to the file and the name the reader
//! already has.
//!
//! What this module does NOT do is guess. A function whose name looks like
//! a test is not a test unless the file says so, and a mark that does not
//! declare a test (`#[cfg(test)]`, which says that a block is only compiled
//! for tests) is not a declaration.

use tree_sitter::Node;

use crate::domain::services::{TestDeclaration, declaration_for, is_test};
use crate::domain::value_objects::Language;

/// How many marks before a declaration this module is willing to read.
///
/// A declaration is preceded by its marks, and a file that writes four
/// attributes before a function is not the case this needs to explain.
/// The bound is here so that a file with a long preamble cannot make the
/// reader walk the whole tree for every symbol.
const MAX_MARKS_BEFORE: usize = 4;

/// Whether the declaration at `node` is a test.
///
/// `name` is the name the reader gave the declaration, and `file` the file
/// it lives in: the languages that name their tests are decided with those
/// two.
pub fn declares_a_test(
    node: Node,
    source: &str,
    file: &str,
    name: &str,
    language: Language,
) -> bool {
    // A language that marks its tests needs the mark; one that names them
    // does not, and asking for a mark it does not use would be noise.
    let mark = match declaration_for(language) {
        Some(TestDeclaration::Attribute(_)) => mark_of(node, source),
        _ => None,
    };
    is_test(language, file, name, mark.as_deref())
}

/// The mark that sits next to the declaration at `node`.
///
/// The mark is the attribute that precedes the declaration: `#[test]` in
/// Rust, `@Test` in Java and Kotlin, `[Fact]` in C#. The name it declares
/// is what the domain compares: the text without its delimiters, and
/// without the arguments that follow it (`#[test_case(1, 2)]` declares
/// `test_case`).
pub fn mark_of(node: Node, source: &str) -> Option<String> {
    let mut sibling = node.prev_sibling();
    let mut read = 0;

    while let Some(previous) = sibling {
        if read >= MAX_MARKS_BEFORE {
            break;
        }
        read += 1;

        let text = text_of(previous, source);
        if !looks_like_a_mark(&text) {
            break;
        }
        if let Some(name) = mark_name(&text) {
            return Some(name.to_string());
        }
        sibling = previous.prev_sibling();
    }

    // Some grammars hang the mark inside the declaration instead of next
    // to it.
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let text = text_of(child, source);
        if looks_like_a_mark(&text)
            && let Some(name) = mark_name(&text)
        {
            return Some(name.to_string());
        }
    }

    None
}

/// The text of a node.
fn text_of(node: Node, source: &str) -> String {
    node.utf8_text(source.as_bytes())
        .unwrap_or_default()
        .to_string()
}

/// Whether the text looks like a mark rather than another declaration.
///
/// A mark starts with `#`, `@` or `[`. Anything else is a declaration that
/// happens to precede this one, and the marks are over.
fn looks_like_a_mark(text: &str) -> bool {
    let text = text.trim();
    text.starts_with('#') || text.starts_with('@') || text.starts_with('[')
}

/// The name a mark declares.
///
/// The delimiters go, and with them the arguments: `#[test_case(1, 2)]`
/// declares `test_case`, and `#[cfg(test)]` declares `cfg` — which is not
/// a test, and that is the point of reading the name and not the text.
fn mark_name(text: &str) -> Option<&str> {
    let inner = text.trim();
    let inner = inner.trim_start_matches(['#', '@', '[']);
    let inner = inner.trim_end_matches([']', ')']);
    let name = inner.split('(').next().unwrap_or(inner).trim();
    (!name.is_empty()).then_some(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::parser::TreeSitterParser;

    /// The symbols of a Rust source, read the way the graph reads them.
    fn rust_symbols(source: &str) -> Vec<crate::domain::aggregates::Symbol> {
        let parser =
            TreeSitterParser::with_cache(Language::Rust).expect("the rust reader is available");
        parser
            .find_all_symbols_with_path(source, "src/thing.rs")
            .expect("the source is rust")
    }

    #[test]
    fn a_rust_test_is_marked_as_one() {
        let symbols = rust_symbols("#[test]\nfn checks() {}\n");
        let test = symbols
            .iter()
            .find(|s| s.name() == "checks")
            .expect("the function is read");
        assert!(test.is_test(), "the mark makes it a test");
    }

    #[test]
    fn a_rust_function_without_a_mark_is_not_a_test() {
        let symbols = rust_symbols("fn helper() {}\n");
        let helper = symbols
            .iter()
            .find(|s| s.name() == "helper")
            .expect("the function is read");
        assert!(!helper.is_test(), "a function with no mark is not a test");
    }

    #[test]
    fn a_mark_that_does_not_declare_a_test_does_not() {
        let symbols =
            rust_symbols("#[cfg(test)]\nmod tests {\n    #[test]\n    fn checks() {}\n}\n");
        let test = symbols
            .iter()
            .find(|s| s.name() == "checks")
            .expect("the function is read");
        assert!(
            test.is_test(),
            "the mark of the function is the one that declares it: {:?}",
            test
        );
    }

    #[test]
    fn a_test_that_is_not_marked_is_not_a_test() {
        let symbols = rust_symbols("fn test_looking() {}\n");
        let helper = symbols
            .iter()
            .find(|s| s.name() == "test_looking")
            .expect("the function is read");
        assert!(
            !helper.is_test(),
            "a name that looks like a test is not a test"
        );
    }

    #[test]
    fn the_name_of_a_mark_ignores_its_arguments() {
        assert_eq!(mark_name("#[test_case(1, 2)]"), Some("test_case"));
        assert_eq!(mark_name("#[cfg(test)]"), Some("cfg"));
        assert_eq!(mark_name("@Test"), Some("Test"));
        assert_eq!(mark_name("[Fact]"), Some("Fact"));
    }
}
