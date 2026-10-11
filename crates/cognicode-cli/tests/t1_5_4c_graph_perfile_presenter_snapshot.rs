//! T1-5.4c — Snapshot test for the `GraphResult::PerFile` presenter.
//!
//! Estado pre-T1-5.4c:
//!   - T1-5.4b migrated `execute_graph::PerFile` to return
//!     `GraphResult::PerFile { file, symbols, dependencies }`.
//!     The text output was still emitted by the 3 inline
//!     `println!` calls in the function (commands.rs:860..862),
//!     plus a progress line at line 851.
//!   - Like T1-5.3c, the text was unreachable from a `tests/`
//!     integration test. A regression in the label, the
//!     indentation, or the count format would only surface
//!     when a human noticed the diff in `cogh graph per-file`
//!     output.
//!
//! What this RED protects:
//!   - The exact byte sequence emitted by the PerFile text path:
//!     the progress line "Getting per-file graph for: {file}\n",
//!     the result header "Per-file graph for {file}:\n", and
//!     the two indented counts "  Symbols: {n}\n" and
//!     "  Dependencies: {n}\n". A regression in any of them
//!     breaks the test and forces the author to consciously
//!     update the snapshot.
//!   - The four lines are NOT separated by blank lines and the
//!     result block has NO trailing newline beyond the per-line
//!     "\n" terminators. The total byte stream is 4*line_length
//!     with no extra characters between lines.
//!   - The format used for `symbols` and `dependencies` is
//!     decimal (matches the pre-migration `{}` formatter).
//!
//! Why snapshot and not golden-file (insta, etc.):
//!   - Same reason as `t1_5_3d_index_presenter_snapshot.rs`:
//!     the project has no snapshot crate, and the strings are
//!     short enough to live inline. A reviewer can verify them
//!     by reading the assertion.
//!
//! Lo que T1-5.4c NO hace (y queda para T1-5.4d..m):
//!   - The other 10 GraphResult variants (OnDemand, Full,
//!     HotPaths, EntryPoints, LeafFunctions, TracePathFound/
//!     NotFound, Mermaid, Hierarchy, Complexity, Impact) have
//!     no presenter yet; their text output is still in the
//!     function. T1-5.4d..m follows the same flow: add a
//!     presenter, replace the inline println! block, snapshot.

use cognicode_core::interface::cli::presenter::present_graph_perfile;
use cognicode_core::interface::cli::result::GraphResult;

fn render(result: &GraphResult) -> String {
    let mut buf: Vec<u8> = Vec::new();
    present_graph_perfile(result, &mut buf)
        .expect("presenter write to Vec<u8> is infallible");
    String::from_utf8(buf).expect("presenter emits UTF-8 text")
}

#[test]
fn snapshot_perfile_typical_counts() {
    // The pilot: file=/lib/foo.rs, 7 symbols, 3 dependencies.
    // The pre-migration output (commands.rs:851 + 860..862) is:
    //   Getting per-file graph for: /lib/foo.rs
    //   Per-file graph for /lib/foo.rs:
    //     Symbols: 7
    //     Dependencies: 3
    let result = GraphResult::PerFile {
        file: "/lib/foo.rs".to_string(),
        symbols: 7,
        dependencies: 3,
    };
    let out = render(&result);
    assert_eq!(
        out,
        "Getting per-file graph for: /lib/foo.rs\n\
         Per-file graph for /lib/foo.rs:\n\
         \x20\x20Symbols: 7\n\
         \x20\x20Dependencies: 3\n",
        "PerFile variant text shape changed; update this test and any user-facing doc"
    );
}

#[test]
fn snapshot_perfile_zero_counts() {
    // The zero case is a real outcome (an empty file with no
    // imports) and must NOT collapse to a "no symbols found"
    // message — PerFile is a counting command, not a search.
    // Pin the exact "Symbols: 0" / "Dependencies: 0" form.
    let result = GraphResult::PerFile {
        file: "/lib/empty.rs".to_string(),
        symbols: 0,
        dependencies: 0,
    };
    let out = render(&result);
    assert_eq!(
        out,
        "Getting per-file graph for: /lib/empty.rs\n\
         Per-file graph for /lib/empty.rs:\n\
         \x20\x20Symbols: 0\n\
         \x20\x20Dependencies: 0\n",
        "PerFile zero-counts text changed; a 'no symbols' message may have been added"
    );
}

#[test]
fn snapshot_perfile_large_counts_use_decimal_format() {
    // The pre-migration `{}` formatter uses decimal. A change
    // to hex or scientific notation would break any downstream
    // consumer that parses the counts (e.g. a `cogh | jq` pipe).
    // Pin the decimal form with a count that would round-trip
    // differently in hex (12345 decimal != 0x3039).
    let result = GraphResult::PerFile {
        file: "/lib/big.rs".to_string(),
        symbols: 12345,
        dependencies: 67890,
    };
    let out = render(&result);
    assert!(
        out.contains("Symbols: 12345"),
        "Symbols must use decimal format: {out:?}"
    );
    assert!(
        out.contains("Dependencies: 67890"),
        "Dependencies must use decimal format: {out:?}"
    );
    // Belt-and-braces: confirm no hex prefix leaked.
    assert!(
        !out.contains("0x"),
        "Decimal format expected; got hex prefix in: {out:?}"
    );
}

#[test]
fn snapshot_perfile_no_trailing_blank_line() {
    // The 4-line block ends with "Dependencies: N\n" — no extra
    // blank line after. A consumer that splits on "\n\n" to
    // detect record boundaries depends on this; an extra
    // trailing "\n" would make the consumer see a phantom
    // empty record.
    let result = GraphResult::PerFile {
        file: "/lib/foo.rs".to_string(),
        symbols: 1,
        dependencies: 1,
    };
    let out = render(&result);
    assert!(
        !out.ends_with("\n\n"),
        "PerFile output must not end with a blank line; got: {out:?}"
    );
    // The last line is the dependencies count, with exactly
    // one trailing newline.
    assert!(
        out.ends_with("Dependencies: 1\n"),
        "PerFile output must end with the dependencies line: {out:?}"
    );
}
