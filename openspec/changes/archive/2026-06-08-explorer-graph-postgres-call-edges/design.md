# Design: PostgreSQL `call_edges` — Read-Path + Minimal Write Helper

## Technical Approach

Pure additive extension: new `call_edges` table in `schema_postgres.sql`, new `EdgeMetadata` value object, 3 new async methods on the existing `Repository` trait, `PostgresRepository` implementation via `sqlx::query_as` + private `EdgeRow`, and a `pub(crate)` `insert_edge()` write helper. No frontend, MCP, GraphStore, or query-language changes. Feature-gated behind existing `postgres` feature.

## Architecture Decisions

### Decision: Schema placement — append to `schema_postgres.sql`

| Option | Tradeoff | Decision |
|--------|----------|----------|
| Append DDL to existing `schema_postgres.sql` | Same `include_str!` + `run_migrations()` pattern as symbols. No new migration framework. | ✅ Chosen |
| Separate `schema_postgres_edges.sql` + new `include_str!` | Cleaner separation but duplicates migration plumbing. Premature at 2 tables. | ❌ Rejected |
| `sqlx-cli` migrations | Pulls `sqlx-sqlite` (conflicts with `rusqlite`). | ❌ Rejected |

**Rationale**: `schema_postgres.sql` is 23 lines. At 2 tables the single-file approach is still clean. The file header already says "Switch to a migration framework once the table count exceeds 3."

### Decision: `EdgeMetadata` — 7-field struct, no `Serialize`

| Option | Tradeoff | Decision |
|--------|----------|----------|
| `EdgeMetadata` with `caller_id, caller_name, callee_id, callee_name, dependency_type, provenance, confidence` | Exact column-for-column parity with SQLite v2 `call_edges`. No `id` field (surrogate PK is persistence detail). | ✅ Chosen |
| `EdgeMetadata` with `FromRow` | Couples domain to `sqlx`. | ❌ Rejected |

**Rationale**: The struct mirrors the 7 data columns (excludes `id SERIAL`). Derives `Debug, Clone, PartialEq` only — no `Serialize` (spec requirement). Ungated — available without `postgres` feature.

### Decision: Parsing `provenance` and `dependency_type` from DB strings

| Option | Tradeoff | Decision |
|--------|----------|----------|
| Add `FromStr` to `Provenance` and `DependencyType` | Reusable, idiomatic Rust. SQLite code stores via `Display`/`Debug` so `FromStr` completes the round-trip. | ✅ Chosen |
| Match-on-string in `EdgeRow::into_edge()` | Works but duplicates parsing logic, no reuse. | ❌ Rejected |

**Rationale**: SQLite stores `Provenance` via `Display` (`"Extracted"`, `"Inferred"`, `"Ambiguous"`) and `DependencyType` via `Debug` (`"Calls"`, `"Imports"`, ...). Adding `FromStr` to both types enables clean parsing in `EdgeRow`. Fallback on parse failure: `Provenance::Extracted`, `DependencyType::Calls` (spec requirement).

### Decision: `insert_edge()` — `pub(crate)` inherent method

| Option | Tradeoff | Decision |
|--------|----------|----------|
| `pub(crate)` inherent method on `PostgresRepository` | Visible to tests within the crate. Not on trait — no API surface leak. | ✅ Chosen |
| Private method + `#[cfg(test)] pub` re-export | More complex, same effective visibility. | ❌ Rejected |

**Rationale**: `pub(crate)` already restricts visibility to the crate. Tests in `postgres_repository.rs` are in the same crate. No additional gating needed.

### Decision: `EdgeRow` — private `FromRow` struct

| Option | Tradeoff | Decision |
|--------|----------|----------|
| Private `#[derive(sqlx::FromRow)] EdgeRow` in `postgres_repository.rs` | Same pattern as existing `SymbolRow`. Keeps `sqlx` out of domain types. | ✅ Chosen |
| `FromRow` on `EdgeMetadata` directly | Couples domain to `sqlx`. Breaks ungated requirement. | ❌ Rejected |

**Rationale**: Identical to the proven `SymbolRow` pattern. `EdgeRow` has 7 `String`/`f64` fields, maps to `EdgeMetadata` via `into_edge()` which parses enums.

## Data Flow

```
                      EdgeMetadata (domain)
                           │
              ┌────────────┼────────────┐
              ▼            │            ▼
     EdgeRow (FromRow)     │    insert_edge()
     sqlx::query_as        │    sqlx::query
              │            │            │
              ▼            │            ▼
     ┌─────────────────────────────────────────┐
     │  call_edges (PostgreSQL)                │
     │  id | caller_id | caller_name |         │
     │     callee_id | callee_name |           │
     │     dependency_type | provenance |       │
     │     confidence                          │
     │  INDEX: caller_id, callee_id            │
     └─────────────────────────────────────────┘
              ▲                          │
              │                          │
     find_edges_by_caller       find_edges_by_callee
     WHERE caller_id = $1       WHERE callee_id = $1
              │                          │
              └──────── Vec<EdgeMetadata>┘
```

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `crates/cognicode-core/src/domain/value_objects/edge_metadata.rs` | **Create** | `EdgeMetadata` struct (7 fields, `Debug, Clone, PartialEq`). No `Serialize`. |
| `crates/cognicode-core/src/domain/value_objects/mod.rs` | **Modify** | Add `pub mod edge_metadata;` + `pub use edge_metadata::EdgeMetadata;` |
| `crates/cognicode-core/src/domain/value_objects/provenance.rs` | **Modify** | Add `FromStr` impl (parse `"Extracted"` etc., fallback on error) |
| `crates/cognicode-core/src/domain/value_objects/dependency_type.rs` | **Modify** | Add `FromStr` impl (parse `"Calls"`, `"calls"` etc., fallback on error) |
| `crates/cognicode-core/src/domain/traits/repository.rs` | **Modify** | Add 3 methods: `find_edges_by_caller`, `find_edges_by_callee`, `count_edges`. Update `EmptyRepo`/`CountingRepo` stubs. |
| `crates/cognicode-core/src/infrastructure/persistence/schema_postgres.sql` | **Modify** | Append `call_edges` DDL + 2 indexes (`IF NOT EXISTS`). |
| `crates/cognicode-core/src/infrastructure/persistence/postgres_repository.rs` | **Modify** | Add `EdgeRow` struct, `into_edge()`, 3 trait method impls, `insert_edge()` helper, ~8 new `pg_test!` tests. |

## Interfaces / Contracts

### `EdgeMetadata` (new)

```rust
/// Value object representing a call-graph edge with metadata.
/// Ungated — available without `postgres` feature.
#[derive(Debug, Clone, PartialEq)]
pub struct EdgeMetadata {
    pub caller_id: String,
    pub caller_name: String,
    pub callee_id: String,
    pub callee_name: String,
    pub dependency_type: DependencyType,
    pub provenance: Provenance,
    pub confidence: f64,
}
```

### `Repository` trait extension (3 new methods)

```rust
#[async_trait]
pub trait Repository: Send + Sync {
    // existing...
    async fn find_symbol_by_qualified_name(&self, name: &str)
        -> Result<Option<Symbol>, RepositoryError>;
    async fn count_symbols(&self) -> Result<usize, RepositoryError>;

    // NEW:
    async fn find_edges_by_caller(&self, caller_id: &str)
        -> Result<Vec<EdgeMetadata>, RepositoryError>;
    async fn find_edges_by_callee(&self, callee_id: &str)
        -> Result<Vec<EdgeMetadata>, RepositoryError>;
    async fn count_edges(&self) -> Result<usize, RepositoryError>;
}
```

### `EdgeRow` (private, feature-gated)

```rust
#[cfg(feature = "postgres")]
#[derive(Debug, sqlx::FromRow)]
struct EdgeRow {
    caller_id: String,
    caller_name: String,
    callee_id: String,
    callee_name: String,
    dependency_type: String,
    provenance: String,
    confidence: f64,
}
```

### SQL queries

```sql
-- find_edges_by_caller
SELECT caller_id, caller_name, callee_id, callee_name,
       dependency_type, provenance, confidence
FROM call_edges
WHERE caller_id = $1
ORDER BY id;

-- find_edges_by_callee
SELECT caller_id, caller_name, callee_id, callee_name,
       dependency_type, provenance, confidence
FROM call_edges
WHERE callee_id = $1
ORDER BY id;

-- count_edges
SELECT COUNT(*) AS n FROM call_edges;

-- insert_edge
INSERT INTO call_edges
  (caller_id, caller_name, callee_id, callee_name,
   dependency_type, provenance, confidence)
VALUES ($1, $2, $3, $4, $5, $6, $7);
```

### `FromStr` additions

```rust
// provenance.rs — Display strings: "Extracted", "Inferred", "Ambiguous"
impl FromStr for Provenance {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Extracted" => Ok(Provenance::Extracted),
            "Inferred" => Ok(Provenance::Inferred),
            "Ambiguous" => Ok(Provenance::Ambiguous),
            _ => Err(()),
        }
    }
}

// dependency_type.rs — Debug strings: "Calls", "Imports", etc.
// Also accept Display lowercase: "calls", "imports", etc.
impl FromStr for DependencyType {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Calls" | "calls" => Ok(DependencyType::Calls),
            "Imports" | "imports" => Ok(DependencyType::Imports),
            "Inherits" | "inherits" => Ok(DependencyType::Inherits),
            "UsesGeneric" | "uses_generic" => Ok(DependencyType::UsesGeneric),
            "References" | "references" => Ok(DependencyType::References),
            "Defines" | "defines" => Ok(DependencyType::Defines),
            "AnnotatedBy" | "annotated_by" => Ok(DependencyType::AnnotatedBy),
            "Contains" | "contains" => Ok(DependencyType::Contains),
            _ => Err(()),
        }
    }
}
```

## Testing Strategy

| Layer | What to Test | Approach |
|-------|-------------|----------|
| Unit | `EdgeMetadata` construction + equality | Plain `#[test]`, no DB |
| Unit | `Provenance::from_str` round-trip through `Display` | `#[test]`, ungated |
| Unit | `DependencyType::from_str` round-trip through `Debug` + `Display` | `#[test]`, ungated |
| Unit | `Repository` trait `EmptyRepo`/`CountingRepo` stubs return `vec![]`/`0` | Existing `#[cfg(test)]` in `repository.rs` |
| Integration | `find_edges_by_caller` returns seeded edges in insertion order | `pg_test!` with isolated DB |
| Integration | `find_edges_by_callee` returns empty `Vec` when no edges | `pg_test!` |
| Integration | `count_edges()` returns 0 on fresh, N after seeding | `pg_test!` |
| Integration | `insert_edge()` round-trip: insert then query back | `pg_test!` |
| Integration | Re-migration preserves existing rows | `pg_test!` |
| Integration | Per-test isolation: one test's inserts invisible to another | `pg_test!` |
| Integration | Column-set parity via `information_schema.columns` | `pg_test!` |
| Integration | `dyn Repository` still works after trait extension | `pg_test!` |

## Migration / Rollout

No migration required. `call_edges` DDL uses `IF NOT EXISTS`. `run_migrations()` on a populated DB is a no-op that preserves every existing `symbols` row. The trait extension is additive — all existing implementations gain 3 new methods with default-compatible stubs.

## Open Questions

- [ ] Should `EdgeMetadata` get a builder or constructor helper? (Not blocking — can add later)
- [ ] Should `FromStr` for `DependencyType` accept ALL case variants or just the two stored formats? (Design accepts both Debug and Display forms — spec says fallback to `Calls` on failure)

## Entropy Constraints

**Method**: Heuristic (CogniCode unavailable)

| Interface | I(X;T) Leakage | I(T;Y) Coverage | Bottleneck Quality | SOLID Check |
|-----------|---------------|-----------------|-------------------|-------------|
| `Repository` (extended) | Low — 5 methods, each independent | High — callers query exactly what they need | ✅ Optimal | SRP ✅, ISP ✅, DIP ✅ |
| `EdgeMetadata` | Low — plain data, no behavior | High — all 7 fields used by every caller | ✅ Optimal | SRP ✅, ISP ✅ |
| `EdgeRow` (private) | Zero — not visible outside module | N/A (internal) | ✅ Optimal | Encapsulated |

**DQS Estimate**: ~0.78 (unchanged — pure extension, no coupling increase)
**OCP**: H(Δ_existing) ≈ 1.58 bits — AMBER but within budget for additive trait extension.
