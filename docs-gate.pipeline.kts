// docs-gate — the lane behind the `merge-gate` status when a change touches
// documentation only.
//
// `scripts/ci/publish-merge-gate.sh` asks `scripts/ci/change-class.sh` what the
// change is. `code` runs `merge-gate.pipeline.kts`, as always. `docs` runs this
// file, and the published status says which of the two produced it, so a
// reader of the status can tell a full verdict from a documentation verdict.
//
// What a documentation change can break, and where this lane looks for it:
//   * the Python contracts, which read the roadmap, the journal, the README and
//     the pipelines: all of them run (`contracts`);
//   * the Rust tests that open repository documents by path: they run against
//     the warm per-checkout target dir, where nothing has to recompile because
//     no crate does `include_str!` of a `.md` (`docs-readers`).
// What it cannot break: compiled code. Nothing in the diff is compiled, which
// is the whole condition `change-class.sh` checks before this file runs and
// that the first stage below checks again.
//
//   pipelinek run --db .pipelinek/docs-gate.db docs-gate.pipeline.kts
import java.io.File

val repoRoot: String = File(".").canonicalPath
val cd = "cd \"$repoRoot\""

pipeline {
    stages {
        stage("scope") {
            // A documentation lane run on a code change would publish a green
            // that nothing earned. It refuses instead, whoever launched it.
            stage("change-is-docs-only") {
                sh("""
                    set -eo pipefail
                    $cd || exit 1
                    class=${'$'}(bash scripts/ci/change-class.sh HEAD origin/main)
                    if [ "${'$'}class" != "docs" ]; then
                        echo "FAIL: change-class says '${'$'}class'; this lane only judges documentation-only changes."
                        echo "      Run merge-gate.pipeline.kts instead (publish-merge-gate.sh picks it on its own)."
                        exit 1
                    fi
                    git diff --name-only ${'$'}(git merge-base origin/main HEAD) HEAD
                """.trimIndent())
            }

            stage("build-isolation") {
                sh("bash scripts/ci/check-build-isolation.sh")
            }
        }

        stage("contracts") {
            stage("contract-toolchain") {
                sh("$cd && python3 -m pip install --disable-pip-version-check \"PyYAML==6.0.3\" \"jsonschema==4.25.1\"")
            }

            stage("build-cognicode-for-contracts") {
                sh("$cd && cargo build -p cognicode-cli --bin cognicode")
            }

            stage("install-cosign") {
                sh("$cd && bash scripts/ci/install-cosign.sh --version 3.1.3")
            }

            stage("contracts") {
                sh("$cd && bash scripts/ci/run-all-contracts.sh")
            }
        }

        stage("docs-readers") {
            // Each crate runs with the feature set the merge gate uses for it, so
            // the test binaries are the ones already built in this target dir.
            stage("rust-tests-that-read-docs") {
                sh("""
                    set -eo pipefail
                    $cd || exit 1
                    pairs=${'$'}(bash scripts/ci/docs-readers.sh HEAD origin/main)
                    echo "${'$'}pairs"
                    for crate in ${'$'}(printf '%s\n' "${'$'}pairs" | cut -d' ' -f1 | sort -u); do
                        case "${'$'}crate" in
                            cognicode-core) features="--features evidence-kernel" ;;
                            cognicode-cli) features="--features ladybug" ;;
                            *) features="" ;;
                        esac
                        targets=${'$'}(printf '%s\n' "${'$'}pairs" | awk -v c="${'$'}crate" '${'$'}1 == c { printf " --test %s", ${'$'}2 }')
                        cargo test -p "${'$'}crate" ${'$'}features ${'$'}targets --quiet
                    done
                """.trimIndent())
            }
        }
    }
}
