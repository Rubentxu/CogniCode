# Exploration: ask-router

## Current State

The `crates/cognicode-explorer/src/mcp.rs` module exposes **17 MCP tools** as a flat surface — the agent must know exactly which tool to call with exactly the right arguments. There is no natural-language entry point. The `query-and-navigation.md` product document envisions a `cognicode_ask` tool that routes natural-language questions to the right verb chain, but this tool does not exist.

All 17 tools already return a standardized `McpResultEnvelope<T>` with six fields (`tool_name`, `version`, `timestamp`, `provenance`, `payload`, `suggested_follow_ups`). The `suggested_follow_ups` field is reserved but currently always empty.

## Affected Areas

- `crates/cognicode-explorer/src/mcp.rs` — 17 tools live here; the `ask` tool would be added as the 18th tool alongside new dispatch logic, new arg struct (`AskArgs { question: String }`), and new routing logic
- `crates/cognicode-explorer/src/service.rs` — `ExplorerService` holds `SymbolRepository`, `SourceReader`, `QualityRepository`, and `LensRegistry`. The router needs read access to these. The service currently has no natural-language routing capability.
- `crates/cognicode-explorer/src/dto.rs` — the `FollowUp` struct already exists on the envelope; the router would populate `suggested_follow_ups` with typed follow-up tool suggestions
- `crates/cognicode-core/src/application/services/impact_analysis.rs` — the `ImpactAnalysisService` backs 9 tools (6 impact + 3 graph). The router dispatches to it through the existing `CallGraph`.
- `docs/explorer-graph/query-and-navigation.md` — defines the curated question set and the higher-level UX verbs the router must implement
- `docs/explorer-graph/target-product-model.md` — defines the 10 core user questions the router must answer

## Tools Inventory (17 existing)

### Explorer Group (8 tools) — No graph required
| # | Tool | Signature | Returns |
|---|------|-----------|---------|
| 1 | `explorer_open_workspace` | `root_path?: string` | `WorkspaceSummary` (graph status, symbol count) |
| 2 | `explorer_spotter_search` | `query: string, kind?: string` | `SpotterResult[]` (exact + FTS5 matches) |
| 3 | `explorer_inspect_object` | `object_id: string` | `InspectableObjectSummary` (type, label, properties, available views) |
| 4 | `explorer_get_views` | `object_id: string` | `ViewDescriptor[]` (available contextual views) |
| 5 | `explorer_get_view` | `object_id: string, view_id: string` | `ContextualView` (blocks, relations, evidence, findings) |
| 6 | `explorer_get_lenses` | `object_id: string` | `LensDescriptor[]` (applicable design lenses) |
| 7 | `explorer_apply_lens` | `object_id: string, lens_id: string` | `LensResult` (findings + summary) |
| 8 | `explorer_query_moldql` | `query: string` | `MoldQLResultDto` (items matched by MoldQL grammar) |

### Impact Group (6 tools) — Graph required
| # | Tool | Signature | Returns |
|---|------|-----------|---------|
| 9 | `impact_radius` | `root: string, max_depth?: usize` | `string[]` (predecessors — who depends on root) |
| 10 | `impact_forward_radius` | `root: string, max_depth?: usize` | `string[]` (successors — what root depends on) |
| 11 | `impact_has_path` | `from: string, to: string` | `{from, to, has_path: bool}` |
| 12 | `impact_shortest_path` | `from: string, to: string` | `PathResultDto|null` (lowest-cost path) |
| 13 | `impact_detect_cycles` | `{}` (no args) | `SccDto[]` (non-trivial SCCs, size ≥ 2) |
| 14 | `impact_component` | `id: string` | `string[]|null` (connected component members) |

### Graph Group (3 tools) — Graph required
| # | Tool | Signature | Returns |
|---|------|-----------|---------|
| 15 | `graph_subgraph` | `root: string, direction?: enum, max_depth?: usize` | `{nodes, edges[]}` (neighborhood subgraph) |
| 16 | `graph_cluster` | `method?: "scc"|"connected"` | `ClusterDto[]` (communities) |
| 17 | `graph_explain` | `from: string, to: string` | `{found, hops[], total_cost, summary}` (evidence chain) |

## Curated Question Set (from target-product-model.md)

| # | Question | English patterns | Primitive chain |
|---|----------|-----------------|-----------------|
| Q1 | What does this symbol do? | "what is X", "describe X", "explain X" | `inspect_object(X)` + `get_view(X, "overview")` |
| Q2 | Who calls this? | "who calls X", "callers of X", "dependents of X" | `impact_radius(X)` + `get_view(X, "call-graph")` |
| Q3 | What does this call? | "what does X call", "callees of X", "dependencies of X" | `impact_forward_radius(X)` |
| Q4 | What connects X and Y? | "how are X and Y connected", "path from X to Y" | `impact_shortest_path(X,Y)` + `graph_explain(X,Y)` |
| Q5 | What changed recently? | "what changed", "recent changes" | NOT YET IMPLEMENTABLE — requires snapshot diff (Phase 4) |
| Q6 | What is risky to change? | "risk of changing X", "is X safe to change", "hotspots" | `get_view(X, "quality")` + `inspect_object(X)` (fan_in/churn) |
| Q7 | Where does this belong? | "where is X in the architecture", "what module is X in" | `impact_component(X)` + `inspect_object(X)` |
| Q8 | What justifies this design? | "why was X designed this way", "ADRs for X" | NOT YET IMPLEMENTABLE — requires Decision graph (Phase 3) |
| Q9 | What is the shape? | "shape of codebase", "clusters", "cycles", "structure" | `impact_detect_cycles()` + `graph_cluster()` |
| Q10 | Where to start? | "where should I start", "entry points", "overview" | `open_workspace()` + `graph_cluster()` + hotspots |

## Higher-Level UX Verbs (from query-and-navigation.md)

| Verb | Maps to | Question |
|------|---------|----------|
| `why` | `explain` + `justified_by` traversal | Q1 variant |
| `what_connects` | `path(X,Y)` + common ancestor | Q4 |
| `what_changed` | Diff snapshots | Q5 |
| `what_is_risky` | Fan-in + complexity + churn + test coverage | Q6 |
| `where_does_it_belong` | `part_of` / `in_system` climb-up | Q7 |
| `what_justifies` | `justified_by` + `cites` | Q8 |
| `what_is_the_shape` | `cluster(level=code)` + god nodes + bridges | Q9 |
| `where_to_start` | Graph trail: communities → hotspots → tests | Q10 |

## Approaches

### 1. Keyword/Pattern-Based Router (RECOMMENDED)

**Description**: A `match`-driven classifier maps natural-language questions to tool chains using keyword and regex patterns. The router parses the question, extracts entities (symbol names, file paths), selects the target tool chain, calls the primitives directly via internal dispatch, and returns a single `McpResultEnvelope`.

**Pros**:
- Simple, ~200 LOC, no external dependencies
- Fully deterministic — same question always gets the same result
- No runtime overhead (no embedding model, no LLM call)
- Easy to extend with new patterns
- Can extract entity references (symbol names) from the question text via existing `spotter_search`
- Covers 8 of 10 curated questions today; Q5 and Q8 are not implementable regardless of routing approach

**Cons**:
- Brittle to phrasing variations — "what calls X" vs "X's callers" must both be handled
- No semantic understanding — "is this function dangerous" won't match unless explicitly patterned
- Maintenance burden grows with pattern count

**Effort**: Low (~1 day)

---

### 2. Embedding-Based Router

**Description**: Pre-compute question embeddings for the 10 curated questions, embed the user's question at runtime, route to the nearest-neighbor question template.

**Pros**:
- Handles phrasing variation naturally
- Works for synonyms and paraphrases

**Cons**:
- Requires an embedding model (external dependency or onnx runtime)
- Non-deterministic — same question can route differently across runs
- Overkill for a 10-question curated set
- Adds ~50MB+ in model weights
- Still needs the same tool-chain execution logic as Approach 1

**Effort**: Medium (~3-5 days)

---

### 3. LLM-Based Router (Deferred)

**Description**: Pass the question + tool list to an LLM, ask it to select the tool chain. The LLM returns a structured JSON dispatch.

**Pros**:
- Handles truly open-ended questions
- Can generate multi-step plans

**Cons**:
- Requires an LLM endpoint (network dependency)
- Adds latency (500ms–5s)
- Non-deterministic
- Overkill for the curated question set
- Security: prompt injection risk

**Effort**: Medium-High (~3-7 days, plus infra)

---

## Recommendation

**Approach 1 (keyword/pattern-based)** for MVP, with the architecture designed to allow a future embedding or LLM router to plug in behind the same `AskRouter` trait.

The router should:
1. Accept a natural-language `question: String`
2. Lowercase and tokenize the question
3. Run keyword/regex matches against the curated question templates in priority order
4. Extract entity references by calling `spotter_search` on noun phrases from the question
5. Dispatch to the tool chain internally (direct `service.` calls, not MCP tool calls — co-located)
6. Return a single `McpResultEnvelope` with:
   - `tool_name`: `"cognicode_ask"`
   - `provenance`: router provenance (`source: "ask-router"`, `confidence: 0.85` for exact matches, lower for fuzzy) plus the primitive's provenance merged
   - `payload`: the routed tool's payload (preserving the primitive's shape)
   - `suggested_follow_ups`: typed `FollowUp` suggestions based on the result

**Routing priority order** (ordered by specificity — more specific patterns first):
1. Path/connection questions (two entities mentioned) → impact_shortest_path + graph_explain
2. Directional caller/callee questions ("who calls X", "what does X call") → impact_radius / impact_forward_radius
3. Risk/hotspot questions ("risky", "hotspot", "dangerous") → get_view("quality")
4. Shape/structure questions ("shape", "cycles", "clusters", "architecture") → detect_cycles + graph_cluster
5. Search/find questions ("find", "search", "where is") → spotter_search
6. Workspace overview ("overview", "workspace", "what's in") → open_workspace
7. What/describe questions (single entity, no directional words) → inspect_object + get_view("overview")
8. Fallback: "I don't understand. Try rephrasing as: [example questions]"

## Key Design Decisions

### Question 1: Internal dispatch or MCP tool chaining?
**Internal dispatch** — the router lives in the same process as the 17 tools. Calling through the MCP stack adds serialization overhead and a second round of envelope wrapping. The router calls `service.spotter_search()`, `service.inspect_object()`, etc. directly.

### Question 2: Single envelope or chained envelopes?
**Single envelope** — the router produces ONE `McpResultEnvelope` with merged provenance. The `provenance.source` field carries `"ask-router"`, and the `payload` is the routed primitive's payload (preserving its shape). This is simpler for consumers than a nested envelope.

### Question 3: How does provenance work?
The router wraps the primitive's result. The outer envelope has:
- `tool_name: "cognicode_ask"`
- `provenance: { confidence: <router confidence>, source: "ask-router" }`
- `payload: <primitive's full result>`

The primitive's own provenance data (from its `TypedRelation` or `EvidenceBlock` fields) is preserved in the payload. Two levels: router-level provenance (did we route correctly?) and primitive-level provenance (is the data trustworthy?).

### Question 4: Follow-up suggestions?
Populate `suggested_follow_ups` with 1-3 `FollowUp` structs based on the result:
- After `inspect_object`: suggest `get_view("call-graph")`, `get_view("quality")`
- After `impact_radius`: suggest `impact_forward_radius`, `impact_shortest_path`
- After `graph_cluster`: suggest `graph_subgraph` for each large cluster
- After `open_workspace`: suggest `graph_cluster`, `spotter_search` for "main"

### Question 5: What's NOT implementable now?
- **Q5 (what changed)**: Requires snapshot diff — no time-windowed graph yet
- **Q8 (what justifies)**: Requires Decision/ADR graph — Phase 3 work
- Router should gracefully report these as "not yet available" rather than failing

## Risks

- **Graph availability**: 9 of 17 tools require `CallGraph` loaded. The router MUST check `graph.is_some()` before dispatching to impact/graph tools and surface a clear error for graph-dependent questions when the graph is absent.
- **Entity extraction accuracy**: The router must call `spotter_search` to resolve natural-language entity names to MVP IDs. Fuzzy matching may produce wrong matches; the router should surface the top N candidates when ambiguous.
- **Pattern coverage**: The curated question set is small but the router MUST handle all 10 questions. Each missing pattern is a user-facing "I don't understand" fallback.
- **Envelope contract**: The `McpResultEnvelope<T>` is generic over `T`. The router's payload will be heterogeneous (different types for different routes). This means either: (a) the envelope uses `serde_json::Value` as the payload type, or (b) the router has its own payload DTO. Option (a) preserves the envelope contract but loses type safety. Option (b) adds a new DTO.
- **Follow-up tool availability**: The `suggested_follow_ups` field references tool names. The router must ensure all suggested tools are actually available (not just graph-dependent).

## Entropy Analysis (Connascence Landscape)

**Method**: Heuristic (CogniCode graph unavailable for structural analysis)

| Component A | Component B | Connascence Type | I(bits) | Severity |
|-------------|-------------|------------------|---------|----------|
| ask-router (new) | mcp.rs dispatch | Name | ~2.0 | ⚠️ MEDIUM | No |
| ask-router (new) | ExplorerService | Type | ~3.0 | ⚠️ MEDIUM | No |
| ask-router (new) | McpResultEnvelope<T> | Type | ~1.5 | ⚠️ LOW | No |
| ask-router (new) | 17 tool schemas | Meaning | ~2.5 | ⚠️ MEDIUM | YES |

**Critical Pairs (I > 3.0 bits)**: None — the router is an additive tool, not a modification of existing dispatch.

**Hidden Connascence (Meaning)**:
- The router implicitly knows which questions map to which primitives. This mapping constitutes **Meaning connascence** — if a primitive's behavior changes, the router's pattern expectations may go stale. Mitigation: router patterns should test against primitive output shapes, not semantic assumptions.

**Coupling Score**: Low (~2.0 bits average). The router adds coupling to `ExplorerService` (needs read access) and the dispatch surface (needs to call primitives), but does not modify existing components.

**Recommendation**: Accept. This is the minimum coupling needed to add a routing layer. The router's trait-based design (a future `AskRouter` trait) would decouple the routing strategy from the MCP handler.

## Ready for Proposal

**Yes** — the exploration is complete. The keyword/pattern-based approach is recommended. Key open design questions are resolved. Only Q5 and Q8 are blocked by missing infrastructure (snapshot diff, Decision graph) — the router should gracefully degrade for these.

The orchestrator should proceed to `sdd-propose` with the following framing:
- **Change**: `ask-router`
- **Scope**: Add a single new MCP tool (`cognicode_ask`) to `mcp.rs` with keyword-pattern routing over the 17 existing tools
- **Not in scope**: Embedding router, LLM router, Q5/Q8 implementation (those are separate changes)
- **Pattern**: Internal dispatch (not MCP chaining), single envelope return, router provenance metadata
