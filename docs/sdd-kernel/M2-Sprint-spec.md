# Kernel Specs: M2 Sprint — Health KPIs and Integrity Gates (M2.1–M2.11)

**Status**: spec
**Date**: 2026-06-17
**Parent ADR**: [ADR-034](../adr/ADR-034-mcp-production-readiness.md)
**Source Proposal**: `sddk/M2-Sprint/proposal` (M2 Sprint Proposal, engram #2143)
**Next Phase**: `sddk/M2-Sprint/design`

## Router Context Used

- **Knowledge Coverage**: sufficient — ADR-034 (Accepted), M1 archive report, and all in-scope symbols located in the codebase: `build_all_tools`, `call_tool_handler`, `classify_status`, `STUB_TOOLS`, `GATED_TOOLS`, `ToolMetrics`, `GraphInsightsService::analyze`, `WorkspaceSession::smart_overview`, `mcp_smoke_all.py`.
- **Context Quality**: C2 — the proposal maps cleanly to evidence in `crates/cognicode-core/src/infrastructure/telemetry/mod.rs`, `interface/mcp/status.rs`, `interface/mcp/rmcp_adapter.rs`, `application/services/graph_insights.rs`, `application/workspace_session.rs::smart_overview`, and `scripts/mcp/mcp_smoke_all.py`.
- **Taxonomy**: dominant axes are (a) **observability completeness** — metrics coverage at the centralized dispatch boundary, (b) **tool surface integrity** — honest capabilities vs. STUB placeholders, (c) **production-readiness gating** — smoke-driven CI contract on stability metadata.
- **Domain Language (resolved)**: `classify_status` taxonomy (`ok | stub | gated | error | missing | skip`), `ToolMetrics` (OTel instruments), `call_tool_handler` (centralized dispatch boundary from M1.1), `InsightsReport` (`summary`, `god_nodes`, `cycle_clusters`, `cycle_breakers`, `communities`, `surprising_connections`, `health_score`, `suggested_questions`), `STUB_TOOLS` / `GATED_TOOLS` constants in `status.rs`, `annotations.stability ∈ {stable, experimental, gated}`.
- **Domain Language (unresolved)**: none blocking — all spec truth is anchored to ADR-034 decisions and M1 archive evidence.
- **Recommended Effort**: verify — the proposal is well-grounded; specs add only the acceptance-criteria scaffolding and an explicit integrity-gate contract. No new design space introduced by the specs.

## Knowledge Provenance

- **Scope source**: `sddk/M2-Sprint/proposal` (engram #2143), `docs/adr/ADR-034-mcp-production-readiness.md`, `docs/sdd-kernel/M1-archive-report.md`.
- **Invariant source**: ADR-034 Decisions 1, 2, 3, 4, 5, 6 + M1 invariants (`call_tool_handler` is the universal instrumentation boundary; `classify_status` is total over the 6-value taxonomy; `ToolMetrics::noop()` is panic-free).
- **Memory-only hints excluded from spec truth**:
  - "60-tool expected surface" from the proposal — verified as design intent, re-anchored to "post-M2.4–M2.7 removal surface" (the exact count is observable, not load-bearing).
  - The "smoke test 60-tool pass count" — treated as observable, not spec truth.

---

## Capability: graph-kpis (M2.1)

### Requirement: Graph-level KPIs are recorded as OTel Gauges after build_graph

The system SHALL record `cognicode.graph.symbols`, `cognicode.graph.edges`, and `cognicode.graph.health_score` as OpenTelemetry Gauges whose values reflect the state of the active call graph at the moment the `build_graph` tool call completes through the centralized dispatch boundary.

#### Scenario: Build graph populates graph KPIs

- **Given** a workspace has not been built and the global `ToolMetrics` is initialized with a recording meter provider
- **When** `tools/call` is invoked with `name = "build_graph"` and a valid path argument, and execution completes successfully through `call_tool_handler`
- **Then** the gauges `cognicode.graph.symbols`, `cognicode.graph.edges`, and `cognicode.graph.health_score` SHALL be observable with values equal to `summary.total_symbols`, `summary.total_edges`, and `health_score` from `GraphInsightsService::analyze(&graph)` on the freshly built call graph

#### Scenario: KPIs updated on re-build

- **Given** a previous `build_graph` call has recorded graph KPIs with symbol count N1
- **When** `build_graph` is invoked again on a workspace with a different size producing a call graph with symbol count N2
- **Then** the gauge `cognicode.graph.symbols` SHALL be observable with value N2 (replaced, not summed)

#### Scenario: Empty graph still records health

- **Given** the active call graph has zero symbols and zero edges
- **When** `build_graph` completes successfully
- **Then** `cognicode.graph.symbols = 0`, `cognicode.graph.edges = 0`, and `cognicode.graph.health_score = 100.0` (per `GraphInsightsService::analyze` contract for empty graphs)

---

## Capability: metrics-endpoint (M2.2)

### Requirement: Prometheus /metrics endpoint served alongside /health

The system SHALL expose a `/metrics` HTTP endpoint on the same listener as `/health` (default `0.0.0.0:9847`) that returns a `200 OK` response with `Content-Type: text/plain; version=0.0.4` containing Prometheus text-format output that includes all instruments registered with the global meter provider.

#### Scenario: /metrics returns Prometheus text with tool calls counter

- **Given** the MCP server is running on `127.0.0.1:9847` and at least one `tools/call` has been processed through the centralized dispatch boundary
- **When** `curl -i http://127.0.0.1:9847/metrics` is executed
- **Then** the response status is `200`, the `Content-Type` is `text/plain; version=0.0.4`, and the body contains a Prometheus exposition line of the form `cognicode_tool_calls_total{tool="<name>",status="<ok|stub|gated|error|missing|skip>"} <value>`

#### Scenario: /metrics and /health coexist

- **Given** the MCP server is running on `127.0.0.1:9847`
- **When** `curl http://127.0.0.1:9847/health` and `curl http://127.0.0.1:9847/metrics` are executed sequentially
- **Then** both endpoints return `200 OK` and the `/mcp` MCP streamable HTTP service remains accessible

#### Scenario: /metrics and OTLP exporter coexist

- **Given** the MCP server is started with both the Prometheus exporter (serving `/metrics`) and the OTLP exporter configured via environment variables
- **When** a tool call is processed
- **Then** `/metrics` shows the updated counter AND the OTLP exporter is initialized without error (no double-registration panic)

---

## Capability: tool-annotations (M2.3)

### Requirement: Every tool registration carries an annotations block

The system SHALL attach an `annotations` object to every tool returned by `tools/list`, containing `title`, `category` (one of `graph | search | refactor | quality | file | composite`), `stability` (one of `stable | experimental | gated`), `requires_graph`, `requires_persistence`, `destructive`, and `estimated_latency_ms`.

#### Scenario: tools/list returns annotated tools

- **Given** the MCP server is running and has registered the full tool surface
- **When** `tools/list` is invoked and the response is parsed
- **Then** every tool in the response has a non-null `annotations` object with all seven fields populated; `annotations.stability` is one of `{"stable", "experimental", "gated"}`; `annotations.category` matches the registered category

#### Scenario: Annotations preserve MCP-spec field alongside CogniCode meta

- **Given** a tool has MCP-spec `ToolAnnotations` (e.g. `title`, `readOnlyHint`) defined by the underlying framework
- **When** `tools/list` returns the tool
- **Then** the response contains the spec-defined `annotations` object AND a CogniCode-specific metadata channel (sibling `meta` field or equivalent) carrying `stability`, `requires_graph`, `requires_persistence` so that MCP 2025-06 spec conformance is preserved

---

## Capability: tool-surface-gating (M2.4–M2.7)

### Requirement: STUB tools with no plan for completion are removed from tools/list

The system SHALL remove the four STUB tools `smart_search`, `compare_graph`, `iac_query`, and `nl_to_symbol` from the `tools/list` response and from any dispatch surface; the `STUB_TOOLS` constant in `status.rs` SHALL be reduced accordingly.

#### Scenario: Four STUB tools absent from tools/list

- **Given** the M2 sprint code is deployed
- **When** `tools/list` is invoked
- **Then** none of `smart_search`, `compare_graph`, `iac_query`, `nl_to_symbol` appear in the response, and the centralized dispatch boundary returns `ToolNotFound` for any of those names

#### Scenario: STUB_TOOLS constant reflects current surface

- **Given** the M2 sprint code is deployed
- **When** the `status::STUB_TOOLS` constant is read at test time
- **Then** it contains only `project_insights` and `project_overview` (the M2.8 and M2.9 targets) and does not include the four gated-out tools

#### Scenario: Roundtrip parity test passes with the post-removal tool surface

- **Given** `mcp_roundtrip_tests::test_all_listed_tools_are_dispatchable` is executed
- **When** the test enumerates `tools/list` and verifies every name is dispatchable
- **Then** the test passes with the post-removal tool count (4 fewer entries than the pre-M2 baseline) and `dispatchable_tool_names()` returns the same reduced name set

---

## Capability: project_insights (M2.8)

### Requirement: project_insights returns real graph analytics output

The system SHALL make the `project_insights` tool delegate its computation to `GraphInsightsService::analyze(&graph)` and return a response whose `communities`, `god_nodes`, `hot_paths`, and `health_score` fields are populated from the `InsightsReport` returned by that service for the active call graph.

#### Scenario: project_insights reports real analytics

- **Given** `build_graph` has completed and produced a call graph with at least one community
- **When** `tools/call` is invoked with `name = "project_insights"`
- **Then** the response body contains `communities` derived from `InsightsReport.communities`, `god_nodes` derived from `InsightsReport.god_nodes`, `hot_paths` derived from `InsightsReport` god-node names ranked by score, and `health_score` equal to `InsightsReport.health_score` — none of these values SHALL be hardcoded constants

#### Scenario: project_insights handles no-graph gracefully

- **Given** `build_graph` has not been run and `load_graph()` returns `Ok(None)`
- **When** `tools/call` is invoked with `name = "project_insights"`
- **Then** the response is a `classify_status = "error"` with an `InterfaceError::Internal` whose message indicates a missing graph, and the centralized dispatch boundary records the call with `status = "error"`

---

## Capability: project_overview (M2.9)

### Requirement: project_overview returns smart_overview output

The system SHALL make the `project_overview` tool delegate to `WorkspaceSession::smart_overview(detail)` and return a response whose `architecture_score`, `hot_paths`, and `entry_points` fields are populated from that call's structured output for the active workspace.

#### Scenario: project_overview returns real architecture score

- **Given** a workspace has been built via `build_graph` and `WorkspaceSession::smart_overview("detailed")` returns an `architecture_score` value S
- **When** `tools/call` is invoked with `name = "project_overview", arguments = {"detail": "detailed"}`
- **Then** the response's `architecture_score` equals S and is NOT the pre-M2 hardcoded constant `85.0`; `hot_paths` contains the top-5 symbols from `smart_overview`; `entry_points` contains the top-5 entry points from `smart_overview`

#### Scenario: project_overview with detail=quick

- **Given** a workspace has been built
- **When** `tools/call` is invoked with `name = "project_overview", arguments = {"detail": "quick"}`
- **Then** the response shape matches `smart_overview("quick")` output (compact subset) and `classify_status` records `status = "ok"` (not `stub`)

---

## Capability: gated-tool-annotations (M2.10)

### Requirement: Gated tools carry stability metadata

The system SHALL annotate the three gated tools `graph_diff`, `graph_timeline`, and `generate_contract` with `annotations.stability = "gated"` and `annotations.requires_persistence = true`.

#### Scenario: Gated tools carry required annotations

- **Given** the M2 sprint code is deployed
- **When** `tools/list` is invoked
- **Then** the tool entries for `graph_diff`, `graph_timeline`, and `generate_contract` each have `annotations.stability = "gated"` AND `annotations.requires_persistence = true`

#### Scenario: Gated annotation matches runtime behavior

- **Given** `graph_diff`, `graph_timeline`, and `generate_contract` carry `stability = "gated"`
- **When** any of them is invoked without the persistence layer configured
- **Then** the response is a `classify_status = "gated"` error and the smoke test does not flag the response as `STUB` (the annotation declared the gap is expected)

---

## Capability: integrity-gate (M2.11)

### Requirement: Smoke test fails when stable tools return STUB

The system SHALL make `scripts/mcp/mcp_smoke_all.py` read `annotations.stability` from each tool in the `tools/list` response and FAIL the smoke run if any tool whose `stability = "stable"` is classified as `STUB` after a real `tools/call`.

#### Scenario: Stable tool returning OK passes the integrity gate

- **Given** a tool is registered with `annotations.stability = "stable"` and returns a real, non-placeholder response
- **When** the smoke test calls the tool through `tools/call` and classifies the response
- **Then** the classification is `OK` (not `STUB`) and the smoke run exits with code `0`

#### Scenario: Stable tool returning STUB fails the integrity gate

- **Given** a tool is registered with `annotations.stability = "stable"`
- **When** the tool's `tools/call` response body matches a STUB pattern (e.g. `results:[]`, `total_candidates:0`, or `note:` prefix in a known STUB tool) as classified by `status::classify_status`
- **Then** the smoke run FAILS with exit code `1` and emits a diagnostic identifying the stable tool that returned STUB

#### Scenario: Experimental or gated tool returning STUB passes the integrity gate

- **Given** a tool is registered with `annotations.stability` ∈ `{"experimental", "gated"}`
- **When** the tool's response is classified as `STUB` (or `GATED` for the gated case)
- **Then** the smoke run does NOT fail on that tool's classification — only stable-tool/STUB combinations trigger the integrity gate

---

## Invariants Covered

- **I-1 (M1.1) — Centralized instrumentation boundary** — `call_tool_handler` is the only path that records `cognicode.tool.calls` / `cognicode.tool.duration` / `cognicode.tool.errors`; verified by M2.1 (graph KPIs flow through the same boundary), M2.2 (counter visible at /metrics), and the no-bypass property of M2.8/M2.9 rewrites.
- **I-2 (ADR-034 §1) — No public STUB tool without `stability: experimental|gated`** — verified by M2.4–M2.7 (4 tools removed) and M2.11 (smoke test fails on stable+STUB).
- **I-3 (ADR-034 §2) — Every tool carries an annotations block** — verified by M2.3 (presence) and M2.10 (gated tools have `stability = "gated"` + `requires_persistence = true`).
- **I-4 — `build_all_tools()` is the single source of truth for `tools/list`** — verified by M2.4–M2.7 `test_all_listed_tools_are_dispatchable` parity test.
- **I-5 — `STUB_TOOLS` and `GATED_TOOLS` constants match actual tool surface** — verified by M2.4–M2.7 (STUB_TOOLS shrinks) and M2.10 (GATED_TOOLS unchanged but annotations are mandatory).
- **I-6 — `classify_status` is total over the 6-value taxonomy (`ok | stub | gated | error | missing | skip`)** — preserved across all rewrites; the smoke test reads the status label from this classifier and never from response-body heuristics alone.
- **I-7 — `GraphInsightsService::analyze` is total** (empty graph returns `health_score = 100.0`) — verified by M2.1 empty-graph scenario and M2.8 first scenario.

## Open Questions

- **M2.12 — CI provider decision**: deferred by the proposal; needs resolution before "M2.5 second-batch" planning. This spec does not constrain M2.12.
- **Metrics hook placement (M2.1)**: should graph KPIs be set inside the `build_graph` handler, inside `GraphInsightsService::analyze`, or via a new `record_graph_metrics(&InsightsReport)` helper called from `call_tool_handler` after the dispatch returns? Spec only requires that the values are observable post-`build_graph` — implementation choice belongs to `sddk-design`.
- **`/metrics` endpoint auth**: open access (current `/health` model) vs. localhost-only binding vs. bearer token — design phase decision. Spec only requires that the endpoint is reachable from the same listener.
- **Annotations transport (M2.3)**: whether to use MCP-spec `ToolAnnotations` for the stability/category fields directly, or carry them in a sibling `meta` object, depends on what the `rmcp` crate (v0.27) supports without fork. Spec only requires that all seven fields are present and parseable by the smoke test.
