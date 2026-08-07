# Design: ask-router

## Technical Approach

A single `cognicode_ask` MCP tool accepts a natural-language `question` string. A new `ask/` module hosts a pure-function router (`AskRouter`) that classifies the question against 8 priority-ordered regex patterns, extracts entity tokens, and returns a `ClassifiedQuestion`. A thin async dispatch layer (`dispatch.rs`) calls `ExplorerService` and `ImpactAnalysisService` methods directly — no MCP chaining, no re-serialization. Every response wraps through the existing `McpResultEnvelope<serde_json::Value>` with `provenance.source = "ask-router"`.

## Architecture Decisions

| Decision | Choice | Rejected | Rationale |
|----------|--------|----------|-----------|
| Module placement | `crates/cognicode-explorer/src/ask/` (4 files) | Inline in `mcp.rs` | `mcp.rs` is 2600+ LOC; the router is 500+ LOC of new logic. Separate module follows the `moldql/` precedent. |
| Pattern storage | `const` slice of `QuestionPattern` structs | HashMap lookup | Compile-time constant, priority-ordered, zero alloc. Small surface (8 patterns) doesn't need dynamic registration. |
| Pattern matching | `regex::Regex` on lowercased question | Pure keyword `contains` | Regex handles multi-word patterns (`path.*between`) in one pass. `regex` is already a transitive dep via `moldql/parser`. |
| Entity extraction | Backtick-quoted tokens + `spotter_search` fallback | NLP/NER | Backtick convention is how agents already embed symbols. Zero-cost extraction from the question string. |
| Follow-up generation | Static table per `QuestionCategory` | LLM-computed | Deterministic, testable, no external deps. The spec requires `(pattern, primary_result)` determinism. |
| Graph gating | Pre-dispatch `Option<Arc<CallGraph>>` check in each graph-dependent arm | Runtime error from `ImpactAnalysisService` | Spec requires *before any primitive call* — surfacing available alternatives is impossible after a generic error. |
| Result shape | `{ primary_result: Value, supporting: Map<String, Value> }` | Per-pattern typed structs | Heterogeneous primitive outputs don't share a schema. `serde_json::Value` matches existing `envelope_ok_direct` pattern. |

## Data Flow

```
cognicode_ask(question, context)
        │
        ▼
   AskRouter::classify(question) ──► ClassifiedQuestion { category, confidence, entities }
        │
        ├─ graph_required? ──► require_graph(graph) ──► Err(graph_unavailable)
        │
        ▼
   dispatch_by_category(category, entities, service, graph)
        │
        ├── spotter_search(entity) ──► entity disambiguation check
        │
        ▼
   primitive chain execution (service.*, ImpactAnalysisService::*)
        │
        ▼
   McpResultEnvelope { payload: {primary_result, supporting}, provenance: "ask-router", suggested_follow_ups }
```

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `src/ask/mod.rs` | Create | Module root — re-exports `AskRouter`, `QuestionCategory` |
| `src/ask/patterns.rs` | Create | `QuestionPattern` struct, `PATTERNS` const slice (8 entries), `QuestionCategory` enum |
| `src/ask/dispatch.rs` | Create | `dispatch_ask()` async fn — pattern match on category, call primitives, build envelope |
| `src/ask/entity.rs` | Create | `extract_entities()` — backtick extraction + `spotter_search` disambiguation |
| `src/ask/followups.rs` | Create | `generate_follow_ups()` — static table per category, produces `Vec<FollowUp>` |
| `src/lib.rs` | Modify | Add `pub mod ask;` |
| `src/mcp.rs` | Modify | Add `TOOL_ASK` constant, `AskArgs` struct, dispatch arm in `match name {}`, tool schema in `build_tool_schemas()` |

## Interfaces / Contracts

```rust
// ask/patterns.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestionCategory {
    PathBetween,     // priority 1, graph-dependent
    ForwardReach,    // priority 2, graph-dependent
    BackwardReach,   // priority 3, graph-dependent
    CodeQuality,     // priority 4, NOT graph-dependent
    Architecture,    // priority 5, graph-dependent
    WorkspaceOverview, // priority 6, graph-dependent
    ComponentCluster,  // priority 7, graph-dependent
    GenericDescription, // priority 8, NOT graph-dependent (fallback)
}

pub struct QuestionPattern {
    pub category: QuestionCategory,
    pub regex: &'static str,
    pub priority: u8,
    pub graph_required: bool,
}

pub const PATTERNS: &[QuestionPattern] = &[ /* 8 entries */ ];

// ask/mod.rs
pub struct ClassifiedQuestion {
    pub category: QuestionCategory,
    pub confidence: f64,
    pub entities: Vec<String>,
}

pub struct AskRouter;

impl AskRouter {
    pub fn classify(question: &str) -> ClassifiedQuestion { /* pure fn */ }
}

// ask/dispatch.rs
pub async fn dispatch_ask(
    classified: ClassifiedQuestion,
    service: &Arc<ExplorerService>,
    graph: &Option<Arc<CallGraph>>,
) -> McpResultEnvelope<serde_json::Value> { /* ... */ }

// ask/entity.rs
pub async fn extract_entities(
    question: &str,
    service: &Arc<ExplorerService>,
) -> (Vec<String>, Vec<FollowUp>) { /* entities, disambiguation follow-ups */ }

// ask/followups.rs
pub fn generate_follow_ups(
    category: QuestionCategory,
    entities: &[String],
    primary_result: &serde_json::Value,
) -> Vec<FollowUp> { /* deterministic per (category, primary_result) */ }

// mcp.rs additions
pub const TOOL_ASK: &str = "cognicode_ask";

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct AskArgs {
    question: Option<String>,
    context: Option<serde_json::Value>,
}
```

## Testing Strategy

| Layer | What to Test | Approach |
|-------|-------------|----------|
| Unit | `AskRouter::classify()` — each of 8 patterns + fallback + overlap priority | `#[test]` with static strings; no service/graph needed |
| Unit | `extract_entities()` — backtick parsing, 0/1/many results | Mock `ExplorerService` via `TestRepo` |
| Unit | `generate_follow_ups()` — deterministic output per (category, entities) | Pure fn, assert exact output |
| Unit | `dispatch_ask()` — each of 8 categories | `#[tokio::test]` with `build_test_service()` + `make_impact_graph()` |
| Integration | `cognicode_ask` in `dispatch()` — end-to-end from args to envelope | Existing `dispatch()` + `call_tool_args()` pattern in `mcp::tests` |
| TDD RED gate | 18 failing tests before any implementation | Tests reference `AskRouter`, `QuestionCategory`, `TOOL_ASK`, dispatch arm — all undefined |

### TDD Sequence (RED gates first)

1. `test_ask_tool_registered_in_tool_list` — `build_tool_schemas().len() == 18`
2. `test_ask_router_classifies_path_between` — `AskRouter::classify("path between foo and bar")`
3. `test_ask_router_classifies_forward_reach`
4. `test_ask_router_classifies_backward_reach`
5. `test_ask_router_classifies_code_quality`
6. `test_ask_router_classifies_architecture`
7. `test_ask_router_classifies_workspace_overview`
8. `test_ask_router_classifies_component_cluster`
9. `test_ask_router_classifies_generic_fallback`
10. `test_ask_router_priority_wins_on_overlap`
11. `test_ask_dispatch_path_between_success`
12. `test_ask_dispatch_graph_unavailable_error`
13. `test_ask_dispatch_entity_disambiguation`
14. `test_ask_dispatch_no_entity_match`
15. `test_ask_followups_inverse_direction`
16. `test_ask_followups_path_includes_dependency`
17. `test_ask_dispatch_missing_question_error`
18. `test_ask_envelope_provenance_source`

## Migration / Rollout

No migration required. Additive change — the 17 existing tools are untouched. `cognicode_ask` is an 18th tool that appears in `tools/list` alongside the others.

## Open Questions

- [ ] Should `context` arg in `AskArgs` influence routing (e.g. override pattern), or is it purely reserved for future use?
