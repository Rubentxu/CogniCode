//! The feedback set of a real project answers, in a bounded time.
//!
//! ## The guarantee
//!
//! `AnalysisService::find_minimal_feedback_set` answers the question
//! "which symbols, if they were gone, would leave this graph without
//! cycles?". It did not answer for a project: it cloned the graph and
//! ran the cycle detection once per candidate, and it looked for the
//! candidate that left the *most* cycles alive — the opposite of what
//! the name says (measured: it did not answer in 20 s over the domain
//! of this repository, and the set it did return held symbols that break
//! no cycle at all).
//!
//! ## The witness
//!
//! A fixture cannot show this: it is small, and the old code answered
//! for it. The witness reads the **real code** of this repository — the
//! `domain` of the core crate — and asks the two questions that matter:
//!
//! 1. does the feedback set of a real project answer within a deadline?
//! 2. does the set it returns leave the graph without cycles?
//!
//! The deadline is what turns a hang into a failure with a message,
//! instead of a test that never finishes.
//!
//! ## What it does not assert
//!
//! It does not assert how many symbols the set holds: that number moves
//! with the repository's own churn, and a test that pins it would break
//! on every commit that touches the code it reads. What it asserts is
//! that the product **answers**, and that the answer it gives is one
//! that breaks every cycle.

use cognicode_core::application::services::analysis_service::AnalysisService;
use cognicode_core::domain::services::CycleDetector;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

/// The real code the witness reads: the domain of the core crate.
///
/// Real product code, and big enough that the code which never finished
/// did not finish over it either (measured 2026-11-05: 5.971 symbols, 62
/// cycles; the old code did not answer in 20 s).
fn real_code() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/domain")
}

/// How long the feedback set of a real project may take.
///
/// The old code did not answer in 20 s over this very graph, so the
/// deadline is far below what the defect costs and far above what the
/// answer takes (measured 2026-11-05: 155 ms).
const DEADLINE: Duration = Duration::from_secs(10);

/// The feedback set of a real project answers, and it answers in time.
#[test]
fn the_feedback_set_of_a_real_project_answers_within_a_deadline() {
    let service = AnalysisService::new();
    service
        .build_project_graph(&real_code())
        .expect("the product graph is built over real code");
    let graph = service.get_project_graph();

    // The witness is real code. A graph of a handful of symbols answers
    // even with the code that never finished, so a witness that shrank
    // into a fixture would stop proving anything.
    assert!(
        graph.symbol_count() > 1_000,
        "the witness reads {} symbols: a graph that small answers even with the code that hung",
        graph.symbol_count()
    );

    let (tx, rx) = mpsc::channel();
    let owned = graph.clone();
    std::thread::spawn(move || {
        let _ = tx.send(service.find_minimal_feedback_set(&owned));
    });

    let chosen = rx.recv_timeout(DEADLINE).unwrap_or_else(|_| {
        panic!(
            "the feedback set of a real project did not answer in {DEADLINE:?}: \
             the caller waits for ever"
        )
    });

    // The set answers the question it is asked: with those symbols gone,
    // the graph holds no cycle any more.
    let mut without = graph.as_ref().clone();
    for symbol in &chosen {
        without.remove_symbol(symbol);
    }
    assert!(
        !CycleDetector::new().detect_cycles(&without).has_cycles,
        "the set the product returned leaves a cycle alive: {chosen:?}"
    );
}
