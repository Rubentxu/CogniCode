//! Regression test: tree-sitter-haskell 0.23.1 wrote one element past its
//! lookahead buffer while scanning a line comment.
//!
//! `build_lightweight_index` aborted the whole MCP server with
//! `corrupted size vs. prev_size` (SIGABRT) on any directory containing a
//! Haskell file. With C instrumentation the invalid write is
//! `tree-sitter-haskell-0.23.1/src/scanner.c:651`, in `advance()`:
//!
//! ```c
//! #define array_push(self, element)                            \
//!   (_array__grow((Array *)(self), 1, array_elem_size(self)),    \
//!    (self)->contents[(self)->size++] = (element))
//! ```
//!
//! `_array__grow` may `ts_realloc` and update `self->contents`. When the
//! realloc moves the block, `realloc` frees the old one, and the store in the
//! second half of the macro still lands in the *old*, freed buffer, because the
//! operand order is unspecified and the compiler is free to load
//! `self->contents` before the call. 0.24.1 rewrote `array_push` to re-read
//! `self->contents` after growing, which is the actual fix.
//!
//! The minimized trigger is a line comment body of at least 9 bytes
//! (`-- Haskel`), which pushes the lookahead array past its first growth.
//!
//! The overflow is a heap write in C, so it aborts the whole process rather
//! than failing an assertion. If this bug is present the test binary dies
//! with SIGABRT instead of reporting a failure.

use cognicode_core::domain::value_objects::Language;
use cognicode_core::infrastructure::parser::TreeSitterParser;

#[test]
fn haskell_line_comment_does_not_overflow_lookahead() {
    // The exact minimized input from the ASan report.
    let comment_line = "-- Haskel";

    let parser = TreeSitterParser::new(Language::Haskell).expect("haskell grammar must load");
    let tree = parser
        .parse_tree(comment_line)
        .expect("haskell line comment must parse");

    assert_eq!(
        tree.root_node().kind(),
        "haskell",
        "a bare haskell line comment is a haskell source_file"
    );
    assert_eq!(
        tree.root_node().child(0).map(|n| n.kind()),
        Some("comment"),
        "a Haskell line comment must produce a comment node"
    );
    assert!(!tree.root_node().has_error());

    // The real production shape: a whole file that opens with a comment and
    // then carries real declarations. This is what aborted the MCP server.
    let source = "\
-- Haskell test fixture
module Main where

main :: IO ()
main = putStrLn \"hi\"
";
    let tree = parser
        .parse_tree(source)
        .expect("haskell file starting with a comment must parse");
    assert!(
        !tree.root_node().has_error(),
        "valid haskell source must not produce an error node: {:?}",
        tree.root_node().to_sexp()
    );
}

#[test]
fn haskell_comment_bodies_of_many_lengths_parse() {
    // Sweep lengths across the lookahead buffer's first growth boundary, since
    // the overflow only fires once the array reallocates.
    let parser = TreeSitterParser::new(Language::Haskell).expect("haskell grammar must load");
    for len in 0..64usize {
        let body = "x".repeat(len);
        let source = format!("-- {body}\nmain = 1\n");
        let tree = parser
            .parse_tree(&source)
            .unwrap_or_else(|e| panic!("comment body length {len} failed to parse: {e:?}"));
        assert!(
            !tree.root_node().has_error(),
            "comment body length {len} produced an error node"
        );
    }
}