//! Characterization tests for the M5 forward-taint engine (pre-e60).
//!
//! These are **black-box** characterizations of `taint_forward`: they pin the
//! behaviour M6 relies on before a `DataflowBackend` imports `TaintPath` as
//! class-B evidence. If they fail, the fix belongs to M5 (`taint_forward`),
//! not to the M6 adapter.
//!
//! See `openspec/changes/e60-lsi-dataflow-backend/exploration-report.md`.

use cognicode_graph_algos::algorithms::{DefUseEdge, taint_forward};

fn edge(from: usize, to: usize, var: &str) -> DefUseEdge {
    DefUseEdge {
        from,
        to,
        variable: var.to_string(),
    }
}

/// T1 — alternate sanitized path.
///
/// ```text
///   source(0) ── sanitizer(1) ── sink(3)
///         └───── clean(2) ──────┘
/// ```
///
/// The taint actually arrives through the clean path. The reconstructed
/// witness must therefore never traverse the sanitizer, even though a shorter
/// topological route through it exists.
#[test]
fn t1_witness_never_traverses_a_sanitizer_when_a_clean_path_exists() {
    let edges = vec![
        edge(0, 1, "t"),
        edge(1, 3, "t"),
        edge(0, 2, "t"),
        edge(2, 3, "t"),
    ];
    let result = taint_forward(&edges, &[0], &[3], &[1]);

    assert_eq!(result.paths.len(), 1, "exactly one (source, sink) pair");
    let path = &result.paths[0];
    assert_eq!(path.source.stmt_id, 0);
    assert_eq!(path.sink.stmt_id, 3);

    let intermediates: Vec<usize> = path.intermediates.iter().map(|s| s.stmt_id).collect();
    assert!(
        !intermediates.contains(&1),
        "the witness must not traverse the sanitizer (node 1); got {intermediates:?}"
    );
    assert!(
        intermediates.contains(&2),
        "the witness should traverse the clean path (node 2); got {intermediates:?}"
    );
}

/// T2 — unequal-depth multi-source merge.
///
/// ```text
///   sourceA(0) ──────────────────────┐
///                                    merge(2) ── mid(3) ── sink(4)
///   sourceB(1) ── x(5) ── y(6) ──────┘
/// ```
///
/// `sourceB` reaches the merge node strictly later than `sourceA`. Both
/// origins must propagate through the merge to the sink, so both
/// `(sourceA, sink)` and `(sourceB, sink)` paths must be reported.
#[test]
fn t2_second_origin_propagates_through_an_already_tainted_merge() {
    let edges = vec![
        edge(0, 2, "a"),
        edge(1, 5, "b"),
        edge(5, 6, "b"),
        edge(6, 2, "b"),
        edge(2, 3, "m"),
        edge(3, 4, "m"),
    ];
    let result = taint_forward(&edges, &[0, 1], &[4], &[]);

    let pairs: Vec<(usize, usize)> = result
        .paths
        .iter()
        .map(|p| (p.source.stmt_id, p.sink.stmt_id))
        .collect();
    assert!(
        pairs.contains(&(0, 4)),
        "expected (sourceA, sink) to be reported; got {pairs:?}"
    );
    assert!(
        pairs.contains(&(1, 4)),
        "expected (sourceB, sink) to be reported; got {pairs:?}"
    );
}
