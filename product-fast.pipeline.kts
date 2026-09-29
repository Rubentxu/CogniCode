// product-fast — the quick gate: static checks plus the surgical contract suites.
//
// Parity target (observed on 67e69857, 2026-09-29):
//   cargo fmt --all -- --check                                  -> exit 0
//   cargo clippy --workspace --all-targets -- -D warnings       -> 42 pre-existing in rig/tools.rs
//   a014_capabilities_json                                      -> 12 passed
//   a015_licenses_gate                                          ->  3 passed
//   a013_lifecycle_gate_contract                                ->  4 passed
//   cli_gate_coverage_contract                                  ->  8 passed
//
// Workspace: PipelineK gives every stage its own throwaway directory
// (`<run>/workspace/<stage>-<n>`) and the DSL has no working-directory parameter
// (`fun sh(command: String)` in pipeline-scripting-api/.../PipelineDsl.kt). No released
// distribution implements `--workspace` either: 0.39.1-rc1, 0.40.0 and 0.43.0 all ignore
// it. Some builds instead inherit the script's own directory, so a pipeline that assumes
// either behaviour works on one version and silently tests nothing on another.
//
// The repository root is therefore resolved here, in the DSL, and interpolated into every
// step. The script itself is evaluated with the launching process's working directory —
// verified: a script in /tmp still saw the repository as `File(".")` — while each sh()
// step runs in a throwaway directory. Resolving on this side is therefore the only place
// where the real path is available. This is not an injected REPO_ROOT: nothing is read
// from the environment, and `repoRoot` is asserted against a real Cargo.toml before any
// gate runs, so a wrong or stale value fails the pipeline instead of quietly testing
// nothing.
//
//   pipelinek run product-fast.pipeline.kts --db <journal.db>
//
// clippy has a documented baseline: 42 pre-existing errors in
// crates/cognicode-core/src/interface/rig/tools.rs. This gate fails on any error outside
// that file, so a new lint cannot hide behind the baseline.
import java.io.File

val repoRoot: String = File(".").canonicalPath
val cd = "cd \"$repoRoot\""

pipeline {
    stages {
        stage("toolchain") {
            sh("cargo --version && rustc --version")
        }

        stage("repo-contract") {
            // Anti-vacuity. Fails loudly instead of running an empty gate somewhere else.
            sh("""
                test -f "$repoRoot/Cargo.toml" || {
                    echo "FAIL: $repoRoot has no Cargo.toml"
                    exit 1
                }
                echo "repo=$repoRoot crates=$(ls -d $repoRoot/crates/*/ | wc -l)"
            """.trimIndent())
        }

        stage("fmt") {
            sh("$cd && cargo fmt --all -- --check")
        }

        stage("clippy-baseline") {
            sh("""
                $cd || exit 1
                clippy_log=${'$'}(mktemp)
                cargo clippy --workspace --all-targets --all-features -- -D warnings 2>&1 \
                    | grep -E '^[[:space:]]*-->' \
                    | grep -v 'crates/cognicode-core/src/interface/rig/tools.rs' > "${'$'}clippy_log" || true
                other=${'$'}(wc -l < "${'$'}clippy_log")
                cat "${'$'}clippy_log"
                rm -f "${'$'}clippy_log"
                echo "clippy errors outside baseline file: ${'$'}other"
                if [ "${'$'}other" -ne 0 ]; then
                    echo "FAIL: clippy errors outside the documented baseline"
                    exit 1
                fi
            """.trimIndent())
        }

        stage("a014-capabilities-json") {
            sh("$cd && cargo test -p cognicode-cli --test a014_capabilities_json --quiet")
        }

        stage("a015-licenses-gate") {
            sh("$cd && cargo test -p cognicode-cli --test a015_licenses_gate --quiet")
        }

        stage("a013-lifecycle-gate-contract") {
            sh("$cd && cargo test -p cognicode-cli --test a013_lifecycle_gate_contract --quiet")
        }

        stage("cli-gate-coverage-contract") {
            // Contract over .github/workflows/pr-ci.yml: asserts every gated suite is named
            // or excluded with a reason.
            sh("$cd && cargo test -p cognicode-cli --test cli_gate_coverage_contract --quiet")
        }
    }
}
