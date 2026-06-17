# ADR-037: Testing Strategy — Sandbox Revival & MCP Quality Validation

**Status:** Accepted
**Date:** 2026-06-17
**Source:** Session output after ADR-034/035/036 + 34 commits adding capabilities without end-to-end validation

## Context

CogniCode MCP server has 64 tools with significant new capabilities added in this
session: type-ref extraction (11 languages), IaC extraction (Terraform + Ansible),
graph checkpointing (ADR-035), file watcher with auto-rebuild, auth middleware,
Prometheus metrics, per-tool timeouts, rate limiting, and Phase 5.2 composite params.

However, **none of these new capabilities have been validated end-to-end**. The
existing test infrastructure covers unit tests (1385+) and MCP roundtrip parity
(33 tests), but does not exercise:

- Real MCP lifecycle through HTTP/SSE transport
- PG-backed Mode B operation (ingest pipeline → graph build → tool queries)
- IaC tool correctness (does `iac_query` find Terraform resources?)
- Type-ref correctness (do walkers produce References edges?)
- Observability validation (do /metrics show real instrument data?)
- Performance regression detection against baselines
- Security posture (auth bypass, rate limit enforcement, timeout behavior)

### Existing Infrastructure (discovered during exploration)

A **sophisticated sandbox infrastructure already exists** but has not been updated
for the new capabilities:

| Component | Location | Status |
|-----------|----------|--------|
| **Sandbox orchestrator** | `crates/cognicode-sandbox/src/main.rs` (161 tests) | Functional but stale |
| **Sandbox core** | `crates/cognicode-core/src/sandbox_core/` (8 modules) | Complete: mcp_core, ground_truth, scoring, history, artifacts, manifest, resource, failure |
| **Scenario manifests** | `sandbox/manifests/` (72 YAML files) | Cover callgraph, indexing, intelligence, refactoring, debugging, AIX, scale, robustness — **missing IaC, type-refs, checkpoints, auth, metrics** |
| **Fixtures** | `sandbox/fixtures/` (33 directories) | Rust, Python, JS, TS, Go, Java — **missing Terraform, Ansible** |
| **Real repos** | `sandbox/repos/` (14 repos) | anyhow, serde, ripgrep, chalk, click, requests, express, commander, etc. |
| **5-dimension scoring** | `sandbox_core/scoring.rs` | Correctitud, Latencia, Escalabilidad, Consistencia, Robustez |
| **Ground truth matching** | `sandbox_core/ground_truth.rs` | Edge, symbol, entry point, hot path, search result matching |
| **MCP lifecycle client** | `sandbox_core/mcp_core.rs` | spawn → initialize → tools/list → tools/call → capture → classify |
| **Run history** | `sandbox_core/history.rs` + `sandbox/results/` | JSONL append-only with trend tracking + health score |
| **Criterion benchmarks** | `target/criterion/` (15+ baselines) | Graph construction, search, cache, traversal, BFS, shortest path |
| **CI smoke** | `scripts/mcp/mcp_smoke_all.py` | HTTP-based smoke with annotation-aware classification |

### Pre-existing Test Failures (10)

| Test | Cause | Fix needed |
|------|-------|------------|
| `test_classify_file` | Scan classification edge case | Investigate file extension mapping |
| `god_nodes_finds_highly_called_symbol` | SymbolId format mismatch | Fix test fixture symbol naming |
| `test_retrieve_and_verify_no_matches` | Flaky (timing/temp dir) | Stabilize or mark `#[ignore]` |
| `test_upsert_one_roundtrip` | Requires PG (schema mismatch) | Run with new PG schema |
| `test_walk_php_type_refs_*` (2) | tree-sitter-php grammar version mismatch | Pin grammar or update walker |
| `test_walk_swift_type_refs_*` (2) | tree-sitter-swift grammar version mismatch | Pin grammar or update walker |
| `test_reparse_on_edit_*` (2) | Manifest/persistence feature flag | Fix or gate behind `#[cfg(feature)]` |

## Decision

### 1. Revive the existing sandbox — do not build new infrastructure

The `cognicode-sandbox` crate + `sandbox_core` modules already provide:
- MCP lifecycle client (spawn server, initialize, call tools, capture responses)
- 5-dimension quality scoring (correctness, latency, scalability, consistency, robustness)
- Ground truth matching (compare tool output against expected values)
- Run history with trend tracking (JSONL append-only)
- Scenario manifests (YAML-driven test matrix)
- Failure classification (infrastructure vs regression vs expected)

**Action:** Update existing manifests and add new ones for new capabilities.
Do NOT rewrite the orchestrator or scoring engine.

### 2. Testing pyramid — 6 layers

```
Layer 6: Benchmark Regression     ← criterion baselines + trend alerts
Layer 5: Sandbox E2E              ← sandbox orchestrator with manifests
Layer 4: Smoke Conformance        ← mcp_smoke_all.py over HTTP/SSE
Layer 3: Integration Tests        ← PG-backed, checkpoint, ingest pipeline
Layer 2: MCP Roundtrip Parity     ← build_all_tools ↔ dispatch (33 tests)
Layer 1: Unit Tests               ← per-module (1385+ tests)
```

Each layer has a specific purpose and KPI target:

| Layer | Purpose | KPI | Target |
|-------|---------|-----|--------|
| Unit | Logic correctness | Test count | 1385+ (existing) |
| Roundtrip parity | tools/list ↔ dispatch match | Parity | 100% (33 tests) |
| Integration | Multi-component flows | Pass rate | 100% (excluding PG-gated) |
| Smoke conformance | MCP protocol + tool availability | Listed/dispatched | 64/64 |
| Sandbox E2E | Tool quality with ground truth | Health score | > 80/100 |
| Benchmark regression | Performance stability | p99 drift | < 10% from baseline |

### 3. New scenario manifests (8 new YAMLs)

| Manifest | Category | Tools covered | Ground truth |
|----------|----------|---------------|--------------|
| `sandbox/manifests/iac/terraform_query.yaml` | IaC | build_graph, iac_query | Known resources + references |
| `sandbox/manifests/iac/ansible_extract.yaml` | IaC | build_graph | Known plays + tasks |
| `sandbox/manifests/typerefs/rust_types.yaml` | Type-refs | get_type_references | Known type annotations |
| `sandbox/manifests/typerefs/go_types.yaml` | Type-refs | get_type_references | Known Go types |
| `sandbox/manifests/checkpoint/pin_and_evict.yaml` | Checkpoints | build_graph ×2 | CheckpointId monotonic |
| `sandbox/manifests/observability/metrics_check.yaml` | Observability | /metrics endpoint | Expected instrument names |
| `sandbox/manifests/security/auth_bypass.yaml` | Security | /mcp with/without token | 401 when token set |
| `sandbox/manifests/watcher/file_change.yaml` | File watcher | modify file → rebuild | Graph updated after debounce |

### 4. New fixtures (3 directories)

| Fixture | Files | Purpose |
|---------|-------|---------|
| `sandbox/fixtures/terraform-iac/` | `main.tf` (2 resources, 1 ref), `variables.tf` | IaC extraction + iac_query |
| `sandbox/fixtures/ansible-playbook/` | `site.yml` (1 play, 2 tasks, 1 import) | Ansible semantic handler |
| `sandbox/fixtures/multi-lang-types/` | `rust_types.rs`, `go_types.go`, `java_types.java` | Type-ref walker validation |

### 5. PG sandbox lifecycle

The sandbox needs a clean PG with the **new schema** (graph_nodes, graph_edges,
scan_manifest, graph_reports). The current container has the **legacy schema**
(symbols, call_edges only).

**Setup script** (`scripts/mcp/reset_pg_sandbox.sh`):
```bash
#!/bin/bash
# Reset PG sandbox to clean state with new schema
systemctl --user stop cognicode-postgres.container
podman volume rm cognicode-pgdata 2>/dev/null
systemctl --user start cognicode-postgres.container
sleep 5  # wait for health check
# Migrations run automatically when cognicode-mcp connects in Mode B
```

### 6. KPIs — measurable and reportable

#### Per-Tool KPIs (collected by sandbox_core scoring engine)

| Dimension | What it measures | Target | Current |
|-----------|------------------|--------|---------|
| **Correctitud** | Ground truth match rate | > 95% | Unknown (needs run) |
| **Latencia** | p50/p99 response time | p50 < 500ms, p99 < 5s | Unknown (needs run) |
| **Escalabilidad** | Degradation with larger inputs | < 2x at 10x scale | Unknown (needs run) |
| **Consistencia** | Same output across runs | 100% deterministic | Unknown (needs run) |
| **Robustez** | Graceful failure handling | Clear errors, no panics | Unknown (needs run) |

#### System-Level KPIs (collected from /metrics)

| KPI | Source | Target |
|-----|--------|--------|
| Tool surface honesty | `tools/list` count + annotations | 64 tools, 0 STUB, 4 GATED |
| Error rate | `cognicode_tool_calls{status="error"}` | < 1% for stable, < 5% experimental |
| Graph build success | `build_graph` result | symbols > 0, edges > 0 |
| /metrics completeness | Prometheus exposition | All 12 instruments present |
| Auth enforcement | /mcp without token | 401 when COGNICODE_MCP_AUTH_TOKEN set |
| Watcher reactivity | File change → /ready flip | < 2s after debounce window |
| Health score | sandbox_core weighted average | > 80/100 |

#### Run History KPIs (tracked across runs via JSONL)

| KPI | Source | Trend |
|-----|--------|-------|
| MCP Health Score | Weighted 5-dimension average | Non-decreasing |
| Per-tool pass rate | Scenario outcomes | Non-decreasing |
| Latency trend | p50/p99 per tool | Non-increasing |
| Coverage | Tools tested / total | Increasing toward 100% |

### 7. Execution — local, no external services

```bash
# Full sandbox run (all manifests)
cargo run -p cognicode-sandbox -- run --manifests sandbox/manifests/

# Category-specific run
cargo run -p cognicode-sandbox -- run --manifests sandbox/manifests/iac/

# Smoke conformance only (fast, no PG required)
python3 scripts/mcp/mcp_smoke_all.py --workspace sandbox/fixtures/rust-callgraph

# Benchmark regression check
cargo bench -p cognicode-core --bench graph_benchmarks -- --save-baseline current

# PG-backed integration test
bash scripts/mcp/reset_pg_sandbox.sh
cargo test -p cognicode-core --features postgres -- --test checkpoint_integration
```

No GitHub Actions. No external CI. Everything runs locally with podman containers.

### 8. Pre-existing test failures — resolution plan

| Test | Resolution |
|------|------------|
| `test_classify_file` | Investigate scan classification logic — likely extension case sensitivity |
| `god_nodes_finds_highly_called_symbol` | Fix SymbolId format in test fixture |
| `test_retrieve_and_verify_no_matches` | Mark `#[ignore]` with note (flaky temp dir contention) |
| `test_upsert_one_roundtrip` | Gate behind `#[cfg(feature = "postgres")]` — already gated, needs PG |
| `test_walk_php_type_refs_*` | Pin `tree-sitter-php` grammar version OR skip if grammar unavailable |
| `test_walk_swift_type_refs_*` | Same as PHP — pin or skip |
| `test_reparse_on_edit_*` | Gate behind `#[cfg(feature = "persistence")]` — persistence feature not active |

## Consequences

### What becomes easier
- **Reviving** is cheaper than building — 72 manifests + 33 fixtures + scoring engine already exist
- **KPIs are machine-readable** — JSONL history + Prometheus metrics enable trend tracking
- **Ground truth matching** catches regressions automatically — no manual verification needed
- **5-dimension scoring** gives a single health score that's reportable and comparable across runs

### What becomes harder
- **PG schema migration** — the current PG has legacy tables; reset requires data loss (acceptable for sandbox)
- **PHP/Swift grammar pinning** — tree-sitter grammars are vendored; version mismatches need Cargo.lock pinning
- **Manifest maintenance** — 72 + 8 new = 80 manifests to maintain as tools evolve

### Implementation priorities

1. **Fix 10 pre-existing test failures** (2h) — unblock the baseline
2. **Reset PG sandbox** (30min) — clean schema for integration tests
3. **Add 3 new fixtures** (2h) — Terraform, Ansible, multi-lang types
4. **Add 8 new manifests** (3h) — IaC, type-refs, checkpoints, observability, security, watcher
5. **Run full sandbox** (1h) — capture baseline health score
6. **Document run procedure** in runbook (1h)

**Total: ~10h to revive and extend the sandbox.**

## Roadmap Reference

This ADR introduces a new roadmap section: **Testing & Validation**. See
`docs/ingest-pipeline-roadmap.md` §Testing & Validation for task tracking.

## References

- [Sandbox plan](../testing-sandbox-plan.md) — earlier draft (superseded by this ADR)
- [ADR-034](ADR-034-mcp-production-readiness.md) — M2.12 CI gate (smoke test)
- [ADR-035](ADR-035-graph-checkpointing.md) — checkpoint integration tests
- `crates/cognicode-core/src/sandbox_core/` — scoring engine, ground truth, MCP lifecycle
- `crates/cognicode-sandbox/src/main.rs` — orchestrator CLI
- `sandbox/manifests/` — 72 scenario manifests
- `sandbox/fixtures/` — 33 test fixtures
- `sandbox/results/` — run history
