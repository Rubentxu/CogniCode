# generic-graph-model Specification (NEW)

## Purpose

Introduces a generic graph model layered on top of the existing code-only `Symbol`/`CallGraph`. The new types — `NodeKind`, `NodeId`, `EdgeKind`, `GraphNode`, `GraphEdge` — subsume all 22 `SymbolKind` variants and 8 `DependencyType` variants, plus new multimodal node/edge kinds. New PG tables (`graph_nodes`, `graph_edges`) coexist additively with the existing `symbols` and `call_edges`. The model is feature-gated behind the `multimodal` Cargo feature and does not mutate any existing code-graph behavior.

## Domain Types

| Type | File | Definition |
|------|------|------------|
| `NodeKind` | `crates/cognicode-core/src/domain/value_objects/node_kind.rs` | `Symbol(SymbolKind) \| Decision \| Doc \| Issue \| Evidence` |
| `NodeId` | `crates/cognicode-core/src/domain/value_objects/node_id.rs` | Newtype `(String, NodeKind)` — typed discrimination |
| `EdgeKind` | `crates/cognicode-core/src/domain/value_objects/edge_kind.rs` | `Dependency(DependencyType) \| Cites \| Justifies \| Resolves \| CorroboratedBy` |
| `GraphNode` | `crates/cognicode-core/src/domain/aggregates/graph_node.rs` | `id: NodeId, kind: NodeKind, label, source_path, metadata: serde_json::Value` |
| `GraphEdge` | `crates/cognicode-core/src/domain/aggregates/graph_edge.rs` | `source: NodeId, target: NodeId, kind: EdgeKind, provenance: Provenance, confidence: f64` |

## Requirements

### Requirement: NodeKind Exhaustive Variants

`NodeKind::as_str()` MUST return the kebab-case discriminator for unit variants AND MUST return `"symbol.{inner}"` for `Symbol(SymbolKind)` where `{inner}` is `SymbolKind::as_str()`. `Display` delegates to `as_str`; `FromStr` is the inverse. Parsing the legacy bare `"symbol"` MUST yield `Err(NodeKindParseError::Unknown)`. All variants derive `Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize`.
(Previously: `as_str()` returned `"symbol"` for every sub-kind and `FromStr` only accepted `"symbol"`.)

#### Scenario: Symbol sub-kinds produce distinct strings
- GIVEN `NodeKind::Symbol(SymbolKind::Function)` and `NodeKind::Symbol(SymbolKind::Class)`
- WHEN formatted via `Display`
- THEN the strings are `"symbol.function"` and `"symbol.class"`
- AND each round-trips via `FromStr`

#### Scenario: Unit variants and legacy rejection
- GIVEN `Decision`, `Doc`, `Evidence` AND the bare string `"symbol"`
- WHEN each is formatted / parsed
- THEN unit variants round-trip
- AND `from_str("symbol")` returns `Err(NodeKindParseError::Unknown)`

#### Scenario: All 5 variants construct and roundtrip
- GIVEN `NodeKind::Symbol(SymbolKind::Function)`, `NodeKind::Decision`, `NodeKind::Doc`, `NodeKind::Issue`, `NodeKind::Evidence`
- WHEN each is serialized to JSON and deserialized back
- THEN each MUST roundtrip without loss
- AND `kind()` discrimination MUST recover the original variant

#### Scenario: Existing SymbolKind variants still match exhaustively
- GIVEN a `match` arm over `NodeKind::Symbol(sk)` covering all 22 `SymbolKind` variants
- WHEN the compiler analyzes the function
- THEN no `_ =>` wildcard arm is reachable (exhaustive)

### Requirement: NodeId Carries Kind Tag

`NodeId` MUST be a tuple `(String, NodeKind)` newtype. Constructors MUST enforce that the string ID format is well-formed for the kind: `Symbol` IDs use the existing `file:name:line` format; `Doc`/`Decision` IDs use `doc:<path>#<slug>`; `Issue` IDs use `issue:<tracker>#<number>`; `Evidence` IDs use `ev:<sha256>`. Mismatched formats MUST produce a `NodeIdError`.

#### Scenario: Symbol NodeId uses file:name:line
- GIVEN `NodeId::symbol("src/main.rs:main:1")`
- WHEN constructed
- THEN `id.0 == "src/main.rs:main:1"` AND `id.1 == NodeKind::Symbol(_)` (any symbol sub-kind)

#### Scenario: Doc NodeId uses doc:<path>#<slug>
- GIVEN `NodeId::doc("docs/adr/0001.md", "context")`
- WHEN constructed
- THEN the id string MUST equal `"doc:docs/adr/0001.md#context"`

#### Scenario: Malformed ID rejected
- GIVEN `NodeId::try_new("free-text", NodeKind::Decision)`
- WHEN evaluated
- THEN it MUST return `Err(NodeIdError::MalformedFormat)`

### Requirement: EdgeKind 5 Variants

The `EdgeKind` enum MUST have exactly 5 variants: `Dependency(DependencyType)`, `Cites`, `Justifies`, `Resolves`, `CorroboratedBy`. `Dependency` wraps the existing 8-variant `DependencyType`. The 4 new variants are unit. All MUST derive `Debug, Clone, PartialEq, Eq, Hash`.

#### Scenario: EdgeKind roundtrips all 5 variants
- GIVEN every variant of `EdgeKind` constructed
- WHEN serialized to JSON
- THEN each MUST roundtrip without loss
- AND `kind == EdgeKind::Dependency(DependencyType::Calls)` for the wrapped code path

### Requirement: GraphEdge Replaces EdgeMetadata for Multimodal Paths

`GraphEdge` MUST expose `source: NodeId`, `target: NodeId`, `kind: EdgeKind`, `provenance: Provenance`, `confidence: f64` (0.0..=1.0). It MUST expose `metadata: serde_json::Value` (typed JSONB) with default `Value::Null`. PG persistence MUST round-trip JSONB bit-exact (no string flattening). A typed adapter (`to_map` / `from_map`) MUST remain for the code-graph path. It MUST NOT have `caller_id`/`callee_id` fields. The existing `EdgeMetadata` struct (caller/callee) MUST remain for the code-graph path.
(Previously: `metadata: HashMap<String, String>` — nested JSON was flattened on PG round-trip.)

#### Scenario: Structured metadata round-trips via PG JSONB
- GIVEN a `GraphEdge` whose `metadata` is `json!({"call_site": {"file": "x.rs", "line": 12}, "tags": ["auth"]})`
- WHEN persisted to PG and loaded back
- THEN the loaded `metadata` equals the original bit-for-bit

#### Scenario: Confidence boundaries
- GIVEN `GraphEdge::new(…)` with confidence values `1.5`, `-0.1`, `f64::NAN`
- WHEN each is evaluated
- THEN `1.5` and `-0.1` return `Err(ConfidenceOutOfRange)`
- AND `f64::NAN` returns `Err(ConfidenceNotFinite)`

#### Scenario: GraphEdge with multimodal kinds
- GIVEN `GraphEdge { source: NodeId::doc("docs/adr.md", "ctx"), target: NodeId::symbol("src/x.rs:f:10"), kind: EdgeKind::Justifies, provenance: Provenance::Extracted, confidence: 0.9 }`
- WHEN constructed
- THEN all fields are accessible and `kind == EdgeKind::Justifies`

#### Scenario: confidence out of range produces error
- GIVEN `GraphEdge::new(source, target, kind, prov, 1.5)`
- WHEN evaluated
- THEN it MUST return `Err(GraphEdgeError::ConfidenceOutOfRange)`

### Requirement: GraphNode exposes typed JSONB properties

`GraphNode` MUST expose `properties: serde_json::Value` (typed JSONB) with default `Value::Object(Default::default())`. PG persistence MUST round-trip JSONB bit-exact. A typed adapter MUST remain for the code-graph path.
(Previously: `properties: HashMap<String, String>`.)

#### Scenario: Structured properties round-trip via PG JSONB
- GIVEN a `GraphNode` whose `properties` is `json!({"complexity": 12, "tags": ["auth"], "nested": {"k": "v"}})`
- WHEN persisted to PG and loaded back
- THEN the loaded `properties` equals the original bit-for-bit

### Requirement: graph_nodes and graph_edges PG Tables

The PK of `graph_nodes` is `(workspace_id, id, kind)`. The unique identity of
`graph_edges` is `(workspace_id, source_id, source_kind, target_id,
target_kind, kind)`. The migration MUST be additive: existing rows are
backfilled with `workspace_id = 'default'` and the old PK is replaced under a
guarded step that fails closed if cross-workspace duplicates exist.
(Previously: `graph_nodes.id` was a global PK and edge uniqueness ignored `workspace_id`.)

```sql
CREATE TABLE graph_nodes (
  workspace_id TEXT NOT NULL,
  id TEXT NOT NULL,
  kind TEXT NOT NULL,
  label TEXT NOT NULL,
  source_path TEXT,
  properties JSONB,
  PRIMARY KEY (workspace_id, id, kind)
);
CREATE INDEX idx_graph_nodes_kind ON graph_nodes(kind);

CREATE TABLE graph_edges (
  workspace_id TEXT NOT NULL,
  source_id TEXT NOT NULL,
  source_kind TEXT NOT NULL,
  target_id TEXT NOT NULL,
  target_kind TEXT NOT NULL,
  kind TEXT NOT NULL,
  provenance TEXT NOT NULL,
  confidence DOUBLE PRECISION NOT NULL,
  metadata JSONB,
  PRIMARY KEY (workspace_id, source_id, source_kind, target_id, target_kind, kind)
);
CREATE INDEX idx_graph_edges_source ON graph_edges(workspace_id, source_id, source_kind);
CREATE INDEX idx_graph_edges_target ON graph_edges(workspace_id, target_id, target_kind);
```

#### Scenario: PK and uniqueness include workspace_id
- GIVEN an empty database
- WHEN migration `m0018_workspace_scoped_identity` runs
- THEN `graph_nodes` PK is `(workspace_id, id, kind)`
- AND `graph_edges` identity is `(workspace_id, source_id, source_kind, target_id, target_kind, kind)`

#### Scenario: Homonymous nodes across workspaces do not collide
- GIVEN row `(ws1, "src/x.rs:foo:1", "symbol.function", …)`
- WHEN an insert with `(ws2, "src/x.rs:foo:1", "symbol.function", …)` runs
- THEN the insert succeeds
- AND `count(*) where id = 'src/x.rs:foo:1'` is `2`

#### Scenario: New tables are created
- GIVEN an empty database
- WHEN migration `m0009_graph_nodes_edges` runs
- THEN `graph_nodes` and `graph_edges` exist with the columns above
- AND `symbols` and `call_edges` are unchanged (regression gate)

#### Scenario: Existing data unaffected
- GIVEN 1000 rows in `symbols` and 5000 rows in `call_edges`
- WHEN the migration runs
- THEN row counts are identical before and after
- AND no `ALTER TABLE symbols` or `ALTER TABLE call_edges` appears in the migration log

### Requirement: GenericGraphRepository Port

Each write/read method MUST additionally scope by `workspace_id`. The Postgres implementation MUST live in `crates/cognicode-core/src/infrastructure/persistence/generic_graph_repository.rs`. The existing `Repository` and `SymbolRepository` traits are untouched.

#### Scenario: Workspace-scoped upsert and incoming edges
- GIVEN empty workspaces `ws1` and `ws2`
- WHEN a `GraphNode` is upserted under `ws1` AND 3 edges point to the same `Doc` target in `ws1` plus 1 in `ws2`
- THEN `find_nodes_by_kind(Function, ws1)` returns the upserted node
- AND `find_nodes_by_kind(Function, ws2)` returns an empty Vec
- AND `find_incoming_edges(target, ws1)` returns exactly 3 edges

#### Scenario: Roundtrip a node and edge
- GIVEN a fresh `GenericGraphRepository` against an empty DB
- WHEN a `GraphNode` is inserted and then `find_nodes_by_kind(NodeKind::Decision)` is called
- THEN the node is returned with all fields intact
- AND a subsequent `GraphEdge` from that node is returned by `find_edges`

#### Scenario: Find incoming edges
- GIVEN 3 edges with different sources pointing to a `Doc` target
- WHEN `find_incoming_edges(target)` is called
- THEN all 3 edges MUST be returned

### Requirement: WorkspaceId value object

`WorkspaceId` in `crates/cognicode-core/src/domain/value_objects/workspace_id.rs` is a non-empty newtype `WorkspaceId(pub String)`. `WorkspaceId::default()` returns `WorkspaceId("default")`.

#### Scenario: Default and empty rejection
- GIVEN `WorkspaceId::default()` and `WorkspaceId::try_new("")`
- WHEN each is evaluated
- THEN `default().as_str() == "default"`
- AND `try_new("")` returns `Err(WorkspaceIdError::Empty)`

### Requirement: multimodal Feature Gate

All new types, traits, and modules MUST be gated by `#[cfg(feature = "multimodal")]`. The PG migration MUST be registered only when the feature is enabled. With the feature off, the codebase MUST compile unchanged and the new types MUST NOT be reachable.

#### Scenario: Build with feature disabled
- GIVEN `cargo build -p cognicode-core --no-default-features`
- WHEN the build completes
- THEN no `node_kind.rs`, `edge_kind.rs`, `graph_node.rs`, `graph_edge.rs`, or `generic_graph_repository.rs` symbols are exported
- AND no PG migration registers `graph_nodes`/`graph_edges`

#### Scenario: Build with feature enabled
- GIVEN `cargo build -p cognicode-core --features multimodal`
- WHEN the build completes
- THEN all 5 new types and the new repository compile
- AND `cargo test -p cognicode-core --features multimodal` runs the multimodal test suite

## Edge Cases

| Edge Case | Expected Behavior |
|-----------|-------------------|
| SymbolKind match arm missing a variant in legacy code | Compiler error at the `SymbolKind` site (not the `NodeKind` site). The wrapper preserves exhaustivity. |
| `NodeId` string is empty | `NodeIdError::Empty` — reject before reaching DB |
| `confidence` is `f64::NAN` | Reject with `GraphEdgeError::ConfidenceNotFinite` |
| `graph_nodes` insert with duplicate `(workspace_id, id, kind)` | `ON CONFLICT (workspace_id, id, kind) DO UPDATE` — workspace-local upsert semantics, no exception |
| Two `NodeKind::Decision` nodes share the same label | Both stored; the `(workspace_id, id, kind)` PK disambiguates |
| PG connection drops mid-batch insert | Transaction rolls back; partial inserts MUST NOT persist |
| A graph node references a `source_path` file that no longer exists | Repository returns the node anyway; downstream rendering may show a broken-link indicator |

## Out of Scope

- Property graph queries (Cypher/GQL syntax) — only the typed 5-method repository
- Time-travel / versioned graphs
- Cross-repo federation
- Migration of existing `symbols`/`call_edges` rows into `graph_nodes`/`graph_edges`
- Changing `CallGraph` or `EdgeMetadata` (the code-graph path stays byte-for-byte unchanged)

## TDD RED Gate

Before any implementation, the following tests MUST exist in `#[cfg(test)] mod tests` blocks under each new file and MUST be RED:

1. `node_kind` — 5-variant enum coverage test (5 constructions + JSON roundtrip)
2. `node_id` — 4 well-formed ID patterns + 3 malformed rejections (≥ 7 tests)
3. `edge_kind` — 5-variant coverage + JSON roundtrip
4. `graph_edge` — confidence 0.0, 1.0, in-range, out-of-range, NaN (≥ 5 tests)
5. `graph_node` — construction + JSON metadata roundtrip
6. PG integration test (`generic_graph_repository`): insert → find by kind → find edges → find incoming edges
7. Feature-gate test: `cargo build --no-default-features` succeeds without the new symbols

RED gate fails if any test passes before its module compiles, or if `cargo build --no-default-features` regresses.

## Dependencies

- Existing `SymbolKind` (22 variants) — preserved as `NodeKind::Symbol(SymbolKind)` payload
- Existing `DependencyType` (8 variants) — preserved as `EdgeKind::Dependency(DependencyType)` payload
- Existing `Provenance` enum — source-agnostic, no new variants needed
- Existing `SymbolId` — kept for the code-graph path; NOT used in `NodeId`
