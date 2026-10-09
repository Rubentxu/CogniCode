#!/usr/bin/env bash
# F6.W3.ter — Tag/workspace coherence gate.
#
# Verifies that the version declared in the workspace Cargo.toml matches the
# version of the git tag being released. This closes the v0.98.0 incident:
# tag v0.98.0 was created before bumping Cargo.toml, so the compiled binaries
# reported 0.97.5 and release-install-smoke.sh rejected them.
#
# Usage:
#   release-tag-coherence.sh                         → reads GITHUB_REF_NAME / tag at HEAD
#   release-tag-coherence.sh <tag>                   → checks against <tag> (e.g. v0.98.0)
#   release-tag-coherence.sh <tag> <sha>             → checks the tag against the cargo
#                                                     workspace at <sha>, AND that <sha>
#                                                     IS the commit the tag names
#                                                     (CI: $GITHUB_SHA). The identity
#                                                     check is what makes the version
#                                                     check mean anything: the numbers can
#                                                     agree while the bytes do not.
#   release-tag-coherence.sh --from-version <v> <sha>
#                                                     → release-validate mode: checks
#                                                     the workspace at <sha> against the
#                                                     bare version <v> (no 'v' prefix
#                                                     required; used when the operator
#                                                     validates a prospective tag without
#                                                     having pushed the tag yet). No tag
#                                                     exists to compare identity against,
#                                                     so only the version is checked.
#
# Exit codes:
#   0  workspace version == tag version (or --from-version value)
#   1  any other mismatch, parse failure, or unreachable <sha>
#   2  usage error
#
# Intentionally lives under scripts/ci/ so it shares the same invocation
# contract as release-install-smoke.sh and stage-platform-payloads.sh.
set -euo pipefail

TAG=""
SHA=""
EXPECTED_VERSION=""

if [ "${1:-}" = "--from-version" ]; then
  if [ $# -lt 3 ]; then
    echo "::error::release-tag-coherence: --from-version requires <version> <sha>" >&2
    exit 2
  fi
  EXPECTED_VERSION="$2"
  SHA="$3"
elif [ $# -eq 0 ]; then
  # Auto-detect from the current git state.
  if [ -n "${GITHUB_REF_NAME:-}" ]; then
    TAG="$GITHUB_REF_NAME"
  elif git describe --tags --exact-match HEAD >/dev/null 2>&1; then
    TAG="$(git describe --tags --exact-match HEAD)"
  else
    echo "::error::release-tag-coherence: no tag argument and HEAD is not on a tag" >&2
    exit 2
  fi
else
  TAG="$1"
  SHA="${2:-}"
fi

if [ -z "$SHA" ]; then
  SHA="$(git rev-parse HEAD)"
fi

# The commit identity, before the version string.
#
# MEDIDO 2026-10-03. This gate used to compare the workspace version against
# the tag name and stop there, so `release-tag-coherence.sh v0.101.6 HEAD`
# answered OK with HEAD two commits ahead of the tag, as long as the workspace
# still said 0.101.6. That is the same class of incident the gate exists to
# catch: the numbers agree and the bytes do not. A tag names one commit, so
# the commit the build runs from has to BE that commit — otherwise what gets
# published carries a version and not the code that was certified for it.
#
# `^{commit}` peels an annotated tag, which is why this cannot be a plain
# `git rev-parse <tag>`. An unresolvable tag is not an absent answer here: the
# caller named a tag and it is not there, so it is a failure.
if [ -n "$TAG" ] && [ -z "$EXPECTED_VERSION" ]; then
  if ! TAG_COMMIT="$(git rev-parse --verify --quiet "${TAG}^{commit}" 2>/dev/null)"; then
    echo "::error::release-tag-coherence: cannot resolve tag '$TAG' to a commit." >&2
    echo "::error::A tag must name the commit that is built, so an unresolvable" >&2
    echo "::error::tag is a failure and not a missing optional input." >&2
    exit 1
  fi
  RESOLVED_SHA="$(git rev-parse --verify --quiet "${SHA}^{commit}" 2>/dev/null || true)"
  if [ -z "$RESOLVED_SHA" ]; then
    echo "::error::release-tag-coherence: cannot resolve sha '$SHA' to a commit." >&2
    exit 1
  fi
  if [ "$TAG_COMMIT" != "$RESOLVED_SHA" ]; then
    echo "::error::release-tag-coherence: sha ${SHA:0:12} is not the commit of tag '$TAG'." >&2
    echo "::error::  tag  '$TAG' -> ${TAG_COMMIT:0:12}" >&2
    echo "::error::  build $RESOLVED_SHA" >&2
    echo "::error::The version can match and the bytes still differ, which is the" >&2
    echo "::error::incident this gate exists for. Build from the tagged commit, or" >&2
    echo "::error::cut a new tag: a published one is not moved." >&2
    exit 1
  fi
fi

if [ -n "$EXPECTED_VERSION" ]; then
  VERSION="$EXPECTED_VERSION"
  echo "validate-mode  expected.version=$VERSION  sha=${SHA:0:12}"
elif [ -n "$TAG" ]; then
  if [[ "$TAG" != v* ]]; then
    echo "::error::release-tag-coherence: tag '$TAG' must start with 'v' (e.g. v0.98.0)" >&2
    exit 2
  fi
  VERSION="${TAG#v}"
  echo "tag=$TAG  version=$VERSION  sha=${SHA:0:12}"
else
  echo "::error::release-tag-coherence: internal state error — neither tag nor --from-version captured" >&2
  exit 2
fi

# Read the workspace version at <sha> WITHOUT checking it out: parse Cargo.toml
# in-memory from the blob. This is the same trick used by `cognicode-release`
# to extract build metadata in restricted CI runners.
#
# Exit 128 from `git show` means the SHA is unreachable (object missing). We
# map that to 1 (parse failure) so callers can treat all "no usable version"
# cases uniformly.
CARGO_TOML_BLOB="$(git show "${SHA}:Cargo.toml" 2>/dev/null || true)"
if [ -z "$CARGO_TOML_BLOB" ]; then
  echo "::error::release-tag-coherence: cannot read Cargo.toml at ${SHA} (object missing or no git history at this SHA)." >&2
  exit 1
fi
WORKSPACE_VERSION="$(printf '%s\n' "$CARGO_TOML_BLOB" \
  | awk -F'"' '/^\[workspace\.package\]/{found=1} found && /^version[ \t]*=/{print $2; exit}')"

if [ -z "$WORKSPACE_VERSION" ]; then
  echo "::error::release-tag-coherence: could not parse workspace.package.version from ${SHA}" >&2
  exit 1
fi

echo "workspace.version=$WORKSPACE_VERSION"

if [ "$VERSION" != "$WORKSPACE_VERSION" ]; then
  echo "::error::release-tag-coherence: workspace.version (${WORKSPACE_VERSION}) does not match expected (${VERSION})." >&2
  if [ -n "$TAG" ]; then
    echo "::error::Bump Cargo.toml's [workspace.package] version to '${VERSION}' and commit before pushing tag ${TAG}." >&2
  else
    echo "::error::Adjust Cargo.toml's [workspace.package] version to '${VERSION}' (validate mode) or pass the matching --from-version." >&2
  fi
  echo "::error::Reference: PRF-JOURNAL §135 (v0.98.0 incident on 2026-09-24)." >&2
  exit 1
fi

if [ -n "$TAG" ]; then
  echo "OK: tag ${TAG} ↔ workspace.version ${WORKSPACE_VERSION}"
else
  echo "OK: validate-mode expected.version ${VERSION} ↔ workspace.version ${WORKSPACE_VERSION}"
fi
