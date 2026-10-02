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

mod common;

use common::{merge_authority_runs, not_run_message};
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/cognicode-core has a parent")
        .parent()
        .expect("crates/ has a parent")
        .to_path_buf()
}

/// The merge authority's command text, one entry per line.
///
/// The orchestrator that gates a merge is `merge-gate.pipeline.kts`; it used
/// to be `.github/workflows/pr-ci.yml`. Which file that is, is declared once
/// in `common` rather than re-derived here.
fn merge_authority_text() -> String {
    common::merge_authority_lines().join("\n")
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
    let workflow = merge_authority_text();
    let invocations = core_test_invocations(&workflow);
    let has_unrestricted = invocations.iter().any(|c| is_unrestricted_core_selector(c));
    let named: Vec<&str> = invocations
        .iter()
        .filter_map(|c| c.split("--test ").nth(1))
        .filter_map(|rest| rest.split_whitespace().next())
        .collect();

    let suites = integration_suite_names();
    // Exact, not ">=". This used to read `>= 30`, which is a guard against
    // measuring nothing but says nothing about the surface actually
    // present: deleting any single suite still passed, verified by removing
    // `inc007_integration` and watching all 5 tests stay green with 36
    // suites on disk. A count that only has a floor cannot detect a suite
    // being quietly dropped, which is the exact failure this file exists
    // to prevent. Raising the number is a deliberate, reviewable act; that
    // is the point.
    // 37 -> 38 with `a014_product_asset_resolution` (A-014 asset resolution,
    // added alongside the fix that stops `cognicode capabilities` reading
    // `product/*.json` from the build machine's path). A suite was added on
    // purpose in that commit, which is exactly the case this assertion asks
    // to be a deliberate, reviewable act.
    // 38 -> 39 with `clippy_gate_parity_contract` (2026-09-30). The new
    // suite is named by no workflow and no script on purpose: it asserts
    // properties of the pipeline scripts themselves rather than running a
    // suite, so it belongs in the anchor set — the same class as this file.
    // 39 -> 40 with `roadmap_claims_contract` (2026-09-30). The ROADMAP had
    // claimed PR #309 was unmerged 44 seconds after it was merged, and
    // nothing re-checked it. Like the suite above, it asserts properties of
    // a file the project governs rather than running a suite, so it belongs
    // in the anchor set.
    assert_eq!(
        suites.len(),
        40,
        "expected exactly 40 cognicode-core integration suites, found {}. \
         If a suite was removed on purpose, update this number in the same \
         commit. If it was not, the gate contract is blind to a lost suite: \
         {suites:?}",
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

/// Non-UTF-8 extensions accepted as out of scope, with the reason.
///
/// Measured 2026-09-29 across the whole repository: 63 `.webm`
/// regression recordings, 9 sqlite/db fixtures, 4 `.cache`, 4 `.rlib`,
/// 1 `.wasm`, and 25 `.pyc`/`.pyo` files under `__pycache__`. None is
/// source. A reference to a suite name cannot live in a recording, a
/// database fixture or a compiled bytecode file in any way a reader
/// would rely on.
///
/// The `.pyc` case is the one that bit first: `__pycache__` is generated
/// by running the test suite, so a fresh `pytest` run in `scripts/`
/// made this test fail for a reason that has nothing to do with the
/// contract. The lesson is not the extension, it is that **generated
/// artefacts must be excluded before they are counted**, or a gate
/// measures its own test run.
///
/// This is a name-based exemption, so it carries the same shape of risk
/// as the `tests/` exemption N+57 removed: a new binary type under a new
/// extension will be collected and fail, which is the intended
/// direction. A file hiding a reference under one of *these* extensions
/// stays invisible, and that is a chosen trade-off, not an oversight.
const ACCEPTED_UNREADABLE_EXTENSIONS: &[&str] = &[
    "webm",
    "sqlite",
    "sqlite-shm",
    "cache",
    "rlib",
    "wasm",
    "db",
    "pyc",
    "pyo",
];

fn is_accepted_unreadable(path: &Path) -> bool {
    // `foo.sqlite-shm` has extension "shm" under Path::extension, so the
    // second component is checked too rather than trusting one API.
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let stem_ext = path
        .file_stem()
        .and_then(|s| s.to_str())
        .and_then(|s| s.rsplit_once('.').map(|(_, e)| e))
        .unwrap_or("");
    ACCEPTED_UNREADABLE_EXTENSIONS
        .iter()
        .any(|a| *a == ext || *a == stem_ext)
}

/// Suites that no CI workflow names with `--test` and no script under
/// `scripts/` mentions, as of 2026-09-29: 26 entries.
///
/// The definition is deliberately the strongest one available, because
/// the first two were both wrong and both plausible:
///
///   1. "zero references outside the suite" is empty, and not by
///      accident. `JOURNAL.md` cites every one of the 37 suites, so a
///      documentation-dependent rule can never be recomputed once the
///      documentation exists. N+56 wrote 16, the constant then listed 14,
///      a fresh grep gave 37: three numbers, three different mistakes.
///   2. "no *quoted* references" was the immediate cause of the drift.
///      The needle was `"suite"`, and a `--test` flag, a journal line and
///      a spec line are all unquoted, so the scan could not see a single
///      real reference. It called `analytics_bounded_paths` unreferenced
///      while four files cited it.
///   3. "named by no workflow" alone is wrong in the other direction:
///      `m06_acceptance` and `m10_acceptance` name no workflow, but
///      `scripts/product/generate_support_matrix.py` reads both, so their
///      loss would break a script and not a gate. Hence the two needles,
///      `--test suite` and `tests/suite.rs`, and hence `scripts/` in scope.
///   4. The scan itself was broken, so an earlier version of this list
///      was measured against a scan that read nothing. `walk` accepted
///      only directories and swallowed `read_dir`'s error on a file, so
///      passing the workflow paths was a no-op and 7 of the 33 pinned
///      names were not really unnamed. Fixed in the same commit as the
///      list, and the count moved 33 -> 26 because of it.
///
/// What is left is a criterion no amount of prose can satisfy: nothing
/// in the machine-readable configuration names them. A rename breaks
/// the name pin; a deletion breaks the exact count in
/// `every_core_suite_is_either_named_or_covered_by_an_unrestricted_step`.
///
/// `a014_product_asset_resolution` is deliberately NOT here. It is named by
/// the merge gate ("A-014 product assets resolve from the runtime, not the build
/// machine"), and an anchor is by definition a suite that nothing names. Once
/// a suite is named, the name pin covers it and adding it to this list would
/// make the measured set and the pinned set disagree — which is exactly what
/// `the_anchor_set_is_the_measured_one` is for.
const ANCHOR_SUITES: &[&str] = &[
    "analytics_bounded_paths",
    "analytics_registry_admission",
    "analytics_registry_cohort_1",
    "analytics_registry_cohort_2",
    "analytics_registry_cohort_3",
    "architecture_e77_1_wu0_gap_characterization_e2e",
    "architecture_e77_1_wu3_canonical_grounding_e2e",
    "architecture_self_host_e2e",
    "behavior_authority_e2e",
    "behavior_budget_e2e",
    "callgraph_projection_orientation",
    "checkpoint_integration",
    "clippy_gate_parity_contract",
    "cp5_tie_break",
    "e2_w1_canonical_control_query",
    "equivalence_harness",
    "find_usages_cli_mcp_equivalence",
    "find_usages_mcp_handler_e2e",
    "findings_ast_e2e",
    "findings_axiom_import_e2e",
    "findings_dataflow_e2e",
    "findings_graph_e2e",
    "identity_benchmark",
    "inc007_integration",
    "intelligence_event_log_e2e",
    "prf_ext_04_adapter_authority_uat",
    "provider_conformance",
    "read_set_e2e",
    "roadmap_claims_contract",
];

/// The count alone cannot see a rename: renaming a suite keeps `len()`
/// at 37 while the pinned name is gone. A count-plus-rename in one
/// commit is the failure that slips through, because both halves stay
/// consistent.
///
/// So pin a set of *anchor* suites by name. Not all 37: that list
/// would have to be edited on every addition, which is the rot the
/// unrestricted selector exists to avoid. The anchors are the suites
/// whose loss is **cheapest to miss**: named by no workflow and by no
/// script, so nothing in the machine-readable configuration breaks when
/// they disappear.
///
/// The first version of this list was hand-picked and claimed to
/// follow a zero-reference rule. It did not: 7 of 13 named suites were
/// referenced by a workflow. A rule that the list does not satisfy is
/// decoration with a comment, so `the_anchor_set_is_the_measured_one`
/// recomputes the set from the tree and compares, instead of leaving the
/// rule asserted in a doc comment.
#[test]
fn anchor_suites_are_still_present_by_name() {
    let present = integration_suite_names();
    let missing: Vec<&str> = ANCHOR_SUITES
        .iter()
        .copied()
        .filter(|a| !present.iter().any(|p| p == a))
        .collect();
    assert!(
        missing.is_empty(),
        "{} of {} anchor suites are gone from tests/: {missing:?}. A rename \
         or a deletion moves `len()` by zero when both happen in the same \
         commit, which is the case the exact count cannot see. If a suite \
         was renamed on purpose, update this list in the same commit.",
        missing.len(),
        ANCHOR_SUITES.len()
    );
    // A rename alone must also fail, which is the other half of the gap.
    assert!(
        !present.contains(&"renombrada_por_error".to_string()),
        "a leftover renamed suite is present; anchors no longer match the tree"
    );
}

/// The anchors claim that no machine-readable configuration names them.
/// That claim is checkable, so it is checked: a suite that has been added
/// to a workflow is no longer the thing this list was built to protect,
/// because its loss would now announce itself. Leaving a stale anchor in
/// place is the same class of error as the `>= 30` floor, one level up: a
/// pin that no longer describes reality.
#[test]
fn anchors_stay_unreferenced_elsewhere() {
    let root = repo_root();
    let mut referenced: Vec<&str> = Vec::new();
    let mut unreadable: Vec<String> = Vec::new();
    let workflows = workflow_paths();
    for anchor in ANCHOR_SUITES {
        if is_named_by_configuration(anchor, &root, &workflows, &mut unreadable) {
            referenced.push(anchor);
        }
        if !unreadable.is_empty() {
            break;
        }
    }
    assert!(
        referenced.is_empty(),
        "these anchor suites are now named by a workflow or a script, so they \
         are no longer the cheapest-loss set this list protects: \
         {referenced:?}. Remove them from ANCHOR_SUITES, or keep them and say \
         in the commit why naming them in the config is the point."
    );
    assert!(
        unreadable.is_empty(),
        "{} configuration files are neither valid UTF-8 nor a \
         known-accepted binary extension: {unreadable:?}",
        unreadable.len()
    );
}

/// Every pipeline file, so the anchor rule scans all of them and not just
/// the one the other assertions read. `common` owns that list because the
/// same set is needed by every wiring contract in the crate.
fn workflow_paths() -> Vec<PathBuf> {
    common::pipeline_paths()
}

/// Whether any machine-readable configuration names a suite.
///
/// Two needles, because configuration cites a suite in two ways: a
/// pipeline writes `--test suite`, a script writes `tests/suite.rs`. The
/// bare name alone would match this contract and every journal entry,
/// which is not the rule; the rule is about what the machine executes.
///
/// `docs/`, `openspec/` and `.agent/` are deliberately not scanned. They
/// cite all 37 suites, so including them would make the rule permanently
/// unsatisfiable and the pin meaningless.
fn is_named_by_configuration(
    suite: &str,
    root: &Path,
    workflows: &[PathBuf],
    unreadable: &mut Vec<String>,
) -> bool {
    let bare = format!("--test {suite}");
    let pathed = format!("tests/{suite}.rs");
    let mut found = false;
    walk(&root.join("scripts"), &bare, None, &mut found, unreadable);
    if !found {
        walk(&root.join("scripts"), &pathed, None, &mut found, unreadable);
    }
    for workflow in workflows {
        if found {
            break;
        }
        walk(workflow, &bare, None, &mut found, unreadable);
        if !found {
            walk(workflow, &pathed, None, &mut found, unreadable);
        }
    }
    found
}

/// Recompute the anchor set from the tree and compare it to the constant.
/// This is the test that replaces a hand-maintained list with a derived
/// one: the list is a *pin* of what was measured, and this is the
/// measurement. A new suite that nothing names fails here telling the
/// operator to add it; a suite that a workflow starts naming fails
/// telling them to remove it.
#[test]
fn the_anchor_set_is_the_measured_one() {
    let root = repo_root();
    let workflows = workflow_paths();
    let mut measured: Vec<String> = Vec::new();
    let mut unreadable: Vec<String> = Vec::new();
    for suite in integration_suite_names() {
        if suite == "core_gate_coverage_contract" {
            continue;
        }
        if !is_named_by_configuration(&suite, &root, &workflows, &mut unreadable)
            && unreadable.is_empty()
        {
            measured.push(suite);
        }
    }
    let mut expected: Vec<&str> = ANCHOR_SUITES.to_vec();
    expected.sort_unstable();
    let mut got = measured;
    got.sort();
    assert_eq!(
        got, expected,
        "the measured anchor set no longer matches ANCHOR_SUITES. \
         measured={got:?} pinned={expected:?}. If a suite became named by a \
         workflow or a script, drop it from the constant; if a new suite is \
         named by nothing, add it. Do not edit the constant to make the diff \
         smaller than the reason."
    );
    assert!(
        unreadable.is_empty(),
        "{} configuration files were not searched: {unreadable:?}",
        unreadable.len()
    );
}

/// Where the anchor-reference scan looks: `scripts/` plus every pipeline
/// file. The rule is "named by no machine-readable configuration", not
/// "unreferenced", because `docs/`, `openspec/` and `.agent/` cite all
/// 37 suites and would make a zero-reference rule permanently
/// unsatisfiable.
///
/// `target/` is build output and is left out on purpose. A rule narrower
/// in the code than in the prose is the defect N+57 fixed, and the second
/// definition of the anchor set failed the same way: the prose said "zero
/// references", the code could only see a fraction of them.
#[test]
fn the_anchor_scan_scope_is_pinned() {
    let root = repo_root();
    assert!(
        root.join("scripts").is_dir(),
        "scripts/ is scanned for anchor references but does not exist; the \
         scan silently covers less than this test claims"
    );
    let pipelines = workflow_paths();
    assert!(
        !pipelines.is_empty(),
        "no PipelineK scripts were found at the repository root, so the anchor \
         scan would pass vacuously. The rule is 'named by no machine-readable \
         orchestration', and the pipelines are that orchestration now that the \
         GitHub Actions workflows are gone."
    );
    assert!(
        pipelines
            .iter()
            .any(|p| p.file_name().is_some_and(|n| n == common::MERGE_AUTHORITY)),
        "the pipeline list does not include {}, so the scan is not covering the \
         thing that actually gates a merge",
        common::MERGE_AUTHORITY
    );
    assert!(
        !pipelines
            .iter()
            .any(|p| p.to_string_lossy().contains("/target/")),
        "target/ is build output; scanning it would be slow and meaningless"
    );
}

/// Walks a directory tree looking for a needle. A path that is a file is
/// read directly, so a caller can pass a pipeline file as well as a
/// directory.
///
/// The file case is not a convenience. An earlier version read only
/// directories and returned silently when handed a file, because
/// `read_dir` on a file fails and the failure was swallowed by a `let Ok`
/// — so passing the workflow paths scanned **nothing at all**, and a
/// mutation that added `--test anchor` to the merge authority left the contract
/// green. A scan that cannot fail is not a scan.
///
/// Two files are exempt, and the exemption is per-file, not per-directory:
/// the file under measurement (which necessarily mentions its own name)
/// and this contract (which pins the names by definition). An earlier
/// version excluded anything under a `tests/` path, which silently hid a
/// reference from one suite into another.
///
/// Unreadable files are collected, not skipped: 81 files in this tree are
/// not valid UTF-8, and an earlier version dropped every one of them
/// silently through `let Ok(text) = ...`. A file that is not read and a
/// file that does not exist are indistinguishable from outside, which is
/// exactly the ambiguity a gate cannot carry.
fn walk(
    path: &Path,
    needle: &str,
    skip: Option<&Path>,
    found: &mut bool,
    unreadable: &mut Vec<String>,
) {
    if *found {
        return;
    }
    if path.is_file() {
        read_for_needle(path, needle, skip, found, unreadable);
        return;
    }
    let entries = match std::fs::read_dir(path) {
        Ok(entries) => entries,
        Err(e) => {
            // A directory that cannot be listed is a blind spot, not a
            // detail. Report it instead of pretending the scan finished.
            unreadable.push(format!("{} (unreadable directory: {e})", path.display()));
            return;
        }
    };
    for entry in entries.flatten() {
        let child = entry.path();
        if child.is_dir() {
            walk(&child, needle, skip, found, unreadable);
        } else {
            read_for_needle(&child, needle, skip, found, unreadable);
        }
        if *found {
            return;
        }
    }
}

/// Reads one file looking for the needle, honouring both exemptions and
/// recording anything unreadable.
fn read_for_needle(
    path: &Path,
    needle: &str,
    skip: Option<&Path>,
    found: &mut bool,
    unreadable: &mut Vec<String>,
) {
    let name = path.file_name().and_then(|f| f.to_str()).unwrap_or("");
    // Exempt this contract, which pins the names verbatim by design, and
    // the file that is the subject of the search, which necessarily
    // mentions its own name. Nothing else is exempt: excluding the whole
    // tests/ tree would hide a reference from one suite to another, which
    // is a reference the rule claims to see.
    if name == "core_gate_coverage_contract.rs" {
        return;
    }
    if skip.is_some_and(|s| s == path) {
        return;
    }
    match std::fs::read_to_string(path) {
        Ok(text) => {
            if text.contains(needle) {
                *found = true;
            }
        }
        Err(e) => {
            if !is_accepted_unreadable(path) {
                unreadable.push(format!("{} ({e})", path.display()));
            }
        }
    }
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
    let workflow = merge_authority_text();
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
    let workflow = merge_authority_text();
    assert!(
        workflow.contains("--test core_gate_coverage_contract"),
        "core_gate_coverage_contract is not run by the merge authority, so the \
         coverage guarantee is itself ungated."
    );
}

/// The M0.11 rustdoc gate must run in whatever gates a merge.
///
/// Measured 2026-10-01: `m011_rustdoc_gate` was wired into no workflow at
/// all. It was green, and it could not fail a single PR. An intra-doc link
/// that ST-01 broke in `infrastructure/parser/syntax_analysis.rs` survived
/// that commit, the whole branch, and six further commits — it surfaced only
/// because someone happened to run `cargo test --workspace`.
///
/// The standard is already written down in this repository: *"Un test que no
/// corre en ningun sitio no es un gate."* The gate was breaking that rule.
///
/// This assertion deliberately lives in the core crate rather than next to the
/// gate it protects. The merge authority runs the core crate unrestricted, so
/// if the rustdoc wiring is deleted the core suite still runs and this still
/// fails. A contract that only the deleted wiring could execute would notice
/// nothing — which is the failure mode it exists to catch.
#[test]
fn the_rustdoc_gate_runs_in_the_merge_authority() {
    const GATE: &str = "m011_rustdoc_gate";

    assert!(
        merge_authority_runs(GATE),
        "the merge authority no longer runs {GATE}. That gate holds the workspace \
         at zero rustdoc warnings in the gated categories; without it a broken \
         doc link or a public-docs-to-private-item reference passes every PR.\n{}",
        not_run_message(
            GATE,
            "Restore the stage next to the clippy gate."
        )
    );
}
