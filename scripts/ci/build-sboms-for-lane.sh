#!/usr/bin/env bash
# Generate canonical CycloneDX SBOMs per published binary and stage
# them in the layout consumed by `stage-platform-payloads.sh`.
#
# Contract (must match what the flatten script expects):
#
#   crates/<component>-<rust-target-triple>.cdx.json
#
# where `<component>` is one of `cogh`, `cognicode`, `cognicode-mcp`
# (the `published_components()` set in
# `crates/cognicode-cli/src/cmd/release_contract.rs`) and
# `<rust-target-triple>` is e.g. `x86_64-unknown-linux-gnu`.
#
# Background:
#
# `cargo cyclonedx --format json` from the workspace root produces no
# output: the tool only writes a SBOM when invoked inside a single
# crate directory. Even from inside a crate it has two relevant
# `--describe` modes:
#
#   --describe crate       (default) one SBOM for the whole crate
#   --describe binaries    one SBOM per cargo `[[bin]]` target, named
#                          `<bin-name>_bin.cdx.json`
#
# The distribution contract demands a SBOM **per published binary**
# (one tarball ships one binary; one SBOM accompanies it). The
# per-crate SBOM conflates all the crate's bins and would attach a
# single SBOM file to multiple tarballs, breaking the per-payload
# pairing that `cognicode-release verify` enforces. Therefore:
#
#   - We invoke `--describe binaries` so each published bin gets its
#     own SBOM file (`cogh_bin.cdx.json`, `cognicode_bin.cdx.json`,
#     `cognicode-mcp_bin.cdx.json`).
#   - We rename those to the canonical layout
#     `<component>-<rust_target>.cdx.json` so the flatten script can
#     find them.
#   - We also rename any pre-existing per-crate `*.cdx.json` files
#     for the published crate, so a previous in-tree commit does not
#     survive into the upload.
#   - We fail loudly if any expected SBOM is missing after generation,
#     so a future drift in `cargo-cyclonedx` output naming cannot
#     silently ship a release with no SBOM.
#
# Side effects (relative to the workspace root):
#   - writes/overwrites `crates/<crate>/<bin>_bin.cdx.json` (cargo-cyclonedx output)
#   - writes the canonical SBOMs under `crates/<component>-<target>.cdx.json`
#   - removes stray per-bin SBOMs of non-published bins
#
# Usage: build-sboms-for-lane.sh <rust-target-triple> [<workspace-root>]
#   workspace-root defaults to $(pwd) and must be the CogniCode repo root.

set -euo pipefail

RUST_TARGET="${1:-}"
WORKSPACE_ROOT="${2:-$(pwd)}"

if [[ -z "${RUST_TARGET}" ]]; then
  echo "::error::usage: $0 <rust-target-triple> [<workspace-root>]" >&2
  exit 1
fi

# Crate and binary mapping for the published components. The
# `published_components()` set is the single source of truth in the
# release_contract; this table mirrors it explicitly so the build
# script can be inspected without compiling Rust.
#
# Format: <component-stem>|<crate-dir>|<bin-name>
COMPONENT_MAP=(
  "cogh|cognicode-cli|cogh"
  "cognicode|cognicode-cli|cognicode"
  "cognicode-mcp|cognicode-mcp|cognicode-mcp"
)

# Ensure cargo-cyclonedx is installed. The binary is not committed to
# the repo because installing it is the only way to pin its exact
# version per CI run.
if ! command -v cargo-cyclonedx >/dev/null 2>&1; then
  echo "::group::Installing cargo-cyclonedx"
  cargo install cargo-cyclonedx --locked
  echo "::endgroup::"
fi

# Clean up any stale per-crate SBOM files for the published crates,
# otherwise the upload would carry a stale `cognicode-cli.cdx.json`
# that the contract no longer recognises.
for entry in "${COMPONENT_MAP[@]}"; do
  IFS='|' read -r _stem crate_dir _bin <<<"${entry}"
  crate_root="${WORKSPACE_ROOT}/crates/${crate_dir}"
  if [[ -d "${crate_root}" ]]; then
    find "${crate_root}" -maxdepth 1 -type f -name '*.cdx.json' -delete
  fi
done

# Generate per-binary SBOMs in each published crate. `--target` pins
# the target triple embedded inside each SBOM metadata block, which
# is what downstream consumers rely on.
for entry in "${COMPONENT_MAP[@]}"; do
  IFS='|' read -r stem crate_dir bin <<<"${entry}"
  crate_root="${WORKSPACE_ROOT}/crates/${crate_dir}"
  if [[ ! -d "${crate_root}" ]]; then
    echo "::error::crate directory missing: ${crate_root}" >&2
    exit 1
  fi
  echo "::group::cargo-cyclonedx for ${crate_dir} (target=${RUST_TARGET})"
  (
    cd "${crate_root}"
    cargo cyclonedx --format json --describe binaries --target "${RUST_TARGET}"
  )
  echo "::endgroup::"
done

# Rename / move each per-bin SBOM to the canonical contract name and
# location the flatten script consumes. Also validates that the
# expected SBOM files actually exist after generation; if a future
# version of cargo-cyclonedx renames its output we fail here with a
# precise error rather than shipping a release with no SBOMs.
MISSING=()
for entry in "${COMPONENT_MAP[@]}"; do
  IFS='|' read -r stem crate_dir bin <<<"${entry}"
  crate_root="${WORKSPACE_ROOT}/crates/${crate_dir}"
  generated="${crate_root}/${bin}_bin.cdx.json"
  canonical="${WORKSPACE_ROOT}/crates/${stem}-${RUST_TARGET}.cdx.json"
  if [[ ! -f "${generated}" ]]; then
    MISSING+=("${generated}")
    continue
  fi
  mv -f -- "${generated}" "${canonical}"
  echo "staged ${canonical}"
done

if (( ${#MISSING[@]} > 0 )); then
  echo "::error::cargo-cyclonedx did not produce the expected per-binary SBOMs:" >&2
  for m in "${MISSING[@]}"; do
    echo "         ${m}" >&2
  done
  echo "       this usually means cargo-cyclonedx output naming changed" >&2
  echo "       or a published crate lost one of its [[bin]] targets." >&2
  exit 1
fi

# Defensive cleanup: any `*_bin.cdx.json` file left behind (in any
# crate, not just the published ones) belongs to a bin that is NOT
# in the published component map (e.g. mcp-client, cognicode-release,
# sandbox-orchestrator, explorer-mcp, explorer-api). They must not
# leak into the upload because the upload path is one level shallow
# and would skip them anyway, but leaving them on disk pollutes the
# workspace and confuses anyone inspecting the tree.
shopt -s nullglob
stray=( "${WORKSPACE_ROOT}"/crates/*/*_bin.cdx.json )
for s in "${stray[@]}"; do
  case "${s}" in
    "${WORKSPACE_ROOT}/crates/cogh-${RUST_TARGET}.cdx.json"|\
    "${WORKSPACE_ROOT}/crates/cognicode-${RUST_TARGET}.cdx.json"|\
    "${WORKSPACE_ROOT}/crates/cognicode-mcp-${RUST_TARGET}.cdx.json") continue ;;
  esac
  echo "::warning::removing stray per-bin SBOM not in published set: ${s}" >&2
  rm -f -- "${s}"
done

echo "build-sboms-for-lane: OK  target=${RUST_TARGET} components=$(printf '%s ' "${COMPONENT_MAP[@]%%|*}")"
