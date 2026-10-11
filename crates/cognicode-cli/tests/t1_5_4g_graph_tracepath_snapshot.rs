//! T1-5.4g — Snapshot test for the `GraphResult::TracePathFound`
//! and `GraphResult::TracePathNotFound` presenters.
//!
//! Estado pre-T1-5.4g:
//!   - T1-5.4b migrated `execute_graph::TracePath` to return
//!     either `GraphResult::TracePathFound { from, to, hops,
//!     elapsed_ms }` or `GraphResult::TracePathNotFound { from,
//!     to, elapsed_ms }`. The text output was still emitted
//!     by inline println! calls inside the function.
//!   - The hop numbering is 1-indexed (pre-migration uses
//!     `i + 1` in the format string). The data side carries
//!     `position: (i+1) as u32` so consumers don't have to
//!     re-compute. A regression to 0-indexed hops would
//!     shift every position by 1.
//!
//! What this RED protects:
//!   - The exact byte sequence of the result block +
//!     summary (the progress line is NOT in the snapshot —
//!     see the 'progress line in the function' note in
//!     presenter.rs).
//!   - The 1-indexed `position` field in PathHop (pin the
//!     '1.', '2.', '3.' prefix; a 0-indexed refactor would
//!     produce '0.', '1.', '2.' and the test catches it).
//!   - The "No path found between '{from}' and '{to}'"
//!     wording (the apostrophes are part of the contract).
//!
//! Lo que T1-5.4g NO testea:
//!   - El 'Warning: graph is PARTIAL' eprintln! a stderr
//!     (decisión registrada en
//!     odd/tasks/t1-5-4-decision-partial-warning.md).
//!   - El 'Tracing path from X to Y in: PATH' progress line:
//!     está en commands.rs como println!, no en el
//!     presenter. La razón es que la variant no carga el
//!     project path (es side-channel en la función; el data
//!     side solo necesita from/to/hops). T1-5.5 decidirá
//!     si moverlo a tracing::info! o dejarlo en commands.rs.

use cognicode_cli::presenter::{present_graph_tracepath_found, present_graph_tracepath_notfound};
use cognicode_core::interface::cli::result::{GraphResult, PathHop};

fn render_found(result: &GraphResult) -> String {
    let mut buf: Vec<u8> = Vec::new();
    present_graph_tracepath_found(result, &mut buf).expect("presenter to Vec<u8> infallible");
    String::from_utf8(buf).expect("UTF-8 text")
}

fn render_notfound(result: &GraphResult) -> String {
    let mut buf: Vec<u8> = Vec::new();
    present_graph_tracepath_notfound(result, &mut buf).expect("presenter to Vec<u8> infallible");
    String::from_utf8(buf).expect("UTF-8 text")
}

#[test]
fn snapshot_tracepath_found_three_hops() {
    // 3 hops, each with a 1-indexed position.
    let result = GraphResult::TracePathFound {
        from: "alpha".to_string(),
        to: "gamma".to_string(),
        hops: vec![
            PathHop {
                position: 1,
                name: "alpha".to_string(),
                file: "/lib/a.rs".to_string(),
                line: 1,
            },
            PathHop {
                position: 2,
                name: "beta".to_string(),
                file: "/lib/b.rs".to_string(),
                line: 5,
            },
            PathHop {
                position: 3,
                name: "gamma".to_string(),
                file: "/lib/c.rs".to_string(),
                line: 10,
            },
        ],
        elapsed_ms: 8,
    };
    let out = render_found(&result);
    // The result block + summary (progress line is in the
    // function, not in the presenter).
    assert_eq!(
        out,
        "\n\
         Path found (3 hops):\n\
         \x20\x201. alpha at /lib/a.rs:1\n\
         \x20\x202. beta at /lib/b.rs:5\n\
         \x20\x203. gamma at /lib/c.rs:10\n\
         \n\
         Trace completed in 8ms\n",
        "TracePathFound text shape changed; check the '1.', '2.', '3.' 1-indexed prefix and 'Path found (N hops):' header"
    );
}

#[test]
fn snapshot_tracepath_found_single_hop() {
    // A direct call: 1 hop. The 'Path found (1 hops):' line
    // uses the plural form even for 1 hop (matches the
    // pre-migration "{N} hops" wording).
    let result = GraphResult::TracePathFound {
        from: "alpha".to_string(),
        to: "beta".to_string(),
        hops: vec![PathHop {
            position: 1,
            name: "beta".to_string(),
            file: "/lib/b.rs".to_string(),
            line: 1,
        }],
        elapsed_ms: 1,
    };
    let out = render_found(&result);
    assert_eq!(
        out,
        "\n\
         Path found (1 hops):\n\
         \x20\x201. beta at /lib/b.rs:1\n\
         \n\
         Trace completed in 1ms\n",
        "TracePathFound single-hop text changed; check '1 hops' (plural) wording"
    );
}

#[test]
fn snapshot_tracepath_notfound() {
    let result = GraphResult::TracePathNotFound {
        from: "alpha".to_string(),
        to: "ghost".to_string(),
        elapsed_ms: 3,
    };
    let out = render_notfound(&result);
    assert_eq!(
        out,
        "\n\
         No path found between 'alpha' and 'ghost'\n\
         \n\
         Trace completed in 3ms\n",
        "TracePathNotFound text shape changed; check 'No path found between' wording and apostrophes"
    );
}

#[test]
fn snapshot_tracepath_hops_are_1indexed_not_0indexed() {
    // Belt-and-braces: pin the 1-indexed prefix explicitly.
    // A 0-indexed refactor (e.g. someone uses `i` instead of
    // `i + 1` in the data side) would produce '0. alpha',
    // '1. beta', '2. gamma' — and this test catches it.
    let result = GraphResult::TracePathFound {
        from: "alpha".to_string(),
        to: "gamma".to_string(),
        hops: vec![
            PathHop {
                position: 1,
                name: "alpha".to_string(),
                file: "/lib/a.rs".to_string(),
                line: 1,
            },
            PathHop {
                position: 2,
                name: "gamma".to_string(),
                file: "/lib/c.rs".to_string(),
                line: 10,
            },
        ],
        elapsed_ms: 1,
    };
    let out = render_found(&result);
    assert!(
        out.contains("\n  1. alpha at"),
        "First hop must be '1. alpha' (1-indexed), not '0. alpha': {out:?}"
    );
    assert!(
        out.contains("\n  2. gamma at"),
        "Second hop must be '2. gamma', not '1. gamma': {out:?}"
    );
    assert!(
        !out.contains("0. alpha"),
        "0-indexed prefix detected; data side must use 1-indexed positions: {out:?}"
    );
}
