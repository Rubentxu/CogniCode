# Delta for generic-graph-model

> Workspace-scoped identity, typed JSONB properties/metadata, inverse
> Display/FromStr for symbol sub-kinds.

## MODIFIED Requirements

### Requirement: NodeKind Exhaustive Variants

`NodeKind::as_str()` MUST return the kebab-case discriminator for unit
variants AND MUST return `"symbol.{inner}"` for `Symbol(SymbolKind)`
where `{inner}` is `SymbolKind::as_str()`. `Display` delegates to
`as_str`; `FromStr` is the inverse. Parsing the legacy bare `"symbol"`
MUST yield `Err(NodeKindParseError::Unknown)`. All variants derive
`Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize`.
(Previously: `as_str()` returned `"symbol"` for every sub-kind and
`FromStr` only accepted `"symbol"`.)

#### Scenario: Symbol sub-kinds produce distinct strings

- GIVEN `NodeKind::Symbol(SymbolKind::Function)` and
  `NodeKind::Symbol(SymbolKind::Class)`
- WHEN formatted via `Display`
- THEN the strings are `"symbol.function"` and `"symbol.class"`
- AND each round-trips via `FromStr`

#### Scenario: Unit variants and legacy rejection

- GIVEN `Decision`, `Doc`, `Evidence` AND the bare string `"symbol"`
- WHEN each is formatted / parsed
- THEN unit variants round-trip
- AND `from_str("symbol")` returns `Err(NodeKindParseError::Unknown)`

### Requirement: GraphEdge Replaces EdgeMetadata for Multimodal Paths

`GraphEdge` MUST expose `metadata: serde_json::Value` (typed JSONB)
with default `Value::Null`. PG persistence MUST round-trip JSONB
bit-exact (no string flattening). A typed adapter (`to_map` / `from_map`)
MUST remain for the code-graph path.
(Previously: `metadata: HashMap<String, String>` — nested JSON was
flattened on PG round-trip.)

#### Scenario: Structured metadata round-trips via PG JSONB

- GIVEN a `GraphEdge` whose `metadata` is
  `json!({"call_site": {"file": "x.rs", "line": 12}, "tags": ["auth"]})`
- WHEN persisted to PG and loaded back
- THEN the loaded `metadata` equals the original bit-for-bit

#### Scenario: Confidence boundaries

- GIVEN `GraphEdge::new(…)` with confidence values `1.5`, `-0.1`, `f64::NAN`
- WHEN each is evaluated
- THEN `1.5` and `-0.1` return `Err(ConfidenceOutOfRange)`
- AND `f64::NAN` returns `Err(ConfidenceNotFinite)`

### Requirement: GraphNode exposes typed JSONB properties

`GraphNode` MUST expose `properties: serde_json::Value` (typed JSONB)
with default `Value::Object(Default::default())`. PG persistence MUST
round-trip JSONB bit-exact. A typed adapter MUST remain for the
code-graph path. (Previously: `properties: HashMap<String, String>`.)

#### Scenario: Structured properties round-trip via PG JSONB

- GIVEN a `GraphNode` whose `properties` is
  `json!({"complexity": 12, "tags": ["auth"], "nested": {"k": "v"}})`
- WHEN persisted to PG and loaded back
- THEN the loaded `properties` equals the original bit-for-bit

### Requirement: graph_nodes and graph_edges PG Tables

The PK of `graph_nodes` is `(workspace_id, id, kind)`. The unique index
of `graph_edges` is `(workspace_id, source_id, target_id, kind)`. The
migration MUST be additive: existing rows are backfilled with
`workspace_id = 'default'` and the old PK is replaced under a guarded
step that fails closed if cross-workspace duplicates exist.
(Previously: `graph_nodes.id` was a global PK and edge uniqueness
ignored `workspace_id`.)

#### Scenario: PK and uniqueness include workspace_id

- GIVEN an empty database
- WHEN migration `m0018_workspace_scoped_identity` runs
- THEN `graph_nodes` PK is `(workspace_id, id, kind)`
- AND `graph_edges` unique index is `(workspace_id, source_id, target_id, kind)`

#### Scenario: Homonymous nodes across workspaces do not collide

- GIVEN row `(ws1, "src/x.rs:foo:1", "symbol.function", …)`
- WHEN an insert with `(ws2, "src/x.rs:foo:1", "symbol.function", …)` runs
- THEN the insert succeeds
- AND `count(*) where id = 'src/x.rs:foo:1'` is `2`

### Requirement: GenericGraphRepository Port

Each write/read method MUST additionally scope by `workspace_id`. The
Postgres implementation MUST live in
`crates/cognicode-core/src/infrastructure/persistence/generic_graph_repository.rs`.
The existing `Repository` and `SymbolRepository` traits are untouched.

#### Scenario: Workspace-scoped upsert and incoming edges

- GIVEN empty workspaces `ws1` and `ws2`
- WHEN a `GraphNode` is upserted under `ws1` AND 3 edges point to the
  same `Doc` target in `ws1` plus 1 in `ws2`
- THEN `find_nodes_by_kind(Function, ws1)` returns the upserted node
- AND `find_nodes_by_kind(Function, ws2)` returns an empty Vec
- AND `find_incoming_edges(target, ws1)` returns exactly 3 edges

## ADDED Requirements

### Requirement: WorkspaceId value object

`WorkspaceId` in
`crates/cognicode-core/src/domain/value_objects/workspace_id.rs` is a
non-empty newtype `WorkspaceId(pub String)`.
`WorkspaceId::default()` returns `WorkspaceId("default")`.

#### Scenario: Default and empty rejection

- GIVEN `WorkspaceId::default()` and `WorkspaceId::try_new("")`
- WHEN each is evaluated
- THEN `default().as_str() == "default"`
- AND `try_new("")` returns `Err(WorkspaceIdError::Empty)`

## REMOVED Requirements

None.

## Out of Scope (locked)

Cross-workspace federation (E22+); C4 kinds already in `NodeKind`;
migration of pre-existing tests that asserted on the old `(id, kind)` PK.