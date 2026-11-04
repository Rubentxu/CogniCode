//! TASK-112 — the revalidation says the truth on the production path.
//!
//! `cognicode refactor --apply` is the only place the revalidation report
//! reaches a reader. Before this unit the report lived in the library and
//! nothing published it, so an apply that changed files said nothing about
//! the verification: a reader had no way to tell «nothing was checked» from
//! «everything was checked and everything held».
//!
//! This test runs the real binary over a real workspace and asserts what
//! the document it prints says:
//!
//! - it carries the revalidation;
//! - the revalidation names no check that did not run, and says why;
//! - the findings block says that nobody looked.
//!
//! The third one is the point of the unit. An empty findings list with
//! `searched: false` is an honest answer — «nobody looked». An empty
//! findings list with `searched: true` would be a claim nobody measured.

use std::path::PathBuf;
use std::process::Command;

use cognicode_core::application::analysis_basis::AnalysisBasis;
use cognicode_core::refactor::plan::{PlannedEdit, build_plan, save_plan_to_file};
use serde_json::Value;

use crate::common;

/// A workspace of its own, named uniquely.
///
/// Two tests of the same binary run in parallel, so a directory named
/// after the process alone would be shared: one test's write would land on
/// top of another's, and both would measure whichever write happened last.
fn fresh_workspace(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "cognicode-cli-revalidation-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default()
    ));
    std::fs::create_dir_all(&dir).expect("create the workspace");
    dir.canonicalize()
        .expect("the workspace has a canonical name")
}

/// The document the CLI prints.
fn run_cli_json(args: &[&str]) -> Value {
    let binary = common::binary_path("cognicode");
    let output = Command::new(&binary)
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("failed to spawn {}: {e}", binary.display()));
    assert!(
        output.status.success(),
        "the CLI exited with {}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let start = stdout
        .find('{')
        .expect("the CLI answers with a JSON object");
    serde_json::from_str(&stdout[start..]).expect("the CLI answer parses")
}

/// The workspace, with one file and one plan that renames `a` to `b`.
///
/// The plan is built by the library's own builder and saved by its own
/// saver, so the file the CLI reads is a real plan: the digest is the one
/// the builder computed.
fn workspace_with_a_plan(label: &str) -> (PathBuf, PathBuf) {
    let root = fresh_workspace(label);
    let source = root.join("src");
    std::fs::create_dir_all(&source).expect("create the source directory");
    let file = source.join("lib.rs");
    std::fs::write(&file, "fn a() {}\n").expect("write the file");

    let plan = build_plan(
        root.clone(),
        AnalysisBasis::for_workspace(root.clone()),
        vec![file.clone()],
        vec![PlannedEdit {
            file: file.clone(),
            byte_range: 3..4,
            replacement: "b".to_string(),
            deterministic: true,
        }],
        Vec::new(),
        Vec::new(),
    )
    .expect("the plan is valid");

    let plan_path = root.join("plan.json");
    save_plan_to_file(&plan, &plan_path).expect("the plan is saved");
    (root, plan_path)
}

/// The apply publishes the revalidation, and the revalidation says that
/// nobody checked anything.
#[test]
fn the_apply_publishes_a_revalidation_that_says_nothing_was_checked() {
    let (root, plan_path) = workspace_with_a_plan("json");

    let answer = run_cli_json(&[
        "refactor",
        "a",
        "--apply",
        "--plan-file",
        plan_path.to_str().expect("the plan path is text"),
        "--authorization-token",
        "token-received-out-of-band",
        "--format",
        "json",
    ]);

    let revalidation = &answer["revalidation"];
    assert!(
        revalidation.is_object(),
        "the apply publishes the revalidation it ran: {answer}"
    );

    assert_eq!(
        revalidation["executed_checks"],
        Value::Array(vec![]),
        "the report names no check that did not run: {revalidation}"
    );
    let reason = revalidation["not_executed_reason"]
        .as_str()
        .unwrap_or_else(|| panic!("the report says why nothing ran: {revalidation}"));
    assert!(
        reason.contains("no incremental extractor"),
        "the reason says that nobody searched: {reason}"
    );

    let findings = &revalidation["findings"];
    assert_eq!(
        findings["searched"],
        Value::Bool(false),
        "the findings block says that nobody looked: {findings}"
    );
    assert_eq!(
        findings["findings"],
        Value::Array(vec![]),
        "an empty list is consistent with «nobody looked»: {findings}"
    );
    assert_eq!(
        revalidation["analyzed_files"],
        serde_json::json!(1),
        "the report says how many files the apply touched: {revalidation}"
    );

    assert_eq!(
        answer["committed"],
        Value::Bool(true),
        "the apply did commit: {answer}"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("src/lib.rs")).expect("read the file"),
        "fn b() {}\n",
        "the apply changed the file it said it changed"
    );
}

/// The text output says the same thing the JSON does.
///
/// A reader of the terminal and a reader of the document must not learn
/// different things about the same apply.
#[test]
fn the_text_output_says_that_no_check_ran() {
    let (_root, plan_path) = workspace_with_a_plan("text");

    let binary = common::binary_path("cognicode");
    let output = Command::new(&binary)
        .args([
            "refactor",
            "a",
            "--apply",
            "--plan-file",
            plan_path.to_str().expect("the plan path is text"),
            "--authorization-token",
            "token-received-out-of-band",
        ])
        .output()
        .unwrap_or_else(|e| panic!("failed to spawn {}: {e}", binary.display()));
    assert!(
        output.status.success(),
        "the CLI exited with {}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Revalidation: no check ran"),
        "the terminal says that no check ran: {stdout}"
    );
    assert!(
        stdout.contains("no incremental extractor"),
        "the terminal says why: {stdout}"
    );
}
