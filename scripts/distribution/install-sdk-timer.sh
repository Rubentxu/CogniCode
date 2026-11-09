#!/usr/bin/env bash
# Prepares the operator side of the SDK auto-release timer in one command:
# copies the two unit templates to ~/.config/systemd/user/ and creates
# ~/.config/cognicode/sdk-release.env (mode 600) naming the dedicated checkout.
#
#   --checkout <path>  clean git checkout of main used only by the timer
#   --apply            write the files; without it the script only prints the plan
#
# It never enables the timer, never runs systemctl, never stores or asks for a
# credential, never clones and never overwrites a file the operator already has
# (an existing env file is kept; a differing unit file stops the run).
set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
checkout=""
apply=0
while [ "$#" -gt 0 ]; do
  case "$1" in
    --checkout) checkout="${2:-}"; shift 2 ;;
    --apply) apply=1; shift ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

[ -n "$checkout" ] || { echo "--checkout <path> is required" >&2; exit 2; }
git -C "$checkout" rev-parse --is-inside-work-tree >/dev/null 2>&1 \
  || { echo "not a git checkout: $checkout" >&2; exit 2; }
checkout="$(cd "$checkout" && pwd)"

home="${HOME:?HOME is not set}"
envdir="$home/.config/cognicode"
envfile="$envdir/sdk-release.env"
unitdir="$home/.config/systemd/user"
src="$root/scripts/distribution/systemd"
units=(cognicode-sdk-auto-release.service cognicode-sdk-auto-release.timer)

# A unit file that already differs is the operator's: stop before writing anything.
for u in "${units[@]}"; do
  if [ -e "$unitdir/$u" ] && ! cmp -s "$src/$u" "$unitdir/$u"; then
    echo "$unitdir/$u exists and differs from the template: not overwriting" >&2
    exit 1
  fi
done

if [ "$apply" -eq 0 ]; then
  echo "dry run (pass --apply to write):"
  [ -e "$envfile" ] && echo "  keep   $envfile" || echo "  create $envfile (mode 600, COGNICODE_CHECKOUT=$checkout)"
  for u in "${units[@]}"; do echo "  copy   $unitdir/$u"; done
  exit 0
fi

mkdir -p "$envdir" "$unitdir"
if [ ! -e "$envfile" ]; then
  ( umask 077; printf 'COGNICODE_CHECKOUT=%s\n' "$checkout" > "$envfile" )
  chmod 600 "$envfile"
  echo "created $envfile"
else
  echo "kept $envfile"
fi
for u in "${units[@]}"; do
  cp "$src/$u" "$unitdir/$u"
  echo "installed $unitdir/$u"
done

cat <<EOF
Next, still yours: store the registry credential (cargo login), then
  bash scripts/distribution/check-sdk-timer.sh
and, once it prints READY:
  systemctl --user daemon-reload
  systemctl --user enable --now cognicode-sdk-auto-release.timer
EOF
