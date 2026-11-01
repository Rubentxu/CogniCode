// impact-gate — the lane behind the `merge-gate` status when a change has a
// bounded blast radius.
//
// It has no stages of its own to keep in step. `scripts/ci/impact.py run`
// reads `merge-gate.pipeline.kts`, keeps the stages that the change can
// affect, and runs them in rings so a red arrives early:
//
//   ring 0  fmt, clippy of the affected crates, every contract
//   ring 1  the crates that changed (core: only the modules that import the
//           changed ones, and the integration tests that import those), plus
//           any gate stage that changed or runs a script that changed
//   ring 2  the crates that depend on them
//
// Which lane runs is decided by `impact.py tier` in `publish-merge-gate.sh`;
// the published status names the lane, so a bounded verdict is never read as
// a full one. The full gate still runs on `main` every night and before every
// tag: that run is what catches a rule here that selected too little.
//
//   pipelinek run --db .pipelinek/impact-gate.db impact-gate.pipeline.kts
import java.io.File

val repoRoot: String = File(".").canonicalPath
val cd = "cd \"$repoRoot\""

pipeline {
    stages {
        stage("plan") {
            // The plan, printed before any work: what changed, what it reaches,
            // and which stages will run and why. Refuses a change that is not
            // of bounded impact, whoever launched the lane.
            stage("impact-plan") {
                sh("""
                    set -eo pipefail
                    $cd || exit 1
                    tier=${'$'}(python3 scripts/ci/impact.py tier --sha HEAD --base origin/main)
                    if [ "${'$'}tier" != "impact" ]; then
                        echo "FAIL: impact.py says '${'$'}tier'; this lane only judges changes of bounded impact."
                        exit 1
                    fi
                    python3 scripts/ci/impact.py plan --sha HEAD --base origin/main
                """.trimIndent())
            }
        }

        stage("rings") {
            stage("impact-run") {
                sh("$cd && python3 scripts/ci/impact.py run --sha HEAD --base origin/main")
            }
        }
    }
}
