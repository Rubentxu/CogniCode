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
// Shell variables inside a **raw** string (`"""..."""` + trimIndent) must be written
// `${'$'}name`; the shell then receives a literal `$` and expands its own. This is not a
// version quirk and the distinction that matters is raw vs normal string, not 0.43.0 vs
// 0.39.1-rc1: in a normal single-line string `\$name` compiles and works, while in the
// multiline raw strings every real step uses it fails with "Unresolved reference". Both
// 0.39.1-rc1 and 0.43.0 behave the same way. `\${'$'}name` is wrong everywhere: the
// backslash survives into the emitted script and bash fails with "syntax error near
// unexpected token `('".
//
// `pipelinek` on PATH resolves 0.43.0. That was not free. A symlink at
// ~/.local/bin/pipelinek pointing at a 0.39.0 install took precedence over the asdf shim
// and ran every pipeline against a compiler the repo did not pin. If validation and run
// ever disagree again, check `command -v pipelinek` first: the bug is a version
// mismatch, never the script.
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

            stage("clippy") {
                // pr-ci.yml:78 — bare `cargo clippy --workspace --all-targets -- -D warnings`.
                // The previous stage name "clippy-baseline" carried an allowlist for 42
                // pre-existing errors in rig/tools.rs that no longer exist (cargo clippy
                // reports 0 on the current tree). The baseline was dead and the pipeline
                // permitted more than CI; restored to the bare assertion CI runs.
                sh("$cd && cargo clippy --workspace --all-targets -- -D warnings")
            }

            stage("rustdoc-gate") {
                // Wired 2026-10-01. The M0.11 rustdoc gate was green and ran in no
                // pipeline at all, so a rustdoc warning that ST-01 introduced in
                // infrastructure/parser/syntax_analysis.rs survived that commit, the
                // whole branch and six more commits: nothing executed it. It surfaced
                // only because someone ran `cargo test --workspace`.
                // Pinned by core_gate_coverage_contract::the_rustdoc_gate_runs_in_both_pipelines.
                sh("$cd && cargo test -p cognicode-runtime --test m011_rustdoc_gate --quiet")
            }

            // The contract suite. One entry point, no list: the pipeline names a
            // script, the script discovers `scripts/ci/test_*.py` by glob. The
            // previous shape named eight contract files inside pr-ci.yml, which
            // meant a new contract passed locally and ran in CI only if whoever
            // wrote it also edited that YAML — how the CP5 skill gate stayed
            // green while no workflow invoked it.
            stage("contracts") {
                sh("$cd && bash scripts/ci/run-all-contracts.sh")
            }

            // CP5 / A-033..A-036. `validate_skills.py` resolves tool names from
            // the published catalog rather than a private list; `verify-skills.sh`
            // checks each bundle has a manifest. Both were correct, both passed,
            // and no workflow invoked them until 2026-10-02.
            //
            // PyYAML is declared here rather than trusted from the runner image:
            // validate_skills.py degrades to a printed SKIP without it, and
            // verify-skills.sh aborts on `import yaml`. Neither is a failure, and
            // both are how a gate goes stale in silence.
            stage("skills-toolchain") {
                sh("$cd && python3 -m pip install --disable-pip-version-check \"PyYAML==6.0.3\"")
            }

            stage("skills-validate") {
                sh("$cd && python3 scripts/validate_skills.py")
            }

            stage("skills-bundles") {
                sh("$cd && bash scripts/verify-skills.sh")
            }
        }

        // ---------------------------------------------------------- supply-chain
        // pr-ci.yml's `supply-chain` job, added in 05c66126. Advisories and
        // licences ran at tag time and never on a pull request, so a dependency
        // with a published advisory merged unnoticed.
        //
        // cargo-deny is installed with a fixed version on purpose: `--locked`
        // alone pins its dependencies while floating the tool, so the gate's
        // behaviour would change with no commit in this repository.
        // Measured 2026-10-02 on this machine: 1m43s to install from scratch.
        stage("supply-chain") {
            stage("install-cargo-deny") {
                sh(
                    """
                    ${'$'}cd || exit 1
                    if command -v cargo-deny >/dev/null 2>&1; then
                        echo "cargo-deny already present, skipping install"
                    else
                        cargo install cargo-deny --version 0.20.2 --locked
                    fi
                    """.trimIndent(),
                )
            }

            stage("advisories") {
                sh("$cd && cargo deny check advisories")
            }

            stage("licenses") {
                sh("$cd && cargo deny check licenses")
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

            // pr-ci.yml also builds the CLI itself. Without it the binary the
            // release-flow suites shell out to by path does not exist, and the
            // reachability checks that look for it have nothing to reach.
            stage("build-cognicode-cli-release") {
                sh("$cd && cargo build --release -p cognicode-cli")
            }

            stage("build-cognicode-release-bin") {
                sh("$cd && cargo build --release --bin cognicode")
            }

            // pr-ci.yml:400. A release binary that compiles and then does not
            // start is caught only here: the build stage is satisfied by an
            // artifact that exists, not by one that runs. This was one of
            // three gates the first version of the coverage inventory could
            // not see, because it matched `cargo <subcommand>` and these run no
            // cargo at all.
            stage("verify-release-binaries") {
                sh("""
                    ${'$'}cd || exit 1
                    set -euo pipefail
                    for bin in cognicode cognicode-mcp cognicode-control-plane; do
                        if [ ! -f "target/release/${'$'}bin" ]; then
                            echo "FAIL: target/release/${'$'}bin was not built"
                            exit 1
                        fi
                        if [ ! -x "target/release/${'$'}bin" ]; then
                            echo "FAIL: target/release/${'$'}bin is not executable"
                            exit 1
                        fi
                    done
                    ./target/release/cognicode --version
                    ./target/release/cognicode-control-plane --help | head -5
                    ./target/release/cognicode-mcp --version
                """.trimIndent())
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

            // pr-ci.yml also runs the selector script itself, not only its
            // contract. The contract pins what the script decides; running the
            // script is what proves the decision is consumable downstream. A
            // green contract over a script nothing calls is the A-013 shape.
            stage("select-suites-script") {
                sh("""
                    ${'$'}cd || exit 1
                    set -euo pipefail
                    PATHS="scripts/ci/run-all-contracts.sh"
                    SELECT_OUTPUT="${'$'}(SELECT_PATHS="${'$'}PATHS" bash scripts/ci/select-suites.sh)"
                    echo "${'$'}SELECT_OUTPUT"
                """.trimIndent())
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

            // pr-ci.yml:567. The unrestricted core step. Without it the five
            // suites behind #![cfg(feature = "evidence-kernel")] compile to
            // empty test binaries and report 0 passed / 0 failed / exit 0.
            // core_gate_coverage_contract pins this stage by command parity.
            stage("core-unrestricted-evidence-kernel") {
                sh("$cd && cargo test -p cognicode-core --features evidence-kernel --quiet")
            }

            // pr-ci.yml:481. A-014 product asset resolution.
            stage("core-a014-product-asset-resolution") {
                sh("$cd && cargo test -p cognicode-core --test a014_product_asset_resolution --quiet")
            }

            // ladybug (pr-ci.yml:388)
            stage("cognicode-ladybug") {
                sh("$cd && cargo test -p cognicode-ladybug --lib --quiet")
            }

            // PR-SEC reachability: every release-artifact path the pipeline
            // downloads is a path some earlier stage produced. Runs here as
            // well as in pr-ci.yml because the Kotlin pipeline is the one that
            // will own release once the migration closes, and a check that only
            // runs on the orchestrator being retired checks nothing.
            stage("bin-tracking") {
                sh("$cd && bash scripts/ci/check-bin-tracking.sh")
            }

            stage("release-artifact-reachability") {
                sh("$cd && bash scripts/ci/check-release-artifact-reachability.sh")
            }

            // CR-07. `/metrics` is the only reason the OpenTelemetry upgrade was
            // worth doing, and the upgrade was the only reason RUSTSEC-2024-0437
            // is gone from deny.toml. A package version that satisfies an
            // advisory without still exposing the endpoint is the regression
            // this pins.
            stage("cr07-metrics-exposition") {
                sh("$cd && cargo test -p cognicode-mcp --test cr07_metrics_exposition_contract --quiet")
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

            // pr-ci.yml:512. QW-09 pins that every release-artifact path the
            // workflow downloads is a path some earlier job produced.
            stage("cli-qw09-release-artifact-reachability") {
                sh("$cd && cargo test -p cognicode-cli --test qw09_release_artifact_reachability --quiet")
            }

            // pr-ci.yml:699. The A-014 twin-cycle identity collision must stay
            // visible in the action register (N+63.2).
            stage("cli-action-register-identity-contract") {
                sh("$cd && cargo test -p cognicode-cli --test action_register_identity_contract --quiet")
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
