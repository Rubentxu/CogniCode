// certification — what is too expensive for a change, and too important to skip.
//
// The last lane. It owns the gates that are right to run rarely and wrong to
// skip: the sandbox campaigns, the T6 regression rule, the performance verdict,
// and the scorecards that measure whether the rare runs are getting worse.
//
// IT IS NOT INTEGRATION, AND IT DOES NOT CALL IT
// ------------------------------------------------
// `integration.pipeline.kts` already runs the full workspace, the feature
// matrix, the coverage gate and the portable builds. This lane does not repeat
// them. Where a gate genuinely needs an independent clean run — the sandbox
// campaigns build containers and read-only fixtures from scratch — it runs its
// own; everything else is either here because it is expensive or here because
// it is missing a lane, and the header of each stage says which.
//
// A MEASURED CORRECTION TO A DIAGNOSIS THAT WAS WRONG
// ---------------------------------------------------
// `sandbox-nightly.yml` has been red on purpose since 2026-10-01, and its two
// steps said why. One reason was that `rootful/setup-podman` had been deleted
// from GitHub. The other was that the lane recipes "live in sandbox/justfile
// and do not resolve from the workspace root".
//
// Both were treated as fact and neither survives a measurement:
//
//   * podman 5.8.7 is installed and rootless on the machine this lane runs on.
//     The action being gone is a fact about a hosted runner, and this is not a
//     hosted runner. That is the whole reason to move off Actions.
//   * the recipes do resolve. `just --justfile sandbox/justfile sandbox-pull`
//     lists and runs. They failed because the workflow called `just
//     sandbox-pull` from the root, where a plain `import` collides on the
//     `build` recipe. Passing the justfile explicitly avoids the collision
//     without renaming a recipe in either file.
//
// So a gate that has been failing on a stated reason for a week is wired here
// with the reason removed rather than carried over. `sandbox-preconditions`
// re-checks both facts and fails if either stops holding, so the lane cannot
// drift back into a red run whose message is a guess.
//
// ADVISORY IS `|| echo`, AND HERE THAT IS THE ONLY OPTION
// ------------------------------------------------------
// Measured: a failing PipelineK stage aborts the run
// (`scripts/ci/probe-pipelinek-semantics.sh`). PipelineK has no
// `continue-on-error`, so a stage that must not fail has to end in something
// that does not fail. Each ADVISORY stage below says what it observed, and
// the reason it does not stop the lane.
//
// The performance verdict is the sharpest case. `perf-budget.toml` budgets 16
// operations and the checker was run against the real ones on 2026-10-02:
// 7 are measured and all 7 are inside budget (`shortest_path` at 299 µs
// against 1000 µs is the tightest, 3.3x of headroom; the loosest is
// `subgraph_extraction_50_nodes` at 72x), and 9 have no benchmark anywhere in
// `crates/*/benches/`. So the checker exits 3 UNMEASURED and cannot return 0,
// and it runs as ADVISORY, saying exactly that.
//
// Splitting exit 1 from exit 3 — fail the lane on a measured regression, report
// the unmeasured ones and continue — is the obvious next step and is NOT done
// here, because the 3.3x figure is a measurement of one development machine and
// not of a CI runner, and a gate whose headroom is unknown on the hardware that
// enforces it is a lane that turns red for a reason nobody can act on. It needs
// the runner measured, or the 9 entries benchmarked or deleted, whichever
// happens first. A benchmark that produces numbers is not a verdict, and
// pretending the second exists is how the first one gets mistaken for it.

import java.io.File

val repoRoot: String = File(".").canonicalPath
val cd = "cd \"$repoRoot\""
val sandboxJust = "just --justfile sandbox/justfile"

pipeline {
    stages {
        // -------------------------------------------------------------- t6 rule
        stage("regression-discipline") {
            // Every `fix(*)` commit in the range must carry a test with it.
            // Pure git introspection, no toolchain, so it costs nothing to run
            // here even though it is a merge-gate-shaped rule.
            stage("t6-regression-test") {
                sh("$cd && bash scripts/ci/check_regression_test.sh")
            }
        }

        // -------------------------------------------------------------- sandbox
        stage("sandbox") {
            // Both facts the workflow got wrong, checked so the lane cannot
            // quietly become the red-on-purpose run it replaced.
            stage("sandbox-preconditions") {
                sh("""
                    $cd || exit 1
                    command -v podman >/dev/null 2>&1 || {
                        echo "podman is not installed."
                        echo "  The sandbox campaigns need it; without it the stages below"
                        echo "  would report a container failure that looks like a product failure."
                        exit 1
                    }
                    podman info --format '{{.Host.Security.Rootless}}' 2>/dev/null | grep -qx true || {
                        echo "podman is present but not rootless."
                        echo "  The container definitions bind-mount host paths that a rootful"
                        echo "  daemon resolves differently, so a green run here would not mean"
                        echo "  what the scenarios say it means."
                        exit 1
                    }
                    $sandboxJust --list >/dev/null 2>&1 || {
                        echo "sandbox/justfile does not list its recipes. The lanes are"
                        echo "unreachable and the steps below would fail on a missing recipe"
                        echo "rather than on a sandbox defect."
                        exit 1
                    }
                    echo "podman $(podman --version | head -1), rootless, sandbox recipes resolvable"
                """.trimIndent())
            }

            // Advisory: image digests move upstream, and a registry that changed
            // is not a sandbox regression.
            stage("sandbox-pull") {
                sh("$cd && $sandboxJust sandbox-pull || echo 'ADVISORY: sandbox-pull reported failures (upstream digests move; this is not a sandbox regression)'")
            }

            // Advisory, as in the workflow. The setup that follows it is what
            // the smoke run actually exercises.
            stage("sandbox-setup") {
                sh("$cd && $sandboxJust sandbox-setup || echo 'ADVISORY: sandbox-setup reported failures'")
            }

            stage("sandbox-ci-smoke") {
                sh("$cd && $sandboxJust sandbox-ci-smoke")
            }

            // Advisory: these classify expected failures by capability, and the
            // classification is informational about the host, not a verdict on
            // the product.
            stage("sandbox-ci-probe") {
                sh("$cd && $sandboxJust sandbox-ci-probe || echo 'ADVISORY: the capability probes reported failures'")
            }
        }

        // ---------------------------------------------------------- performance
        stage("performance") {
            // The verdict, not the measurement. `cargo bench` producing numbers
            // lives in integration as ADVISORY; deciding against a budget is a
            // different act, and it is the one that belongs here.
            //
            // It cannot be blocking yet. Measured 2026-10-02: 9 of the 16
            // budgeted operations have no benchmark and return exit 3
            // UNMEASURED, so the checker cannot return 0 and wiring it as a
            // gate would make this lane permanently red, which teaches everyone
            // to ignore it — the exact outcome `perf-budget.toml`'s own
            // `ENFORCEMENT: none` records honestly. The header says what would
            // have to be measured first.
            stage("perf-budget-verdict") {
                sh("""
                    $cd || exit 1
                    set +e
                    out=$(bash scripts/perf-budget-check.sh 2>&1)
                    rc=${'$'}?
                    set -e
                    echo "${'$'}out"
                    if [ "${'$'}rc" -eq 0 ]; then
                        echo "every budgeted operation is measured and inside its budget"
                    else
                        echo "ADVISORY: the performance verdict is exit ${'$'}rc, not a PASS."
                        echo "  0 = every budgeted operation measured and inside budget"
                        echo "  1 = a measured operation exceeded its budget"
                        echo "  3 = a budgeted operation has no measurement at all"
                        echo "  3 is what this repository returns today, because 9 of the 16"
                        echo "  budgeted operations have no benchmark. That is a fact about"
                        echo "  perf-budget.toml, not about this run, and it is why the stage is"
                        echo "  advisory rather than a gate."
                    fi
                """.trimIndent())
            }
        }

        // ------------------------------------------------------------ scorecards
        // The nightly cadence `sandbox-nightly.yml` claims for itself and does
        // not run. It is here because it is the thing that measures whether the
        // rare runs above are getting worse, which is the reason to run them
        // rarely at all.
        stage("scorecard-nightly") {
            sh("""
                $cd || exit 1
                if [ -z "${'$'}{CERTIFICATION_SCORECARD:-}" ]; then
                    echo "CERTIFICATION_SCORECARD is unset; skipping the scorecard campaign."
                    echo "  It rewrites sandbox/results/ from a window of past runs, so it is"
                    echo "  opt-in rather than a side effect of running this lane."
                    exit 0
                fi
                just scorecard-nightly
            """.trimIndent())
        }

        stage("scorecard-streak") {
            sh("""
                $cd || exit 1
                if [ -z "${'$'}{CERTIFICATION_SCORECARD:-}" ]; then
                    echo "CERTIFICATION_SCORECARD is unset; skipping the streak counter."
                    exit 0
                fi
                just scorecard-streak
            """.trimIndent())
        }
    }
}
