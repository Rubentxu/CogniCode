#!/usr/bin/env bash
# Server-side projection of the local git-control policy (GCTL-MSG-002,
# conventional-commits, forbidden-trailers, identity-policy).
#
# WHY THIS EXISTS
#   GitHub builds squash-merge commits server-side from the PR title and
#   body, so no local hook ever sees the message that lands on main.
#   PR #805 landed a "Generated with Claude Code" footer that way. This
#   guard validates exactly what the hosting will write: the composed
#   squash message (title + body) and every commit the PR carries.
#
# SOURCE OF TRUTH
#   The git-control config (~/.config/git-control/config.toml) owns the
#   dictionary. This script mirrors its ENFORCED block rules only — the
#   terms the hosting can evaluate. It does NOT mirror `cursor` (severity
#   warn: a false positive, the product has a pagination cursor.rs) nor
#   `pi`. If the config's dictionary changes, sync this file and say so in
#   the commit message.
#
# usage: policy-guard.sh <pr-number>   (on a GitHub runner, gh + GITHUB_TOKEN)
set -euo pipefail

pr="${1:?pr number required}"
repo="${GITHUB_REPOSITORY:?GITHUB_REPOSITORY required}"

# ── The policy, mirrored from the git-control config ────────────────────
types='feat|fix|docs|refactor|perf|test|build|ci|chore|release|evidence|cert'
agent_terms='generated with|claude|anthropic|codex|openai|mcode|opencode|zcode|copilot|gemini|aider|cody'
allowed_emails='^(ilargia\.c\.g@gmail\.com|haizea\.cabrera\.g@gmail\.com|pabloformacion13@gmail\.com|rubentxu74@gmail\.com|socketstuido@gmail\.com|nomeacuerdodelputocorreo@gmail\.com|rubentxudev@gmail\.com|rdcabrerag@eservicios\.indra\.es|Rubentxu@users\.noreply\.github\.com|noreply@github\.com)$'

violations=0
say() { printf '%s\n' "$*" >&2; violations=$((violations + 1)); }

# `term` busca la palabra completa, sin distinguir mayúsculas — el mismo
# criterio del mecanismo local: "claude" no dispara dentro de otra palabra.
has_term() {
  printf '%s' "$1" | grep -iqE "\b($agent_terms)\b"
}

# ── 1. El mensaje que el squash escribirá: título + cuerpo de la PR ──────
title="$(gh api "repos/$repo/pulls/$pr" --jq .title)"
body="$(gh api "repos/$repo/pulls/$pr" --jq .body)"

printf 'pr title: %s\n' "$title" >&2

if has_term "$title"; then
  say "DENY GCTL-MSG-002: the PR TITLE carries an agent name; it becomes the squash commit's header"
fi
if [ -n "$body" ] && has_term "$body"; then
  say "DENY GCTL-MSG-002: the PR BODY carries an agent name or footer; it becomes the squash commit's message body"
fi
if [ -n "$body" ] && printf '%s' "$body" | grep -iqE '^\s*(co-authored-by|co-author)\s*:'; then
  say "DENY GCTL-TRAILER: credit trailers go to AUTHORS.md, not the message"
fi
if ! printf '%s' "$title" | grep -qE "^($types)(\([^)]+\))?!?: .+"; then
  say "DENY GCTL-CONV: the PR title is not a conventional header of the allowed types ($types)"
fi
if [ "${#title}" -gt 120 ]; then
  say "DENY GCTL-CONV: the PR title exceeds header_max (120): ${#title} chars"
fi

# ── 2. Cada commit de la PR: mensaje y autoría ───────────────────────────
while IFS=$'\t' read -r sha msg aemail cemail; do
  if has_term "$msg"; then
    say "DENY GCTL-MSG-002: commit ${sha:0:8} carries an agent name in its message"
  fi
  if printf '%s' "$msg" | grep -iqE '^\s*(co-authored-by|co-author)\s*:'; then
    say "DENY GCTL-TRAILER: commit ${sha:0:8} carries a credit trailer"
  fi
  head_line="$(printf '%s' "$msg" | head -1)"
  if ! printf '%s' "$head_line" | grep -qE "^($types)(\([^)]+\))?!?: .+"; then
    say "DENY GCTL-CONV: commit ${sha:0:8} header is not conventional: $head_line"
  fi
  if ! printf '%s' "$aemail" | grep -iqE "$allowed_emails"; then
    say "DENY GCTL-ID: commit ${sha:0:8} author email is not in the allowlist: $aemail"
  fi
  if ! printf '%s' "$cemail" | grep -iqE "$allowed_emails"; then
    say "DENY GCTL-ID: commit ${sha:0:8} committer email is not in the allowlist: $cemail"
  fi
done < <(gh api "repos/$repo/pulls/$pr/commits" --paginate \
  --jq '.[] | [.sha, (.commit.message | gsub("\t"; " ")), (.commit.author.email // ""), (.commit.committer.email // "")] | @tsv')

if [ "$violations" -gt 0 ]; then
  say "policy-guard: $violations violation(s) — the merge stays blocked"
  exit 1
fi
printf 'policy-guard: ok — the squash message and every commit satisfy the policy\n' >&2
