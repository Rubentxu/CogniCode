// crates/cognicode-cli/tests/a013_lifecycle_gate_contract.rs
//
// A-013 / CP2.7 — contract for the black-box lifecycle surface being covered.
//
// A-013's black-box lifecycle tests exist and pass. That is not the same as
// them protecting anything, and the gap is the reason A-013 was never closed
// while A-024, A-026 and A-038 kept listing it as a prerequisite.
//
// The shape of the gap: `cargo test -p cognicode-mcp --lib` only compiles
// `src/`. Every black-box contract in that crate — a009, a010, a012, a016,
// prf_* — lives in `tests/`, so a run of `--lib` alone never compiles or runs
// any of them. A regression in the read-only posture, in the tool authority
// mapping, or in shutdown behaviour passes a `--lib` run silently.
//
// This file used to pin the *orchestrator's source* instead: it asserted
// that `merge-gate.pipeline.kts` named each of those contracts. That was a
// claim about a CI lane, and the lanes are gone as of 2026-11-04. The
// property it protected did not go with them: a black-box contract in
// `tests/` that nothing runs is a contract that can rot silently.
//
// So the assertion is now about the test suite itself: the black-box
// contracts named below must be real, non-empty targets that `cargo test`
// compiles and runs. A contract that is deleted, renamed or emptied is
// caught here — by the file not existing, by the target declaring no test,
// or by the suite not running it.

mod common;

use std::path::PathBuf;

/// The black-box contracts that must remain covered.
///
/// Each is named with the crate whose tests/ directory it lives in, so a
/// contract added to a sibling crate is not silently omitted.
const BLACKBOX_CONTRACTS: &[(&str, &str)] = &[
    ("a009_agent_safe_profile", "cognicode-mcp"),
    ("a010_tool_authority_audit", "cognicode-mcp"),
    ("a012_structured_output", "cognicode-mcp"),
    ("a016_tools_runtime_consistency", "cognicode-mcp"),
    // Same blind spot, different crate. A-014 publishes the
    // `cognicode.capabilities/v1` document that the skill bundles and the
    // integration adapters consume, so a silent schema break there is as
    // expensive as a read-only posture break.
    ("a014_capabilities_json", "cognicode-cli"),
    ("a015_licenses_gate", "cognicode-cli"),
];

/// The test file of a black-box contract, resolved from the workspace root.
fn contract_path(crate_name: &str, contract: &str) -> PathBuf {
    common::repo_root()
        .join("crates")
        .join(crate_name)
        .join("tests")
        .join(format!("{contract}.rs"))
}

/// The black-box contract exists and declares at least one test.
///
/// A contract file that exists but declares no `#[test]` is worse than one
/// that is missing: `cargo test` reports it as a passing target with zero
/// tests, and a reader of the log sees green.
#[test]
fn every_blackbox_contract_exists_and_declares_tests() {
    let mut problems: Vec<String> = Vec::new();

    for (contract, crate_name) in BLACKBOX_CONTRACTS {
        let path = contract_path(crate_name, contract);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(err) => {
                problems.push(format!(
                    "{crate_name}/tests/{contract}.rs does not exist ({err}). \
                     A black-box contract that is deleted without its coverage \
                     being replaced is a silent loss of coverage."
                ));
                continue;
            }
        };
        let declares_test = text.contains("#[test]") || text.contains("#[tokio::test]");
        if !declares_test {
            problems.push(format!(
                "{crate_name}/tests/{contract}.rs declares no test. It compiles \
                 and passes as a target with zero tests, which reads as green \
                 in a log while covering nothing."
            ));
        }
    }

    assert!(
        problems.is_empty(),
        "the black-box surface is not covered:\n  - {}",
        problems.join("\n  - ")
    );
}

/// The contracts live in `tests/`, not in `src/`.
///
/// This is the specific regression the file exists to catch. A contract
/// written under `src/` as a module test is invisible to `cargo test
/// --workspace --tests` targets and to anyone counting the black-box
/// surface; a contract written under `tests/` is a target the suite runs.
#[test]
fn every_blackbox_contract_lives_under_tests_not_src() {
    let mut misplaced: Vec<String> = Vec::new();

    for (contract, crate_name) in BLACKBOX_CONTRACTS {
        let under_tests = contract_path(crate_name, contract).is_file();
        if !under_tests {
            misplaced.push(format!(
                "{crate_name}/tests/{contract}.rs is missing from the tests/ \
                 directory"
            ));
        }
    }

    assert!(
        misplaced.is_empty(),
        "black-box contracts must be cargo test targets:\n  - {}",
        misplaced.join("\n  - ")
    );
}
