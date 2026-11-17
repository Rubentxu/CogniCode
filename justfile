# ============================================================================
# CogniCode — Justfile de automatización
# ============================================================================
#
# Uso:
#   just                  → Muestra todos los comandos disponibles
#   just build            → Build completo del workspace
#   just build-wasm       → Build del frontend del Explorer
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
EXPLORER_PORT := env_var_or_default("EXPLORER_PORT", "5180")
PROJECT_PATH := env_var_or_default("COGNICODE_PROJECT_PATH", "")
version := env_var_or_default("COGNICODE_VERSION", `sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1`)

# ─── Default ──────────────────────────────────────────────────────────────────

default:
    @just --list

# ─── Build ────────────────────────────────────────────────────────────────────

# Build the whole workspace. The Explorer API is not built here: it lives
# in Rubentxu/cognicode-explorer (see docs/distribution/EXPLORER_HANDOFF.md).
build:
    @echo "🔨 Building the workspace..."
    bld rust cargo build --workspace

# Build only the Explorer frontend
build-wasm:
    @echo "🔨 Building Explorer frontend..."
    cd {{ EXPLORER_UI_DIR }} && npm ci && npm run build

# Build the standalone lane: statically linked against musl, so the artifact
# runs on any Linux x86_64 whatever glibc the host carries.
#
# The artifacts are named by the release contract:
#   {component}-{version}-{platform_token}.tar.gz
# and each one holds a single `bin/{component}`, which is the layout the
# installer expects.
build-standalone:
    #!/usr/bin/env bash
    set -euo pipefail
    VERSION={{ version }}
    TOKEN=x86_64-unknown-linux-musl
    # The pinned toolchain (rust-toolchain.toml) is the one that must carry
    # the target: adding it to `stable` builds a lane the pinned toolchain
    # cannot build, because cargo follows the pin.
    TOOLCHAIN=$(sed -n 's/^channel = "\(.*\)"/\1/p' rust-toolchain.toml | head -1)
    rustup target add "${TOKEN}" --toolchain "${TOOLCHAIN}"
    # The crates with C code (ring) need a musl C compiler; cc-rs cannot
    # find one by itself when the build host is a glibc one.
    export CC_x86_64_unknown_linux_musl="${CC_x86_64_unknown_linux_musl:-musl-gcc}"
    OUT=$(scripts/ci/target-dir.sh)/${TOKEN}/release
    bld rust cargo build --release --target "${TOKEN}" -p cognicode-cli --bin cogh --bin cognicode
    bld rust cargo build --release --target "${TOKEN}" -p cognicode-mcp --bin cognicode-mcp --bin cognicode-mcp-server
    mkdir -p dist
    pkg=$(mktemp -d)
    for c in cogh cognicode cognicode-mcp; do
      mkdir -p "${pkg}/bin"
      cp "${OUT}/${c}" "${pkg}/bin/${c}"
      tar -czf "dist/${c}-${VERSION}-${TOKEN}.tar.gz" -C "${pkg}" bin
      rm -rf "${pkg}/bin"
    done
    rm -rf "${pkg}"
    echo "✅ Built the standalone lane (${TOKEN}) in dist/"

# Alias kept for the recipes that used to call it.
build-musl: build-standalone

# Build the native lane (dynamically linked against the host glibc) and
# package it with the same contract names the standalone lane uses.
#
# The native lane IS the host lane, so it is built without an explicit
# `--target`: the artifacts land in `<target-dir>/release/`, which is also
# where the release tests and `cargo run --release --bin cognicode-release`
# look for them. The host triple is read from the toolchain rather than
# assumed, and a host that is not the published native lane fails loud
# instead of publishing assets under a token nobody can install.
# Build the native lane (dynamically linked against the host glibc)
build-native:
    #!/usr/bin/env bash
    set -euo pipefail
    VERSION={{ version }}
    TOKEN=$(rustc -vV | sed -n 's/^host: //p' | head -1)
    if [[ "${TOKEN}" != "x86_64-unknown-linux-gnu" ]]; then
      echo "error: this host builds '${TOKEN}', and the published native lane is" >&2
      echo "       'x86_64-unknown-linux-gnu'. Build the release on an x86_64" >&2
      echo "       host, or extend the release contract first." >&2
      exit 1
    fi
    OUT=$(scripts/ci/target-dir.sh)/release
    bld rust cargo build --release -p cognicode-cli --bin cogh --bin cognicode
    bld rust cargo build --release -p cognicode-mcp --bin cognicode-mcp --bin cognicode-mcp-server
    mkdir -p dist
    pkg=$(mktemp -d)
    for c in cogh cognicode cognicode-mcp; do
      mkdir -p "${pkg}/bin"
      cp "${OUT}/${c}" "${pkg}/bin/${c}"
      tar -czf "dist/${c}-${VERSION}-${TOKEN}.tar.gz" -C "${pkg}" bin
      rm -rf "${pkg}/bin"
    done
    rm -rf "${pkg}"
    echo "✅ Built the native lane (${TOKEN}) in dist/"

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
build-release:
    @echo "🔨 Building the workspace (release)..."
    bld rust cargo build --workspace --release

# Clean build artifacts
clean:
    @echo "🧹 Cleaning..."
    cargo clean
    rm -rf {{ EXPLORER_UI_DIR }}/dist
    echo "Cleaned"

# ─── Check ────────────────────────────────────────────────────────────────────

# Check compilation (fast, no binary output)
check:
    @echo "✅ Checking compilation..."
    bld rust cargo check --workspace

# Run clippy lints
lint:
    @echo "🔍 Running clippy..."
    bld rust cargo clippy --workspace --all-targets -- -D warnings

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
    bld rust cargo test --workspace --lib

# Run all integration tests (with mocks)
test-integration:
    @echo "🧪 Running integration tests..."
    bld rust cargo test --workspace --tests

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

# The faithful-render lane: the Helm tests against the real `helm template`
# engine, with the helm pinned in .tool-versions and COGNICODE_REQUIRE_HELM=1,
# so a missing binary fails the lane instead of skipping it.
helm-lane:
    @echo "Running the Helm faithful-render lane..."
    bash scripts/ci/helm-lane.sh

# One command per SDK release. `just sdk-release 0.103.0` bumps the version,
# rehearses, and opens the PR; after merging it, `just sdk-release --publish`
# publishes, verifies and tags.
sdk-release *ARGS:
    bash scripts/distribution/release-sdk.sh {{ARGS}}

# Publish the SDK crates at the workspace version if crates.io does not have it
# yet; does nothing otherwise. `just sdk-auto-release --check` only reports.
sdk-auto-release *ARGS:
    bash scripts/distribution/auto-release-sdk.sh {{ARGS}}

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
    bld rust cargo test --workspace -- --include-ignored --test-threads=1 || \
    bld rust cargo test --workspace -- --ignored --test-threads=1

# Run all lifecycle tests (single-threaded to avoid lock contention)
test-lifecycle:
    @echo "🧪 Running lifecycle tests..."
    bld rust cargo test -p cognicode-cli --bin cogh lifecycle -- --test-threads=1

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

# Build Docker image (MCP stdio + HTTP/SSE; the Explorer API lives in the
# external repository, which builds its own container)
docker-build:
    @echo "🐳 Building Docker image..."
    docker build -t cognicode .

# Run Docker container (the HTTP/SSE server refuses to start without a token)
docker-run:
    @echo "🐳 Running Docker container..."
    docker run -p 9847:9847 -e COGNICODE_MCP_AUTH_TOKEN="$${COGNICODE_MCP_AUTH_TOKEN:?set COGNICODE_MCP_AUTH_TOKEN}" cognicode

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
    @echo "Explorer UI (mocks): just explorer-dev → http://localhost:{{ EXPLORER_PORT }}"
    @echo "Explorer API:        construida y servida desde Rubentxu/cognicode-explorer"
    @echo ""

# Install dependencies
install:
    @echo "📦 Installing dependencies..."
    npm install 2>/dev/null || true
    npx playwright install chromium 2>/dev/null || true
    @echo "Dependencies installed"

# Open dashboard in browser
open:
    @echo "🌐 Opening Explorer..."
    @xdg-open http://localhost:{{ EXPLORER_PORT }} 2>/dev/null || \
     open http://localhost:{{ EXPLORER_PORT }} 2>/dev/null || \
     echo "Open http://localhost:{{ EXPLORER_PORT }} in your browser"

# Full setup from scratch
setup: install build
    @echo "✅ Setup complete!"
    @echo "UI con mocks: just explorer-dev → http://localhost:{{ EXPLORER_PORT }}"
    @echo "Explorer API: se construye y sirve desde Rubentxu/cognicode-explorer"

# ============================================================================
# Explorer UI (React + TypeScript)
# ============================================================================

# Quick start with mock data (no PG, no API needed)
explorer-mock:
    @echo "🚀 Starting Explorer UI with mock data (no backend)..."
    cd {{ EXPLORER_UI_DIR }} && npm run dev:mock

# Stop the Explorer UI dev server
explorer-stop:
    @echo "🛑 Stopping Explorer UI..."
    @fuser -k {{ EXPLORER_PORT }}/tcp 2>/dev/null || true
    @echo "✅ Explorer UI stopped"

# Dev mode: frontend with MSW mocks (no backend needed)
explorer-dev:
    @echo "🚀 Starting Explorer UI with mock data..."
    cd {{ EXPLORER_UI_DIR }} && npm run dev:mock

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
    bld rust cargo bench -p cognicode-core --bench graph_benchmarks -- --output-format bencher

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
    bld rust cargo test -p cognicode-core --test it --features evidence-kernel -- equivalence_harness:: --nocapture

# LSI M3 identity benchmark (E38): tiered continuity matcher over the
# lsi-identity ground-truth fixtures plus the workspace isolation suite.
# Fixed argv; fails (non-zero exit) when any scoring gate is missed
# (precision >= 0.95, recall >= 0.90, line-shift retention == 1.00,
# move retention >= 0.99), the matcher-convention pin changes, a case
# verification fails, or a workspace collision is detected.
lsi-identity:
    #!/usr/bin/env bash
    set -euo pipefail
    bld rust cargo test -p cognicode-core --test it --test identity_benchmark --features evidence-kernel -- workspace_isolation:: --nocapture

# LSI M4 semantic provider conformance (E39): declared per-tier precision
# targets over sandbox/fixtures/lsi-providers (Rust/TS serverless S0/S1,
# Java availability-gated on the jdtls PATH probe — no spawn), plus the
# gated fact-bridge tier/provenance unit filters and the CP-5 cross-layer
# tie-break. Fixed argv; fails on any tier contradiction.
lsi-providers:
    #!/usr/bin/env bash
    set -euo pipefail
    bld rust cargo test -p cognicode-core --test it -- provider_conformance:: --nocapture
    bld rust cargo test -p cognicode-core --test it --features evidence-kernel -- cp5_tie_break:: --nocapture
    bld rust cargo test -p cognicode-core --lib fact_bridge --features evidence-kernel

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
    bld rust cargo build --release --manifest-path crates/spike-ladybug/Cargo.toml
    cargo run --manifest-path crates/spike-ladybug/Cargo.toml --example s1_bootstrap
    bld rust cargo test --manifest-path crates/spike-ladybug/Cargo.toml --tests

# E29 S1 spike — clean cache + build artifacts (forces fresh prebuilt download)
spike-ladybug-clean:
    rm -rf crates/spike-ladybug/target crates/spike-ladybug/.cache

# E29 S2 spike — schema load + COPY FROM + query validation
spike-ladybug-s2:
    cargo run --manifest-path crates/spike-ladybug/Cargo.toml --example s2_schema_create
    cargo run --manifest-path crates/spike-ladybug/Cargo.toml --example s2_copy_from
    cargo run --manifest-path crates/spike-ladybug/Cargo.toml --example s2_query_validation
    bld rust cargo test --manifest-path crates/spike-ladybug/Cargo.toml --tests

# E29 S2 spike — clean S2 .lbdb artifacts
spike-ladybug-s2-clean:
    rm -f s2_schema.lbdb s2_copy_from.lbdb s2_query_validation.lbdb

# E29 S3 spike — run all S3 examples and tests end-to-end
spike-ladybug-s3:
    cargo run --manifest-path crates/spike-ladybug/Cargo.toml --example s3_lock_holder -- --mode=rw --path=/tmp/s3_e2e.lbdb --hold-secs=1
    cargo run --manifest-path crates/spike-ladybug/Cargo.toml --example s3_concurrency
    bld rust cargo test --manifest-path crates/spike-ladybug/Cargo.toml --test s3_concurrency
    bld rust cargo test --manifest-path crates/spike-ladybug/Cargo.toml --test s3_multi_process

# E29 S3 spike — clean S3 .lbdb artifacts
spike-ladybug-s3-clean:
    rm -f /tmp/s3_e2e.lbdb

# E29 S4 spike — run all S4 examples and tests end-to-end
spike-ladybug-s4:
    cargo run --manifest-path crates/spike-ladybug/Cargo.toml --example s4_writer -- --mode=clean --path=/tmp/s4_e2e.lbdb --rows=1000
    bld rust cargo test --manifest-path crates/spike-ladybug/Cargo.toml --test s4_crash_recovery

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
    bld rust cargo test --manifest-path crates/spike-ladybug/Cargo.toml --test s5_latency -- --nocapture

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

# Build every published lane and stage the release.
#
# `dist/release/` is EXACTLY what the release publishes: the payloads the
# contract declares, the bundle manifest of each Tier-1 platform, the
# inventory and SHA256SUMS.
#
# The staging directory is built from the CONTRACT, not from a glob over
# `dist/`. The glob would also stage the skill bundles the contract does not
# publish (`cognicode-agent-hardness`, `cognicode-developer`,
# `cognicode-pr-review` live in `skills/` and are not part of the published
# surface), and the factory would read them as component payloads. A release
# that ships what it did not declare is a release nobody can verify.
release-stage:
    #!/usr/bin/env bash
    set -euo pipefail
    VERSION={{ version }}
    TAG=v${VERSION}
    COMMIT=$(git rev-parse HEAD)
    just build-native
    just build-standalone
    just bundle-skills
    bld rust cargo build --release --bin cognicode-release
    REL=$(scripts/ci/target-dir.sh)/release/cognicode-release
    STAGING=$(mktemp -d)
    OUT=$(mktemp -d)
    trap 'rm -rf "${STAGING}" "${OUT}"' EXIT
    # The component payloads of every Tier-1 platform, named by the contract.
    for token in $("${REL}" platforms); do
      for name in $("${REL}" plan --platform "${token}" --version "${VERSION}"); do
        cp "dist/${name}" "${STAGING}/"
      done
    done
    # The skill bundles the contract publishes.
    for id in $("${REL}" skills --published); do
      cp "dist/${id}-${VERSION}.tar.gz" "${STAGING}/"
    done
    "${REL}" generate \
        --staging "${STAGING}" --out "${OUT}" \
        --version "${VERSION}" --tag "${TAG}" --source-commit "${COMMIT}"
    "${REL}" verify \
        --staging "${STAGING}" --version "${VERSION}" --tag "${TAG}"
    rm -rf dist/release
    mkdir -p dist/release
    cp "${OUT}"/* dist/release/
    echo "✅ Release staged in dist/release/ (${TAG})"
    ls -1 dist/release/

# ─── Release ──────────────────────────────────────────────────────────────────

# Create a draft GitHub release with every staged artifact.
#
# The assets come from `dist/release/`, which is exactly what the contract
# declares: uploading a glob over `dist/` would publish the skill bundles
# that the release does not declare.
release-draft tag="{{version}}":
    #!/usr/bin/env bash
    set -euo pipefail
    gh release create "{{ tag }}" --draft || true
    for f in dist/release/*; do
    [ -f "$f" ] || continue
    gh release upload "{{ tag }}" "$f" || true
    done

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
