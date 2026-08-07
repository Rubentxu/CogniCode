## Exploration: Named Views

### Current State

The CogniCode Explorer has no persistent named views today. The closest existing artifact is the **exploration path** (`ExplorationPath` in `crates/cognicode-explorer/src/dto.rs`), which captures a user's navigation history (columns visited, active view, lens, objects touched). However, exploration paths are:

- Stored **in-memory only** behind `Arc<Mutex<HashMap<String, ExplorationPath>>>` in `ExplorerService` — they do not survive a restart.
- Not shareable — no link-stable URL resolves to a named view.
- Not intentionally saved by the user — they accumulate as navigation history, not as bookmarked projections.

The existing "views" in the codebase are **contextual views** (`ContextualView`): parameterized projections of a single object through a specific lens (overview, call-graph, source, evidence, quality, symbols, hotspots, dependencies, etc.). These are computed on-demand via `ExplorerService::contextual_view(object_id, view_id)` — they have no persistence layer and no user-defined names.

The **MCP** surface has 24 tools, including `explorer_get_views`/`explorer_get_view` for contextual views, and `graph_subgraph` for extracting neighborhood subgraphs. None of these save or recall named projections.

### Affected Areas

| Path | Why Affected |
|------|-------------|
| `crates/cognicode-explorer/src/dto.rs` | New DTOs: `NamedView`, `NamedViewDescriptor`, `SaveViewRequest` |
| `crates/cognicode-explorer/src/service.rs` | New service methods: `save_view`, `load_view`, `list_views`, `delete_view` |
| `crates/cognicode-explorer/src/mcp.rs` | 4 new MCP tools: `view_save`, `view_load`, `view_list`, `view_delete` |
| `crates/cognicode-core/src/infrastructure/persistence/schema_postgres.sql` | New `named_views` table (or `views` table) |
| `crates/cognicode-core/src/infrastructure/persistence/postgres_repository.rs` | Save/load/list/delete for named_views |
| `docs/explorer-graph/glossary.md` | Add/update "Named view" entry with implementation details |
| `docs/explorer-graph/visualizations.md` | The "Named view landing" visualization references this store |
| `docs/explorer-graph/roadmap.md` | Phase 3 specifies the named view store |

### Data Model Analysis

#### What is a "named view"?

From the target product model and glossary, a **named view** is a saved, shareable, link-stable projection of the graph. It is defined as a **four-tuple**:

```
(level, lens, focus_node, bounded_radius)
```

Plus metadata: `name`, `description`, `creator/owner`, `created_at`, `updated_at`.

The visualization doc (`visualizations.md`, line 73) states:
> A named view is a saved four-tuple plus a name, a description, and a creation time. A shared link resolves to a named view.

From the glossary:
> A named view is a saved, shareable, link-stable projection of the graph. A named view is what a user pastes into a chat message or a PR description to point a teammate at a specific lens on a specific region.

#### Relationship to existing concepts

| Concept | Relationship to Named View |
|---------|---------------------------|
| ExplorationPath | **Precursor** — captures navigation history. Named views are intentional saves of specific projections. Exploration paths are the "seed" (per current-state-audit.md line 139). |
| ContextualView | **Renderer** — a named view stores the parameters; the contextual view renderer produces the actual view when the named view is loaded. |
| ExplorerQL | **Query surface** — a named view IS a saved ExplorerQL query/projection. Phase 3 of query-and-navigation.md: "Named queries: save a query by name and call it from another query." |
| graph_subgraph MCP tool | **Closest primitive** — the `graph_subgraph(root, direction, max_depth)` tool produces a subgraph that, combined with a lens, is essentially a named view. |

#### Proposed schema for `named_views` table

```sql
CREATE TABLE IF NOT EXISTS named_views (
    id              TEXT PRIMARY KEY,       -- UUID, deterministic
    name            TEXT NOT NULL,          -- user-provided name (1-256 chars)
    description     TEXT,                   -- optional description
    workspace_id    TEXT NOT NULL,          -- FK to workspace/source
    focus_node      TEXT NOT NULL,          -- object_id of the focused node
    level           TEXT NOT NULL,          -- C4 level: code|component|container|system
    lens            TEXT,                   -- lens_id (e.g. "overview", "call-graph", "hotspots")
    direction       TEXT DEFAULT 'both',    -- SubgraphDirection serialized
    max_depth       INTEGER DEFAULT 3,      -- bounding radius
    owner           TEXT,                   -- user identifier
    share_link      TEXT,                   -- unique shareable link slug
    projection_config JSONB,                -- flexible: extra projection params, node/edge kind filters
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_named_views_workspace ON named_views(workspace_id);
CREATE INDEX IF NOT EXISTS idx_named_views_owner ON named_views(owner);
CREATE UNIQUE INDEX IF NOT EXISTS idx_named_views_share ON named_views(share_link);
```

The `projection_config` JSONB column carries optional extra filters:
```json
{
  "node_kinds": ["Symbol", "Component"],
  "edge_kinds": ["calls", "called_by", "part_of"],
  "confidence_min": 0.5,
  "provenance_filter": ["extracted", "inferred"]
}
```

#### Why PostgreSQL?
- The canonical store is PostgreSQL (`stack-recommendation.md`)
- Named views are persistent, shareable artifacts — they belong in the canonical store, not in-memory
- `JSONB` is already used in the schema strategy for flexible payload

### v1 Scope Recommendation

Per the roadmap (Phase 3): "Scope them as light, shareable, link-stable artifacts in v1; no editing history in v1."

| Feature | v1 | v2+ |
|---------|:--:|:---:|
| Save view (CRUD) | ✅ | ✅ |
| Load view by ID | ✅ | ✅ |
| List user's views | ✅ | ✅ |
| Delete view | ✅ | ✅ |
| Share-by-link | ❌ | ✅ |
| Version history | ❌ | ✅ |
| Edit/update view | ❌ (replace) | ✅ |
| Fork/copy view | ❌ | ✅ |
| View-as-ExplorerQL export | ❌ | ✅ |
| View-to-Mermaid export | ❌ | ✅ |
| Team workspaces / shared views | ❌ | ✅ |

v1 is **pure CRUD**: save, load, list, delete. No versioning, no sharing, no editing. This keeps the slice small and reviewable.

### MCP Tool Surface

Four new tools complete the surface:

| Tool | Signature | Returns |
|------|-----------|---------|
| `view_save` | `(name, description?, focus_node, level, lens?, direction?, max_depth?)` | `NamedView` |
| `view_load` | `(view_id)` | `ContextualView` rendered from the saved projection |
| `view_list` | `(workspace_id)` | `Vec<NamedViewDescriptor>` (id, name, description, focus_node, created_at) |
| `view_delete` | `(view_id)` | `{ deleted: bool }` |

This brings the total MCP tool count from 24 to 28. Each tool returns the standard `McpResultEnvelope`.

### Entropy Analysis (Connascence Landscape)

**Method**: Heuristic

| Component A | Component B | Connascence Type | I(bits) | Severity |
|-------------|-------------|------------------|---------|----------|
| `dto.rs` (new NamedView) | `service.rs` (CRUD methods) | Name | 0.5 | ✅ OK |
| `service.rs` | `mcp.rs` (tool dispatch) | Name | 0.5 | ✅ OK |
| `postgres_repository.rs` | `schema_postgres.sql` | Meaning | 1.0 | ⚠️ Low — schema and repository must agree on column names |
| `mcp.rs` (envelope) | `dto.rs` (NamedView serialization) | Type | 1.0 | ⚠️ Low — standard MCP envelope pattern, already established |
| `service.rs` (save_view) | `postgres_repository.rs` (persist) | Name | 0.5 | ✅ OK |
| New `named_views` table | Existing `symbols`/`call_edges` | Meaning | 0.0 | ✅ OK — no coupling; the view references a focus_node by object_id string, but that's a weak reference, not a FK |

**Critical Pairs (I > 3.0 bits)**: None — this change is entirely additive and does not modify the existing 24-tool surface or the `ExplorationPath` mechanism.

**Hidden Connascence (Meaning/Timing)**: The `share_link` column implies a future URL resolution mechanism. If the share link is generated at save time (deterministic slug from UUID), there's no hidden coupling. If it requires a separate URL-shortening service, that would introduce meaning connascence. **Recommendation**: generate the share link deterministically from a UUID at save time.

**SOLID-Entropy Violations**: None detected. The change is:
- **SRP**: The `NamedViewRepository` trait (or methods on `PostgresRepository`) focuses on one responsibility.
- **OCP**: Pure extension — no existing code paths are modified.
- **LSP**: Not applicable (no new subtypes of existing traits).
- **ISP**: The new MCP tools and service methods are minimal — exactly the CRUD surface.
- **DIP**: The service depends on a trait (to be defined: `NamedViewRepository`), not a concrete implementation.

**Coupling Score**: H_external ≈ 0.5 bits (low — only the MCP envelope pattern is shared; the core projection engine is untouched)

**Recommendation**: Accept. The change is low-risk, purely additive, and follows established patterns (MCP tool dispatch, JSONB schema, service/repository separation).

### Approach Comparison

| Approach | Pros | Cons | Complexity |
|----------|------|------|------------|
| **A: New `named_views` table in PostgreSQL** | Canonical, durable, queryable, team-shareable. Follows the PostgreSQL-is-truth architecture. | Requires `postgres` feature for views to work. Schema migration needed. | Medium |
| **B: In-memory + JSON snapshot export** | Zero schema migration. No PG dependency. Fast iteration. | Not durable. Not team-shareable. Violates the "PostgreSQL is canonical" decision. | Low |
| **C: Extend ExplorationPath with `is_saved` flag** | Minimal new code. Reuses existing struct. | Wrong abstraction — paths are history, views are bookmarks. Conflates concerns. No share-by-link path. | Low |

**Recommendation: Approach A**. The roadmap and architecture decisions commit to PostgreSQL as the canonical store. Approach B violates that decision. Approach C conflates exploration history with intentional views.

### Risks

- **Share link collision**: Mitigated by using UUIDs as the basis for share links (2^122 space). Deterministic slug generation avoids a URL-shortening service dependency.
- **Schema migration**: The `named_views` table is additive (no ALTER on existing tables). The migration strategy in `postgres_repository.rs` already supports adding new `CREATE TABLE IF NOT EXISTS` statements to `schema_postgres.sql`.
- **Feature gate**: Views should compile even without the `postgres` feature by providing an in-memory fallback implementation behind a trait. The `postgres` feature enables the real persist-implementation.
- **Scope creep**: The v1 scope MUST stay at CRUD. Sharing, versioning, and editing are Phase 3+ concerns.

### Ready for Proposal
**Yes** — the data model, persistence strategy, MCP surface, and scope boundaries are clear. The orchestrator should proceed to `sdd-propose` for the `named-views` change.
