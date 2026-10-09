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
                    $cd || exit 1
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

        // ── El binario LSP del gate, y por que tiene que estar aqui ────────
        //
        // MEDIDO 2026-10-04. El gate corre `cargo test -p cognicode-core
        // --lib`, y en esa suite vive
        // `test_hierarchy_falls_through_within_the_bounded_readiness`, que se
        // auto-salta con un `println!` cuando `rust-analyzer` no esta en el
        // PATH. Es decir: el gate se saltaba el test y daba verde. Y no era
        // solo que faltara el binario: el shim de rustup lo hace existir sin
        // funcionar (`~/.cargo/bin/rust-analyzer` es un symlink a `rustup`, y
        // responde `Unknown binary` con codigo 1), de modo que un
        // `command -v` habria dado la respuesta equivocada. Medido con
        // `grep 'component add'` sobre este pipeline, los scripts de CI y los
        // workflows: nadie lo instalaba.
        //
        // El coste de no tenerlo esta medido en las dos direcciones, y por eso
        // la decision es facil: el gate completo tardaba 20m17s sin el
        // componente y 22m48s con el. Dos minutos y medio para que un test que
        // hoy no se ejecuta, se ejecute.
        //
        // Se instala como componente del toolchain y no como `cargo install`:
        // es un binario de rustup, `rustup component add` es idempotente y no
        // compila nada. El `if` evita el trabajo cuando ya esta, y la
        // comprobacion posterior convierte "lo instale" en "responde", que es
        // la distincion que el shim rompia.
        stage("lsp-toolchain") {
            stage("install-rust-analyzer") {
                sh(
                    """
                    $cd || exit 1
                    if rust-analyzer --version >/dev/null 2>&1; then
                        echo "rust-analyzer already present, skipping install"
                    else
                        rustup component add rust-analyzer
                    fi
                    """.trimIndent(),
                )
            }

            stage("rust-analyzer-answers") {
                sh(
                    """
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
                    """.trimIndent(),
                )
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
                    $cd || exit 1
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

            // QW-09 used to have two stages here: a shell guard and a Rust
            // contract, both reading `pr-ci.yml`. Both are gone, and the
            // property is stronger for it. The original question was GitHub's
            // — "does this job download the artifact an earlier job uploaded?" —
            // which only exists because every Actions job gets a fresh runner
            // and `needs:` does not mean "inherits artifacts". PipelineK has
            // neither problem: the stages share one filesystem (measured, not
            // assumed), so the surviving property is an ordering one — every
            // step that consumes a release artifact needs an earlier step that
            // produced it. `scripts/ci/test_pipeline_artifact_reachability.py`
            // states that, and it is discovered by the `contracts` stage above
            // rather than named by this one, and it scans every `*.pipeline.kts`
            // rather than a single workflow.

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
            //
            // It is also the only stage in this lane that runs a whole crate's
            // suite rather than one `--test` target, and it is the only stage
            // that died: four clean runs, all of them here, all of them after
            // 10.127 / 10.126 / 10.135 / 10.129 ms with exit 101 and not one
            // byte of output captured, in a run that had produced 30 KB from
            // the same command when the stage stood alone.
            //
            // What is NOT the cause, each measured rather than assumed: not the
            // size of the output (PipelineK captured 21 MB in this position,
            // from one writer and from thirty, in both cases whole); not stderr
            // (520 KB of it captured from a step that failed); not a capture
            // cap (700 KB and 21 MB both survived); not memory (55-65 GB free
            // at the moment of failure, no OOM kill); not disk; not the cargo
            // build-directory lock (held on purpose with flock: cargo waits
            // 120 s and is killed by the timeout, it does not exit 101); not
            // the stage's position (38 trivial stages in front of it pass) and
            // not any single predecessor (five contiguous prefixes of this lane
            // pass, and the stage passes after the ladybug crate, after a
            // feature switch on the same crate, and after core with
            // evidence-kernel).
            //
            // So this is a workaround for a defect inside the orchestrator, not
            // a diagnosis, and it is written down as one. What is known is that
            // redirecting this stage's output makes the difference: the same
            // lane, the same tree, the same command, passes 64 of 64 with the
            // redirection and fails 4 of 4 without it. The tail is there so
            // that if it ever fails again the output is on disk and named,
            // instead of a run database that says only "shell exited with
            // code 101" — which is what four failures produced.
            stage("cli-unrestricted-ladybug") {
                sh("""
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
                """.trimIndent())
            }

            // the gate contract itself (pr-ci.yml:578)
            stage("cli-gate-coverage-contract") {
                sh("$cd && cargo test -p cognicode-cli --test cli_gate_coverage_contract --quiet")
            }

            // mcp (pr-ci.yml:453-501)
            // One stage per contract, the way the workflow spelled them out. The
            // loop this replaces ran the same 22 commands in the same order, so
            // nothing about coverage changes; what changes is that a failure names the
            // contract instead of the loop, and a contract that asserts "the merge gate
            // runs me" becomes checkable by a reader and by a checker.
            stage("mcp-a009-agent-safe-profile") {
                sh("$cd && cargo test -p cognicode-mcp --test a009_agent_safe_profile --quiet")
            }
            // One stage per contract, the way the workflow spelled them out. The
            // loop this replaces ran the same 22 commands in the same order, so
            // nothing about coverage changes; what changes is that a failure names the
            // contract instead of the loop, and a contract that asserts "the merge gate
            // runs me" becomes checkable by a reader and by a checker.
            stage("mcp-a010-tool-authority-audit") {
                sh("$cd && cargo test -p cognicode-mcp --test a010_tool_authority_audit --quiet")
            }
            // One stage per contract, the way the workflow spelled them out. The
            // loop this replaces ran the same 22 commands in the same order, so
            // nothing about coverage changes; what changes is that a failure names the
            // contract instead of the loop, and a contract that asserts "the merge gate
            // runs me" becomes checkable by a reader and by a checker.
            stage("mcp-a012-structured-output") {
                sh("$cd && cargo test -p cognicode-mcp --test a012_structured_output --quiet")
            }
            // One stage per contract, the way the workflow spelled them out. The
            // loop this replaces ran the same 22 commands in the same order, so
            // nothing about coverage changes; what changes is that a failure names the
            // contract instead of the loop, and a contract that asserts "the merge gate
            // runs me" becomes checkable by a reader and by a checker.
            stage("mcp-a013-lifecycle-uat") {
                sh("$cd && cargo test -p cognicode-mcp --test a013_lifecycle_uat --quiet")
            }
            // One stage per contract, the way the workflow spelled them out. The
            // loop this replaces ran the same 22 commands in the same order, so
            // nothing about coverage changes; what changes is that a failure names the
            // contract instead of the loop, and a contract that asserts "the merge gate
            // runs me" becomes checkable by a reader and by a checker.
            stage("mcp-prf-sec-02-read-only-uat") {
                sh("$cd && cargo test -p cognicode-mcp --test prf_sec_02_read_only_uat --quiet")
            }
            // One stage per contract, the way the workflow spelled them out. The
            // loop this replaces ran the same 22 commands in the same order, so
            // nothing about coverage changes; what changes is that a failure names the
            // contract instead of the loop, and a contract that asserts "the merge gate
            // runs me" becomes checkable by a reader and by a checker.
            stage("mcp-a016-tools-runtime-consistency") {
                sh("$cd && cargo test -p cognicode-mcp --test a016_tools_runtime_consistency --quiet")
            }
            // One stage per contract, the way the workflow spelled them out. The
            // loop this replaces ran the same 22 commands in the same order, so
            // nothing about coverage changes; what changes is that a failure names the
            // contract instead of the loop, and a contract that asserts "the merge gate
            // runs me" becomes checkable by a reader and by a checker.
            stage("mcp-prf-ana-02-uat") {
                sh("$cd && cargo test -p cognicode-mcp --test prf_ana_02_uat --quiet")
            }
            // One stage per contract, the way the workflow spelled them out. The
            // loop this replaces ran the same 22 commands in the same order, so
            // nothing about coverage changes; what changes is that a failure names the
            // contract instead of the loop, and a contract that asserts "the merge gate
            // runs me" becomes checkable by a reader and by a checker.
            stage("mcp-prf-ana-05-uat") {
                sh("$cd && cargo test -p cognicode-mcp --test prf_ana_05_uat --quiet")
            }
            // One stage per contract, the way the workflow spelled them out. The
            // loop this replaces ran the same 22 commands in the same order, so
            // nothing about coverage changes; what changes is that a failure names the
            // contract instead of the loop, and a contract that asserts "the merge gate
            // runs me" becomes checkable by a reader and by a checker.
            stage("mcp-prf-ana-07-uat") {
                sh("$cd && cargo test -p cognicode-mcp --test prf_ana_07_uat --quiet")
            }
            // One stage per contract, the way the workflow spelled them out. The
            // loop this replaces ran the same 22 commands in the same order, so
            // nothing about coverage changes; what changes is that a failure names the
            // contract instead of the loop, and a contract that asserts "the merge gate
            // runs me" becomes checkable by a reader and by a checker.
            stage("mcp-prf-ana-08-uat") {
                sh("$cd && cargo test -p cognicode-mcp --test prf_ana_08_uat --quiet")
            }
            // One stage per contract, the way the workflow spelled them out. The
            // loop this replaces ran the same 22 commands in the same order, so
            // nothing about coverage changes; what changes is that a failure names the
            // contract instead of the loop, and a contract that asserts "the merge gate
            // runs me" becomes checkable by a reader and by a checker.
            stage("mcp-prf-cli-04-two-process-uat") {
                sh("$cd && cargo test -p cognicode-mcp --test prf_cli_04_two_process_uat --quiet")
            }
            // One stage per contract, the way the workflow spelled them out. The
            // loop this replaces ran the same 22 commands in the same order, so
            // nothing about coverage changes; what changes is that a failure names the
            // contract instead of the loop, and a contract that asserts "the merge gate
            // runs me" becomes checkable by a reader and by a checker.
            stage("mcp-prf-f4-w3-corrupt-cache-recovery") {
                sh("$cd && cargo test -p cognicode-mcp --test prf_f4_w3_corrupt_cache_recovery --quiet")
            }
            // One stage per contract, the way the workflow spelled them out. The
            // loop this replaces ran the same 22 commands in the same order, so
            // nothing about coverage changes; what changes is that a failure names the
            // contract instead of the loop, and a contract that asserts "the merge gate
            // runs me" becomes checkable by a reader and by a checker.
            stage("mcp-prf-f5-w3-signal-cancel") {
                sh("$cd && cargo test -p cognicode-mcp --test prf_f5_w3_signal_cancel --quiet")
            }
            // One stage per contract, the way the workflow spelled them out. The
            // loop this replaces ran the same 22 commands in the same order, so
            // nothing about coverage changes; what changes is that a failure names the
            // contract instead of the loop, and a contract that asserts "the merge gate
            // runs me" becomes checkable by a reader and by a checker.
            stage("mcp-prf-mcp-02-uat") {
                sh("$cd && cargo test -p cognicode-mcp --test prf_mcp_02_uat --quiet")
            }
            // One stage per contract, the way the workflow spelled them out. The
            // loop this replaces ran the same 22 commands in the same order, so
            // nothing about coverage changes; what changes is that a failure names the
            // contract instead of the loop, and a contract that asserts "the merge gate
            // runs me" becomes checkable by a reader and by a checker.
            stage("mcp-prf-sec-01-uat") {
                sh("$cd && cargo test -p cognicode-mcp --test prf_sec_01_uat --quiet")
            }
            // One stage per contract, the way the workflow spelled them out. The
            // loop this replaces ran the same 22 commands in the same order, so
            // nothing about coverage changes; what changes is that a failure names the
            // contract instead of the loop, and a contract that asserts "the merge gate
            // runs me" becomes checkable by a reader and by a checker.
            stage("mcp-prf-sec-03-telemetry-optin-uat") {
                sh("$cd && cargo test -p cognicode-mcp --test prf_sec_03_telemetry_optin_uat --quiet")
            }
            // One stage per contract, the way the workflow spelled them out. The
            // loop this replaces ran the same 22 commands in the same order, so
            // nothing about coverage changes; what changes is that a failure names the
            // contract instead of the loop, and a contract that asserts "the merge gate
            // runs me" becomes checkable by a reader and by a checker.
            stage("mcp-prf-sec-05-shutdown-recovery-uat") {
                sh("$cd && cargo test -p cognicode-mcp --test prf_sec_05_shutdown_recovery_uat --quiet")
            }
            // One stage per contract, the way the workflow spelled them out. The
            // loop this replaces ran the same 22 commands in the same order, so
            // nothing about coverage changes; what changes is that a failure names the
            // contract instead of the loop, and a contract that asserts "the merge gate
            // runs me" becomes checkable by a reader and by a checker.
            stage("mcp-prf-state-01-data-catalog-uat") {
                sh("$cd && cargo test -p cognicode-mcp --test prf_state_01_data_catalog_uat --quiet")
            }
            // One stage per contract, the way the workflow spelled them out. The
            // loop this replaces ran the same 22 commands in the same order, so
            // nothing about coverage changes; what changes is that a failure names the
            // contract instead of the loop, and a contract that asserts "the merge gate
            // runs me" becomes checkable by a reader and by a checker.
            stage("mcp-prf-state-02-uat") {
                sh("$cd && cargo test -p cognicode-mcp --test prf_state_02_uat --quiet")
            }
            // One stage per contract, the way the workflow spelled them out. The
            // loop this replaces ran the same 22 commands in the same order, so
            // nothing about coverage changes; what changes is that a failure names the
            // contract instead of the loop, and a contract that asserts "the merge gate
            // runs me" becomes checkable by a reader and by a checker.
            stage("mcp-prf-state-03-04-uat") {
                sh("$cd && cargo test -p cognicode-mcp --test prf_state_03_04_uat --quiet")
            }
            // One stage per contract, the way the workflow spelled them out. The
            // loop this replaces ran the same 22 commands in the same order, so
            // nothing about coverage changes; what changes is that a failure names the
            // contract instead of the loop, and a contract that asserts "the merge gate
            // runs me" becomes checkable by a reader and by a checker.
            stage("mcp-prf-state-03-concurrent-uat") {
                sh("$cd && cargo test -p cognicode-mcp --test prf_state_03_concurrent_uat --quiet")
            }
            // One stage per contract, the way the workflow spelled them out. The
            // loop this replaces ran the same 22 commands in the same order, so
            // nothing about coverage changes; what changes is that a failure names the
            // contract instead of the loop, and a contract that asserts "the merge gate
            // runs me" becomes checkable by a reader and by a checker.
            stage("mcp-prf-state-04-isolation-uat") {
                sh("$cd && cargo test -p cognicode-mcp --test prf_state_04_isolation_uat --quiet")
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
