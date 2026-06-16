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
EXPLORER_PORT := env_var_or_default("EXPLORER_PORT", "5180")
EXPLORER_API_PORT := env_var_or_default("EXPLORER_API_PORT", "3456")
EXPLORER_API_RELEASE := "target/release/explorer-api"
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
    cd {{EXPLORER_UI_DIR}} && npm ci && npm run build

# Build in release mode
build-release: build-server

# Clean build artifacts
clean:
    @echo "🧹 Cleaning..."
    cargo clean
    rm -rf {{EXPLORER_UI_DIR}}/dist
    echo "Cleaned"

# ─── Run ──────────────────────────────────────────────────────────────────────

# Build and start the Explorer API server
run: stop build-release
    @echo "🚀 Starting Explorer API on http://localhost:{{PORT}}"
    @if curl -s --max-time 1 http://localhost:{{PORT}}/health > /dev/null 2>&1; then \
        echo "❌ Port {{PORT}} still in use. Try: just stop && just run"; exit 1; \
    fi
    DATABASE_URL=postgres://cognicode:cognicode@localhost:5432/cognicode \
        ./{{EXPLORER_API_RELEASE}} --listen 127.0.0.1:{{PORT}}

# Start server (without rebuilding)
start: stop
    @echo "🚀 Starting Explorer API (no rebuild)..."
    @if curl -s --max-time 1 http://localhost:{{PORT}}/health > /dev/null 2>&1; then \
        echo "❌ Port {{PORT}} still in use. Try: just stop"; exit 1; \
    fi
    DATABASE_URL=postgres://cognicode:cognicode@localhost:5432/cognicode \
        ./{{EXPLORER_API_RELEASE}} --listen 127.0.0.1:{{PORT}}

# Run in dev mode — one command to start everything: PG + API + Frontend
dev:
    #!/usr/bin/env bash
    set -euo pipefail
    ROOT="$(cd "$(dirname "{{justfile()}}")" && pwd)"
    API_BIN="$ROOT/{{EXPLORER_API_RELEASE}}"
    UI_DIR="$ROOT/{{EXPLORER_UI_DIR}}"
    API_PORT="{{EXPLORER_API_PORT}}"
    UI_PORT="{{EXPLORER_PORT}}"

    echo "═══════════════════════════════════════════════════════"
    echo "  CogniCode — Dev Mode"
    echo "═══════════════════════════════════════════════════════"
    echo ""

    # 1. PostgreSQL via quadlet (systemd + podman)
    echo "🐘 [1/4] Starting PostgreSQL..."
    # Ensure quadlet files are installed
    QUADLET_DIR="$HOME/.config/containers/systemd"
    if [ ! -f "$QUADLET_DIR/cognicode-postgres.container" ]; then
        echo "   📋 Installing quadlet files..."
        cp "$ROOT/quadlets/cognicode-postgres.container" "$QUADLET_DIR/"
        cp "$ROOT/quadlets/cognicode-pgdata.volume" "$QUADLET_DIR/"
        systemctl --user daemon-reload
    fi
    systemctl --user start cognicode-postgres 2>/dev/null || true
    # Unset LD_LIBRARY_PATH to avoid flatpak lib conflicts (e.g. Zed's libselinux.so.1)
    for i in $(seq 1 30); do
        PG_CHECK=$(env -u LD_LIBRARY_PATH podman exec cognicode-postgres pg_isready -U cognicode -d cognicode 2>&1) && PG_OK=true || PG_OK=false
        if [ "$PG_OK" = "true" ]; then
            echo "   ✅ PostgreSQL ready"
            break
        fi
        if [ "$i" -eq 30 ]; then
            echo "   ❌ PostgreSQL not ready after 30s"
            echo "   pg_isready output: $PG_CHECK"
            echo "   Container status:"
            env -u LD_LIBRARY_PATH podman ps -a --filter name=cognicode-postgres 2>&1 || true
            echo "   Try: just dev-pg-status"
            exit 1
        fi
        sleep 1
    done

    # 2. Build API binary
    echo "🔨 [2/4] Building Explorer API..."
    cargo build -p cognicode-runtime --bin explorer-api --release
    echo "   ✅ API binary ready"

    # 3. Install frontend deps
    echo "📦 [3/4] Installing frontend deps..."
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
    DATABASE_URL=postgres://cognicode:cognicode@localhost:5432/cognicode \
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

# Run unit tests with PostgreSQL backend
test-pg:
	@echo "🧪 Running unit tests (with PG)..."
	TEST_DATABASE_URL=postgres://cognicode:cognicode@localhost:5432/cognicode_TEST \
		cargo test --workspace --no-fail-fast --features postgres

# Run ignored tests (flaky, slow, requires external tools)
test-ignored:
	@echo "🧪 Running ignored tests (single-threaded)..."
	cargo test --workspace -- --include-ignored --test-threads=1 || \
	cargo test --workspace -- --ignored --test-threads=1

# Run unit tests for a specific crate
test-crate crate:
	cargo test -p {{crate}} --no-fail-fast

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
        DATABASE_URL=postgres://cognicode:cognicode@localhost:5432/cognicode \
            nohup ./{{EXPLORER_API_RELEASE}} --listen 127.0.0.1:{{PORT}} > /tmp/cognicode-server.log 2>&1 & \
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
    docker build -t cognicode-explorer .

# Run Docker container
docker-run:
    @echo "🐳 Running Docker container..."
    docker run -p {{PORT}}:{{PORT}} cognicode-explorer

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
    @echo "Explorer:   http://localhost:{{EXPLORER_PORT}}"
    @echo "API:        http://localhost:{{PORT}}"
    @echo "Health:     http://localhost:{{PORT}}/health"
    @echo "Server PID: $$(pgrep -f explorer-api | head -1 || echo 'not running')"
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
    @xdg-open http://localhost:{{EXPLORER_PORT}} 2>/dev/null || \
     open http://localhost:{{EXPLORER_PORT}} 2>/dev/null || \
     echo "Open http://localhost:{{EXPLORER_PORT}} in your browser"

# Full setup from scratch
setup: install build
    @echo "✅ Setup complete!"
    @echo "Run 'just run' to start the Explorer API."
    @echo "Then visit http://localhost:{{EXPLORER_PORT}}"

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
    cargo build -p cognicode-runtime --bin explorer-api --release
    @echo "🚀 Starting Explorer API on http://127.0.0.1:{{EXPLORER_API_PORT}}..."
    DATABASE_URL=postgres://cognicode:cognicode@localhost:5432/cognicode \
        cargo run -p cognicode-runtime --bin explorer-api --release -- --listen 127.0.0.1:{{EXPLORER_API_PORT}}

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

# ─── PostgreSQL local dev (quadlet: systemd + podman) ──────────────────

# Install the quadlet files and start PostgreSQL.
# The container is managed by systemd user units; data persists
# in the named volume `cognicode-pgdata`.
dev-pg:
    #!/usr/bin/env bash
    set -euo pipefail
    QUADLET_DIR="$HOME/.config/containers/systemd"
    SRC_DIR="quadlets"
    # Install quadlet files if not present
    for f in cognicode-postgres.container cognicode-pgdata.volume; do
        if [ ! -f "$QUADLET_DIR/$f" ]; then
            echo "📋 Installing quadlet: $f"
            cp "$SRC_DIR/$f" "$QUADLET_DIR/$f"
        fi
    done
    # Reload systemd to pick up new units
    systemctl --user daemon-reload
    # Start PostgreSQL
    echo "🐘 Starting cognicode-postgres..."
    systemctl --user start cognicode-postgres
    echo "⏳ Waiting for PostgreSQL..."
    for i in $(seq 1 30); do
        if env -u LD_LIBRARY_PATH podman exec cognicode-postgres pg_isready -U cognicode -d cognicode > /dev/null 2>&1; then
            echo "✅ PostgreSQL is ready."
            echo "DATABASE_URL=postgres://cognicode:cognicode@localhost:5432/cognicode"
            exit 0
        fi
        sleep 1
    done
    echo "❌ PostgreSQL failed to become ready in 30s"; exit 1

# Stop PostgreSQL (preserves data volume).
dev-pg-stop:
    @echo "🛑 Stopping PostgreSQL..."
    systemctl --user stop cognicode-postgres
    @echo "✅ PostgreSQL stopped (data preserved)."

# Tear down PostgreSQL + remove volume (DESTRUCTIVE — drops all data).
dev-pg-down:
    @echo "🛑 Stopping and removing PostgreSQL..."
    systemctl --user stop cognicode-postgres 2>/dev/null || true
    systemctl --user reset-failed cognicode-postgres 2>/dev/null || true
    env -u LD_LIBRARY_PATH podman rm -f cognicode-postgres 2>/dev/null || true
    env -u LD_LIBRARY_PATH podman volume rm cognicode-pgdata 2>/dev/null || true
    @echo "✅ PostgreSQL and volume removed."

# Show PostgreSQL status
dev-pg-status:
    @echo "📊 PostgreSQL Status"
    systemctl --user status cognicode-postgres 2>/dev/null || echo "Not installed. Run: just dev-pg"
    @echo ""
    @env -u LD_LIBRARY_PATH podman exec cognicode-postgres pg_isready -U cognicode -d cognicode 2>/dev/null || echo "Not responding"

# Uninstall quadlet files from systemd
dev-pg-uninstall:
    @echo "🗑️  Removing quadlet files..."
    systemctl --user stop cognicode-postgres 2>/dev/null || true
    rm -f ~/.config/containers/systemd/cognicode-postgres.container
    rm -f ~/.config/containers/systemd/cognicode-pgdata.volume
    systemctl --user daemon-reload
    @echo "✅ Quadlet files removed."

# ─── Performance budget ──────────────────────────────────────────────────────

# Run the performance budget gate (bench + compare against perf-budget.toml)
perf:
    #!/usr/bin/env bash
    ./scripts/perf-budget-check.sh

# Run the raw Criterion benchmarks with bencher output (no budget check)
perf-bench:
    cargo bench -p cognicode-core --bench graph_benchmarks -- --output-format bencher

