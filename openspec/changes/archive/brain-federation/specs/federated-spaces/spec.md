# federated-spaces Specification (NEW)

## Purpose

First-class **Space** concept: a named, typed collection of graph data (a repo, a docs corpus, an issue tracker). Spaces are the unit of federation. Defined in `cognicode-core/src/domain/value_objects/space.rs` and `cognicode-core/src/domain/value_objects/space_id.rs`. All types and the PG migration are gated by the `multimodal` feature.

## Domain Types

| Type | File | Definition |
|------|------|------------|
| `SpaceId` | `crates/cognicode-core/src/domain/value_objects/space_id.rs` | Newtype `pub struct SpaceId(pub String)` — opaque, non-empty, Display + From<String> |
| `SpaceKind` | `crates/cognicode-core/src/domain/value_objects/space.rs` | `enum SpaceKind { Repo, Docs, Issues }` — 3 variants, derives `Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize` |
| `Space` | `crates/cognicode-core/src/domain/value_objects/space.rs` | `pub struct Space { id: SpaceId, name: String, kind: SpaceKind, source_path: Option<PathBuf>, config: serde_json::Value }` |
| `SpaceError` | `crates/cognicode-core/src/domain/value_objects/space.rs` | `enum SpaceError { EmptyId, EmptyName, ReservedId(String) }` |

## Requirements

### Requirement: SpaceId Non-Empty and Opaque

`SpaceId(impl Into<String>)` MUST reject empty strings via `try_new`. Reserved id `"default"` is constructed via `SpaceId::default()` and used for backward compatibility.

#### Scenario: Empty id rejected
- GIVEN `SpaceId::try_new("")`
- WHEN evaluated
- THEN it MUST return `Err(SpaceError::EmptyId)`

#### Scenario: Default id constant
- GIVEN `SpaceId::default()`
- THEN the result MUST equal `SpaceId("default".into())`

### Requirement: SpaceKind Exactly 3 Variants

`SpaceKind` MUST be `enum SpaceKind { Repo, Docs, Issues }` with no payload. All variants derive `Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize`.

#### Scenario: Roundtrip all 3 variants
- GIVEN `SpaceKind::Repo`, `SpaceKind::Docs`, `SpaceKind::Issues`
- WHEN serialized to JSON and deserialized
- THEN each MUST roundtrip without loss

### Requirement: Space Value Object

`Space` MUST contain `id: SpaceId, name: String, kind: SpaceKind, source_path: Option<PathBuf>, config: serde_json::Value`. `Space::new(id, name, kind)` MUST reject empty `name` via `try_new` returning `Err(SpaceError::EmptyName)`. The `config` field defaults to `serde_json::json!({})` when omitted.

#### Scenario: Construction with name and kind
- GIVEN `Space::try_new(SpaceId::default(), "auth-repo".into(), SpaceKind::Repo)`
- THEN `space.name == "auth-repo"` AND `space.kind == SpaceKind::Repo` AND `space.config == json!({})`

#### Scenario: Empty name rejected
- GIVEN `Space::try_new(SpaceId::default(), "".into(), SpaceKind::Repo)`
- THEN it MUST return `Err(SpaceError::EmptyName)`

### Requirement: spaces PG Table

The PG schema MUST add a `spaces` table gated by the `multimodal` feature:

```sql
CREATE TABLE spaces (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  kind TEXT NOT NULL CHECK (kind IN ('Repo','Docs','Issues')),
  source_path TEXT,
  config JSONB NOT NULL DEFAULT '{}'::jsonb,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_spaces_kind ON spaces(kind);
```

The migration MUST be additive (no ALTER on existing tables). Existing data MUST be unaffected.

#### Scenario: New table created
- GIVEN an empty database with the `multimodal` migration set applied
- WHEN a new migration `m00xx_spaces_table` runs
- THEN the `spaces` table exists with the columns above

#### Scenario: Default space auto-seeded
- GIVEN a freshly migrated multimodal database
- WHEN `SELECT * FROM spaces` runs
- THEN exactly one row exists with `id='default', name='default', kind='Repo'`

### Requirement: SpaceRegistry In-Memory CRUD

`SpaceRegistry` (in `cognicode-explorer/src/federation/space_registry.rs`) MUST expose `register(Space) -> Result<SpaceId>`, `get(SpaceId) -> Option<Space>`, `list() -> Vec<Space>`, `unregister(SpaceId) -> bool`. Registration MUST validate uniqueness of `id` (re-registering the same id returns `Err(SpaceError::Duplicate)`).

#### Scenario: Register and get
- GIVEN an empty registry
- WHEN `register(space_a)` and `get(SpaceId("a"))` run
- THEN `get` returns `Some(space_a)` with all fields intact

#### Scenario: Duplicate id rejected
- GIVEN a registry containing `space_a` with id `"a"`
- WHEN `register(space_b_with_id_a)` runs
- THEN it MUST return `Err(SpaceError::Duplicate)`

## Edge Cases

| Edge Case | Expected Behavior |
|-----------|-------------------|
| `SpaceId` with whitespace only (`"   "`) | Reject via `try_new` (whitespace is treated as empty after trim) |
| `Space` with `source_path = None` | Allowed; the space is virtual (e.g. ad-hoc docs collection) |
| `config` JSONB round-trips a non-empty object | All keys/values preserved through PG `JSONB` |
| Two spaces share the same `name` | Allowed (names are not unique; only `id` is) |
| `unregister` a space that holds nodes | Allowed; the space disappears but its nodes are not garbage-collected (PG row stays; `space_id` column is preserved) |

## Out of Scope

- Authorization / RBAC on spaces
- Cross-space node merging (covered by `merge-candidate-detection`)
- Space hierarchy / nesting
- Space templates or presets

## TDD RED Gate

Before implementation: (1) `space_id` tests for `try_new` empty rejection + `default()` constant; (2) `space_kind` JSON roundtrip for 3 variants; (3) `space::new` empty-name rejection + config default; (4) PG integration test that creates the `spaces` table and seeds the default row; (5) `SpaceRegistry` unit tests for register/get/list/unregister + duplicate. RED gate fails if any test compiles/passes before its module exists.

## Dependencies

- `NodeId` (generic-graph-model) — referenced by `FederatedNode` wrapper
- `multimodal` Cargo feature (already established in `cognicode-core`)

## Multimodal Feature Gate

All new types, the `SpaceRegistry`, and the `spaces` migration MUST be `#[cfg(feature = "multimodal")]`. With the feature disabled, no `space` symbol is exported and no migration runs. Build with `cargo build -p cognicode-core --no-default-features` MUST compile unchanged.
