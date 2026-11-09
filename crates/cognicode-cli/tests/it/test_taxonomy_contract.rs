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
///
/// What identifies a step is the runner and what it runs, NOT the scope: since
/// TASK-142 the radius of the gate is the answer of `scripts/gate_scope.py`,
/// so the literal `--workspace` and the literal `-p <crate>` are no longer in
/// the command line. A step is still a step when the scope is computed.
const GATE_STEPS: &[&str] = &[
    "cargo nextest run --profile gate",
    "--lib",
    "cargo nextest run --profile gate",
    "--tests",
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
/// Since TASK-142 the crates that are mutated are the ones the batch touched,
/// and the command line takes them from the answer of `scripts/gate_scope.py`.
/// So the scope is no longer a literal `-p cognicode-graph-algos`: what the
/// gate declares is which crates are AFFORDABLE to mutate, and which crate the
/// CALENDAR measures in its turn. `cognicode-graph-algos` is that crate: it
/// holds the graph algorithms the analysis is built on, and it is the only one
/// where the mutation measurement is both meaningful and affordable.
///
/// The crates that are out of the scope are named here as well, because a
/// scope that only says what it includes reads like a scope that includes
/// everything else: the exclusion is part of the declaration.
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
        script.contains("MUTANT_AFFORDABLE_PACKAGES"),
        "the gate script no longer declares which crates are affordable to \
         mutate; the scope of the mutation step has to be written down \
         somewhere"
    );

    assert!(
        script.contains("MUTANT_CALENDAR_PACKAGE"),
        "the gate script no longer declares which crate the calendar measures \
         in its turn; a calendar without a crate is a calendar that measures \
         nothing"
    );

    assert!(
        script.contains("cognicode-graph-algos"),
        "the mutation scope no longer names cognicode-graph-algos; if the \
         scope changed on purpose, this contract and the gate script have to \
         change in the same diff"
    );

    // The crates that are out of the scope have to be named in the gate
    // script, with the reason. A scope that hides what it excludes is a
    // scope that claims more coverage than it has.
    for excluded in [
        "cognicode-core",
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

// ===========================================================================
// Test 5 — the tests that share the tree take turns
// ===========================================================================

/// The test modules that write into the repository tree.
///
/// A test that writes into `crates/` shares state with every other test
/// that does the same: they delete and rewrite the same files. Under
/// `cargo test` the `#[serial]` annotations took turns; under nextest each
/// test is its own process, so the annotations no longer serialize
/// anything. The three tests of the SBOM contract delete each other's
/// files: each one removes the SBOMs the others are producing, and the
/// one that finds its file gone fails with "read
/// crates/cogh-x86_64-unknown-linux-gnu.cdx.json: No such file or
/// directory".
fn modules_that_write_into_the_tree() -> Vec<String> {
    let root = repo_root();
    let mut out = Vec::new();

    // The test trees of every crate: a source file that mentions
    // `crates/` is not a test that shares state with other tests.
    let mut pending = Vec::new();
    let Ok(crates) = std::fs::read_dir(root.join("crates")) else {
        return out;
    };
    for entry in crates.flatten() {
        let tests = entry.path().join("tests");
        if tests.is_dir() {
            pending.push(tests);
        }
    }
    while let Some(dir) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            if writes_into_the_tree(&text)
                && let Some(name) = path.file_stem().and_then(|n| n.to_str())
            {
                out.push(name.to_string());
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// True when the module WRITES into the repository tree.
///
/// A path into the tree and a write are two different facts, and only
/// the two together are shared state: a test that READS a source file
/// from the tree shares nothing, and a test that writes into its own
/// temp dir shares nothing either.
///
/// The resolution is textual, but it is not naive: the tree path
/// usually travels through a variable —`let path = repo_root().join(...)`,
/// then `for p in &paths`, then `fs::remove_file(&p)`— so the detector
/// follows the names it taints. A path is tainted when it comes from
/// `repo_root()`; a write is shared when the argument it takes is a
/// tainted name, or when the write itself names the tree.
fn writes_into_the_tree(text: &str) -> bool {
    let mut tainted: Vec<String> = Vec::new();

    for chunk in text.split(';') {
        let chunk = chunk.trim();

        // A binding: `let path = repo_root().join(...)`.
        let binding = chunk
            .strip_prefix("let ")
            .map(|rest| rest.strip_prefix("mut ").unwrap_or(rest));
        if let Some(rest) = binding
            && let Some((lhs, rhs)) = rest.split_once('=')
        {
            let name = leading_identifier(lhs);
            let rooted = (rhs.contains("repo_root()") && !is_build_directory(rhs))
                || names_in(rhs).iter().any(|n| is_tainted(n, &tainted));
            if !name.is_empty() && rooted {
                taint(&mut tainted, &name);
            }
        }

        // A loop over tree paths: `for p in &paths`.
        if let Some(rest) = chunk.strip_prefix("for ")
            && let Some((var, iter)) = rest.split_once(" in ")
        {
            let name = leading_identifier(var);
            let rooted = (iter.contains("repo_root()") && !is_build_directory(iter))
                || names_in(iter).iter().any(|n| is_tainted(n, &tainted));
            if !name.is_empty() && rooted {
                taint(&mut tainted, &name);
            }
        }

        // A write whose DESTINATION is a tree path.
        //
        // The destination is not always the first argument: `fs::copy`
        // and `fs::rename` read from the source and write to the
        // target, so a copy that READS a tracked file out of the tree
        // is not shared state.
        for (op, destination) in [
            ("fs::write", 0),
            ("fs::remove_file", 0),
            ("fs::remove_dir_all", 0),
            ("fs::create_dir_all", 0),
            ("fs::create_dir", 0),
            ("fs::rename", 1),
            ("fs::copy", 1),
        ] {
            let Some(at) = chunk.find(op) else {
                continue;
            };
            let call_arguments = arguments(&chunk[at..]);
            let Some(argument) = call_arguments.get(destination) else {
                continue;
            };
            if argument.contains("repo_root()")
                || names_in(argument).iter().any(|n| is_tainted(n, &tainted))
            {
                return true;
            }
        }

        // A shell command that mutates the tree: `rm -f crates/...`.
        //
        // The verb has to be the START of a quoted command, not a
        // substring somewhere in a word: "confirm" ends in "rm", and a
        // detector that cannot tell the two flags a test that only
        // reads.
        if chunk.contains("Command::new") {
            for literal in quoted_literals(chunk) {
                let command = literal.trim_start();
                if ["rm ", "mv ", "cp ", "mkdir "]
                    .iter()
                    .any(|verb| command.starts_with(verb))
                    && literal.contains("crates/")
                {
                    return true;
                }
            }
        }
    }

    false
}

/// True when the path lands in the build directory.
///
/// `target/` is cargo's, not the tests': cargo writes there while the
/// suite runs, and a test that stages a scratch directory under it
/// with a unique name shares nothing with another test. The shared
/// state this contract is about lives in the SOURCE tree —a tracked
/// file that two tests would write over each other.
fn is_build_directory(text: &str) -> bool {
    text.contains("join(\"target\")")
        || text.contains("join(\"target/")
        || text.contains("CARGO_TARGET_DIR")
}

/// The identifier a binding starts with: `path` out of `path: PathBuf`.
fn leading_identifier(text: &str) -> String {
    text.trim()
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect()
}

/// The identifiers of a piece of source text.
fn names_in(text: &str) -> Vec<String> {
    text.split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

fn is_tainted(name: &str, tainted: &[String]) -> bool {
    tainted.iter().any(|t| t == name)
}

fn taint(tainted: &mut Vec<String>, name: &str) {
    if !tainted.iter().any(|t| t == name) {
        tainted.push(name.to_string());
    }
}

/// The contents of the double-quoted literals of a piece of source.
fn quoted_literals(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find('"') {
        let after = &rest[open + 1..];
        match after.find('"') {
            Some(close) => {
                out.push(after[..close].to_string());
                rest = &after[close + 1..];
            }
            None => break,
        }
    }
    out
}

/// The arguments of a call, split on the commas that are not nested.
fn arguments(call: &str) -> Vec<String> {
    let Some(open) = call.find('(') else {
        return Vec::new();
    };
    let rest = &call[open + 1..];
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut current = String::new();
    for c in rest.chars() {
        match c {
            '(' | '[' | '{' => {
                depth += 1;
                current.push(c);
            }
            ')' | ']' | '}' => {
                if depth == 0 {
                    out.push(current.trim().to_string());
                    return out;
                }
                depth -= 1;
                current.push(c);
            }
            ',' if depth == 0 => {
                out.push(current.trim().to_string());
                current.clear();
            }
            _ => current.push(c),
        }
    }
    out.push(current.trim().to_string());
    out
}

/// The filters of the nextest groups that run one test at a time.
///
/// `.config/nextest.toml` is the containment for the state that nextest
/// stopped sharing. The file is small and regular — `[test-groups.<name>]`
/// declares `max-threads`, and `[[profile.gate.overrides]]` binds a filter
/// to a group — so reading it as text is enough, and it keeps this
/// contract independent of a TOML parser.
fn single_threaded_filters() -> Vec<String> {
    let text = std::fs::read_to_string(repo_root().join(".config/nextest.toml"))
        .expect(".config/nextest.toml is readable");

    let mut group_threads: Vec<(String, usize)> = Vec::new();
    let mut current_group: Option<String> = None;
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("[test-groups.") {
            current_group = Some(rest.trim_end_matches(']').to_string());
            continue;
        }
        if let Some(group) = current_group.clone()
            && let Some(threads) = line
                .strip_prefix("max-threads = ")
                .and_then(|n| n.trim().parse::<usize>().ok())
        {
            group_threads.push((group, threads));
            continue;
        }
        if line.starts_with('[') {
            current_group = None;
        }
    }

    let mut overrides: Vec<(String, String)> = Vec::new();
    let mut filter: Option<String> = None;
    let mut group: Option<String> = None;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with("[[profile.gate.overrides]]") {
            if let (Some(f), Some(g)) = (filter.take(), group.take()) {
                overrides.push((f, g));
            }
            continue;
        }
        if let Some(value) = line.strip_prefix("filter = ") {
            filter = Some(value.trim().trim_matches('\'').to_string());
        }
        if let Some(value) = line.strip_prefix("test-group = ") {
            group = Some(value.trim().trim_matches('\'').to_string());
        }
    }
    if let (Some(f), Some(g)) = (filter, group) {
        overrides.push((f, g));
    }

    overrides
        .into_iter()
        .filter(|(_, group)| {
            group_threads
                .iter()
                .any(|(name, threads)| name == group && *threads == 1)
        })
        .map(|(filter, _)| filter)
        .collect()
}

/// Every test that writes into the tree is in a group of one thread.
///
/// The groups are the containment, and a containment that nobody checks is
/// a containment that gets deleted the first time somebody tidies the
/// configuration file.
///
/// The detection resolves the tree path through the names it travels by,
/// so a test that READS a tracked file, or that writes into its own temp
/// directory, is not mistaken for a test that shares the tree. The build
/// directory (`target/`) is cargo's, not the tests'.
///
/// What this does NOT check: a shared state reached through a shape the
/// resolution does not follow — a path that crosses a function boundary,
/// for instance. Such a test needs its own group, and this contract will
/// not say so.
#[test]
fn the_tests_that_share_the_tree_take_turns() {
    let shared = modules_that_write_into_the_tree();
    assert!(
        !shared.is_empty(),
        "no test module writes into the repository tree, so this contract \
         is looking at nothing; the detection has stopped seeing the shape \
         it was written for"
    );
    assert!(
        shared.iter().any(|m| m == "prf_f6_w3_bis_sbom_contract"),
        "the SBOM contract is the module this contract was written for — the \
         one that writes `crates/<component>-<target>.cdx.json` into the tree \
         and deletes it again. The detection has stopped seeing it: {shared:?}"
    );

    let filters = single_threaded_filters();
    for module in shared {
        assert!(
            filters.iter().any(|filter| filter.contains(&module)),
            "the tests of `{module}` write into the repository tree, so they \
             share state with each other; under nextest each test is its own \
             process and `#[serial]` no longer takes turns — they need a \
             group with `max-threads = 1`, and no group covers them: {filters:?}"
        );
    }
}
