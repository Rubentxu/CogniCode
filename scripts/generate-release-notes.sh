#!/usr/bin/env bash
set -e

PREV_TAG="${1:-$(git describe --tags --abbrev=0 HEAD^)}"
CURRENT_TAG="${2:-$(git describe --tags --abbrev=0)}"

echo "## Changes since $PREV_TAG"
echo ""
echo "### Features"
git log "$PREV_TAG..$CURRENT_TAG" --grep="feat" --format="  - %s" | grep "^  - feat"
echo ""
echo "### Fixes"
git log "$PREV_TAG..$CURRENT_TAG" --grep="fix" --format="  - %s" | grep "^  - fix"
echo ""
echo "### Other changes"
git log "$PREV_TAG..$CURRENT_TAG" --format="  - %s" | grep -v "^  - feat" | grep -v "^  - fix"
