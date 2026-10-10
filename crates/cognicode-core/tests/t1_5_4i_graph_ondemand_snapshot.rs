//! T1-5.4i — Snapshot test for the `GraphResult::OnDemand` presenter.
//!
//! Estado pre-T1-5.4i:
//!   - T1-5.4b migrated `execute_graph::OnDemand` to return
//!     `GraphResult::OnDemand { symbol, depth, direction,
//!     elapsed_ms, root_name, root_file, root_line,
//!     root_column, entries_len }`. The text output (5
//!     lines: progress + blank + timing + Root + Entries)
//!     was still emitted by inline println! calls.
//!   - The 'Root:' line uses the parenthesized 3-tuple form
//!     'name (file:line:column)' — different from the
//!     'name at file:line' form used by EntryPoints,
//!     LeafFunctions, and TracePath. The snapshot pins this
//!     distinction.
//!
//! What this RED protects:
//!   - The exact byte sequence of the 5-line output (progress,
//!     blank, timing, Root, Entries).
//!   - The 'depth=N, direction=DIR' suffix in the progress
//!     line (with single quotes around the symbol name).
//!   - The 'Root: {name} ({file}:{line}:{column})' form
//!     (note: parens, not 'at', not '@').

use cognicode_core::interface::cli::presenter::present_graph_ondemand;
use cognicode_core::interface::cli::result::GraphResult;

fn render(result: &GraphResult) -> String {
    let mut buf: Vec<u8> = Vec::new();
    present_graph_ondemand(result, &mut buf).expect("presenter to Vec<u8> infallible");
    String::from_utf8(buf).expect("UTF-8 text")
}

#[test]
fn snapshot_ondemand_typical() {
    let result = GraphResult::OnDemand {
        symbol: "alpha".to_string(),
        depth: 3,
        direction: "both".to_string(),
        elapsed_ms: 42,
        root_name: "alpha".to_string(),
        root_file: "/lib/foo.rs".to_string(),
        root_line: 10,
        root_column: 1,
        entries_len: 5,
    };
    let out = render(&result);
    assert_eq!(
        out,
        "Building on-demand subgraph for 'alpha' (depth=3, direction=both)\n\
         \n\
         Subgraph built in 42ms\n\
         Root: alpha (/lib/foo.rs:10:1)\n\
         Entries: 5\n",
        "OnDemand text shape changed; check the 'Root: NAME (FILE:LINE:COL)' form and the progress line's single quotes"
    );
}

#[test]
fn snapshot_ondemand_zero_entries() {
    // The data side carries entries_len (which can be 0).
    // The presenter must print "Entries: 0", not a "no
    // entries" message.
    let result = GraphResult::OnDemand {
        symbol: "ghost".to_string(),
        depth: 1,
        direction: "out".to_string(),
        elapsed_ms: 1,
        root_name: "ghost".to_string(),
        root_file: "/lib/x.rs".to_string(),
        root_line: 1,
        root_column: 1,
        entries_len: 0,
    };
    let out = render(&result);
    assert!(
        out.contains("Entries: 0"),
        "Entries line must say 'Entries: 0' (not a 'no entries' message): {out:?}"
    );
    assert!(
        out.contains("direction=out"),
        "Progress line must include the direction arg: {out:?}"
    );
}

#[test]
fn snapshot_ondemand_root_uses_parenthesized_3tuple() {
    // Belt-and-braces: pin that the Root line uses the
    // 3-tuple form (file:line:column in parens), NOT the
    // 'name at file:line' form used by EntryPoints and
    // other sub-commands. A refactor that unifies the two
    // would break the OnDemand-specific snapshot.
    let result = GraphResult::OnDemand {
        symbol: "x".to_string(),
        depth: 1,
        direction: "in".to_string(),
        elapsed_ms: 1,
        root_name: "x".to_string(),
        root_file: "/a.rs".to_string(),
        root_line: 1,
        root_column: 1,
        entries_len: 0,
    };
    let out = render(&result);
    assert!(
        out.contains("Root: x (/a.rs:1:1)"),
        "Root line must use the 3-tuple form 'name (file:line:column)': {out:?}"
    );
    assert!(
        !out.contains("Root: x at /a.rs"),
        "Root line must NOT use the 'at' form (that's for other sub-commands): {out:?}"
    );
}
