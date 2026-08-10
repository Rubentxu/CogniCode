# ADR-027: LadybugDB hybrid schema strategy

**Status**: ACCEPTED (implemented in `LadybugStore` via `quality_schema_ddls()` with `:Issue`, `:Baseline`, `:Rule` tables using MAP<STRING,STRING> + typed columns; reconciled via `debt-e29-3-1` to namespace `:QualityIssue`, `:QualityBaseline`, `:QualityRule` per ADR-027 collision; 2026-08-03)
**Date**: 2026-07-30
**Deciders**: User, OpenCode orchestrator

## Context

LadybugDB is schema-full: every node table requires pre-defined schema with typed columns and a mandatory primary key. This differs from PostgreSQL JSONB columns that accept arbitrary JSON. CogniCode's domain has two categories of properties:

1. **Stable properties**: known at schema design time (symbol name, file path, line number, node kind, edge kind, revision_id, etc.)
2. **Emergent properties**: discovered or added over time without schema migration (ownership metadata, custom annotations, extracted facts from LLM, etc.)

Using typed columns for everything would require constant schema migrations. Using `MAP<STRING,STRING>` for everything loses type safety and makes queries harder.

## Decision

Use a hybrid strategy: typed columns for stable properties, `MAP<STRING,STRING>` for emergent/unknown properties.

### 1. Typed columns for stable properties

Every node table has mandatory typed columns derived from the domain entity. The primary key is `SERIAL INT64` (auto-increment). All domain fields that are known at schema design time use native Cypher/LadybugDB types:

```
CREATE NODE TABLE Symbol (
  id SERIAL INT64 PRIMARY KEY,
  workspace_id INT64 NOT NULL,
  revision_id INT64 NOT NULL,
  name STRING NOT NULL,
  kind STRING NOT NULL,       -- NodeKind as string: "function", "struct", etc.
  file_path STRING NOT NULL,
  line_number INT64 NOT NULL,
  signature STRING,            -- nullable: not all symbols have signatures
  valid_from INT64 NOT NULL,  -- revision_id of creation
  valid_to INT64 NOT NULL     -- revision_id of deletion, -1 = current
);
```

### 2. MAP<STRING,STRING> for emergent properties

Every node table has an optional `properties` column of type `MAP<STRING,STRING>` for properties discovered after schema design:

```
CREATE NODE TABLE Symbol (
  -- ... typed columns above ...
  properties MAP<STRING,STRING],  -- emergent properties
);
```

The `properties` map stores:
- Ownership metadata (`codeowners`, `last_author`, `author_email`)
- Extracted facts from LLM analysis
- Custom user annotations
- Any property that would otherwise require a schema migration

**Important**: `MAP<STRING,STRING>` stores only string values. For non-string values (numbers, booleans), the adapter serializes to JSON string and deserializes on read.

### 3. Property access patterns

Rust adapters map typed columns directly to domain fields. The `properties` map is serialized/deserialized as `serde_json::Value` in the domain layer:

```rust
// In the Rust adapter (pseudocode)
fn row_to_symbol(row: &lbug::Row) -> Symbol {
    let properties_json: serde_json::Value = row.get::<lbug::Map<String, String>>("properties")
        .map(|m| serde_json::from_map(m.into_iter().collect::<std::collections::HashMap<_, _>>()).unwrap_or_default())
        .unwrap_or_default();
    Symbol {
        id: row.get::<i64>("id"),
        workspace_id: row.get::<i64>("workspace_id"),
        revision_id: row.get::<i64>("revision_id"),
        name: row.get::<String>("name"),
        kind: NodeKind::from_str(&row.get::<String>("kind")),
        file_path: row.get::<String>("file_path"),
        line_number: row.get::<i64>("line_number"),
        signature: row.get::<Option<String>>("signature"),
        valid_from: row.get::<i64>("valid_from"),
        valid_to: row.get::<i64>("valid_to"),
        properties: properties_json,
    }
}
```

Domain code accesses `properties` via typed accessors:

```rust
impl Symbol {
    pub fn property(&self, key: &str) -> Option<&str> {
        self.properties.get(key).and_then(|v| v.as_str())
    }

    pub fn set_property(&mut self, key: String, value: serde_json::Value) {
        self.properties[key] = value;
    }
}
```

### 4. Multi-label nodes (LadybugDB v0.18+)

A node can have multiple labels. This is used for nodes that belong to multiple categories. The label is set at insertion time:

```cypher
-- A symbol that is also a decision
CREATE (s:Symbol {id: 1, workspace_id: 1, revision_id: 1, name: 'CachePolicy', kind: 'struct', file_path: 'src/cache.rs', line_number: 42, valid_from: 1, valid_to: -1}) SET s:Decision;
```

Labels used across the schema:
- Node type labels: `Symbol`, `Decision`, `Doc`, `Evidence`, `Issue`, `Component`, `Container`, `System`, `Route`, `Rule`, `Baseline`
- Entity labels: `Investigation`, `Artifact`, `ExplorationSession`, `NamedView`, `ViewSpec`
- State labels: `GraphReport`, `AnalyticsRun`, `FileRecord`, `Revision`, `Workspace`, `Space`

Multi-label enables poly形式 queries:
```cypher
-- Find all Decision nodes (regardless of whether they are also Symbol nodes)
MATCH (n:Decision) RETURN n;

-- Find nodes that are both Symbol and Component
MATCH (n:Symbol:Component) RETURN n;
```

### 5. Indexing strategy

Every node table has indexes on:
- Primary key (`id`) — automatic with `SERIAL PRIMARY KEY`
- `(workspace_id, revision_id)` — for temporal queries
- `kind` — for node-type filtering
- `valid_to = -1` — for current-state queries (index on constant -1 is cheap but useful for the query planner)

### 6. Relationship tables

Relationship tables are structured the same way as node tables:

```
CREATE REL TABLE Calls (
  id SERIAL INT64 PRIMARY KEY,
  workspace_id INT64 NOT NULL,
  revision_id INT64 NOT NULL,
  caller_id INT64 NOT NULL,
  callee_id INT64 NOT NULL,
  provenance STRING,           -- Provenance enum as string: "extractor", "llm", etc.
  confidence REAL,             -- f32/f64 for confidence score [0.0, 1.0]
  valid_from INT64 NOT NULL,
  valid_to INT64 NOT NULL,
  properties MAP<STRING,STRING],  -- emergent edge properties
);
```

Indexes on relationship tables:
- Primary key (`id`)
- `(caller_id, valid_to = -1)` — for current callees lookup
- `(callee_id, valid_to = -1)` — for current callers lookup
- `(workspace_id, revision_id)` — for temporal queries

## Alternatives considered

### All typed columns

Rejected — CogniCode's emergent property model (ownership, LLM-extracted facts) would require schema migrations for every new property type.

### All MAP<STRING,STRING>

Rejected — loses type safety for stable fields, makes queries verbose (no SQL-typed comparisons), harder to index and optimize.

### Separate extension tables

Rejected — multi-label nodes in v0.18+ achieve the same poly形式 benefit without the join complexity.

## Consequences

### Positive

- Schema migrations only for truly new stable fields (rare)
- Type safety for the majority of queries (typed columns)
- Flexibility for emergent properties (MAP column)
- Multi-label nodes reduce table count
- Consistent indexing strategy enables efficient temporal queries

### Negative

- `properties` map must be serialized/deserialized (small overhead)
- Two access patterns in domain code (typed vs. map)
- MAP column queries are string-keyed (no type-level enforcement for map values)

### Mitigations

- `serde_json::Value` adapter in domain layer normalizes access
- Property accessors on domain entities hide the map details
- Indexes on typed columns cover the common queries

## References

- [ADR-026](./ADR-026-ladybugdb-canonical-migration.md)
- [LadybugDB CREATE TABLE docs](https://docs.ladybugdb.com/cypher/data-definition/create-table/)
- [LadybugDB MAP type docs](https://docs.ladybugdb.com/cypher/data-types/)
- [LadybugDB multi-label docs](https://docs.ladybugdb.com/cypher/data-definition/create-table/#multiple-labels)
