#!/usr/bin/env bash
# Publishes the workspace version of the SDK crates when (and only when) it is
# not yet on crates.io. Safe to run on a schedule: with nothing new it does
# nothing, and a release that stopped halfway is finished, not repeated.
# Running it IS the operator's standing authorisation to publish the version
# that main declares; it never logs in and never tags.
#
#   --check   print the decision (up to date / PENDING crates) and stop
#
# Env: SDK_REGISTRY_API  registry base URL (default https://crates.io/api/v1/crates;
#                        tests point it at a file:// listing)
set -euo pipefail

cd "$(dirname "$0")/../.."

check=0
for arg in "$@"; do
  case "$arg" in
    --check) check=1 ;;
    *) echo "unknown argument: $arg" >&2; exit 2 ;;
  esac
done

crates=(cognicode-graph-algos cognicode-macros cognicode-core cognicode-graph-wasm)
version="$(awk '/^\[workspace.package\]/{p=1;next} /^\[/{p=0} p&&/^version *=/{gsub(/"/,"",$3);print $3;exit}' Cargo.toml)"
[ -n "$version" ] || { echo "cannot read the workspace version" >&2; exit 1; }
api="${SDK_REGISTRY_API:-https://crates.io/api/v1/crates}"

pending=()
for name in "${crates[@]}"; do
  if ! curl -s --max-time 20 -A 'cognicode-auto-release' "$api/$name" | grep -q "\"num\":\"$version\""; then
    pending+=("$name")
  fi
done

if [ "${#pending[@]}" -eq 0 ]; then
  echo "up to date: ${crates[*]} $version are on crates.io"
  exit 0
fi

echo "PENDING for $version: ${pending[*]}"
if [ "$check" -eq 1 ]; then
  exit 0
fi

# The runner re-checks credential, clean tree and HEAD == origin/main, and stops
# before any upload when one is missing.
git fetch origin main
exec bash scripts/distribution/publish-sdk.sh --confirm-publish --resume
