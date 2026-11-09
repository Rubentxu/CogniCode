#!/usr/bin/env bash
# Read-only check of the operator-side prerequisites of the SDK auto-release
# timer (docs/distribution/SDK_RELEASE.md, "Automatic releases"). It answers:
# "if the operator enabled the timer now, would the service run?" It never
# installs, enables, publishes or logs in, and never prints a credential.
#
#   --skip-units  do not run systemd-analyze on the unit templates
#
# Prints READY or BLOCKED with every missing condition; exit 0 only for READY.
set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
skip_units=0
for arg in "$@"; do
  case "$arg" in
    --skip-units) skip_units=1 ;;
    *) echo "unknown argument: $arg" >&2; exit 2 ;;
  esac
done

blocked=()
home="${HOME:-/nonexistent}"
envfile="$home/.config/cognicode/sdk-release.env"

# 1. The unit templates must load.
if [ "$skip_units" -eq 0 ]; then
  if command -v systemd-analyze >/dev/null 2>&1; then
    units="$root/scripts/distribution/systemd"
    if ! systemd-analyze verify --user \
      "$units/cognicode-sdk-auto-release.service" "$units/cognicode-sdk-auto-release.timer" >/dev/null 2>&1; then
      blocked+=("systemd-analyze verify rejects the unit templates in scripts/distribution/systemd/")
    fi
  else
    blocked+=("systemd-analyze not found: cannot verify the unit templates")
  fi
fi

# 2. The operator-owned environment file: present, mode 600, names a checkout.
checkout=""
if [ ! -f "$envfile" ]; then
  blocked+=("$envfile is missing: create it (mode 600) with COGNICODE_CHECKOUT=<clean checkout of main>")
else
  mode="$(stat -c '%a' "$envfile")"
  [ "$mode" = "600" ] || blocked+=("$envfile has mode $mode: it must be mode 600")
  checkout="$(sed -n 's/^COGNICODE_CHECKOUT=//p' "$envfile" | head -n1)"
  [ -n "$checkout" ] || blocked+=("$envfile does not define COGNICODE_CHECKOUT")
fi

# 3. That checkout must be a clean git work tree.
if [ -n "$checkout" ]; then
  if ! git -C "$checkout" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    blocked+=("COGNICODE_CHECKOUT is not a git checkout: $checkout")
  elif [ -n "$(git -C "$checkout" status --porcelain)" ]; then
    blocked+=("COGNICODE_CHECKOUT has uncommitted changes: $checkout")
  fi
fi

# 4. A registry credential: presence only, the value is never read into output.
cargo_home="${CARGO_HOME:-$home/.cargo}"
has_token=0
[ -s "$cargo_home/credentials.toml" ] || [ -s "$cargo_home/credentials" ] && has_token=1
if [ "$has_token" -eq 0 ] && { [ ! -f "$envfile" ] || ! grep -q '^CARGO_REGISTRY_TOKEN=' "$envfile"; }; then
  blocked+=("no crates.io credential: run cargo login or add CARGO_REGISTRY_TOKEN to $envfile")
fi

if [ "${#blocked[@]}" -eq 0 ]; then
  echo "READY: the timer prerequisites hold; enabling it is the operator's step"
  exit 0
fi
echo "BLOCKED:"
printf '  - %s\n' "${blocked[@]}"
exit 1
