#!/usr/bin/env bash
# Publishes the four SDK library crates in dependency order, then verifies them
# from a clean consumer project. OUTWARD-FACING AND IRREVERSIBLE (a published
# version can only be yanked), so it needs --confirm-publish and a READY
# preflight. It never logs in: the operator provides the credential.
#
#   --confirm-publish  required; without it nothing happens (exit 2)
#   --resume           skip crates whose version is already on crates.io
#                      (to finish a release that stopped halfway)
#
# Env: SDK_PUBLISH_PREFLIGHT_ARGS (extra preflight flags, used by tests)
#      SDK_INDEX_TIMEOUT_SECS (per-crate indexation wait, default 600)
set -euo pipefail

cd "$(dirname "$0")/../.."

confirm=0
resume=0
for arg in "$@"; do
  case "$arg" in
    --confirm-publish) confirm=1 ;;
    --resume) resume=1 ;;
    *) echo "unknown argument: $arg" >&2; exit 2 ;;
  esac
done
if [ "$confirm" -ne 1 ]; then
  echo "refusing to publish: pass --confirm-publish (irreversible; see docs/distribution/SDK_RELEASE.md)" >&2
  exit 2
fi

crates=(cognicode-graph-algos cognicode-macros cognicode-core cognicode-graph-wasm)
version="$(awk '/^\[workspace.package\]/{p=1;next} /^\[/{p=0} p&&/^version *=/{gsub(/"/,"",$3);print $3;exit}' Cargo.toml)"
timeout_secs="${SDK_INDEX_TIMEOUT_SECS:-600}"

is_published() {
  curl -s --max-time 20 -A 'cognicode-publish-runner' \
    "https://crates.io/api/v1/crates/$1" | grep -q "\"num\":\"$version\""
}

# shellcheck disable=SC2086
preflight_args="${SDK_PUBLISH_PREFLIGHT_ARGS:-}"
if [ "$resume" -eq 1 ]; then
  # A half-published release would trip the "already published" check.
  preflight_args="$preflight_args --skip-registry"
fi
# shellcheck disable=SC2086
bash scripts/distribution/preflight-sdk-publish.sh $preflight_args

for name in "${crates[@]}"; do
  if [ "$resume" -eq 1 ] && is_published "$name"; then
    echo "skip $name $version (already on crates.io)"
    continue
  fi
  echo "publishing $name $version"
  cargo publish --locked -p "$name"
  waited=0
  until is_published "$name"; do
    if [ "$waited" -ge "$timeout_secs" ]; then
      echo "STOPPED: $name $version not indexed after ${timeout_secs}s; rerun with --resume" >&2
      exit 1
    fi
    sleep 10
    waited=$((waited + 10))
  done
  echo "indexed $name $version"
done

bash scripts/distribution/verify-sdk-consumer.sh --confirm-published
echo "PUBLISHED and verified: ${crates[*]} $version. Next: tag the release commit."
