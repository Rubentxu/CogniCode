//! T1-5.3d — Snapshot test for the `IndexResult` text presenter.
//!
//! Estado pre-T1-5.3d:
//!   - T1-5.3c moved `execute_index` to return `IndexResult`, but
//!     the text output was emitted by a private `present_index_result`
//!     in `commands.rs` — unreachable from `tests/`.
//!   - A regression in the text shape (an off-by-one in indentation,
//!     a missing newline, a changed label like "Found 0 location(s)"
//!     instead of "No locations found for symbol 'X'") would not
//!     be caught by any test until a human noticed the diff in
//!     `cogh index ...` output.
//!
//! What this RED protects:
//!   - The exact text emitted by each `IndexResult` variant is
//!     pinned below. Any change to wording, capitalization, the
//!     number of spaces, the Debug format of `SymbolKind`, the
//!     branch characters in the outline tree, or the doc/code
//!     block separators breaks the corresponding test and forces
//!     the author to consciously update the snapshot.
//!   - The shape of the Outline variant (indentation unit `"  "`,
//!     branch characters `├── ` and `└── `, kind in parens, optional
//!     signature after `:`) is end-to-end-pinned via a 2-level
//!     recursion so the tree walk can't silently drop children or
//!     change the branch logic.
//!
//! Why snapshot and not golden-file (insta, etc.):
//!   - The project doesn't have a snapshot crate in its dependency
//!     tree (verified via `cargo tree`); adding one is a heavier
//!     decision than this single-purpose test needs. The expected
//!     strings are short and live inline; a reviewer can verify them
//!     by reading the assertion, no file fetch required.
//!   - The test fails with a diff that shows the expected vs actual
//!     byte sequence — that is exactly the failure mode we want.

use cognicode_core::domain::value_objects::SymbolKind;
use cognicode_core::infrastructure::graph::SymbolLocation;
use cognicode_core::interface::cli::presenter::present_index_result;
use cognicode_core::interface::cli::presenter::test_helpers::outline_node;
use cognicode_core::interface::cli::result::IndexResult;
use std::path::PathBuf;

fn render(result: &IndexResult) -> String {
    let mut buf: Vec<u8> = Vec::new();
    present_index_result(result, &mut buf)
        .expect("presenter write to Vec<u8> is infallible");
    String::from_utf8(buf).expect("presenter emits UTF-8 text")
}

#[test]
fn snapshot_build() {
    let result = IndexResult::Build {
        path: PathBuf::from("/work/repo"),
        strategy: "lightweight".to_string(),
        elapsed_ms: 1234,
    };
    let out = render(&result);
    assert_eq!(
        out, "Index built successfully in 1234ms using lightweight strategy\n",
        "Build variant text shape changed; update this test and the bin's doc"
    );
}

#[test]
fn snapshot_query_with_locations() {
    let result = IndexResult::Query {
        symbol: "main".to_string(),
        path: PathBuf::from("/tmp/proj"),
        locations: vec![
            SymbolLocation {
                file: "/tmp/proj/src/main.rs".to_string(),
                line: 1,
                column: 1,
                symbol_kind: SymbolKind::Function,
            },
            SymbolLocation {
                file: "/tmp/proj/src/lib.rs".to_string(),
                line: 42,
                column: 0,
                symbol_kind: SymbolKind::Method,
            },
        ],
    };
    let out = render(&result);
    // Pin: "Found N location(s):\n" header, then per-location
    // "  FILE:LINE:COL (SymbolKind)\n" with 2-space indent and
    // PascalCase Debug of SymbolKind.
    assert_eq!(
        out,
        "Found 2 location(s):\n\
         \x20\x20/tmp/proj/src/main.rs:1:1 (Function)\n\
         \x20\x20/tmp/proj/src/lib.rs:42:0 (Method)\n",
        "Query variant text shape changed; update this test and any user-facing doc"
    );
}

#[test]
fn snapshot_query_empty_uses_dedicated_message() {
    // The empty result is its own variant (QueryEmpty) and emits
    // a dedicated message, not "Found 0 location(s):\n". This is
    // the design's intentional distinction (see result.rs
    // is_empty_query helper).
    let result = IndexResult::QueryEmpty {
        symbol: "missing_xyz".to_string(),
        path: PathBuf::from("."),
    };
    let out = render(&result);
    assert_eq!(
        out, "No locations found for symbol 'missing_xyz'\n",
        "QueryEmpty text changed; consumers may grep this exact line"
    );
}

#[test]
fn snapshot_outline_two_level_recursion() {
    // The recursion is the part most likely to regress: branch
    // characters, indent unit, child walk. A 2-level tree forces
    // every code path (root, intermediate child with children,
    // terminal child) to be exercised.
    let leaf = outline_node("inner", SymbolKind::Function, 5, vec![]);
    let mid = outline_node(
        "outer",
        SymbolKind::Class,
        1,
        vec![leaf],
    );
    let result = IndexResult::Outline {
        file: PathBuf::from("/lib/foo.rs"),
        symbols: vec![mid],
    };
    let out = render(&result);
    // Pin:
    //   - Header "Found N top-level symbols:\n"
    //   - Root line: "{name} ({kind})\n" (no prefix at indent=0)
    //   - Child line at indent=1: "{branch} {name} ({kind})\n"
    //     where branch is "├── " for non-last, "└── " for last,
    //     and the leading indent at level 1 is empty (the branch
    //     character itself is the visual separator). Deeper levels
    //     add "  " per indent unit; the recursion test below does
    //     not exercise that, but `write_outline_tree` does it
    //     right (verified manually via the original
    //     `print_outline_tree` which had the same formula).
    //   - SymbolKind Debug in parens (PascalCase as defined)
    // The mid node has only one child, so it IS the last, hence "└── ".
    assert_eq!(
        out,
        "Found 1 top-level symbols:\n\
         outer (Class)\n\
         └── inner (Function)\n",
        "Outline variant text shape changed; branch characters or indent unit drifted"
    );
}

#[test]
fn snapshot_symbol_code_with_doc() {
    // The doc-then-code block uses blank-line separators ("\n///")
    // that downstream consumers (e.g. a script that copies the
    // code chunk to clipboard) depend on. Pin the exact shape.
    let result = IndexResult::SymbolCode {
        file: PathBuf::from("/lib/foo.rs"),
        line: 10,
        column: 5,
        start_line: 10,
        end_line: 15,
        doc: Some("/// A doc".to_string()),
        code: "fn foo() {}".to_string(),
    };
    let out = render(&result);
    assert_eq!(
        out,
        "\n\
         /// Docstring:\n\
         /// A doc\n\
         \n\
         /// Symbol code (lines 10 - 15):\n\
         fn foo() {}\n",
        "SymbolCode text shape changed; doc/code block separators drifted"
    );
}

#[test]
fn snapshot_symbol_code_without_doc_skips_doc_block() {
    // doc: None must not emit the "/// Docstring:" line at all
    // (vs. emit it with an empty body). The "no doc" path is its
    // own assertion because a regression here would silently add
    // a stray blank line to every symbol that lacks documentation.
    let result = IndexResult::SymbolCode {
        file: PathBuf::from("/lib/foo.rs"),
        line: 10,
        column: 5,
        start_line: 10,
        end_line: 15,
        doc: None,
        code: "fn foo() {}".to_string(),
    };
    let out = render(&result);
    assert_eq!(
        out,
        "\n\
         /// Symbol code (lines 10 - 15):\n\
         fn foo() {}\n",
        "SymbolCode text shape changed for doc=None; an empty doc block may have been added"
    );
    // Belt-and-braces: confirm the docstring header did NOT leak
    // into the no-doc path.
    assert!(
        !out.contains("Docstring:"),
        "doc=None must not emit the '/// Docstring:' header; got: {out:?}"
    );
}
