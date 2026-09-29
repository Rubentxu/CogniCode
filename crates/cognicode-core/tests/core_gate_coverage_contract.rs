//! Gate-coverage contract for the `cognicode-core` integration surface.
//!
//! The sibling contract for `cognicode-cli`
//! (`crates/cognicode-cli/tests/cli_gate_coverage_contract.rs`) documents
//! why counting beats substring-matching: narrowing a selector to `--bins`
//! or `--tests` keeps hundreds of tests and silently drops every
//! `tests/*.rs` target, while every `contains("cargo test")` assertion
//! still passes. This file is the same argument applied to the core crate.
//!
//! Three failure modes, none of which is "the step disappeared":
//!
//!   1. **Ungated suites.** `merge-gate` is the only required check. It
//!      runs `--lib` plus four named suites. As of 2026-09-29 that left
//!      27 of the 36 `tests/*.rs` suites referenced by no workflow at
//!      all, and 3 more reachable only from `ci.yml`, which declares
//!      `on: workflow_dispatch` and therefore never blocks a merge.
//!      Measured: 3098 tests pass in 23s with `--features evidence-kernel`,
//!      of which only 2794 are `--lib`.
//!
//!   2. **Vacuous feature gates.** Five suites sit behind a crate-level
//!      `#![cfg(feature = "evidence-kernel")]`. Without the feature they
//!      compile to an empty binary: `0 passed; 0 failed; exit 0`. That is
//!      strictly worse than never running, because it reports success
//!      (Lesson 117). The same trap the CLI contract pins for
//!      `--features ladybug`.
//!
//!   3. **A suite that compiles to nothing.** A `#[test]` count of zero
//!      in a gated suite is the fingerprint of failure mode 2, and it is
//!      asserted explicitly so a future feature split cannot hide behind a
//!      green that means "nothing ran".
//!
//! The primary assertion is a count against the filesystem, for the
//! reason above: every `tests/*.rs` must be a target the gate compiles.

use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/cognicode-core has a parent")
        .parent()
        .expect("crates/ has a parent")
        .to_path_buf()
}

fn read_workflow() -> String {
    let path = repo_root().join(".github/workflows/pr-ci.yml");
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

/// Every integration-suite source file in the crate, sorted.
fn integration_suite_names() -> Vec<String> {
    let dir = repo_root().join("crates/cognicode-core/tests");
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            if path.extension()?.to_str()? != "rs" {
                return None;
            }
            Some(path.file_stem()?.to_str()?.to_string())
        })
        .collect();
    names.sort();
    names
}

/// The `cargo test -p cognicode-core ...` invocations present in the
/// workflow, one entry per line of actual command text.
fn core_test_invocations(workflow: &str) -> Vec<String> {
    workflow
        .lines()
        .map(str::trim)
        .filter(|l| l.contains("cargo test -p cognicode-core"))
        .map(|l| {
            // Keep only the cargo invocation, dropping YAML noise and
            // trailing comments, so a selector can be compared as a whole.
            l.split('#').next().unwrap_or(l).trim().to_string()
        })
        .collect()
}

/// A selector that compiles every `tests/*.rs` target.
///
/// `--lib` is the specific defect this pins: it is valid for this crate
/// (unlike the bin-only CLI crate) and it is what every historical step
/// used, so it reads as responsible while skipping all 36 integration
/// targets by construction. Naming any target narrows it.
fn is_unrestricted_core_selector(command: &str) -> bool {
    if !command.contains("cargo test -p cognicode-core") {
        return false;
    }
    ![
        "--lib",
        "--bins",
        "--tests",
        "--benches",
        "--examples",
        "--doc",
    ]
    .iter()
    .any(|flag| command.contains(flag))
        && !command.contains("--test ")
}

#[test]
fn every_core_suite_is_either_named_or_covered_by_an_unrestricted_step() {
    let workflow = read_workflow();
    let invocations = core_test_invocations(&workflow);
    let has_unrestricted = invocations.iter().any(|c| is_unrestricted_core_selector(c));
    let named: Vec<&str> = invocations
        .iter()
        .filter_map(|c| c.split("--test ").nth(1))
        .filter_map(|rest| rest.split_whitespace().next())
        .collect();

    let suites = integration_suite_names();
    assert!(
        suites.len() >= 30,
        "expected the crate to still carry its full integration surface, \
         found only {} suites; the gate contract is not measuring what it \
         thinks it is: {suites:?}",
        suites.len()
    );

    let ungated: Vec<&String> = suites
        .iter()
        .filter(|s| !has_unrestricted && !named.contains(&s.as_str()))
        .collect();
    assert!(
        ungated.is_empty(),
        "{} of {} cognicode-core integration suites are neither named in a \
         `--test` step nor covered by an unrestricted step: {ungated:?}",
        ungated.len(),
        suites.len()
    );
}

#[test]
fn the_gate_builds_the_evidence_kernel_feature() {
    // The five `evidence-kernel` suites compile to an empty test binary
    // without the feature: 0 passed, 0 failed, exit 0, green under any
    // invocation including a manual one. A gate that names them and does
    // not pass the feature is worse than one that ignores them, because
    // it reports success for tests that cannot fail.
    //
    // The CLI contract pins the same trap for `--features ladybug`; this
    // is the core-crate instance of the same defect class.
    let workflow = read_workflow();
    let invocations = core_test_invocations(&workflow);
    let unrestricted: Vec<&String> = invocations
        .iter()
        .filter(|c| is_unrestricted_core_selector(c))
        .collect();
    assert!(
        !unrestricted.is_empty(),
        "no unrestricted cognicode-core selector: {invocations:?}"
    );
    let with_feature = unrestricted
        .iter()
        .any(|c| c.contains("--features") && c.contains("evidence-kernel"));
    assert!(
        with_feature,
        "the unrestricted cognicode-core step does not pass \
         --features evidence-kernel. The five suites behind that feature \
         compile to empty test binaries without it and report 0 passed / \
         0 failed / exit 0. Restricted selectors found: {unrestricted:?}"
    );
}

#[test]
fn no_suite_is_gated_behind_a_feature_nobody_builds() {
    // The specific inventory, so a new feature-gated suite cannot join
    // the blind set without this failing. `behavior_budget_e2e` is here
    // deliberately: it does not carry the cfg itself, it documents the
    // requirement in its header, which is a weaker and easier-to-miss
    // form of the same problem.
    let expected = [
        "behavior_budget_e2e",
        "cp5_tie_break",
        "equivalence_harness",
        "identity_benchmark",
        "workspace_isolation",
    ];

    let dir = repo_root().join("crates/cognicode-core/tests");
    let mut found: Vec<String> = Vec::new();
    for name in &expected {
        let path = dir.join(format!("{name}.rs"));
        let src = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        // The cfg has to be at crate level (line 1, after any shebang
        // there is none) to blank the whole binary. A `#[cfg]` on a
        // single test would leave the rest of the suite running.
        if src
            .lines()
            .next()
            .is_some_and(|l| l.contains("#![cfg(feature"))
        {
            found.push((*name).to_string());
        }
    }
    assert_eq!(
        found.len(),
        expected.len() - 1,
        "expected exactly {} of the {expected:?} suites to carry a \
         crate-level feature cfg, found {found:?}. If a suite here is now \
         compiled unconditionally, drop it from this list rather than \
         leaving a stale pin.",
        expected.len() - 1
    );
}

#[test]
fn a_gated_suite_still_declares_tests() {
    // The fingerprint of failure mode 2, asserted on the source rather
    // than on a run: a suite that is entirely inside a feature cfg and
    // declares no tests is an empty file with a green exit code.
    for name in [
        "cp5_tie_break",
        "equivalence_harness",
        "identity_benchmark",
        "workspace_isolation",
    ] {
        let path = repo_root()
            .join("crates/cognicode-core/tests")
            .join(format!("{name}.rs"));
        let src = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        let declared = src
            .lines()
            .filter(|l| {
                let t = l.trim();
                t == "#[test]" || t == "#[tokio::test]"
            })
            .count();
        assert!(
            declared > 0,
            "{name} declares no tests at all; a suite that cannot fail is \
             not coverage"
        );
    }
}

#[test]
fn the_coverage_contract_itself_is_pinned() {
    // Self-reference, same as the CLI contract: a contract that protects
    // coverage but is itself ungated disappears exactly as quietly as the
    // gap it closes.
    let path = repo_root().join("crates/cognicode-core/tests");
    assert!(
        Path::new(&path)
            .join("core_gate_coverage_contract.rs")
            .exists(),
        "this contract file is missing from tests/; it cannot be running"
    );
    let workflow = read_workflow();
    assert!(
        workflow.contains("--test core_gate_coverage_contract"),
        "core_gate_coverage_contract is not named in pr-ci.yml, so the \
         coverage guarantee is itself ungated."
    );
}
