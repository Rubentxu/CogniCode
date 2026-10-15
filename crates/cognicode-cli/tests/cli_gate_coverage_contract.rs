//! Gate-coverage contract for the `cognicode-cli` integration surface.
//!
//! The sibling contract for `cognicode-core`
//! (`crates/cognicode-core/tests/core_gate_coverage_contract.rs`) documents
//! why counting beats substring-matching: narrowing a selector to `--bins`
//! or `--tests` keeps hundreds of tests and silently drops every
//! `tests/*.rs` target, while every `contains("cargo test")` assertion
//! still passes. This file is the same argument applied to the CLI crate.
//!
//! ## One suite, one owning lane (B1, 2026-10-15)
//!
//! Until this change the merge authority ran one *unrestricted* invocation per
//! crate — `cargo test -p cognicode-cli --features ladybug` — and what this file
//! protected was "that invocation still exists and is still broad". It was broad
//! for a reason that stopped being true: five of the crate's suites tar,
//! generate and verify release-profile artifacts, so the merge authority built
//! four release artifacts on every merge to feed them. B1 moved those suites,
//! and the builds that fed them, to `release-candidate.pipeline.kts`, the lane
//! that produces release bytes as the candidate itself (ADR-TH-005).
//!
//! That splits the surface in two, and the split is what this file now pins:
//!
//!   * `merge-gate.pipeline.kts` names every `tests/*.rs` target **except** the
//!     release-profile ones, and builds the debug-profile binaries those suites
//!     spawn by path;
//!   * `release-candidate.pipeline.kts` names **every** release-profile suite,
//!     and builds the release artifacts they consume.
//!
//! Four properties, none of them a substring check:
//!
//!   1. **Exhaustive.** Merge plus candidate name every `tests/*.rs` target of
//!      the crate. A suite added to the crate and named in neither lane fails
//!      here rather than never running.
//!   2. **No vacuous owner.** The merge authority names no release-profile suite:
//!      it no longer builds what they need, so naming one would be a red gate
//!      waiting for an artifact that is not there — and the "fix" for that red
//!      gate is the skip this contract exists to prevent.
//!   3. **Measured, not declared.** The release-profile set is recomputed from
//!      the suites' own source: a suite that resolves `release_bin_path()` or
//!      `release_dir()` must be in the declared list. Declaring it is the edit
//!      that moves it to the lane that owns it.
//!   4. **The preconditions exist.** Merge builds the debug binaries its
//!      subprocess suites spawn; the candidate lane builds the release binaries
//!      the moved suites tar.
//!
//! Nothing here was excluded for being slow, and nothing was replaced with a
//! skip: a suite that cannot run in a lane fails loudly where it does run.

mod common;

use common::{RELEASE_CANDIDATE_AUTHORITY, merge_authority_lines, pipeline_text};
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/cognicode-cli has a parent")
        .parent()
        .expect("crates/ has a parent")
        .to_path_buf()
}

/// The merge authority's command text, one entry per line.
///
/// The orchestrator that gates a merge is `merge-gate.pipeline.kts`; it used
/// to be `.github/workflows/pr-ci.yml`. Which file that is, is declared once
/// in `common` rather than re-derived here, so a second orchestrator cannot
/// quietly become a third source of truth.
fn merge_authority_text() -> String {
    merge_authority_lines().join("\n")
}

/// The candidate lane's command text, same shape.
fn candidate_authority_text() -> String {
    pipeline_text(RELEASE_CANDIDATE_AUTHORITY)
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

/// Lines with `\`-continuations joined, so a command that wraps counts as one.
///
/// A cargo invocation that carries 49 `--test` flags is written across 49 lines
/// for the reader, and a line-based scan would see only the first of them. The
/// joining is what lets the count stay a count.
fn logical_lines(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut current = String::new();
    for line in text.lines() {
        let trimmed = line.trim();
        match trimmed.strip_suffix('\\') {
            Some(head) => {
                current.push_str(head);
                current.push(' ');
            }
            None => {
                current.push_str(trimmed);
                out.push(std::mem::take(&mut current));
            }
        }
    }
    if !current.trim().is_empty() {
        out.push(current);
    }
    out
}

/// The `cargo test -p cognicode-cli ...` invocations present in one lane.
fn cli_invocations(text: &str) -> Vec<String> {
    logical_lines(text)
        .into_iter()
        .filter(|l| l.contains("cargo test -p cognicode-cli"))
        .collect()
}

/// Every `--test <name>` mentioned by a lane's CLI invocations, sorted, unique.
fn cli_named_targets(text: &str) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for invocation in cli_invocations(text) {
        let mut rest = invocation.as_str();
        while let Some(at) = rest.find("--test ") {
            rest = &rest[at + "--test ".len()..];
            let name = rest
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .to_string();
            if !name.is_empty() && !name.starts_with('-') {
                names.push(name);
            }
        }
    }
    names.sort();
    names.dedup();
    names
}

/// A selector that is `--lib` and nothing else, the historical defect for this
/// bin-only crate: it is invalid for the crate and skips every `tests/` target.
///
/// `--lib --bins` is not that selector: the unit surface legitimately lives in
/// `src/` and in the bin targets, and the integration targets are named by the
/// enumerated invocation next to it. What is rejected is `--lib` without
/// `--bins`, which is a selector that reads responsible and runs no bins.
fn is_lib_without_bins(command: &str) -> bool {
    command.contains("--lib") && !command.contains("--bins")
}

/// Suites that execute release-profile artifacts. Owner: the candidate lane.
///
/// Each of these resolves `common::release_bin_path()` (the `cognicode-release`
/// tool) or `common::release_dir()` (the directory holding `cogh`, `cognicode`
/// and `cognicode-mcp`), and each of them tar s, generates or verifies those
/// bytes. None of them can be evidence in a lane that builds no release binary;
/// in this lane they would be a red gate whose cheapest repair is a skip.
const RELEASE_PROFILE_SUITES: &[&str] = &[
    "prf_dist_01_06_release_candidate_uat",
    "prf_dist_workflow_flatten_uat",
    "prf_f6_w1_release_coherence",
    "prf_f6_w2_staging_contract",
    "published_skill_bundles_are_required",
];

fn declared_release_profile_suites() -> Vec<String> {
    let mut names: Vec<String> = RELEASE_PROFILE_SUITES
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    names.sort();
    names
}

/// The release-profile set, measured from the suites' own source.
///
/// The scan is deliberately textual: a suite that spawns a real release binary
/// does it through these two helpers, and a suite that mentions them in prose
/// lands in the same bucket. Over-inclusion costs a declaration (and therefore a
/// move to the lane that can run it); under-inclusion is what this exists to
/// catch, because a release-dependent suite that runs in `merge-gate` is a red
/// gate whose cheapest repair is a skip.
fn measured_release_profile_suites() -> Vec<String> {
    let dir = repo_root().join("crates/cognicode-cli/tests");
    let mut names: Vec<String> = Vec::new();
    for entry in
        std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
    {
        let path = entry.expect("dir entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string();
        // `common` is the shared harness, not a target; this file is the
        // measuring instrument and names the helpers on purpose.
        if stem == "common" || stem == "cli_gate_coverage_contract" {
            continue;
        }
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        if source.contains("release_bin_path(") || source.contains("release_dir()") {
            names.push(stem);
        }
    }
    names.sort();
    names
}

#[test]
fn the_release_profile_set_is_the_measured_one() {
    // A suite that starts exercising release bytes without being declared here
    // would keep running in a lane that builds none of them; declaring it is the
    // act that moves it. This is the assertion that makes the matrix maintain
    // itself instead of being a list somebody remembers to update.
    assert_eq!(
        measured_release_profile_suites(),
        declared_release_profile_suites(),
        "the release-profile set measured from `crates/cognicode-cli/tests/*.rs` \
         does not match RELEASE_PROFILE_SUITES. A suite that resolves \
         `release_bin_path()` or `release_dir()` executes release-profile bytes, \
         and the lane that builds them is `{RELEASE_CANDIDATE_AUTHORITY}`: add it \
         to RELEASE_PROFILE_SUITES and to the candidate lane's release-contracts \
         group, and remove it from the merge enumeration."
    );
}

#[test]
fn no_orchestrator_runs_lib_only_for_the_cli_crate() {
    // The original defect. `--lib` is invalid for a bin-only crate, and even if
    // it resolved it would skip every `tests/` target by construction, which is
    // how 21 suites stayed green and unexecuted. `--lib --bins` is the unit
    // surface and is fine; `--lib` alone is not.
    for (lane, text) in [
        ("merge-gate.pipeline.kts", merge_authority_text()),
        (RELEASE_CANDIDATE_AUTHORITY, candidate_authority_text()),
    ] {
        let offenders: Vec<String> = cli_invocations(&text)
            .into_iter()
            .filter(|c| is_lib_without_bins(c))
            .collect();
        assert!(
            offenders.is_empty(),
            "{lane} runs `cargo test -p cognicode-cli --lib` without `--bins`: \
             {offenders:?}. cognicode-cli is bin-only, and `--lib` skips every \
             `tests/` target, which is the blind spot that left 21 integration \
             suites unexecuted."
        );
    }
}

#[test]
fn merge_runs_every_cli_suite_that_does_not_need_release_artifacts() {
    let text = merge_authority_text();
    let named = cli_named_targets(&text);
    let suites = integration_suite_names();
    let declared = declared_release_profile_suites();

    let expected: Vec<String> = suites
        .iter()
        .filter(|s| !declared.contains(*s))
        .cloned()
        .collect();
    let missing: Vec<&String> = expected.iter().filter(|s| !named.contains(s)).collect();
    assert!(
        missing.is_empty(),
        "{} of {} CLI integration suites are not named by the merge authority: \
         {missing:?}. Without a `--test` step each of them depends on somebody \
         remembering, and the next suite added to the crate is ungated by \
         default. All named: {named:?}",
        missing.len(),
        expected.len()
    );

    let unexpected: Vec<&String> = named
        .iter()
        .filter(|s| suites.contains(*s) && declared.contains(*s))
        .collect();
    assert!(
        unexpected.is_empty(),
        "the merge authority names release-profile suites it cannot run: \
         {unexpected:?}. It builds no release artifact since B1 (ADR-TH-005), so \
         these are a red gate waiting for a binary that is not there. Their lane \
         is {RELEASE_CANDIDATE_AUTHORITY}."
    );
}

#[test]
fn the_candidate_lane_owns_every_release_profile_suite() {
    let text = candidate_authority_text();
    let named = cli_named_targets(&text);
    let declared = declared_release_profile_suites();
    let missing: Vec<&String> = declared.iter().filter(|s| !named.contains(*s)).collect();
    assert!(
        missing.is_empty(),
        "the candidate lane does not name these release-profile suites: \
         {missing:?}. Moving a suite out of the merge authority without naming it \
         in the lane that builds its preconditions converts a gated suite into an \
         ungated one, which is the original defect wearing a different hat."
    );
}

#[test]
fn the_candidate_lane_builds_the_release_artifacts_those_suites_consume() {
    // Two different producers, and neither is decoration: `toolchain` builds the
    // release tool the suites execute, and `binaries-$target` builds the three
    // components they tar. The verifier that used to live in the merge lane
    // (`verify-release-binaries`) exists here as `binary-smoke-$target`, which
    // also knows how to execute a cross target.
    let text = candidate_authority_text();
    for (needle, why) in [
        (
            "cargo build --release --bin cognicode-release",
            "the `cognicode-release` tool the moved suites execute",
        ),
        (
            "--bin cogh --bin cognicode",
            "`cogh` and `cognicode`, which PRF-DIST-01/06 and F6 tar as payloads",
        ),
        (
            "-p cognicode-mcp --bin cognicode-mcp",
            "`cognicode-mcp`, which the two-process UAT spawns from the release dir",
        ),
        (
            "binary-smoke-$target",
            "the \"exists, is executable and starts\" assertion the merge lane used \
             to make about release binaries",
        ),
    ] {
        assert!(
            text.contains(needle),
            "the candidate lane no longer contains `{needle}`, which produces {why}."
        );
    }
}

#[test]
fn the_merge_gate_builds_the_debug_binaries_its_subprocess_suites_spawn() {
    // The merge lane's own preconditions. Three shapes of consumer, and none of
    // them is satisfied by `cargo test` alone: the MCP subprocess UATs resolve
    // `cognicode-mcp` from the target directory, the `cogh` unit tests derive
    // their sibling from `current_exe`, and the core suite
    // `b3_w2_progressive_context` spawns `cognicode` by path.
    let text = merge_authority_text();
    for needle in [
        "cargo build -p cognicode-mcp",
        "cargo build -p cognicode-cli --bins",
        "cargo build --bin cognicode-control-plane",
    ] {
        assert!(
            text.contains(needle),
            "the merge authority no longer contains `{needle}`. Its suites spawn \
             those binaries by path; without the build the suites either fail or \
             (worse, and measured) resolve an older artifact."
        );
    }
    assert!(
        !text.contains("cargo build --release"),
        "the merge authority builds a release artifact again. B1 (ADR-TH-005) \
         moved every release build to {RELEASE_CANDIDATE_AUTHORITY}, because the \
         only suites that read release bytes live there now. Re-adding one here \
         needs the same decision the change recorded."
    );
}

#[test]
fn the_gate_runs_the_ladybug_feature_with_the_suites_that_need_it() {
    // A suite behind a crate-level `#![cfg(feature = "ladybug")]` compiles to an
    // empty binary when the feature is off: 0 tests, exit 0, green under any
    // invocation, including a manual one. That is strictly worse than never
    // being run, because it reports success.
    let text = merge_authority_text();
    let carriers: Vec<String> = cli_invocations(&text)
        .into_iter()
        .filter(|c| c.contains("--features") && c.contains("ladybug"))
        .collect();
    assert!(
        !carriers.is_empty(),
        "no CLI invocation of the merge authority passes `--features ladybug`. \
         cognicode-cli's default feature set is empty, so \
         `evidence_cli_mcp_equivalence` compiles to an empty test binary and \
         reports 0 passed / 0 failed / exit 0."
    );

    // The suite that carries the cfg must be reached by a carrier, not by any
    // invocation: naming it without the feature is the vacuous shape.
    let named_by_enumeration = cli_invocations(&text).into_iter().any(|c| {
        c.contains("--test evidence_cli_mcp_equivalence")
            && c.contains("--features")
            && c.contains("ladybug")
    });
    assert!(
        named_by_enumeration,
        "the invocation that runs `evidence_cli_mcp_equivalence` does not pass \
         `--features ladybug`, so the suite compiles to an empty binary. \
         Carriers found: {carriers:?}"
    );
}

/// The A-016 MCP contract suite must itself be gated. It is the only
/// pin for the class of drift R5 cares about (published tool inventory
/// vs runtime `MUTATING_TOOLS`); if nothing the merge authority runs compiles it,
/// a tool added to the runtime without a contract entry stays invisible
/// to every required check and merge goes green anyway.
#[test]
fn the_a016_tools_runtime_consistency_suite_is_gated() {
    let workflow = merge_authority_text();
    assert!(
        workflow.contains("--test a016_tools_runtime_consistency"),
        "the merge authority never references a016_tools_runtime_consistency, so the \
         only contract↔runtime mutating-tools drift pin is ungated: a change \
         breaking it reads green at merge time. Add a named stage running \
         `cargo test -p cognicode-mcp --test a016_tools_runtime_consistency \
         --quiet` to the merge-gate job, next to the other black-box MCP \
         contract stages"
    );
}

/// MCP integration suites deliberately left out of the *required* gate
/// surface, each with the reason. This is the explicit side of the
/// named-or-excluded contract for `crates/cognicode-mcp/tests/*.rs`: a
/// suite not in this table and not named by a lane step is an
/// ungated hole.
///
/// Nothing here is excluded for being "slow". `continuation_e2e` needs
/// the RUST_SANDBOX_BOOTSTRAP=1 Tier-1 fixture repos (gitignored
/// bootstrap artifacts, suite self-skips without them);
/// `prf_mcp_03_network_off_uat` needs `unshare -rn` (rootless network
/// namespaces are not guaranteed on every CI runner image, and the
/// suite hard-fails the anti-vacuity guard if unshare is missing).
/// The rest of the MCP `tests/` surface is gated: every other suite
/// runs as a named merge-gate or release-candidate step.
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

/// MCP suites that execute release-profile artifacts. Owner: candidate lane.
const RELEASE_PROFILE_MCP_SUITES: &[&str] = &["prf_cli_04_two_process_uat"];

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

/// `cargo test -p cognicode-mcp ...` invocations in one lane.
fn mcp_test_invocations(text: &str) -> Vec<String> {
    logical_lines(text)
        .into_iter()
        .filter(|l| l.contains("cargo test -p cognicode-mcp"))
        .collect()
}

/// Every `--test <name>` mentioned by a lane's MCP invocations.
fn mcp_named_targets(text: &str) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for invocation in mcp_test_invocations(text) {
        let mut rest = invocation.as_str();
        while let Some(at) = rest.find("--test ") {
            rest = &rest[at + "--test ".len()..];
            let name = rest
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .to_string();
            if !name.is_empty() && !name.starts_with('-') {
                names.push(name);
            }
        }
    }
    names.sort();
    names.dedup();
    names
}

#[test]
fn the_release_profile_mcp_set_is_the_measured_one() {
    let dir = repo_root().join("crates/cognicode-mcp/tests");
    let mut measured: Vec<String> = Vec::new();
    for entry in
        std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
    {
        let path = entry.expect("dir entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string();
        if stem == "common" {
            continue;
        }
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        if source.contains("release_dir()") || source.contains("release_bin_path(") {
            measured.push(stem);
        }
    }
    measured.sort();
    let mut declared: Vec<String> = RELEASE_PROFILE_MCP_SUITES
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    declared.sort();
    assert_eq!(
        measured, declared,
        "the release-profile MCP set measured from source does not match \
         RELEASE_PROFILE_MCP_SUITES; move the suite to the candidate lane's \
         release-contracts group and declare it here."
    );
}

#[test]
fn the_release_profile_mcp_suite_is_owned_by_the_candidate_lane() {
    let merge = merge_authority_text();
    let candidate = candidate_authority_text();
    assert!(
        !mcp_named_targets(&merge).contains(&"prf_cli_04_two_process_uat".to_string()),
        "the merge authority still names prf_cli_04_two_process_uat, which spawns \
         `cognicode` and `cognicode-mcp` from the release directory. That lane \
         builds no release binary since B1, so the step is red until it is skipped."
    );
    assert!(
        mcp_named_targets(&candidate).contains(&"prf_cli_04_two_process_uat".to_string()),
        "{RELEASE_CANDIDATE_AUTHORITY} does not name prf_cli_04_two_process_uat, so \
         once it leaves the merge lane nothing gates it."
    );
}

/// Total named-or-excluded coverage for the MCP crate: each `tests/*.rs`
/// must be named by some lane step or sit in the exclusion table with a
/// reason.
#[test]
fn every_mcp_suite_is_named_by_a_lane_or_excluded_with_a_reason() {
    let mut named = mcp_named_targets(&merge_authority_text());
    named.extend(mcp_named_targets(&candidate_authority_text()));
    named.sort();
    named.dedup();

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
        if named.contains(s) || excluded.contains(&s.as_str()) {
            continue;
        }
        holes.push(s);
    }
    assert!(
        holes.is_empty(),
        "{} MCP integration suites are neither named by a lane step nor \
         listed in EXCLUDED_MCP_SUITES with a reason: {holes:?}. Gate them \
         with a named step (fast/local suites), move them to the lane that \
         builds their preconditions, or document the exclusion with its \
         reason; a silent hole re-reads as coverage it does not have. \
         All {} suites: {suites:?}",
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
            !excluded.contains(&name.as_str()),
            "suite {name} is both named by a lane step and listed in \
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
    let workflow = merge_authority_text();
    assert!(
        workflow.contains("--test cli_gate_coverage_contract"),
        "cli_gate_coverage_contract is not run by the merge authority, so the \
         coverage guarantee is itself ungated."
    );
}
