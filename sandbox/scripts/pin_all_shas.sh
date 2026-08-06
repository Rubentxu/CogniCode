#!/usr/bin/env bash
# Resolve all repo refs (tags/branches) to exact SHA via git ls-remote
# Outputs updated pin_repo calls with SHA pins
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CLONE_SCRIPT="${SCRIPT_DIR}/clone_repos.sh"

if [ ! -f "$CLONE_SCRIPT" ]; then
    echo "ERROR: clone_repos.sh not found at $CLONE_SCRIPT" >&2
    exit 2
fi

# Extract repo entries from clone_repos.sh
extract_repos() {
    grep -E '^\s*pin_repo\s+"[^"]+"\s+"https://' "$CLONE_SCRIPT" | \
        sed -nE 's/.*pin_repo\s+"([^"]+)"\s+"([^"]+)"\s+"([^"]+)"\s*"([^"]*)".*/\1|\2|\3|\4/p'
}

# Resolve a ref to SHA via git ls-remote
resolve_sha() {
    local url="$1"
    local ref="$2"
    
    if echo "$ref" | grep -qE '^[0-9a-f]{8,40}$'; then
        # Already a SHA
        echo "$ref"
        return 0
    fi
    
    # Try git ls-remote first (no clone needed)
    if sha=$(git ls-remote "$url" "$ref" 2>/dev/null | cut -f1); then
        if [ -n "$sha" ]; then
            echo "$sha"
            return 0
        fi
    fi
    
    # Fallback: check if already cloned locally
    return 1
}

echo "=== Resolving all repo SHAs ==="
echo ""

while IFS='|' read -r name url ref ref_type; do
    if [ -z "$name" ]; then
        continue
    fi
    
    echo "Resolving: $name ($ref)"
    
    sha=$(resolve_sha "$url" "$ref")
    if [ -n "$sha" ]; then
        echo "  SHA: $sha"
        echo "  Updated pin_repo call:"
        echo "  pin_repo \"$name\" \"$url\" \"$sha\" \"commit\""
    else
        echo "  WARNING: Could not resolve SHA for $name at $ref — leaving as-is"
        # Try to get from local clone if exists
        target_dir="${SCRIPT_DIR}/../repos/$name"
        if [ -d "$target_dir" ]; then
            local_sha=$(git -C "$target_dir" rev-parse HEAD 2>/dev/null || echo "")
            if [ -n "$local_sha" ]; then
                echo "  Using local clone SHA: $local_sha"
                sha="$local_sha"
            fi
        fi
    fi
    echo ""
done < <(extract_repos)

echo "=== Resolution complete ==="
echo ""
echo "To update clone_repos.sh with SHA pins, run:"
echo "  bash ${SCRIPT_DIR}/pin_all_shas.sh --update"
