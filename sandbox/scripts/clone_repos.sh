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
    "03eec42c3313b36da416be1486e9ecac345784d5" \
    "commit"

# ─── Rust: ripgrep ──────────────────────────────────────────────────────────
# Pinned at 14.1.1 — a small, fast Rust CLI tool with real validation pipeline
pin_repo \
    "ripgrep" \
    "https://github.com/BurntSushi/ripgrep.git" \
    "4649aa9700619f94cf9c66876e9549d83420e16c" \
    "commit"

# ─── Rust: anyhow ───────────────────────────────────────────────────────────
# Pinned at 1.0.86 — a tiny Rust error handling library with minimal dependencies
pin_repo \
    "anyhow" \
    "https://github.com/dtolnay/anyhow.git" \
    "8ea1819c4c7829d0eb09e54a52806f382b8d445b" \
    "commit"

# ─── Python: pallets/click ────────────────────────────────────────────────────
# Pinned at 8.1.7 — stable, well-tested CLI framework
pin_repo \
    "click" \
    "https://github.com/pallets/click.git" \
    "874ca2bc1c30d93a4ac6e36a15ed685eafe89097" \
    "commit"

# ─── Python: urllib3 ────────────────────────────────────────────────────────
# Pinned at 2.1.0 — a second Python real repo beyond click
pin_repo \
    "urllib3" \
    "https://github.com/urllib3/urllib3.git" \
    "69be2992f8a25a1f27e49f339e4d5b98dec07462" \
    "commit"

# ─── Python: requests ────────────────────────────────────────────────────────
# Pinned at v2.32.3 — the popular HTTP library for Python
pin_repo \
    "requests" \
    "https://github.com/psf/requests.git" \
    "0e322af87745eff34caffe4df68456ebc20d9068" \
    "commit"

# ─── JavaScript: chalk ───────────────────────────────────────────────────────
# Pinned at v5.1.0 — a small, popular JS CLI tool
# Tier B real repo — used for JS smoke expansion
pin_repo \
    "chalk" \
    "https://github.com/chalk/chalk.git" \
    "92c55db46f2396c18764e55e6a52dcb49884a42b" \
    "commit"

# ─── TypeScript: commander.js ───────────────────────────────────────────────
# Pinned at v11.0.0 — a popular TS CLI framework
# Tier B real repo — used for TS smoke expansion
pin_repo \
    "commander" \
    "https://github.com/tj/commander.js.git" \
    "4ef19faac1564743d8c7e3ce89ef8d190e1551b4" \
    "commit"

# ─── Go: spf13/cobra ─────────────────────────────────────────────────────────
# Pinned at v1.8.1 — a popular Go CLI framework
# Tier B real repo — used for Go smoke expansion
pin_repo \
    "go/cobra" \
    "https://github.com/spf13/cobra.git" \
    "e94f6d0dd9a5e5738dca6bce03c4b1207ffbc0ec" \
    "commit"

# ─── Go: charmbracelet/bubbletea ──────────────────────────────────────────────
# Pinned at v1.3.9 — a popular Go TUI framework
# Tier B real repo — used for Go smoke expansion
pin_repo \
    "go/bubbletea" \
    "https://github.com/charmbracelet/bubbletea.git" \
    "ffa05021909e14c478cbe138ca78effbea04e4e0" \
    "commit"

# ─── Go: samber/lo ────────────────────────────────────────────────────────────
# Pinned at v1.43.0 — a popular Go utility library
# Tier B real repo — used for Go smoke expansion
pin_repo \
    "go/lo" \
    "https://github.com/samber/lo.git" \
    "35e49f2c9607a7f7f6cde872a42d8718d9c3d053" \
    "commit"

# ─── Java: spring-projects/spring-petclinic ────────────────────────────────────
# Pinned to concrete SHA edf4db28affcc4741c79850a3d95bc3f177b5ff9 (reproducibility)
# Tier B real repo — used for Java smoke expansion
pin_repo \
    "java/spring-petclinic" \
    "https://github.com/spring-projects/spring-petclinic.git" \
    "edf4db28affcc4741c79850a3d95bc3f177b5ff9" \
    "commit"

# ─── JavaScript: expressjs/express ─────────────────────────────────────────────
# Pinned at 4.21.0 — the popular Node.js web framework
# Tier B real repo — used for JS smoke expansion
pin_repo \
    "javascript/express" \
    "https://github.com/expressjs/express.git" \
    "7e562c6d8daddff4604f8efaaf9db2cf98c6dcff" \
    "commit"

# ─── TypeScript: colinhacks/zod ────────────────────────────────────────────────
# Pinned at v3.24.1 — a popular TypeScript schema validation library
# Tier B real repo — used for TS smoke expansion
pin_repo \
    "typescript/zod" \
    "https://github.com/colinhacks/zod.git" \
    "65adeeacef0274abbda5438470a3d2bfd376256d" \
    "commit"

# ─── Ruby: sinatra/sinatra ─────────────────────────────────────────────────────
# Pinned at v4.1.0 — lightweight Ruby web framework (~2K LOC)
pin_repo \
    "ruby/sinatra" \
    "https://github.com/sinatra/sinatra.git" \
    "73f3291d114b5b211e067263eeb9c0e197fe8500" \
    "commit"

# ─── PHP: slimphp/Slim ─────────────────────────────────────────────────────────
# Pinned at 4.14.0 — PHP micro-framework (~4K LOC)
pin_repo \
    "php/slim" \
    "https://github.com/slimphp/Slim.git" \
    "5943393b88716eb9e82c4161caa956af63423913" \
    "commit"

# ─── C: redis/hiredis ──────────────────────────────────────────────────────────
# Pinned at v1.2.0 — minimal C Redis client (~3K LOC)
pin_repo \
    "c/hiredis" \
    "https://github.com/redis/hiredis.git" \
    "60e5075d4ac77424809f855ba3e398df7aacefe8" \
    "commit"

# ─── C++: nlohmann/json ────────────────────────────────────────────────────────
# Pinned at v3.11.3 — single-header JSON library (~25K LOC but one file)
pin_repo \
    "cpp/json" \
    "https://github.com/nlohmann/json.git" \
    "9cca280a4d0ccf0c08f47a99aa71d1b0e52f8d03" \
    "commit"

# ─── Swift: apple/swift-argument-parser ────────────────────────────────────────
# Pinned at 1.5.0 — Swift CLI argument parser (~3K LOC)
pin_repo \
    "swift/argument-parser" \
    "https://github.com/apple/swift-argument-parser.git" \
    "41982a3656a71c768319979febd796c6fd111d5c" \
    "commit"

# ─── Elixir: elixir-lang/elixir ────────────────────────────────────────────────
# Pinned at v1.18.2 — Elixir standard library (small core)
pin_repo \
    "elixir/elixir" \
    "https://github.com/elixir-lang/elixir.git" \
    "175c8243b23c4cfcaaa99e60b030085bfef8e9a0" \
    "commit"

# ─── C#: dotnet/roslyn ─────────────────────────────────────────────────────────
# Pinned at a stable tag — C# compiler
pin_repo \
    "csharp/roslyn" \
    "https://github.com/dotnet/roslyn.git" \
    "f33ae3e34c00d93ae70abf2e8f5d38ba45563405" \
    "commit"

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

# ─── C#: spectreconsole/spectre.console ──────────────────────────────────────
# Pinned at main — compact .NET library (~5K LOC, 12MB shallow)
pin_repo \
    "csharp/spectre-console" \
    "https://github.com/spectreconsole/spectre.console.git" \
    "0acc92fada6c42f13984e79c2b5f3d993bdfb099" \
    "commit"
