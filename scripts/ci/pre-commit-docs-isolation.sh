#!/usr/bin/env bash
# pre-commit-docs-isolation.sh — F6.W3.quarter pre-commit guard.
#
# Detects the pattern that caused the F6.W3.ter accidental-reversion bug
# (JOURNAL §136 + §137): a single commit that mixes `docs/prf/*` PRF
# artifacts with code/workflow changes when those docs have been staged
# in earlier session activity that the operator forgot about.
#
# The bug surface:
#
#   1. Operator (or agent) edits STATE.md / appends to JOURNAL.md, leaving
#      those files STAGED in the index from a prior session.
#   2. Later, operator runs `git add .github/workflows/release.yml` and
#      commits — without realizing the staged docs are about to be picked
#      up by the same commit.
#   3. The commit silently captures the doc edits (or reverts them, if
#      the staged docs were a previous-commit's content), corrupting the
#      traceability trail expected by the PRF program.
#
# This hook refuses the commit if BOTH conditions are present:
#
#   (a) ≥1 file under docs/prf/* staged, AND
#   (b) ≥1 file under crates/, scripts/, or .github/workflows/ staged.
#
# In practice this combination almost always means "two unrelated change
# families got merged into one commit by mistake" — the very shape of
# the d40e61b2 incident where adding workflow files picked up a
# previously-staged doc reversion.
#
# Bypass for intentional mixed commits:
#   GIT_SKIP_DOCS_ISOLATION=1 git commit ...
#   git commit --no-verify ...
#
# Both intentional escapes are acceptable: the operator who knows this
# pattern can override it; the operator who doesn't will get the warning.
#
# Exit codes:
#   0  commit allowed
#   1  commit blocked (mixed staging detected)
#   2  invocation error (no git context)
#
# Intentionally lives under scripts/ci/ so it shares the same invocation
# contract as the other release/audit scripts. The hook installation
# itself is operator-gated: see `scripts/ci/install-docs-isolation-hook.sh`
# for the one-line install that wires this script into the operator's
# `~/.git-hooks/pre-commit`.
set -uo pipefail

# Bypass
if [ "${GIT_SKIP_DOCS_ISOLATION:-0}" != "0" ] && [ -n "${GIT_SKIP_DOCS_ISOLATION:-}" ]; then
  exit 0
fi

# Read staged paths from the index (what is about to be committed,
# without expanding the worktree). `git diff --cached --name-only` is
# the canonical pre-commit hook API.
if ! STAGED="$(git diff --cached --name-only --diff-filter=ACMR 2>/dev/null)"; then
  # Not in a git context, or no staged changes: nothing to check.
  exit 0
fi

DOCS=()
CODE=()
while IFS= read -r p; do
  [ -n "$p" ] || continue
  case "$p" in
    docs/prf/*|docs/PRF/*)
      DOCS+=("$p")
      ;;
    crates/*.rs|crates/*/Cargo.toml|scripts/*.sh|scripts/**/*.sh|.github/workflows/*.yml|.github/workflows/*.yaml)
      CODE+=("$p")
      ;;
  esac
done <<< "$STAGED"

DOC_COUNT="${#DOCS[@]}"
CODE_COUNT="${#CODE[@]}"

if [ "$DOC_COUNT" -gt 0 ] && [ "$CODE_COUNT" -gt 0 ]; then
  {
    echo "commit-guard (docs-isolation): ❌ COMMIT BLOQUEADO"
    echo
    echo "El commit actual mezcla artefactos PRF (docs/prf/*) con cambios de"
    echo "código/infraestructura — patrón del bug d40e61b2 (JOURNAL §137): un"
    echo "commit que captura docs PRF pre-staged de una sesión anterior junto"
    echo "con código/workflow nuevos suele revertir/corromper la trazabilidad."
    echo
    echo "  docs staged ($DOC_COUNT):"
    for d in "${DOCS[@]}"; do echo "    $d"; done
    echo
    echo "  code/infra staged ($CODE_COUNT):"
    for c in "${CODE[@]}"; do echo "    $c"; done
    echo
    echo "Recomendación — dividir en dos commits:"
    echo
    echo "  git restore --staged docs/prf/        # sacar docs del staging"
    echo "  git commit -m \"...\"                   # commit solo de código"
    echo "  git add -f docs/prf/                  # restage docs"
    echo "  git commit -m \"...\"                   # commit solo de docs"
    echo
    echo "Si la mezcla es INTENCIONAL (p. ej. nuevo script CI + docs del mismo"
    echo "cambio), bypass explícito:"
    echo
    echo "  GIT_SKIP_DOCS_ISOLATION=1 git commit ..."
    echo "  # o"
    echo "  git commit --no-verify ..."
  } >&2
  exit 1
fi

exit 0
