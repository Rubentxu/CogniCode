#!/usr/bin/env bash
# Probe: what does PipelineK actually do, that GitHub Actions used to do for us?
#
# Two properties of the orchestrator were assumed rather than measured while the
# CI/CD cutover was under way, and both of them are load-bearing: the first is
# why QW-09 was re-expressed instead of translated, and the second is why
# `blocking` still blocks. A header in `merge-gate.pipeline.kts` asserted the
# opposite of the first one and was wrong until 2026-10-02.
#
# This script measures both and compares them against the recorded expectation.
# It is not a gate: it needs the `pipelinek` binary, which a CI runner does not
# have, and the GitHub Actions runners that do have it are the thing being
# retired. Run it by hand after upgrading PipelineK, and treat a changed answer
# as a change of policy that needs reviewing rather than a version bump.
#
#   bash scripts/ci/probe-pipelinek-semantics.sh
#
# Measured 2026-10-02 under pipelinek 0.46.0.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

if ! command -v pipelinek >/dev/null 2>&1; then
    cat >&2 <<'EOF'
ERROR: the `pipelinek` binary is not on PATH.

    asdf install pipelinek 0.46.0

This probe is not part of the contract suite on purpose: the merge gate runs on
a GitHub Actions runner that does not ship PipelineK, and the workflows that
do are the ones being retired. See scripts/ci/pipeline_authority.py.
EOF
    exit 2
fi

# `pipelinek --version` is not a subcommand: the CLI is
# `pipeline <validate|run> ...` and rejects the flag. The version is asked of
# the installer that put the binary on PATH, and "unknown" is a fine answer.
VERSION="$(asdf current pipelinek 2>/dev/null | sed 's/^[[:space:]]*//' || true)"
echo "pipelinek: ${VERSION:-version unknown (the CLI has no --version)}"

failures=0

# ---------------------------------------------------------------------------
# Probe 1 — do stages share a filesystem and a working directory?
#
# The original header of merge-gate.pipeline.kts claimed every stage got a
# throwaway directory, and a whole justification for interpolating the repo
# root into every step rested on that. It was false.
# ---------------------------------------------------------------------------
write_probe() {
    cat >"$WORK/$1" <<EOF
pipeline {
    stages {
        stage("writer") {
            sh("pwd > pwd-writer.txt && echo hello > shared.txt && echo WROTE")
        }
        stage("reader") {
            sh("pwd > pwd-reader.txt && test -f shared.txt && echo SHARED || echo NOT_SHARED")
        }
    }
}
EOF
}

write_probe share.pipeline.kts
share_out="$(cd "$WORK" && pipelinek run share.pipeline.kts 2>&1 || true)"

writer_pwd="$(cat "$WORK/pwd-writer.txt" 2>/dev/null || echo '<missing>')"
reader_pwd="$(cat "$WORK/pwd-reader.txt" 2>/dev/null || echo '<missing>')"

echo
echo "probe 1 — filesystem and cwd sharing"
echo "  writer pwd : $writer_pwd"
echo "  reader pwd : $reader_pwd"

if grep -q 'SHARED' <<<"$share_out" && ! grep -q 'NOT_SHARED' <<<"$share_out"; then
    echo "  RESULT: shared, as recorded"
else
    echo "  RESULT: CHANGED — stages no longer share a filesystem."
    echo "          QW-09's ordering invariant is no longer sufficient; the"
    echo "          reachability property has to be re-derived for whatever"
    echo "          isolation model this version has."
    failures=$((failures + 1))
fi

if [ "$writer_pwd" = "$reader_pwd" ]; then
    echo "  cwd: identical, as recorded"
else
    echo "  cwd: CHANGED — stages no longer share a working directory."
    failures=$((failures + 1))
fi

# ---------------------------------------------------------------------------
# Probe 2 — does a failing stage abort the pipeline?
#
# This is what `set -euo pipefail` gave the old workflow, implicitly. It is the
# whole reason `blocking` still blocks, and the reason every ADVISORY stage in
# integration.pipeline.kts ends its command with `|| echo`: PipelineK has no
# `continue-on-error`, so a non-zero exit is the only way to stop a lane, and
# the only way not to stop it is to not exit non-zero.
# ---------------------------------------------------------------------------
cat >"$WORK/abort.pipeline.kts" <<'EOF'
pipeline {
    stages {
        stage("probe-write-before") {
            sh("echo before > probe-before.txt && echo WROTE")
        }
        stage("probe-fail") {
            sh("echo 'failing on purpose' && exit 7")
        }
        stage("probe-write-after") {
            sh("echo after > probe-after.txt && echo WROTE")
        }
    }
}
EOF

set +e
abort_out="$(cd "$WORK" && pipelinek run abort.pipeline.kts 2>&1)"
abort_code=$?
set -e

echo
echo "probe 2 — failure propagation"
echo "  exit code: $abort_code"
echo "  stage 3 ran: $([ -f "$WORK/probe-after.txt" ] && echo yes || echo no)"

if [ "$abort_code" -ne 0 ] && [ ! -f "$WORK/probe-after.txt" ]; then
    echo "  RESULT: a failing stage aborts the pipeline, as recorded."
    echo "          A trailing \"|| echo ADVISORY: ...\" is therefore the only way"
    echo "          to express advisory here, and a blocking gate cannot be"
    echo "          skipped by a later stage that happens to pass."
else
    echo "  RESULT: CHANGED — a failing stage no longer aborts the pipeline."
    echo "          Every stage after a failure now runs, which means every"
    echo "          blocking gate in every lane has to be re-checked for whether"
    echo "          it still fails the run on its own."
    failures=$((failures + 1))
fi

echo
if [ "$failures" -eq 0 ]; then
    echo "PASS — PipelineK behaves as this repository now assumes."
    exit 0
fi
echo "FAIL — $failures recorded expectation(s) no longer hold. Read the change as"
echo "       a policy change, not as a version bump."
exit 1
