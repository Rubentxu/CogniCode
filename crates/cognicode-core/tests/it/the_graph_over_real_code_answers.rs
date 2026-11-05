//! The product graph, built over **real code**, answers for the code it read.
//!
//! ## The guarantee
//!
//! Everywhere else in the suite, the graph is built over a fixture: a handful
//! of lines the test wrote itself. That is the right way to pin behaviour,
//! and it is blind to one thing — whether the graph survives **real** code. A
//! parser that does not descend into an impl block, a walker that skips a
//! nested directory, a resolver that drops the calls: none of those show up in
//! a fixture, because a fixture has no impl blocks and no nested directories.
//!
//! So this file builds the product graph over a real subtree of this
//! repository and asks the questions a client asks:
//!
//! 1. the build returns Ok and the graph holds the symbols the real code
//!    declares — including one that lives in a **nested directory**;
//! 2. it holds the methods that live **inside an impl block** — that is
//!    where most of a real code base's declarations are;
//! 3. the graph holds the **relations** between them — a graph with no edges
//!    would answer every question below with a fiction;
//! 4. `get_file_symbols` finds what a real file declares;
//! 5. `impact_of` resolves a name the graph holds;
//! 6. `get_entry_points` and `get_leaf_functions` are not empty;
//! 7. the graph draws itself.
//!
//! ## Why this file exists
//!
//! Two `#[ignore]`d tests in
//! `src/application/services/analysis_service.rs` covered this and could not
//! be kept: both of them **hang** on a real tree (measured: they do not
//! finish in 300 s), and a test that never finishes cannot live in the gate.
//! Their own `#[ignore]` reason claimed three substitutes; two of the three
//! do not cover what the reason says they cover (measured, and recorded in
//! the receipt of TASK-70, group 2). The real-code character was the one
//! thing nobody else held, so it moved here, over a subtree small enough to
//! finish.
//!
//! ## How it stays cheap
//!
//! The subtree is bounded on purpose. The whole crate is 35,403 symbols, and
//! building that is what made the retired tests hang; this directory is a
//! fraction of it and takes 0,17 s. The assertions are about **structure** —
//! a symbol the code declares is in the graph, and the graph has relations —
//! never about a count that the repository's own churn would move.
//!
//! No network, no clock, no binary, no fixture directory: the code under test
//! is the code this repository ships.

use cognicode_core::application::services::analysis_service::AnalysisService;
use cognicode_core::domain::aggregates::call_graph::MermaidOptions;
use std::path::PathBuf;

/// A real subtree of this repository: the LSP infrastructure.
///
/// Real product code, small enough to build in the time a suite can afford,
/// with a nested directory inside it (the walk has to descend) and without
/// test modules (a `*_tests.rs` sibling would make the witness about tests
/// rather than about the product).
fn real_code() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/infrastructure/lsp")
}

/// The real file whose declarations the assertions name: it lives in the
/// nested directory of the subtree.
fn real_file() -> PathBuf {
    real_code().join("providers/composite.rs")
}

/// A type this repository's own code declares, in the nested directory.
///
/// Pinning a name is a promise about the repository's own source, and the
/// repository already accepts that promise elsewhere: the running benchmark
/// of the lightweight index asks for `build_project_graph` by name.
const A_DECLARED_TYPE: &str = "CompositeProvider";

/// A method the real code declares **inside an impl block**.
const A_DECLARED_METHOD: &str = "wait_timeout_secs";

#[test]
fn the_graph_over_real_code_answers_for_the_code_it_read() {
    let service = AnalysisService::new();

    service
        .build_project_graph(&real_code())
        .expect("the product graph is built over real code");
    let graph = service.get_project_graph();

    // 1. The graph holds what the real code declares — including what is
    //    declared in a nested directory, which the walk has to descend into.
    assert!(
        !graph.find_by_name(A_DECLARED_TYPE).is_empty(),
        "the graph was built over real code and does not hold {A_DECLARED_TYPE}: \
         the real code declares it and the graph does not have it"
    );

    // 2. Including the methods that live inside an impl block: that is where
    //    most of a real code base's declarations are.
    assert!(
        !graph.find_by_name(A_DECLARED_METHOD).is_empty(),
        "the graph was built over real code and does not hold {A_DECLARED_METHOD}: \
         the real code declares it inside an impl block and the graph does not \
         have it"
    );

    // 3. And it holds the relations between them. A graph with no edges
    //    answers every question below with a fiction: it would call every
    //    symbol an entry point and every symbol a leaf.
    assert!(
        graph.edge_count() > 0,
        "the graph of real code has no relations: the calls the real code \
         declares were not resolved"
    );

    // 4. The file view agrees: reading the real file finds its declarations.
    let symbols = service
        .get_file_symbols(&real_file())
        .expect("the symbols of a real file are read");
    assert!(
        symbols.iter().any(|symbol| symbol.name == A_DECLARED_TYPE),
        "the real file declares {A_DECLARED_TYPE} and reading it does not find it"
    );

    // 5. The question a client asks — "what depends on this name?" — is
    //    answered for a name the graph really holds.
    let impact = AnalysisService::impact_of(&graph, A_DECLARED_TYPE)
        .expect("the graph holds the name the impact question asks about");
    assert!(
        impact.matched >= 1,
        "a name the graph holds must match at least one symbol; it matched {}",
        impact.matched
    );

    // 6. The two questions the MCP publishes about a graph have an answer.
    assert!(
        !service.get_entry_points().is_empty(),
        "the graph of real code has no entry point: every symbol is called by \
         another one, which cannot be true of real code"
    );
    assert!(
        !service.get_leaf_functions().is_empty(),
        "the graph of real code has no leaf function: every symbol calls \
         another one, which cannot be true of real code"
    );

    // 7. And the graph draws itself.
    let mermaid = graph.to_mermaid_with_options("Real code", &MermaidOptions::default());
    assert!(
        mermaid.contains("flowchart"),
        "the graph of real code does not draw itself"
    );
}
