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
// Run from the repository root, with the script path relative to it: every sh() step inherits
// the working directory of the process that launches the CLI, so the steps below run inside
// the cargo workspace. Invoking the CLI from anywhere else makes them fail on a missing
// Cargo.toml rather than silently testing nothing.
//
//   pipelinek run product-fast.pipeline.kts --db <journal.db>
//
// clippy has a documented baseline: 42 pre-existing errors in
// crates/cognicode-core/src/interface/rig/tools.rs. This gate fails on any error outside
// that file, so a new lint cannot hide behind the baseline.
pipeline {
    stages {
        stage("toolchain") {
            sh("cargo --version && rustc --version")
        }

        stage("repo-contract") {
            // Anti-vacuity: prove the steps below really run inside the repository.
            sh("""
                test -f Cargo.toml || { echo "FAIL: steps are not running in the repo workspace"; exit 1; }
                echo "workspace=$(pwd) crates=$(ls -d crates/*/ | wc -l)"
            """)
        }

        stage("fmt") {
            sh("cargo fmt --all -- --check")
        }

        stage("clippy-baseline") {
            sh("""
                other=$(cargo clippy --workspace --all-targets --all-features -- -D warnings 2>&1 \
                        | grep -E '^[[:space:]]*-->' \
                        | grep -v 'crates/cognicode-core/src/interface/rig/tools.rs' | wc -l)
                echo "clippy errors outside baseline file: ${'$'}other"
                if [ "${'$'}other" -ne 0 ]; then
                    echo "FAIL: clippy errors outside the documented baseline"
                    exit 1
                fi
            """)
        }

        stage("a014-capabilities-json") {
            sh("cargo test -p cognicode-cli --test a014_capabilities_json --quiet")
        }

        stage("a015-licenses-gate") {
            sh("cargo test -p cognicode-cli --test a015_licenses_gate --quiet")
        }

        stage("a013-lifecycle-gate-contract") {
            sh("cargo test -p cognicode-cli --test a013_lifecycle_gate_contract --quiet")
        }

        stage("cli-gate-coverage-contract") {
            // Contract over .github/workflows/pr-ci.yml: asserts every gated suite is named
            // or excluded with a reason.
            sh("cargo test -p cognicode-cli --test cli_gate_coverage_contract --quiet")
        }
    }
}
