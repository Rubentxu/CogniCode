#!/usr/bin/env bash
# Verify -- or explicitly decline to verify -- build provenance for release
# artifacts.
#
# WHY THIS IS A SCRIPT AND NOT A STAGE BODY
# ------------------------------------------
# `release.pipeline.kts` carried two stages that each half-implemented one
# policy. `provenance/attestations` ran `gh attestation verify` over every
# archive unconditionally. `provenance/provenance-required` implemented the
# documented `RELEASE_REQUIRE_PROVENANCE` switch. The header described the
# second; the code did the first.
#
# Those two disagree in the one direction that matters:
#
#   * the header promised the stage "fails closed when
#     RELEASE_REQUIRE_PROVENANCE=1 and no attestation exists", and that
#     "publishing runs without generated provenance" otherwise;
#   * the code could not publish at all, because the unconditional loop
#     returned non-zero on every candidate.
#
# Nothing generates an attestation in this repository -- `attest-build-provenance`
# was a GitHub Action and PipelineK has no action runtime -- so the measured
# behaviour was `gh attestation verify` exiting 1 with HTTP 404 for every file.
# The release was unreachable, and the header said it was not.
#
# One implementation, called from one stage, is the only way the header and the
# gate can stop disagreeing. A second stage reading the same variable is a
# second source of truth, which is how the disagreement started.
#
# THE POLICY
# ----------
#   RELEASE_REQUIRE_PROVENANCE=1   every file is verified; any file without a
#                                  verified attestation is fatal
#   unset, or anything else        nothing is fatal; what was found is reported
#
# Reporting is not skipping. The verification is still run in both modes, so a
# release published without provenance says so on the lane's own output instead
# of by silence.
#
# A file that does not exist is fatal in both modes: a gate asked to verify
# something that is not there was invoked wrong, and that is not a question
# about provenance.
#
# Usage: verify-provenance.sh <owner/name> <file>...

set -uo pipefail

usage() {
    printf 'usage: %s <owner/name> <file>...\n' "$(basename "$0")" >&2
    exit 2
}

if [ "$#" -lt 2 ]; then
    usage
fi

repo="$1"
shift

enforce=0
if [ "${RELEASE_REQUIRE_PROVENANCE:-0}" = "1" ]; then
    enforce=1
fi

if [ "$enforce" -eq 1 ]; then
    echo "provenance ENFORCED: every artifact must carry a verified attestation"
else
    echo "provenance NOT enforced for this run (RELEASE_REQUIRE_PROVENANCE != 1)."
    echo "  Each artifact is still checked, and the result is reported below."
    echo "  A missing attestation will not stop this release. That is a choice,"
    echo "  not a passing check: publish without provenance knowingly."
fi

missing=0
unattested=0

for artifact in "$@"; do
    if [ ! -f "$artifact" ]; then
        printf '  MISSING FILE  %s\n' "$artifact"
        missing=1
        continue
    fi
    if output=$(gh attestation verify "$artifact" --repo "$repo" 2>&1); then
        printf '  VERIFIED      %s\n' "$artifact"
    else
        printf '  NO PROVENANCE %s\n' "$artifact"
        # La salida de `gh` es la unica evidencia de por que no verifico; sin
        # ella este gate seria una afirmacion sin respaldo.
        printf '%s\n' "$output" | sed 's/^/      /'
        unattested=1
    fi
done

if [ "$missing" -ne 0 ]; then
    echo "FAIL: an artifact this gate was asked to verify does not exist."
    echo "      That is a broken invocation, not a provenance verdict."
    exit 1
fi

if [ "$unattested" -ne 0 ] && [ "$enforce" -eq 1 ]; then
    echo "FAIL: provenance is enforced and at least one artifact has none."
    echo "      No stage in this lane can generate one: the generator was a"
    echo "      GitHub Action. Choose a replacement before enforcing this."
    exit 1
fi

if [ "$unattested" -ne 0 ]; then
    echo "WARNING: this release ships without provenance on the artifacts above."
    echo "         `gh attestation verify` is the consumer-facing half; it will"
    echo "         fail for anyone who checks. See R2 in the roadmap."
fi

exit 0
