#!/usr/bin/env bash
# Squash-merges a pull request only after git-control has judged the exact
# title and body that GitHub will write to main.
#
# GitHub builds the squash commit server-side, so the local commit-msg hook
# never sees it. This wrapper builds the same message, runs it through
# `git-control check` (the single source of the message policy; no term list
# lives here) and calls `gh pr merge --squash` with that very subject and body.
#
# usage: gh-pr-merge-checked.sh <pr-number> [--subject TEXT] [--body TEXT]
#   --subject  defaults to the pull request title
#   --body     defaults to the branch commits, as GitHub composes them
# env: GIT_CONTROL_BIN (default git-control), GH_BIN (default gh)
set -euo pipefail

git_control="${GIT_CONTROL_BIN:-git-control}"
gh_bin="${GH_BIN:-gh}"

pr="${1:-}"
if [[ ! "$pr" =~ ^[0-9]+$ ]]; then
  echo "usage: $0 <pr-number> [--subject TEXT] [--body TEXT]" >&2
  exit 2
fi
shift

subject=""
body=""
have_subject=0
have_body=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --subject) subject="${2:?--subject needs a value}"; have_subject=1; shift 2 ;;
    --body) body="${2:?--body needs a value}"; have_body=1; shift 2 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

if ! command -v "$git_control" >/dev/null 2>&1; then
  echo "git-control not found: refusing to merge without a validator" >&2
  exit 3
fi

if [[ $have_subject -eq 0 ]]; then
  subject="$("$gh_bin" pr view "$pr" --json title --jq .title)"
fi
if [[ $have_body -eq 0 ]]; then
  body="$("$gh_bin" pr view "$pr" --json commits \
    --jq '.commits[] | "* " + .messageHeadline + "\n\n" + .messageBody')"
fi

# GitHub appends " (#N)" to the squash header, so it is validated with it.
message="$(mktemp)"
trap 'rm -f "$message"' EXIT
printf '%s (#%s)\n\n%s\n' "$subject" "$pr" "$body" > "$message"

verdict="$(mktemp)"
trap 'rm -f "$message" "$verdict"' EXIT
if ! "$git_control" check --file "$message" --json > "$verdict"; then
  echo "squash message blocked by the message policy; nothing was merged:" >&2
  python3 - "$verdict" >&2 <<'PY'
import json, sys
try:
    data = json.load(open(sys.argv[1], encoding="utf-8"))
except Exception:
    print("  (git-control gave no readable verdict)")
    raise SystemExit
for d in data.get("diagnostics", []):
    where = (d.get("location") or {}).get("line")
    terms = [e.get("text", "").strip() for e in d.get("evidence", []) if e.get("text")]
    print(f"  rule {d.get('id')}: {d.get('summary')} (message line {where}) matched: {', '.join(terms) or '-'}")
PY
  exit 1
fi

exec "$gh_bin" pr merge "$pr" --squash --subject "$subject" --body "$body"
