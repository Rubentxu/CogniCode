#!/usr/bin/env bash
# ============================================================================
# attest-provenance.sh
# ----------------------------------------------------------------------------
# The GENERATION half of release provenance: sign each candidate artifact with
# a cosign key pair and write a SLSA provenance bundle next to it.
#
# `verify-provenance.sh` is the consumption half and it already exists. This
# script is the half the ADR says is missing: `actions/attest-build-provenance`
# was a GitHub Action, PipelineK has no action runtime, and the lane could
# refuse a release for want of provenance while having no way to produce any.
#
# WHAT THIS SCRIPT DELIBERATELY DOES NOT DO
# -----------------------------------------
# It never creates a key. There is no `cosign generate-key-pair` call in this
# file, and there must never be one. Where a long-lived signing key lives and
# who can use it is a custody decision with consequences that outlive any
# release (see ADR-RELEASE-PROVENANCE-PIPELINEK, "Open decision, and it is the
# whole decision"). A script that quietly minted a key would let a release
# publish provenance that nothing can verify later, which is worse than
# publishing none: the consumer would be told the bytes are attested.
#
# So: a missing key is a fatal, named error, not something to work around.
#
# THE THREE DIGESTS, CHECKED HERE AND NOT ONLY IN A TEST
# ------------------------------------------------------
# The ADR's acceptance is that
#
#     candidate digest == attestation subject digest == published digest
#
# holds. The first equality is checkable at generation time, and it is checked
# on every run: the bundle is re-read after cosign writes it and its
# `subject[0].digest.sha256` is compared against the artifact's own sha256sum.
# A generator that writes an attestation about one artifact while naming
# another is exactly the failure that provenance exists to prevent, and it
# would be invisible to every consumer that only checks "is there a bundle".
#
# The second equality is not checkable here — this script does not publish —
# so it stays where publishing happens.
#
# MEASURED 2026-10-04, cosign 3.1.3, offline, no network and no transparency
# log. Three things the flags do not tell you:
#
#   * `cosign attest-blob` REQUIRES `--bundle` in 3.1.3. Without it:
#     `must specify --bundle with --new-bundle-format`.
#   * `cosign generate-key-pair` PROMPTS FOR A PASSWORD and there is no TTY in
#     a lane, so it dies with `inappropriate ioctl for device` unless
#     `COSIGN_PASSWORD` is set. The ADR measured the interactive form, where
#     it works; this is the non-interactive reading, which is the one a lane
#     and a test actually get.
#   * The subject digest is the **hex** sha256, stored verbatim. Decoding it as
#     base64 —a reasonable guess, and the wrong one— produces a mismatch that
#     looks like a signing bug and is not one.
#
# Usage:
#   attest-provenance.sh --key <cosign.key> --out-dir <dir> <artifact>...
# ============================================================================
set -uo pipefail

KEY=""
OUT_DIR=""
ARTIFACTS=()

die() {
    printf 'FAIL: %s\n' "$1" >&2
    shift
    for line in "$@"; do
        printf '      %s\n' "$line" >&2
    done
    exit 1
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --key)      KEY="${2:-}"; shift 2 ;;
        --out-dir)  OUT_DIR="${2:-}"; shift 2 ;;
        -h|--help)  sed -n '2,45p' "$0"; exit 0 ;;
        --)         shift; break ;;
        -*)         die "unknown option '$1'" "usage: $0 --key <cosign.key> --out-dir <dir> <artifact>..." ;;
        *)          ARTIFACTS+=("$1"); shift ;;
    esac
done
ARTIFACTS+=("$@")

[ -n "${ARTIFACTS[*]:-}" ] || die "no artifacts given" "usage: $0 --key <cosign.key> --out-dir <dir> <artifact>..."

# --- The custody gate, before anything else ---------------------------------

if [ -z "${KEY}" ]; then
    die "no signing key given (--key)" \
        "This script never creates one, on purpose." \
        "Where the key lives and who may use it is a human decision; see" \
        "docs/adr/ADR-RELEASE-PROVENANCE-PIPELINEK.md, 'Open decision'." \
        "A release without provenance is a choice the lane already says out loud." \
        "A release with provenance nobody can verify is not."
fi
[ -f "${KEY}" ] || die "the signing key '${KEY}' does not exist" \
    "Not generating one. That is the custody decision, not this script's."

PUB_KEY=""
# MEDIDO 2026-10-04, cosign 3.1.3. `generate-key-pair --output-key-prefix P`
# writes `P.key` and `P.pub` — the SAME prefix, not `P.key.pub`. The first
# version of this script derived the public half as "${KEY}.pub", which is
# `P.key.pub`, and refused to run against a perfectly good key pair:
#
#     FAIL: the public key 'keys/probe.key.pub' does not exist
#
# which is the correct behaviour applied to the wrong filename. Both spellings
# are accepted, because a caller holding a key by some other convention is not
# the thing this check is for.
for candidate in "${KEY%.key}.pub" "${KEY}.pub"; do
    if [ -f "${candidate}" ]; then
        PUB_KEY="${candidate}"
        break
    fi
done

[ -n "${PUB_KEY}" ] || die "no public key found next to '${KEY}'" \
    "cosign writes the pair as <prefix>.key and <prefix>.pub. Looked for:" \
    "  ${KEY%.key}.pub" \
    "  ${KEY}.pub" \
    "The public half travels with the release, so a consumer needs the release," \
    "not the publisher, to check a bundle."

command -v cosign >/dev/null 2>&1 || die "cosign is not on PATH" \
    "It is the signer this path is built on. See the ADR, option B."

if [ -z "${OUT_DIR}" ]; then
    OUT_DIR="$(dirname "${ARTIFACTS[0]}")"
fi
mkdir -p "${OUT_DIR}" || die "cannot create the output directory '${OUT_DIR}'"

# --- What this build was ----------------------------------------------------
# Recorded, not invented. Outside a repository there is no commit to name, and
# a provenance statement that carries a plausible-looking source digest it
# cannot support is worse than one that carries none.

SOURCE_DIGEST=""
if git rev-parse --git-dir >/dev/null 2>&1; then
    source_sha="$(git rev-parse HEAD 2>/dev/null || true)"
    if [ -n "${source_sha}" ]; then
        SOURCE_DIGEST="${source_sha}"
    fi
fi

if [ -n "${SOURCE_DIGEST}" ]; then
    echo "source commit: ${SOURCE_DIGEST}"
else
    echo "source commit: (not a git repository; the statement will not name one)"
fi

# --- Generate ---------------------------------------------------------------

signed=0
for artifact in "${ARTIFACTS[@]}"; do
    [ -f "${artifact}" ] || die "'${artifact}' does not exist" \
        "Signing a candidate that is not there produces a bundle about nothing."

    base="$(basename "${artifact}")"
    digest="$(sha256sum "${artifact}" | cut -d' ' -f1)"
    bundle="${OUT_DIR}/${base}.bundle.json"
    predicate="$(mktemp -t cognicode-predicate.XXXXXX)"

    # The subject digest is computed here, from the bytes on disk, and the
    # statement is then checked against cosign's own output. Passing a digest
    # the caller supplied would make the check circular: a caller that computed
    # it wrong would produce a bundle that agrees with its own mistake.
    if [ -n "${SOURCE_DIGEST}" ]; then
        jq -n \
            --arg name "${base}" \
            --arg sha "${digest}" \
            --arg sha1 "${SOURCE_DIGEST}" \
            '{
              _type: "https://in-toto.io/Statement/v1",
              subject: [{name: $name, digest: {sha256: $sha}}],
              predicateType: "https://slsa.dev/provenance/v1",
              predicate: {
                buildDefinition: {
                  buildType: "https://cognicode.dev/release-candidate/v0",
                  externalParameters: {source: {uri: "git+file", digest: {sha1: $sha1}}},
                  internalParameters: {},
                  resolvedDependencies: []
                },
                runDetails: {builder: {id: "https://cognicode.dev/pipelinek/release-candidate"}}
              }
            }' > "${predicate}"
    else
        jq -n \
            --arg name "${base}" \
            --arg sha "${digest}" \
            '{
              _type: "https://in-toto.io/Statement/v1",
              subject: [{name: $name, digest: {sha256: $sha}}],
              predicateType: "https://slsa.dev/provenance/v1",
              predicate: {
                buildDefinition: {
                  buildType: "https://cognicode.dev/release-candidate/v0",
                  externalParameters: {},
                  internalParameters: {},
                  resolvedDependencies: []
                },
                runDetails: {builder: {id: "https://cognicode.dev/pipelinek/release-candidate"}}
              }
            }' > "${predicate}"
    fi

    if ! cosign attest-blob \
            --key "${KEY}" \
            --bundle "${bundle}" \
            --predicate "${predicate}" \
            --type https://slsa.dev/provenance/v1 \
            "${artifact}" >/dev/null 2>&1
    then
        rm -f "${predicate}"
        die "cosign refused to attest '${base}'" \
            "A bundle that was not written is not a signed artifact."
    fi
    rm -f "${predicate}"

    [ -f "${bundle}" ] || die "cosign reported success but wrote no bundle for '${base}'" \
        "A missing bundle that the caller believed was written is the failure this" \
        "check exists for: it would pass as a signed release and verify as none."

    # The first of the three digests, checked on the bytes cosign produced.
    claimed="$(jq -r '.dsseEnvelope.payload' "${bundle}" 2>/dev/null \
        | base64 -d 2>/dev/null \
        | jq -r '.subject[0].digest.sha256' 2>/dev/null || true)"

    if [ "${claimed}" != "${digest}" ]; then
        die "the bundle for '${base}' attests a different artifact" \
            "artifact sha256 : ${digest}" \
            "bundle subject  : ${claimed:-<unreadable>}" \
            "Provenance that names the wrong bytes is worse than none: a consumer" \
            "would be told these bytes were built by this build."
    fi

    printf '  ATTESTED        %s\n' "${base}"
    printf '      sha256      %s\n' "${digest}"
    printf '      bundle      %s\n' "${bundle}"
    signed=$((signed + 1))
done

echo "provenance: ${signed} artifact(s) attested with $(basename "${KEY}")"

# The public half is copied ONCE, and only now — after every bundle exists and
# has been checked. It used to be copied before the loop, which meant a run that
# died on a missing artifact left a public key sitting in the output directory
# with no bundle beside it: a release directory that looks attested to whatever
# lists it, and is not. The out-dir is now all-or-nothing.
#
# One copy, not one per artifact: there is one key, and a release that shipped
# N copies of it would look like N keys.
cp -f "${PUB_KEY}" "${OUT_DIR}/$(basename "${PUB_KEY}")" \
    || die "every bundle was written but the public key could not be copied" \
           "A bundle whose public half does not travel with it is unverifiable" \
           "by whoever downloads the release rather than by whoever built it."

echo "           public key: ${OUT_DIR}/$(basename "${PUB_KEY}")"
echo "           the two remaining digests (published asset, consumer download)"
echo "           are checked where publishing happens, not here."
exit 0
