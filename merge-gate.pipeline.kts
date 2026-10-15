// merge-gate — the pipeline a merge is gated on.
//
// This was built to full parity with `.github/workflows/pr-ci.yml` and then
// took its place. The line numbers in the comments below are left where they
// were: they are provenance, pointing at the stage each gate came from, and
// the question they answered — "does the required check run this?" — is
// answered here now. The orchestrator that decides what runs is this file,
// declared once in `scripts/ci/pipeline_authority.py` and read from there by
// every wiring contract.
//
// The four GitHub jobs it replaces, for the record:
//   check        -> fmt + clippy                        (pr-ci.yml:56)
//   build-binary -> release builds of mcp + control-plane (pr-ci.yml:118)
//   test-pr      -> core + cli + mcp suites             (pr-ci.yml:239)
//   selector     -> CR-08 suite selector                (pr-ci.yml:173)
//   merge-gate   -> fan-out assertion + drift lint + compat matrix (pr-ci.yml:330)
//
// Workspace: the repository root is resolved here and interpolated into every
// step, because PipelineK runs each `sh()` step in the launching process's
// working directory and a step must not depend on where the launcher stood.
//
// An earlier revision of this header claimed the opposite — that PipelineK
// gives every stage its own throwaway directory, so no step could see another's
// output, and that is why the root is interpolated. That claim was false and
// was measured out of it on 2026-10-02 with a two-stage probe run under
// pipelinek 0.46.0 and no `cd` prefix anywhere:
//
//     WRITER_PWD  = <repo root>
//     READER_PWD  = <repo root>
//     RELATIVE_SHARED = yes
//     REPO_VISIBLE    = yes
//
// Stages share the filesystem and the working directory. A file one stage
// writes at a relative path is visible to the next, and a release binary built
// in one stage can be executed by another.
//
// The consequence is not cosmetic. QW-09 exists because in GitHub Actions
// `needs:` means "wait for that job", not "inherit that job's artifacts" — every
// job gets a fresh runner. That failure mode does not exist here, so the
// reachability property is not "each stage must obtain its own binaries" but
// "a step must not consume a path that a later step produces". Ordering is the
// whole invariant. It is pinned by
// `scripts/ci/test_pipeline_artifact_reachability.py` over these scripts rather
// than over YAML, and deliberately so: pinning it over `pr-ci.yml` would have
// made the retiring orchestrator the authority for a property of this one.
//
// The same probe found the other half: a stage can write into the repository
// itself. Nothing here cleans up after itself, so a step that writes a
// relative path writes into the working tree.
//
// And the second thing that probe measures is the one this file silently
// depends on everywhere: **a failing stage aborts the pipeline.** Measured on
// 2026-10-02 under pipelinek 0.46.0 with a three-stage probe — write, fail with
// exit 7, write — where the third stage's marker was never created and the
// process exited 1:
//
//     StepFailed ... failureKind: SCRIPT, message: shell exited with code 7
//     RunFinished ... outcome: failure        <- immediately, no stage 3
//
// That is what `set -euo pipefail` gave the old workflow, implicitly, and it is
// why no stage here needs a `set -e` of its own. It is also the reason every
// ADVISORY stage in `integration.pipeline.kts` ends its command with
// `|| echo 'ADVISORY: …'`: PipelineK has no `continue-on-error`, so a non-zero
// exit is the only way to stop a lane, and the only way not to stop one is to
// not exit non-zero. `|| echo` is not a loose substitute for a policy flag
// here; it is the whole mechanism.
//
// Both probes are reproducible with `scripts/ci/probe-pipelinek-semantics.sh`.
// It is not a contract — a CI runner does not ship the `pipelinek` binary — so
// it is the thing to re-run by hand after a version bump, and a changed answer
// is a change of policy to review rather than a version number to accept.
//
// Shell variables inside a **raw** string (`"""..."""` + trimIndent) must be
// written `${'$'}name`; the shell then receives a literal `$` and expands its
// own. This is not a version quirk and the distinction that matters is raw vs
// normal string, not 0.46.0 vs 0.39.1-rc1: in a normal single-line string
// `\$name` compiles and works, while in the multiline raw strings every real
// step uses it fails with "Unresolved reference". `\${'$'}name` is wrong
// everywhere: the backslash survives into the emitted script and bash fails
// with "syntax error near unexpected token `('".
//
// `pipelinek` on PATH resolves 0.46.0. That was not free. A symlink at
// ~/.local/bin/pipelinek pointing at a 0.39.0 install took precedence over the
// asdf shim and ran every pipeline against a compiler the repo did not pin. If
// validation and run ever disagree again, check `command -v pipelinek` first:
// the bug is a version mismatch, never the script.
//
//   pipelinek run merge-gate.pipeline.kts --db <journal.db>
import java.io.File

val repoRoot: String = File(".").canonicalPath
val cd = "cd \"$repoRoot\""

// The file that says what gates a merge. Declared here so `repo-contract` can
// check that the pipeline it is running inside actually exists, instead of
// checking that some other orchestrator's file does. That check used to name
// `.github/workflows/pr-ci.yml`, which meant the pipeline refused to run until
// a file that the cutover exists to delete was present: the gate could not
// outlive its own predecessor. The name below is the same fact the wiring
// contracts read from `scripts/ci/pipeline_authority.py`.
val authority = "merge-gate.pipeline.kts"

// Where a stage that produces a lot of output leaves it, so that a failure is
// something a person can read instead of an exit code with no story attached.
// Outside the repository: a lane that writes its logs into the tree makes
// `cargo fmt --check` and the contract suite see files nobody committed.
val logDir: String = File(System.getProperty("java.io.tmpdir"), "cognicode-logs").absolutePath
val cdLog = "mkdir -p \"$logDir\""

pipeline {
    stages {
        stage("static") {
            // toolchain fmt clippy — what a merge has to look clean
            // Formerly 4 stage(s); one stage, same commands, same order.
            // Fail-fast is unchanged: PipelineK aborts the run on a non-zero exit,
            // and these blocks are sequenced with `set -euo pipefail` in force.
            sh("""
            # ── toolchain
            # ─────────────────────────────────────────────
cargo --version && rustc --version

            # ── repo-contract
            # ─────────────────────────────────────────────

                    test -f "$repoRoot/Cargo.toml" || {
                        echo "FAIL: $repoRoot has no Cargo.toml"
                        exit 1
                    }
                    test -f "$repoRoot/$authority" || {
                        echo "FAIL: the merge authority $authority is missing from \$repoRoot."
                        echo "      A pipeline that cannot find itself cannot gate anything."
                        exit 1
                    }
                    test -d "$repoRoot/scripts/ci" || {
                        echo "FAIL: \$repoRoot/scripts/ci is missing, so there is no contract suite to run"
                        exit 1
                    }
                    echo "repo=$repoRoot authority=$authority crates=$(ls -d $repoRoot/crates/*/ | wc -l)"

            # ── fmt
            # ─────────────────────────────────────────────
$cd && cargo fmt --all -- --check

            # ── clippy
            # ─────────────────────────────────────────────
$cd && cargo clippy --workspace --all-targets -- -D warnings
            """.trimIndent())
        }

        stage("docs") {
            // the workspace builds its documentation with zero warnings
            // Formerly 1 stage(s); one stage, same commands, same order.
            // Fail-fast is unchanged: PipelineK aborts the run on a non-zero exit,
            // and these blocks are sequenced with `set -euo pipefail` in force.
            sh("""
            # ── rustdoc-gate
            # ─────────────────────────────────────────────
$cd && cargo test -p cognicode-runtime --test m011_rustdoc_gate --quiet
            """.trimIndent())
        }

        stage("contracts") {
            // the checks that keep every other gate honest
            // Formerly 2 stage(s); one stage, same commands, same order.
            // Fail-fast is unchanged: PipelineK aborts the run on a non-zero exit,
            // and these blocks are sequenced with `set -euo pipefail` in force.
            sh("""
            # ── contract-toolchain
            # ─────────────────────────────────────────────
            $cd || exit 1
            python3 -m pip install --disable-pip-version-check \"PyYAML==6.0.3\" \"jsonschema==4.25.1\"

            # ── contracts
            # ─────────────────────────────────────────────
$cd && bash scripts/ci/run-all-contracts.sh
            """.trimIndent())
        }

        stage("skills") {
            // skill bundles resolve their tools from the published catalog
            // Formerly 2 stage(s); one stage, same commands, same order.
            // Fail-fast is unchanged: PipelineK aborts the run on a non-zero exit,
            // and these blocks are sequenced with `set -euo pipefail` in force.
            sh("""
            # ── skills-validate
            # ─────────────────────────────────────────────
$cd && python3 scripts/validate_skills.py

            # ── skills-bundles
            # ─────────────────────────────────────────────
$cd && bash scripts/verify-skills.sh
            """.trimIndent())
        }

        stage("supply-chain") {
            // advisories and licences of the dependency graph
            // Formerly 3 stage(s); one stage, same commands, same order.
            // Fail-fast is unchanged: PipelineK aborts the run on a non-zero exit,
            // and these blocks are sequenced with `set -euo pipefail` in force.
            sh("""
            # ── install-cargo-deny
            # ─────────────────────────────────────────────

                    $cd || exit 1
                    if command -v cargo-deny >/dev/null 2>&1; then
                        echo "cargo-deny already present, skipping install"
                    else
                        cargo install cargo-deny --version 0.20.2 --locked
                    fi

            # ── advisories
            # ─────────────────────────────────────────────
$cd && cargo deny check advisories

            # ── licenses
            # ─────────────────────────────────────────────
$cd && cargo deny check licenses
            """.trimIndent())
        }

        stage("lsp") {
            // the LSP provider answers against a real rust-analyzer
            // Formerly 2 stage(s); one stage, same commands, same order.
            // Fail-fast is unchanged: PipelineK aborts the run on a non-zero exit,
            // and these blocks are sequenced with `set -euo pipefail` in force.
            sh("""
            # ── install-rust-analyzer
            # ─────────────────────────────────────────────

                    $cd || exit 1
                    if rust-analyzer --version >/dev/null 2>&1; then
                        echo "rust-analyzer already present, skipping install"
                    else
                        rustup component add rust-analyzer
                    fi

            # ── rust-analyzer-answers
            # ─────────────────────────────────────────────

                    $cd || exit 1
                    # Un shim en el PATH que sale con 1 haria que el paso
                    # anterior pasara y los tests se saltaran igual. Preguntar
                    # al binario es lo unico que distingue "instalado" de
                    # "utilizable", y es la misma distincion que hace
                    # `command_reports_version` en lsp_integration_test.rs.
                    if ! rust-analyzer --version >/dev/null 2>&1; then
                        echo "FAIL: rust-analyzer esta en el PATH pero no responde." >&2
                        echo "Si responde con error, el shim es de rustup y el" >&2
                        echo "componente no esta instalado: los tests LSP se" >&2
                        echo "saltarian en silencio, que es lo que este paso" >&2
                        echo "existe para evitar." >&2
                        exit 1
                    fi
                    rust-analyzer --version
            """.trimIndent())
        }

        stage("build") {
            // every binary a release needs, built in release profile
            // Formerly 6 stage(s); one stage, same commands, same order.
            // Fail-fast is unchanged: PipelineK aborts the run on a non-zero exit,
            // and these blocks are sequenced with `set -euo pipefail` in force.
            sh("""
            # ── build-binary
            # ─────────────────────────────────────────────

                    $cd || exit 1
                    set -euo pipefail
                    # The binaries were written wherever cargo says they go, which is
                    # not necessarily the repository's `target/`. This stage asked a
                    # hardcoded path for them and failed on 2026-10-02 with
                    # "target/release/cognicode was not built" immediately after the
                    # stage that builds it had succeeded. A verifier that hardcodes
                    # where the artifact lands is not verifying the artifact.
                    TARGET_DIR=$(scripts/ci/target-dir.sh) || exit 1
                    for bin in cognicode cognicode-mcp cognicode-control-plane; do
                        if [ ! -f "${'$'}TARGET_DIR/release/${'$'}bin" ]; then
                            echo "FAIL: ${'$'}TARGET_DIR/release/${'$'}bin was not built"
                            echo "  cargo target dir: ${'$'}TARGET_DIR"
                            exit 1
                        fi
                        if [ ! -x "${'$'}TARGET_DIR/release/${'$'}bin" ]; then
                            echo "FAIL: ${'$'}TARGET_DIR/release/${'$'}bin is not executable"
                            exit 1
                        fi
                    done
                    "${'$'}TARGET_DIR/release/cognicode" --version
                    "${'$'}TARGET_DIR/release/cognicode-control-plane" --help | head -5
                    "${'$'}TARGET_DIR/release/cognicode-mcp" --version

            # ── build-cognicode-mcp-release
            # ─────────────────────────────────────────────
$cd && cargo build --release -p cognicode-mcp

            # ── build-control-plane-release
            # ─────────────────────────────────────────────
$cd && cargo build --release --bin cognicode-control-plane

            # ── build-cognicode-cli-release
            # ─────────────────────────────────────────────
$cd && cargo build --release -p cognicode-cli

            # ── build-cognicode-release-bin
            # ─────────────────────────────────────────────
$cd && cargo build --release --bin cognicode

            # ── verify-release-binaries
            # ─────────────────────────────────────────────

                    $cd || exit 1
                    set -euo pipefail
                    # The binaries were written wherever cargo says they go, which is
                    # not necessarily the repository's `target/`. This stage asked a
                    # hardcoded path for them and failed on 2026-10-02 with
                    # "target/release/cognicode was not built" immediately after the
                    # stage that builds it had succeeded. A verifier that hardcodes
                    # where the artifact lands is not verifying the artifact.
                    TARGET_DIR=$(scripts/ci/target-dir.sh) || exit 1
                    for bin in cognicode cognicode-mcp cognicode-control-plane; do
                        if [ ! -f "${'$'}TARGET_DIR/release/${'$'}bin" ]; then
                            echo "FAIL: ${'$'}TARGET_DIR/release/${'$'}bin was not built"
                            echo "  cargo target dir: ${'$'}TARGET_DIR"
                            exit 1
                        fi
                        if [ ! -x "${'$'}TARGET_DIR/release/${'$'}bin" ]; then
                            echo "FAIL: ${'$'}TARGET_DIR/release/${'$'}bin is not executable"
                            exit 1
                        fi
                    done
                    "${'$'}TARGET_DIR/release/cognicode" --version
                    "${'$'}TARGET_DIR/release/cognicode-control-plane" --help | head -5
                    "${'$'}TARGET_DIR/release/cognicode-mcp" --version
            """.trimIndent())
        }

        stage("selector") {
            // the CR-08 suite selector and its own contract
            // Formerly 2 stage(s); one stage, same commands, same order.
            // Fail-fast is unchanged: PipelineK aborts the run on a non-zero exit,
            // and these blocks are sequenced with `set -euo pipefail` in force.
            sh("""
            # ── qw08-crate-selector
            # ─────────────────────────────────────────────
$cd && cargo test -p cognicode-cli --test qw08_crate_selector --quiet

            # ── select-suites-script
            # ─────────────────────────────────────────────

                    $cd || exit 1
                    set -euo pipefail
                    PATHS="scripts/ci/run-all-contracts.sh"
                    SELECT_OUTPUT="${'$'}(SELECT_PATHS="${'$'}PATHS" bash scripts/ci/select-suites.sh)"
                    echo "${'$'}SELECT_OUTPUT"
            """.trimIndent())
        }

        stage("tests") {
            // the product test battery
            // Formerly 40 stage(s); one stage, same commands, same order.
            // Fail-fast is unchanged: PipelineK aborts the run on a non-zero exit,
            // and these blocks are sequenced with `set -euo pipefail` in force.
            sh("""
            # ── core-lib
            # ─────────────────────────────────────────────
$cd && cargo test -p cognicode-core --lib --quiet

            # ── core-adversarial-campaign
            # ─────────────────────────────────────────────
$cd && cargo test -p cognicode-core --test prf_sec_07_adversarial_campaign --quiet

            # ── core-prf-mcp-05
            # ─────────────────────────────────────────────
$cd && cargo test -p cognicode-core --lib prf_mcp_05 --quiet

            # ── core-adversarial-e2e
            # ─────────────────────────────────────────────
$cd && cargo test -p cognicode-core --test prf_h06_adversarial_e2e --quiet

            # ── core-unrestricted-evidence-kernel
            # ─────────────────────────────────────────────

                    $cd || exit 1
                    $cdLog || exit 1
                    set +e
                    cargo test -p cognicode-core --features evidence-kernel --quiet \
                        > "$logDir/core-unrestricted-evidence-kernel.log" 2>&1
                    rc=${'$'}?
                    set -e
                    tail -40 "$logDir/core-unrestricted-evidence-kernel.log"
                    echo "the full log is $logDir/core-unrestricted-evidence-kernel.log"
                    exit ${'$'}rc

            # ── core-a014-product-asset-resolution
            # ─────────────────────────────────────────────
$cd && cargo test -p cognicode-core --test a014_product_asset_resolution --quiet

            # ── cognicode-ladybug
            # ─────────────────────────────────────────────
$cd && cargo test -p cognicode-ladybug --lib --quiet

            # ── bin-tracking
            # ─────────────────────────────────────────────
$cd && bash scripts/ci/check-bin-tracking.sh

            # ── cr07-metrics-exposition
            # ─────────────────────────────────────────────
$cd && cargo test -p cognicode-mcp --test cr07_metrics_exposition_contract --quiet

            # ── cli-qw03-bin-tracking
            # ─────────────────────────────────────────────
$cd && cargo test -p cognicode-cli --test qw03_bin_tracking_guard --quiet

            # ── cli-qw04-preflight
            # ─────────────────────────────────────────────
$cd && cargo test -p cognicode-cli --test qw04_preflight_contract --quiet

            # ── cli-a013-lifecycle-gate-contract
            # ─────────────────────────────────────────────
$cd && cargo test -p cognicode-cli --test a013_lifecycle_gate_contract --quiet

            # ── cli-a014-capabilities-json
            # ─────────────────────────────────────────────
$cd && cargo test -p cognicode-cli --test a014_capabilities_json --quiet

            # ── cli-a015-licenses-gate
            # ─────────────────────────────────────────────
$cd && cargo test -p cognicode-cli --test a015_licenses_gate --quiet

            # ── cli-a015-onboarding-gate
            # ─────────────────────────────────────────────
$cd && cargo test -p cognicode-cli --test a015_onboarding_gate --quiet

            # ── cli-action-register-identity-contract
            # ─────────────────────────────────────────────
$cd && cargo test -p cognicode-cli --test action_register_identity_contract --quiet

            # ── cli-unrestricted-ladybug
            # ─────────────────────────────────────────────

                    $cd || exit 1
                    $cdLog || exit 1
                    set +e
                    cargo test -p cognicode-cli --features ladybug --quiet \
                        > "$logDir/cli-unrestricted-ladybug.log" 2>&1
                    rc=${'$'}?
                    set -e
                    tail -40 "$logDir/cli-unrestricted-ladybug.log"
                    echo "the full log is $logDir/cli-unrestricted-ladybug.log"
                    exit ${'$'}rc

            # ── cli-gate-coverage-contract
            # ─────────────────────────────────────────────
$cd && cargo test -p cognicode-cli --test cli_gate_coverage_contract --quiet

            # ── mcp-a009-agent-safe-profile
            # ─────────────────────────────────────────────
$cd && cargo test -p cognicode-mcp --test a009_agent_safe_profile --quiet

            # ── mcp-a010-tool-authority-audit
            # ─────────────────────────────────────────────
$cd && cargo test -p cognicode-mcp --test a010_tool_authority_audit --quiet

            # ── mcp-a012-structured-output
            # ─────────────────────────────────────────────
$cd && cargo test -p cognicode-mcp --test a012_structured_output --quiet

            # ── mcp-a013-lifecycle-uat
            # ─────────────────────────────────────────────
$cd && cargo test -p cognicode-mcp --test a013_lifecycle_uat --quiet

            # ── mcp-prf-sec-02-read-only-uat
            # ─────────────────────────────────────────────
$cd && cargo test -p cognicode-mcp --test prf_sec_02_read_only_uat --quiet

            # ── mcp-a016-tools-runtime-consistency
            # ─────────────────────────────────────────────
$cd && cargo test -p cognicode-mcp --test a016_tools_runtime_consistency --quiet

            # ── mcp-prf-h06-adversarial-e2e
            # ─────────────────────────────────────────────
            # MEDIDO 2026-10-15. Esta suite pasó de `cognicode-core` a
            # `cognicode-mcp` porque `CARGO_BIN_EXE_<name>` solo existe para
            # el paquete que declara el binario, y el binario bajo prueba es
            # `cognicode-mcp`. Ese traslado la dejó sin etapa que la nombrara:
            # el gate seguía verde y la suite de seguridad había dejado de
            # correr. Lo detectó `cli_gate_coverage_contract::every_mcp_suite
            # _is_named_by_the_gate_or_excluded_with_a_reason`, que es
            # exactamente el agujero que dice cubrir.
            #
            # Antes de ese commit la suite estaba en el stage `core-adversarial-e2e`.
$cd && cargo test -p cognicode-mcp --test prf_h06_adversarial_e2e --quiet

            # ── mcp-prf-ana-02-uat
            # ─────────────────────────────────────────────
$cd && cargo test -p cognicode-mcp --test prf_ana_02_uat --quiet

            # ── mcp-prf-ana-05-uat
            # ─────────────────────────────────────────────
$cd && cargo test -p cognicode-mcp --test prf_ana_05_uat --quiet

            # ── mcp-prf-ana-07-uat
            # ─────────────────────────────────────────────
$cd && cargo test -p cognicode-mcp --test prf_ana_07_uat --quiet

            # ── mcp-prf-ana-08-uat
            # ─────────────────────────────────────────────
$cd && cargo test -p cognicode-mcp --test prf_ana_08_uat --quiet

            # ── mcp-prf-cli-04-two-process-uat
            # ─────────────────────────────────────────────
$cd && cargo test -p cognicode-mcp --test prf_cli_04_two_process_uat --quiet

            # ── mcp-prf-f4-w3-corrupt-cache-recovery
            # ─────────────────────────────────────────────
$cd && cargo test -p cognicode-mcp --test prf_f4_w3_corrupt_cache_recovery --quiet

            # ── mcp-prf-f5-w3-signal-cancel
            # ─────────────────────────────────────────────
$cd && cargo test -p cognicode-mcp --test prf_f5_w3_signal_cancel --quiet

            # ── mcp-prf-mcp-02-uat
            # ─────────────────────────────────────────────
$cd && cargo test -p cognicode-mcp --test prf_mcp_02_uat --quiet

            # ── mcp-prf-sec-01-uat
            # ─────────────────────────────────────────────
$cd && cargo test -p cognicode-mcp --test prf_sec_01_uat --quiet

            # ── mcp-prf-sec-03-telemetry-optin-uat
            # ─────────────────────────────────────────────
$cd && cargo test -p cognicode-mcp --test prf_sec_03_telemetry_optin_uat --quiet

            # ── mcp-prf-sec-05-shutdown-recovery-uat
            # ─────────────────────────────────────────────
$cd && cargo test -p cognicode-mcp --test prf_sec_05_shutdown_recovery_uat --quiet

            # ── mcp-prf-state-01-data-catalog-uat
            # ─────────────────────────────────────────────
$cd && cargo test -p cognicode-mcp --test prf_state_01_data_catalog_uat --quiet

            # ── mcp-prf-state-02-uat
            # ─────────────────────────────────────────────
$cd && cargo test -p cognicode-mcp --test prf_state_02_uat --quiet

            # ── mcp-prf-state-03-04-uat
            # ─────────────────────────────────────────────
$cd && cargo test -p cognicode-mcp --test prf_state_03_04_uat --quiet

            # ── mcp-prf-state-03-concurrent-uat
            # ─────────────────────────────────────────────
$cd && cargo test -p cognicode-mcp --test prf_state_03_concurrent_uat --quiet

            # ── mcp-prf-state-04-isolation-uat
            # ─────────────────────────────────────────────
$cd && cargo test -p cognicode-mcp --test prf_state_04_isolation_uat --quiet
            """.trimIndent())
        }

        stage("governance") {
            // drift, compatibility and the gate-coverage contract
            // Formerly 5 stage(s); one stage, same commands, same order.
            // Fail-fast is unchanged: PipelineK aborts the run on a non-zero exit,
            // and these blocks are sequenced with `set -euo pipefail` in force.
            sh("""
            # ── capabilities-drift-lint
            # ─────────────────────────────────────────────
$cd && python3 sandbox/scripts/capabilities_drift_lint.py --strict

            # ── compat-matrix-0-97-x
            # ─────────────────────────────────────────────

                    $cd || exit 1
                    if [ -x sandbox/.compat/0.97.3/cognicode-mcp ]; then
                        echo "compat fixture present, running the matrix"
                        cargo test -p cognicode-mcp --test find_usages_compat_0_97 -- \
                            --skip compat_forward_0973_server_handles_modern_request
                    else
                        echo "SKIP: sandbox/.compat/0.97.3/cognicode-mcp absent (ADR-PRF-008 policy R1)"
                    fi

            # ── core-gate-coverage-contract
            # ─────────────────────────────────────────────
$cd && cargo test -p cognicode-core --features evidence-kernel --test core_gate_coverage_contract --quiet

            # ── e91-w7-regression-budget
            # ─────────────────────────────────────────────
$cd && cargo test -p cognicode-core --test e91_w7_regression_budget --quiet

            # ── e91-w8-stage-profile
            # ─────────────────────────────────────────────
$cd && cargo test -p cognicode-core --test e91_w8_stage_profile --quiet
            """.trimIndent())
        }
    }
}
