# Edge Provenance and Confidence Specification

## Purpose

Every `CallGraph` edge SHALL carry `Provenance` and `confidence` metadata so consumers distinguish AST-extracted edges from heuristic inferences. Out of scope: PostgreSQL, new node kinds, new edge kinds, frontend, MCP envelope, query language.

## Requirements

### Requirement: Provenance Enum

`Provenance` MUST be a closed three-variant enum: `Extracted`, `Inferred`, `Ambiguous`. It MUST derive `Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default`. `Extracted` SHALL be the default. Adding a variant MUST bump the bincode version.

#### Scenario: Default and closure

- GIVEN `Provenance::default()` and the closed enum
- THEN the value MUST equal `Provenance::Extracted`
- AND no other variants may exist

### Requirement: Edge Confidence Field

Every edge MUST carry `confidence: f64` in `[0.0, 1.0]`. The system MUST reject `NaN`, `±inf`, and any value outside the interval.

#### Scenario: Range and rejection

- GIVEN a new edge
- WHEN `confidence = 0.85` is assigned it MUST roundtrip exactly
- AND WHEN `confidence ∈ {1.2, -0.1, NaN, inf}` is supplied the system MUST return a domain error and MUST NOT insert the edge

### Requirement: Confidence Rule Assignment Semantics

A `confidence_rules` service MUST assign `(Provenance, confidence)` per extraction context: tree-sitter direct call → `(Extracted, 1.0)`; heuristic resolver with score `s` → `(Inferred, s)` where `s ∈ [0.5, 0.9]`; unresolved target → `(Ambiguous, c)` where `c ≤ 0.5`. The service is the sole sanctioned assignment path.

#### Scenario: Each extractor maps to a rule

- GIVEN edges from tree-sitter, heuristic (score 0.7), and unresolved extractors
- WHEN the service is called for each
- THEN results MUST be `(Extracted, 1.0)`, `(Inferred, 0.7)`, and `(Ambiguous, c)` with `c ≤ 0.5` respectively

### Requirement: Backward-Compatible Graph Persistence Upgrade

Every persisted blob MUST start with magic `b"CCG1"` (4 bytes) then format version `u8`. `v1` is legacy; `v2` is new. A `GraphStore` MUST reject mismatched magic with `StoreError::Corrupted` and MUST read `v1` payloads by converting to in-memory `v2` edges with `(Extracted, 1.0)` defaults.

#### Scenario: Roundtrip, legacy load, magic rejection

- GIVEN a v2 graph, a v1 legacy blob, and a blob with bad magic
- WHEN `save_graph`/`load_graph` runs for v2, `load_graph` runs for v1 and bad magic
- THEN v2 roundtrips preserving metadata, v1 loads with `(Extracted, 1.0)` defaults, and bad magic returns `StoreError::Corrupted`

### Requirement: SQLite Schema Evolution and Idempotence

`call_edges` MUST be extended with `provenance TEXT NOT NULL DEFAULT 'Extracted'` and `confidence REAL NOT NULL DEFAULT 1.0`. The migration MUST use `ALTER TABLE ADD COLUMN` guarded by `PRAGMA table_info` or `user_version` so `initialize_schema` is idempotent. `populate_edges` MUST populate the new columns.

#### Scenario: Fresh schema, idempotence, legacy defaults

- GIVEN an empty SQLite file
- WHEN `initialize_schema` runs
- THEN `call_edges` MUST contain `provenance` and `confidence` with the defaults
- AND re-running MUST NOT error and MUST preserve existing rows
- AND legacy v1 rows MUST receive `(Extracted, 1.0)`

### Requirement: Non-Breaking API Behavior

All pre-existing public methods of `CallGraph` and `GraphStore` MUST remain callable with the same signatures. `add_dependency(source, target, dep_type)` MUST remain and MUST route through `confidence_rules` with a default extractor. `add_dependency_with_provenance(...)` and `edges_with_metadata()` SHALL be added without removing existing ones.

#### Scenario: Legacy and new APIs coexist

- GIVEN a caller invoking `graph.add_dependency(&a, &b, DependencyType::Calls)`
- WHEN the call completes
- THEN an edge MUST be created with metadata assigned by `confidence_rules`
- AND the existing test suite MUST pass without modification

### Requirement: Testability

The system MUST provide: golden tests pinning `(Provenance, f64)` for every extractor × score; bincode roundtrip tests for v1→v2 and v2→v2; SQLite migration tests for columns and idempotence; invariant tests asserting no edge has `confidence` outside `[0.0, 1.0]`.

#### Scenario: Golden and invariant tests

- GIVEN a frozen fixture `(extractor, score) -> (Provenance, f64)`
- WHEN the service is called it MUST match the fixture bit-for-bit (f64 exact)
- AND a post-condition on every `CallGraph` test MUST assert `confidence ∈ [0.0, 1.0]` and MUST fail loudly on violation
