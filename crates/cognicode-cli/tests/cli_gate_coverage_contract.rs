//! Gate-coverage contract for the `cognicode-cli` integration surface.
//!
//! This exists because the merge gate spent its whole history running
//! `cargo test -p cognicode-cli --lib`, which is not even a valid target
//! for this crate (it is bin-only, no `src/lib.rs`). Every black-box
//! contract in `tests/` therefore went unexecuted while reading as green,
//! and 21 of those suites were never named in any workflow.
//!
//! Two failure modes have to be caught, and they are not the same shape:
//!
//!   1. The gate step disappears. Caught by requiring the selector.
//!   2. The gate step is *narrowed* to a selector that still looks
//!      responsible, e.g. `cargo test -p cognicode-cli --features ladybug
//!      --bins`. That keeps the 325 bin tests and silently drops all 21
//!      `tests/` targets, re-opening exactly the gap this contract
//!      exists to close, while every substring assertion below still
//!      passes.
//!
//! So the primary assertion is a *count against the filesystem*: every
//! `.rs` file in `tests/` must be a target the gate actually compiles.
//! A substring check cannot catch mode 2; a count can.
//!
//! The secondary assertion is the one that would have caught the original
//! defect on its own: `cargo test -p cognicode-cli --lib` must not appear
//! in any workflow, because it is both wrong for this crate and a
//! selector that skips every `tests/` target by construction.
//!
//! Third assertion, added after the gate went red for real: the gate can
//! also fail by *executing* a suite whose preconditions the workflow does
//! not create. See
//! `the_gate_provides_what_the_release_flow_suites_require`.

use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/cognicode-cli has a parent")
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
    let dir = repo_root().join("crates/cognicode-cli/tests");
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

/// The `cargo test -p cognicode-cli ...` invocations present in the
/// workflow, one entry per line of actual command text.
fn cli_test_invocations(workflow: &str) -> Vec<String> {
    workflow
        .lines()
        .map(str::trim)
        .filter(|l| l.contains("cargo test -p cognicode-cli"))
        .map(|l| {
            // Keep only the cargo invocation, dropping any YAML noise or
            // trailing comments, so a selector can be compared as a whole.
            l.split('#').next().unwrap_or(l).trim().to_string()
        })
        .collect()
}

/// A selector that compiles every `tests/*.rs` target and every bin.
///
/// "Unrestricted" means the workflow names a package and no target at
/// all, which is what makes cargo expand the selection to lib + bins +
/// every `tests/` target + doctests. Naming any target narrows it, and
/// the narrowings are not equivalent:
///
///   * `--lib`   — invalid here (bin-only crate) and skips `tests/`
///   * `--bins`  — keeps the 325 bin tests, drops all 21 `tests/` targets
///   * `--tests` — keeps the integration targets, drops the bins
///   * `--test X`— compiles exactly one target
fn is_unrestricted_cli_selector(command: &str) -> bool {
    if !command.contains("cargo test -p cognicode-cli") {
        return false;
    }
    // Any target-narrowing flag disqualifies it. This is a blocklist and
    // not an allowlist on purpose: a new cargo narrowing flag added later
    // should be caught here rather than silently pass as "unrestricted".
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
fn no_workflow_runs_lib_only_for_the_cli_crate() {
    // The original defect. `--lib` is invalid for a bin-only crate, and
    // even if it resolved it would skip every `tests/` target by
    // construction, which is how 21 suites stayed green and unexecuted.
    let workflow = read_workflow();
    let offenders: Vec<&str> = workflow
        .lines()
        .map(str::trim)
        .filter(|l| l.contains("cargo test -p cognicode-cli --lib"))
        .collect();
    assert!(
        offenders.is_empty(),
        "pr-ci.yml runs `cargo test -p cognicode-cli --lib` at: {offenders:?}. \
         cognicode-cli is bin-only, and `--lib` skips every `tests/` target, \
         which is the blind spot that left 21 integration suites unexecuted."
    );
}

#[test]
fn the_merge_gate_compiles_every_integration_suite() {
    // The count-based assertion that a substring check cannot make: the
    // gate must contain at least one unrestricted invocation, so newly
    // added suites are picked up without anyone editing the workflow.
    let workflow = read_workflow();
    let invocations = cli_test_invocations(&workflow);
    let unrestricted: Vec<&String> = invocations
        .iter()
        .filter(|c| is_unrestricted_cli_selector(c))
        .collect();
    assert!(
        !unrestricted.is_empty(),
        "pr-ci.yml has no unrestricted `cargo test -p cognicode-cli` \
         invocation. Found only: {invocations:?}. Without one, every \
         `tests/*.rs` target and every bin target depends on individual \
         `--test` steps, and the next suite added to the crate is \
         ungated by default."
    );
}

#[test]
fn every_suite_is_either_gated_by_name_or_covered_by_the_unrestricted_step() {
    // A test suite in this crate is protected if EITHER a named `--test`
    // step exists OR an unrestricted step compiles it. This is the
    // assertion that fails loudly when someone replaces the broad step
    // with a narrower selector while leaving the old named steps intact.
    let workflow = read_workflow();
    let invocations = cli_test_invocations(&workflow);
    let has_unrestricted = invocations.iter().any(|c| is_unrestricted_cli_selector(c));
    let named: Vec<&str> = invocations
        .iter()
        .filter_map(|c| c.split("--test ").nth(1))
        .filter_map(|rest| rest.split_whitespace().next())
        .collect();

    let suites = integration_suite_names();
    assert!(
        suites.len() >= 20,
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
        "{} integration suites are neither named in a `--test` step nor \
         covered by an unrestricted step: {ungated:?}. All {} suites: \
         {suites:?}",
        ungated.len(),
        suites.len()
    );
}

#[test]
fn the_ladybug_feature_is_built_by_the_gate() {
    // A suite behind a crate-level `#![cfg(feature = "ladybug")]` compiles
    // to an empty binary when the feature is off: 0 tests, exit 0, green
    // under any invocation, including a manual one. That is strictly worse
    // than never being run, because it reports success.
    //
    // Asserting the feature is present in the unrestricted step is what
    // makes those tests reachable at all.
    let workflow = read_workflow();
    let invocations = cli_test_invocations(&workflow);
    let unrestricted: Vec<&String> = invocations
        .iter()
        .filter(|c| is_unrestricted_cli_selector(c))
        .collect();
    assert!(
        !unrestricted.is_empty(),
        "no unrestricted CLI invocation to carry the feature flag: {invocations:?}"
    );
    let with_feature = unrestricted
        .iter()
        .any(|c| c.contains("--features") && c.contains("ladybug"));
    assert!(
        with_feature,
        "the unrestricted CLI step does not pass --features ladybug. \
         cognicode-cli's default feature set is empty, so \
         `evidence_cli_mcp_equivalence` compiles to an empty test binary \
         and reports 0 passed / 0 failed / exit 0. The restricted \
         selectors found: {unrestricted:?}"
    );
}

#[test]
fn the_gate_provides_what_the_release_flow_suites_require() {
    // PR #307, run 36496128455. The unrestricted step compiles every
    // `tests/*.rs` target, and four of those
    // (`prf_dist_01_06_release_candidate_uat`,
    // `prf_dist_workflow_flatten_uat`, `prf_f6_w1_release_coherence`,
    // `prf_f6_w2_staging_contract`) shell out to real release-profile
    // binaries via `common::release_bin_path()` / `common::release_dir()`.
    //
    // The gate job did not build them, so merge-gate went red with
    // `missing release binary cogh at target/release/cogh`. It stayed
    // invisible locally because a developer machine with a global
    // `target-dir` override has those binaries sitting in the shared
    // release dir, so the same command reports 27 passed / 0 failed.
    //
    // Two things are worth pinning and they are not the same assertion:
    //
    //   1. the four suites that need release binaries are still compiled
    //      by the gate (narrowing the selector to hide them would convert
    //      a red gate into an ungated suite, which is the original defect
    //      wearing a different hat), and
    //   2. the gate creates the binaries they need. Removing this step
    //      is the regression that produced the red build, so it has to
    //      fail here rather than at 23:17 in a remote log nobody reads.
    let workflow = read_workflow();

    let release_dependent = [
        "prf_dist_01_06_release_candidate_uat",
        "prf_dist_workflow_flatten_uat",
        "prf_f6_w1_release_coherence",
        "prf_f6_w2_staging_contract",
    ];
    let invocations = cli_test_invocations(&workflow);
    let has_unrestricted = invocations.iter().any(|c| is_unrestricted_cli_selector(c));
    assert!(
        has_unrestricted,
        "no unrestricted CLI selector: {invocations:?}. The four release-flow \
         suites below depend on it to be compiled at all."
    );

    for suite in release_dependent {
        let file = repo_root()
            .join("crates/cognicode-cli/tests")
            .join(format!("{suite}.rs"));
        assert!(
            file.exists(),
            "release-flow suite {suite} no longer exists; update the list in \
             this contract to match reality instead of leaving a stale pin"
        );
    }

    // The precondition itself. `cargo build --release` for the CLI crate
    // produces `cogh`, `cognicode` and `cognicode-release`, which is
    // exactly the set the four suites resolve through
    // `common::release_bin_path()` and `common::release_dir()`.
    let builds_release = workflow.contains("cargo build --release -p cognicode-cli");
    assert!(
        builds_release,
        "the merge gate runs suites that require real release-profile \
         binaries (cogh, cognicode, cognicode-release) but never builds \
         them. On a clean CI runner `target/release/` does not exist, so \
         `common::release_dir()` falls through to a directory that is not \
         there and the suites panic with `missing release binary cogh`. \
         Locally this is masked by whatever release binaries happen to \
         exist on the developer's machine. Add the release build to the \
         merge-gate job, before the unrestricted step."
    );
}

/// The A-016 MCP contract suite must itself be gated. It is the only
/// pin for the class of drift R5 cares about (published tool inventory
/// vs runtime `MUTATING_TOOLS`); if no step in `pr-ci.yml` compiles it,
/// a tool added to the runtime without a contract entry stays invisible
/// to every required check and merge goes green anyway.
#[test]
fn the_a016_tools_runtime_consistency_suite_is_gated() {
    let workflow = read_workflow();
    assert!(
        workflow.contains("--test a016_tools_runtime_consistency"),
        "pr-ci.yml never references a016_tools_runtime_consistency, so the \
         only contract↔runtime mutating-tools drift pin is ungated: a change \
         breaking it reads green at merge time. Add a named step \
         `cargo test -p cognicode-mcp --test a016_tools_runtime_consistency \
         --quiet` to the merge-gate job, next to the other black-box MCP \
         contract steps"
    );
}

/// MCP integration suites deliberately left out of the *required* gate
/// surface, each with the reason. This is the explicit side of the
/// named-or-excluded contract for `crates/cognicode-mcp/tests/*.rs`: a
/// suite not in this table and not named by a workflow step is an
/// ungated hole.
///
/// Nothing here is excluded for being "slow". `continuation_e2e` needs
/// the RUST_SANDBOX_BOOTSTRAP=1 Tier-1 fixture repos (gitignored
/// bootstrap artifacts, suite self-skips without them);
/// `prf_mcp_03_network_off_uat` needs `unshare -rn` (rootless network
/// namespaces are not guaranteed on every CI runner image, and the
/// suite hard-fails the anti-vacuity guard if unshare is missing).
/// The rest of the MCP `tests/` surface is gated: every other suite
/// runs as a named merge-gate step.
const EXCLUDED_MCP_SUITES: &[(&str, &str)] = &[
    (
        "continuation_e2e",
        "requires RUST_SANDBOX_BOOTSTRAP=1 Tier-1 fixture repos under sandbox/repos; self-skips without the bootstrap preflight",
    ),
    (
        "prf_mcp_03_network_off_uat",
        "requires unshare -rn (rootless network namespace); not guaranteed on every CI runner image and the suite's anti-vacuity guard fails without it",
    ),
];

/// Every MCP integration-suite source file, sorted, `common` excluded
/// (it is a shared harness module, not a test target).
fn mcp_integration_suite_names() -> Vec<String> {
    let dir = repo_root().join("crates/cognicode-mcp/tests");
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            if path.extension()?.to_str()? != "rs" {
                return None;
            }
            let stem = path.file_stem()?.to_str()?.to_string();
            if stem == "common" {
                return None;
            }
            Some(stem)
        })
        .collect();
    names.sort();
    names
}

/// `cargo test -p cognicode-mcp ...` invocations in the workflow.
fn mcp_test_invocations(workflow: &str) -> Vec<String> {
    workflow
        .lines()
        .map(str::trim)
        .filter(|l| l.contains("cargo test -p cognicode-mcp"))
        .map(|l| l.split('#').next().unwrap_or(l).trim().to_string())
        .collect()
}

/// Total named-or-excluded coverage for the MCP crate: each `tests/*.rs`
/// must either be named by a workflow step or sit in the exclusion table
/// with a reason. The broad MCP gate step is `--lib`, which does not
/// compile `tests/`, so "covered by an unrestricted step" is not a valid
/// escape hatch here and the enumeration is the whole contract.
#[test]
fn every_mcp_suite_is_named_by_the_gate_or_excluded_with_a_reason() {
    let workflow = read_workflow();
    let invocations = mcp_test_invocations(&workflow);
    let named: Vec<&str> = invocations
        .iter()
        .filter_map(|c| c.split("--test ").nth(1))
        .filter_map(|rest| rest.split_whitespace().next())
        .collect();

    let suites = mcp_integration_suite_names();
    assert!(
        suites.len() >= 20,
        "expected the MCP crate to still carry its full integration \
         surface, found only {} suites; the contract is not measuring \
         what it thinks it is: {suites:?}",
        suites.len()
    );

    let excluded: Vec<&str> = EXCLUDED_MCP_SUITES.iter().map(|(s, _)| *s).collect();
    let mut holes: Vec<&String> = Vec::new();
    for s in &suites {
        if named.contains(&s.as_str()) || excluded.contains(&s.as_str()) {
            continue;
        }
        holes.push(s);
    }
    assert!(
        holes.is_empty(),
        "{} MCP integration suites are neither named by a workflow step nor \
         listed in EXCLUDED_MCP_SUITES with a reason: {holes:?}. Gate them \
         with a named step (fast/local suites) or document the exclusion \
         with its reason; a silent hole re-reads as coverage it does not \
         have. All {} suites: {suites:?}",
        holes.len(),
        suites.len()
    );

    // Keep the table honest in both directions: an entry whose suite no
    // longer exists is a stale excuse, not documentation.
    for (name, reason) in EXCLUDED_MCP_SUITES {
        assert!(
            suites.contains(&name.to_string()),
            "EXCLUDED_MCP_SUITES lists {name} but no such suite exists \
             anymore; delete the entry instead of keeping a stale excuse"
        );
        assert!(
            !reason.is_empty(),
            "exclusion entry {name} has an empty reason; every exclusion \
             must say why it is not gated"
        );
    }
    for name in &named {
        assert!(
            !excluded.contains(name),
            "suite {name} is both named by a workflow step and listed in \
             EXCLUDED_MCP_SUITES; pick one side of the contract"
        );
    }
}

#[test]
fn the_coverage_contract_itself_is_pinned() {
    // Self-reference. A contract that protects coverage but is itself
    // ungated disappears exactly as quietly as the gap it closes.
    let path = repo_root().join("crates/cognicode-cli/tests");
    assert!(
        Path::new(&path)
            .join("cli_gate_coverage_contract.rs")
            .exists(),
        "this contract file is missing from tests/; it cannot be running"
    );
    let workflow = read_workflow();
    assert!(
        workflow.contains("--test cli_gate_coverage_contract"),
        "cli_gate_coverage_contract is not named in pr-ci.yml, so the \
         coverage guarantee is itself ungated."
    );
}
