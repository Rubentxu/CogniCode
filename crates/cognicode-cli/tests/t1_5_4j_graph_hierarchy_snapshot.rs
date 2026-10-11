//! T1-5.4j — Snapshot test for the `GraphResult::Hierarchy` presenter.
//!
//! Estado pre-T1-5.4j:
//!   - T1-5.4b migrated `execute_graph::Hierarchy` to return
//!     `GraphResult::Hierarchy { symbol, depth, direction,
//!     root_file, root_line, root_column, buckets,
//!     total_entries, elapsed_ms }`. The text output (a
//!     multi-block hierarchy with per-depth entries) was
//!     still emitted by ~10 inline println! calls.
//!   - The 'Root:' line uses 'name at file:line:col' (4-tuple
//!     with trailing column), NOT the 'name (file:line:column)'
//!     3-tuple form used by OnDemand. Pin the distinction.
//!   - The per-depth entry lines are 2-space indented under
//!     the '  Depth N: M entries' line. The 2-space indent
//!     is part of the contract.
//!
//! What this RED protects:
//!   - The exact byte sequence of the result block + summary
//!     (the progress line is NOT in the snapshot — same
//!     pattern as TracePath/Mermaid).
//!   - The 'Root: NAME at FILE:LINE:COL' form (with 'at',
//!     not the 3-tuple parens form used by OnDemand).
//!   - The per-depth entry line format '    - NAME (DIR)
//!     at FILE:LINE' (4-space indent, hyphen-space prefix).
//!   - The per-depth entry count truncation at 5 (the data
//!     side has at most 5 entries per bucket).

use cognicode_cli::presenter::present_graph_hierarchy;
use cognicode_core::interface::cli::result::{DepthBucket, DepthEntry, GraphResult};

fn render(result: &GraphResult) -> String {
    let mut buf: Vec<u8> = Vec::new();
    present_graph_hierarchy(result, &mut buf).expect("presenter to Vec<u8> infallible");
    String::from_utf8(buf).expect("UTF-8 text")
}

#[test]
fn snapshot_hierarchy_two_depths() {
    // Two depth buckets. depth=1 has 2 entries; depth=2 has
    // 1 entry. The total is 3.
    let result = GraphResult::Hierarchy {
        symbol: "alpha".to_string(),
        depth: 2,
        direction: "out".to_string(),
        root_file: "/lib/a.rs".to_string(),
        root_line: 1,
        root_column: 1,
        buckets: vec![
            DepthBucket {
                depth: 1,
                entries: vec![
                    DepthEntry {
                        name: "beta".to_string(),
                        direction: "callees".to_string(),
                        file: "/lib/b.rs".to_string(),
                        line: 5,
                    },
                    DepthEntry {
                        name: "gamma".to_string(),
                        direction: "callees".to_string(),
                        file: "/lib/c.rs".to_string(),
                        line: 10,
                    },
                ],
            },
            DepthBucket {
                depth: 2,
                entries: vec![DepthEntry {
                    name: "delta".to_string(),
                    direction: "callees".to_string(),
                    file: "/lib/d.rs".to_string(),
                    line: 1,
                }],
            },
        ],
        total_entries: 3,
        elapsed_ms: 12,
    };
    let out = render(&result);
    // Pin the byte sequence. Note: the 'Root:' line uses
    // 'at' (not parens); the per-depth entries use 4-space
    // indent + '- '.
    assert_eq!(
        out,
        "\n\
         Call hierarchy for 'alpha':\n\
         Root: alpha at /lib/a.rs:1:1\n\
         \n\
         Entries by depth:\n\
         \x20\x20Depth 1: 2 entries\n\
         \x20\x20\x20\x20- beta (callees) at /lib/b.rs:5\n\
         \x20\x20\x20\x20- gamma (callees) at /lib/c.rs:10\n\
         \x20\x20Depth 2: 1 entries\n\
         \x20\x20\x20\x20- delta (callees) at /lib/d.rs:1\n\
         \n\
         Total entries: 3 in 12ms\n",
        "Hierarchy text shape changed; check the 'Root: NAME at FILE:LINE:COL' form and the 2/4-space indents"
    );
}

#[test]
fn snapshot_hierarchy_empty_no_buckets() {
    // No entries at any depth. The result block has the
    // 'Call hierarchy' and 'Root:' lines + the 'Entries by
    // depth:' header + the summary, but no '  Depth N:' lines.
    let result = GraphResult::Hierarchy {
        symbol: "ghost".to_string(),
        depth: 3,
        direction: "in".to_string(),
        root_file: "/lib/g.rs".to_string(),
        root_line: 1,
        root_column: 1,
        buckets: vec![],
        total_entries: 0,
        elapsed_ms: 1,
    };
    let out = render(&result);
    assert_eq!(
        out,
        "\n\
         Call hierarchy for 'ghost':\n\
         Root: ghost at /lib/g.rs:1:1\n\
         \n\
         Entries by depth:\n\
         \n\
         Total entries: 0 in 1ms\n",
        "Hierarchy empty-case text changed; check the blank line between 'Entries by depth:' and 'Total entries'"
    );
}

#[test]
fn snapshot_hierarchy_root_uses_at_form_not_paren_3tuple() {
    // Belt-and-braces: pin that the Root line uses the
    // 'at' form (Hierarchy), NOT the paren 3-tuple form
    // (OnDemand). A refactor that unifies the two would
    // silently change the OnDemand vs Hierarchy output.
    let result = GraphResult::Hierarchy {
        symbol: "x".to_string(),
        depth: 1,
        direction: "in".to_string(),
        root_file: "/a.rs".to_string(),
        root_line: 1,
        root_column: 1,
        buckets: vec![],
        total_entries: 0,
        elapsed_ms: 1,
    };
    let out = render(&result);
    assert!(
        out.contains("Root: x at /a.rs:1:1"),
        "Root line must use the 'at' form: {out:?}"
    );
    assert!(
        !out.contains("Root: x (/a.rs:1:1)"),
        "Root line must NOT use the paren 3-tuple form (that's for OnDemand): {out:?}"
    );
}

#[test]
fn snapshot_hierarchy_per_depth_count_includes_truncated_entries() {
    // The data side has at most 5 entries per bucket
    // (iter().take(5) at commands.rs:1437). The 'N entries'
    // count on the 'Depth:' line uses entries.len() (the
    // post-truncation count), NOT the un-truncated count.
    // With 5 entries in the bucket, the line says
    // '5 entries' (matches). The truncation is invisible
    // to the user when the count is <= 5.
    let bucket = DepthBucket {
        depth: 1,
        entries: (0..5)
            .map(|i| DepthEntry {
                name: format!("sym{i}"),
                direction: "callees".to_string(),
                file: "/lib/x.rs".to_string(),
                line: i + 1,
            })
            .collect(),
    };
    let result = GraphResult::Hierarchy {
        symbol: "alpha".to_string(),
        depth: 1,
        direction: "out".to_string(),
        root_file: "/lib/a.rs".to_string(),
        root_line: 1,
        root_column: 1,
        buckets: vec![bucket],
        total_entries: 5,
        elapsed_ms: 1,
    };
    let out = render(&result);
    assert!(
        out.contains("  Depth 1: 5 entries"),
        "Per-depth count uses entries.len() (5 in this case): {out:?}"
    );
    // The 5 per-depth entry lines should follow.
    let entry_lines: Vec<&str> = out.lines().filter(|l| l.starts_with("    - ")).collect();
    assert_eq!(
        entry_lines.len(),
        5,
        "5 per-depth entry lines expected (one per bucket entry), got {}",
        entry_lines.len()
    );
}
