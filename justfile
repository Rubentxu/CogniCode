# ============================================================================
# CogniCode — Justfile de automatización
# ============================================================================
#
# Uso:
#   just                  → Muestra todos los comandos disponibles
#   just build            → Build completo (server + WASM)
#   just run              → Build y arranca el dashboard
#   just test             → Ejecuta todos los tests
#   just doc              → Abre la documentación
#   just fmt-check        → Formato: cargo fmt --all --check (M0.7)
#   just docs-check       → Auditoría de rustdoc: cargo doc --no-deps (M0.8 audit)
#
# ============================================================================

# ─── Variables ────────────────────────────────────────────────────────────────

set dotenv-load
set positional-arguments := false

EXPLORER_UI_DIR := "apps/explorer-ui"
EXPLORER_API_BIN := "target/debug/cognicode-explorer-api"
EXPLORER_PORT := env_var_or_default("EXPLORER_PORT", "5180")
EXPLORER_API_PORT := env_var_or_default("EXPLORER_API_PORT", "3456")
EXPLORER_API_RELEASE := "target/release/explorer-api"
EXPLORER_REAL_API_PORT := env_var_or_default("EXPLORER_REAL_API_PORT", "8010")
PORT := EXPLORER_API_PORT
PROJECT_PATH := env_var_or_default("COGNICODE_PROJECT_PATH", "")
version := env_var_or_default("COGNICODE_VERSION", `sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1`)

# ─── Default ──────────────────────────────────────────────────────────────────

default:
    @just --list

# ─── Build ────────────────────────────────────────────────────────────────────

# Build everything: Explorer API + frontend
build: build-server

# Build only the Explorer API binary
build-server:
    @echo "🔨 Building Explorer API..."
    cargo build -p cognicode-runtime --bin explorer-api --release

# Build only the Explorer frontend
build-wasm:
    @echo "🔨 Building Explorer frontend..."
    cd {{ EXPLORER_UI_DIR }} && npm ci && npm run build

# Build musl binary with version
build-musl:
    #!/usr/bin/env bash
    set -euo pipefail
    VERSION=$(grep '^version = ' Cargo.toml | head -1 | sed 's/version = "//' | sed 's/"//')
    rustup target add x86_64-unknown-linux-musl --toolchain stable
    cargo build --release --target x86_64-unknown-linux-musl -p cognicode-mcp -p cognicode-runtime --bin explorer-api
    mkdir -p dist
    tar -czf dist/cognicode-${VERSION}-x86_64-unknown-linux-musl.tar.gz \
        -C target/x86_64-unknown-linux-musl/release/ explorer-api cognicode-mcp cognicode-runtime
    echo "✅ Built dist/cognicode-${VERSION}-x86_64-unknown-linux-musl.tar.gz"

# Build skill bundle tar.gz artifacts
bundle-skills:
    #!/usr/bin/env bash
    set -euo pipefail
    VERSION={{ version }}
    mkdir -p dist
    for skill in skills/*/; do
        name=$(basename "$skill")
        if [ ! -f "$skill/manifest.yaml" ]; then
            echo "⚠️  Skipping $name (no manifest.yaml)"
            continue
        fi
        echo "Bundling $name..."
        tar -czf "dist/${name}-${VERSION}.tar.gz" -C "$skill" .
        sha256=$(sha256sum "dist/${name}-${VERSION}.tar.gz" | cut -d' ' -f1)
        echo "  ✅ dist/${name}-${VERSION}.tar.gz"
        echo "  sha256: $sha256"
    done
    echo "Skill bundles built in dist/"

# Build in release mode
build-release: build-server

# Clean build artifacts
clean:
    @echo "🧹 Cleaning..."
    cargo clean
    rm -rf {{ EXPLORER_UI_DIR }}/dist
    echo "Cleaned"

# ─── Run ──────────────────────────────────────────────────────────────────────

# Build and start the Explorer API server
run: stop build-release
    @echo "🚀 Starting Explorer API on http://localhost:{{ PORT }}"
    @if curl -s --max-time 1 http://localhost:{{ PORT }}/health > /dev/null 2>&1; then \
        echo "❌ Port {{ PORT }} still in use. Try: just stop && just run"; exit 1; \
    fi
    ./{{ EXPLORER_API_RELEASE }} --listen 127.0.0.1:{{ PORT }}

# Start server (without rebuilding)
start: stop
    @echo "🚀 Starting Explorer API (no rebuild)..."
    @if curl -s --max-time 1 http://localhost:{{ PORT }}/health > /dev/null 2>&1; then \
        echo "❌ Port {{ PORT }} still in use. Try: just stop"; exit 1; \
    fi
    ./{{ EXPLORER_API_RELEASE }} --listen 127.0.0.1:{{ PORT }}

# Run in dev mode — one command to start everything: API + Frontend
dev:
    #!/usr/bin/env bash
    set -euo pipefail
    ROOT="$(cd "$(dirname "{{ justfile() }}")" && pwd)"
    API_BIN="$ROOT/{{ EXPLORER_API_RELEASE }}"
    UI_DIR="$ROOT/{{ EXPLORER_UI_DIR }}"
    API_PORT="{{ EXPLORER_API_PORT }}"
    UI_PORT="{{ EXPLORER_PORT }}"

    echo "═══════════════════════════════════════════════════════"
    echo "  CogniCode — Dev Mode"
    echo "═══════════════════════════════════════════════════════"
    echo ""

    # 1. Build API binary
    echo "🔨 [1/3] Building Explorer API..."
    cargo build -p cognicode-runtime --bin explorer-api --release
    echo "   ✅ API binary ready"

    # 2. Install frontend deps
    echo "📦 [2/3] Installing frontend deps..."
    (cd "$UI_DIR" && npm ci --prefer-offline 2>/dev/null || npm install)
    echo "   ✅ Frontend deps ready"

    # 4. Start both servers
    echo ""
    echo "═══════════════════════════════════════════════════════"
    echo "  Frontend:  http://localhost:$UI_PORT"
    echo "  API:       http://localhost:$API_PORT"
    echo ""
    echo "  Press Ctrl+C to stop everything"
    echo "═══════════════════════════════════════════════════════"
    echo ""

    # Start API in background
    "$API_BIN" --listen "127.0.0.1:$API_PORT" &
    API_PID=$!

    # Start frontend dev server in foreground (so Ctrl+C works naturally)
    cleanup() {
        echo ""
        echo "🛑 Stopping..."
        kill $API_PID 2>/dev/null || true
        wait $API_PID 2>/dev/null || true
        echo "✅ Done"
    }
    trap cleanup EXIT INT TERM
    (cd "$UI_DIR" && npx vite --host 127.0.0.1 --port "$UI_PORT")

# ─── Check ────────────────────────────────────────────────────────────────────

# Check compilation (fast, no binary output)
check:
    @echo "✅ Checking compilation..."
    cargo check -p cognicode-runtime --bin explorer-api

# Run clippy lints
lint:
    @echo "🔍 Running clippy..."
    cargo clippy -p cognicode-runtime --bin explorer-api -- -D warnings

# Format code
fmt:
    @echo "📝 Formatting..."
    cargo fmt

# Verify fmt does not drift (M0.7 audit gate)
fmt-check:
    @echo "🔎 Checking formatting..."
    cargo fmt --all --check

# Audit rustdoc warnings (M0.8 audit gate)
docs-check:
    @echo "🔎 Auditing rustdoc..."
    @cargo doc --workspace --no-deps 2>&1 | tee /tmp/rustdoc.log > /dev/null
    @echo "Total warnings:"
    @grep -cE "^warning:" /tmp/rustdoc.log
    @echo "Categories breakdown:"
    @grep -oE "rustdoc::[a-z_]+" /tmp/rustdoc.log | sort | uniq -c | sort -rn

# ─── Tests ────────────────────────────────────────────────────────────────────

# Run all tests (unit + integration)
test: test-unit test-integration

# Run all unit tests
test-unit:
    @echo "🧪 Running unit tests..."
    cargo test --workspace --lib

# Run all integration tests (with mocks)
test-integration:
    @echo "🧪 Running integration tests..."
    cargo test --workspace --tests

# The gate: unit tests + integration tests + mutants. All three, or it is red.
gate:
    @echo "🛡️  The gate..."
    bash scripts/test-gate.sh

# Mutation testing only. The scope lives in scripts/test-gate.sh, and this
# recipe calls that script rather than repeating the scope: two files naming
# the same crates is two sources of truth, and the second one rots.
mutants:
    @echo "🧬 Running mutants..."
    bash scripts/test-gate.sh --mutants

# One command per SDK release. `just sdk-release 0.103.0` bumps the version,
# rehearses, and opens the PR; after merging it, `just sdk-release --publish`
# publishes, verifies and tags.
sdk-release *ARGS:
    bash scripts/distribution/release-sdk.sh {{ARGS}}

# Publish the SDK crates at the workspace version if crates.io does not have it
# yet; does nothing otherwise. `just sdk-auto-release --check` only reports.
sdk-auto-release *ARGS:
    bash scripts/distribution/auto-release-sdk.sh {{ARGS}}

# Read-only: what would stop the SDK auto-release timer from running.
sdk-timer-check:
    bash scripts/distribution/check-sdk-timer.sh

# Dry run of the operator-side install of the SDK auto-release timer (add --apply by hand to write).
sdk-timer-install checkout:
    bash scripts/distribution/install-sdk-timer.sh --checkout {{checkout}}

# Measure the build times: cold, no-change, touch a test, touch the core.
# Not a test: it compiles the workspace four times, so it is asked for on
# purpose and it lives outside the suite. It measures in an isolated target
# directory and prints the sccache hit rate.
build-time:
    @echo "⏱️  Measuring the build times..."
    bash scripts/build-time.sh

# Measure the agent-facing surface (v2 goals vs legacy tools) against slo-budget.toml.
# On demand, outside the suite and the gate: every call is its own process with a
# time limit and a memory ceiling. `just harness-ab run --repos kotlin`, then `report`.
harness-ab *ARGS:
    python3 -I scripts/measure/harness_ab.py {{ARGS}}

# Run ignored tests (flaky, slow, requires external tools)
test-ignored:
    @echo "🧪 Running ignored tests (single-threaded)..."
    cargo test --workspace -- --include-ignored --test-threads=1 || \
    cargo test --workspace -- --ignored --test-threads=1

# Run all lifecycle tests (single-threaded to avoid lock contention)
test-lifecycle:
    @echo "🧪 Running lifecycle tests..."
    cargo test -p cognicode-cli --bin cogh lifecycle -- --test-threads=1

start-server:
    @if curl -s --max-time 2 http://localhost:{{ PORT }}/health > /dev/null 2>&1; then \
        echo "🔄 Server already running"; \
    else \
        echo "🔄 Starting server..."; \
        nohup ./{{ EXPLORER_API_RELEASE }} --listen 127.0.0.1:{{ PORT }} > /tmp/cognicode-server.log 2>&1 & \
        sleep 2; \
        echo "Server started"; \
    fi

# ─── API ───────────────────────────────────────────────────────────────────────

# Test API health endpoint
api-health:
    @curl -s --connect-timeout 5 --max-time 300 http://localhost:{{ PORT }}/health

# Register a project via API
api-register project_path project_name="":
    @echo "📋 Registering project..."
    @test -n "{{ project_name }}" && NAME="{{ project_name }}" || NAME="$$(basename {{ project_path }})"
    curl -s --connect-timeout 5 --max-time 300 -X POST http://localhost:{{ PORT }}/api/projects/register \
        -H "Content-Type: application/json" \
        -d "{\"name\": \"$$NAME\", \"path\": \"{{ project_path }}\"}" | python3 -m json.tool

# List projects via API
api-projects:
    @curl -s --connect-timeout 5 --max-time 300 http://localhost:{{ PORT }}/api/projects | python3 -m json.tool

# Run analysis via API
api-analyze project_path:
    @echo "🔍 Running analysis..."
    curl -s --connect-timeout 5 --max-time 300 -X POST http://localhost:{{ PORT }}/api/analysis \
        -H "Content-Type: application/json" \
        -d "{\"project_path\": \"{{ project_path }}\", \"quick\": true, \"changed_only\": true}" | python3 -m json.tool

# Validate project path
api-validate project_path:
    curl -s --connect-timeout 5 --max-time 300 -X POST http://localhost:{{ PORT }}/api/validate-path \
        -H "Content-Type: application/json" \
        -d "{\"project_path\": \"{{ project_path }}\"}" | python3 -m json.tool

# Get project history
api-history project_path:
    @ENCODED=$$(echo -n "{{ project_path }}" | python3 -c "import sys,urllib.parse; print(urllib.parse.quote(sys.stdin.read(), safe=''))") && \
     curl -s --connect-timeout 5 --max-time 300 "http://localhost:{{ PORT }}/api/projects/$$ENCODED/history" | python3 -m json.tool

# ─── Docs ──────────────────────────────────────────────────────────────────────

# Open documentation
doc:
    @echo "📖 Opening documentation..."
    @test -f docs/dashboard/README.md && echo "Documentation: docs/dashboard/README.md" || \
        echo "Documentation not found."

# Take screenshots for documentation
docs-screenshots:
    @echo "📸 Taking screenshots..."
    @echo "Screenshot capture is manual: run 'just explorer-dev' and capture with playwright-cli."

# ─── Docker ────────────────────────────────────────────────────────────────────

# Build Docker image
docker-build:
    @echo "🐳 Building Docker image..."
    docker build -t cognicode-explorer .

# Run Docker container
docker-run:
    @echo "🐳 Running Docker container..."
    docker run -p {{ PORT }}:{{ PORT }} cognicode-explorer

# ─── Git ───────────────────────────────────────────────────────────────────────

# Quick commit with message
commit msg:
    @echo "💾 Committing..."
    git add -A
    git commit -m "{{ msg }}"
    git push

# Push current branch
push:
    git push

# ─── Utils ─────────────────────────────────────────────────────────────────────

# Show project status
status:
    @echo "📊 Project Status"
    @echo "================"
    @echo "Explorer:   http://localhost:{{ EXPLORER_PORT }}"
    @echo "API:        http://localhost:{{ PORT }}"
    @echo "Health:     http://localhost:{{ PORT }}/health"
    @echo "Server PID: $$(pgrep -f explorer-api | head -1 || echo 'not running')"
    @echo ""

# Stop the server
stop:
    @echo "🛑 Stopping server..."
    @fuser -k {{ PORT }}/tcp 2>/dev/null || true
    @sleep 1
    @echo "✅ Port {{ PORT }} freed"

# Install dependencies
install:
    @echo "📦 Installing dependencies..."
    npm install 2>/dev/null || true
    npx playwright install chromium 2>/dev/null || true
    @echo "Dependencies installed"

# Watch for changes and rebuild server
watch-server:
    @echo "👀 Watching for changes..."
    cargo watch -x "build -p cognicode-runtime --bin explorer-api"

# Open dashboard in browser
open:
    @echo "🌐 Opening Explorer..."
    @xdg-open http://localhost:{{ EXPLORER_PORT }} 2>/dev/null || \
     open http://localhost:{{ EXPLORER_PORT }} 2>/dev/null || \
     echo "Open http://localhost:{{ EXPLORER_PORT }} in your browser"

# Full setup from scratch
setup: install build
    @echo "✅ Setup complete!"
    @echo "Run 'just run' to start the Explorer API."
    @echo "Then visit http://localhost:{{ EXPLORER_PORT }}"

# ============================================================================
# Explorer UI (React + TypeScript)
# ============================================================================

# One command to rule them all: API + Frontend
# Alias for `just dev` — kept for backwards compatibility.
explorer-local:
    @just dev

# Quick start with mock data (no PG, no API needed)
explorer-mock:
    @echo "🚀 Starting Explorer UI with mock data (no backend)..."
    cd {{ EXPLORER_UI_DIR }} && npm run dev:mock

# Stop all Explorer processes
explorer-stop:
    @echo "🛑 Stopping Explorer..."
    @fuser -k {{ EXPLORER_API_PORT }}/tcp 2>/dev/null || true
    @fuser -k {{ EXPLORER_PORT }}/tcp 2>/dev/null || true
    @echo "✅ Explorer stopped"

# Dev mode: frontend with MSW mocks (no backend needed)
explorer-dev:
    @echo "🚀 Starting Explorer UI with mock data..."
    cd {{ EXPLORER_UI_DIR }} && npm run dev:mock

# Dev mode: frontend + API server (requires built binary)
explorer-full:
    @echo "🚀 Starting Explorer full stack..."
    @echo "  Terminal 1 (API server):"
    @echo "    just explorer-api"
    @echo ""
    @echo "  Terminal 2 (Frontend with live API):"
    @echo "    cd {{ EXPLORER_UI_DIR }} && npm run dev -- --host 127.0.0.1 --port {{ EXPLORER_PORT }}"
    @echo ""
    @echo "  Frontend: http://127.0.0.1:{{ EXPLORER_PORT }}"
    @echo "  API:      http://127.0.0.1:{{ EXPLORER_API_PORT }}"

# Open and scan the current repo through the real Explorer API.
# Safe for git: tracked-file dirtiness is checked before/after.
explorer-real-ingest workspace="":
    bash scripts/explorer/real_ingest.sh "{{ workspace }}"

# Start the real Explorer API in the background.
# Logs/PID live under .tmp-explorer-real/ (gitignored).
explorer-real-api-up:
    bash scripts/explorer/real_api_up.sh "{{ EXPLORER_REAL_API_PORT }}"

# Stop the real Explorer API started by explorer-real-api-up.
explorer-real-api-stop:
    bash scripts/explorer/real_api_stop.sh "{{ EXPLORER_REAL_API_PORT }}"

# Smoke-check the real stack against this repo: ingest -> API health -> workspace open -> landing.
explorer-real-smoke workspace="": explorer-real-ingest explorer-real-api-up
    bash scripts/explorer/real_smoke.sh "{{ workspace }}" "{{ EXPLORER_REAL_API_PORT }}"

# Start the real Explorer UI (Vite) against the real API after a smoke bootstrap.
explorer-real-ui: explorer-real-smoke
    @echo "🚀 Starting Explorer UI against real API..."
    cd {{ EXPLORER_UI_DIR }} && EXPLORER_API_TARGET=http://127.0.0.1:{{ EXPLORER_REAL_API_PORT }} VITE_USE_MOCKS=false npm run dev:real

# Build and start the Explorer API server
explorer-api:
    @echo "🔨 Building Explorer API..."
    cargo build -p cognicode-runtime --bin explorer-api --release
    @echo "🚀 Starting Explorer API on http://127.0.0.1:{{ EXPLORER_API_PORT }}..."
    cargo run -p cognicode-runtime --bin explorer-api --release -- --listen 127.0.0.1:{{ EXPLORER_API_PORT }}

# Build Explorer frontend for production
explorer-build:
    @echo "📦 Building Explorer UI..."
    cd {{ EXPLORER_UI_DIR }} && npm ci && npm run build

# Run Explorer unit tests
explorer-test:
    @echo "🧪 Running Explorer UI tests..."
    cd {{ EXPLORER_UI_DIR }} && npm test

explorer-lint:
    @echo "🔍 Linting Explorer UI..."
    cd {{ EXPLORER_UI_DIR }} && npm run lint

# Explorer: run all checks (lint + unit)
explorer-check: explorer-lint explorer-test

# Explorer: capture screenshots for docs
explorer-screenshots:
    @echo "📸 Capturing Explorer screenshots..."
    @test -d docs/explorer-ui/screenshots || mkdir -p docs/explorer-ui/screenshots
    @echo "Run 'just explorer-dev' first, then use playwright-cli to capture."

# ─── Performance budget ──────────────────────────────────────────────────────

# Run the performance budget gate (bench + compare against perf-budget.toml)
perf:
    #!/usr/bin/env bash
    ./scripts/perf-budget-check.sh

# Run the raw Criterion benchmarks with bencher output (no budget check)
perf-bench:
    cargo bench -p cognicode-core --bench graph_benchmarks -- --output-format bencher

# ─── LSI M0 evidence (E36) ───────────────────────────────────────────────────

# LSI M0 golden fixtures: verify consumer outputs against committed goldens.
# Writes nothing. Pass mode="accept" to re-baseline intentionally changed output.
lsi-fixtures mode="check":
    #!/usr/bin/env bash
    set -euo pipefail
    if [ "{{ mode }}" = "check" ]; then
        python3 sandbox/scripts/capture_lsi_fixtures.py --check
    elif [ "{{ mode }}" = "accept" ]; then
        python3 sandbox/scripts/capture_lsi_fixtures.py --accept
    else
        echo "unknown mode '{{ mode }}' (expected check|accept)" >&2; exit 2
    fi

# LSI M0 benchmark baseline: capture the baseline artifact or compare against it.
# Pass mode="compare" (add threshold="25" to make regressions fail the recipe).
lsi-baseline mode="capture" threshold="":
    #!/usr/bin/env bash
    set -euo pipefail
    if [ "{{ mode }}" = "capture" ]; then
        python3 sandbox/scripts/lsi_bench_baseline.py capture
    elif [ "{{ mode }}" = "compare" ]; then
        if [ -n "{{ threshold }}" ]; then
            python3 sandbox/scripts/lsi_bench_baseline.py compare \
                --output sandbox/results/lsi-baseline/delta.json \
                --fail-above {{ threshold }}
        else
            python3 sandbox/scripts/lsi_bench_baseline.py compare \
                --output sandbox/results/lsi-baseline/delta.json
        fi
    else
        echo "unknown mode '{{ mode }}' (expected capture|compare)" >&2; exit 2
    fi

# LSI M2 equivalence harness (E37): legacy vs fact-derived CallGraph
# projections over the golden fixtures. Fixed argv; fails (non-zero exit)
# when any non-quarantined fixture scores below the declared threshold
# (0.99) or the entity-identity convention pin changes.
lsi-equivalence:
    #!/usr/bin/env bash
    set -euo pipefail
    cargo test -p cognicode-core --test it --features evidence-kernel -- equivalence_harness:: --nocapture

# LSI M3 identity benchmark (E38): tiered continuity matcher over the
# lsi-identity ground-truth fixtures plus the workspace isolation suite.
# Fixed argv; fails (non-zero exit) when any scoring gate is missed
# (precision >= 0.95, recall >= 0.90, line-shift retention == 1.00,
# move retention >= 0.99), the matcher-convention pin changes, a case
# verification fails, or a workspace collision is detected.
lsi-identity:
    #!/usr/bin/env bash
    set -euo pipefail
    cargo test -p cognicode-core --test it --test identity_benchmark --features evidence-kernel -- workspace_isolation:: --nocapture

# LSI M4 semantic provider conformance (E39): declared per-tier precision
# targets over sandbox/fixtures/lsi-providers (Rust/TS serverless S0/S1,
# Java availability-gated on the jdtls PATH probe — no spawn), plus the
# gated fact-bridge tier/provenance unit filters and the CP-5 cross-layer
# tie-break. Fixed argv; fails on any tier contradiction.
lsi-providers:
    #!/usr/bin/env bash
    set -euo pipefail
    cargo test -p cognicode-core --test it -- provider_conformance:: --nocapture
    cargo test -p cognicode-core --test it --features evidence-kernel -- cp5_tie_break:: --nocapture
    cargo test -p cognicode-core --lib fact_bridge --features evidence-kernel

# ─── Sandbox ─────────────────────────────────────────────────────────────────

sandbox-iac:
    #!/usr/bin/env bash
    set -e
    echo "=== Running IaC sandbox tests ==="
    cargo run -p cognicode-sandbox -- run sandbox/manifests/iac/

# Run full sandbox baseline (all manifests)
sandbox-all:
    #!/usr/bin/env bash
    set -e
    echo "=== Running full sandbox baseline ==="
    cargo run -p cognicode-sandbox -- run sandbox/manifests/

# Run sandbox with specific manifest path
sandbox-run manifests:
    #!/usr/bin/env bash
    set -e
    echo "=== Running sandbox: {{ manifests }} ==="
    cargo run -p cognicode-sandbox -- run {{ manifests }}

# Generate HTML smoke test report from the latest per-run campaign
# Falls back to the historical aggregate if no campaign exists
sandbox-report-html output="/tmp/cognicode-smoke-report.html":
    #!/usr/bin/env bash
    set -euo pipefail
    LATEST_RUN=$(ls -t sandbox/results-runs 2>/dev/null | head -1)
    if [ -n "$LATEST_RUN" ] && [ -f "sandbox/results-runs/$LATEST_RUN/report.html" ]; then
        RESULTS_DIR="sandbox/results-runs/$LATEST_RUN"
        echo "📊 Using latest per-run campaign: $LATEST_RUN"
    else
        RESULTS_DIR="sandbox/results"
        echo "📊 No campaign found, using historical aggregate from sandbox/results/"
    fi
    python3 sandbox/scripts/generate_html_report.py --results-dir "$RESULTS_DIR" --output "{{ output }}"
    echo "✅ Report: {{ output }}"
    xdg-open "{{ output }}" 2>/dev/null || open "{{ output }}" 2>/dev/null || echo "Open: {{ output }}"

# Run a per-run campaign: isolated results dir + per-run report
# Each invocation gets a unique run-id under sandbox/results-runs/
sandbox-run-campaign manifests:
    #!/usr/bin/env bash
    bash sandbox/scripts/run_campaign.sh {{ manifests }}

# Run stability / repeat testing: executes the same manifests N times and produces
# stability.json with flakiness analysis.
# Usage: just sandbox-stability <manifest-path> [repeat-count]
# Example: just sandbox-stability sandbox/manifests/tier_c_lua.yaml 3
sandbox-stability manifests repeat="3":
    #!/usr/bin/env bash
    set -euo pipefail
    echo "=== Stability run: {{ manifests }} x {{ repeat }} repeats ==="
    bash sandbox/scripts/run_campaign.sh --repeat {{ repeat }} {{ manifests }}
    echo "✅ Stability analysis complete"
    echo "   Report: sandbox/results-runs/<run-id>/report.html"
    echo "   Stability: sandbox/results-runs/<run-id>/stability.json"

# Run sandbox in dry-run mode (list scenarios without executing)
sandbox-plan manifests:
    #!/usr/bin/env bash
    echo "=== Sandbox plan: {{ manifests }} ==="
    cargo run -p cognicode-sandbox -- run {{ manifests }} --dry-run 2>&1 | head -50

# Show sandbox results summary
sandbox-results:
    #!/usr/bin/env bash
    set -e
    RESULTS_DIR="${1:-sandbox/results}"
    if [ ! -d "$RESULTS_DIR" ]; then
        echo "No results directory: $RESULTS_DIR"
        exit 1
    fi
    echo "=== Sandbox Results ==="
    # Show latest run
    LATEST=$(ls -t "$RESULTS_DIR"/*.jsonl 2>/dev/null | head -1)
    if [ -n "$LATEST" ]; then
        echo "Latest: $LATEST"
        wc -l < "$LATEST"
    else
        echo "No results found"
    fi

# E29 S1 spike — build + run + test LadybugDB lbug 0.19.0 (prebuilt static lib, no cmake needed)
spike-ladybug:
    cargo build --release --manifest-path crates/spike-ladybug/Cargo.toml
    cargo run --manifest-path crates/spike-ladybug/Cargo.toml --example s1_bootstrap
    cargo test --manifest-path crates/spike-ladybug/Cargo.toml --tests

# E29 S1 spike — clean cache + build artifacts (forces fresh prebuilt download)
spike-ladybug-clean:
    rm -rf crates/spike-ladybug/target crates/spike-ladybug/.cache

# E29 S2 spike — schema load + COPY FROM + query validation
spike-ladybug-s2:
    cargo run --manifest-path crates/spike-ladybug/Cargo.toml --example s2_schema_create
    cargo run --manifest-path crates/spike-ladybug/Cargo.toml --example s2_copy_from
    cargo run --manifest-path crates/spike-ladybug/Cargo.toml --example s2_query_validation
    cargo test --manifest-path crates/spike-ladybug/Cargo.toml --tests

# E29 S2 spike — clean S2 .lbdb artifacts
spike-ladybug-s2-clean:
    rm -f s2_schema.lbdb s2_copy_from.lbdb s2_query_validation.lbdb

# E29 S3 spike — run all S3 examples and tests end-to-end
spike-ladybug-s3:
    cargo run --manifest-path crates/spike-ladybug/Cargo.toml --example s3_lock_holder -- --mode=rw --path=/tmp/s3_e2e.lbdb --hold-secs=1
    cargo run --manifest-path crates/spike-ladybug/Cargo.toml --example s3_concurrency
    cargo test --manifest-path crates/spike-ladybug/Cargo.toml --test s3_concurrency
    cargo test --manifest-path crates/spike-ladybug/Cargo.toml --test s3_multi_process

# E29 S3 spike — clean S3 .lbdb artifacts
spike-ladybug-s3-clean:
    rm -f /tmp/s3_e2e.lbdb

# E29 S4 spike — run all S4 examples and tests end-to-end
spike-ladybug-s4:
    cargo run --manifest-path crates/spike-ladybug/Cargo.toml --example s4_writer -- --mode=clean --path=/tmp/s4_e2e.lbdb --rows=1000
    cargo test --manifest-path crates/spike-ladybug/Cargo.toml --test s4_crash_recovery

# E29 S4 spike — clean S4 .lbdb artifacts
spike-ladybug-s4-clean:
    rm -f /tmp/s4_e2e.lbdb

# E29 S5 spike — populate dual engines then run full benchmark tests
# Requires PG running at postgres://cognicode:cognicode@localhost:5432/cognicode
spike-ladybug-s5:
    @echo "=== S5: Populating databases ==="
    cargo run --manifest-path crates/spike-ladybug/Cargo.toml --example s5_populate -- \
        --lbug-path=/tmp/s5_full6.lbdb \
        --pg-url=postgres://cognicode:cognicode@localhost:5432/cognicode \
        --rows=10000
    @echo "=== S5: Running full latency benchmarks ==="
    cargo test --manifest-path crates/spike-ladybug/Cargo.toml --test s5_latency -- --nocapture

# E29 S5 spike — clean S5 artifacts
spike-ladybug-s5-clean:
    rm -f /tmp/s5_full6.lbdb
    psql postgres://cognicode:cognicode@localhost:5432/cognicode -c "DROP TABLE IF EXISTS graph_edges; DROP TABLE IF EXISTS graph_nodes;" 2>/dev/null || true

# E29 S6 spike — run S6 Cypher compatibility probes
spike-ladybug-s6:
    @echo "=== S6: Cypher compatibility probes ==="
    cargo run --manifest-path crates/spike-ladybug/Cargo.toml --example s6_cypher_compat

# E29 S6 spike — clean S6 artifacts
spike-ladybug-s6-clean:
    rm -f /tmp/s6_*.lbdb

# Build musl bundle artifacts (tar.gz)
bundle-musl: build-musl
    mkdir -p dist
    tar -czf dist/cognicode-{{version}}-x86_64-unknown-linux-musl.tar.gz \
        -C target/x86_64-unknown-linux-musl/release/cognicode-mcp \
        --strip-components=3 .

# ─── Release ──────────────────────────────────────────────────────────────────

# Create a draft GitHub release
release-draft tag="{{version}}":
    gh release create "{{ tag }}" --draft || true
    test -f dist/cognicode-{{version}}-x86_64-unknown-linux-musl.tar.gz && \
        gh release upload "{{ tag }}" dist/cognicode-{{version}}-x86_64-unknown-linux-musl.tar.gz || true

# Publish an existing draft release
release-publish tag="{{version}}":
    gh release edit "{{ tag }}" --draft=false

# ─── E2E tests ────────────────────────────────────────────────────────────────

# E2E: install in clean HOME (no pre-existing ~/.cognicode)
test-clean-home:
    #!/usr/bin/env bash
    set -e
    export HOME=/tmp/cognicode-test-home-$(date +%s)
    export XDG_CONFIG_HOME="$HOME/.config"
    export XDG_DATA_HOME="$HOME/.local/share"
    mkdir -p "$HOME"
    # Run install
    cargo run --release --bin cogh -- install --profile core
    # Verify tracker was created
    test -f "$HOME/.cognicode/tracker/version"
    # Verify shims were created
    test -f "$HOME/.local/bin/cognicode" -o -f "$HOME/.cognicode/shims/cognicode"
    echo "clean-HOME install: PASS"
    rm -rf "$HOME"

# Lint conventional commits since last tag
commit-lint:
    @git log --format="%s" $(git describe --tags --abbrev=0 HEAD^)..HEAD | while read line; do echo "$$line" | grep -qE "^(feat|fix|docs|style|refactor|test|chore|perf|ci|revert)(\(.+\))?: .+" || { echo "INVALID: $$line"; exit 1; }; done && echo "All commits conventional"

# Verify workspace version in Cargo.toml matches the latest semver tag
# Warning-only guardrail — never fails the build
verify-version-sync:
    #!/usr/bin/env bash
    set -euo pipefail

    # Extract workspace version: robust awk parsing of [workspace.package] block
    WORKSPACE_VERSION=`awk '/^\[workspace.package\]/{f=1} f && /^version *=/{print; exit}' Cargo.toml | grep -oE '[0-9]+\.[0-9]+\.[0-9]+'`

    # Get latest semver tag, strip leading 'v' prefix
    LATEST_TAG=`git tag --sort=-version:refname | head -1 | sed 's/^v//'`

    if [ "$WORKSPACE_VERSION" = "$LATEST_TAG" ]; then
        echo "✅ Version sync OK: workspace version $WORKSPACE_VERSION matches latest tag v$LATEST_TAG"
        exit 0
    else
        echo "::warning::version-sync: workspace version ($WORKSPACE_VERSION) differs from latest tag (v$LATEST_TAG)"
        exit 0
    fi

# Verify the release contract coherence (R9, support tiers, runners) in CI form
release-check:
    bash scripts/check-release-matrix.sh

# Generate the release artifacts for a staging directory of produced payloads.
# Derives BundleManifest v2, the ReleaseInventory and SHA256SUMS.
#   just release-generate staging release 0.95.0 v0.95.0 <source-commit>
release-generate staging out version tag source_commit:
    cargo run --release --bin cognicode-release -- generate \
        --staging {{staging}} --out {{out}} \
        --version {{version}} --tag {{tag}} --source-commit {{source_commit}}

# Verify a staged release against the e84 contract (R1-R9) without publishing.
#   just release-verify release 0.95.0 v0.95.0
release-verify staging version tag:
    cargo run --release --bin cognicode-release -- verify \
        --staging {{staging}} --version {{version}} --tag {{tag}}

# Show the canonical payload names for a platform (the per-lane ledger)
release-plan platform version:
    cargo run --release --bin cognicode-release -- plan \
        --platform {{platform}} --version {{version}}
