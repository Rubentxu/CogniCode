//! W0 — Post-pruning orthodoxy (regression for the poda radical v0).
//!
//! `chore(retire): drop 19 test binaries per poda radical v0 (W0.1)` —
//! commit `013a11d1` — retired 14 presenter snapshots in `cognicode-cli`,
//! the two gate-coverage contracts, `e91_w8_stage_profile`,
//! `b6_w2_perf_budgets`, `identity_benchmark`, and
//! `clippy_gate_parity_contract`. `chore(ci): retire gate contracts
//! removed 2026-10-18 (W0.1 unblock)` — commit `c8a96e64` — removed
//! the corresponding `merge-gate.pipeline.kts` and
//! `product-fast.pipeline.kts` stages so the gate does not invoke
//! a path that no longer exists.
//!
//! The poda radical recipe is explicit: a future change that
//! re-adds a reference to a retired path in an ACTIVE scope
//! (pipeline kts, CI test scripts, Cargo.toml `[[test]]` entries)
//! defeats the cleanup. A future change that drops a protected
//! path defeats the manifest's KEEP set.
//!
//! Historical scopes (`JOURNAL.md`, `MAINTENANCE.md`, `ROADMAP.md`,
//! `docs/prf/*`, `openspec/*`) are excluded by design: they
//! record the cleanup, and a test that forbids mentions of the
//! retired names would be wrong the day the next journal entry
//! is written.
//!
//! The test scans ONLY the active pipeline kts. A `cargo test`
//! invocation whose argument includes a retired basename is the
//! failure mode the cleanup was designed to prevent; comment-only
//! references like `// (was core_gate_coverage_contract::...; contract
//! retired 2026-10-18)` are the right shape and are skipped.

use std::path::{Path, PathBuf};

const RETIRED: &[&str] = &[
    "crates/cognicode-cli/tests/t1_5_3d_index_presenter_snapshot.rs",
    "crates/cognicode-cli/tests/t1_5_4c_graph_perfile_presenter_snapshot.rs",
    "crates/cognicode-cli/tests/t1_5_4d_graph_entrypoints_leaffunctions_snapshot.rs",
    "crates/cognicode-cli/tests/t1_5_4e_graph_hotpaths_snapshot.rs",
    "crates/cognicode-cli/tests/t1_5_4f_graph_complexity_snapshot.rs",
    "crates/cognicode-cli/tests/t1_5_4g_graph_tracepath_snapshot.rs",
    "crates/cognicode-cli/tests/t1_5_4h_graph_mermaid_snapshot.rs",
    "crates/cognicode-cli/tests/t1_5_4i_graph_ondemand_snapshot.rs",
    "crates/cognicode-cli/tests/t1_5_4j_graph_hierarchy_snapshot.rs",
    "crates/cognicode-cli/tests/t1_5_4k_graph_full_snapshot.rs",
    "crates/cognicode-cli/tests/t1_5_4l_graph_impact_snapshot.rs",
    "crates/cognicode-cli/tests/t1_5_6a_navigate_presenter_snapshot.rs",
    "crates/cognicode-cli/tests/t1_5_snapshot.rs",
    "crates/cognicode-cli/tests/cli_gate_coverage_contract.rs",
    "crates/cognicode-core/tests/core_gate_coverage_contract.rs",
    "crates/cognicode-core/tests/clippy_gate_parity_contract.rs",
    "crates/cognicode-core/tests/b6_w2_perf_budgets.rs",
    "crates/cognicode-core/tests/e91_w8_stage_profile.rs",
    "crates/cognicode-core/tests/identity_benchmark.rs",
];

const PROTECTED: &[&str] = &[
    "crates/cognicode-core/tests/workspace_isolation.rs",
    "crates/cognicode-core/tests/prf_sec_07_adversarial_campaign.rs",
    "crates/cognicode-core/tests/cu2_w2_finding_projection.rs",
    "crates/cognicode-core/tests/cu2_w3_guard_transfer.rs",
    "crates/cognicode-core/tests/cu2_w4_live_consumer.rs",
    "crates/cognicode-cli/tests/cli_exit_code_propagation.rs",
    "crates/cognicode-mcp/tests/a009_agent_safe_profile.rs",
    "crates/cognicode-mcp/tests/a010_tool_authority_audit.rs",
    "crates/cognicode-mcp/tests/prf_h06_adversarial_e2e.rs",
];

const ACTIVE_SCOPES: &[&str] = &[
    "merge-gate.pipeline.kts",
    "product-fast.pipeline.kts",
    "integration.pipeline.kts",
    "release-candidate.pipeline.kts",
    "release.pipeline.kts",
    "certification.pipeline.kts",
];

/// Workspace root from a test crate manifest. cargo runs integration
/// tests with cwd = the package root, so we walk two `..` to reach the
/// workspace root and resolve every relative path from there.
fn workspace() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn abs(rel: &str) -> PathBuf {
    workspace().join(rel)
}

// ===========================================================================
// Test 1 — the 19 retired test paths are gone from disk
// ===========================================================================

/// The poda radical v0 manifest retired these 19 test binaries. A
/// reappearance of any of them on disk (after a merge, after a
/// cherry-pick, after a manual restore) defeats the cleanup. The
/// test walks the listed paths; a present path is a hard failure
/// with the path printed so a contributor can decide what to do:
/// either remove the file again, or — if the reintroduction is
/// intentional and the manifest must change — update both the
/// manifest and this test in the same diff.
#[test]
fn retired_test_paths_are_not_on_disk() {
    for rel in RETIRED {
        let path = abs(rel);
        assert!(
            !path.exists(),
            "poda radical v0 retired {rel} but the file exists at {}; \
             either remove the file again, or — if the reintroduction \
             is intentional — update the manifest and this test together",
            path.display()
        );
    }
}

// ===========================================================================
// Test 2 — the 9 protected test paths are still on disk
// ===========================================================================

/// The manifest's KEEP set is the spine of the L0 cleanup. These
/// 9 paths pin the security boundary (workspace_isolation,
/// prf_sec_07, prf_h06), the consumer contract (cu2_w2/w3/w4), the
/// agent-safe profile (a009, a010), and the exit code contract
/// (cli_exit_code_propagation). A disappearance of any of them is
/// a hard failure: the protection is gone, and the test that
/// detected the disappearance is also gone (it could not run).
#[test]
fn protected_test_paths_are_still_on_disk() {
    for rel in PROTECTED {
        let path = abs(rel);
        assert!(
            path.exists(),
            "manifest KEEP set path missing: {rel} (expected at {})",
            path.display()
        );
    }
}

// ===========================================================================
// Test 3 — no active pipeline invokes a retired test
// ===========================================================================

/// Active scopes (PipelineK kts) MUST NOT contain a `cargo test`
/// invocation whose argument references a retired test basename.
/// Comment-only references like
/// `// (was core_gate_coverage_contract::...; contract retired ...)`
/// are the right shape and are skipped. A line that contains
/// `cargo test` AND a retired basename is the failure mode the
/// c8a96e64 commit closed; the test pins that closure.
#[test]
fn no_active_pipeline_invokes_a_retired_test() {
    for scope in ACTIVE_SCOPES {
        let path = abs(scope);
        if !path.exists() {
            // A scope file can be added or removed by an unrelated
            // change; the test skips silently so the suite does
            // not become brittle to optional pipeline files.
            continue;
        }
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        for (line_no, line) in text.lines().enumerate() {
            let trimmed = line.trim_start();
            // Comment line: skip. This includes `//`, `// ...`,
            // and the block-comment opener `/*` followed by text on
            // the same line. We do not parse nested block comments;
            // the pipeline kts in this repo do not nest.
            if trimmed.starts_with("//")
                || trimmed.starts_with("/*")
                || trimmed.starts_with("*")
            {
                continue;
            }
            // The failure mode is `cargo test ... <basename>`. A
            // line that does not call cargo test cannot invoke a
            // retired test by definition.
            if !line.contains("cargo test") {
                continue;
            }
            for retired in RETIRED {
                let basename = Path::new(retired)
                    .file_name()
                    .and_then(|s| s.to_str())
                    .and_then(|s| s.strip_suffix(".rs"))
                    .unwrap_or(retired);
                if line.contains(basename) {
                    panic!(
                        "{scope}:{}: active invocation of retired test \
                         `{basename}` (full path: {retired}). A `cargo test` \
                         line in an active scope must not reference a \
                         retired test; remove the invocation, or — if \
                         the reintroduction is intentional — update the \
                         manifest and this test in the same diff.\n\
                         offending line: {line}",
                        line_no + 1
                    );
                }
            }
        }
    }
}
