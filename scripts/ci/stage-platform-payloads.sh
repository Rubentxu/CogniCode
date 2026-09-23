#!/usr/bin/env bash
# Stage platform payloads from the per-lane download-artifact tree into
# the flat layout `cognicode-release generate` expects.
#
# Background: GitHub Actions `upload-artifact@v4` preserves the relative
# path inside `path:`. The build lanes upload `dist/*.tar.gz`, and
# `download-artifact@v4` with `merge-multiple: true` does not flatten
# the resulting tree. Concretely:
#
#   staging/payloads-<platform>/dist/<component>-<ver>-<plat>.tar.gz
#
# `cognicode-release generate` (release_factory.rs:164) scans staging/
# flat with `std::fs::read_dir(staging)` and only accepts .tar.gz at the
# root. The mismatch caused the v0.97.4 release job (Actions run
# 35874781973) to fail with:
#
#   missing artifact `cogh-0.97.4-x86_64-unknown-linux-gnu.tar.gz`:
#   component `cogh` is published but was not produced for platform `linux-x86-64`
#
# This script is the explicit bridge between those two contracts:
#
# 1. It refuses to start unless the only files present in the staging
#    tree are the `payloads-*` lane dirs (no stray READMEs, no
#    pre-existing flattened payloads that would silently overwrite
#    fresh ones, no orphan SBOMs at the root).
# 2. It scans each `payloads-<platform>/` dir for the canonical 3
#    component tarballs (cogh, cognicode, cognicode-mcp) plus the 3
#    matching CycloneDX SBOMs (crates/<component>-<platform>.cdx.json).
# 3. It refuses on duplicates (two lanes claiming the same filename)
#    or missing components for a platform.
# 4. It copies the artifacts to the staging root with their canonical
#    names. The lane subdirs are kept untouched (the smoke script and
#    `verify` can still read them as a cross-check if needed).
#
# Exit codes:
#   0  success; staging root now contains the canonical 6+3 files
#   1  invalid staging layout (missing lanes, missing components,
#      duplicates, unknown files at the root, etc.)
#   2  unexpected internal failure (mkdir/cp error)
#
# Usage: stage-platform-payloads.sh <staging-dir>

set -euo pipefail

STAGING="${1:-}"

if [[ -z "${STAGING}" ]]; then
  echo "::error::usage: $0 <staging-dir>" >&2
  exit 1
fi
if [[ ! -d "${STAGING}" ]]; then
  echo "::error::staging dir does not exist: ${STAGING}" >&2
  exit 1
fi

# Canonical contract for Tier-1 release factory.
TIER1_PLATFORMS=(x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu)
COMPONENTS=(cogh cognicode cognicode-mcp)

# Reject any file at the staging root other than the lane dirs and
# pre-staged portable skill bundle tarballs. This catches: stray
# READMEs/Markdown, accidental top-level uploads, or a previous
# flattened run that left orphan tarballs at the root. The downstream
# release factory would silently include such files in the SHA256SUMS
# and break reproducibility.
shopt -s nullglob
for entry in "${STAGING}"/*; do
  if [[ -f "${entry}" ]]; then
    fname="$(basename "${entry}")"
    case "${fname}" in
      # Skill bundles produced by `just bundle-skills` and staged by
      # the workflow before this script runs; pass through.
      *-*.tar.gz) ;;
      *)
        echo "::error::unexpected file at staging root: ${fname}" >&2
        echo "       only payloads-* lane directories and pre-staged skill bundles are allowed" >&2
        exit 1
        ;;
    esac
  fi
done

# Collect the lane directories exactly once.
LANES=()
for entry in "${STAGING}"/payloads-*; do
  if [[ -d "${entry}" ]]; then
    LANES+=("${entry}")
  fi
done

if (( ${#LANES[@]} == 0 )); then
  echo "::error::no payloads-* lane directories found in ${STAGING}" >&2
  echo "       expected at least one of:" >&2
  for p in "${TIER1_PLATFORMS[@]}"; do
    echo "         ${STAGING}/payloads-${p}/" >&2
  done
  exit 1
fi

# Helper: assert exactly one source per (component, platform) payload
# name. Two lanes claiming the same filename is a contract violation
# and we refuse silently picking either.
declare -A SEEN_PAYLOADS=()
declare -A SEEN_SBOMS=()

copy_unique() {
  local kind="$1" src="$2" dest_name="$3"
  local seen_key="${kind}:${dest_name}"
  if [[ -n "${SEEN_PAYLOADS[$seen_key]+x}" ]] || [[ -n "${SEEN_SBOMS[$seen_key]+x}" ]]; then
    echo "::error::duplicate ${kind} name '${dest_name}' from two lanes:" >&2
    echo "       previous: ${SEEN_PAYLOADS[$seen_key]:-}" >&2
    echo "       new     : ${src}" >&2
    exit 1
  fi
  if [[ "${kind}" == "payload" ]]; then
    SEEN_PAYLOADS[$seen_key]="${src}"
  else
    SEEN_SBOMS[$seen_key]="${src}"
  fi
  cp -f -- "${src}" "${STAGING}/${dest_name}"
}

# Walk each lane and copy the canonical payloads + SBOMs.
for lane in "${LANES[@]}"; do
  lane_name="$(basename "${lane}")"
  # lane_name == payloads-<platform>
  platform="${lane_name#payloads-}"
  if [[ "${platform}" == "${lane_name}" ]]; then
    echo "::error::lane dir does not match payloads-<platform> pattern: ${lane_name}" >&2
    exit 1
  fi
  dist_dir="${lane}/dist"
  crates_dir="${lane}/crates"
  if [[ ! -d "${dist_dir}" ]]; then
    echo "::error::lane ${lane_name} has no dist/ subdirectory" >&2
    exit 1
  fi

  # Require every component for the platform.
  for comp in "${COMPONENTS[@]}"; do
    # The script does not own the version; it just looks for the
    # single .tar.gz file under dist/ that begins with the component
    # stem and platform triple. The build lanes name them canonically
    # as <comp>-<ver>-<plat>.tar.gz.
    found_payload="$(find "${dist_dir}" -mindepth 1 -maxdepth 1 \
      -name "${comp}-*-${platform}.tar.gz" -print -quit || true)"
    if [[ -z "${found_payload}" ]]; then
      echo "::error::lane ${lane_name} missing payload for component ${comp} on ${platform}" >&2
      exit 1
    fi
    payload_basename="$(basename "${found_payload}")"
    copy_unique payload "${found_payload}" "${payload_basename}"

    sbom_name="${comp}-${platform}.cdx.json"
    sbom_src="${crates_dir}/${sbom_name}"
    if [[ ! -f "${sbom_src}" ]]; then
      echo "::error::lane ${lane_name} missing SBOM ${sbom_name}" >&2
      exit 1
    fi
    copy_unique sbom "${sbom_src}" "${sbom_name}"
  done
done

# Final sanity: every expected (component, platform) pair must be
# represented at the staging root. We rely on the released components
# enumeration being the same set as COMPONENTS above; if the contract
# diverges, the release_factory's build_inventory will fail loudly.
for platform in "${TIER1_PLATFORMS[@]}"; do
  for comp in "${COMPONENTS[@]}"; do
    if ! compgen -G "${STAGING}/${comp}-*-${platform}.tar.gz" > /dev/null; then
      echo "::error::after flatten, missing ${comp}-*-${platform}.tar.gz at staging root" >&2
      exit 1
    fi
  done
done

echo "stage-platform-payloads: OK  lanes=${#LANES[@]} platforms=${TIER1_PLATFORMS[*]} components=${COMPONENTS[*]}"
