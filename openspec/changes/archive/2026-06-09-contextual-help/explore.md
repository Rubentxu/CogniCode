# Exploration: Contextual Help ("What Can I Do Here?")

**Change**: `contextual-help`
**Date**: 2026-06-09
**Phase**: sdd-explore

## Current State

### What the product docs say

`docs/explorer-graph/help-and-onboarding.md` (§"What Can I Do Here?") defines the feature precisely:
- Per-object-kind suggested question panel (3-5 prompts)
- Bound to the focused object via `object_kind`
- Content-driven, lives in the `docs/help` layer
- Updated reactively as the user navigates — a hotspot gains a risk prompt
- The first prompt is always the "where do I start" question for that kind
- Each prompt names a verb, an object, and a one-line expectation

`docs/explorer-graph/target-product-model.md` defines the 10 node kinds and 10 core user questions the product commits to answering. "What can I do here?" is not one of the 10 core questions — it's a meta-question that routes to the right core question.

`docs/explorer-graph/glossary.md` defines "Suggested question" as:
> A prompt the product offers a user based on the focused object kind. Suggested questions are the user-facing form of the curated question set in `target-product-model.md`. They are content, not code, and they live in the docs/help layer.

### What the codebase has today

**Follow-up system** (`crates/cognicode-explorer/src/ask/followups.rs`):
- Post-answer follow-ups, deterministic per `(QuestionCategory, entities, primary_result)`
- 8 categories, each produces 1–3 `FollowUp { tool, reason, kind }` entries
- `kind` discriminator: `related_inverse`, `hint`, `no_entity_match`, etc.
- The `suggested_follow_ups` field in `McpResultEnvelope` is populated ONLY by `cognicode_ask` — all 27 other tools emit an empty `[]`

**Ask router** (`crates/cognicode-explorer/src/ask/mod.rs`):
- Pure-function classifier: `AskRouter::classify(question) → ClassifiedQuestion`
- `ClassifiedQuestion { category, confidence, entities }`
- No per-object-kind awareness — patterns match on question text, not on the focused object

**Entity extraction** (`crates/cognicode-explorer/src/ask/entity.rs`):
- `extract_backtick_tokens(question) → Vec<String>`
- `extract_entities(question, service) → (Vec<ExtractedEntity>, Vec<FollowUp>)`
- Resolves tokens against `spotter_search` with ≥0.6 threshold

**MCP surface** (`crates/cognicode-explorer/src/mcp.rs`):
- 28 tools: 8 explorer, 6 impact, 3 graph, 1 ask, 6 brain-session, 4 named-views
- No `explorer_suggest_actions` or `explorer_get_help` tool exists
- The `McpResultEnvelope` has `suggested_follow_ups: Vec<FollowUp>` — infrastructurally present but unused outside `cognicode_ask`

**Object types** (`dto.rs` + `schemas.ts`):
- 9 types: `workspace`, `scope`, `symbol`, `file`, `module`, `evidence`, `decision_artifact`, `quality_issue`, `rule`
- The UI has full Zod schemas for all types

**UI** (`apps/explorer-ui/src/components/Shell.tsx`, `ObjectInspector/index.tsx`):
- 4-column responsive layout: MillerColumns | ObjectInspector | LensPanel | InteractiveGraph
- ObjectInspector shows: header (title + kind badge), ViewTabs (tab strip), Blocks (content area)
- The empty state says "No object selected — Drill into the Miller Columns or open the Spotter"
- **No contextual help panel, no suggested questions section exists today**
- The `useObject` hook fetches `InspectableObjectSummary` which includes `object_type`

## Affected Areas

| File/Path | Why Affected |
|-----------|-------------|
| `apps/explorer-ui/src/components/ObjectInspector/index.tsx` | Where the suggested question panel would render — the ObjectInspector owns the focused object |
| `apps/explorer-ui/src/components/Shell.tsx` | If help is a separate panel (4th column) rather than inline in the inspector |
| `apps/explorer-ui/src/state/context.ts` | May need new state if help content is fetched async |
| `crates/cognicode-explorer/src/mcp.rs` | If a new MCP tool is needed; if existing tools should populate `suggested_follow_ups` |
| `crates/cognicode-explorer/src/dto.rs` | If a new `SuggestActions` DTO is needed |
| `crates/cognicode-explorer/src/ask/followups.rs` | If the follow-up system is reused/adapted for proactive suggestions |
| `crates/cognicode-explorer/src/service.rs` | If a new service method for per-object suggestions is needed |
| `crates/cognicode-explorer/src/api.rs` | If a new REST endpoint exposes suggestions |
| `docs/explorer-graph/help-and-onboarding.md` | The per-kind suggestion content source — the content contract |
| `apps/explorer-ui/src/api/schemas.ts` | If a new Zod schema is needed for suggestion payloads |

## Suggested Questions Per Object Kind

From the product docs (`help-and-onboarding.md`), the v1 contract:

### Symbol (most common focused object)
| Prompt | Maps To | MCP Tool |
|--------|---------|----------|
| "What does this symbol do?" | `cognicode_ask` with `"what does \`{label}\` do?"` | `cognicode_ask` → `GenericDescription` |
| "Who calls this?" | `cognicode_ask` with `"who calls \`{label}\`?"` | `cognicode_ask` → `BackwardReach` |
| "What is risky to change here?" | `cognicode_ask` with `"is \`{label}\` risky?"` | `cognicode_ask` → `CodeQuality` |
| "Where does this belong?" | `cognicode_ask` with `"where does \`{label}\` belong?"` | `cognicode_ask` → `ComponentCluster` |
| "What justifies this?" | `explorer_inspect_object` → evidence view | `explorer_inspect_object` |

### File
| Prompt | Maps To | MCP Tool |
|--------|---------|----------|
| "What is in this file?" | `explorer_inspect_object` → overview view | `explorer_inspect_object` |
| "What is risky in this file?" | `cognicode_ask` with `"is \`{label}\` risky?"` | `cognicode_ask` → `CodeQuality` |
| "What changed in this file?" | `explorer_get_view` with view_id="changelog" | `explorer_get_view` |
| "Where does this file belong?" | `explorer_inspect_object` | `explorer_inspect_object` |

### Scope / Module
| Prompt | Maps To | MCP Tool |
|--------|---------|----------|
| "What lives in this scope?" | `explorer_inspect_object` | `explorer_inspect_object` |
| "What depends on this scope?" | `cognicode_ask` with `"who depends on \`{label}\`?"` | `cognicode_ask` → `BackwardReach` |
| "What changed in this scope?" | `explorer_get_view` with view_id="changelog" | `explorer_get_view` |

### Workspace
| Prompt | Maps To | MCP Tool |
|--------|---------|----------|
| "What are the moving parts?" | `explorer_open_workspace` | `explorer_open_workspace` |
| "What is the shape?" | `cognicode_ask` with `"architecture shape?"` | `cognicode_ask` → `Architecture` |
| "Where do I start?" | `cognicode_ask` with `"where to start?"` | `cognicode_ask` → `WorkspaceOverview` |

### Decision / Evidence / QualityIssue / Rule
These are less interactive in v1 — simpler prompts like "What does this justify?" / "What cites this?" mapped to `explorer_inspect_object`.

## Approaches

### 1. Frontend-Only — Static Suggestion Map

A static `Record<InspectableObjectType, SuggestedPrompt[]>` hardcoded in the UI. Each prompt has a `{ tool, params }` map. Clicking prepopulates the Spotter or calls the MCP tool directly.

```typescript
const SUGGESTIONS: Record<string, { question: string; tool: string; params: Record<string,string> }[]> = {
  symbol: [
    { question: "Who calls this?", tool: "cognicode_ask", params: { question: "who calls `${label}`?" } },
    { question: "What does this call?", tool: "cognicode_ask", params: { question: "what does `${label}` call?" } },
    // ...
  ],
};
```

| Pros | Cons |
|------|------|
| Zero backend changes | Suggestions stale without redeploy |
| Instantly fast (no network) | Content in code, not in docs/help layer as spec requires |
| Easy to A/B test | No dynamic suggestions (e.g., hotspot-aware prompts) |
| Trivial to implement | Can't adapt to graph state |

**Effort**: Low

### 2. Backend-Driven — New MCP Tool `explorer_suggest_actions`

A new MCP tool `explorer_suggest_actions(object_id)` that returns `Vec<SuggestedAction>` with `{ question, tool, params, rationale }`. The backend reads object type from `inspect_object`, applies per-type content from a `docs/help/suggestions.toml` file, optionally enriches with graph signals (is this a hotspot? → add risk prompt).

```rust
// New DTO
struct SuggestedAction {
    question: String,
    tool: String,
    params: serde_json::Value,
    rationale: String,
    priority: u8,
}
```

| Pros | Cons |
|------|------|
| Content in docs/help layer (spec-compliant) | New MCP tool = 29th tool |
| Dynamic — hotspot-aware suggestions possible | Requires backend change + API change + UI wiring |
| Single source of truth, versioned with code | Network latency on every focus change |
| Reuses `McpResultEnvelope` and existing `InspectableObjectSummary.object_type` | Over-engineering for v1 static prompts |

**Effort**: Medium

### 3. Extend `suggested_follow_ups` on Existing Tools

Instead of a new tool, populate the already-existing `suggested_follow_ups` field on `explorer_inspect_object`, `explorer_get_views`, etc. The UI reads this field from every tool response and renders it.

```rust
// In the inspect_object dispatch arm:
envelope_ok(
    TOOL_INSPECT_OBJECT,
    &service.inspect_object(&object_id),
    None,  // <- today: no follow-ups
)
// becomes:
envelope_ok(
    TOOL_INSPECT_OBJECT,
    &service.inspect_object(&object_id),
    Some(build_symbol_suggestions(&object_id)),  // <- populate follow-ups
)
```

| Pros | Cons |
|------|------|
| No new tool surface — reuses existing envelope | Pollutes the tool response with UI concerns |
| Piggybacks on existing API calls (no extra network) | Each tool needs its own suggestion builder |
| FollowUp struct already has `tool` + `reason` fields | Different tools would return different suggestion sets — inconsistent |
| Spec says "suggested follow-ups" should be non-empty | The envelope field was designed for post-answer, not pre-question |

**Effort**: Low–Medium

### 4. Hybrid — Frontend Static Map + Backend Hotspot Enrichment

Start with approach 1 (frontend static map). Add a lightweight `explorer_inspect_object` extension that returns an `active_signals` field (e.g., `{ is_hotspot: true, is_bridge: true }`). The frontend uses these signals to dynamically show/hide prompts from the static map.

| Pros | Cons |
|------|------|
| Fast initial implementation | Two-phase rollout |
| Content still lives in code (not docs/help) until phase 2 | Signal detection needs backend work |
| Graceful degradation — works without signals | |

**Effort**: Medium (phased)

## Recommendation

**Approach 1 (Frontend-Only Static Map) for the v1 implementation**, with a clear path to Approach 2 (backend-driven via `explorer_suggest_actions` or a `suggestions` field on `explorer_inspect_object`) when hotspot-aware prompts become necessary.

**Why approach 1 first:**
1. The spec says content lives in `docs/help` — but v1 can start in a `suggestedQuestions.ts` config file with a clear migration path to TOML/YAML later
2. Zero backend changes means this ships in one PR
3. The UI work is the hard part: placement, accessibility, keyboard navigation, responsive behavior at 4 breakpoints
4. The `InspectableObjectType` enum is stable — it won't change
5. Hotspot-aware prompts are a phase 2 concern; v1 just needs the per-kind static prompts

**How suggestions map to MCP tools:**

```
Symbol:
  "What does this symbol do?"      → cognicode_ask("what does `{label}` do?")
  "Who calls this?"                → cognicode_ask("who calls `{label}`?")
  "What is risky to change here?"  → cognicode_ask("is `{label}` risky?")
  "Where does this belong?"        → cognicode_ask("where does `{label}` belong?")
  "What justifies this?"           → explorer_inspect_object({object_id})

File:
  "What is in this file?"          → explorer_inspect_object({object_id})
  "What is risky in this file?"    → cognicode_ask("is `{label}` risky?")
  "Where does this file belong?"   → explorer_inspect_object({object_id})

Scope/Module:
  "What lives in this scope?"      → explorer_inspect_object({object_id})
  "What depends on this scope?"    → cognicode_ask("who depends on `{label}`?")
  "What changed in this scope?"    → explorer_get_view({object_id}, "changelog")

Workspace:
  "What are the moving parts?"     → explorer_open_workspace()
  "What is the shape?"             → cognicode_ask("architecture shape?")
  "Where do I start?"              → cognicode_ask("where to start?")

Decision:
  "What does this justify?"        → explorer_inspect_object({object_id})
  "What contradicts this?"         → explorer_get_view({object_id}, "evidence")

Issue:
  "What does this resolve?"        → explorer_get_view({object_id}, "resolves")
  "What cites this?"               → explorer_get_view({object_id}, "cited-by")

Evidence/QualityIssue/Rule:
  "Inspect this in context"        → explorer_inspect_object({object_id})
```

**UI surface for suggestions:**

Per the progressive disclosure strategy in `help-and-onboarding.md`, the suggested question panel should appear:
- **Inline in the ObjectInspector**, between the header (title + kind badge) and the ViewTabs
- As a horizontal pill-strip of 3-5 clickable chips/buttons
- Each chip: question text as label, icon/emoji prefix for the tool category
- Clicking dispatches the corresponding MCP tool call and focuses the result
- On tablet/small viewports: collapses to a single "What can I do?" button that opens a popover

**Does clicking trigger cognicode_ask?**

For question-based prompts: YES. For inspect/view-based prompts: use the direct MCP tool. The `cognicode_ask` router already handles all 8 pattern categories. Formatting the prompt as a natural-language question and routing through `cognicode_ask` gives the user the full `McpResultEnvelope` with `primary_result`, `supporting`, and `suggested_follow_ups`.

## Risks

- **Risk 1 — Content drift**: If the static frontend map drifts from the `docs/help` content, users get suggestions that don't match the help text. Mitigation: add a CI check that validates suggestions config against the docs.
- **Risk 2 — cognicode_ask requires graph**: 6 of 8 patterns are graph-dependent. If the user opens a workspace without building the call graph, those prompts will return `graph_unavailable`. Mitigation: detect graph status from `WorkspaceSummary.graph_status` and show/hide graph-dependent prompts.
- **Risk 3 — UI clutter at small breakpoints**: 5 suggestion pills on a phone screen is noise. Mitigation: collapse to a single "What can I do?" popover below 900px (already the "small" Shell viewport).
- **Risk 4 — ObjectInspector doesn't know about MCP tools**: The ObjectInspector today only reads REST API data, not MCP. Suggestion clicks need to either call REST endpoints or the MCP bridge. Mitigation: reuse existing `useObject` / `useViews` hooks for inspect/get_view prompts; add a lightweight `useAsk` hook for `cognicode_ask` prompts.

## Ready for Proposal

**Yes.** The exploration answers all key questions:
1. ✅ Per-object suggestions defined (from product docs)
2. ✅ Recommendation: frontend-first static map (approach 1)
3. ✅ UI placement: inline in ObjectInspector between header and ViewTabs
4. ✅ Click behavior: dispatch `cognicode_ask` or direct tool calls
5. ✅ 9 object types mapped to 3-5 prompts each
6. ✅ No new MCP tool needed for v1
7. ✅ Clear migration path to approach 2 when hotspot-aware prompts are needed

### Entropy Analysis (Connascence Landscape)

**Method**: Heuristic

| Component A | Component B | Connascence Type | I(bits) | Severity |
|-------------|-------------|------------------|---------|----------|
| `mcp.rs` (FollowUp struct) | `ask/followups.rs` | Name | ~1.0 (2 files) | ⚠️ Medium |
| `ask/patterns.rs` (QuestionCategory) | `ask/dispatch.rs` | Type | ~1.58 (3 files) | ⚠️ Medium |
| `ask/entity.rs` (FollowUp struct) | `mcp.rs` | Name | ~1.0 (2 files) | ✅ Low |
| `dto.rs` (InspectableObjectType) | `api/schemas.ts` | Meaning | ~2.0 (cross-language enum) | ⚠️ HIGH — HIDDEN |
| `Shell.tsx` | `ObjectInspector/index.tsx` | Composition (Name) | ~1.0 (2 files) | ✅ Low |
| `help-and-onboarding.md` (content contract) | UI code | Meaning | ~1.0 (documentation-dependent) | ⚠️ Medium — HIDDEN |

**Critical Pairs (I > 3.0 bits)**: None

**Hidden Connascence (Meaning/Timing)**:
- `dto.rs` ↔ `api/schemas.ts`: The `InspectableObjectType` enum is mirrored across Rust and TypeScript. Adding a type in one must propagate to the other. This is high-risk meaning connascence — a type exists in Rust but not in Zod = silent parse failure. Adding a new object type would trigger this.
- `help-and-onboarding.md` ↔ UI suggestion config: The per-kind suggestion list in the doc is the content contract. If suggestions change in the doc but not in code, the UI becomes stale. This is content-connascence.

**SOLID-Entropy Violations**: None in the exploration scope. This change is additive.

**Coupling Score**: H_external ≈ 1.6 bits (2 main connascence pairs between frontend and backend)
**Recommendation**: Accept for v1. The cross-language type mirroring is pre-existing debt, not introduced by this change.
