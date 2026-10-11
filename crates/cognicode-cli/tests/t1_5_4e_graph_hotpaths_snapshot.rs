//! T1-5.4e — Snapshot test for the `GraphResult::HotPaths` presenter.
//!
//! Estado pre-T1-5.4e:
//!   - T1-5.4b migrated `execute_graph::HotPaths` to return
//!     `GraphResult::HotPaths { path, limit, min_fan_in,
//!     entries, elapsed_ms }`. The text output (a formatted
//!     table with column widths 40/8/8 and an 80-dash
//!     separator) was still emitted by 6 inline println!
//!     calls inside the function.
//!   - The column widths are part of the contract:
//!     `cogh graph hot-paths | column -t` and similar
//!     downstream consumers depend on them. A regression in
//!     the widths (e.g. someone changing 40 to 30 to "look
//!     nicer") would silently break alignment without the
//!     binary complaining.
//!
//! What this RED protects:
//!   - The exact byte sequence of the stdout output: progress
//!     line, blank line, section header, column header with
//!     specific widths, 80-dash separator, data rows, blank,
//!     summary.
//!   - The column widths `{:<40} {:>8} {:>8}  Location` — pin
//!     the exact alignment so a refactor that changes them
//!     fails the test.
//!   - The 80-dash separator `--------------------------------------------------------------------------------`
//!     (80 chars, no spaces). A change to 78 or 82 dashes
//!     breaks grep-based UATs that match the line length.
//!   - The summary uses `entries.len()` (not the unfiltered
//!     `limit`); the analyzer already applied the limit, so
//!     `entries.len()` is the post-filter count.

use cognicode_cli::presenter::present_graph_hotpaths;
use cognicode_core::interface::cli::result::{GraphResult, HotPathEntry};

fn render(result: &GraphResult) -> String {
    let mut buf: Vec<u8> = Vec::new();
    present_graph_hotpaths(result, &mut buf).expect("presenter to Vec<u8> infallible");
    String::from_utf8(buf).expect("UTF-8 text")
}

#[test]
fn snapshot_hotpaths_three_rows() {
    // Three hot paths. The column widths are 40/8/8 for the
    // first three columns; the "Location" column has no width
    // and is just appended after two spaces.
    let result = GraphResult::HotPaths {
        path: "/work/repo".to_string(),
        limit: 10,
        min_fan_in: 1,
        entries: vec![
            HotPathEntry {
                name: "alpha".to_string(),
                fan_in: 12,
                fan_out: 3,
                file: "/lib/foo.rs".to_string(),
                line: 10,
            },
            HotPathEntry {
                name: "beta_with_a_long_name_that_uses_40_chars_or_so".to_string(),
                fan_in: 7,
                fan_out: 2,
                file: "/lib/bar.rs".to_string(),
                line: 42,
            },
            HotPathEntry {
                name: "gamma".to_string(),
                fan_in: 1,
                fan_out: 0,
                file: "/lib/baz.rs".to_string(),
                line: 1,
            },
        ],
        elapsed_ms: 50,
    };
    let out = render(&result);
    // Pin the exact byte sequence. The "Function" column is
    // 40 chars wide left-justified, "Fan-in" and "Fan-out"
    // are 8 chars wide right-justified, and the data rows
    // follow the same alignment. The separator is exactly
    // 80 dashes.
    //
    // The expected string below is the byte-for-byte output
    // of the presenter; the column widths (40/8/8) plus the
    // format-string literal spaces (' ', '  ') between
    // columns produce the exact padding. The two long lines
    // below are 78 and 81 chars wide; the test below this
    // one pins the separator at exactly 80 chars.
    assert_eq!(
        out,
        "Finding hot paths in: /work/repo (limit=10, min_fan_in=1)\n\
         \n\
         Hot paths (most called functions):\n\
         Function                                   Fan-in  Fan-out  Location\n\
         --------------------------------------------------------------------------------\n\
         alpha                                          12        3  /lib/foo.rs:10\n\
         beta_with_a_long_name_that_uses_40_chars_or_so        7        2  /lib/bar.rs:42\n\
         gamma                                           1        0  /lib/baz.rs:1\n\
         \n\
         Found 3 hot paths in 50ms\n",
        "HotPaths text shape changed; check the 40/8/8 column widths and 80-dash separator"
    );
}

#[test]
fn snapshot_hotpaths_empty_filtered_set() {
    // No hot paths match the filter (e.g. min_fan_in too high).
    // The table has a header + separator but no data rows; the
    // summary still says "Found 0 hot paths".
    let result = GraphResult::HotPaths {
        path: "/work/empty".to_string(),
        limit: 10,
        min_fan_in: 999,
        entries: vec![],
        elapsed_ms: 5,
    };
    let out = render(&result);
    assert_eq!(
        out,
        "Finding hot paths in: /work/empty (limit=10, min_fan_in=999)\n\
         \n\
         Hot paths (most called functions):\n\
         Function                                   Fan-in  Fan-out  Location\n\
         --------------------------------------------------------------------------------\n\
         \n\
         Found 0 hot paths in 5ms\n",
        "HotPaths empty-case text changed; check the blank line between separator and summary"
    );
}

#[test]
fn snapshot_hotpaths_separator_is_exactly_80_dashes() {
    // Pin the 80-dash length independently of the full
    // output. A regression to 78 or 82 dashes would change
    // the visual width but might pass a less strict
    // snapshot. This dedicated test makes the contract
    // explicit.
    let result = GraphResult::HotPaths {
        path: "/p".to_string(),
        limit: 1,
        min_fan_in: 1,
        entries: vec![],
        elapsed_ms: 0,
    };
    let out = render(&result);
    // Find the line that is ONLY dashes (and non-empty, so
    // the blank lines between sections don't count as
    // separators).
    let separator_line: Vec<&str> = out
        .lines()
        .filter(|l| !l.is_empty() && l.chars().all(|c| c == '-'))
        .collect();
    assert_eq!(
        separator_line.len(),
        1,
        "Exactly one separator line expected, got {}: {separator_line:?}",
        separator_line.len()
    );
    assert_eq!(
        separator_line[0].len(),
        80,
        "Separator must be exactly 80 dashes wide: got {} chars",
        separator_line[0].len()
    );
}

#[test]
fn snapshot_hotpaths_summary_uses_entries_len_not_limit() {
    // The summary must say "Found N hot paths" where N is
    // entries.len() (the post-filter count), not the `limit`
    // arg (which is the pre-filter upper bound). With 2
    // entries and limit=10, the user sees "Found 2", not
    // "Found 10".
    let result = GraphResult::HotPaths {
        path: "/p".to_string(),
        limit: 10,
        min_fan_in: 1,
        entries: vec![
            HotPathEntry {
                name: "a".to_string(),
                fan_in: 5,
                fan_out: 1,
                file: "/a.rs".to_string(),
                line: 1,
            },
            HotPathEntry {
                name: "b".to_string(),
                fan_in: 3,
                fan_out: 0,
                file: "/b.rs".to_string(),
                line: 1,
            },
        ],
        elapsed_ms: 5,
    };
    let out = render(&result);
    assert!(
        out.contains("Found 2 hot paths"),
        "Summary must use entries.len() (2), not limit (10): {out:?}"
    );
    assert!(
        !out.contains("Found 10 hot paths"),
        "Summary must NOT use the unfiltered limit: {out:?}"
    );
}
