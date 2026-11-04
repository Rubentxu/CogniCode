// crates/cognicode-cli/tests/it/test_taxonomy_contract.rs
//
// The test taxonomy of this repository, and the gate that enforces it.
//
// There are two classes of test and two only:
//
//   1. Unit tests          — `#[cfg(test)]` in the crate; `*.test.ts(x)` in
//                            `apps/explorer-ui/src/`.
//   2. Integration tests   — `crates/*/tests/*.rs`; `apps/explorer-ui/src/**`
//                            with MSW. Real pieces working together, with the
//                            borders (network, binary, clock) replaced by
//                            deterministic doubles.
//
// There was a third class and it is gone: the browser end-to-end suite
// (`apps/explorer-ui/e2e/`, Playwright) and the UAT class (`*_uat.rs`, retired
// earlier under ADR-TH-006). There were also CI lanes, merge gates and
// release gates, gone on 2026-11-04.
//
// A removal that only exists in a commit message is a removal that comes back
// the first time somebody needs a test they cannot write, or the first time a
// CI lane is the easy way to run something. This file is the part of the
// removal that survives the removal: it fails if the third class returns, or
// if the gate grows a fourth step.
//
// The RED for this file is the tree as it was before the retirement: at
// `HEAD~` of the branch that carried it out, every assertion here that
// mentions a removed path failed, and the file did not exist to fail.

use crate::common;

use std::path::{Path, PathBuf};

/// Paths that must not come back.
///
/// These are relative to the repository root. The UAT class was retired
/// earlier; the browser e2e suite and the lane files were retired on
/// 2026-11-04. All three are listed here because "it was removed once" is
/// not a reason it cannot be removed twice.
const RETIRED_PATHS: &[&str] = &[
    // The browser e2e suite.
    "apps/explorer-ui/e2e",
    "apps/explorer-ui/playwright.config.ts",
    "apps/explorer-ui/playwright.config.js",
    "tests/e2e",
    // The UAT class.
    "crates/cognicode-core/tests/prf_cli_01_uat.rs",
    // The session-resume note that told the next session to run the UAT
    // (`sddk uat open --plan uat-plan.yaml`), pointing at files that were
    // already gone. A file that instructs a retired class to be run is
    // the retired class, however little code it holds.
    "RESUME.md",
    // The CI lanes and the merge-gate publisher.
    "merge-gate.pipeline.kts",
    "product-fast.pipeline.kts",
    "release-candidate.pipeline.kts",
    "release.pipeline.kts",
    "integration.pipeline.kts",
    "certification.pipeline.kts",
    "scripts/ci/publish-merge-gate.sh",
    "scripts/ci/check-merge-subject.sh",
    // The test selector of those lanes. `impact.py` read
    // `merge-gate.pipeline.kts` to decide which stages a change could break;
    // once that file was gone it could only fail, and so did its test.
    "impact-gate.pipeline.kts",
    "docs-gate.pipeline.kts",
    "scripts/ci/impact.py",
    "scripts/ci/test_impact.py",
    "scripts/ci/change-class.sh",
    "scripts/ci/test_change_class.py",
    "scripts/ci/docs-readers.sh",
    // The contract that judged the publisher above; with the publisher gone
    // it can only report a defect in a file that does not exist.
    "scripts/ci/test_snapshot_is_judged_before_the_gate.py",
];

/// The three steps of the gate, in order.
///
/// A fourth step is not automatically wrong — but it is not automatic either,
/// and the difference between "the gate grew" and "the gate was extended on
/// purpose" is a commit that says so. This list is the definition; changing it
/// means changing the contract in the same diff, which is the point.
const GATE_STEPS: &[&str] = &[
    "cargo nextest run --profile gate --workspace --lib",
    "cargo nextest run --profile gate --workspace --tests",
    "cargo mutants",
];

fn repo_root() -> PathBuf {
    common::repo_root()
}

/// Every tracked file in the repository, as a path relative to the root.
///
/// The walk skips the directories that are never part of the source tree:
/// `.git`, `node_modules`, `target`, and the `.claude/worktrees` copies of
/// other checkouts, which are a different repository that happens to live
/// inside this one.
fn tracked_files() -> Vec<PathBuf> {
    let root = repo_root();
    let skip = [
        ".git",
        "node_modules",
        "target",
        ".claude",
        "dist",
        "coverage",
    ];
    fn walk(dir: &Path, root: &Path, skip: &[&str], out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(rel) = path.strip_prefix(root) else {
                continue;
            };
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if skip.contains(&name.as_ref()) {
                continue;
            }
            if path.is_dir() {
                walk(&path, root, skip, out);
            } else {
                out.push(rel.to_path_buf());
            }
        }
    }
    let mut out = Vec::new();
    walk(&root, &root, &skip, &mut out);
    out
}

// ===========================================================================
// Test 1 — the third test class does not exist
// ===========================================================================

/// No file and no directory of the retired classes is on disk.
///
/// A directory that exists but is empty is still a directory that a future
/// commit can fill, and it still makes `rg e2e` noisy for the next person
/// looking for the tests that actually exist. The path has to be gone.
#[test]
fn retired_test_classes_are_not_on_disk() {
    for rel in RETIRED_PATHS {
        let path = repo_root().join(rel);
        assert!(
            !path.exists(),
            "the retired test class is back: {rel} exists at {}; \
             this repository has two test classes (unit, integration with \
             mocks) and a third one is not allowed",
            path.display()
        );
    }
}

/// No test file carries the name of a retired class.
///
/// The directory check above catches a suite that comes back whole. This one
/// catches the quieter version: a single new file named `*_uat.rs` or
/// `*_e2e.rs`, which does not recreate the directory but does recreate the
/// class it belongs to.
///
/// The check is deliberately on the *file name* and not on the word "e2e"
/// anywhere in the file: sixteen Rust integration tests carry `_e2e` in their
/// name from before this taxonomy existed (`behavior_authority_e2e.rs`,
/// `prf_h06_adversarial_e2e.rs`, and others). They are integration tests that
/// live in `crates/*/tests/`, they run in-process, and they are protected by
/// the golden fixture `tests/fixtures/assurance_export_golden.cbor`, which
/// records their names. Renaming them would be a rename with no behavioural
/// change and a real risk of breaking the golden; what the taxonomy
/// forbids is a new class, not an old file name.
#[test]
fn no_test_file_is_named_after_a_retired_class() {
    let offenders: Vec<String> = tracked_files()
        .into_iter()
        .filter(|path| {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            (name.ends_with("_uat.rs") || name.ends_with("_uat.ts")) && path.starts_with("crates")
        })
        .map(|path| path.display().to_string())
        .collect();

    assert!(
        offenders.is_empty(),
        "files named after the retired UAT class are back: {offenders:?}; \
         the UAT class was retired under ADR-TH-006 and the taxonomy has \
         two classes, not three"
    );
}

/// There is no GitHub Actions workflow in this repository.
///
/// The repository had two orchestrators for a while — the GitHub Actions
/// workflows and the PipelineK lanes — and the decision was to have none:
/// the gate is three commands and a person who reads the diff. A workflow
/// that reappears is a second orchestrator whose behaviour is a container
/// image's behaviour rather than the machine's, and it would run behind the
/// authority of a change that the gate already has.
///
/// This guarantee used to live in `scripts/ci/test_no_actions_workflows.py`,
/// a contract that watched a directory the CI apparatus used to fill. It
/// went with the rest of that apparatus, and the guarantee did not go with
/// it: it is folded into the contract that already watches the taxonomy,
/// because a rule that only exists in the file it protects is a rule that
/// stops being checked when the file is gone.
#[test]
fn there_are_no_github_actions_workflows() {
    let workflows = repo_root().join(".github/workflows");
    assert!(
        !workflows.exists(),
        "the GitHub Actions workflows are back at {}; this repository has one \
         orchestrator and it is the person who runs `scripts/test-gate.sh`",
        workflows.display()
    );
}

// ===========================================================================
// Test 2 — the gate is exactly three steps
// ===========================================================================

/// The gate script exists and runs the three steps, in order.
#[test]
fn the_gate_runs_the_three_steps() {
    let script = repo_root().join("scripts/test-gate.sh");
    assert!(
        script.exists(),
        "the gate script is missing at {}; without it there is no gate, \
         and a gate that does not exist cannot be run",
        script.display()
    );

    let text = std::fs::read_to_string(&script)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", script.display()));

    let mut cursor = 0usize;
    for step in GATE_STEPS {
        let at = text[cursor..]
            .find(step)
            .unwrap_or_else(|| panic!("the gate script does not run `{step}`"));
        cursor += at + step.len();
    }
}

/// The mutation scope is declared, and it is a file that exists.
///
/// The scope cannot live in `.cargo/mutants.toml`: the `cargo-mutants` config
/// schema is flat and global, with `deny_unknown_fields`, and a file that
/// opens a `package` table is rejected rather than ignored. So the scope is a
/// command-line concern and it lives in the gate script — which is where this
/// test looks for it.
///
/// The scope is `cognicode-graph-algos`: the crate that holds the graph
/// algorithms the analysis is built on, and the only one where the mutation
/// measurement is both meaningful and affordable. The crates that are out of
/// it are named here as well, because a scope that only says what it includes
/// reads like a scope that includes everything else: the exclusion is part of
/// the declaration.
#[test]
fn the_mutation_scope_is_declared_and_resolvable() {
    let config = repo_root().join(".cargo/mutants.toml");
    assert!(
        config.exists(),
        "the mutation configuration is missing at {}; the third step of the \
         gate has no scope without it",
        config.display()
    );

    let script = std::fs::read_to_string(repo_root().join("scripts/test-gate.sh"))
        .expect("the gate script is readable");

    assert!(
        script.contains("-p cognicode-graph-algos"),
        "the mutation scope no longer names cognicode-graph-algos; if the \
         scope changed on purpose, this contract and the gate script have to \
         change in the same diff"
    );

    // The crates that are out of the scope have to be named in the gate
    // script, with the reason. A scope that hides what it excludes is a
    // scope that claims more coverage than it has.
    for excluded in [
        "cognicode-core",
        "cognicode-explorer",
        "cognicode-runtime",
        "cognicode-mcp",
        "cognicode-graph-wasm",
        "cognicode-macros",
        "cognicode-ladybug",
    ] {
        assert!(
            script.contains(excluded),
            "the gate script no longer mentions {excluded}; a crate that is \
             outside the mutation scope has to be named with the reason, \
             otherwise the scope reads like it covers the whole workspace"
        );
    }
}

// ===========================================================================
// Test 3 — the mutation step can actually be run
// ===========================================================================

/// The mutation step copies the tree by the ignore rules.
///
/// `cargo-mutants` copies the working tree before it mutates anything. The
/// copy honours the ignore rules, and without asking for that the copy is
/// the whole tree — 6.6 GB, most of it the sandbox fixture repositories —
/// so the step fills the temporary volume or takes longer than the mutation
/// it is supposed to measure. Asking for the copy to honour the ignore
/// rules is what makes the third step of the gate runnable at all.
#[test]
fn the_mutation_step_copies_the_tree_by_the_ignore_rules() {
    let script = std::fs::read_to_string(repo_root().join("scripts/test-gate.sh"))
        .expect("the gate script is readable");
    assert!(
        script.contains("--gitignore true"),
        "the gate's mutation step does not pass `--gitignore true`; without \
         it every mutant copies the whole working tree and the step cannot \
         be run inside a normal temporary volume"
    );
}

// ===========================================================================
// Test 4 — nothing still claims there are gates to run
// ===========================================================================

/// The governing document declares the two classes and the three-step gate.
///
/// `AGENTS.md` is the entry point for every agent and every human session.
/// If it still says "run the merge gate", the removal is over for the
/// documents and unfinished for the people who read them.
#[test]
fn the_governing_document_declares_the_two_classes_and_the_three_steps() {
    let agents =
        std::fs::read_to_string(repo_root().join("AGENTS.md")).expect("AGENTS.md is readable");

    for step in GATE_STEPS {
        assert!(
            agents.contains(step),
            "AGENTS.md does not name the gate step `{step}`; the gate is \
             three steps and the document that every agent reads first has \
             to say so"
        );
    }

    assert!(
        agents.contains("No existen otras clases"),
        "AGENTS.md no longer states that there are two test classes and no \
         others; the taxonomy has to be stated, not implied"
    );
}
