# ============================================================================
# CogniCode — Justfile de automatización
# ============================================================================
#
# Uso:
#   just                  → Muestra todos los comandos disponibles
#   just build            → Build completo (server + WASM)
#   just run              → Build y arranca el dashboard
#   just test             → Ejecuta todos los tests
#   just e2e              → Tests end-to-end con Playwright
#   just doc              → Abre la documentación
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

# Run in dev mode — one command to start everything: PG + API + Frontend
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
    echo "  PG:        localhost:5432/cognicode"
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

# ─── Tests ────────────────────────────────────────────────────────────────────

# Run all tests (unit + e2e)
test: test-unit test-e2e

# Run all unit tests
test-unit:
    @echo "🧪 Running unit tests..."
    cargo test --workspace --no-fail-fast

# Run ignored tests (flaky, slow, requires external tools)
test-ignored:
    @echo "🧪 Running ignored tests (single-threaded)..."
    cargo test --workspace -- --include-ignored --test-threads=1 || \
    cargo test --workspace -- --ignored --test-threads=1

# Run unit tests for a specific crate
test-crate crate:
    cargo test -p {{ crate }} --no-fail-fast

# Run end-to-end tests with Playwright
test-e2e: start-server
    @echo "🎭 Running e2e tests..."
    npx playwright test --config=tests/e2e/playwright.config.js --reporter=list

# Run e2e tests (reuse existing server)
test-e2e-quick:
    @echo "🎭 Running e2e tests (quick)..."
    npx playwright test --config=tests/e2e/playwright.config.js --reporter=list

# Start server for tests
start-server:
    @if curl -s --max-time 2 http://localhost:{{ PORT }}/health > /dev/null 2>&1; then \
        echo "🔄 Server already running"; \
    else \
        echo "🔄 Starting server..."; \
        nohup ./{{ EXPLORER_API_RELEASE }} --listen 127.0.0.1:{{ PORT }} > /tmp/cognicode-server.log 2>&1 & \
        sleep 2; \
        echo "Server started"; \
    fi

# Run e2e tests with Playwright UI (headed mode)
test-e2e-headed:
    @echo "🎭 Running e2e tests (headed)..."
    npx playwright test --config=tests/e2e/playwright.config.js --headed

# Run e2e test suite (Node.js independent)
test-e2e-suite:
    @echo "🎭 Running e2e suite (standalone)..."
    node tests/e2e/suite.js

# Show Playwright test report
test-report:
    @echo "📊 Opening test report..."
    npx playwright show-report tests/e2e/report/html 2>/dev/null || \
    echo "No HTML report found. Run 'just test-e2e' first."

# ─── API ───────────────────────────────────────────────────────────────────────

# Test API health endpoint
api-health:
    @curl -s http://localhost:{{ PORT }}/health

# Register a project via API
api-register project_path project_name="":
    @echo "📋 Registering project..."
    @test -n "{{ project_name }}" && NAME="{{ project_name }}" || NAME="$$(basename {{ project_path }})"
    curl -s -X POST http://localhost:{{ PORT }}/api/projects/register \
        -H "Content-Type: application/json" \
        -d "{\"name\": \"$$NAME\", \"path\": \"{{ project_path }}\"}" | python3 -m json.tool

# List projects via API
api-projects:
    @curl -s http://localhost:{{ PORT }}/api/projects | python3 -m json.tool

# Run analysis via API
api-analyze project_path:
    @echo "🔍 Running analysis..."
    curl -s -X POST http://localhost:{{ PORT }}/api/analysis \
        -H "Content-Type: application/json" \
        -d "{\"project_path\": \"{{ project_path }}\", \"quick\": true, \"changed_only\": true}" | python3 -m json.tool

# Validate project path
api-validate project_path:
    curl -s -X POST http://localhost:{{ PORT }}/api/validate-path \
        -H "Content-Type: application/json" \
        -d "{\"project_path\": \"{{ project_path }}\"}" | python3 -m json.tool

# Get project history
api-history project_path:
    @ENCODED=$$(echo -n "{{ project_path }}" | python3 -c "import sys,urllib.parse; print(urllib.parse.quote(sys.stdin.read(), safe=''))") && \
     curl -s "http://localhost:{{ PORT }}/api/projects/$$ENCODED/history" | python3 -m json.tool

# ─── Docs ──────────────────────────────────────────────────────────────────────

# Open documentation
doc:
    @echo "📖 Opening documentation..."
    @test -f docs/dashboard/README.md && echo "Documentation: docs/dashboard/README.md" || \
        echo "Documentation not found. Run 'just docs-screenshots' first."

# Take screenshots for documentation
docs-screenshots:
    @echo "📸 Taking screenshots..."
    @test -f tests/e2e/screenshots.js && node tests/e2e/screenshots.js || echo "Create tests/e2e/screenshots.js first"

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

# ─── CI (local-only) ───────────────────────────────────────────────────────────
#
# CI runs locally via `act` (https://github.com/nektos/act) backed by
# `podman`. There are NO GitHub-hosted runners in this project by policy
# (v1.0.0 readiness program decision). The workflow YAMLs live in
# `.github/workflows/` but only carry `workflow_dispatch:` / `workflow_call:`
# triggers, so they never execute on GitHub without an explicit trigger.
#
# Requirements:
#   - act (/usr/local/bin/act, 0.2.89+)
#   - podman with a running user socket (/run/user/1000/podman/podman.sock)
#   - ~/.config/act/actrc listing the catthehacker/ubuntu images
#
# Default workflow: regression-check.yml (T6 enforcement).

# T6 — fix(*) commits must include a regression test (local-only enforcement)
ci-t6:
    @echo "🛡️  T6 regression test check (local-only via act+podman)..."
    @test -x scripts/ci/check_regression_test.sh || chmod +x scripts/ci/check_regression_test.sh
    DOCKER_HOST=unix:///run/user/1000/podman/podman.sock \
        act -W .github/workflows/regression-check.yml workflow_dispatch

# Dry-run T6 check (no container, validates the workflow syntactically)
ci-t6-dry:
    @echo "🛡️  T6 dry-run (validates workflow + script)..."
    DOCKER_HOST=unix:///run/user/1000/podman/podman.sock \
        act -n -W .github/workflows/regression-check.yml workflow_dispatch

# Run all enabled CI workflows locally (currently just T6)
ci-local:
    @echo "🛡️  Running all CI workflows locally via act+podman..."
    DOCKER_HOST=unix:///run/user/1000/podman/podman.sock \
        act -W .github/workflows/regression-check.yml workflow_dispatch

# T7 — regenerate the per-scenario flaky log (sandbox/results/flaky_scenarios.{md,json})
scorecard-stability window_days="30":
    @echo "📊 T7 flaky-scenarios log (rolling window: {{window_days}} days)..."
    python3 sandbox/scripts/build_flaky_log.py --window-days {{window_days}}
    @echo "==> Inspect: sandbox/results/flaky_scenarios.md"
    @grep -E "Total scenarios|Passing|Failing|Quarantined" sandbox/results/flaky_scenarios.md | head -5

# T7 nightly cadence — regenerate the log, archive snapshot, score the gates
scorecard-nightly:
    @echo "📊 T7 nightly (log + archive + G6/G7 alert)..."
    python3 sandbox/scripts/build_flaky_log.py --window-days 30 --archive
    @echo "==> Running G6 (per-tool CV < 10%)..."
    @test -f sandbox/results/stability.json || { echo "ERROR: sandbox/results/stability.json missing"; exit 1; }
    python3 -c "import json; d=json.load(open('sandbox/results/stability.json')); cvs=[f.get('mean_cv') for f in d.get('families_runtorun',d.get('families',{})).values() if f.get('mean_cv') is not None]; max_cv=max(cvs) if cvs else 0; print(f'G6 max family CV = {max_cv*100:.2f}% (budget <10%)'); import sys; sys.exit(1 if max_cv>=0.10 else 0)"
    @echo "==> Inspect: sandbox/results/flaky_scenarios.md"
    @grep -E "Total scenarios|Passing|Failing|Quarantined" sandbox/results/flaky_scenarios.md | head -5

# E31-G — 3-consecutive-scorecard-runs counter (per ADR-031 §3)
scorecard-streak runs="sandbox/results/ci_smoke,sandbox/results/quality,sandbox/results/full_run":
    @echo "📈 E31-G scorecard streak — fresh run + counter update..."
    @python3 sandbox/scripts/release_scorecard.py \
        --runs "{{runs}}" \
        --stability sandbox/results/stability.json \
        --coverage-matrix sandbox/reports/conformance_matrix.yaml \
        --output sandbox/results/scorecard_run
    @python3 sandbox/scripts/scorecard_streak.py \
        --record sandbox/results/scorecard_run.json \
        --purpose nightly || true
    @echo "==> Current streak:"
    @python3 -c "import json; d=json.load(open('sandbox/results/scorecard_streak.json')); print(f'  {d[\"current_streak\"]}/{d[\"goal\"]} ({d[\"verdict\"]})')"

# E31-G status — show current streak state
scorecard-streak-status:
    @test -f sandbox/results/scorecard_streak.json || { echo "No streak recorded yet. Run: just scorecard-streak"; exit 0; }
    @echo "==> Current streak:"
    @python3 -c "import json; d=json.load(open('sandbox/results/scorecard_streak.json')); print(f'  {d[\"current_streak\"]}/{d[\"goal\"]} ({d[\"verdict\"]})'); h=d.get('history',[]); print(f'  Last {min(5,len(h))} of {len(h)} runs:'); [print(f'    {x[\"at\"][:19]}  {x[\"purpose\"]:15s}  {x[\"verdict\"]:6s}  @{x[\"streak_after\"]}') for x in h[-5:]]"

# E31 audit — verify E31 program deliverables + tag chain integrity
post-e31-audit:
    @bash sandbox/scripts/post_e31_audit.sh

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

# One command to rule them all: PG + API + Frontend
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

# Build the current repo into PostgreSQL via the real MCP server.
# Safe for git: ingest persists in local PostgreSQL only; tracked-file dirtiness is checked before/after.
explorer-real-ingest workspace="":
    bash scripts/explorer/real_ingest.sh "{{ workspace }}"

# Start the real Explorer API against PostgreSQL in the background.
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

# Run Explorer E2E tests
explorer-e2e:
    @echo "🎭 Running Explorer E2E tests..."
    cd {{ EXPLORER_UI_DIR }} && npm run test:e2e

# Run Explorer E2E N times with flakiness report (default: 3 repeats)
# Usage: just explorer-e2e-stability 3
explorer-e2e-stability repeat="3":
    @echo "🔁 Explorer E2E stability run ({{ repeat }} repeats)..."
    bash {{ EXPLORER_UI_DIR }}/scripts/run_e2e_campaign.sh {{ repeat }}

# Generate HTML report from the latest Explorer E2E campaign
# Usage: just explorer-e2e-report [/path/to/output.html]
explorer-e2e-report output="{{ EXPLORER_UI_DIR }}/e2e-runs/latest/report.html":
    @echo "📊 Generating Explorer E2E HTML report..."
    python3 {{ EXPLORER_UI_DIR }}/scripts/generate_e2e_report.py --output {{ output }}
    @echo "✅ Report: {{ output }}"

# Run Explorer lint
explorer-lint:
    @echo "🔍 Linting Explorer UI..."
    cd {{ EXPLORER_UI_DIR }} && npm run lint

# Explorer: run all checks (lint + unit + e2e)
explorer-check: explorer-lint explorer-test explorer-e2e

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

# ─── Sandbox ─────────────────────────────────────────────────────────────────

sandbox-iac:
    #!/usr/bin/env bash
    set -e
    export DATABASE_URL="postgres://cognicode:cognicode@localhost:5432/cognicode"
    echo "=== Running IaC sandbox tests ==="
    echo "DATABASE_URL=$DATABASE_URL"
    cargo run -p cognicode-sandbox -- run sandbox/manifests/iac/

# Run full sandbox baseline (all manifests — requires PG)
sandbox-all:
    #!/usr/bin/env bash
    set -e
    export DATABASE_URL="postgres://cognicode:cognicode@localhost:5432/cognicode"
    echo "=== Running full sandbox baseline ==="
    echo "DATABASE_URL=$DATABASE_URL"
    cargo run -p cognicode-sandbox -- run sandbox/manifests/

# Run sandbox with specific manifest path
sandbox-run manifests:
    #!/usr/bin/env bash
    set -e
    export DATABASE_URL="postgres://cognicode:cognicode@localhost:5432/cognicode"
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
    export DATABASE_URL="postgres://cognicode:cognicode@localhost:5432/cognicode"
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
