#!/usr/bin/env bash
# Clone and provision sandbox repos with pinned commits/digests
# Phase 1: serde-rs/serde + pallets/click pinned at specific commits
# Phase 2: Adds JS (chalk) and TS (commander) real repos
# Phase 3: Adds Go (cobra, bubbletea, lo), Java (spring-petclinic),
#          JS (express), TS (zod) real repos for coverage expansion
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="${SCRIPT_DIR}/../repos"
mkdir -p "$REPO_DIR"

# Pin function: clone or update a repo at a specific tag/branch/commit
# Use commit SHA for precise pinning, or tag/branch name for range
pin_repo() {
    local name="$1"
    local url="$2"
    local ref="${3:-}"      # tag, branch, or commit SHA
    local ref_type="${4:-}"  # "tag", "branch", or "commit" (auto-detected if empty)
    local target_dir="$REPO_DIR/$name"

    # Auto-detect ref type if not specified
    if [ -z "$ref_type" ]; then
        # Check if it looks like a SHA (hex string > 8 chars)
        if echo "$ref" | grep -qE '^[0-9a-f]{8,40}$'; then
            ref_type="commit"
        # Check if it has a dot (likely a version tag like "14.1.1" or "v5.1.0")
        elif echo "$ref" | grep -qE '^[0-9]+\.' || echo "$ref" | grep -qE '^v[0-9]'; then
            ref_type="tag"
        else
            ref_type="branch"
        fi
    fi

    if [ -d "$target_dir" ]; then
        echo "[pin_repo] $name already exists — checking ref..."
        cd "$target_dir"
        local current_ref
        current_ref=$(git rev-parse HEAD 2>/dev/null || echo "")
        if [ -n "$ref" ]; then
            local target_ref
            if [ "$ref_type" = "commit" ]; then
                target_ref="$ref"
            else
                # For tags/branches, get the commit they'd resolve to
                target_ref=$(git rev-parse "$ref" 2>/dev/null || echo "")
            fi
            if [ "$current_ref" != "$target_ref" ]; then
                echo "[pin_repo] WARNING: $name at $(git rev-parse --short HEAD), expected $ref — updating..."
                git fetch origin
                if [ "$ref_type" = "tag" ]; then
                    git fetch origin "refs/tags/$ref:refs/tags/$ref" --depth=1 2>/dev/null || true
                elif [ "$ref_type" = "branch" ]; then
                    git fetch origin "$ref" --depth=1 2>/dev/null || true
                fi
                git checkout "$ref" --force
            else
                echo "[pin_repo] $name at expected ref $ref"
            fi
        fi
        cd - > /dev/null
    else
        echo "[pin_repo] Cloning $name (ref: $ref, type: $ref_type)..."
        if [ -n "$ref" ]; then
            if [ "$ref_type" = "tag" ]; then
                # For tags: clone without --branch, then checkout tag
                git clone --depth=1 "$url" "$target_dir"
                cd "$target_dir"
                git fetch origin "refs/tags/$ref:refs/tags/$ref" --depth=1 2>/dev/null || true
                git checkout "$ref" --force
            elif [ "$ref_type" = "commit" ]; then
                # For specific commit: clone, fetch the commit, checkout
                git clone --depth=1 "$url" "$target_dir"
                cd "$target_dir"
                git fetch origin "$ref" --depth=1
                git checkout "$ref" --force
            else
                # For branch: use --branch
                git clone --depth=1 --branch "$ref" "$url" "$target_dir"
            fi
        else
            git clone --depth=1 "$url" "$target_dir"
        fi
        cd "$target_dir"
        echo "[pin_repo] $name pinned at $(git rev-parse --short HEAD)"
        cd - > /dev/null
    fi
}

echo "=== Provisioning CogniCode Sandbox Repos ==="
echo "Repo directory: $REPO_DIR"

# ─── Rust: serde-rs/serde ───────────────────────────────────────────────────
# Pinned at v1.0.195 — a stable, representative Rust crate
pin_repo \
    "serde" \
    "https://github.com/serde-rs/serde.git" \
    "v1.0.195" \
    ""  # No specific commit pin — branch is sufficient for Tier A fixture

# ─── Rust: ripgrep ──────────────────────────────────────────────────────────
# Pinned at 14.1.1 — a small, fast Rust CLI tool with real validation pipeline
pin_repo \
    "ripgrep" \
    "https://github.com/BurntSushi/ripgrep.git" \
    "14.1.1" \
    "tag"

# ─── Rust: anyhow ───────────────────────────────────────────────────────────
# Pinned at 1.0.86 — a tiny Rust error handling library with minimal dependencies
pin_repo \
    "anyhow" \
    "https://github.com/dtolnay/anyhow.git" \
    "1.0.86" \
    "tag"

# ─── Python: pallets/click ────────────────────────────────────────────────────
# Pinned at 8.1.7 — stable, well-tested CLI framework
pin_repo \
    "click" \
    "https://github.com/pallets/click.git" \
    "8.1.7" \
    ""

# ─── Python: urllib3 ────────────────────────────────────────────────────────
# Pinned at 2.1.0 — a second Python real repo beyond click
pin_repo \
    "urllib3" \
    "https://github.com/urllib3/urllib3.git" \
    "2.1.0" \
    ""

# ─── Python: requests ────────────────────────────────────────────────────────
# Pinned at v2.32.3 — the popular HTTP library for Python
pin_repo \
    "requests" \
    "https://github.com/psf/requests.git" \
    "v2.32.3" \
    "tag"

# ─── JavaScript: chalk ───────────────────────────────────────────────────────
# Pinned at v5.1.0 — a small, popular JS CLI tool
# Tier B real repo — used for JS smoke expansion
pin_repo \
    "chalk" \
    "https://github.com/chalk/chalk.git" \
    "v5.1.0" \
    ""

# ─── TypeScript: commander.js ───────────────────────────────────────────────
# Pinned at v11.0.0 — a popular TS CLI framework
# Tier B real repo — used for TS smoke expansion
pin_repo \
    "commander" \
    "https://github.com/tj/commander.js.git" \
    "v11.0.0" \
    ""

# ─── Go: spf13/cobra ─────────────────────────────────────────────────────────
# Pinned at v1.8.1 — a popular Go CLI framework
# Tier B real repo — used for Go smoke expansion
pin_repo \
    "go/cobra" \
    "https://github.com/spf13/cobra.git" \
    "v1.8.1" \
    ""

# ─── Go: charmbracelet/bubbletea ──────────────────────────────────────────────
# Pinned at v1.3.9 — a popular Go TUI framework
# Tier B real repo — used for Go smoke expansion
pin_repo \
    "go/bubbletea" \
    "https://github.com/charmbracelet/bubbletea.git" \
    "v1.3.9" \
    ""

# ─── Go: samber/lo ────────────────────────────────────────────────────────────
# Pinned at v1.43.0 — a popular Go utility library
# Tier B real repo — used for Go smoke expansion
pin_repo \
    "go/lo" \
    "https://github.com/samber/lo.git" \
    "v1.43.0" \
    ""

# ─── Java: spring-projects/spring-petclinic ────────────────────────────────────
# Pinned at main — a popular Java/Spring Boot demo app
# Tier B real repo — used for Java smoke expansion
pin_repo \
    "java/spring-petclinic" \
    "https://github.com/spring-projects/spring-petclinic.git" \
    "main" \
    ""

# ─── JavaScript: expressjs/express ─────────────────────────────────────────────
# Pinned at 4.21.0 — the popular Node.js web framework
# Tier B real repo — used for JS smoke expansion
pin_repo \
    "javascript/express" \
    "https://github.com/expressjs/express.git" \
    "4.21.0" \
    ""

# ─── TypeScript: colinhacks/zod ────────────────────────────────────────────────
# Pinned at v3.24.1 — a popular TypeScript schema validation library
# Tier B real repo — used for TS smoke expansion
pin_repo \
    "typescript/zod" \
    "https://github.com/colinhacks/zod.git" \
    "v3.24.1" \
    ""

# ─── Java: java-sample (Maven project from fixtures) ─────────────────────────
# Already provisioned at sandbox/fixtures/java-sample
# This is a minimal Maven project for Phase 3 capability probes
if [ ! -d "$REPO_DIR/java-sample" ]; then
    echo "[pin_repo] Symlinking java-sample fixture..."
    ln -sf "${SCRIPT_DIR}/../fixtures/java-sample" "$REPO_DIR/java-sample"
fi

echo "=== Repo provisioning complete ==="
echo "Repo directory contents:"
ls -la "$REPO_DIR/"
echo ""
echo "Note: Digest pins in container files must be updated separately when images change."
echo "JS/TS real repos (chalk, commander) need: npm ci --frozen-lockfile"
