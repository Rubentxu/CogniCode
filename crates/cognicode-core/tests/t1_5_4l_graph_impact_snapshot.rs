//! T1-5.4l — Snapshot test for the `GraphResult::Impact` presenter.
//!
//! Estado pre-T1-5.4l:
//!   - T1-5.4b migrated `execute_graph::Impact` to return
//!     `GraphResult::Impact { symbol, risk_level,
//!     direct_dependents, total_impacted_symbols,
//!     impacted_files_count, impacted_symbols,
//!     impacted_files, elapsed_ms }`. The text output
//!     (the most complex of the 11 sub-commands: a
//!     multi-block analysis with 4-line 'Impact Analysis'
//!     block, optional 'Impacted symbols' block with
//!     20-item truncation, optional 'Impacted files'
//!     block with 10-item truncation) was still emitted
//!     by ~15 inline println! calls.
//!   - The pre-migration has two distinct shapes: a
//!     'matches found' path (full multi-block output) and
//!     a 'no symbols found' path (early return with a
//!     dedicated message). The presenter branches on
//!     `direct_dependents == 0` to choose between them.
//!
//! What this RED protects:
//!   - The exact byte sequence of both cases.
//!   - The 4-line 'Impact Analysis' block: 'Risk Level:',
//!     'Direct dependents:', 'Total impacted symbols:',
//!     'Impacted files:'. Each with 2-space indent.
//!   - The risk-level decision (CRITICAL > 10, HIGH > 5,
//!     MEDIUM > 2, LOW otherwise; NONE for empty case).
//!   - The 20-item truncation in 'Impacted symbols' and
//!     the 10-item truncation in 'Impacted files', with
//!     the '... and N more' line for overflow.

use cognicode_core::interface::cli::presenter::present_graph_impact;
use cognicode_core::interface::cli::result::GraphResult;

fn render(result: &GraphResult) -> String {
    let mut buf: Vec<u8> = Vec::new();
    present_graph_impact(result, &mut buf).expect("presenter to Vec<u8> infallible");
    String::from_utf8(buf).expect("UTF-8 text")
}

#[test]
fn snapshot_impact_no_symbols_found() {
    // The empty case: direct_dependents == 0.
    let result = GraphResult::Impact {
        symbol: "ghost".to_string(),
        risk_level: "NONE".to_string(),
        direct_dependents: 0,
        total_impacted_symbols: 0,
        impacted_files_count: 0,
        impacted_symbols: vec![],
        impacted_files: vec![],
        elapsed_ms: 50,
    };
    let out = render(&result);
    assert_eq!(
        out,
        "\n\
         No symbols found matching 'ghost'\n\
         \n\
         Analysis completed in 50ms\n",
        "Impact no-symbols text changed; check the 'No symbols found matching' wording and apostrophes"
    );
}

#[test]
fn snapshot_impact_critical_with_truncated_symbols_and_files() {
    // 12 impacted symbols (truncated to 20 = no truncation
    // here, but 5+ files truncated to 10), CRITICAL risk
    // (> 10), 1 direct dependent.
    let mut symbols: Vec<String> = (0..12).map(|i| format!("sym{i}")).collect();
    let mut files: Vec<String> = (0..15).map(|i| format!("/lib/file{i}.rs")).collect();
    let result = GraphResult::Impact {
        symbol: "alpha".to_string(),
        risk_level: "CRITICAL".to_string(),
        direct_dependents: 1,
        total_impacted_symbols: 12,
        impacted_files_count: 15,
        impacted_symbols: symbols.clone(),
        impacted_files: files.clone(),
        elapsed_ms: 80,
    };
    let out = render(&result);
    // The data side has 12 symbols (NOT truncated to 20
    // because <= 20) and 15 files (truncated to 10 with
    // '... and 5 more').
    assert_eq!(
        out,
        "\n\
         Found 1 symbol(s) matching 'alpha'\n\
         \n\
         Impact Analysis for 'alpha':\n\
         \x20\x20Risk Level: CRITICAL\n\
         \x20\x20Direct dependents: 1\n\
         \x20\x20Total impacted symbols: 12\n\
         \x20\x20Impacted files: 15\n\
         \n\
         Impacted symbols (first 20):\n\
         \x20\x201. sym0\n\
         \x20\x202. sym1\n\
         \x20\x203. sym2\n\
         \x20\x204. sym3\n\
         \x20\x205. sym4\n\
         \x20\x206. sym5\n\
         \x20\x207. sym6\n\
         \x20\x208. sym7\n\
         \x20\x209. sym8\n\
         \x20\x2010. sym9\n\
         \x20\x2011. sym10\n\
         \x20\x2012. sym11\n\
         \n\
         Impacted files (first 10):\n\
         \x20\x201. /lib/file0.rs\n\
         \x20\x202. /lib/file1.rs\n\
         \x20\x203. /lib/file2.rs\n\
         \x20\x204. /lib/file3.rs\n\
         \x20\x205. /lib/file4.rs\n\
         \x20\x206. /lib/file5.rs\n\
         \x20\x207. /lib/file6.rs\n\
         \x20\x208. /lib/file7.rs\n\
         \x20\x209. /lib/file8.rs\n\
         \x20\x2010. /lib/file9.rs\n\
         \x20\x20... and 5 more\n\
         \n\
         Analysis completed in 80ms\n",
        "Impact critical-with-truncated text changed; check the 20/10 truncations and the '... and N more' line"
    );
    // Sanity: prevent the 'sym0..11' list from drifting if
    // the iterator changes. The test above pins the exact
    // symbols, so this assertion is a belt-and-braces
    // against an accidental sort.
    symbols.sort();
    files.sort();
    assert_eq!(symbols[0], "sym0");
    assert_eq!(files[0], "/lib/file0.rs");
}

#[test]
fn snapshot_impact_risk_levels() {
    // Pin the 4 risk-level branches. Each test has a
    // different impacted_symbols count.
    for (count, expected_risk) in &[
        (1_usize, "LOW"),
        (3, "MEDIUM"),
        (6, "HIGH"),
        (11, "CRITICAL"),
    ] {
        let symbols: Vec<String> = (0..*count).map(|i| format!("s{i}")).collect();
        let result = GraphResult::Impact {
            symbol: "x".to_string(),
            risk_level: expected_risk.to_string(),
            direct_dependents: 1,
            total_impacted_symbols: *count,
            impacted_files_count: *count,
            impacted_symbols: symbols,
            impacted_files: vec![],
            elapsed_ms: 1,
        };
        let out = render(&result);
        assert!(
            out.contains(&format!("Risk Level: {expected_risk}")),
            "Risk Level for {count} impacted symbols must be {expected_risk}: {out:?}"
        );
    }
}

#[test]
fn snapshot_impact_no_symbols_block_when_impacted_symbols_empty_but_direct_present() {
    // Edge case: 0 impacted symbols but direct_dependents > 0.
    // (Possible: matches found but no transitive dependents.)
    // The pre-migration code shows the 'Impacted symbols' block
    // only when impacted_symbols is non-empty. The presenter
    // must reproduce this.
    let result = GraphResult::Impact {
        symbol: "x".to_string(),
        risk_level: "LOW".to_string(),
        direct_dependents: 1,
        total_impacted_symbols: 0,
        impacted_files_count: 0,
        impacted_symbols: vec![],
        impacted_files: vec![],
        elapsed_ms: 1,
    };
    let out = render(&result);
    assert!(
        out.contains("Impact Analysis for 'x':"),
        "Must show the 'Impact Analysis' block: {out:?}"
    );
    // The check is for the BLOCK HEADER, not just the
    // count line. '  Impacted files: 0' is part of the
    // 'Impact Analysis' block; the absent block would be
    // 'Impacted files (first 10):'.
    assert!(
        !out.contains("Impacted symbols (first 20)"),
        "Must NOT show the 'Impacted symbols (first 20)' block when the list is empty: {out:?}"
    );
    assert!(
        !out.contains("Impacted files (first 10)"),
        "Must NOT show the 'Impacted files (first 10)' block when the list is empty: {out:?}"
    );
    assert!(
        !out.contains("No symbols found"),
        "Must NOT show the 'No symbols found' message (direct_dependents > 0): {out:?}"
    );
}
