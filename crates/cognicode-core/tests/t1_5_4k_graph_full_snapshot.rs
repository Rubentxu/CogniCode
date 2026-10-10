//! T1-5.4k — Snapshot test for the `GraphResult::Full` presenter.
//!
//! Estado pre-T1-5.4k:
//!   - T1-5.4b migrated `execute_graph::Full` to return
//!     `GraphResult::Full { path, rebuild, elapsed_ms,
//!     symbols, dependencies, status, skipped_files }`.
//!     The text output was still emitted by 4 inline
//!     println! calls in the function (progress + 3 result
//!     lines), with the Skipped lines going to eprintln!
//!     (stderr side-channel).
//!   - The json branch (commands.rs:944..983) builds an
//!     ad-hoc `FullGraphJson` struct with a `schema_version`
//!     field; this is preserved in this commit and is the
//!     one remaining in-function text-formatting call.
//!     T1-5.5 will consolidate the json dispatch through
//!     the GraphResult enum.
//!
//! What this RED protects:
//!   - The exact byte sequence of the stdout output:
//!     progress line (with optional ' (rebuild)' suffix),
//!     timing line, two indented count lines.
//!   - The ' (rebuild)' suffix is conditional on the
//!     `rebuild` field. Pin the positive and negative cases.
//!   - The Skipped lines are NOT in the presenter's output
//!     (they stay as eprintln! in the function — same
//!     pattern as the 'Warning: graph is PARTIAL' in other
//!     sub-commands).
//!   - The json branch in commands.rs is NOT exercised by
//!     this snapshot test; it has its own integration test
//!     coverage via the existing `prf_cli_*` UAT suite.

use cognicode_core::interface::cli::presenter::present_graph_full;
use cognicode_core::interface::cli::result::{GraphResult, SkippedFile};

fn render(result: &GraphResult) -> String {
    let mut buf: Vec<u8> = Vec::new();
    present_graph_full(result, &mut buf).expect("presenter to Vec<u8> infallible");
    String::from_utf8(buf).expect("UTF-8 text")
}

#[test]
fn snapshot_full_rebuild_with_skipped() {
    // Rebuild=true, 1 skipped file. The ' (rebuild)' suffix
    // appears; the 'Skipped: ...' line is NOT in the
    // presenter's output (it goes to stderr in commands.rs).
    let result = GraphResult::Full {
        path: "/work/repo".to_string(),
        rebuild: true,
        elapsed_ms: 1234,
        symbols: 500,
        dependencies: 1200,
        status: "partial".to_string(),
        skipped_files: vec![SkippedFile {
            path: "/work/repo/bad.rs".to_string(),
            reason: "ParseError".to_string(),
        }],
    };
    let out = render(&result);
    assert_eq!(
        out,
        "Building full project graph at: /work/repo (rebuild)\n\
         Full graph built in 1234ms\n\
         \x20\x20Total symbols: 500\n\
         \x20\x20Total dependencies: 1200\n",
        "Full rebuild-with-skipped text changed; check the '(rebuild)' suffix and the 2-space indent"
    );
    // Belt-and-braces: the Skipped line is NOT in the
    // stdout output.
    assert!(
        !out.contains("Skipped:"),
        "Skipped lines go to stderr; the presenter's stdout must NOT contain 'Skipped:': {out:?}"
    );
}

#[test]
fn snapshot_full_no_rebuild_no_skipped() {
    // Rebuild=false, no skipped files. No ' (rebuild)' suffix.
    let result = GraphResult::Full {
        path: "/work/repo".to_string(),
        rebuild: false,
        elapsed_ms: 800,
        symbols: 100,
        dependencies: 200,
        status: "complete".to_string(),
        skipped_files: vec![],
    };
    let out = render(&result);
    assert_eq!(
        out,
        "Building full project graph at: /work/repo\n\
         Full graph built in 800ms\n\
         \x20\x20Total symbols: 100\n\
         \x20\x20Total dependencies: 200\n",
        "Full no-rebuild text changed; check the absence of '(rebuild)' suffix"
    );
    assert!(
        !out.contains("(rebuild)"),
        "rebuild=false must NOT emit the '(rebuild)' suffix: {out:?}"
    );
}

#[test]
fn snapshot_full_rebuild_flag_drives_parenthetical_suffix() {
    // Belt-and-braces: pin the 'rebuild' field's effect on
    // the progress line. The pre-migration code uses
    // 'if *rebuild { " (rebuild)" } else { "" }' to decide.
    // A refactor that drops the conditional would emit
    // ' (rebuild)' always, or never.
    let r_true = GraphResult::Full {
        path: "/p".to_string(),
        rebuild: true,
        elapsed_ms: 1,
        symbols: 0,
        dependencies: 0,
        status: "complete".to_string(),
        skipped_files: vec![],
    };
    let r_false = GraphResult::Full {
        path: "/p".to_string(),
        rebuild: false,
        elapsed_ms: 1,
        symbols: 0,
        dependencies: 0,
        status: "complete".to_string(),
        skipped_files: vec![],
    };
    let out_true = render(&r_true);
    let out_false = render(&r_false);
    assert!(
        out_true.contains("(rebuild)"),
        "rebuild=true must emit '(rebuild)': {out_true:?}"
    );
    assert!(
        !out_false.contains("(rebuild)"),
        "rebuild=false must NOT emit '(rebuild)': {out_false:?}"
    );
}
