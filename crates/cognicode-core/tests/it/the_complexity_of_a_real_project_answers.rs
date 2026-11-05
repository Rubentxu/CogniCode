//! The complexity of a real project answers, in a bounded time.
//!
//! ## The guarantee
//!
//! `CallGraphAnalyzer::calculate_complexity` is on the path of three
//! consumer routes: the `Analyze` port (`ServiceAnalyzer`), the workspace
//! session and the CLI's `graph complexity`. It answered for a fixture
//! and never for a project: the depth it published was the longest of the
//! chains a depth-first search happened to walk into, and it looked for
//! one chain per symbol **and** per entry point — 725 million searches
//! over the graph of this repository's own core crate (measured: it did
//! not answer in 20 s).
//!
//! A product that does not answer is a product that hangs its consumer.
//!
//! ## The witness
//!
//! A fixture cannot show this: it is small, and the old code answered
//! for it. The witness reads the **real code** of this repository — the
//! `domain` of the core crate — and asks the two questions a consumer
//! asks:
//!
//! 1. does the complexity of a real project answer within a deadline?
//! 2. does the route the consumer reaches publish that answer?
//!
//! The deadline is what turns a hang into a failure with a message,
//! instead of a test that never finishes.
//!
//! ## What it does not assert
//!
//! It does not assert a number. The depth of a real project moves with
//! the repository's own churn, and a test that pins it would break on
//! every commit that touches the code it reads. What it asserts is that
//! the product **answers**, and that the answer it publishes is the one
//! it measured.

use cognicode_core::application::analysis_basis::{
    AnalysisBasis, AnalysisOp, AnalysisValue, Analyze, GraphValue, OutcomeState,
};
use cognicode_core::application::consumer_analysis::ServiceAnalyzer;
use cognicode_core::application::services::analysis_service::AnalysisService;
use cognicode_core::domain::services::CallGraphAnalyzer;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

/// The real code the witness reads: the domain of the core crate.
///
/// Real product code, and big enough that the code which never finished
/// did not finish over it either (measured: 5.932 symbols, 5.319 entry
/// points; the old code did not answer in 20 s).
fn real_code() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/domain")
}

/// How long the complexity of a real project may take.
///
/// The old code did not answer in 20 s over this very graph, so the
/// deadline is far below what the defect costs and far above what the
/// answer takes (measured: 32 ms).
const DEADLINE: Duration = Duration::from_secs(10);

/// How long the consumer route may take.
///
/// The route builds the graph before it measures it, so its deadline
/// carries the build (measured: 0,9 s over this subtree) on top of the
/// measurement.
const ROUTE_DEADLINE: Duration = Duration::from_secs(60);

/// The complexity of a real project answers, and it answers in time.
#[test]
fn the_complexity_of_a_real_project_answers_within_a_deadline() {
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

    let analyzer = CallGraphAnalyzer::new();
    let (tx, rx) = mpsc::channel();
    let owned = graph.clone();
    std::thread::spawn(move || {
        let _ = tx.send(analyzer.calculate_complexity(&owned));
    });

    let report = rx.recv_timeout(DEADLINE).unwrap_or_else(|_| {
        panic!(
            "the complexity of a real project did not answer in {DEADLINE:?}: \
             the consumer that asked waits for ever"
        )
    });

    // The report describes the graph it was asked about.
    assert_eq!(
        report.total_symbols,
        graph.symbol_count(),
        "the report counts symbols the graph does not hold"
    );
    assert!(
        report.max_depth >= 1,
        "a real project has a chain of calls, and the report says it has none"
    );
    assert!(
        report.entry_point_count > 0,
        "a real project has entry points, and the report says it has none"
    );
}

/// The route the consumer reaches publishes the depth of a real project.
///
/// `ServiceAnalyzer` is the production adapter of the `Analyze` port: it
/// is what the CLI and the SDK ask. The depth it publishes is the one
/// the analyzer measured — not a number it made up, and not a silence.
#[test]
fn the_consumer_route_publishes_the_depth_of_a_real_project() {
    let path = real_code();
    let basis = AnalysisBasis::for_workspace(path.clone());
    let analyzer = ServiceAnalyzer::new();

    // The route carries its own deadline: it is a synchronous call, and a
    // route that never answers would hold the whole suite instead of
    // failing with a message.
    let (tx, rx) = mpsc::channel();
    let workspace = path.clone();
    std::thread::spawn(move || {
        let _ = tx.send(analyzer.execute(&basis, AnalysisOp::GraphComplexity { path: workspace }));
    });

    let outcome = rx.recv_timeout(ROUTE_DEADLINE).unwrap_or_else(|_| {
        panic!(
            "the route a consumer reaches did not answer in {ROUTE_DEADLINE:?} \
             for a real project"
        )
    });

    match outcome.state() {
        OutcomeState::Complete | OutcomeState::Partial => {}
        other => panic!("the complexity of a real project did not answer: {other:?}"),
    }

    let Some(AnalysisValue::Graph(GraphValue::Complexity { metrics, .. })) = outcome.value() else {
        panic!("the route published another value");
    };

    let depth = metrics["max_depth"]
        .as_u64()
        .unwrap_or_else(|| panic!("the route published no depth: {metrics}"));
    assert!(
        depth >= 1,
        "the route published a depth of {depth}: a real project has a chain of calls"
    );
}
