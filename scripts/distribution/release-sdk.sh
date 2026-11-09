#!/usr/bin/env bash
# One command per release of the four SDK library crates.
#
#   release-sdk.sh <version>   prepare: bump the version everywhere, refresh
#                              Cargo.lock, rehearse the publish (no upload),
#                              commit on release/v<version>, push, open the PR
#   release-sdk.sh --publish   after that PR is merged: publish in order,
#                              verify from a clean consumer, tag v<version>
#                              and push the tag
#
# main only accepts changes through a PR, so the bump cannot skip it; this
# script does everything around that one merge. --publish is irreversible
# (a published version can only be yanked) and needs the registry credential;
# it never logs in.
set -euo pipefail

cd "$(dirname "$0")/../.."

current="$(awk '/^\[workspace.package\]/{p=1;next} /^\[/{p=0} p&&/^version *=/{gsub(/"/,"",$3);print $3;exit}' Cargo.toml)"
[ -n "$current" ] || { echo "cannot read the workspace version" >&2; exit 1; }

case "${1:-}" in
  "")
    echo "usage: release-sdk.sh <version> | --publish" >&2
    exit 2
    ;;
  --publish)
    [ "$(git branch --show-current)" = "main" ] || { echo "run --publish from main" >&2; exit 1; }
    git fetch origin main
    git merge --ff-only origin/main
    version="$(awk '/^\[workspace.package\]/{p=1;next} /^\[/{p=0} p&&/^version *=/{gsub(/"/,"",$3);print $3;exit}' Cargo.toml)"
    bash scripts/distribution/publish-sdk.sh --confirm-publish --resume
    if ! git rev-parse -q --verify "refs/tags/v$version" >/dev/null; then
      git tag -a "v$version" -m "cognicode v$version"
    fi
    git push origin "v$version"
    echo "RELEASED v$version: published, verified and tagged."
    exit 0
    ;;
esac

new="$1"
echo "$new" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$' || { echo "not a x.y.z version: $new" >&2; exit 2; }
if [ "$(printf '%s\n%s\n' "$current" "$new" | sort -V | tail -1)" != "$new" ] || [ "$current" = "$new" ]; then
  echo "version $new is not greater than the current $current" >&2
  exit 2
fi
[ -z "$(git status --porcelain)" ] || { echo "the working tree is not clean" >&2; exit 1; }
git fetch origin main
[ "$(git rev-parse HEAD)" = "$(git rev-parse origin/main)" ] || { echo "HEAD is not at origin/main" >&2; exit 1; }

old="${current//./\\.}"
sed -i -E \
  -e "/^version = /s/\"$old\"/\"$new\"/" \
  -e "/^cognicode-(core|graph-algos|macros) = /s/version = \"$old\"/version = \"$new\"/" \
  Cargo.toml
cargo update --workspace --offline

branch="release/v$new"
git checkout -b "$branch"
git add Cargo.toml
[ ! -f Cargo.lock ] || git add Cargo.lock
git commit -q -m "chore(release): v$new"

bash scripts/distribution/rehearse-sdk-publish.sh
git push -u origin "$branch"
gh pr create --base main --head "$branch" --title "chore(release): v$new" \
  --body "Bumps the SDK to $new. Rehearsal (cargo publish --dry-run) passed. After merging, run: bash scripts/distribution/release-sdk.sh --publish"
echo "PREPARED v$new. Merge the PR, then run: bash scripts/distribution/release-sdk.sh --publish"
