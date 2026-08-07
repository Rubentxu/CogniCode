# Exploration: mcp-result-envelope — Standardized MCP Tool Result Envelope

## Current State

The 17 MCP tools in `crates/cognicode-explorer/src/mcp.rs` produce return values through two distinct serialization paths with **no envelope wrapper**. Every tool returns bare JSON — consumers receive raw payloads with zero metadata about provenance, tool identity, generation timestamp, or result version.

The roadmap (`docs/explorer-graph/roadmap.md`, Phase 2) explicitly calls for:

> "Every MCP tool result carries provenance and confidence metadata in a stable envelope."

And defines the envelope fields: **payload, provenance, confidence, suggested follow-up questions.**

### Two Serialization Paths

| Path | Helper | Used By | Return Shape | Error Encoding |
|------|--------|---------|-------------|----------------|
| `ok()` | Wraps `ExplorerResult<T>` | 8 explorer tools | `Result` → success text or error text | `CallToolResult::error` via `ExplorerError::Display` |
| `ok_direct()` | Serializes `T` directly | 9 graph-tools (6 impact + 3 graph-primitives) | Raw `T` JSON | `CallToolResult::error` via inline `err()` |

### Return Type Catalog — All 17 Tools

#### Group A: Explorer Tools (via `ok()` → `ExplorerResult<T>`)

| # | Tool | Return Type | Has provenance? | Has confidence? |
|---|------|------------|-----------------|-----------------|
| 1 | `explorer_open_workspace` | `WorkspaceSummary` | ❌ | ❌ |
| 2 | `explorer_spotter_search` | `Vec<SpotterResult>` | ❌ (only per-object `score: f32`) | ❌ |
| 3 | `explorer_inspect_object` | `InspectableObjectSummary` | ❌ | ❌ |
| 4 | `explorer_get_views` | `Vec<ViewDescriptor>` | ❌ | ❌ |
| 5 | `explorer_get_view` | `ContextualView` | ✅ (via `TypedRelation.provenance: Option<String>`) | ✅ (via `TypedRelation.confidence: Option<f64>`, `EvidenceBlock.confidence: Option<f32>`) |
| 6 | `explorer_get_lenses` | `Vec<LensDescriptor>` | ❌ | ❌ |
| 7 | `explorer_apply_lens` | `LensResult` | ❌ | ✅ (via `DesignFinding.confidence: f32`) |
| 8 | `explorer_query_moldql` | `MoldQLResultDto` | ❌ | ❌ |

Inner-entity metadata exists on `TypedRelation` (provenance, confidence), `EvidenceBlock` (provenance, confidence), and `DesignFinding` (confidence) — but only in `explorer_get_view` and `explorer_apply_lens`.

#### Group B: Impact Tools (via `ok_direct()` → raw values)

| # | Tool | Return Type | Has provenance? | Has confidence? |
|---|------|------------|-----------------|-----------------|
| 9 | `impact_radius` | `Vec<String>` (symbol IDs) | ❌ | ❌ |
| 10 | `impact_forward_radius` | `Vec<String>` (symbol IDs) | ❌ | ❌ |
| 11 | `impact_has_path` | `HasPathResult {from, to, has_path}` | ❌ | ❌ |
| 12 | `impact_shortest_path` | `Option<PathResultDto>` | ❌ | ✅ (`total_cost` derived from `1 - confidence`) |
| 13 | `impact_detect_cycles` | `Vec<SccDto {members, size}>` | ❌ | ❌ |
| 14 | `impact_component` | `Option<Vec<String>>` | ❌ | ❌ |

#### Group C: Graph-Primitive Tools (via `ok_direct()` → structured DTOs)

| # | Tool | Return Type | Has provenance? | Has confidence? |
|---|------|------------|-----------------|-----------------|
| 15 | `graph_subgraph` | `SubgraphResultDto {nodes, edges}` | ❌ | ✅ (per-edge `confidence: f64`) |
| 16 | `graph_cluster` | `ClusterResultDto([{members, size}])` | ❌ | ❌ |
| 17 | `graph_explain` | `ExplainResultDto {found, hops, total_cost, summary}` | ❌ | ✅ (per-hop `confidence: f64`) |

### Key Inconsistencies

1. **No tool identity in results.** Consumers cannot tell which tool produced a result without round-tripping the request. `impact_radius` and `impact_forward_radius` both return `Vec<String>` — identical JSON shapes, opposite semantics.

2. **No timestamp.** Results are atemporal. An agent cannot know if a result is fresh, stale, or from a different invocation.

3. **No version.** If tool semantics change across versions, consumers have no signal.

4. **Heterogeneous error handling.** Group A errors go through `ExplorerError::Display` → `CallToolResult::error`. Group B/C errors go through `err()` → `CallToolResult::error`. The error text shape differs (Group A includes full service error details; Group B/C include `"missing required arg"` / `"impact analysis unavailable"`).

5. **Bare raw arrays.** `impact_radius` and `impact_forward_radius` return untyped `Vec<String>` — indistinguishable at the JSON level.

6. **Inconsistent null handling.** `impact_shortest_path` returns JSON `null` for unreachable targets. `graph_explain` returns structured `{found: false}` instead. Same conceptual result, different wire shapes.

7. **`ok()` vs `ok_direct()` divergence.** Two separate helper functions doing the same thing (`serde_json::to_string_pretty` + `CallToolResult::success`) with different error wrapping — coupling risk.

## Affected Areas

| File | Role | Impact |
|------|------|--------|
| `crates/cognicode-explorer/src/mcp.rs` (2,398 lines) | All 17 tool dispatch arms + `ok()`/`ok_direct()` helpers + schemas | Primary change site: every dispatch arm must wrap result in envelope; merge `ok()`/`ok_direct()` into one helper |
| `crates/cognicode-explorer/src/dto.rs` (360 lines) | DTO definitions (`TypedRelation`, `EvidenceBlock`, `ContextualView`, `MoldQLResultDto`, etc.) | New `McpResultEnvelope<T>` struct and per-tool payload type aliases |
| `crates/cognicode-core/src/application/dto/impact_dto.rs` (382 lines) | Impact DTOs (`PathResultDto`, `SccDto`, `SubgraphResultDto`, `ExplainResultDto`, etc.) | Envelope wraps these; no structural change to DTOs themselves |
| `crates/cognicode-core/src/application/services/impact_analysis.rs` (939 lines) | `ImpactAnalysisService` — produces return values for 9 tools | No change needed — service returns pure values; envelope is MCP-layer concern |
| `crates/cognicode-explorer/src/service.rs` (1,955 lines) | `ExplorerService` — produces `ExplorerResult<T>` for 8 tools | No change needed — service returns pure values; envelope is MCP-layer concern |
| `crates/cognicode-explorer/src/bin/mcp.rs` | MCP server binary | No structural change; tests may need envelope-aware assertion helpers |
| `docs/explorer-graph/roadmap.md` | Phase 2 spec: "Every MCP tool result carries provenance and confidence metadata in a stable envelope." | Reference document; defines the contract |
| `docs/explorer-graph/target-product-model.md` | Confidence/provenance semantics | Defines the model values the envelope exposes |
| `crates/cognicode-explorer/src/mcp.rs` tests (lines 968–2398) | 40+ dispatch tests asserting raw JSON shapes | All tests must be updated to unwrap envelope before asserting payload |

## Approaches

### Approach 1: Generic Envelope Wrapper (Recommended)

Introduce a single `McpResultEnvelope<T>` struct that wraps every tool result:

```rust
struct McpResultEnvelope<T: Serialize> {
    tool_name: String,            // e.g. "graph_subgraph"
    version: String,              // e.g. "1.0.0" from CARGO_PKG_VERSION
    timestamp: String,            // ISO-8601 timestamp
    provenance: ProvenanceSummary, // overall result provenance
    payload: T,                   // the existing tool-specific JSON
    suggested_follow_ups: Vec<String>, // agent UX hints (Phase 2 roadmap)
}
```

- **Pros**: Single type, single serialization path, backward-compatible (new fields additive), replaces both `ok()` and `ok_direct()`, TypeScript/JSON consumers can deserialize generically
- **Cons**: All 17 tools change simultaneously; ~40 tests need updating; migration plan must handle old clients
- **Effort**: Medium (200–300 lines of code, ~40 test updates)

### Approach 2: Per-Tool Envelope Structs

Define 17 separate envelope structs (e.g., `ImpactRadiusEnvelope`, `SubgraphEnvelope`, etc.), each with its own payload field type.

- **Pros**: Type-safe at the Rust level; no generic parameter
- **Cons**: 17× code duplication; harder to enforce consistency; agents must know 17 envelope shapes; defeats the purpose of a "stable envelope"
- **Effort**: High (500+ lines, high maintenance burden)

### Approach 3: Sidecar Metadata (Hybrid)

Keep existing payloads untouched. Add a parallel `explorer_result_metadata` tool that returns timestamp/provenance for the most recent invocation.

- **Pros**: Zero breaking changes; fastest to implement
- **Cons**: Requires two round-trips per query; fragile (metadata could desync from result); violates roadmap spec of a single envelope
- **Effort**: Low (50–100 lines)

## Recommendation

**Approach 1 — Generic Envelope Wrapper.** This is what the roadmap specifies. The existing code already has partial metadata flowing through inner DTOs (`TypedRelation.provenance/confidence`, `EvidenceBlock.provenance/confidence`). The envelope adds the outer wrapper that agents expect for every tool call: identity, timing, version, and suggested follow-ups.

### Strategy: Wrap, Don't Restructure

The payload field (`T`) is the existing JSON the tool already produces. We do NOT restructure existing DTOs — we wrap them:

```
BEFORE:           AFTER:
{                 {
  "nodes": [...],   "tool_name": "graph_subgraph",
  "edges": [...]    "version": "0.4.0",
}                   "timestamp": "2026-06-09T12:00:00Z",
                    "provenance": {"source": "call_graph", "confidence_hedge": "edges_from_ast"},
                    "payload": {
                      "nodes": [...],
                      "edges": [...]
                    },
                    "suggested_follow_ups": ["graph_explain", "impact_radius"]
                  }
```

### Migration: Phase It, Don't Break

1. **Phase A (this change)**: Introduce `McpResultEnvelope<T>`, update all 17 dispatch arms, update all tests, merge `ok()`/`ok_direct()` into single `ok_envelope()` helper.
2. **Phase B (next change)**: Add `suggested_follow_ups` population logic (requires a question router — dependency on Phase 2 roadmap).
3. **Phase C (Phase 2 roadmap)**: "Ask" entry point routes natural language to primitives with envelope.

### EvidenceBlock Relationship

`EvidenceBlock` is an **inner entity** — it lives inside the payload (e.g., inside `ContextualView.evidence`). The envelope wraps the **outer result**. They serve different layers:

| Layer | Struct | Carries |
|-------|--------|---------|
| Outer (envelope) | `McpResultEnvelope<T>` | tool identity, version, timestamp, result-level provenance, follow-up questions |
| Inner (per-entity) | `TypedRelation` / `EvidenceBlock` | per-edge provenance and confidence |

`EvidenceBlock` is NOT replaced or subset by the envelope — it's nested inside `payload`. The two coexist at different semantic levels.

### Envelope Field Design

| Field | Type | Source | Rationale |
|-------|------|--------|-----------|
| `tool_name` | `String` | `TOOL_*` constant | Self-describing results; agents can disambiguate identical payload shapes |
| `version` | `String` | `env!("CARGO_PKG_VERSION")` | Agents can detect version drift |
| `timestamp` | `String` | `chrono::Utc::now().to_rfc3339()` | Freshness signal for agent reasoning |
| `provenance` | `ProvenanceSummary` | Context-dependent (see below) | Machine-readable result trust signal |
| `payload` | `T` | Existing tool return value | Unchanged — backward compatible at the semantic level |
| `suggested_follow_ups` | `Vec<String>` | Hardcoded per-tool (Phase A), dynamic router (Phase B) | Agent UX per roadmap |

#### ProvenanceSummary

```rust
struct ProvenanceSummary {
    data_source: String,          // "call_graph", "symbol_repository", "fts5_index"
    extraction_method: String,    // "direct_ast", "heuristic_inference", "manual"
    confidence_range: Option<ConfidenceRange>, // aggregate confidence bounds
}

struct ConfidenceRange {
    min: f64,
    max: f64,
}
```

For Group B/C tools: `data_source = "call_graph"`, `extraction_method = "direct_ast"` (edges are AST-extracted). For Group A tools: depends on the tool (`symbol_repository`, `fts5_index` for spotter_search, etc.).

## Risks

1. **Test blast radius.** ~40 dispatch tests assert raw JSON shapes. Must update every test to unwrap `.payload` before assertions. Mitigation: add a test helper `unwrap_envelope(text: &str) -> serde_json::Value` and use it everywhere. Effort: 1–2 hours of mechanical updates.

2. **Breaking change for MCP consumers.** Agents that parse the raw JSON of today's results will see a new outer wrapper. Mitigation: the payload field is the EXISTING JSON shape — agents need to navigate one level deeper into `result.payload`. Document the migration; the `tool_name` + `version` fields let agents detect the new format and adapt.

3. **`ok()` / `ok_direct()` merge complexity.** Currently `ok()` wraps `ExplorerResult<T>` and handles `Ok`/`Err` branching; `ok_direct()` serializes `T` directly. Merging them into a single `ok_envelope()` requires handling `ExplorerResult` variants within the helper. Mitigation: replace both with a single `envelope_ok_result<T>` that takes `ExplorerResult<T>` and a second `envelope_ok_direct<T>` for raw values — but both produce identical envelope shapes. Phase 2 can merge them further.

4. **ProvenanceSummary accuracy.** Group A tools (`spotter_search`) blend exact matches with FTS5 fuzzy results — the `confidence_range` should reflect this. Mitigation: start with `data_source` only; add `confidence_range` in a follow-up when individual result-level confidence tracking lands.

5. **Timestamp precision.** `chrono::Utc::now().to_rfc3339()` produces sub-second timestamps. For high-throughput agent loops, multiple results may share the same second. Mitigation: rfc3339 format already supports fractional seconds; document that timestamp is "at generation time" and use it for ordering, not identity.

6. **Suggested follow-ups static in Phase A.** Hardcoding `suggested_follow_ups: vec![]` or per-tool static lists is acceptable for this change. The Phase 2 "ask" router will make them dynamic. Mitigation: document as a "Phase A stub" in the design.

## Entropy Analysis (Connascence Landscape)

**Method**: Heuristic (code reading — CogniCode unavailable for quantitative estimation).

### Connascence Pairs

| Component A | Component B | Type | I(bits) | Severity |
|-------------|-------------|------|---------|----------|
| `ok()` helper | Group A dispatch arms (×8) | Name | ~3.0 | ⚠️ Medium |
| `ok_direct()` helper | Group B/C dispatch arms (×9) | Name | ~3.2 | ⚠️ Medium |
| `TOOL_*` constants (×17) | `dispatch()` match arms (×17) | Name | ~4.1 | ❌ High |
| `TOOL_*` constants | `build_tool_schemas()` (×17) | Name | ~4.1 | ❌ High |
| `ExplorerResult<T>` type | 8 explorer service methods | Type | ~3.0 | ⚠️ Medium |
| `CallToolResult` (rmcp) | `ok()`/`ok_direct()`/`err()` helpers | Type | ~0.5 | ✅ Low |
| `serde_json::to_string_pretty` | `ok()` + `ok_direct()` | Algorithm | ~1.0 | ⚠️ Low |
| `TypedRelation.provenance` | `EvidenceBlock.provenance` | Meaning | ~1.0 | ⚠️ Hidden |
| `CallToolResult::error` shape | All 17 dispatch arms (error paths) | Meaning | ~3.0 | ⚠️ Hidden |
| Service return type | `ok()` vs `ok_direct()` choice | Meaning | ~2.5 | ⚠️ Hidden |

### Critical Pairs (I > 3.0 bits)

- **`TOOL_*` constants → `dispatch()` match arms**: Each constant is matched in `dispatch()` and separately listed in `build_tool_schemas()`. Adding a new tool touches 3 independent locations. The current 17-tool surface has I ≈ 4.1 bits (log2(17) ≈ 4.09). The envelope change adds another reference point per tool. **Mitigation**: after this change, consider a declarative tool registry that derives both the schema and the dispatch arm from a single source.

### Hidden Connascence

- **`CallToolResult::error` shape inconsistency**: Error text from `ExplorerError::Display` (Group A) differs from inline `err("missing required arg")` text (Group B/C). Agents parsing error text for structured recovery are coupled to text shapes that are NOT contractually documented. The envelope change does NOT fix this — but documenting it is the first step toward a structured error format (future change).

### SOLID-Entropy Violations

- **OCP**: Adding a new tool currently requires modifying `dispatch()` (adding a match arm), `TOOL_NAMES`, and `build_tool_schemas()`. H(Δ_existing) ≈ 3–4 bits per new tool. The envelope change does NOT worsen this — it inherits the existing coupling. A tool registry (future refactor) would reduce H(Δ_existing) to < 1.0 bit.

- **SRP**: `dispatch()` currently does 3 things: arg parsing, graph guard checking, service call + serialization. F ≈ 1.58 bits. The envelope change slightly reduces F by consolidating serialization into the envelope helper — a minor SRP improvement.

### Coupling Score

- **H_external**: ~3.5 bits average across tool/dispatch coupling. In the "Medium-High" range. The envelope change introduces new coupling (`McpResultEnvelope<T>` type × 17 dispatch arms) but this is **additive structure**, not additional coupling — it creates a single point of consistency that 17 arms depend on, rather than 17 independent serialization paths.

### Entropy Budget for This Change

| Metric | Estimate (bits) | Threshold | Status |
|--------|-----------------|-----------|--------|
| H(Δ_existing) | ~1.5 (test updates mechanical) | < 1.0 | ⚠️ Slightly above |
| H(Δ_new) | ~4.0 (new type + helper) | > 0 | ✅ |
| New connascence pairs | 1 (envelope → all dispatch arms) | < 3 | ✅ |
| OCP compliant? | Partial (dispatch needs modification) | — | ⚠️ Acceptable for structural change |

**Verdict**: 🟡 The entropy cost is justified — this change replaces 2 divergent serialization paths with 1 consistent envelope. The net information-theoretic cost is negative over time: future tool additions will have LOWER coupling (one envelope pattern to follow, not two).

## Ready for Proposal

**Yes.** The roadmap specifies this, the gap is well-understood, and the approach is additive. The primary risk is test update volume (mechanical, not semantic). The envelope should be a single generic `McpResultEnvelope<T>` struct with `tool_name`, `version`, `timestamp`, `provenance`, `payload`, and `suggested_follow_ups` fields, wrapping all 17 tools uniformly.
