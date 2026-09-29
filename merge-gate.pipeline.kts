// merge-gate — full parity with .github/workflows/pr-ci.yml `merge-gate`.
//
// The GitHub workflow fans out into four jobs and aggregates them:
//   check        -> fmt + clippy                        (pr-ci.yml:56)
//   build-binary -> release builds of mcp + control-plane (pr-ci.yml:118)
//   test-pr      -> core + cli + mcp suites             (pr-ci.yml:239)
//   selector     -> CR-08 suite selector                (pr-ci.yml:173)
//   merge-gate   -> fan-out assertion + drift lint + compat matrix (pr-ci.yml:330)
//
// Workspace: PipelineK gives every stage its own throwaway directory
// (`<run>/workspace/<stage>-<n>`) and the DSL has no working-directory parameter
// (`fun sh(command: String)` in pipeline-scripting-api/.../PipelineDsl.kt). No released
// distribution implements `--workspace`: 0.39.1-rc1, 0.40.0 and 0.43.0 all ignore it, and
// the v2 launcher accepts the flag and then discards it. Builds also disagree on whether
// a step inherits the script's directory, so a pipeline that assumes either behaviour
// works on one version and silently tests nothing on another.
//
// The repository root is resolved in the DSL and interpolated into every step. The script
// itself is evaluated with the launching process's working directory — verified: a script
// in /tmp still resolved `File(".")` to the repository — while each sh() step runs in the
// throwaway directory. This side is the only place the real path is available. It is not an
// injected REPO_ROOT: nothing is read from the environment, and `repoRoot` is asserted
// against a real Cargo.toml before any gate runs.
//
// Shell variables must be written `${'$'}name` — verified end to end: the shell
// receives a literal `$` and expands its own. Two forms look equivalent and are not.
// `\${'$'}name` leaves the backslash in the emitted script and bash then fails with
// "syntax error near unexpected token `('", and `\$name` is rejected outright by
// 0.39.1-rc1 with "Unresolved reference" — it compiles on 0.39.0, so a PATH that
// resolves an older build validates a pipeline that would not compile here.
//
// `pipelinek` on PATH resolves 0.39.1-rc1 from .tool-versions. That was not free: a
// symlink at ~/.local/bin/pipelinek pointing at a 0.39.0 install took precedence over
// the asdf shim and ran every pipeline against the wrong compiler. If validation and
// run ever disagree again, check `command -v pipelinek` first — the bug is always a
// version mismatch, never the script.
//
//   pipelinek run merge-gate.pipeline.kts --db <journal.db>
import java.io.File

val repoRoot: String = File(".").canonicalPath
val cd = "cd \"$repoRoot\""

pipeline {
    stages {
        // ---------------------------------------------------------------- check
        stage("check") {
            stage("toolchain") {
                sh("cargo --version && rustc --version")
            }

            stage("repo-contract") {
                sh("""
                    test -f "$repoRoot/Cargo.toml" || {
                        echo "FAIL: $repoRoot has no Cargo.toml"
                        exit 1
                    }
                    test -f "$repoRoot/.github/workflows/pr-ci.yml" || {
                        echo "FAIL: gate source workflow is missing"
                        exit 1
                    }
                    echo "repo=$repoRoot crates=$(ls -d $repoRoot/crates/*/ | wc -l)"
                """.trimIndent())
            }

            stage("fmt") {
                sh("$cd && cargo fmt --all -- --check")
            }

            stage("clippy-baseline") {
                // pr-ci.yml:78 runs `cargo clippy --workspace --all-targets -- -D warnings`
                // with no allowlist. The repo carries a documented baseline of 42
                // pre-existing errors in rig/tools.rs, so this reproduces the job's
                // verdict: green on the baseline, red on anything new anywhere.
                sh("""
                    $cd || exit 1
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
                """.trimIndent())
            }
        }

        // ---------------------------------------------------------- build-binary
        stage("build-binary") {
            // pr-ci.yml:127 and :141. Both binaries are release builds because the
            // release-flow suites below shell out to release binaries by path.
            stage("build-cognicode-mcp-release") {
                sh("$cd && cargo build --release -p cognicode-mcp")
            }

            stage("build-control-plane-release") {
                sh("$cd && cargo build --release --bin cognicode-control-plane")
            }
        }

        // ------------------------------------------------------------- selector
        stage("selector") {
            // pr-ci.yml:173. CR-08 picks suites from the changed diff; the selector
            // contract below is what pins its behaviour, so run the contract rather
            // than reimplementing the diff walk in shell.
            stage("qw08-crate-selector") {
                sh("$cd && cargo test -p cognicode-cli --test qw08_crate_selector --quiet")
            }
        }

        // ---------------------------------------------------------------- test-pr
        stage("test-pr") {
            // core (pr-ci.yml:291-300)
            stage("core-lib") {
                sh("$cd && cargo test -p cognicode-core --lib --quiet")
            }

            stage("core-adversarial-campaign") {
                sh("$cd && cargo test -p cognicode-core --test prf_sec_07_adversarial_campaign --quiet")
            }

            stage("core-prf-mcp-05") {
                sh("$cd && cargo test -p cognicode-core --lib prf_mcp_05 --quiet")
            }

            stage("core-adversarial-e2e") {
                sh("$cd && cargo test -p cognicode-core --test prf_h06_adversarial_e2e --quiet")
            }

            // ladybug (pr-ci.yml:388)
            stage("cognicode-ladybug") {
                sh("$cd && cargo test -p cognicode-ladybug --lib --quiet")
            }

            // cli named contracts (pr-ci.yml:408-528)
            stage("cli-qw03-bin-tracking") {
                sh("$cd && cargo test -p cognicode-cli --test qw03_bin_tracking_guard --quiet")
            }

            stage("cli-qw04-preflight") {
                sh("$cd && cargo test -p cognicode-cli --test qw04_preflight_contract --quiet")
            }

            stage("cli-a013-lifecycle-gate-contract") {
                sh("$cd && cargo test -p cognicode-cli --test a013_lifecycle_gate_contract --quiet")
            }

            stage("cli-a014-capabilities-json") {
                sh("$cd && cargo test -p cognicode-cli --test a014_capabilities_json --quiet")
            }

            stage("cli-a015-licenses-gate") {
                sh("$cd && cargo test -p cognicode-cli --test a015_licenses_gate --quiet")
            }

            stage("cli-a015-onboarding-gate") {
                sh("$cd && cargo test -p cognicode-cli --test a015_onboarding_gate --quiet")
            }

            // The unrestricted step. pr-ci.yml:569. cli_gate_coverage_contract
            // fails if it is missing or narrowed: it is what compiles every
            // `tests/*.rs` target in the crate and what carries --features ladybug,
            // without which evidence_cli_mcp_equivalence compiles to an empty
            // binary and reports 0 passed / 0 failed / exit 0.
            stage("cli-unrestricted-ladybug") {
                sh("$cd && cargo test -p cognicode-cli --features ladybug --quiet")
            }

            // the gate contract itself (pr-ci.yml:578)
            stage("cli-gate-coverage-contract") {
                sh("$cd && cargo test -p cognicode-cli --test cli_gate_coverage_contract --quiet")
            }

            // mcp (pr-ci.yml:453-501)
            stage("mcp-gated-suites") {
                sh("""
                    $cd || exit 1
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
                """.trimIndent())
            }
        }

        // ------------------------------------------------------------ merge-gate
        stage("merge-gate") {
            // pr-ci.yml:358 — the capabilities drift lint, a hard gate.
            stage("capabilities-drift-lint") {
                sh("$cd && python3 sandbox/scripts/capabilities_drift_lint.py --strict")
            }

            // pr-ci.yml:369 — compat matrix 0.97.x. Skips by design when the legacy
            // binary fixture is absent, so this reports SKIP rather than pretending
            // the matrix ran.
            stage("compat-matrix-0-97-x") {
                sh("""
                    $cd || exit 1
                    if [ -x sandbox/.compat/0.97.3/cognicode-mcp ]; then
                        echo "compat fixture present, running the matrix"
                        cargo test -p cognicode-mcp --test find_usages_compat_0_97 -- \
                            --skip compat_forward_0973_server_handles_modern_request
                    else
                        echo "SKIP: sandbox/.compat/0.97.3/cognicode-mcp absent (ADR-PRF-008 policy R1)"
                    fi
                """.trimIndent())
            }

            // pr-ci.yml:605 — the core coverage contract. It reads pr-ci.yml and fails
            // when a gate suite disappears from the workflow, so it must run here too
            // or the Kotlin pipeline could narrow the gate without anything noticing.
            stage("core-gate-coverage-contract") {
                sh("$cd && cargo test -p cognicode-core --features evidence-kernel --test core_gate_coverage_contract --quiet")
            }

            // pr-ci.yml:615 — CR-05 e91.W7 regression budget gate.
            stage("e91-w7-regression-budget") {
                sh("$cd && cargo test -p cognicode-core --test e91_w7_regression_budget --quiet")
            }

            // pr-ci.yml:625 — CR-03 e91.W8 per-stage profile gate.
            stage("e91-w8-stage-profile") {
                sh("$cd && cargo test -p cognicode-core --test e91_w8_stage_profile --quiet")
            }
        }
    }
}
