# Design: multimodal-docs-source

## Technical Approach

Additive "Generic Graph Layer" (Approach 2 from exploration) layered *on top* of the existing code-only `Symbol`/`CallGraph`. New types (`NodeKind`, `EdgeKind`, `GraphNode`, `GraphEdge`, `NodeId`) live in `cognicode-core` domain alongside existing types. New PG tables (`graph_nodes`, `graph_edges`) coexist additively with `symbols`/`call_edges`. A `SourceExtractor` trait abstracts file-to-graph ingestion; `DocsExtractor` is the first implementation. Everything is gated behind `multimodal` Cargo feature.

## Architecture Decisions

| Decision | Choice | Rejected | Rationale |
|----------|--------|----------|-----------|
| Graph model | Generic layer alongside existing | Extend in-place; Full rewrite | Additive → OCP-safe. Existing CallGraph byte-for-byte unchanged. Dual-model risk mitigated by feature gate. |
| NodeId format | `(String, NodeKind)` newtype | UUID; Raw String | Typed discrimination enables compile-time kind safety. Kind-specific format validation. |
| MD parsing | `pulldown-cmark` (new dep) | regex; custom parser | Industry-standard, event-based, zero-copy. Handles ADR front-matter via `pull_down` events. |
| Docs confidence | Pure functions (`DocsConfidenceRules`) | ML model; configurable per-user | Deterministic, testable, spec-defined 4-tier scoring. |
| Feature gate | `multimodal` feature per crate | Runtime config; separate binary | Compile-time zero-cost when disabled. No dead code. Matches existing `postgres`/`sqlite` pattern. |

## Data Model

### NodeKind (`cognicode-core/src/domain/value_objects/node_kind.rs`)

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum NodeKind {
    Symbol(SymbolKind),
    Decision,
    Doc,
    Issue,
    Evidence,
}
```

### EdgeKind (`cognicode-core/src/domain/value_objects/edge_kind.rs`)

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EdgeKind {
    Dependency(DependencyType),
    Cites,
    Justifies,
    Resolves,
    CorroboratedBy,
}
```

### NodeId (`cognicode-core/src/domain/value_objects/node_id.rs`)

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(String, NodeKind);

impl NodeId {
    pub fn symbol(id: &str) -> Result<Self, NodeIdError> { /* validates file:name:line */ }
    pub fn doc(path: &str, slug: &str) -> Result<Self, NodeIdError> { /* doc:<path>#<slug> */ }
    pub fn decision(path: &str, slug: &str) -> Result<Self, NodeIdError> { /* decision:<path>#<slug> */ }
    pub fn issue(tracker: &str, number: &str) -> Result<Self, NodeIdError> { /* issue:<tracker>#<number> */ }
    pub fn evidence(sha256: &str) -> Result<Self, NodeIdError> { /* ev:<sha256> */ }
    pub fn kind(&self) -> &NodeKind { &self.1 }
    pub fn id_str(&self) -> &str { &self.0 }
}
```

### GraphNode (`cognicode-core/src/domain/aggregates/graph_node.rs`)

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: NodeId,
    pub label: String,
    pub source_path: Option<String>,
    pub metadata: serde_json::Value,
}
```

### GraphEdge (`cognicode-core/src/domain/aggregates/graph_edge.rs`)

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphEdge {
    pub source: NodeId,
    pub target: NodeId,
    pub kind: EdgeKind,
    pub provenance: Provenance,
    pub confidence: f64, // validated [0.0, 1.0] + finite
}
```

## PG Schema

Migration: `m0009_graph_nodes_edges.sql` (loaded via `include_str!`, following existing pattern).

```sql
CREATE TABLE IF NOT EXISTS graph_nodes (
    id          TEXT NOT NULL,
    kind        TEXT NOT NULL,
    label       TEXT NOT NULL,
    source_path TEXT,
    metadata    JSONB,
    PRIMARY KEY (id, kind)
);
CREATE INDEX IF NOT EXISTS idx_graph_nodes_kind ON graph_nodes(kind);
CREATE INDEX IF NOT EXISTS idx_graph_nodes_fts ON graph_nodes
    USING gin(to_tsvector('english', coalesce(label,'') || ' ' || coalesce(metadata::text,'')));

CREATE TABLE IF NOT EXISTS graph_edges (
    source_id   TEXT NOT NULL,
    source_kind TEXT NOT NULL,
    target_id   TEXT NOT NULL,
    target_kind TEXT NOT NULL,
    kind        TEXT NOT NULL,
    provenance  TEXT NOT NULL,
    confidence  DOUBLE PRECISION NOT NULL,
    PRIMARY KEY (source_id, source_kind, target_id, target_kind, kind)
);
CREATE INDEX IF NOT EXISTS idx_graph_edges_source ON graph_edges(source_id, source_kind);
CREATE INDEX IF NOT EXISTS idx_graph_edges_target ON graph_edges(target_id, target_kind);
```

**Coexistence**: `symbols`/`call_edges` untouched. No `ALTER`, no data migration.

## Source Extractor

### Trait (`cognicode-explorer/src/ports/source_extractor.rs`)

```rust
#[async_trait]
pub trait SourceExtractor: Send + Sync {
    async fn extract(&self, source: SourcePath) -> Result<Vec<ExtractedNode>, ExtractionError>;
}
pub enum SourcePath { LocalPath(PathBuf), GitBlob { repo: String, sha: String, path: String } }
pub struct ExtractedNode { pub node: GraphNode, pub edges: Vec<GraphEdge> }
```

### DocsExtractor (`cognicode-core/src/domain/services/docs_extractor.rs`)

Flow: read file → detect ADR front-matter → parse with `pulldown-cmark` → extract headings/links/code-fences → resolve links via `DocsConfidenceRules` → emit `(GraphNode, Vec<GraphEdge>)`.

### DocsConfidenceRules (`cognicode-core/src/domain/services/docs_confidence.rs`)

| Rule | Confidence | Provenance |
|------|-----------|------------|
| `link_exact` | 0.9 | Extracted |
| `heading_match` | 0.7 | Extracted |
| `link_fuzzy` | 0.6 | Ambiguous |
| `unresolved` | 0.3 | Ambiguous |

## GraphRepository Port (`cognicode-explorer/src/ports/graph_repository.rs`)

```rust
#[async_trait]
pub trait GraphRepository: Send + Sync {
    async fn upsert_node(&self, node: GraphNode) -> Result<()>;
    async fn upsert_edge(&self, edge: GraphEdge) -> Result<()>;
    async fn find_nodes_by_kind(&self, kind: NodeKind) -> Result<Vec<GraphNode>>;
    async fn find_edges(&self, source: &NodeId, kind: Option<&EdgeKind>) -> Result<Vec<GraphEdge>>;
    async fn find_incoming_edges(&self, target: &NodeId) -> Result<Vec<GraphEdge>>;
    async fn search(&self, query: &str, kinds: &[NodeKind], limit: usize, cursor: Option<String>) -> Result<SearchPage>;
}
```

PG impl: `cognicode-core/src/infrastructure/persistence/generic_graph_repository.rs`. Uses `sqlx::Postgres` + FTS5 `to_tsvector`.

## MCP Tools

### `docs_ingest` — Input: `{ paths: string[], recursive?: bool }` → `IngestionSummary`
### `graph_search` — Input: `{ query: string, kinds?: string[], limit?: number, cursor?: string }` → `{ results: SearchResult[], total: number, next_cursor?: string }`

Both registered in `mcp.rs` under `#[cfg(feature = "multimodal")]`. Feature disabled → tools not in `tools/list`, call returns `-32601`.

## ExplorerQL Extensions

`TargetType` gets `Decisions` + `Docs` variants. `compile.rs` dispatches to `GraphRepository::find_nodes_by_kind`. Existing 4 targets byte-for-byte unchanged. WHERE fields: `Decisions { status, date, label }`, `Docs { section, label, source_path }`.

## Frontend Changes

- `GraphNodeStyleClass`: extend from `z.enum([3])` to `z.enum([7])` (add decision, doc, issue, evidence)
- `GraphEdgeStyleClass`: extend from `z.enum([3])` to `z.enum([7])` (add edge.cites, edge.justifies, edge.resolves, edge.corroborated_by)
- `stylesheet.ts`: add 4 node styles (diamond/round-octagon/triangle/ellipse) + 4 edge styles (dotted/solid/dashed variants)
- `ObjectInspector`: render kind badge + metadata table + Citations section for multimodal nodes
- Backend `style_class_for`/`edge_style_class_for`: extend match arms for new `NodeKind`/`EdgeKind`

## Feature Gate

`multimodal` feature in `cognicode-core` and `cognicode-explorer` Cargo.toml. Pulled in by `cognicode-mcp` and `cognicode-cli`.

| Feature | Compiles with | Compiles without |
|---------|-------------|-----------------|
| `multimodal` | All new types, DocsExtractor, PG migration, MCP tools, ExplorerQL targets | None of the above. `cargo build --no-default-features` produces zero new symbols |

## Module Dependency Diagram

```
cognicode-core
├── domain/value_objects/
│   ├── symbol_kind.rs        (existing, untouched)
│   ├── dependency_type.rs    (existing, untouched)
│   ├── provenance.rs         (existing, untouched)
│   ├── node_kind.rs          (NEW, #[cfg(multimodal)])
│   ├── edge_kind.rs          (NEW, #[cfg(multimodal)])
│   └── node_id.rs            (NEW, #[cfg(multimodal)])
├── domain/aggregates/
│   ├── symbol.rs             (existing, untouched)
│   ├── call_graph.rs         (existing, untouched)
│   ├── graph_node.rs         (NEW, #[cfg(multimodal)])
│   └── graph_edge.rs         (NEW, #[cfg(multimodal)])
├── domain/services/
│   ├── docs_extractor.rs     (NEW, #[cfg(multimodal)], dep: pulldown-cmark)
│   └── docs_confidence.rs    (NEW, #[cfg(multimodal)])
└── infrastructure/persistence/
    ├── generic_graph_repository.rs (NEW, #[cfg(all(postgres, multimodal))])
    └── schema_postgres.sql   (MODIFIED: add graph_nodes/graph_edges DDL)

cognicode-explorer
├── ports/
│   ├── graph_repository.rs   (NEW, #[cfg(multimodal)])
│   └── source_extractor.rs   (NEW, #[cfg(multimodal)])
├── adapters/
│   └── docs_source_adapter.rs (NEW, #[cfg(multimodal)])
├── moldql/
│   ├── ast.rs                (MODIFIED: TargetType += Decisions, Docs)
│   ├── parser_explorerql.rs  (MODIFIED: accept new keywords)
│   └── compile.rs            (MODIFIED: dispatch multimodal targets)
├── dto.rs                    (MODIFIED: InspectableObjectType += DocNode, DecisionNode)
├── api.rs                    (MODIFIED: style_class_for/edge_style_class_for)
└── mcp.rs                    (MODIFIED: += docs_ingest, graph_search tools)
```

## Testing Strategy

| Layer | What | Approach |
|-------|------|----------|
| Unit | NodeKind/EdgeKind/NodeId roundtrip, confidence rules, GraphEdge validation | Pure Rust tests in each module |
| Integration | PG roundtrip (upsert → find → search), DocsExtractor against fixtures | `sqlx::test` with PG container |
| E2E | MCP tool schema validation, CLI exit codes, frontend Zod parse | Integration tests + Playwright |

## Information Bottleneck Check (Protocol C)

| Interface X→T→Y | I(X;T) | I(T;Y) | Δ | Status |
|------------------|--------|--------|---|--------|
| `SourceExtractor` (file bytes → `Vec<ExtractedNode>`) → PG store | ~5.2 bits | ~4.8 bits | 0.4 | ✅ Good |
| `GraphRepository` (domain types → SQL rows → domain types) | ~6.1 bits | ~5.8 bits | 0.3 | ✅ Good |
| `style_class_for` (NodeKind → string → Cytoscape) | ~2.6 bits | ~2.6 bits | 0.0 | ✅ Perfect |
| `graph_search` FTS5 (text query → SearchPage → JSON) | ~4.3 bits | ~3.2 bits | 1.1 | ⚠️ Flag: score normalization loses precision |
| ExplorerQL compile (AST → repo call → results) | ~3.8 bits | ~3.5 bits | 0.3 | ✅ Good |

**Flagged**: `graph_search` I(X;T) - I(T;Y) = 1.1 bits. The FTS5 → normalized score compression discards rank detail. Mitigation: expose raw `rank` alongside `score` in the response so downstream can reconstruct if needed.

## Migration / Rollout

No data migration required. Additive DDL only. Feature gate controls activation.

## Open Questions

- [ ] Should `pulldown-cmark` be a workspace dep or per-crate dep?
- [ ] `graph_search` score normalization formula (0.6*fts5 + 0.4*kind_bonus) — validate with real data after first ingest?
