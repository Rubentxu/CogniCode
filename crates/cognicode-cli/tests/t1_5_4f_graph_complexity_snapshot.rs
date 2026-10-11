//! T1-5.4f — Snapshot test for the `GraphResult::Complexity` presenter.
//!
//! Estado pre-T1-5.4f:
//!   - T1-5.4b migrated `execute_graph::Complexity` to return
//!     `GraphResult::Complexity { path, metrics, elapsed_ms }`.
//!     The text output (a 9-metric counting block) was still
//!     emitted by 11 inline println! calls inside the function.
//!   - The 9 metric labels and their fan-out thresholds
//!     ('>=10', '5-9', '<5') are part of the contract: a
//!     consumer that parses the labels would break if a
//!     refactor renamed 'High fan-out' to 'High fan-in' or
//!     changed the '>=' to '>'.
//!
//! What this RED protects:
//!   - The exact byte sequence of the stdout output: progress
//!     line, blank line, section header, 9 indented metric
//!     lines, blank line, summary.
//!   - The 9 metric labels (the test enumerates each one).
//!   - The 2-space indent for each metric line.
//!
//! Note on the fan-out labels: the pre-migration labels use
//! 'fan-out' (functions that call many others), not
//! 'fan-in'. A common mistake in a refactor is to swap them
//! because the two concepts are easy to confuse. The snapshot
//! pins the exact wording.

use cognicode_core::interface::cli::presenter::present_graph_complexity;
use cognicode_core::interface::cli::result::{ComplexityMetrics, GraphResult};

fn render(result: &GraphResult) -> String {
    let mut buf: Vec<u8> = Vec::new();
    present_graph_complexity(result, &mut buf).expect("presenter to Vec<u8> infallible");
    String::from_utf8(buf).expect("UTF-8 text")
}

#[test]
fn snapshot_complexity_typical() {
    // 9 metrics populated. The exact labels and the 2-space
    // indent are part of the contract.
    let result = GraphResult::Complexity {
        path: "/work/repo".to_string(),
        metrics: ComplexityMetrics {
            total_symbols: 100,
            total_edges: 200,
            max_depth: 5,
            cyclomatic_complexity: 42,
            high_fan_out_count: 3,
            medium_fan_out_count: 7,
            low_fan_out_count: 90,
            entry_point_count: 4,
            leaf_function_count: 12,
        },
        elapsed_ms: 60,
    };
    let out = render(&result);
    // Pin the exact byte sequence. Each metric line is
    // "<indent><label>: <value>\n" with 2-space indent and
    // ': ' between label and value.
    assert_eq!(
        out,
        "Calculating complexity metrics for: /work/repo\n\
         \n\
         Complexity Metrics:\n\
         \x20\x20Total symbols: 100\n\
         \x20\x20Total edges: 200\n\
         \x20\x20Max depth: 5\n\
         \x20\x20Cyclomatic complexity: 42\n\
         \x20\x20High fan-out (>=10): 3\n\
         \x20\x20Medium fan-out (5-9): 7\n\
         \x20\x20Low fan-out (<5): 90\n\
         \x20\x20Entry points: 4\n\
         \x20\x20Leaf functions: 12\n\
         \n\
         Analysis completed in 60ms\n",
        "Complexity text shape changed; check the 9 metric labels and the 2-space indent"
    );
}

#[test]
fn snapshot_complexity_zero_metrics() {
    // All metrics zero (an empty repo). The 9 lines still
    // appear with ': 0' — no collapsing to a 'no metrics'
    // message. Pin this so a refactor doesn't drop the
    // zero-value case.
    let result = GraphResult::Complexity {
        path: "/work/empty".to_string(),
        metrics: ComplexityMetrics {
            total_symbols: 0,
            total_edges: 0,
            max_depth: 0,
            cyclomatic_complexity: 0,
            high_fan_out_count: 0,
            medium_fan_out_count: 0,
            low_fan_out_count: 0,
            entry_point_count: 0,
            leaf_function_count: 0,
        },
        elapsed_ms: 1,
    };
    let out = render(&result);
    assert_eq!(
        out,
        "Calculating complexity metrics for: /work/empty\n\
         \n\
         Complexity Metrics:\n\
         \x20\x20Total symbols: 0\n\
         \x20\x20Total edges: 0\n\
         \x20\x20Max depth: 0\n\
         \x20\x20Cyclomatic complexity: 0\n\
         \x20\x20High fan-out (>=10): 0\n\
         \x20\x20Medium fan-out (5-9): 0\n\
         \x20\x20Low fan-out (<5): 0\n\
         \x20\x20Entry points: 0\n\
         \x20\x20Leaf functions: 0\n\
         \n\
         Analysis completed in 1ms\n",
        "Complexity zero-case text changed; check that the 9 labels still appear with ': 0'"
    );
}

#[test]
fn snapshot_complexity_fan_out_labels_are_fan_out_not_fan_in() {
    // The labels use 'fan-out' (functions that CALL many
    // others), not 'fan-in' (functions that are CALLED BY
    // many others). The two are easy to swap in a refactor;
    // a downstream consumer that parses the labels would
    // silently misclassify metrics. Pin the exact wording
    // with a positive assertion and a negative one.
    let result = GraphResult::Complexity {
        path: "/p".to_string(),
        metrics: ComplexityMetrics {
            total_symbols: 1,
            total_edges: 1,
            max_depth: 1,
            cyclomatic_complexity: 1,
            high_fan_out_count: 1,
            medium_fan_out_count: 1,
            low_fan_out_count: 1,
            entry_point_count: 1,
            leaf_function_count: 1,
        },
        elapsed_ms: 1,
    };
    let out = render(&result);
    assert!(
        out.contains("High fan-out (>=10)"),
        "Label must say 'fan-out' (calls many), not 'fan-in' (called by many): {out:?}"
    );
    assert!(
        out.contains("Medium fan-out (5-9)"),
        "Label must say 'fan-out': {out:?}"
    );
    assert!(
        out.contains("Low fan-out (<5)"),
        "Label must say 'fan-out': {out:?}"
    );
    // Belt-and-braces: ensure no fan-in label snuck in.
    assert!(
        !out.contains("fan-in"),
        "No 'fan-in' label expected in Complexity output; got: {out:?}"
    );
}
