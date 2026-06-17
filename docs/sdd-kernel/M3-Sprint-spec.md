# Kernel Specs: M3 Sprint — Operational Hardening (M3.1–M3.7, M2.12)

**Status**: spec
**Date**: 2026-06-17
**Parent ADR**: [ADR-034](../adr/ADR-034-mcp-production-readiness.md) §7 "Operational hardening"
**Source Proposal**: `sddk/M3-Sprint/proposal`
**Source Exploration**: `sddk/M3-Sprint/explore` (engram #2165)
**Next Phase**: `sddk/M3-Sprint/design`

## Router Context Used

- **Knowledge Coverage**: sufficient — ADR-034 §7, M2 archive report, M1+M2 spec artifacts, in-scope symbols located: `call_tool_handler` (rmcp_adapter.rs:1013), `cognicode_meta` (rmcp_adapter.rs:125), `RateLimiter` (security.rs:877), `InputValidator` (security.rs:82), `/health` and `/metrics` routes (server.rs:92-93), `ToolMetrics` (telemetry/mod.rs).
- **Context Quality**: C2 — proposal maps cleanly to evidence in `crates/cognicode-mcp/src/server.rs`, `crates/cognicode-core/src/interface/mcp/rmcp_adapter.rs`, `crates/cognicode-core/src/interface/mcp/security.rs`, `docs/mcp-production-roadmap.md` §Sprint M3.
- **Taxonomy**: dominant axes are (a) **operational observability** — readiness probe, structured per-call log, SLO definitions, (b) **fault isolation** — per-category timeouts, rate limits, (c) **deployment surface** — Bearer-token auth, env-var-driven config, runbook completeness, (d) **CI integrity** — smoke as merge gate.
- **Domain Language (resolved)**:
  - `/health` (existing) = process alive, returns `200 OK` regardless of graph state
  - `/ready` (new) = graph loaded AND queryable through the dispatch boundary, returns `503` until then
  - Tool categories: `graph | search | refactor | quality | file | composite | navigation | aix`
  - `cognicode_meta.estimated_latency_ms` — per-tool latency hint (M2.3, 60 tools)
  - `classify_status` taxonomy: `ok | stub | gated | error | missing | skip`
  - `RateLimiter::check_with_key(client_key)` — token bucket, 100 tokens/60s default (security.rs:928)
  - `COGNICODE_MCP_AUTH_TOKEN` — env var for Bearer-token enforcement (from ADR-034/roadmap, undefined pre-spec)
  - `error_type` — string label on a tool-call error response (existing on `InterfaceError`)
- **Domain Language (unresolved)**:
  - "Graph loaded AND queryable" for `/ready` — Spec anchors this to: `build_graph` has completed at least once for the active session AND `load_graph()` returns `Ok(Some(_))` (i.e. a graph is currently in memory and dispatchable). Cross-mode semantics are: Mode A (standalone) — after first `build_graph`; Mode B (PG) — after PG pool is established AND graph has been loaded into the in-memory cache.
  - "Graph analytics" rate-limit bucket — Spec anchors this to: the `graph` category per `cognicode_meta.category` (M2.3 mapping). Other categories inherit the default `RateLimiter` (100/60s) without per-tool sub-buckets.
  - "Tools with cognicode_meta category=graph" — Spec anchors to the seven known category strings in `cognicode_meta` definitions: `graph | file | navigation | search | quality | refactor | composite | aix`.
  - "Per-tool timeout mapping" — Spec defines the full mapping below (graph→60s, navigation→45s, search→500ms, default→30s); the user's acceptance criteria test only `graph` and `search`, but the design must implement the full table.
- **Recommended Effort**: verify — M1 and M2 foundations are solid; M3 only adds operational wrapping. No new domain design space. The only greenfield is M3.5 (auth middleware); the rest are wrapping/cross-cutting changes to known insertion points.

## Knowledge Provenance

- **Scope source**: `sddk/M3-Sprint/explore` (engram #2165), `docs/adr/ADR-034-mcp-production-readiness.md` §7, `docs/mcp-production-roadmap.md` §Sprint M3, M2 archive report.
- **Invariant source**:
  - I-1 (M1.1) — `call_tool_handler` is the universal instrumentation boundary. M3 timeout + rate-limit wrapping must NOT bypass this boundary.
  - I-2 (ADR-034 §1) — No public STUB tool without `stability: experimental|gated`. Preserved.
  - I-3 (ADR-034 §2) — Every tool carries `cognicode_meta`. Preserved; the timeout mapping reads from this.
  - I-4 (M2) — `build_all_tools()` is the single source of truth for `tools/list`. Preserved.
  - I-7 (M1) — `classify_status` is total over the 6-value taxonomy. M3.4 structured log reads from this.
- **Memory-only hints excluded from spec truth**:
  - The "LSP 45s" budget in the explore — kept as a verification note (see Open Questions), not as a hard spec invariant; ADR-034 mentions it but the user's M3.2 acceptance only locks `graph` and `search`.
  - The "/ready Mode B = PG pool + graph loaded" refinement — derived from code evidence, but flagged as an Open Question for the design phase to confirm.

---

## Capability: readiness-probe (M3.1)

### Requirement: The system SHALL expose a /ready endpoint distinct from /health

The system SHALL expose a `/ready` HTTP endpoint on the same listener as `/health` and `/metrics` (default `0.0.0.0:9847`) that returns `200 OK` with `{"status":"ready","graph_loaded":true}` when the active call graph is loaded and dispatchable through the centralized boundary, and `503 Service Unavailable` otherwise. The pre-existing `/health` endpoint SHALL continue to return `200 OK` regardless of graph state.

#### Scenario: Pre-build /ready returns 503

- **Given** the MCP server has just started and no `build_graph` call has yet been processed through `call_tool_handler`
- **When** `curl -i http://127.0.0.1:9847/ready` is executed
- **Then** the response status is `503 Service Unavailable`
- **And** the body is a JSON object that does NOT include `"graph_loaded":true` (the body shape itself is implementation-defined; the load-bearing assertion is the status code and the absence of the `ready` marker)

#### Scenario: Post-build /ready returns 200 with body

- **Given** the MCP server is running and a `tools/call` for `build_graph` has completed successfully through the centralized dispatch boundary, producing an active call graph
- **When** `curl -i http://127.0.0.1:9847/ready` is executed
- **Then** the response status is `200 OK`
- **And** the body is `{"status":"ready","graph_loaded":true}` (exact field set; additional fields are allowed but the two named fields SHALL be present with the stated values)

#### Scenario: /health remains independent of /ready

- **Given** the MCP server has just started and no `build_graph` call has been processed
- **When** `curl -i http://127.0.0.1:9847/health` and `curl -i http://127.0.0.1:9847/ready` are executed
- **Then** `/health` returns `200 OK` (process-alive signal) AND `/ready` returns `503 Service Unavailable` (graph-not-loaded signal), demonstrating that the two endpoints report independent state

---

## Capability: per-tool-timeout (M3.2)

### Requirement: Tool calls SHALL be bounded by a per-category timeout

The system SHALL enforce a timeout on every `tools/call` flow, where the duration is determined by the tool's `cognicode_meta.category` per the following mapping:

| Category    | Default timeout |
|-------------|-----------------|
| `graph`     | 60 s            |
| `navigation`| 45 s            |
| `search`    | 500 ms          |
| (any other) | 30 s            |

When a tool call exceeds its category timeout, the centralized dispatch boundary SHALL return a response with `status = "error"` and an `error_type` value that includes the substring `"timeout"`. The `cognicode.tool.errors` counter SHALL be incremented with `error_type = "timeout"`.

#### Scenario: graph-category tool times out at 60s

- **Given** a tool is registered with `cognicode_meta.category = "graph"` (e.g. `build_graph`, `graph_pagerank`, `graph_communities`)
- **And** the tool's handler is configured to take longer than 60s before returning
- **When** `tools/call` is invoked for that tool
- **Then** the call returns within ≤ 60s of invocation start with a response whose `status` label is `"error"` and whose `error_type` contains the substring `"timeout"`
- **And** the structured log line for the call (M3.4) records `status="error", error_type="timeout"`

#### Scenario: search-category tool times out at 500ms

- **Given** a tool is registered with `cognicode_meta.category = "search"` (e.g. `semantic_search`, `graph_search_idf`, `find_symbols_by_name`)
- **And** the tool's handler is configured to take longer than 500ms before returning
- **When** `tools/call` is invoked for that tool
- **Then** the call returns within ≤ 500ms of invocation start with a response whose `status` label is `"error"` and whose `error_type` contains the substring `"timeout"`

#### Scenario: default timeout applies to unmapped categories

- **Given** a tool is registered with a category that is NOT `graph`, `navigation`, or `search` (e.g. `file`, `quality`, `refactor`, `composite`, `aix`)
- **And** the tool's handler is configured to take longer than 30s but less than 60s before returning
- **When** `tools/call` is invoked for that tool
- **Then** the call returns within ≤ 30s of invocation start with `status = "error"` and `error_type` containing `"timeout"`

---

## Capability: rate-limit-enforcement (M3.3)

### Requirement: Rate limit SHALL be enforced at the dispatch boundary for graph-category tools

The system SHALL enforce a per-client-key rate limit at the `call_tool_handler` boundary for tools whose `cognicode_meta.category = "graph"`, using the existing `RateLimiter` (security.rs:877) with a default of 100 tokens per 60-second window per client. When a call is rate-limited, the response SHALL be a tool-call error with an `error_type` that includes the substring `"rate_limit_exceeded"`. The existing `cognicode.tool.errors` counter SHALL be incremented for the rate-limited call.

#### Scenario: 101st call from same client within 60s is rate-limited

- **Given** a client key K has made 100 successful calls to a graph-category tool within the current 60s window
- **And** the `RateLimiter` for K has consumed all 100 tokens for that window
- **When** the 101st call from K for a graph-category tool arrives within the same 60s window
- **Then** the response is a tool-call error with `status = "error"` and `error_type` containing the substring `"rate_limit_exceeded"`
- **And** `cognicode.tool.errors` is incremented for the rate-limited call with `error_type = "rate_limit_exceeded"`

#### Scenario: rate-limited call does not consume a successful-call metric

- **Given** 100 successful calls to a graph-category tool have been made by client K within the current 60s window
- **And** `cognicode.tool.calls{tool=<name>,status="ok"}` shows 100 entries
- **When** the 101st call is rate-limited
- **Then** the rate-limited response does NOT increment the `status="ok"` counter
- **And** the rate-limited response is recorded in the structured log (M3.4) with `status="error", error_type="rate_limit_exceeded"`
- **And** the observable metric state is: 100 ok-calls + 1 error-call with `error_type="rate_limit_exceeded"` (not 101 ok + 1 error)

#### Scenario: rate limit is per-client-key (not global)

- **Given** client K1 has consumed all 100 tokens in the current 60s window
- **When** client K2 makes a call to the same graph-category tool
- **Then** the call from K2 succeeds (returns within the per-category timeout) and is not rate-limited
- **And** the `RateLimiter` for K2 shows 99 remaining tokens

---

## Capability: structured-per-call-log (M3.4)

### Requirement: Every tool call SHALL emit a structured tracing event with tool, duration_ms, and status

The system SHALL emit, at the centralized dispatch boundary in `call_tool_handler`, a `tracing::info!` event (or equivalent structured log line) for every tool call — successful, errored, gated, missing, or skipped — with at least the following fields:

- `tool`: the tool name (string)
- `duration_ms`: wall-clock duration in milliseconds (integer)
- `status`: the `classify_status` value (`ok | stub | gated | error | missing | skip`)

The log line SHALL be emitted in a structured format parseable by standard log tooling (e.g. JSON, logfmt, or `tracing` field syntax), such that `tool_call` records can be queried and filtered on those three fields.

#### Scenario: structured log emitted on successful call

- **Given** the MCP server is running with `tracing_subscriber` configured for structured output
- **When** a `tools/call` for tool `T` completes successfully through `call_tool_handler`
- **Then** exactly one log entry tagged `tool_call` (or with the message text `tool_call`) is emitted
- **And** the entry contains fields `tool=T`, `duration_ms=<integer ≥ 0>`, `status="ok"`

#### Scenario: structured log emitted on errored call (covers timeout and rate-limit)

- **Given** a tool call results in a timeout (M3.2) or a rate-limit denial (M3.3)
- **When** the call returns through `call_tool_handler`
- **Then** exactly one log entry tagged `tool_call` is emitted
- **And** the entry contains fields `tool=<name>`, `duration_ms=<integer>`, `status="error"`
- **And** the entry contains an `error_type` field whose value includes `"timeout"` or `"rate_limit_exceeded"` respectively

---

## Capability: auth-middleware (M3.5)

### Requirement: Bearer-token auth SHALL be enforced when COGNICODE_MCP_AUTH_TOKEN is set

The system SHALL read the environment variable `COGNICODE_MCP_AUTH_TOKEN` at server startup. When the variable is unset or empty, the system SHALL permit all incoming HTTP requests without an `Authorization` header (localhost/dev mode). When the variable is set to a non-empty value, the system SHALL enforce Bearer-token authentication on all HTTP routes (`/mcp`, `/health`, `/ready`, `/metrics`) — a request without an `Authorization: Bearer <token>` header matching the configured value SHALL be rejected with `401 Unauthorized`, and a request with a matching `Authorization: Bearer <token>` header SHALL be permitted through.

#### Scenario: no env var set — requests pass through

- **Given** `COGNICODE_MCP_AUTH_TOKEN` is unset (or set to the empty string) in the server's environment at startup
- **When** a request to `POST /mcp` is made WITHOUT an `Authorization` header
- **Then** the request is permitted through to the MCP streamable-HTTP service and the request is processed normally (no `401` is returned)

#### Scenario: env var set, no auth header — 401 Unauthorized

- **Given** `COGNICODE_MCP_AUTH_TOKEN=secret123` is set in the server's environment at startup
- **When** a request to `POST /mcp` is made WITHOUT any `Authorization` header
- **Then** the response status is `401 Unauthorized`
- **And** the request body is NOT processed by the MCP streamable-HTTP service (the auth check happens before routing)

#### Scenario: env var set, valid Bearer token — passes through

- **Given** `COGNICODE_MCP_AUTH_TOKEN=secret123` is set in the server's environment at startup
- **When** a request to `POST /mcp` is made WITH `Authorization: Bearer secret123`
- **Then** the request is permitted through to the MCP streamable-HTTP service and the request is processed normally (no `401` is returned)

#### Scenario: env var set, invalid Bearer token — 401 Unauthorized

- **Given** `COGNICODE_MCP_AUTH_TOKEN=secret123` is set in the server's environment at startup
- **When** a request to `POST /mcp` is made WITH `Authorization: Bearer wrong-token`
- **Then** the response status is `401 Unauthorized`
- **And** the request body is NOT processed by the MCP streamable-HTTP service

---

## Capability: deployment-runbook (M3.6)

### Requirement: A deployment runbook SHALL be published at docs/operations/mcp-server-runbook.md

The system SHALL publish a runbook document at `docs/operations/mcp-server-runbook.md` that allows a new operator to deploy, monitor, and troubleshoot the MCP server. The runbook SHALL cover:

1. **Deployment** — building the server binary, configuring the listen address, configuring PostgreSQL (when using Mode B), configuring the auth token, and starting the process.
2. **Monitoring** — how to scrape `/metrics`, how to read `/health` vs `/ready`, and how to interpret the structured per-call log lines.
3. **Troubleshooting** — common failure modes (graph not loaded, rate-limited clients, timeouts, auth failures) and their remediations.

#### Scenario: new operator can deploy the MCP server using the runbook

- **Given** a new operator has read access to `docs/operations/mcp-server-runbook.md`
- **When** the operator follows the runbook's deployment section end-to-end on a fresh host
- **Then** the operator can produce a running `cognicode-mcp-server` process that responds to `GET /health` with `200 OK`
- **And** the runbook explicitly states the env vars that must be set (`COGNICODE_MCP_AUTH_TOKEN`, `DATABASE_URL` for Mode B) and the required values for a basic deployment

#### Scenario: new operator can monitor and troubleshoot the MCP server using the runbook

- **Given** the MCP server is running in production
- **When** the operator follows the runbook's monitoring and troubleshooting sections
- **Then** the operator can identify and remediate at least these three failure modes by following the runbook alone (no external knowledge required): (a) `/ready` returns 503 (graph not yet built), (b) a client is rate-limited (M3.3), (c) a tool call timed out (M3.2)
- **And** the runbook explicitly cross-references the SLO document at `docs/operations/mcp-slos.md`

---

## Capability: slo-definitions (M3.7)

### Requirement: SLOs SHALL be published at docs/operations/mcp-slos.md with per-category latency targets

The system SHALL publish a service-level-objective (SLO) document at `docs/operations/mcp-slos.md` that defines, for every `cognicode_meta.category` in the registered tool set, a p99 latency target derived from the existing `estimated_latency_ms` annotations (M2.3). The SLO document SHALL include at minimum the following category-level targets (all p99 latencies unless stated otherwise):

| Category     | p99 latency target | Source budget     |
|--------------|--------------------|-------------------|
| `search`     | ≤ 500 ms           | ADR-034 §7        |
| `file`       | ≤ 200 ms           | estimated_latency_ms range |
| `navigation` | ≤ 1 s              | estimated_latency_ms range |
| `quality`    | ≤ 2 s              | estimated_latency_ms range |
| `refactor`   | ≤ 2 s              | estimated_latency_ms range |
| `graph`      | ≤ 5 s              | ADR-034 §7        |
| `composite`  | ≤ 5 s              | estimated_latency_ms range |
| `aix`        | ≤ 10 s             | estimated_latency_ms range |

The SLO document SHALL also state availability and error-rate targets for the MCP server as a whole (e.g. `99.5%` of non-error responses per calendar month; error rate `< 1%` of `cognicode.tool.calls{status="ok"}`).

#### Scenario: each tool category has a documented p99 latency target

- **Given** the SLO document is published at `docs/operations/mcp-slos.md`
- **When** an operator or SRE reads the SLO document
- **Then** for every `cognicode_meta.category` that appears in the registered tool set (i.e. `graph | search | refactor | quality | file | composite | navigation | aix`), the document lists a p99 latency target as a numeric value with explicit units (e.g. `≤ 500 ms`)
- **And** the document identifies which OTel histogram — `cognicode.tool.duration` — should be used to measure conformance

#### Scenario: SLO document is the source of truth for monitoring alerts

- **Given** the SLO document is published and Prometheus is scraping `/metrics`
- **When** an operator configures a latency alert on `histogram_quantile(0.99, sum by (le) (rate(cognicode_tool_duration_seconds_bucket[5m])))`
- **Then** the alert threshold SHALL be derivable from the per-category p99 targets in the SLO document
- **And** the SLO document states the per-category thresholds explicitly (no ambiguity about whether `search` alerts at 200 ms or 500 ms or 1 s)

---

## Capability: ci-smoke-gate (M2.12 — carried forward from M2)

### Requirement: The MCP smoke test SHALL run as a CI gate on every PR

The system SHALL publish a GitHub Actions workflow at `.github/workflows/mcp-smoke.yml` that, on every `pull_request` event (and on `push` to `main` / `develop`), builds the MCP server, starts it, runs `build_graph`, runs the existing `scripts/mcp/mcp_smoke_all.py` smoke against it, and fails the workflow (blocking the PR from being merged) if the smoke run reports any non-OK classification for a tool whose `annotations.stability = "stable"`.

#### Scenario: PR opened — smoke runs and blocks merge on failure

- **Given** `.github/workflows/mcp-smoke.yml` is published and the existing `mcp_smoke_all.py` smoke test exists
- **And** a pull request is opened against `main` (or `develop`)
- **When** the workflow is triggered by the `pull_request` event
- **Then** the workflow builds the MCP server, starts it, runs `build_graph`, and runs `mcp_smoke_all.py` against the live server
- **And** if the smoke run reports a `STUB` or `ERROR` classification for any tool with `annotations.stability = "stable"`, the workflow job exits with a non-zero status and the PR merge button is blocked by the required-check failure
- **And** if the smoke run passes (no stable-tool regression), the workflow job exits with status `success` and the PR is unblocked from a smoke-gate perspective

#### Scenario: workflow coexists with existing CI

- **Given** `.github/workflows/ci.yml` and `.github/workflows/perf-budget.yml` already exist
- **When** a PR is opened
- **Then** the new `mcp-smoke.yml` workflow runs as an additional required check alongside the existing workflows — it does NOT replace or modify `ci.yml` or `perf-budget.yml`

---

## Invariants Covered

- **I-1 (M1.1) — `call_tool_handler` is the universal instrumentation boundary** — verified by M3.2 (timeout wraps inside the boundary), M3.3 (rate-limit check inside the boundary), M3.4 (structured log emitted inside the boundary). No new instrumented paths are introduced.
- **I-3 (ADR-034 §2 / M2.3) — Every tool carries `cognicode_meta`** — verified by M3.2 (timeout mapping reads `category` from this object) and M3.3 (rate-limit gate reads `category` from this object). If annotations are missing, the default 30s timeout and default 100/60s rate limit apply.
- **I-7 (M1) — `classify_status` is total over the 6-value taxonomy** — verified by M3.4 (every log line carries one of the 6 status values); the log cannot emit a status outside the taxonomy.
- **I-8 (M3 NEW) — `/ready` and `/health` report independent state** — verified by M3.1 third scenario; this invariant prevents the common failure mode of conflating liveness with readiness.
- **I-9 (M3 NEW) — Auth is opt-in via env var** — verified by M3.5 first scenario; default behavior is no auth (preserves localhost dev workflow), auth is enforced only when the operator explicitly opts in. The CI gate (M2.12) MUST NOT depend on the auth env var being set.
- **I-10 (M3 NEW) — Rate-limit metric accounting is exact** — verified by M3.3 second scenario; a rate-limited call increments the error counter but does not double-count as a successful call.

## Open Questions

- **Per-tool timeout override mechanism** — Should the timeout mapping be hard-coded (per ADR-034 §7) or overridable via env var (e.g. `COGNICODE_MCP_TIMEOUT_GRAPH_MS=60000`)? Spec requires the mapping to be honored; the override surface is a design decision.
- **"Graph loaded AND queryable" for `/ready` in Mode B (PG)** — In Mode A (standalone) the contract is unambiguous (after first `build_graph`). In Mode B (PG), the question is whether `/ready` should also require an active PG connection, or whether a graph in the in-memory cache is sufficient. Spec anchors on "graph in memory and dispatchable"; design must confirm with the connection-pool wiring.
- **Auth middleware placement relative to `/metrics` and `/health`** — Should `/metrics` and `/health` be exempt from auth (to allow Prometheus scrapers and orchestrators to probe without holding a token), or should the spec require auth on ALL routes when the env var is set? Current spec wording requires auth on ALL routes. Design may need to split into two sub-routers if the operator wants the metrics endpoint to be scraper-friendly without a token.
- **"Graph analytics" scope for rate limit (M3.3)** — The acceptance criteria use the term "graph analytics". Spec anchors this to `cognicode_meta.category = "graph"`. The roadmap text says "expensive tools (graph analytics, LSP operations)" — should `navigation` (LSP) tools also get a per-tool sub-bucket, or is the default 100/60s rate limit sufficient for LSP? Spec only locks `graph`; `navigation` is a design follow-up.
- **CI workflow trigger scope for M2.12** — The acceptance criteria say "PR is opened". The existing `ci.yml` runs on both `push` and `pull_request`. Should `mcp-smoke.yml` match that pattern, or be PR-only? Spec only locks the `pull_request` behavior; `push` triggering is a design follow-up.
- **SLO availability target** — Spec requires the SLO document to state an availability target but does not pin a specific value (e.g. `99.5%` vs `99.9%`). Design must choose a value consistent with the operational posture (single-instance dev deployment vs multi-region production).
