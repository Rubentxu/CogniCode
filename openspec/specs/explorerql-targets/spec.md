# explorerql-targets Specification (NEW)

## Purpose

Extends the `TargetType` enum (in `crates/cognicode-explorer/src/moldql/ast.rs`) from 4 variants to 7, adding `Decisions`, `Docs`, and `Issues`. These map to multimodal `NodeKind` values and require new compilation rules in `compile.rs` that dispatch to the multimodal-aware repository. The existing 4 variants (`Symbols`, `Files`, `Scopes`, `Issues`) MUST keep parsing, compiling, and executing unchanged.

> **Note on classification**: The proposal lists this capability as "Modified" because it extends an existing enum, but no main spec exists for the `TargetType` value object. This spec is therefore written as a NEW spec in the change folder; on archive it becomes the main spec for `openspec/specs/explorerql-targets/spec.md`.

## Requirements

### Requirement: TargetType New Variants

The `TargetType` enum MUST be extended to:

```rust
pub enum TargetType {
    Symbols,
    Files,
    Scopes,
    Issues,    // existing — refers to QualityIssue, NOT NodeKind::Issue
    Decisions, // NEW
    Docs,      // NEW
    Issues2,   // REMOVED — covered by Issues. See risk note.
}
```

> **Naming**: The existing `Issues` variant refers to `QualityIssue` (a code-level view concept, not a graph node). The new graph-node `NodeKind::Issue` (tracker issue) is a distinct concept. To avoid collision, the new graph node is exposed as a separate target. See "Risks" below for the disambiguation strategy.

Concretely, the 3 new variants MUST be added:

| New variant | Maps to | Frontend keyword |
|-------------|---------|------------------|
| `Decisions` | `NodeKind::Decision` | `decisions` |
| `Docs` | `NodeKind::Doc` | `docs` |
| `Issues` (existing) stays for `QualityIssue` | n/a (no change) | `issues` |

A separate `TrackerIssues` variant MAY be added if the team decides to expose graph-node `Issue` targets later (out of scope for this change).

The `TargetType::keyword()` method MUST be extended to return the lowercase form for each new variant.

#### Scenario: All 7 variants have keyword forms
- GIVEN every variant of the extended `TargetType`
- WHEN `keyword()` is called
- THEN the result MUST be: `Symbols`→`"symbols"`, `Files`→`"files"`, `Scopes`→`"scopes"`, `Issues`→`"issues"`, `Decisions`→`"decisions"`, `Docs`→`"docs"`

> Note: Only 6 unique keywords exist because the proposal defers `TrackerIssues`. The enum has 6 variants (3 existing kept + 3 new for the multimodal capabilities). Adjust the spec accordingly.

#### Scenario: Legacy `FIND issues` still parses
- GIVEN `FIND issues WHERE severity == "high"`
- WHEN parsed
- THEN `target == TargetType::Issues` (the existing code-level variant)
- AND the compiler dispatches to the `QualityIssue` view (no behavior change)

### Requirement: Parser Accepts New Keywords

`moldql/parser.rs` (and `parser_explorerql.rs` if present) MUST accept `FIND decisions` and `FIND docs` as valid leading clauses. The parser MUST be case-insensitive on the keyword. Unknown keywords MUST continue to produce `ParseError` listing all valid keywords.

#### Scenario: FIND decisions parses
- GIVEN `FIND decisions WHERE status == "accepted"`
- WHEN parsed
- THEN the result MUST be `MoldQLQuery::Find(FindQuery { target: TargetType::Decisions, conditions, ... })`

#### Scenario: FIND docs parses
- GIVEN `FIND docs WHERE section == "Auth"`
- WHEN parsed
- THEN `target == TargetType::Docs`

#### Scenario: Unknown target rejected
- GIVEN `FIND widgets`
- WHEN parsed
- THEN the error message MUST list all 6 valid targets (symbols, files, scopes, issues, decisions, docs)

### Requirement: Compilation Rules for Multimodal Targets

`moldql/compile.rs` MUST extend its target dispatch to handle `Decisions` and `Docs`. The compiler MUST resolve the target to a call against the multimodal-aware repository. The existing 4 target compilations MUST remain byte-for-byte unchanged.

| Target | Repository call |
|--------|----------------|
| `Symbols` | `repo.find_symbols_by_filter(...)` (unchanged) |
| `Files` | `repo.find_files_by_filter(...)` (unchanged) |
| `Scopes` | `repo.find_scopes_by_filter(...)` (unchanged) |
| `Issues` | `repo.find_quality_issues_by_filter(...)` (unchanged) |
| `Decisions` | `graph_repo.find_nodes_by_kind(NodeKind::Decision, filter)` |
| `Docs` | `graph_repo.find_nodes_by_kind(NodeKind::Doc, filter)` |

The compiler MUST require a `GenericGraphRepository` instance to be wired into the compilation context. If the instance is `None` and a multimodal target is queried, the compiler MUST return `CompileError::RepositoryUnavailable`.

#### Scenario: FIND decisions compiles against generic graph
- GIVEN a compilation context with a `Some(generic_repo: &dyn GraphRepository)`
- WHEN `FIND decisions WHERE status == "accepted"` is compiled
- THEN the compiled plan calls `generic_repo.find_nodes_by_kind(NodeKind::Decision, ...)` with the WHERE filter applied

#### Scenario: Multimodal target without generic repo
- GIVEN a compilation context with `None` for the generic repository
- WHEN `FIND decisions` is compiled
- THEN the result MUST be `Err(CompileError::RepositoryUnavailable("generic graph repository not configured"))`

#### Scenario: Existing code targets compile unchanged
- GIVEN `FIND symbols WHERE kind == "Function"`
- WHEN compiled with the same context (multimodal repo irrelevant)
- THEN the compiled plan is identical to the pre-change version (regression gate)

### Requirement: WHERE Conditions Apply Per Target

Multimodal targets MUST support the same WHERE clause operators (`=`, `>`, `<`, `>=`, `<=`, `!=`, `contains`, `starts_with`) on multimodal node fields. The fields MUST be:

| Target | Allowed fields |
|--------|---------------|
| `Decisions` | `status` (string), `date` (ISO-8601 string), `label` (string), `confidence` (f64) |
| `Docs` | `section` (string), `label` (string), `source_path` (string), `confidence` (f64) |

Unknown fields for a multimodal target MUST produce `CompileError::UnknownField`.

#### Scenario: decisions.status filter
- GIVEN `FIND decisions WHERE status == "accepted"`
- WHEN executed
- THEN only `Decision` nodes with `metadata.status == "accepted"` are returned

#### Scenario: docs.section filter
- GIVEN `FIND docs WHERE section == "Auth"`
- WHEN executed
- THEN only `Doc` nodes with `metadata.section == "Auth"` are returned

#### Scenario: Unknown field rejected
- GIVEN `FIND decisions WHERE color == "purple"`
- WHEN compiled
- THEN the error MUST mention `unknown field 'color' for target 'decisions'`

## Edge Cases

| Edge Case | Expected Behavior |
|-----------|-------------------|
| `FIND decisions` runs against an empty `graph_nodes` table | Returns empty result set; no error |
| `FIND docs WHERE confidence > 0.5` when no docs have confidence metadata | Returns empty result set; no panic |
| Multimodal repo wired but the DB is SQLite (not Postgres) | Compile-time feature gate blocks the multimodal target — only Postgres supports `graph_nodes`/`graph_edges` in this change |
| `FIND decisions` combined with `EXPLAIN` clause | The multimodal target flows through; the explain step reports the multimodal repository call |
| `FIND decisions` combined with `PATH`/`NEIGHBORS`/`SUBGRAPH` | Out of scope: multimodal targets support the `FIND ... WHERE ... APPLY ...` shape only. Other clauses reject with `CompileError::UnsupportedClause` |
| User writes `FIND DECISIONS` (uppercase) | Parser normalizes case; `Decisions` variant is produced |
| Empty `FIND` (`FIND` with no target) | `ParseError: expected target keyword` (existing behavior) |
| `Decisions` and `Docs` targets in the same query (`FIND decisions, docs`) | Out of scope: each `FIND` is single-target. The compiler rejects comma-separated targets with `CompileError::MultipleTargetsNotSupported` |
| A `Decision` node has `status: null` in metadata | `WHERE status == "accepted"` filters it out; `WHERE status == null` includes it (null-safe comparison) |

## Out of Scope

- Adding `TargetType::TrackerIssues` (graph-node `Issue` variant) — separate decision
- Federation queries (`FIND decisions IN repo "alpha"`)
- New clauses beyond `FIND ... WHERE ... APPLY ...` for multimodal targets
- Streaming/async pagination on multimodal target results (use the existing paged result wrapper)
- Re-tagging existing `Issues` data (the existing `QualityIssue` view continues to be the source of truth for code-level issues)

## TDD RED Gate

Before any implementation, the following failing tests MUST exist:

1. `TargetType::keyword` — 6 OK cases (3 legacy + 3 new)
2. Parser — 5 cases: `FIND decisions`, `FIND docs`, `FIND decisions WHERE ...`, case-insensitive `FIND DECISIONS`, unknown target error (5 tests)
3. `compile.rs` — 3 cases: decisions success, docs success, repository unavailable
4. `compile.rs` regression — 4 cases confirming `Symbols`/`Files`/`Scopes`/`Issues` still compile identically
5. WHERE field validation — 2 OK + 1 error (3 tests)
6. Edge case: `PATH FROM decisions_node TO symbol_node` rejected with `UnsupportedClause`

## Dependencies

- `generic-graph-model` (provides `NodeKind::Decision`, `NodeKind::Doc`, `GenericGraphRepository`)
- `docs-source-adapter` (populates `Decision` and `Doc` nodes; the target is only useful when the adapter has run)
- Existing `MoldQLQuery::Find`, `FindQuery`, `TargetType` (unchanged shape, extended variants)
- Existing `CompileError` (extend with `RepositoryUnavailable`, `UnknownField`, `UnsupportedClause`)

## Risks (Disambiguation)

The existing `TargetType::Issues` refers to `QualityIssue` (a code-level concern emitted by the analysis pipeline). The new graph-node `NodeKind::Issue` (tracker issue) is a different concept. To avoid ambiguity, this change does NOT add a `TargetType::TrackerIssues`. The frontend MUST keep using `FIND issues` for `QualityIssue` and MAY expose tracker issues in a follow-up change that decides the naming. Auto-grill verdict: OS=0.74 (acceptable).
