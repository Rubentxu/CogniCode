#!/usr/bin/env bash
# Read-only preflight for publishing the four SDK library crates.
#
# It answers one question: "if the operator said publish now, what would stop
# it?" It never uploads, never logs in and never writes. Output is READY or
# BLOCKED with every missing condition named; the exit code is 0 only for READY.
#
#   --skip-registry  do not query crates.io (offline / deterministic tests)
#   --skip-git       do not check the working tree or origin/main
set -euo pipefail

cd "$(dirname "$0")/../.."

skip_registry=0
skip_git=0
for arg in "$@"; do
  case "$arg" in
    --skip-registry) skip_registry=1 ;;
    --skip-git) skip_git=1 ;;
    *) echo "unknown argument: $arg" >&2; exit 2 ;;
  esac
done

# Publication order: dependencies first (see docs/distribution/SDK_RELEASE.md).
crates=(cognicode-graph-algos cognicode-macros cognicode-core cognicode-graph-wasm)
version="$(awk '/^\[workspace.package\]/{p=1;next} /^\[/{p=0} p&&/^version *=/{gsub(/"/,"",$3);print $3;exit}' Cargo.toml)"

blocked=()

# 1. Credential: presence only, the value is never read into output.
cargo_home="${CARGO_HOME:-${HOME:-/nonexistent}/.cargo}"
if [ -z "${CARGO_REGISTRY_TOKEN:-}" ] \
  && [ ! -s "$cargo_home/credentials.toml" ] && [ ! -s "$cargo_home/credentials" ]; then
  blocked+=("crates.io credential missing: log in to the registry or export CARGO_REGISTRY_TOKEN")
fi

# 2. Names and version on crates.io (HTTP 404 = free, 200 = exists).
if [ "$skip_registry" -eq 0 ]; then
  for name in "${crates[@]}"; do
    body="$(mktemp)"
    code="$(curl -s -o "$body" -w '%{http_code}' --max-time 20 \
      -A 'cognicode-publish-preflight (read-only)' \
      "https://crates.io/api/v1/crates/$name" || echo 000)"
    case "$code" in
      404) ;;
      200) if grep -q "\"num\":\"$version\"" "$body"; then
             blocked+=("$name $version is already published")
           fi ;;
      *) blocked+=("$name: crates.io answered HTTP $code, availability unknown") ;;
    esac
    rm -f "$body"
  done
fi

# 3. Release commit: clean tree, equal to origin/main.
if [ "$skip_git" -eq 0 ]; then
  if [ -n "$(git status --porcelain)" ]; then
    blocked+=("working tree is not clean")
  fi
  if [ "$(git rev-parse HEAD)" != "$(git rev-parse origin/main 2>/dev/null || echo none)" ]; then
    blocked+=("HEAD is not origin/main")
  fi
fi

echo "SDK publish preflight: version ${version:-unknown}, order: ${crates[*]}"
if [ "${#blocked[@]}" -eq 0 ]; then
  echo "READY"
  exit 0
fi
echo "BLOCKED"
for reason in "${blocked[@]}"; do echo "  - $reason"; done
exit 1
