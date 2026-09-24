#!/usr/bin/env bash
# install-docs-isolation-hook.sh — operator-gated installer.
#
# Wires scripts/ci/pre-commit-docs-isolation.sh into the operator's
# global git-hooks directory (~/.git-hooks/) so that every commit on
# any repo with core.hooksPath pointing there gets the docs-isolation
# guard.
#
# This script MODIFIES the operator's personal hook directory. It is
# operator-gated: only run if the operator has reviewed the guard's
# behaviour (JOURNAL §137) and decided to enable it.
#
# Idempotent: if the hook is already installed (linking to this script),
# running the installer again is a no-op. If the hook exists but points
# elsewhere, it is replaced.
#
# Usage:
#   ./scripts/ci/install-docs-isolation-hook.sh           # install
#   ./scripts/ci/install-docs-isolation-hook.sh --uninstall   # remove
#
# Exit codes:
#   0  installed (or already installed, no change)
#   1  install failed (hook directory missing, etc.)
#
# The script lives under scripts/ci/ to match the project's convention.
set -euo pipefail

HOOKS_DIR="${HOOKS_DIR:-$HOME/.git-hooks}"
HOOK_NAME="pre-commit"
TARGET="$HOOKS_DIR/$HOOK_NAME"

# Locate this installer + the guard script (works from any CWD inside the repo)
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
GUARD="$SCRIPT_DIR/pre-commit-docs-isolation.sh"

if [ ! -x "$GUARD" ]; then
  echo "❌ installer: guard script not found or not executable at $GUARD" >&2
  echo "   chmod +x scripts/ci/pre-commit-docs-isolation.sh and retry." >&2
  exit 1
fi

if [ "${1:-}" = "--uninstall" ]; then
  if [ -L "$TARGET" ] && [ "$(readlink "$TARGET")" = "$GUARD" ]; then
    rm "$TARGET"
    echo "✓ uninstalled: removed $TARGET (was a symlink to $GUARD)"
  elif [ -e "$TARGET" ]; then
    echo "⚠️  $TARGET exists but is NOT a symlink to $GUARD (owner: $(stat -c '%U' "$TARGET" 2>/dev/null || echo unknown))." >&2
    echo "   refusing to delete it. Remove manually if desired." >&2
    exit 1
  else
    echo "✓ uninstalled: $TARGET did not exist."
  fi
  exit 0
fi

# install path
if [ ! -d "$HOOKS_DIR" ]; then
  echo "❌ installer: hooks directory $HOOKS_DIR does not exist." >&2
  echo "   Your global hook setup appears non-standard. Either create the directory" >&2
  echo "   manually or wire the guard another way (see AGENTS.md for this project)." >&2
  exit 1
fi

# If the existing pre-commit hook is one of this project's known global
# hooks (commit-msg, pre-push, etc.) or any file at all, refuse to clobber.
# Force the operator to decide.
if [ -e "$TARGET" ] && ! [ -L "$TARGET" ]; then
  echo "⚠️  $TARGET exists as a regular file (or non-matching symlink)." >&2
  echo "   The installer refuses to overwrite it silently. Decide what to do:" >&2
  echo "     1. Inspect the existing hook:  cat $TARGET" >&2
  echo "     2. If you want to chain the guard behind it, manually add:" >&2
  echo "        bash $GUARD \"\$@\"" >&2
  echo "     3. To replace it with the guard-only hook:  rm $TARGET && $0" >&2
  exit 1
fi

if [ -L "$TARGET" ] && [ "$(readlink "$TARGET")" = "$GUARD" ]; then
  echo "✓ $TARGET already installed (symlink to $GUARD). No-op."
  exit 0
fi

ln -s "$GUARD" "$TARGET"
echo "✓ installed: $TARGET -> $GUARD"
echo ""
echo "From now on, every commit on any repo you make with hooksPath=$HOOKS_DIR"
echo "will run the docs-isolation guard. To skip for one commit:"
echo "  GIT_SKIP_DOCS_ISOLATION=1 git commit ..."
echo "or"
echo "  git commit --no-verify ..."
echo ""
echo "To uninstall:  scripts/ci/install-docs-isolation-hook.sh --uninstall"
