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

DASHBOARD_DIR := "crates/cognicode-dashboard"
DIST_DIR := DASHBOARD_DIR / "dist"
SERVER_BIN := "target/debug/cognicode-dashboard-server"
PORT := env_var_or_default("PORT", "3000")

# ─── Default ──────────────────────────────────────────────────────────────────

default:
    @just --list

# ─── Build ────────────────────────────────────────────────────────────────────

# Build everything: server + WASM frontend
build: build-server build-wasm copy-assets

# Build only the server binary
build-server:
    @echo "🔨 Building server..."
    cargo build --bin cognicode-dashboard-server

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
    cargo build --release --bin cognicode-dashboard-server
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
run: stop build copy-assets
    @echo "🚀 Starting dashboard on http://localhost:{{PORT}}"
    @if curl -s --max-time 1 http://localhost:{{PORT}}/health > /dev/null 2>&1; then \
        echo "❌ Port {{PORT}} still in use. Try: just stop && just run"; exit 1; \
    fi
    DIST_DIR={{DIST_DIR}} cargo run --bin cognicode-dashboard-server

# Start server (without rebuilding)
start: stop
    @echo "🚀 Starting dashboard (no rebuild)..."
    @if curl -s --max-time 1 http://localhost:{{PORT}}/health > /dev/null 2>&1; then \
        echo "❌ Port {{PORT}} still in use. Try: lsof -i :{{PORT}}"; exit 1; \
    fi
    DIST_DIR={{DIST_DIR}} ./{{SERVER_BIN}}

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
        DIST_DIR={{DIST_DIR}} nohup ./{{SERVER_BIN}} > /tmp/cognicode-server.log 2>&1 & \
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
