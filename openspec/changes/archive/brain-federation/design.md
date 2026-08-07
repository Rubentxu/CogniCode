# Design: brain-federation

## Technical Approach

Add a `federation/` submodule under `cognicode-explorer` that owns the **Space** concept, a per-space `GraphRepository` registry, a `FederatedGraphService` that multiplexes queries across N spaces, a heuristic `MergeDetector`, and the 3 new MCP tools. The existing `BrainSessionService` gains a `Vec<SpaceId>` field and an `Arc<FederatedGraphService>`; the single-graph model is preserved as a degenerate case (one space = current behavior, no wire-shape change). All new types and code paths are gated by the `multimodal` feature so the default build stays byte-for-byte unchanged.

## Architecture Decisions

### Decision: Federation service lives inside `cognicode-explorer` (not a new crate)

| Option | Tradeoff | Decision |
|--------|----------|----------|
| Service in `cognicode-explorer::federation` | Minimal crate overhead, direct access to `BrainSessionService`, ~600 LOC | **Chosen** |
| New `cognicode-federation` crate | Cleaner hexagonal boundary, reusable outside explorer | Rejected (over-engineered for current scale) |
| Federation inside `cognicode-core` | Tangles domain types with infrastructure | Rejected |

**Rationale**: Federation is a session-level concern (consumed by brain tools and the session service). It belongs in the explorer crate next to the session that uses it. The cost of a new crate (workspace dep wiring, pub-surface, CI step) does not pay off until a second consumer materializes.

### Decision: FederatedNodeId format `"{space_id}::{node_id}"`

| Option | Tradeoff | Decision |
|--------|----------|----------|
| `{space_id}::{local_id}` | Deterministic, parseable, URL-friendly via `::`→`/` rewrite, no allocation on parse | **Chosen** |
| `{space_id}/{local_id}` (slash) | Wire-friendly but collides with `path/to/file:symbol:line` IDs | Rejected |
| Tuple `(SpaceId, NodeId)` wrapper | Strongly typed but breaks `Display` for log lines | Rejected |
| New `CompositeNodeId` type replacing `NodeId` | Breaking change to `generic-graph-model` | Rejected (out of scope) |

**Rationale**: The `::` separator is reserved (rejected at `try_new`) and does NOT appear in existing `NodeId` formats (`file:name:line`, `doc:path#slug`, `issue:tracker#num`, `ev:sha256`). Parsing is a single `split_once("::")` — zero allocation on the hot path. The `NodeId` type itself is unchanged; the space prefix lives in the `FederatedNodeId` wrapper. This is the **Information Bottleneck** pattern — the federated layer absorbs the space-scoping concern and exposes a uniform interface to consumers.

### Decision: Default space is implicit, not listed

`brain_open` without `spaces` and `brain_open` with `spaces: []` both produce a session with `space_count == 0` and an empty `spaces` list. The default space (`SpaceId("default")`) is **implicit** — it exists in the PG `spaces` table (seeded by the migration) but is NOT registered into the session's space list. This keeps the wire shape clean: a session with no explicit spaces behaves byte-for-byte as the pre-federation implementation.

**Rationale**: Backward compatibility is a hard requirement (success criterion). Listing the default space would change the wire shape of every `brain_status` response and break the 18 existing one-shot tools' contract. Implicit default is the only path that satisfies the spec.

### Decision: Merge candidates are "suggest, never merge"

The `MergeDetector` returns `Vec<MergeCandidate>` with a `confidence` field. The `brain_spaces` tool surfaces these as suggestions. There is no `brain_merge` tool, no auto-merge, and no write-back to the graph. Downstream tools (or humans) consume the suggestions and decide.

**Rationale**: Merge false positives are a known risk (e.g. `User` in auth vs `User` in billing). Auto-merge would silently corrupt the graph. The spec pins this as a "suggest, don't merge" UX.

### Decision: Detection runs on `brain_spaces` only (not `brain_ask`)

The N² brute-force detector is too expensive for the hot ask path. `brain_spaces` is the dedicated introspection tool; the cost is bounded by N=5000 nodes per call. `brain_ask` and `brain_status` skip detection entirely.

**Rationale**: Decouples the hot ask path (sub-100ms target) from the introspection path. `brain_spaces` is expected to be called rarely (humans or tools explicitly exploring federation state).

## Data Flow

```
Agent                mcp.rs              brain_add_space arm    BrainSessionService       FederatedGraphService       per-space GraphRepositories
  │                     │                       │                       │                              │                              │
  ├─ tools/call ──────→ ├─ dispatch arm ──────→ ├─ parse args          │                              │                              │
  │   brain_add_space   │                       ├─ validate kind/id   │                              │                              │
  │                     │                       ├─ service.add_space() │                              │                              │
  │                     │                       │                       ├─ spaces.push(SpaceId)        │                              │
  │                     │                       │                       ├─ fed_graph.add_space() ─────→ │                              │
  │                     │                       │                       │                              ├─ repos.insert(id, repo)     │
  │                     │←─ envelope_ok ─────────┤                       │                              │                              │
  │                     │                       │                       │                              │                              │

  │                     │                       │                       │                              │                              │
  ├─ tools/call ──────→ ├─ brain_ask arm ─────→ ├─ session.get(S)      │                              │                              │
  │   brain_ask         │                       ├─ service.ask_with()  │                              │                              │
  │                     │                       │                       ├─ prepend focus_node          │                              │
  │                     │                       │                       ├─ fed_graph.search() ────────→ ├─ join_all(repos.search) ───→ │
  │                     │                       │                       │                              │                              │
  │                     │                       │                       │←─ merged FederatedSearchPage ─┤←─ per-space pages ──────────┤
  │                     │                       │                       │                              │                              │
  │                     │                       │                       ├─ ask router dispatch (same path as before)
  │                     │                       │                       ├─ history.push (success only)  │                              │
  │                     │←─ envelope_ok ─────────┤                       │                              │                              │
  │                     │                       │                       │                              │                              │

  │                     │                       │                       │                              │                              │
  ├─ tools/call ──────→ ├─ brain_spaces arm ──→ ├─ service.spaces()    │                              │                              │
  │   brain_spaces      │                       ├─ merge_detector.run() │                              │                              │
  │                     │                       │                       │                              │                              │
  │                     │                       │                       │                              ├─ collect all FederatedNodes  │
  │                     │                       │                       │                              ├─ detect(nodes)               │
  │                     │                       │                       │←─ Vec<MergeCandidate> ───────┤                              │
  │                     │←─ envelope_ok ─────────┤                       │                              │                              │
```

## File Changes

| File | Action | Lines (est.) | Description |
|------|--------|-------------|-------------|
| `crates/cognicode-core/src/domain/value_objects/space.rs` | Create | ~80 | `Space`, `SpaceKind`, `SpaceError` |
| `crates/cognicode-core/src/domain/value_objects/space_id.rs` | Create | ~50 | `SpaceId` newtype + `try_new` |
| `crates/cognicode-explorer/src/federation/mod.rs` | Create | ~15 | `pub mod space_registry; pub mod federated_node; pub mod federated_node_id; pub mod federated_graph_service; pub mod merge_candidate; pub mod merge_detector;` |
| `crates/cognicode-explorer/src/federation/space_registry.rs` | Create | ~120 | `SpaceRegistry` CRUD with `try_register`, `get`, `list`, `unregister` |
| `crates/cognicode-explorer/src/federation/federated_node.rs` | Create | ~80 | `FederatedNode { node, space_id }` + `federated_id()` + `Display` |
| `crates/cognicode-explorer/src/federation/federated_node_id.rs` | Create | ~120 | `FederatedNodeId` newtype, `try_new`, `space_id()`, `local_id()`, `Display` |
| `crates/cognicode-explorer/src/federation/federated_graph_service.rs` | Create | ~280 | `FederatedGraphService::new/add_space/spaces/federated_search/get_node/find_outgoing_edges/detect_merge_candidates` |
| `crates/cognicode-explorer/src/federation/merge_candidate.rs` | Create | ~70 | `MergeCandidate`, `MergeReason` |
| `crates/cognicode-explorer/src/federation/merge_detector.rs` | Create | ~150 | `MergeDetector::detect(&[FederatedNode]) -> Vec<MergeCandidate>` + label normalization + scoring |
| `crates/cognicode-explorer/src/session/state.rs` | Modify | +~15 | Add `spaces: Vec<SpaceId>` field + serde default |
| `crates/cognicode-explorer/src/session/service.rs` | Modify | +~80 | Hold `Arc<FederatedGraphService>`; add `add_space/remove_space/spaces/federated_graph`; route `ask_with_session` through federation when multimodal |
| `crates/cognicode-explorer/src/session/registry.rs` | Modify | +~30 | `open` accepts optional `Vec<SpaceSpec>`; `BrainSessionService::new` wires the federation service |
| `crates/cognicode-explorer/src/mcp.rs` | Modify | +~360 | 3 new constants (TOOL_BRAIN_ADD_SPACE, _REMOVE_SPACE, _SPACES), 3 arg structs, 3 dispatch arms, 3 tool schemas, `brain_open` extension for `spaces`, `brain_status` extension for `space_count` + `spaces`; TOOL_NAMES 24→27 |
| `crates/cognicode-explorer/src/lib.rs` | Modify | +1 | `#[cfg(feature = "multimodal")] pub mod federation;` |
| `schema_postgres.sql` | Modify | +~15 | `spaces` table + `m00xx_spaces` migration; nullable `space_id TEXT` column on `graph_nodes` (default `'default'`) |

**Total**: ~9 new files (~965 LOC), 5 modified files (~501 LOC added). Net ~1466 LOC.

## Interfaces / Contracts

### `SpaceId` (cognicode-core)

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SpaceId(pub String);

impl SpaceId {
    pub fn try_new(s: impl Into<String>) -> Result<Self, SpaceError>;
    pub fn default() -> Self;  // SpaceId("default".into())
}

impl Display for SpaceId { /* prints inner string */ }
```

### `Space` (cognicode-core)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Space {
    pub id: SpaceId,
    pub name: String,
    pub kind: SpaceKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_path: Option<PathBuf>,
    #[serde(default = "default_config")]
    pub config: serde_json::Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SpaceKind { Repo, Docs, Issues }

impl Space {
    pub fn try_new(id: SpaceId, name: String, kind: SpaceKind) -> Result<Self, SpaceError>;
}
```

### `FederatedNodeId` (cognicode-explorer)

```rust
pub struct FederatedNodeId(pub String);

impl FederatedNodeId {
    pub fn try_new(s: impl Into<String>) -> Result<Self, FederatedNodeIdError>;
    pub fn space_id(&self) -> Option<SpaceId>;     // parses left of "::"
    pub fn local_id(&self) -> Option<&str>;        // parses right of "::"
}

impl Display for FederatedNodeId { /* prints inner string */ }
```

### `FederatedNode` (cognicode-explorer)

```rust
pub struct FederatedNode {
    pub node: GraphNode,         // local id, no space prefix
    pub space_id: SpaceId,
}

impl FederatedNode {
    pub fn federated_id(&self) -> FederatedNodeId; // "{space_id}::{node.id}"
}
```

### `FederatedGraphService` (cognicode-explorer)

```rust
pub struct FederatedGraphService {
    spaces: HashMap<SpaceId, Arc<dyn GraphRepository>>,
    order: Vec<SpaceId>,         // insertion order for stable iteration
}

impl FederatedGraphService {
    pub fn new() -> Self;
    pub fn add_space(&mut self, id: SpaceId, repo: Arc<dyn GraphRepository>);
    pub fn spaces(&self) -> Vec<SpaceId>;
    pub async fn federated_search(
        &self, query: &str, node_kinds: &[NodeKind],
        limit: usize, cursor: Option<&str>,
    ) -> ExplorerResult<FederatedSearchPage>;
    pub async fn get_node(&self, id: FederatedNodeId) -> ExplorerResult<Option<FederatedNode>>;
    pub async fn find_outgoing_edges(&self, id: FederatedNodeId) -> ExplorerResult<Vec<GraphEdge>>;
    pub fn detect_merge_candidates(&self) -> Vec<MergeCandidate>;
}
```

### `MergeCandidate` (cognicode-explorer)

```rust
pub struct MergeCandidate {
    pub left: FederatedNode,
    pub right: FederatedNode,
    pub confidence: f64,                  // 0.0..=1.0
    pub reasons: Vec<MergeReason>,
}

#[non_exhaustive]
pub enum MergeReason { LabelMatch, KindMatch, PropertyOverlap }
```

### `BrainSessionState` (modified)

```rust
pub struct BrainSessionState {
    // ... existing fields ...
    pub spaces: Vec<SpaceId>,             // NEW — default empty, registration order
}
```

### `BrainSessionService` (modified)

```rust
pub struct BrainSessionService {
    state: Mutex<BrainSessionState>,
    service: Arc<ExplorerService>,
    graph: Option<Arc<CallGraph>>,
    federated: Arc<FederatedGraphService>,  // NEW
}

impl BrainSessionService {
    // ... existing methods ...
    pub fn add_space(&self, space: Space) -> Result<(), SpaceError>;        // NEW
    pub fn remove_space(&self, id: SpaceId) -> bool;                        // NEW
    pub fn spaces(&self) -> Vec<Space>;                                      // NEW
    pub fn federated_graph(&self) -> Arc<FederatedGraphService>;             // NEW
}
```

### PG Schema (multimodal migration)

```sql
-- m00xx_spaces.sql
CREATE TABLE spaces (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  kind TEXT NOT NULL CHECK (kind IN ('Repo','Docs','Issues')),
  source_path TEXT,
  config JSONB NOT NULL DEFAULT '{}'::jsonb,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_spaces_kind ON spaces(kind);

-- Seed the default space (implied by every pre-federation node).
INSERT INTO spaces (id, name, kind) VALUES ('default', 'default', 'Repo')
  ON CONFLICT (id) DO NOTHING;

-- Additive column on the existing graph_nodes table.
ALTER TABLE graph_nodes ADD COLUMN space_id TEXT NOT NULL DEFAULT 'default';
CREATE INDEX idx_graph_nodes_space_id ON graph_nodes(space_id);
```

The migration is **additive**: no `ALTER` on `symbols`/`call_edges` and the new column has a `DEFAULT 'default'`, so existing rows are backfilled automatically.

## Entropy Budget (Protocol B)

| Metric | Estimate (bits) | Threshold | Status |
|--------|-----------------|-----------|--------|
| H(Δ_existing) | ~2.7 | < 3.0 | ✅ Acceptable |
| H(Δ_new) | ~4.5 | > 0 | ✅ |
| New connascence pairs | 4 | < 5 | ✅ |
| OCP compliant? | Yes | yes | ✅ |
| Federation IB (Protocol C) | clean | — | ✅ See below |

**Connascence delta**: `SpaceId ↔ SpaceKind` (Type, 1.5 bits), `FederatedNodeId ↔ {space, local}` (Meaning, 2.0 bits), `BrainSessionState.spaces ↔ BrainSessionService.federated` (Meaning, 1.8 bits), `PG graph_nodes.space_id ↔ SpaceId` (Meaning, 1.2 bits). Total: 6.5 bits across 4 new pairs — well within the 5-pair soft limit when the architectural value is factored in.

**OCP verdict**: ✅. The federation layer is open for extension (new `SpaceKind` variants, new scoring components in `MergeReason`) and closed for modification (the 18 one-shot tools and the 6 existing brain tools' wire shape are unchanged).

## Information Bottleneck Check (Protocol C)

The `FederatedGraphService` interface is the bottleneck. Evaluation:

| Criterion | Score | Evidence |
|-----------|-------|----------|
| **Minimum necessary distinctness** | ✅ | The interface exposes 5 async methods + 1 sync. Each consumer (brain_ask, brain_focus, brain_spaces) only sees the federated API — the per-space repos are private. |
| **Composability** | ✅ | A new consumer (e.g. `cognicode_ask` with multi-space query) can use the federation service without knowing the space count. |
| **Information hiding** | ✅ | `HashMap<SpaceId, Arc<dyn GraphRepository>>` is private. Consumers cannot reach inside to query a single space directly; they MUST go through the federated API. |
| **Interface stability** | ✅ | Adding a new method is a non-breaking change (consumers ignore it). Removing a method is a breaking change (gated by the multimodal feature anyway). |
| **Bottleneck identification** | ✅ | `FederatedGraphService` is the only place that knows about N spaces. If a new "smart router" is needed (e.g. cost-based fan-out), it lives HERE, not in every consumer. |

**Verdict**: ✅. The interface is the right bottleneck. No consumer leaks the per-space implementation detail.

## Testing Strategy

| Layer | What | Approach |
|-------|------|----------|
| Unit | `SpaceId::try_new` | Plain `#[test]` for empty + reserved |
| Unit | `Space::try_new` | Plain `#[test]` for empty name + config default |
| Unit | `FederatedNodeId::try_new` | Plain `#[test]` for valid/missing-separator/empty-segment |
| Unit | `FederatedNode::federated_id` | Plain `#[test]` for roundtrip |
| Unit | `FederatedGraphService::add_space` idempotency | Plain `#[test]` |
| Unit | `FederatedGraphService::federated_search` | Mock-backed `#[tokio::test]` with 2 mock repos |
| Unit | `MergeDetector` scoring | Plain `#[test]` for the 4 confidence levels (0.5, 0.7, 0.8, 1.0) |
| Unit | `BrainSessionState::spaces` serde | Plain `#[test]` for `[]` roundtrip |
| Unit | `BrainSessionService::add_space/remove_space` | Plain `#[test]` |
| Integration | `brain_open` with `spaces[]` | End-to-end through `dispatch()` |
| Integration | `brain_add_space` / `brain_remove_space` / `brain_spaces` | End-to-end through `dispatch()` |
| Integration | Backward compat: `brain_open({})` and `brain_ask` produce pre-federation wire shape | Regression guard |
| Integration | `TOOL_NAMES.len() == 27` | Regression guard |
| PG | `spaces` table migration + default row seed | sqlx test against a real DB |
| PG | `graph_nodes.space_id` defaults to `'default'` for existing rows | sqlx migration test |

## Migration / Rollback

| Phase | Action |
|-------|--------|
| **Apply** | Run `m00xx_spaces` migration (additive). `graph_nodes.space_id` backfills to `'default'`. New `spaces` row seeded. Code wires the federation layer. |
| **Rollback** | `DROP TABLE spaces; ALTER TABLE graph_nodes DROP COLUMN space_id;` (column is nullable-by-default; drop is non-destructive). Revert code (5 modified files + 9 new files removed). No data loss. |

## Open Questions

None — all design decisions resolved by the spec and the proposal.
