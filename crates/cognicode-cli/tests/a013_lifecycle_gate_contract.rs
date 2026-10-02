// crates/cognicode-cli/tests/a013_lifecycle_gate_contract.rs
//
// A-013 / CP2.7 — contract for the lifecycle UAT being *in the gate*.
//
// A-013's nine black-box lifecycle tests exist and pass. That is not the
// same as them protecting anything, and the gap is the reason A-013 was
// never closed while A-024, A-026 and A-038 kept listing it as a
// prerequisite.
//
// The CR-08 selector maps `crates/cognicode-mcp/**` to the `mcp` suite,
// which the merge authority runs as `cargo test -p cognicode-mcp --lib`. Every
// black-box contract in that crate — a009, a010, a012, a013, prf_*_uat —
// lives in `tests/`, not in `src/`, so `--lib` never compiles or runs any
// of them. A regression in the read-only posture, in the tool authority
// mapping, or in shutdown behaviour passes the whole gate silently.
//
// So this file pins the coverage claim itself rather than the lifecycle:
// it asserts the merge gate names each of those contracts. It is the same
// shape as `qw08_crate_selector.rs` (assert over the orchestrator's source,
// not over a runtime), for the same reason: a CI contract that is not
// enforced can rot silently, and nothing else in the repo would notice.
//
// The orchestrator used to be `.github/workflows/pr-ci.yml`; it is now
// `merge-gate.pipeline.kts`, and `common` says which one without this file
// having to know. What each test demands has not moved: a merge cannot pass
// unless the contract runs. The `needs:`-graph parsing that the YAML version
// needed is gone with the YAML.
//
// Deliberately lexical rather than a YAML parse. The repo pins no YAML
// crate on the Rust side and the assertion is "this exact cargo command
// appears in this exact file", which a regex answers without inventing a
// dependency. A parse would be stricter about indentation and weaker
// about intent: a renamed suite would be caught here, a re-indented step
// would not.

mod common;

use common::{merge_authority_lines, merge_authority_runs, not_run_message};

/// The black-box contracts that exist in the crate but that `--lib` skips.
///
/// Each is named with the crate whose tests/ directory it lives in, so a
/// contract added to a sibling crate is not silently omitted.
const BLACKBOX_CONTRACTS: &[(&str, &str)] = &[
    ("a009_agent_safe_profile", "cognicode-mcp"),
    ("a010_tool_authority_audit", "cognicode-mcp"),
    ("a012_structured_output", "cognicode-mcp"),
    ("a013_lifecycle_uat", "cognicode-mcp"),
    ("prf_sec_02_read_only_uat", "cognicode-mcp"),
    // Same `--lib` blind spot, different crate. A-014 publishes the
    // `cognicode.capabilities/v1` document that the skill bundles and the
    // integration adapters consume, so a silent schema break there is as
    // expensive as a read-only posture break.
    ("a014_capabilities_json", "cognicode-cli"),
    ("a015_licenses_gate", "cognicode-cli"),
];

#[test]
fn a013_lifecycle_uat_is_pinned_in_the_merge_gate() {
    assert!(
        merge_authority_runs("--test a013_lifecycle_uat"),
        "The A-013 lifecycle contract exists and passes locally, but the CR-08 \
         `mcp` suite runs `cargo test -p cognicode-mcp --lib`, which never \
         compiles tests/ — so the startup -> shutdown contract is unprotected.\n{}",
        not_run_message(
            "cargo test -p cognicode-mcp --test a013_lifecycle_uat",
            "a013_lifecycle_uat must run in the merge authority"
        )
    );
}

#[test]
fn every_blackbox_mcp_contract_is_pinned() {
    let missing: Vec<&str> = BLACKBOX_CONTRACTS
        .iter()
        .filter(|(name, _)| !merge_authority_runs(&format!("--test {name}")))
        .map(|(name, _)| *name)
        .collect();
    assert!(
        missing.is_empty(),
        "these black-box contracts in crates/{}/tests/ are not run by the merge \
         authority and therefore never gate a merge: {missing:?}. \
         `cargo test -p cognicode-mcp --lib` only compiles src/; every \
         contract in tests/ is skipped by the selector's mcp suite.",
        BLACKBOX_CONTRACTS[0].1
    );
}

#[test]
fn the_mcp_suite_does_not_rely_on_lib_only_for_these_contracts() {
    // Guards against the specific regression: someone "simplifying" the
    // pinned step back to `--lib` because the individual `--test` lines
    // below it look redundant. A-013 is not covered by --lib, so the
    // simplification is a silent loss of coverage.
    let lines = merge_authority_lines();
    let lib_only_mcp = lines
        .iter()
        .filter(|l| l.contains("cargo test -p cognicode-mcp --lib"))
        .count();
    let pinned_mcp = lines
        .iter()
        .filter(|l| l.contains("cargo test -p cognicode-mcp --test"))
        .count();
    assert!(
        pinned_mcp > 0,
        "the merge authority runs no `cargo test -p cognicode-mcp --test <name>` \
         at all, so the whole black-box surface of the MCP contract is uncovered."
    );
    assert!(
        lib_only_mcp + pinned_mcp > 0 && pinned_mcp > 0,
        "expected both --lib ({lib_only_mcp}) and pinned --test ({pinned_mcp}) \
         invocations for cognicode-mcp"
    );
}

#[test]
fn a014_capabilities_contract_is_pinned() {
    // A-014 is the discovery contract other units consume: the skill bundles
    // (A-033..037) and any integration adapter read
    // `cognicode capabilities --format json`. Its tests live in `tests/`, so
    // the CLI `mcp`/`cli` suites run with `--lib` and never touch them.
    assert!(
        merge_authority_runs("--test a014_capabilities_json"),
        "The `cognicode.capabilities/v1` schema is the discovery contract for \
         skills and integration adapters, and nothing in the merge gate would \
         notice it breaking.\n{}",
        not_run_message(
            "cargo test -p cognicode-cli --test a014_capabilities_json",
            "a014_capabilities_json must run in the merge authority"
        )
    );
    assert!(
        merge_authority_runs("--test a015_licenses_gate"),
        "The published license allow-list is not checked against crate metadata \
         by the gate.\n{}",
        not_run_message(
            "cargo test -p cognicode-cli --test a015_licenses_gate",
            "a015_licenses_gate must run in the merge authority"
        )
    );
}
