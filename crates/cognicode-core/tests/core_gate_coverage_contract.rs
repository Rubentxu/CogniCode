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
    // Exact, not ">=". This used to read `>= 30`, which is a guard against
    // measuring nothing but says nothing about the surface actually
    // present: deleting any single suite still passed, verified by removing
    // `inc007_integration` and watching all 5 tests stay green with 36
    // suites on disk. A count that only has a floor cannot detect a suite
    // being quietly dropped, which is the exact failure this file exists
    // to prevent. Raising the number is a deliberate, reviewable act; that
    // is the point.
    assert_eq!(
        suites.len(),
        37,
        "expected exactly 37 cognicode-core integration suites, found {}. \
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

/// The suites with zero references outside their own file, measured with
/// ripgrep over the tree on 2026-09-29. One list, not two: a duplicated
/// pin list is a pin list that gets amended in one place and goes stale
/// in the other.
const ANCHOR_SUITES: &[&str] = &[
    "analytics_bounded_paths",
    "analytics_registry_admission",
    "analytics_registry_cohort_1",
    "analytics_registry_cohort_2",
    "architecture_e77_1_wu0_gap_characterization_e2e",
    "architecture_e77_1_wu3_canonical_grounding_e2e",
    "find_usages_cli_mcp_equivalence",
    "findings_ast_e2e",
    "findings_axiom_import_e2e",
    "findings_dataflow_e2e",
    "findings_graph_e2e",
    "prf_ext_04_adapter_authority_uat",
    "prf_h06_adversarial_e2e",
    "read_set_e2e",
];

/// The count alone cannot see a rename: renaming a suite keeps `len()`
/// at 37 while the pinned name is gone. A count-plus-rename in one
/// commit is the failure that slips through, because both halves stay
/// consistent.
///
/// So pin a set of *anchor* suites by name. Not all 37: that list
/// would have to be edited on every addition, which is the rot the
/// unrestricted selector exists to avoid. The anchors are the suites
/// with **zero references anywhere outside their own file**, measured
/// with ripgrep over the tree. Losing coverage that nothing mentions
/// is how a gate rots unnoticed, and those are the suites whose loss
/// is cheapest to miss.
///
/// The first version of this list was hand-picked and claimed to
/// follow that rule. It did not: 7 of 13 named suites were referenced
/// by a workflow. A rule that the list does not satisfy is decoration
/// with a comment, so the anchors are the measured zero-reference set
/// and `anchors_stay_unreferenced_elsewhere` re-checks the rule rather
/// than leaving it asserted in a doc comment.
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

/// The anchors claim to be the suites nothing else references. That claim
/// is checkable, so it is checked: an anchor that has acquired a reference
/// is no longer the thing this list was built to protect, because its loss
/// would now announce itself. Leaving a stale anchor in place is the same
/// class of error as the `>= 30` floor, one level up: a pin that no longer
/// describes reality.
#[test]
fn anchors_stay_unreferenced_elsewhere() {
    let root = repo_root();
    // Only this contract is exempt: it pins the very names, quoted on
    // purpose, and no suite can cite itself by its own quoted file name.
    // Everything else counts, including other suites, which an earlier
    // version of this test excluded wholesale.
    let mut referenced: Vec<&str> = Vec::new();
    for anchor in ANCHOR_SUITES {
        let needle = format!("\"{anchor}\"");
        let mut found = false;
        for dir in ANCHOR_SCAN_DIRS {
            walk(&root.join(dir), &needle, &mut found);
            if found {
                break;
            }
        }
        if found {
            referenced.push(anchor);
        }
    }
    assert!(
        referenced.is_empty(),
        "these anchor suites are now referenced somewhere in the repo, so \
         they are no longer the silently-loseable set this list protects: \
         {referenced:?}. Either move them out of the anchor list or keep \
         them if the reference is what should break."
    );
}

/// Where the anchor-reference scan looks. Four directories, and the
/// repository root holds about eighty more; the rule sentence is
/// narrowed to match, because a rule narrower in the code than in the
/// prose is the defect N+57 fixed. Verified 2026-09-29: no anchor is
/// referenced from a root-level file or from the untracked scratch
/// directories. `target/` is build output and is left out on purpose.
const ANCHOR_SCAN_DIRS: &[&str] = &["crates", ".github", "docs", "openspec"];

/// The scan scope is pinned, so widening or shrinking it is a reviewed
/// act rather than a silent drift.
#[test]
fn the_anchor_scan_scope_is_pinned() {
    let root = repo_root();
    for dir in ANCHOR_SCAN_DIRS {
        assert!(
            root.join(dir).is_dir(),
            "{dir} is scanned for anchor references but does not exist; the \
             scan silently covers less than this test claims"
        );
    }
    assert!(
        !ANCHOR_SCAN_DIRS.contains(&"target"),
        "target/ is build output; scanning it would be slow and meaningless"
    );
    assert_eq!(
        ANCHOR_SCAN_DIRS.len(),
        4,
        "the anchor scan scope changed to {ANCHOR_SCAN_DIRS:?}. Widen it only \
         together with the rule sentence, and remember that target/ and the \
         untracked scratch directories are the intended exclusions."
    );
}

/// Walks a directory tree looking for a quoted needle.
///
/// Two files are exempt, and the exemption is per-file, not per-directory:
/// the suite's own source (which naturally mentions nothing, but is the
/// thing being measured) and this contract (which pins the names by
/// definition). An earlier version excluded anything under a `tests/`
/// path, which silently hid a reference from one suite into another. The
/// rule claims "referenced nowhere else"; a rule that cannot see a whole
/// class of references is weaker than the sentence describing it.
fn walk(dir: &Path, needle: &str, found: &mut bool) {
    if *found {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, needle, found);
        } else if let Ok(text) = std::fs::read_to_string(&path) {
            let name = path.file_name().and_then(|f| f.to_str()).unwrap_or("");
            // Exempt only this contract, which pins the names verbatim by
            // design. No suite file is exempt: a suite cannot reference
            // itself by its own quoted file name in a way that would match,
            // and excluding the whole tests/ tree would hide a reference
            // from one suite to another, which is a reference the rule
            // claims to see.
            let is_contract = name == "core_gate_coverage_contract.rs";
            if text.contains(needle) && !is_contract {
                *found = true;
                return;
            }
        }
        if *found {
            return;
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
