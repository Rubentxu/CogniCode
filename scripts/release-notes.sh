#!/usr/bin/env bash
set -e
VERSION="$1"
if [ -z "$VERSION" ]; then echo "Usage: $0 <version>"; exit 1; fi

# Extract changelog entries for this version
grep -A 20 "^## \[${VERSION}\]" CHANGELOG.md | tail -n +3 | head -n -2
