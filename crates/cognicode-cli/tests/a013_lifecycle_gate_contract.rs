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
// which `pr-ci.yml` runs as `cargo test -p cognicode-mcp --lib`. Every
// black-box contract in that crate — a009, a010, a012, a013, prf_*_uat —
// lives in `tests/`, not in `src/`, so `--lib` never compiles or runs any
// of them. A regression in the read-only posture, in the tool authority
// mapping, or in shutdown behaviour passes the whole gate silently.
//
// So this file pins the coverage claim itself rather than the lifecycle:
// it asserts the merge gate names each of those contracts. It is the same
// shape as `qw08_crate_selector.rs` (assert over the workflow YAML, not
// over a runtime), for the same reason: a CI contract that is not
// enforced can rot silently, and nothing else in the repo would notice.
//
// Deliberately lexical rather than a YAML parse. The repo pins no YAML
// crate on the Rust side and the assertion is "this exact cargo command
// appears in this exact file", which a regex answers without inventing a
// dependency. A parse would be stricter about indentation and weaker
// about intent: a renamed suite would be caught here, a re-indented step
// would not.

use std::fs;
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    // crates/cognicode-cli -> crates -> workspace root
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop();
    p.pop();
    p
}

fn read_workflow() -> String {
    let path = workspace_root()
        .join(".github")
        .join("workflows")
        .join("pr-ci.yml");
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

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
    let workflow = read_workflow();
    assert!(
        workflow.contains("--test a013_lifecycle_uat"),
        "a013_lifecycle_uat is not named anywhere in pr-ci.yml. The A-013 \
         lifecycle contract exists and passes locally, but the CR-08 `mcp` \
         suite runs `cargo test -p cognicode-mcp --lib`, which never \
         compiles tests/ — so the startup -> shutdown contract is unprotected."
    );
}

#[test]
fn every_blackbox_mcp_contract_is_pinned() {
    let workflow = read_workflow();
    let missing: Vec<&str> = BLACKBOX_CONTRACTS
        .iter()
        .filter(|(name, _)| !workflow.contains(&format!("--test {name}")))
        .map(|(name, _)| *name)
        .collect();
    assert!(
        missing.is_empty(),
        "these black-box contracts in crates/{}/tests/ are not named in \
         pr-ci.yml and therefore never run in the merge gate: {missing:?}. \
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
    let workflow = read_workflow();
    let lib_only_mcp = workflow
        .lines()
        .filter(|l| l.contains("cargo test -p cognicode-mcp --lib"))
        .count();
    let pinned_mcp = workflow
        .lines()
        .filter(|l| l.contains("cargo test -p cognicode-mcp --test"))
        .count();
    assert!(
        pinned_mcp > 0,
        "pr-ci.yml runs no `cargo test -p cognicode-mcp --test <name>` at all, \
         so the whole black-box surface of the MCP contract is uncovered."
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
    let workflow = read_workflow();
    assert!(
        workflow.contains("--test a014_capabilities_json"),
        "a014_capabilities_json is not named in pr-ci.yml. The \
         `cognicode.capabilities/v1` schema is the discovery contract for \
         skills and integration adapters, and nothing in the merge gate would \
         notice it breaking."
    );
    assert!(
        workflow.contains("--test a015_licenses_gate"),
        "a015_licenses_gate is not named in pr-ci.yml, so the published \
         license allow-list is not checked against crate metadata by the gate."
    );
}
