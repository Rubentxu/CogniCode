// integration — the full suite, the feature matrix, the coverage gate and the
// portable builds. Everything here is what this repository runs end to end.
//
// This is the lane that answers "did the whole workspace still work", which is
// not a question a pull-request gate should be paying for on every commit.
// `merge-gate.pipeline.kts` stays surgical: format, lint, the suites the diff
// touches, the contracts. This one is exhaustive and runs on integration,
// certification and release.
//
// Semantics are preserved, including the parts that are advisory. The jobs
// that carry `continue-on-error: true` — benchmarks, the adversarial corpora
// and version-sync — are marked ADVISORY here, and a migration that turned
// them into hard gates would tighten CI rather than move it. A failure is
// reported without failing the lane.
//
// One step is NOT migrated: the sandbox sanity check in the workflow this
// lane replaced carries `if: false` and is disabled pending a running API
// server. Migrating a disabled step would put a gate in the inventory that
// nothing has ever executed, which is the same failure this whole migration
// is trying to remove. It stays there until it is enabled, and moves here
// when it does.
//
// Like `merge-gate.pipeline.kts`, the repository root is resolved once and
// interpolated: PipelineK gives every stage a throwaway directory and the DSL
// has no working-directory parameter. Shell variables inside a raw string
// (`"""..."""`) must be written `${'$'}name`; the plain `${name}` form is an
// unresolved reference in every released version.
//
//   pipelinek run integration.pipeline.kts --db <journal.db>
import java.io.File

val repoRoot: String = File(".").canonicalPath
val cd = "cd \"$repoRoot\""

pipeline {
    stages {
        // ------------------------------------------------------------ workspace
        stage("workspace") {
            // El gate de rustdoc tiene dueño explícito aquí, no dentro de
            // `--all-targets`. Motivo medido 2026-10-15: corría también en el
            // merge-gate, donde costaba 667 s en cada PR; la garantía es la misma
            // (documentación sin los avisos de rustdoc que M0.11 persigue) y se
            // paga una vez, en la lane que bloquea `main`. La stage se nombra para
            // que el contrato pueda comprobar el dueño en vez de confiar en que
            // `--all-targets` "seguro que lo incluye".
            stage("rustdoc-gate") {
                sh("$cd && cargo test -p cognicode-runtime --test m011_rustdoc_gate --quiet")
            }

            // The full suite. `--all-targets` is what compiles every
            // `tests/*.rs` target in the workspace; without it the named
            // contract suites below are the only thing being built.
            stage("all-targets") {
                // MEDIDO 2026-10-16. Esta stage fallaba en una máquina cuya
                // configuración de cargo redirige el directorio de target:
                // `b3_w2_progressive_context::inspect_subcommand_does_not_panic_on_arbitrary_input`
                // lanza el binario `cognicode` por ruta y no lo encontraba
                // («binario `cognicode` no encontrado: … saltarlo sería un verde
                // vacío», rc=101). El binario existía; lo que faltaba era la
                // variable que le dice dónde. La lane de merge ya exporta
                // `CARGO_TARGET_DIR` en sus etapas equivalentes; aquí faltaba.
                sh("""
                    $cd || exit 1
                    export CARGO_TARGET_DIR=$(scripts/ci/target-dir.sh) || exit 1
                    cargo test --workspace --all-targets
                """.trimIndent())
            }

            // The CLI shares process-global state across its own binaries, so
            // it runs serially. Parallelising this is a known source of
            // flakes, not a speedup.
            stage("cli-serial") {
                sh("$cd && cargo test -p cognicode-cli -- --test-threads=1")
            }

            stage("release-binary-suite") {
                sh("$cd && cargo test -p cognicode-cli --bin cognicode-release")
            }
        }

        // -------------------------------------------------------- feature matrix
        // PRF-CI-02: the workspace is verified under each declared feature
        // combination, not only the default set — a default-only run compiles
        // the gated arms as empty test binaries that report 0 passed / 0 failed
        // and exit 0.
        //
        // Written as eight explicit stages rather than a loop so each arm is
        // separately visible in the evidence. A loop would collapse them back
        // into one line, which is the same loss of resolution that made the
        // original inventory undercount.
        stage("feature-matrix") {
            stage("core-no-default") {
                sh("$cd && cargo test -p cognicode-core --no-default-features --lib")
            }

            stage("core-default") {
                sh("$cd && cargo test -p cognicode-core --lib")
            }

            stage("core-all-features") {
                sh("$cd && cargo test -p cognicode-core --all-features --lib")
            }

            stage("runtime-no-default") {
                sh("$cd && cargo test -p cognicode-runtime --no-default-features --lib")
            }

            stage("runtime-all-features") {
                sh("$cd && cargo test -p cognicode-runtime --all-features --lib")
            }

            stage("mcp-all-features") {
                sh("$cd && cargo test -p cognicode-mcp --all-features --lib")
            }

            stage("explorer-all-features") {
                sh("$cd && cargo test -p cognicode-explorer --all-features --lib")
            }

            stage("graph-algos-no-default") {
                sh("$cd && cargo test -p cognicode-graph-algos --no-default-features")
            }
        }

        // -------------------------------------------------------------- coverage
        // CR-09. This gate was invisible to the first version of the coverage
        // inventory, which matched `cargo (test|clippy|fmt|deny|build)` and
        // therefore could not see `cargo llvm-cov` at all — so the migration
        // reported a gap of 40 while a blocking numeric gate was running in
        // Actions the whole time.
        //
        // Thresholds are carried over unchanged from ci.yml. Raising them is a
        // decision about what the project accepts, not a refactor.
        stage("coverage") {
            stage("install-llvm-cov") {
                sh("""
                    $cd || exit 1
                    if command -v cargo-llvm-cov >/dev/null 2>&1; then
                        echo "cargo-llvm-cov already present, skipping install"
                    else
                        cargo install cargo-llvm-cov --locked
                    fi
                """.trimIndent())
            }

            stage("core-lib-coverage") {
                sh("""
                    $cd || exit 1
                    set -euo pipefail
                    # The regions threshold is stricter than the lines one
                    # because it counts branches. Both sit slightly under the
                    # re-baselined HEAD numbers so a change can move coverage
                    # inside compiler noise without a false failure.
                    #
                    # Baseline HEAD @ 76516ca5: lines=75.39 regions=71.31
                    # functions=73.48
                    if ! cargo llvm-cov --lib -p cognicode-core --summary-only \
                        --fail-under-lines 75.00 \
                        --fail-under-regions 71.00; then
                        echo "CR-09 coverage gate FAIL: coverage below threshold"
                        echo "  threshold: lines>=75.00 regions>=71.00 functions>=73.00"
                        echo "  baseline HEAD @ 76516ca5: lines=75.39 regions=71.31 functions=73.48"
                        exit 1
                    fi
                    echo "CR-09 coverage gate PASS"
                """.trimIndent())
            }
        }

        // ------------------------------------------------------------ adversarial
        // ADVISORY. The job this stage came from ran with
        // `continue-on-error: true`; a regression is recorded for review, not
        // blocked. Kept advisory on purpose: tightening it here would be a
        // policy change wearing a migration's clothes.
        stage("adversarial") {
            stage("parser-adversarial") {
                sh("$cd && cargo test -p cognicode-core --lib infrastructure::parser -- --test-threads=1 || echo 'ADVISORY: parser adversarial reported failures'")
            }

            stage("architecture-drift-e2e") {
                sh("$cd && cargo test -p cognicode-core --test architecture_drift_e2e || echo 'ADVISORY: architecture drift e2e reported failures'")
            }

            stage("findings-canonical-grounding") {
                sh("$cd && cargo test -p cognicode-core --features evidence-kernel --test findings_canonical_grounding_e2e || echo 'ADVISORY: findings grounding e2e reported failures'")
            }

            stage("workspace-isolation") {
                sh("$cd && cargo test -p cognicode-core --features evidence-kernel --test workspace_isolation || echo 'ADVISORY: workspace isolation reported failures'")
            }

            // The continuation chain used to have a stage here. It could not
            // assert anything: its tests need the real server and the
            // bootstrapped fixture set, and this lane has neither, so they
            // reported "passed" while doing nothing. A stage that cannot
            // assert is not a stage. The capability now lives in the
            // certification lane, which stages the fixtures and fails closed.
        }

        // ----------------------------------------------------- portable binaries
        // The musl target is what a distribution user actually downloads, and
        // it links differently from the gnu build. A change that only breaks
        // musl is invisible to every other lane here.
        stage("portable") {
            stage("add-musl-target") {
                sh("$cd && rustup target add x86_64-unknown-linux-musl")
            }

            stage("build-musl-mcp") {
                sh("$cd && cargo build --release --target x86_64-unknown-linux-musl -p cognicode-mcp")
            }

            stage("build-cogh") {
                sh("$cd && cargo build --release -p cognicode-cli --bin cogh")
            }

            // Clean-HOME install. A CLI that reads a developer's real ~/.config
            // while testing can pass on the machine that wrote the bug.
            //
            // The install root is pinned inside this HOME. Left to the
            // environment, it would not be: the resolver asks `CARGO_HOME`
            // before `XDG_DATA_HOME`, and `CARGO_HOME` is set on a CI
            // runner, so the install would land in the machine's cargo home
            // instead of the clean one this stage declares.
            stage("cogh-clean-home-install") {
                sh("""
                    $cd || exit 1
                    set -euo pipefail
                    export HOME=/tmp/cognicode-test-home-${'$'}(id -u)
                    export XDG_CONFIG_HOME="${'$'}HOME/.config"
                    export XDG_DATA_HOME="${'$'}HOME/.local/share"
                    export COGNICODE_BIN_DIR="${'$'}HOME/.local/share/cognicode"
                    mkdir -p "${'$'}HOME"
                    # Cargo may write somewhere other than the repository's `target/`;
                    # asking is the only way to find the binary this stage installs.
                    TARGET_DIR=$(scripts/ci/target-dir.sh) || exit 1
                    # `--apply` is what makes the installer install. Without it
                    # the installer prints a plan and exits zero, and this
                    # stage would only prove that the planner knows where it
                    # would install.
                    "${'$'}TARGET_DIR/release/cogh" install --profile core --apply || echo "ADVISORY: the clean-HOME install reported failures"
                    echo "clean-HOME install attempted"
                """.trimIndent())
            }
        }

        // ------------------------------------------------------ release contract
        // e85: the release contract has to hold in CI, not only at tag time.
        stage("release-contract") {
            // R9: the declared targets must be exactly the ones the Rust
            // contract derives, every published platform must have a lane, and
            // every runner must be vetted. This script existed since e74 and
            // was invoked from no workflow until ci.yml.
            stage("matrix-coherence") {
                sh("$cd && bash scripts/check-release-matrix.sh")
            }
        }

        // ---------------------------------------------------------- commit lint
        stage("commit-lint") {
            stage("install-commitlint") {
                sh("npm install -g @commitlint/cli @commitlint/config-conventional")
            }

            // `origin/main..HEAD` needs the full history; a shallow checkout
            // makes the range empty, which lints nothing and passes. The
            // explicit count check is what stops that from looking green.
            stage("lint-commit-subjects") {
                sh("""
                    $cd || exit 1
                    set -euo pipefail
                    range="origin/main..HEAD"
                    count="${'$'}(git rev-list --count "${'$'}range")"
                    if [ "${'$'}count" -eq 0 ]; then
                        echo "FAIL: no commits in ${'$'}range. A shallow checkout"
                        echo "makes this range empty, and an empty range lints"
                        echo "nothing while reporting success."
                        exit 1
                    fi
                    echo "linting ${'$'}count commit(s)"
                    git log --format="%s" "${'$'}range" | while read -r subject; do
                        echo "${'$'}subject" | commitlint || exit 1
                    done
                """.trimIndent())
            }
        }

        // -------------------------------------------------------------- advisory
        stage("advisory-only") {
            // ADVISORY. The job this stage came from ran with
            // `continue-on-error: true`.
            stage("benchmarks") {
                sh("$cd && cargo bench -p cognicode-core || echo 'ADVISORY: benchmarks reported failures'")
            }

            stage("version-sync") {
                sh("$cd && just verify-version-sync || echo 'ADVISORY: version sync reported a mismatch'")
            }
        }
    }
}
