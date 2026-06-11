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

set dotenv-load := true
set positional-arguments := false

EXPLORER_UI_DIR := "apps/explorer-ui"
EXPLORER_API_BIN := "target/debug/cognicode-explorer-api"
EXPLORER_PORT := env_var_or_default("EXPLORER_PORT", "5173")
EXPLORER_API_PORT := env_var_or_default("EXPLORER_API_PORT", "3456")
DASHBOARD_DIR := "crates/cognicode-dashboard"
DIST_DIR := DASHBOARD_DIR / "dist"
SERVER_BIN := "target/release/cognicode-dashboard-server"
PORT := env_var_or_default("PORT", "3000")
PROJECT_PATH := env_var_or_default("COGNICODE_PROJECT_PATH", "")

# ─── Default ──────────────────────────────────────────────────────────────────

default:
    @just --list

# ─── Build ────────────────────────────────────────────────────────────────────

# Build everything: server + WASM frontend
build: build-server build-wasm copy-assets

# Build only the server binary
build-server:
    @echo "🔨 Building server..."
    cargo build --bin cognicode-dashboard-server --features server

# Build only the WASM frontend
build-wasm:
    @echo "🔨 Building WASM frontend..."
    cd {{DASHBOARD_DIR}} && trunk build --no-default-features

# Copy style assets to dist directory
copy-assets:
    @echo "📋 Copying assets..."
    cp -r {{DASHBOARD_DIR}}/style {{DIST_DIR}}/style/

# Build in release mode
build-release:
    @echo "🔨 Building release..."
    cargo build --release --bin cognicode-dashboard-server --features server
    cd {{DASHBOARD_DIR}} && trunk build --release --no-default-features
    cp -r {{DASHBOARD_DIR}}/style {{DIST_DIR}}/style/

# Clean build artifacts
clean:
    @echo "🧹 Cleaning..."
    cargo clean
    rm -rf {{DIST_DIR}}/*
    echo "Cleaned"

# ─── Run ──────────────────────────────────────────────────────────────────────

# Build and start the dashboard server
run: stop build-release copy-assets
    @echo "🚀 Starting dashboard on http://localhost:{{PORT}}"
    @if curl -s --max-time 1 http://localhost:{{PORT}}/health > /dev/null 2>&1; then \
        echo "❌ Port {{PORT}} still in use. Try: just stop && just run"; exit 1; \
    fi
    @if [ "{{PROJECT_PATH}}" != "" ]; then \
        echo "📂 Auto-discovering project: {{PROJECT_PATH}}"; \
    fi
    DIST_DIR={{DIST_DIR}} COGNICODE_PROJECT_PATH={{PROJECT_PATH}} ./{{SERVER_BIN}}

# Start server (without rebuilding)
start: stop
    @echo "🚀 Starting dashboard (no rebuild)..."
    @if curl -s --max-time 1 http://localhost:{{PORT}}/health > /dev/null 2>&1; then \
        echo "❌ Port {{PORT}} still in use. Try: lsof -i :{{PORT}}"; exit 1; \
    fi
    @if [ "{{PROJECT_PATH}}" != "" ]; then \
        echo "📂 Auto-discovering project: {{PROJECT_PATH}}"; \
    fi
    DIST_DIR={{DIST_DIR}} COGNICODE_PROJECT_PATH={{PROJECT_PATH}} ./{{SERVER_BIN}}

# Run in dev mode (trunk serve for frontend + cargo run for server)
dev:
    @echo "🔧 Dev mode — run these in separate terminals:"
    @echo ""
    @echo "  Terminal 1 (API server):"
    @echo "    just start"
    @echo ""
    @echo "  Terminal 2 (WASM hot-reload):"
    @echo "    cd {{DASHBOARD_DIR}} && trunk serve --no-default-features"
    @echo ""
    @echo "  Frontend: http://localhost:8080 (trunk proxy)"
    @echo "  API:      http://localhost:{{PORT}}"

# ─── Check ────────────────────────────────────────────────────────────────────

# Check compilation (fast, no binary output)
check:
    @echo "✅ Checking compilation..."
    cargo check --bin cognicode-dashboard-server
    cd {{DASHBOARD_DIR}} && cargo check --lib

# Run clippy lints
lint:
    @echo "🔍 Running clippy..."
    cargo clippy --bin cognicode-dashboard-server -- -D warnings
    cd {{DASHBOARD_DIR}} && cargo clippy --lib -- -D warnings

# Format code
fmt:
    @echo "📝 Formatting..."
    cargo fmt

# ─── Tests ────────────────────────────────────────────────────────────────────

# Run all tests (unit + e2e)
test: test-unit test-e2e

# Run unit tests
test-unit:
    @echo "🧪 Running unit tests..."
    cd {{DASHBOARD_DIR}} && cargo test --lib

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
    @if curl -s --max-time 2 http://localhost:{{PORT}}/health > /dev/null 2>&1; then \
        echo "🔄 Server already running"; \
    else \
        echo "🔄 Starting server..."; \
        DIST_DIR={{DIST_DIR}} COGNICODE_PROJECT_PATH={{PROJECT_PATH}} nohup ./{{SERVER_BIN}} > /tmp/cognicode-server.log 2>&1 & \
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
    @curl -s http://localhost:{{PORT}}/health

# Register a project via API
api-register project_path project_name="":
    @echo "📋 Registering project..."
    @test -n "{{project_name}}" && NAME="{{project_name}}" || NAME="$$(basename {{project_path}})"
    curl -s -X POST http://localhost:{{PORT}}/api/projects/register \
        -H "Content-Type: application/json" \
        -d "{\"name\": \"$$NAME\", \"path\": \"{{project_path}}\"}" | python3 -m json.tool

# List projects via API
api-projects:
    @curl -s http://localhost:{{PORT}}/api/projects | python3 -m json.tool

# Run analysis via API
api-analyze project_path:
    @echo "🔍 Running analysis..."
    curl -s -X POST http://localhost:{{PORT}}/api/analysis \
        -H "Content-Type: application/json" \
        -d "{\"project_path\": \"{{project_path}}\", \"quick\": true, \"changed_only\": true}" | python3 -m json.tool

# Validate project path
api-validate project_path:
    curl -s -X POST http://localhost:{{PORT}}/api/validate-path \
        -H "Content-Type: application/json" \
        -d "{\"project_path\": \"{{project_path}}\"}" | python3 -m json.tool

# Get project history
api-history project_path:
    @ENCODED=$$(echo -n "{{project_path}}" | python3 -c "import sys,urllib.parse; print(urllib.parse.quote(sys.stdin.read(), safe=''))") && \
     curl -s "http://localhost:{{PORT}}/api/projects/$$ENCODED/history" | python3 -m json.tool

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
    docker build -t cognicode-dashboard -f Dockerfile.dashboard .

# Run Docker container
docker-run:
    @echo "🐳 Running Docker container..."
    docker run -p {{PORT}}:{{PORT}} -e PORT={{PORT}} -v $(pwd)/{{DIST_DIR}}:/app/dist cognicode-dashboard

# ─── Git ───────────────────────────────────────────────────────────────────────

# Quick commit with message
commit msg:
    @echo "💾 Committing..."
    git add -A
    git commit -m "{{msg}}"
    git push

# Push current branch
push:
    git push

# ─── Utils ─────────────────────────────────────────────────────────────────────

# Show project status
status:
    @echo "📊 Project Status"
    @echo "================"
    @echo "Dashboard:  http://localhost:{{PORT}}"
    @echo "Health:     http://localhost:{{PORT}}/health"
    @echo "Tests:      tests/e2e/dashboard.spec.js (61 tests)"
    @echo "Docs:       docs/dashboard/README.md"
    @echo "Server PID: $$(pgrep -f cognicode-dashboard-server | head -1 || echo 'not running')"
    @echo ""

# Stop the server
stop:
    @echo "🛑 Stopping server..."
    @fuser -k {{PORT}}/tcp 2>/dev/null || true
    @sleep 1
    @echo "✅ Port {{PORT}} freed"

# Install dependencies
install:
    @echo "📦 Installing dependencies..."
    rustup target add wasm32-unknown-unknown 2>/dev/null || true
    cargo install trunk 2>/dev/null || true
    npm install 2>/dev/null || true
    npx playwright install chromium 2>/dev/null || true
    @echo "Dependencies installed"

# Watch for changes and rebuild server
watch-server:
    @echo "👀 Watching for changes..."
    cargo watch -x "build --bin cognicode-dashboard-server"

# Open dashboard in browser
open:
    @echo "🌐 Opening dashboard..."
    @xdg-open http://localhost:{{PORT}} 2>/dev/null || \
     open http://localhost:{{PORT}} 2>/dev/null || \
     echo "Open http://localhost:{{PORT}} in your browser"

# Full setup from scratch
setup: install build
    @echo "✅ Setup complete!"
    @echo "Run 'just run' to start the dashboard."
    @echo "Then visit http://localhost:{{PORT}}"

# ============================================================================
# Explorer UI (React + TypeScript)
# ============================================================================

# One command to rule them all: PG + API + Frontend
# Starts PostgreSQL, builds the API, installs npm deps, and
# launches both the API server and the React dev server.
# Frontend: http://localhost:{EXPLORER_PORT}  (default 5173)
# API:      http://localhost:{EXPLORER_API_PORT} (default 3456)
explorer-local:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "═══════════════════════════════════════════════════════"
    echo "  CogniCode Explorer — Local Dev Environment"
    echo "═══════════════════════════════════════════════════════"
    echo ""

    # 1. PostgreSQL
    echo "🐘 [1/4] Starting PostgreSQL..."
    if docker compose exec -T postgres pg_isready -U cognicode -d cognicode > /dev/null 2>&1; then
        echo "   ✅ PostgreSQL already running"
    else
        docker compose up -d postgres
        echo "   ⏳ Waiting for PostgreSQL..."
        for i in $(seq 1 30); do
            if docker compose exec -T postgres pg_isready -U cognicode -d cognicode > /dev/null 2>&1; then
                echo "   ✅ PostgreSQL ready"
                break
            fi
            sleep 1
        done
    fi

    # 2. Build API binary
    echo "🔨 [2/4] Building Explorer API..."
    cargo build -p cognicode-explorer --bin cognicode-explorer-api --features multimodal
    echo "   ✅ API binary ready"

    # 3. Install frontend deps
    echo "📦 [3/4] Installing frontend deps..."
    cd {{EXPLORER_UI_DIR}} && npm ci --prefer-offline 2>/dev/null || npm install
    echo "   ✅ Frontend deps ready"

    # 4. Start both servers
    echo "🚀 [4/4] Starting servers..."
    echo ""
    echo "═══════════════════════════════════════════════════════"
    echo "  Frontend:  http://localhost:{{EXPLORER_PORT}}"
    echo "  API:       http://localhost:{{EXPLORER_API_PORT}}"
    echo "  PG:        localhost:5432/cognicode"
    echo ""
    echo "  Press Ctrl+C to stop everything"
    echo "═══════════════════════════════════════════════════════"
    echo ""

    # Start API in background
    DATABASE_URL=postgres://cognicode:cognicode@localhost:5432/cognicode \
        ./target/debug/cognicode-explorer-api &
    API_PID=$!

    # Start frontend dev server in background
    cd {{EXPLORER_UI_DIR}} && VITE_API_URL=http://localhost:{{EXPLORER_API_PORT}} npx vite --host 127.0.0.1 --port {{EXPLORER_PORT}} &
    UI_PID=$!

    # Trap Ctrl+C to kill both
    cleanup() {
        echo ""
        echo "🛑 Stopping servers..."
        kill $API_PID 2>/dev/null || true
        kill $UI_PID 2>/dev/null || true
        wait $API_PID 2>/dev/null || true
        wait $UI_PID 2>/dev/null || true
        echo "✅ Servers stopped"
    }
    trap cleanup EXIT INT TERM

    # Wait for either to exit
    wait -n $API_PID $UI_PID 2>/dev/null || true

# Quick start with mock data (no PG, no API needed)
explorer-mock:
    @echo "🚀 Starting Explorer UI with mock data (no backend)..."
    cd {{EXPLORER_UI_DIR}} && npm run dev:mock

# Stop all Explorer processes
explorer-stop:
    @echo "🛑 Stopping Explorer..."
    @fuser -k {{EXPLORER_API_PORT}}/tcp 2>/dev/null || true
    @fuser -k {{EXPLORER_PORT}}/tcp 2>/dev/null || true
    @echo "✅ Explorer stopped"

# Dev mode: frontend with MSW mocks (no backend needed)
explorer-dev:
    @echo "🚀 Starting Explorer UI with mock data..."
    cd {{EXPLORER_UI_DIR}} && npm run dev:mock

# Dev mode: frontend + API server (requires built binary)
explorer-full:
    @echo "🚀 Starting Explorer full stack..."
    @echo "  Terminal 1 (API server):"
    @echo "    just explorer-api"
    @echo ""
    @echo "  Terminal 2 (Frontend with live API):"
    @echo "    cd {{EXPLORER_UI_DIR}} && npm run dev -- --host 127.0.0.1 --port {{EXPLORER_PORT}}"
    @echo ""
    @echo "  Frontend: http://127.0.0.1:{{EXPLORER_PORT}}"
    @echo "  API:      http://127.0.0.1:{{EXPLORER_API_PORT}}"

# Build and start the Explorer API server
explorer-api:
    @echo "🔨 Building Explorer API..."
    cargo build -p cognicode-explorer --bin cognicode-explorer-api
    @echo "🚀 Starting Explorer API on http://127.0.0.1:{{EXPLORER_API_PORT}}..."
    cargo run -p cognicode-explorer --bin cognicode-explorer-api

# Build Explorer frontend for production
explorer-build:
    @echo "📦 Building Explorer UI..."
    cd {{EXPLORER_UI_DIR}} && npm ci && npm run build

# Run Explorer unit tests
explorer-test:
    @echo "🧪 Running Explorer UI tests..."
    cd {{EXPLORER_UI_DIR}} && npm test

# Run Explorer E2E tests
explorer-e2e:
    @echo "🎭 Running Explorer E2E tests..."
    cd {{EXPLORER_UI_DIR}} && npm run test:e2e

# Run Explorer lint
explorer-lint:
    @echo "🔍 Linting Explorer UI..."
    cd {{EXPLORER_UI_DIR}} && npm run lint

# Explorer: run all checks (lint + unit + e2e)
explorer-check: explorer-lint explorer-test explorer-e2e

# Explorer: capture screenshots for docs
explorer-screenshots:
    @echo "📸 Capturing Explorer screenshots..."
    @test -d docs/explorer-ui/screenshots || mkdir -p docs/explorer-ui/screenshots
    @echo "Run 'just explorer-dev' first, then use playwright-cli to capture."

# ─── PostgreSQL local dev (PR 3 of postgres-default-config) ───────────────────

# Bring up a local PostgreSQL 16 instance via docker-compose, wait
# until it accepts connections, and print the URL the explorer
# binaries should use. Subsequent runs reuse the named volume
# `cognicode_pg_data`, so data is preserved across restarts.
dev-pg:
    @echo "🐘 Starting PostgreSQL 16 (docker compose)..."
    docker compose up -d postgres
    @echo "⏳ Waiting for pg_isready..."
    @for i in $(seq 1 30); do \
        if docker compose exec -T postgres pg_isready -U cognicode -d cognicode > /dev/null 2>&1; then \
            echo "✅ PostgreSQL is ready."; \
            echo "DATABASE_URL=postgres://cognicode:cognicode@localhost:5432/cognicode"; \
            exit 0; \
        fi; \
        sleep 1; \
    done; \
    echo "❌ PostgreSQL failed to become ready in 30s"; exit 1

# Run the workspace test suite with TEST_DATABASE_URL pointing at
# the local dev stack. SQLite-only tests are not compiled (they
# require `--features sqlite` on the explorer crate).
test-pg:
    @echo "🧪 Running cargo test --workspace with TEST_DATABASE_URL..."
    @if ! docker compose exec -T postgres pg_isready -U cognicode -d cognicode > /dev/null 2>&1; then \
        echo "❌ PostgreSQL is not running. Start it with: just dev-pg"; exit 1; \
    fi
    TEST_DATABASE_URL=postgres://cognicode:cognicode@localhost:5432/cognicode \
        cargo test --workspace

# Tear down the local PG stack (DESTRUCTIVE — drops the volume).
dev-pg-down:
    @echo "🛑 Stopping PostgreSQL..."
    docker compose down

# ─── Performance budget ──────────────────────────────────────────────────────

# Run the performance budget gate (bench + compare against perf-budget.toml)
perf:
    #!/usr/bin/env bash
    ./scripts/perf-budget-check.sh

# Run the raw Criterion benchmarks with bencher output (no budget check)
perf-bench:
    cargo bench -p cognicode-core --bench graph_benchmarks -- --output-format bencher

