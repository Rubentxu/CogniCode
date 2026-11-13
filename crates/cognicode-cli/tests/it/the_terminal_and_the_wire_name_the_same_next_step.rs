//! The terminal and the wire say the same thing about the same answer.
//!
//! ## The gap
//!
//! A graph build reaches a person through two routes that build their own
//! JSON of the answer:
//!
//! - **the wire**: `BuildGraphOutput`, serialised by the handler, wrapped in
//!   the envelope by `success_result`;
//! - **the terminal**: `GraphResult::Full`, projected by the CLI presenter
//!   into a smaller JSON of its own (`rejected_relations`, where the wire
//!   says `rejected_relationships`) and wrapped by the same `envelope_for`.
//!
//! The envelope reader tolerates both spellings, so nothing failed when the
//! two projections drifted apart. The existing equivalence suites compare the
//! *counts* the two routes publish; none of them compared what each route
//! tells the reader to do next, which is the part an agent acts on.
//!
//! ## What this file pins
//!
//! One `AnalysisOutcome` goes through both routes. The commands and the
//! reasons the terminal prints under "Siguiente:" are exactly the ones the
//! wire publishes as links. A hole counted on one side and missed on the
//! other changes the reason, so the comparison fails on the first drift.
//!
//! The outcome is a value: no process, no binary, no network, no clock.

use cognicode_cli::cli::commands::Format as CliFormat;
use cognicode_cli::cli::follow_ups;
use cognicode_cli::cli::presenter_bridge::present_graph_full;
use cognicode_cli::cli::result::CommandOutput;
use cognicode_cli::presenter::present_output;
use cognicode_core::application::analysis_basis::{
    AnalysisError, AnalysisOutcome, AnalysisValue, GraphValue,
};
use cognicode_core::domain::aggregates::call_graph::SymbolId;
use cognicode_core::domain::value_objects::{
    DependencyType, PendingClass, PendingReference, RejectedRelation,
};
use cognicode_core::interface::hateoas::{Envelope, HandleContext, envelope_for};
use cognicode_mcp::presenter_bridge::present_build_graph;
use serde_json::Map;

/// The tree both routes talk about.
const TREE: &str = "/repo";

/// A build that left out one file, one reference and one relation.
fn the_partial_outcome() -> AnalysisOutcome<AnalysisValue> {
    AnalysisOutcome::partial(
        AnalysisValue::Graph(GraphValue::Full {
            path: TREE.into(),
            symbols: 4,
            dependencies: 2,
            elapsed_ms: 10,
            pending_references: vec![PendingReference {
                origin_file: "src/a.rs".to_string(),
                origin_line: Some(7),
                source: SymbolId::new("src/a.rs:caller:3"),
                target: "ghost".to_string(),
                kind: DependencyType::Calls,
                class: PendingClass::Unresolved,
            }],
            rejected_relations: vec![RejectedRelation {
                source: SymbolId::new("src/a.rs:caller:3"),
                target: SymbolId::new("src/b.rs:callee:1"),
                kind: DependencyType::Calls,
                reason: "target not in the graph".to_string(),
            }],
        }),
        vec![AnalysisError::gap("graph", "src/broken.rs: syntax error")],
    )
}

/// What the terminal prints under "Siguiente:", as `(command, reason)`.
fn on_the_terminal(outcome: &AnalysisOutcome<AnalysisValue>) -> Vec<(String, String)> {
    let output = CommandOutput::Graph(present_graph_full(outcome, TREE, false));
    let mut buffer = Vec::new();
    present_output(&output, CliFormat::Text, &mut buffer).expect("the presenter writes");
    let text = String::from_utf8(buffer).expect("the presenter writes utf-8");

    let (_, after) = text
        .split_once("Siguiente:\n")
        .unwrap_or_else(|| panic!("a partial build says what comes next: {text}"));
    after
        .lines()
        .map(|line| {
            let (command, reason) = line
                .trim()
                .split_once(" — ")
                .unwrap_or_else(|| panic!("a next step is `command — reason`: {line:?}"));
            (command.to_string(), reason.to_string())
        })
        .collect()
}

/// The envelope the wire publishes for the same outcome.
fn on_the_wire(outcome: &AnalysisOutcome<AnalysisValue>) -> Envelope {
    let payload =
        serde_json::to_value(present_build_graph(outcome)).expect("the output serialises");
    let context = HandleContext {
        workspace: TREE.to_string(),
        snapshot: "none".to_string(),
    };
    envelope_for("build_graph", &Map::new(), &payload, &context)
}

#[test]
fn the_terminal_prints_the_links_the_wire_publishes() {
    let outcome = the_partial_outcome();

    let terminal = on_the_terminal(&outcome);
    let wire = follow_ups::annotated_commands(&on_the_wire(&outcome));

    assert!(
        !wire.is_empty(),
        "a partial build offers a way forward on the wire"
    );
    assert_eq!(
        terminal, wire,
        "the same outcome, the same next step, on both routes"
    );
}

#[test]
fn both_routes_count_the_same_holes() {
    let outcome = the_partial_outcome();
    let terminal = on_the_terminal(&outcome);
    let wire = on_the_wire(&outcome);

    let coverage = wire.basis.coverage.expect("the wire declares its coverage");
    assert_eq!(
        (
            coverage.skipped_files,
            coverage.pending_references,
            coverage.rejected_relationships
        ),
        (1, 1, 1),
        "the wire counts the three kinds of hole"
    );
    assert!(
        terminal
            .iter()
            .any(|(_, reason)| reason.contains("1 file, 1 reference and 1 relation")),
        "the terminal counts the same three kinds of hole: {terminal:?}"
    );
}
