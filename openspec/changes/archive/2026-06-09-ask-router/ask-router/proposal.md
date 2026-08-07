# Proposal: ask-router

## Intent

Agents must know exact tool names and argument shapes to query the 17-tool MCP surface. The `cognicode_ask` single-entry tool translates natural-language questions into dispatched primitive chains, eliminating the need to memorize 17 tool signatures.

## Scope

### In Scope
- `cognicode_ask` MCP tool accepting NL `question` + optional `context` args
- Keyword/pattern-based router (priority-ordered regex matching) over 17 primitives
- Internal dispatch through `ExplorerService` and `CallGraph` (NOT MCP chaining)
- `serde_json::Value` payload wrapping heterogeneous primitive results
- Router provenance: `source = "ask-router"`, confidence derived from match strength
- `suggested_follow_ups` populated with 1-3 `FollowUp` hints per result
- Graph availability gating for 9 graph-dependent tools
- Entity extraction: `spotter_search` fallback when NL entity is ambiguous
- 8 curated question patterns (see below)

### Out of Scope
- Q5 "What changed?" — requires snapshot diff (Phase 4)
- Q8 "What justifies X?" — requires Decision graph (Phase 3)
- Embedding-based or LLM-based classification
- New DTOs in `dto.rs` — reuse `McpResultEnvelope<serde_json::Value>`
- Changes to existing 17 tool implementations or their schemas

## Capabilities

### New Capabilities
- `ask-router`: keyword/pattern-based NL question router that classifies intent and dispatches to one or more primitive tool chains, returning a unified `McpResultEnvelope` with router provenance and follow-up suggestions

### Modified Capabilities
None — all 17 existing tools, DTOs, and service methods remain untouched.

## Approach

**Router design**: Priority-ordered regex patterns (most specific first) map NL questions to tool chains:

| Priority | Pattern | Tool Chain |
|----------|---------|-----------|
| 1 (highest) | `connects.*→.*\|path.*between\|how.*depends` | `spotter_search(x2)` → `impact_shortest_path` → `graph_explain` |
| 2 | `calls →\|what does.*call\|forward` | `spotter_search` → `impact_forward_radius` |
| 3 | `→ calls\|who calls\|callers\|depends on` | `spotter_search` → `impact_radius` → `get_view("call-graph")` |
| 4 | `risky\|quality\|smells` | `spotter_search` → `get_view("quality")` → `inspect_object` |
| 5 | `shape\|architecture\|cycles\|structure` | `impact_detect_cycles` → `graph_cluster` |
| 6 | `where.*start\|entry point\|overview\|workspace` | `open_workspace` → `graph_cluster` → `apply_lens("hotspots")` |
| 7 | `belongs\|component\|cluster` | `spotter_search` → `impact_component` → `inspect_object` |
| 8 | `what is\|describe\|explain` | `spotter_search` → `inspect_object` → `get_view("overview")` |

**Dispatch model**: Internal (co-located) — the ask router calls `service.` and `graph.` methods directly, sharing the same `Arc<ExplorerService>` and `Arc<CallGraph>` already held by `ExplorerMcpHandler`. No MCP-to-MCP chaining, no serialization overhead.

**Result envelope**: `McpResultEnvelope<serde_json::Value>` — payload is a merged JSON object with `primary_result` (key tool output) and `supporting` (auxiliary results). Provenance set to `ProvenanceMetadata { source: Some("ask-router"), confidence: match_score }`.

**Entity extraction**: When the NL question contains ambiguous entity names, the router runs `spotter_search(query, kind=None)` and surfaces top-3 candidates in `suggested_follow_ups` under a special `"entity_disambiguation"` follow-up.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `crates/cognicode-explorer/src/mcp.rs` | New | `cognicode_ask` dispatch arm + `AskArgs` struct + tool schema (added to `match` and `build_tool_schemas`) |
| `crates/cognicode-explorer/src/ask/` | New | Router module: `mod.rs`, `patterns.rs`, `dispatch.rs`, `entity.rs` |
| `crates/cognicode-explorer/src/service.rs` | None | Read-only usage via existing public API |
| `crates/cognicode-explorer/src/dto.rs` | None | No DTO changes |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Entity extraction: ambiguous NL names yield wrong `spotter_search` results | Medium | Surface top-3 candidates as follow-ups; let agent disambiguate |
| Graph gating: 9 tools fail when graph is absent | Medium | Pre-dispatch check: return `"graph unavailable"` error with available (non-graph) alternatives listed |
| Envelope heterogeneity: `serde_json::Value` payload shape varies per question | Low | Merge into stable `{primary_result, supporting}` structure; document per-pattern payload schema |
| Meaning connascence: patterns encode assumptions about primitive return shapes | Medium | Add integration tests per pattern against real service/graph output shapes |

## Rollback Plan

1. Remove `cognicode_ask` from `TOOL_NAMES` constant and `build_tool_schemas()`
2. Remove the `match` arm in `dispatch()`
3. Delete `crates/cognicode-explorer/src/ask/` directory
4. Existing 17 tools continue working — zero impact on existing surface

## Dependencies

- Existing `ExplorerService` public API (read-only)
- Existing `McpResultEnvelope`, `ProvenanceMetadata`, `FollowUp` types
- `CallGraph` held by `ExplorerMcpHandler` for 9 graph-dependent patterns

## Success Criteria

- [ ] `cognicode_ask` tool appears in `tools/list` with schema documenting `question` and `context` params
- [ ] All 8 curated NL questions dispatch correctly and return non-error envelopes
- [ ] Graph-absent mode returns graceful degradation for impact/graph questions
- [ ] Router provenance (`source = "ask-router"`) present in every response
- [ ] `suggested_follow_ups` non-empty for ambiguous entity extraction
- [ ] Integration tests cover all 8 patterns with mock service + mock graph

## Entropy Budget

**Method**: Heuristic (CogniCode unavailable)

| Metric | Estimate (bits) | Threshold | Status |
|--------|-----------------|-----------|--------|
| H(Δ_existing) | 0.0 | < 1.0 | ✅ OCP compliant |
| H(Δ_new) | 2.32 (5 components: router, patterns, dispatch, entity, follow-ups) | > 0 | ✅ |
| New connascence pairs introduced | 3 (Name: router ↔ ExplorerService API, Meaning: pattern assumptions about primitive shapes, Name: ask module ↔ McpResultEnvelope) | < 3 | ⚠️ Medium |
| OCP compliant? | Yes — pure extension | yes | ✅ |

**Breaking Change Indicators**: None — zero H(Δ_existing).
**Verdict**: 🟢 Green — low-coupling additive change with isolated router module.
