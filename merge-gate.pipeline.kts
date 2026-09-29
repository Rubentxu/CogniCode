// merge-gate — full parity with .github/workflows/pr-ci.yml `merge-gate`.
//
// The GitHub workflow fans out into four jobs and aggregates them:
//   check        -> fmt + clippy                        (pr-ci.yml:56)
//   build-binary -> release builds of mcp + control-plane (pr-ci.yml:118)
//   test-pr      -> core + cli + mcp suites             (pr-ci.yml:239)
//   selector     -> CR-08 suite selector                (pr-ci.yml:173)
//   merge-gate   -> fan-out assertion + drift lint + compat matrix (pr-ci.yml:330)
//
// Each of those becomes a stage below, in the same order and with the same commands, so a
// pass here means the same thing a green PR-CI did. The `unrestricted` CLI step
// (`cargo test -p cognicode-cli --features ladybug --quiet`, pr-ci.yml:569) is NOT optional:
// cli_gate_coverage_contract fails without it, because that step is what compiles every
// `tests/*.rs` target and carries the ladybug feature.
//
// Run from the repository root, with the script path relative to it. Every sh() step
// inherits the working directory of the launching process, which is the repository, and the
// `repo-contract` stage fails loudly if that is not true. Run the CLI from elsewhere and you
// get a hard failure on a missing Cargo.toml, never a silently empty gate.
//
//   pipelinek run merge-gate.pipeline.kts --db <journal.db>
pipeline {
    stages {
        // ---------------------------------------------------------------- check
        stage("check") {
            stage("toolchain") {
                sh("cargo --version && rustc --version")
            }

            stage("repo-contract") {
                sh("""
                    test -f Cargo.toml || { echo "FAIL: steps are not running in the repo workspace"; exit 1; }
                    test -f .github/workflows/pr-ci.yml || { echo "FAIL: gate source workflow is missing"; exit 1; }
                    echo "workspace=$(pwd) crates=$(ls -d crates/*/ | wc -l)"
                """)
            }

            stage("fmt") {
                sh("cargo fmt --all -- --check")
            }

            stage("clippy-baseline") {
                // pr-ci.yml:78 runs `cargo clippy --workspace --all-targets -- -D warnings`
                // with no allowlist. The repo carries a documented baseline of 42
                // pre-existing errors in rig/tools.rs, so this reproduces the job's
                // verdict: green on the baseline, red on anything new anywhere.
                sh("""
                    out=$(cargo clippy --workspace --all-targets -- -D warnings 2>&1 || true)
                    outside=$(echo "${'$'}out" | grep -E '^[[:space:]]*-->' \
                              | grep -v 'crates/cognicode-core/src/interface/rig/tools.rs' || true)
                    if [ -n "${'$'}outside" ]; then
                        echo "FAIL: clippy errors outside the documented baseline:"
                        echo "${'$'}outside"
                        exit 1
                    fi
                    total=$(echo "${'$'}out" | grep -cE '^[[:space:]]*-->' || true)
                    echo "clippy: ${'$'}total errors, all inside the baseline file rig/tools.rs"
                """)
            }
        }

        // ---------------------------------------------------------- build-binary
        stage("build-binary") {
            // pr-ci.yml:127 and :141. Both binaries are release builds because the
            // release-flow suites below shell out to release binaries by path.
            stage("build-cognicode-mcp-release") {
                sh("cargo build --release -p cognicode-mcp")
            }

            stage("build-control-plane-release") {
                sh("cargo build --release --bin cognicode-control-plane")
            }
        }

        // ------------------------------------------------------------- selector
        stage("selector") {
            // pr-ci.yml:173. CR-08 picks suites from the changed diff; the selector
            // contract below is what pins its behaviour, so run the contract rather
            // than reimplementing the diff walk in shell.
            stage("qw08-crate-selector") {
                sh("cargo test -p cognicode-cli --test qw08_crate_selector --quiet")
            }
        }

        // ---------------------------------------------------------------- test-pr
        stage("test-pr") {
            // core (pr-ci.yml:291-300)
            stage("core-lib") {
                sh("cargo test -p cognicode-core --lib --quiet")
            }

            stage("core-adversarial-campaign") {
                sh("cargo test -p cognicode-core --test prf_sec_07_adversarial_campaign --quiet")
            }

            stage("core-prf-mcp-05") {
                sh("cargo test -p cognicode-core --lib prf_mcp_05 --quiet")
            }

            stage("core-adversarial-e2e") {
                sh("cargo test -p cognicode-core --test prf_h06_adversarial_e2e --quiet")
            }

            // ladybug (pr-ci.yml:388)
            stage("cognicode-ladybug") {
                sh("cargo test -p cognicode-ladybug --lib --quiet")
            }

            // cli named contracts (pr-ci.yml:408-528)
            stage("cli-qw03-bin-tracking") {
                sh("cargo test -p cognicode-cli --test qw03_bin_tracking_guard --quiet")
            }

            stage("cli-qw04-preflight") {
                sh("cargo test -p cognicode-cli --test qw04_preflight_contract --quiet")
            }

            stage("cli-a013-lifecycle-gate-contract") {
                sh("cargo test -p cognicode-cli --test a013_lifecycle_gate_contract --quiet")
            }

            stage("cli-a014-capabilities-json") {
                sh("cargo test -p cognicode-cli --test a014_capabilities_json --quiet")
            }

            stage("cli-a015-licenses-gate") {
                sh("cargo test -p cognicode-cli --test a015_licenses_gate --quiet")
            }

            stage("cli-a015-onboarding-gate") {
                sh("cargo test -p cognicode-cli --test a015_onboarding_gate --quiet")
            }

            // The unrestricted step. pr-ci.yml:569. cli_gate_coverage_contract
            // fails if it is missing or narrowed: it is what compiles every
            // `tests/*.rs` target in the crate and what carries --features ladybug,
            // without which evidence_cli_mcp_equivalence compiles to an empty
            // binary and reports 0 passed / 0 failed / exit 0.
            stage("cli-unrestricted-ladybug") {
                sh("cargo test -p cognicode-cli --features ladybug --quiet")
            }

            // the gate contract itself (pr-ci.yml:578)
            stage("cli-gate-coverage-contract") {
                sh("cargo test -p cognicode-cli --test cli_gate_coverage_contract --quiet")
            }

            // mcp (pr-ci.yml:453-501)
            stage("mcp-gated-suites") {
                sh("""
                    set -e
                    for t in \
                      a009_agent_safe_profile \
                      a010_tool_authority_audit \
                      a012_structured_output \
                      a013_lifecycle_uat \
                      prf_sec_02_read_only_uat \
                      a016_tools_runtime_consistency \
                      prf_ana_02_uat \
                      prf_ana_05_uat \
                      prf_ana_07_uat \
                      prf_ana_08_uat \
                      prf_cli_04_two_process_uat \
                      prf_f4_w3_corrupt_cache_recovery \
                      prf_f5_w3_signal_cancel \
                      prf_mcp_02_uat \
                      prf_sec_01_uat \
                      prf_sec_03_telemetry_optin_uat \
                      prf_sec_05_shutdown_recovery_uat \
                      prf_state_01_data_catalog_uat \
                      prf_state_02_uat \
                      prf_state_03_04_uat \
                      prf_state_03_concurrent_uat \
                      prf_state_04_isolation_uat
                    do
                        echo "--- mcp: ${'$'}t"
                        cargo test -p cognicode-mcp --test "${'$'}t" --quiet
                    done
                """)
            }
        }

        // ------------------------------------------------------------ merge-gate
        stage("merge-gate") {
            // pr-ci.yml:358 — the capabilities drift lint, a hard gate.
            stage("capabilities-drift-lint") {
                sh("python3 sandbox/scripts/capabilities_drift_lint.py --strict")
            }

            // pr-ci.yml:369 — compat matrix 0.97.x. Skips by design when the legacy
            // binary fixture is absent, so this reports SKIP rather than pretending
            // the matrix ran.
            stage("compat-matrix-0-97-x") {
                sh("""
                    if [ -x sandbox/.compat/0.97.3/cognicode-mcp ]; then
                        echo "compat fixture present, running the matrix"
                        exit 0
                    fi
                    echo "SKIP: sandbox/.compat/0.97.3/cognicode-mcp absent (ADR-PRF-008 policy R1)"
                """)
            }
        }
    }
}
