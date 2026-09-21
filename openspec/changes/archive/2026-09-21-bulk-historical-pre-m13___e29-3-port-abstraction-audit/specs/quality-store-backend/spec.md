# Quality Store Backend Specification

> Capability: `quality-store-backend` · Change: `e29-3-port-abstraction-audit`
> Branch: `main` @ `10f27d59` · Strict TDD: ACTIVE · Test runner:
> `cargo test -p cognicode-ladybug` (the 10 new in-crate tests live
> alongside the lbug suite)

## Purpose

`LadybugStore` ships a complete implementation of
`cognicode_core::domain::ports::QualityStore` (10 methods) backed by
lbug tables for `issues`, `baselines`, `rules` per ADR-028 §Ladybug
Schema. Read methods degrade gracefully on an empty/missing database
(0 rows ⇒ empty `Vec` / zero count, no error); write methods surface
I/O failures as `QualityError::Store(...)`. Each method has a TDD
in-crate test (red → green).

## ADDED Requirements

### Requirement: `LadybugStore` implements all 10 `QualityStore` methods with real bodies

`LadybugStore` (in `crates/cognicode-ladybug/src/`) MUST provide an
`impl cognicode_core::domain::ports::QualityStore for LadybugStore`
block. No method body MUST be `unimplemented!()`, `todo!()`, panic,
or return a hard-coded `Err(QualityError::Store("stub"))` for the
read path. The 10 methods are exactly:

1. `issues_for_file(&self, file: &str) -> Result<Vec<QualityIssue>, QualityError>`
2. `issues_for_scope(&self, scope_prefix: &str) -> Result<Vec<QualityIssue>, QualityError>` — boundary-aware: `scope = "src"` MUST NOT match `src_extra.rs`
3. `issues_at_line(&self, file: &str, line: u32) -> Result<Vec<QualityIssue>, QualityError>`
4. `issue_by_id(&self, id: i64) -> Result<Option<QualityIssue>, QualityError>` — returns `Ok(None)` when the id is absent
5. `rule_summary(&self, rule_id: &str) -> Result<RuleSummary, QualityError>`
6. `quality_gate(&self, workspace_id: Option<&str>) -> Result<QualityGateSummary, QualityError>`
7. `open_issues_count(&self, workspace_id: Option<&str>) -> Result<usize, QualityError>`
8. `issues_for_workspace(&self, workspace_id: Option<&str>, filter: &IssueFilter) -> Result<Vec<QualityIssue>, QualityError>`
9. `insert_issues(&self, issues: &[NewIssue]) -> Result<UpsertSummary, QualityError>` — returns `inserted` and `updated` counts
10. `delete_issue(&self, workspace_id: &str, rule_id: &str, file_path: &str, line: u32) -> Result<bool, QualityError>` — returns `Ok(true)` when a row was deleted, `Ok(false)` when no match

#### Scenario: 10 method bodies exist with no stubs

- GIVEN `grep -rn "unimplemented!()\|todo!()" crates/cognicode-ladybug/src/quality_store.rs crates/cognicode-ladybug/src/store*.rs crates/cognicode-ladybug/src/ladybug*.rs` (the actual file path comes from the schema amendment; this scenario is satisfied for whichever file holds the `impl QualityStore for LadybugStore` block)
- WHEN the search runs
- THEN result count MUST be 0 inside the QualityStore impl block
- AND `impl cognicode_core::domain::ports::QualityStore for LadybugStore { ... }` MUST contain all 10 method bodies

[needs-multimodal] — not gated.

#### Scenario: 10 unit tests, one per method, all green (TDD red → green)

- GIVEN the lbug in-crate test module
- WHEN `cargo test -p cognicode-ladybug` runs
- THEN 10 unit tests MUST exist, each named after a method (e.g. `quality_store_issues_for_file`, `quality_store_issues_for_scope`, `quality_store_issues_at_line`, `quality_store_issue_by_id`, `quality_store_rule_summary`, `quality_store_quality_gate`, `quality_store_open_issues_count`, `quality_store_issues_for_workspace`, `quality_store_insert_issues`, `quality_store_delete_issue`)
- AND all 10 MUST pass (the red phase wrote each test first against the pre-change stub; the green phase replaces stubs with lbug-backed impls)

[needs-multimodal] — not gated.

#### Scenario: read methods degrade gracefully on an empty database

- GIVEN a freshly-created lbug database whose `issues`/`baselines`/`rules` tables are empty (post-`m_quality_v1` migration)
- WHEN any of the 8 read methods (1–8) is invoked
- THEN the result MUST be `Ok(<empty>)` (empty `Vec`, zero count, default `QualityGateSummary`) — NOT `Err(...)`

#### Scenario: boundary-aware scope prefix

- GIVEN the scope_prefix is `"src"` and a row exists in `issues` with `file_path = "src_extra.rs"`
- WHEN `issues_for_scope("src")` is invoked
- THEN the `src_extra.rs` row MUST NOT be in the returned `Vec`
- AND only rows whose `file_path` equals `"src"` exactly OR starts with `"src/"` MUST be returned

#### Scenario: write conflict surfaces as `UpsertSummary` (no duplicate row)

- GIVEN two `insert_issues` batches target the same `(workspace_id, rule_id, file_path, line)` natural key
- WHEN the second batch is inserted (no manual delete in between)
- THEN the second call MUST return `Ok(UpsertSummary { inserted: 0, updated: 1 })`
- AND the table MUST hold exactly one row for that key (no duplicate inserted)

#### Scenario: `delete_issue` returns the deletion flag, not an error

- GIVEN a row matching `(workspace_id, rule_id, file_path, line)` exists
- WHEN `delete_issue(...)` is invoked
- THEN it MUST return `Ok(true)`
- AND a subsequent call for the same key MUST return `Ok(false)` (idempotent semantics)

### Requirement: lbug tables for `issues`, `baselines`, `rules` exist with the ADR-028 schema

When `LadybugStore::open(path)` runs, it MUST register and apply a
migration (`m_quality_v1` or equivalent) that creates the three
tables. The schema MUST mirror the previous PG schema faithfully:

- `issues(id, workspace_id, rule_id, severity, category, file_path, line, message, status, created_at)` — natural key `(workspace_id, rule_id, file_path, line)`
- `baselines(workspace_id, rating, total_issues, blockers, criticals, debt_minutes, last_run)` — keyed by `workspace_id`
- `rules(rule_id, description, category)` — keyed by `rule_id`

#### Scenario: schema applies on store open and the suite passes the round-trip

- GIVEN a fresh lbug DB path that does not yet contain the three tables
- WHEN `LadybugStore::open(path)` is called
- THEN the three tables MUST be queryable via the `QualityStore` impl immediately afterwards
- AND `open_issues_count(None)` MUST return `Ok(0)` on the empty DB (graceful degradation)

[needs-multimodal] — not gated.
