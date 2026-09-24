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

# Canonical contract for Tier-1 release factory. The `*_TRIPLE` is the
# Rust target triple that appears in canonical tarball and SBOM names
# (matches `platform_token(Platform)` in crates/cognicode-cli/src/cmd/
# release_contract.rs:84). The `SHORT_*` value is the lane directory
# suffix produced by both `release.yml` and `release-validate.yml`
# (`payloads-${{ matrix.platform }}`, where `matrix.platform` is the
# short identifier like `linux-x86-64`).
declare -A PLATFORM_SHORT_TO_TRIPLE=(
  ["linux-x86-64"]="x86_64-unknown-linux-gnu"
  ["linux-aarch64"]="aarch64-unknown-linux-gnu"
)
TIER1_TRIPLES=(x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu)
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
  for p in "${TIER1_TRIPLES[@]}"; do
    echo "         ${STAGING}/payloads-${p}/  (or its short alias payloads-linux-<arch>64/)" >&2
  done
  exit 1
fi

# Helper: assert exactly one source per (component, platform) payload
# name. Two lanes claiming the same filename is a contract violation
# and we refuse silently picking either.
declare -A SEEN_PAYLOADS=()
declare -A SEEN_SBOMS=()
# Track which lane directory maps to which target triple so we can
# reject two different alias dirs claiming the same platform.
declare -A TRIPLE_OWNED_BY=()

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
  # lane_name == payloads-<suffix>. The suffix may be either the short
  # platform identifier (linux-x86-64 / linux-aarch64) that the
  # workflows upload, or the canonical target triple as an explicit
  # alias for tests/fixtures that build the layout by hand. Anything
  # else is rejected so unknown platforms never reach the release
  # factory.
  suffix="${lane_name#payloads-}"
  if [[ "${suffix}" == "${lane_name}" ]]; then
    echo "::error::lane dir does not match payloads-<suffix> pattern: ${lane_name}" >&2
    exit 1
  fi
  if [[ -n "${PLATFORM_SHORT_TO_TRIPLE[$suffix]+x}" ]]; then
    platform="${PLATFORM_SHORT_TO_TRIPLE[$suffix]}"
    lane_alias_kind="short"
  elif [[ " ${TIER1_TRIPLES[*]} " == *" ${suffix} "* ]]; then
    platform="${suffix}"
    lane_alias_kind="triple"
  else
    echo "::error::lane ${lane_name} uses unknown platform suffix '${suffix}'" >&2
    echo "       accepted short identifiers:" >&2
    for s in "${!PLATFORM_SHORT_TO_TRIPLE[@]}"; do
      echo "         payloads-${s}" >&2
    done
    echo "       accepted target-triple aliases:" >&2
    for t in "${TIER1_TRIPLES[@]}"; do
      echo "         payloads-${t}" >&2
    done
    exit 1
  fi

  # Reject two lanes claiming the same target triple. Either two
  # short-form lanes (e.g. both `payloads-linux-x86-64`) or a short
  # lane plus its triple alias (e.g. `payloads-linux-x86-64` and
  # `payloads-x86_64-unknown-linux-gnu`) is forbidden — the result
  # would be ambiguous regardless of which one we kept.
  if [[ -n "${TRIPLE_OWNED_BY[$platform]+x}" ]]; then
    echo "::error::two lane directories claim the same platform ${platform}:" >&2
    echo "       previous: ${TRIPLE_OWNED_BY[$platform]} (${lane_alias_kind})" >&2
    echo "       new     : ${lane_name} (${lane_alias_kind})" >&2
    exit 1
  fi
  TRIPLE_OWNED_BY[$platform]="${lane_name}"

  dist_dir="${lane}/dist"
  crates_dir="${lane}/crates"
  if [[ ! -d "${dist_dir}" ]]; then
    echo "::error::lane ${lane_name} has no dist/ subdirectory" >&2
    exit 1
  fi

  # Require every component for the platform. Tarballs are named with
  # the canonical target triple (matches `cognicode-release name
  # --platform <short> --version <v>` output in release_contract.rs).
  for comp in "${COMPONENTS[@]}"; do
    found_payload="$(find "${dist_dir}" -mindepth 1 -maxdepth 1 \
      -name "${comp}-*-${platform}.tar.gz" -print -quit || true)"
    if [[ -z "${found_payload}" ]]; then
      echo "::error::lane ${lane_name} (platform ${platform}) missing payload for component ${comp}" >&2
      echo "       expected: ${dist_dir}/${comp}-*-${platform}.tar.gz" >&2
      exit 1
    fi
    payload_basename="$(basename "${found_payload}")"
    # Defensive: the tarball's embedded triple must agree with the
    # resolved lane platform. A lane named `payloads-linux-x86-64`
    # cannot ship an `aarch64-...` tarball. The find above already
    # restricts the glob to `${platform}` so the basename is bounded
    # by construction; this regex is a belt-and-suspenders check
    # against accidental wildcards or symbolic links.
    if ! [[ "${payload_basename}" =~ ^${comp}-.+-${platform}\.tar\.gz$ ]]; then
      echo "::error::lane ${lane_name} (platform ${platform}) ships payload '${payload_basename}' whose embedded triple does not match the lane platform" >&2
      exit 1
    fi
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

# Final sanity: every Tier-1 platform must be represented at the
# staging root (not just any non-zero number of lanes). This catches
# the case where only one lane was uploaded and the other was
# silently skipped.
for platform in "${TIER1_TRIPLES[@]}"; do
  if [[ -z "${TRIPLE_OWNED_BY[$platform]+x}" ]]; then
    echo "::error::after flatten, no lane claimed Tier-1 platform ${platform}" >&2
    echo "       present lanes owned: ${!TRIPLE_OWNED_BY[*]:-(none)}" >&2
    exit 1
  fi
  for comp in "${COMPONENTS[@]}"; do
    if ! compgen -G "${STAGING}/${comp}-*-${platform}.tar.gz" > /dev/null; then
      echo "::error::after flatten, missing ${comp}-*-${platform}.tar.gz at staging root" >&2
      exit 1
    fi
  done
done

echo "stage-platform-payloads: OK  lanes=${#LANES[@]} platforms=${TIER1_TRIPLES[*]} components=${COMPONENTS[*]}"
