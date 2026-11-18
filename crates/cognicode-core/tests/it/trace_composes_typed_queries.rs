//! The `trace` goal composes one typed query over the project graph:
//! lookup, expand, filter and top-k under a budget. These tests drive the
//! goal over a real tree and read what it publishes: the rows, the plan it
//! ran, what it spent and whether the answer is complete.

#![cfg(unix)]

use cognicode_core::interface::cursor::CursorContext;
use cognicode_core::interface::goals::trace::{self, GoalTrace, TraceLimits};
use cognicode_core::interface::hateoas::envelope_for;
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};

fn write(path: &Path, contents: &str) {
    std::fs::create_dir_all(path.parent().expect("a parent")).expect("directories");
    std::fs::write(path, contents).expect("the file is written");
}

/// A tree where `target` is called from two modules and from itself through
/// a cycle (`ping` and `pong`), and where `twin` is declared twice.
fn tree() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().to_path_buf();
    write(
        &root.join("src/lib.rs"),
        "pub mod api;\npub mod jobs;\npub fn target() -> u32 {\n    1\n}\n",
    );
    write(
        &root.join("src/api/mod.rs"),
        "pub fn from_api() -> u32 {\n    crate::target()\n}\n\
         pub fn also_from_api() -> u32 {\n    crate::target()\n}\n",
    );
    write(
        &root.join("src/jobs/mod.rs"),
        "pub fn from_jobs() -> u32 {\n    crate::target()\n}\n\
         pub fn ping(n: u32) -> u32 {\n    pong(n)\n}\n\
         pub fn pong(n: u32) -> u32 {\n    ping(n)\n}\n",
    );
    write(
        &root.join("src/one.rs"),
        "pub fn twin() -> u32 {\n    1\n}\n",
    );
    write(
        &root.join("src/two.rs"),
        "pub fn twin() -> u32 {\n    2\n}\n",
    );
    (dir, root)
}

fn run(root: &Path, symbol: &str, direction: &str, depth: u32, limits: &TraceLimits) -> GoalTrace {
    trace::trace(root, None, symbol, direction, depth, limits).expect("the goal answers")
}

fn names(answer: &GoalTrace) -> Vec<String> {
    let mut names: Vec<String> = answer.entries.iter().map(|e| e.name.clone()).collect();
    names.sort();
    names
}

#[test]
fn a_module_filter_keeps_only_the_callers_of_that_module() {
    let (_tmp, root) = tree();
    let limits = TraceLimits {
        module: Some("api".into()),
        ..TraceLimits::default()
    };

    let answer = run(&root, "target", "callers", 1, &limits);

    assert_eq!(names(&answer), ["also_from_api", "from_api"]);
    assert_eq!(answer.status, "complete");
}

#[test]
fn top_k_lists_the_best_rows_and_says_how_many_matched() {
    let (_tmp, root) = tree();
    let limits = TraceLimits {
        top_k: Some(1),
        ..TraceLimits::default()
    };

    let answer = run(&root, "target", "callers", 1, &limits);

    assert_eq!(answer.entries.len(), 1);
    assert_eq!(answer.explain.matched, 3);
    assert_eq!(answer.explain.listed, 1);
    assert!(
        answer
            .summary
            .contains("3 matched and the best 1 are listed")
    );
}

#[test]
fn the_explain_publishes_the_plan_and_what_it_spent() {
    let (_tmp, root) = tree();

    let answer = run(&root, "target", "callers", 1, &TraceLimits::default());

    assert!(answer.explain.spent_visits > 0);
    assert!(answer.explain.estimated_visits > 0);
    let rendered = trace::render(&serde_json::to_value(&answer).expect("JSON"));
    assert!(rendered.contains("plan: lookup > expand > filter > topk"));
}

#[test]
fn a_budget_that_runs_out_gives_a_partial_answer_and_a_partial_envelope() {
    let (_tmp, root) = tree();
    let limits = TraceLimits {
        budget: Some(2),
        ..TraceLimits::default()
    };

    let answer = run(&root, "target", "callers", 1, &limits);

    assert_eq!(answer.status, "partial");
    assert!(answer.explain.spent_visits <= 2);
    assert!(answer.summary.contains("partial"));
    let full = run(&root, "target", "callers", 1, &TraceLimits::default());
    assert!(answer.entries.len() < full.entries.len());

    let value = serde_json::to_value(&answer).expect("JSON");
    let ctx = CursorContext {
        workspace: root.to_string_lossy().into_owned(),
        snapshot: String::new(),
    };
    let envelope = envelope_for("trace", &Map::new(), &value, &ctx);
    assert_eq!(envelope.state, "partial");
}

#[test]
fn a_cycle_ends_and_lists_each_symbol_once() {
    let (_tmp, root) = tree();

    let answer = run(&root, "ping", "callers", 6, &TraceLimits::default());

    assert_eq!(names(&answer), ["pong"]);
    assert_eq!(answer.status, "complete");
}

#[test]
fn a_homonym_is_refused_with_its_candidates_and_never_picked() {
    let (_tmp, root) = tree();

    let error = trace::trace(&root, None, "twin", "callers", 1, &TraceLimits::default())
        .expect_err("two declarations share the name");

    assert!(error.contains("twin"), "{error}");
    assert!(
        error.contains("one.rs") && error.contains("two.rs"),
        "{error}"
    );
}

#[test]
fn a_query_that_is_not_valid_is_refused_without_running() {
    let (_tmp, root) = tree();

    let depth = trace::trace(&root, None, "target", "callers", 0, &TraceLimits::default())
        .expect_err("depth zero is not a query");
    let side = trace::trace(
        &root,
        None,
        "target",
        "MATCH (n) RETURN n",
        1,
        &TraceLimits::default(),
    )
    .expect_err("text is not a side");
    let top_k = trace::trace(
        &root,
        None,
        "target",
        "callers",
        1,
        &TraceLimits {
            top_k: Some(0),
            ..TraceLimits::default()
        },
    )
    .expect_err("a top-k of zero lists nothing");

    assert!(depth.contains("not valid"), "{depth}");
    assert!(side.contains("is not a side to follow"), "{side}");
    assert!(top_k.contains("not valid"), "{top_k}");
}

#[test]
fn the_goal_reads_the_limits_from_its_arguments() {
    let (_tmp, root) = tree();
    let value: Value = serde_json::json!({
        "symbol": "target", "direction": "callers", "depth": 1, "module": "jobs", "top_k": 5
    });
    let input: trace::TraceInput = serde_json::from_value(value).expect("the input parses");

    assert_eq!(input.module.as_deref(), Some("jobs"));
    assert_eq!(input.top_k, Some(5));
    let limits = TraceLimits {
        module: input.module,
        top_k: input.top_k,
        budget: input.budget,
    };
    let answer = run(&root, &input.symbol, &input.direction, input.depth, &limits);
    assert_eq!(names(&answer), ["from_jobs"]);
}
