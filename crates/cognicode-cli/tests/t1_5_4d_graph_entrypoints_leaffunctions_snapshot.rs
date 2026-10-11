//! T1-5.4d — Snapshot test for the `GraphResult::EntryPoints`
//! and `GraphResult::LeafFunctions` presenters.
//!
//! Estado pre-T1-5.4d:
//!   - T1-5.4b migrated `execute_graph::EntryPoints` and
//!     `execute_graph::LeafFunctions` to return `GraphResult::*`.
//!     The text output was still emitted by 4 inline println!
//!     calls per arm (progress + blank + header + per-row +
//!     blank + summary) inside the function.
//!   - Like T1-5.3d / T1-5.4c, the text was unreachable from
//!     a `tests/` integration test. A regression in the label,
//!     the 2-space indent, the truncation at 20, or the
//!     blank-line separator would only surface when a human
//!     noticed the diff in `cogh graph entry-points` /
//!     `cogh graph leaf-functions` output.
//!
//! What this RED protects:
//!   - The exact byte sequence of the stdout output (progress
//!     line, blank line, header, up to 20 data lines, blank
//!     line, summary) for both variants.
//!   - The 'no incoming edges' / 'no outgoing edges' wording
//!     in the headers (different concept, easy to swap by
//!     mistake in a refactor).
//!   - The 'Found N entry points' / 'Found N leaf functions'
//!     wording in the summaries (singular vs plural; 'entry
//!     points' is always 2 words even when N=1; 'leaf
//!     functions' is the same).
//!   - The 'total' field is the un-truncated count; the data
//!     lines are up to 20. The summary uses 'total', so a
//!     user sees "Found 47 entry points" with only 20 listed.
//!
//! Lo que T1-5.4d NO testea (por diseño):
//!   - El 'Warning: graph is PARTIAL' eprintln! a stderr no
//!     es parte de estos snapshots (decisión documentada en
//!     odd/tasks/t1-5-4-decision-partial-warning.md).

use cognicode_cli::presenter::{present_graph_entrypoints, present_graph_leaffunctions};
use cognicode_core::interface::cli::result::GraphResult;

fn render_eps(result: &GraphResult) -> String {
    let mut buf: Vec<u8> = Vec::new();
    present_graph_entrypoints(result, &mut buf).expect("presenter to Vec<u8> infallible");
    String::from_utf8(buf).expect("UTF-8 text")
}

fn render_lf(result: &GraphResult) -> String {
    let mut buf: Vec<u8> = Vec::new();
    present_graph_leaffunctions(result, &mut buf).expect("presenter to Vec<u8> infallible");
    String::from_utf8(buf).expect("UTF-8 text")
}

#[test]
fn snapshot_entrypoints_typical() {
    // 3 of 47 entry points listed (the presenter doesn't truncate;
    // the data side already has at most 20 via iter().take(20)).
    let result = GraphResult::EntryPoints {
        path: "/work/repo".to_string(),
        symbols: vec![
            cognicode_core::interface::cli::result::SymbolRef {
                name: "main".to_string(),
                file: "/lib/main.rs".to_string(),
                line: 1,
                column: 1,
            },
            cognicode_core::interface::cli::result::SymbolRef {
                name: "alpha".to_string(),
                file: "/lib/foo.rs".to_string(),
                line: 10,
                column: 1,
            },
            cognicode_core::interface::cli::result::SymbolRef {
                name: "beta".to_string(),
                file: "/lib/bar.rs".to_string(),
                line: 5,
                column: 1,
            },
        ],
        total: 47,
        elapsed_ms: 30,
    };
    let out = render_eps(&result);
    assert_eq!(
        out,
        "Finding entry points in: /work/repo\n\
         \n\
         Entry points (no incoming edges):\n\
         \x20\x20main at /lib/main.rs:1:1\n\
         \x20\x20alpha at /lib/foo.rs:10:1\n\
         \x20\x20beta at /lib/bar.rs:5:1\n\
         \n\
         Found 47 entry points in 30ms\n",
        "EntryPoints text shape changed; check the 'no incoming edges' header and 'Found N entry points' summary"
    );
}

#[test]
fn snapshot_entrypoints_empty() {
    // 0 entry points: only the progress, blank, header, blank, summary lines.
    // No "  {name} at {file}:{line}:{column}" lines (the symbols vec is empty).
    let result = GraphResult::EntryPoints {
        path: "/work/empty".to_string(),
        symbols: vec![],
        total: 0,
        elapsed_ms: 5,
    };
    let out = render_eps(&result);
    assert_eq!(
        out,
        "Finding entry points in: /work/empty\n\
         \n\
         Entry points (no incoming edges):\n\
         \n\
         Found 0 entry points in 5ms\n",
        "EntryPoints empty-case text changed; check the blank lines around the empty data block"
    );
}

#[test]
fn snapshot_leaffunctions_typical() {
    // Same shape as entry points; only the labels differ.
    // Pin 'no outgoing edges' (vs 'no incoming' for entry
    // points) and 'leaf functions' (vs 'entry points' in
    // summary).
    let result = GraphResult::LeafFunctions {
        path: "/work/repo".to_string(),
        symbols: vec![cognicode_core::interface::cli::result::SymbolRef {
            name: "alpha".to_string(),
            file: "/lib/foo.rs".to_string(),
            line: 10,
            column: 1,
        }],
        total: 1,
        elapsed_ms: 8,
    };
    let out = render_lf(&result);
    assert_eq!(
        out,
        "Finding leaf functions in: /work/repo\n\
         \n\
         Leaf functions (no outgoing edges):\n\
         \x20\x20alpha at /lib/foo.rs:10:1\n\
         \n\
         Found 1 leaf functions in 8ms\n",
        "LeafFunctions text shape changed; check 'no outgoing edges' and 'Found N leaf functions'"
    );
}

#[test]
fn snapshot_entrypoints_summary_uses_total_not_symbols_len() {
    // The 'Found N' summary uses 'total' (the un-truncated
    // count), not 'symbols.len()'. With 2 listed but 47
    // total, the user sees the real number of roots. This
    // invariant matters: a naive 'symbols.len()' would hide
    // the truncation from the user.
    let result = GraphResult::EntryPoints {
        path: "/work/repo".to_string(),
        symbols: vec![
            cognicode_core::interface::cli::result::SymbolRef {
                name: "a".to_string(),
                file: "/lib/a.rs".to_string(),
                line: 1,
                column: 1,
            },
            cognicode_core::interface::cli::result::SymbolRef {
                name: "b".to_string(),
                file: "/lib/b.rs".to_string(),
                line: 1,
                column: 1,
            },
        ],
        total: 47, // 47 total, 2 listed
        elapsed_ms: 10,
    };
    let out = render_eps(&result);
    assert!(
        out.contains("Found 47 entry points"),
        "Summary must use the un-truncated 'total' (47), not symbols.len() (2): {out:?}"
    );
    assert!(
        !out.contains("Found 2 entry points"),
        "Summary must NOT use symbols.len(); that would hide the truncation: {out:?}"
    );
}
