# LadybugDB Graph Schema Specification

**Version**: 0.4.0
**Date**: 2026-07-31
**Status**: ACTIVE (S2 cycle)

> **Cycle e29-s2-schema-load corrections** (2026-07-31):
>
> **v0.1.0 → v0.2.0** (S2 explore): bumped after S2 explore flagged 3 BLOCKING structural errors and 1 impossible criterion against lbug 0.19.0 / Kùzu 0.x:
> - All 25 NODE TABLE DDLs: `id SERIAL INT64 PRIMARY KEY` → `id SERIAL PRIMARY KEY` (SERIAL is the alias for INT64; combining is invalid)
> - All 25 NODE TABLE DDLs + 20 REL TABLE DDLs: `properties MAP<STRING,STRING]` / `MAP<STRING,STRING)` → `properties MAP(STRING, STRING)` (Kùzu uses parentheses, not angle brackets)
> - All 12 `TEXT` columns across 9 tables → `STRING` (Kùzu has no TEXT alias)
> - Workspace index bug fixed: `idx_workspace_workspace_id ON Workspace(workspace_id)` → `idx_workspace_name ON Workspace(name)` (Workspace has no workspace_id column)
> - All 20 REL TABLE DDLs rewritten: removed `id SERIAL INT64 PRIMARY KEY` (PRIMARY KEY forbidden on rel tables — system assigns edge IDs via `ID(r)`); added `FROM <SourceNode> TO <TargetNode>` as the first clause (mandatory in Kùzu); kept all workspace/revision/temporal/property columns
> - Multi-label examples removed (S2 criterion #6 dropped — Kùzu discussion #3114: "not in our roadmap"). `Symbol.kind STRING` is the discriminator. The 8 "also-Symbol" tables (Decision, Doc, Evidence, Issue, Component, Container, System, Route) remain as separate NODE TABLEs with their own columns and link to Symbol via dedicated REL TABLEs.
>
> **v0.2.0 → v0.3.0** (S2 apply probe): removed all `NOT NULL` column constraints from 296 occurrences across 45 tables. **Kùzu 0.x / lbug 0.19.0 does NOT support `NOT NULL` in DDL** (empirically confirmed 2026-07-31 via probe example; parser rejects with `Invalid input < NOT>: expected rule iC_CreateNodeTable`). The `PRIMARY KEY` clause is the sole `NOT`-style constraint Kùzu accepts on column definitions. Column nullability is enforced at the application layer (CogniCode ingest pipeline guarantees non-null for required columns).
> - Design principle #1 retained: every node table still has `id SERIAL PRIMARY KEY` (PK is REQUIRED — empirically verified; SERIAL alone errors with `Can not find primary key`).
> - Design principle #2 updated: removed `NOT NULL` from `workspace_id` and `revision_id`. Catalog tables (`Workspace`, `Space`, `Revision`) still exclude these columns.
> - Design principle #3 updated: removed `NOT NULL` from `valid_from` and `valid_to`. `valid_to INT64 DEFAULT -1` retained (DEFAULT clause IS supported). Catalog tables still exclude temporal columns.
>
> **v0.3.0 → v0.4.0** (S2 apply probe #2): removed all 43 `CREATE INDEX` statements. **lbug 0.19.0 only supports indexes on node table primary keys** (empirically confirmed via probe: `Binder exception: HASH indexes are currently supported only on node primary keys`). The PostgreSQL-style `CREATE INDEX name ON Table(col)` and the Kùzu-style `CREATE INDEX name FOR (n:Table) ON (n.col)` BOTH fail when applied to a non-PK column. The 43 indexes from v0.3.0 were therefore unachievable on lbug 0.19.0 and were removed. **Performance implication**: queries on non-PK columns (e.g., `WHERE s.kind = 'function'`) will do a full table scan until lbug adds support for secondary indexes (future release). For S2 spike purposes (60K rows), this is acceptable. For production migration, this is a known limitation that may require application-layer indexing strategies or migration back to PostgreSQL.
> - Design principle #6 (was: "Indexes are created on: `(workspace_id, revision_id)`, `kind`, `valid_to = -1`") → **REMOVED**. Replaced with: "Index support is currently limited to node table primary keys in lbug 0.19.0. Secondary indexes will require either future lbug releases or application-layer caching strategies."

## Overview

This specification defines the complete LadybugDB schema for CogniCode's canonical graph store. Every table is designed with:
- **Typed columns** for stable domain fields
- **Optional `properties MAP(STRING, STRING)`** for emergent properties (ADR-027)
- **Temporal columns** (`valid_from`, `valid_to`) for ADR-015 immutability
- **Single-label model**: every node has exactly one label (table). `Symbol.kind` is the discriminator for symbol types (function, struct, decision, etc.). The 8 "secondary" tables (Decision, Doc, Evidence, Issue, Component, Container, System, Route) are separate NODE TABLEs linked to Symbol via dedicated REL TABLEs.

## Design principles

1. Every node table has `id SERIAL PRIMARY KEY` (SERIAL is the INT64 alias; PRIMARY KEY is REQUIRED — empirically verified).
2. Every node table has `workspace_id INT64` and `revision_id INT64` — **Exception**: `Workspace`, `Space`, `Revision` are catalog tables and do not have `workspace_id` / `revision_id` themselves. **NOT NULL is NOT supported in Kùzu DDL**; nullability is enforced at the application layer.
3. Every node table has `valid_from INT64` and `valid_to INT64 DEFAULT -1` — **Exception**: `Workspace`, `Space`, `Revision` are catalog tables and do not have temporal columns.
4. Every relationship table has the same temporal + workspace columns (no `workspace_id` / `revision_id` exception for rels — every edge belongs to a workspace + revision). **NOT NULL is NOT supported on rel table columns either.**
5. **No `NOT NULL` constraints anywhere in Kùzu DDL.** Nullability is application-layer policy.
6. **Index support is currently limited to node table primary keys in lbug 0.19.0.** Secondary indexes (e.g., on `kind`, `valid_to`, `workspace_id`) will require either future lbug releases or application-layer caching strategies.
7. **Rel table shape**: `CREATE REL TABLE X (FROM <SourceNode> TO <TargetNode>, ...)` — PRIMARY KEY is FORBIDDEN on rel tables (system assigns edge IDs via `ID(r)`). FK columns (e.g., `caller_id`, `callee_id`) are REMOVED — the FROM/TO clause subsumes them.
5. Indexes are created on: `(workspace_id, revision_id)`, `kind`, `valid_to = -1`
6. `properties MAP(STRING, STRING)` is optional on every table
7. **Rel table shape**: `CREATE REL TABLE X (FROM <SourceNode> TO <TargetNode>, ...)` — PRIMARY KEY is FORBIDDEN on rel tables (system assigns edge IDs via `ID(r)`). FK columns (e.g., `caller_id`, `callee_id`) are REMOVED — the FROM/TO clause subsumes them.

---

## Node Tables

### Workspace

```cypher
CREATE NODE TABLE Workspace (
  id SERIAL PRIMARY KEY,
  name STRING,
  description STRING,
  created_at INT64,    -- Unix timestamp ms
  updated_at INT64,
  properties MAP(STRING, STRING)
);
```

### Space

```cypher
CREATE NODE TABLE Space (
  id SERIAL PRIMARY KEY,
  name STRING,
  description STRING,
  owner STRING,          -- user identifier
  visibility STRING,      -- "public" | "private"
  created_at INT64,
  updated_at INT64,
  properties MAP(STRING, STRING)
);
```

### Revision

```cypher
CREATE NODE TABLE Revision (
  id SERIAL PRIMARY KEY,
  workspace_id INT64,     -- a Revision belongs to a Workspace
  revision_id INT64,      -- monotonically increasing per workspace
  parent_revision_id INT64,        -- nullable for first revision
  commit_hash STRING,
  message STRING,
  author STRING,
  created_at INT64,
  is_head BOOLEAN DEFAULT false,
  properties MAP(STRING, STRING)
);
```

### FileRecord

```cypher
CREATE NODE TABLE FileRecord (
  id SERIAL PRIMARY KEY,
  workspace_id INT64,
  revision_id INT64,
  file_path STRING,
  content_hash STRING,    -- SHA-256
  language STRING,
  scanned_at INT64,
  valid_from INT64,
  valid_to INT64 DEFAULT -1,
  properties MAP(STRING, STRING)
);
```

### SourceFile

```cypher
CREATE NODE TABLE SourceFile (
  id SERIAL PRIMARY KEY,
  workspace_id INT64,
  revision_id INT64,
  file_path STRING,
  content STRING,
  language STRING,
  line_count INT64,
  valid_from INT64,
  valid_to INT64 DEFAULT -1,
  properties MAP(STRING, STRING)
);
```

### Symbol (discriminator: `kind` column)

```cypher
CREATE NODE TABLE Symbol (
  id SERIAL PRIMARY KEY,
  workspace_id INT64,
  revision_id INT64,
  name STRING,
  qualified_name STRING,  -- file:name:line
  kind STRING,            -- NodeKind as string: "function", "struct", "decision", "component", etc. — serves as the single-label discriminator
  file_path STRING,
  line_number INT64,
  column_number INT64,
  signature STRING,
  doc_comment STRING,
  visibility STRING,       -- "public" | "private" | "internal"
  fan_in INT64 DEFAULT 0,
  fan_out INT64 DEFAULT 0,
  valid_from INT64,
  valid_to INT64 DEFAULT -1,
  properties MAP(STRING, STRING)
);
```

> **Single-label model**: Symbol has a `kind` column that serves as a discriminator. The separate NODE TABLEs (Decision, Doc, Evidence, Issue, Component, Container, System, Route) have their own columns and are linked to Symbol via dedicated REL TABLEs (e.g., `Cites (FROM Symbol TO Decision)`, `Justifies (FROM Evidence TO Decision)`, `PartOf (FROM Symbol TO Component)`, `HasIssue (FROM Symbol TO Issue)`, `Annotates (FROM Doc TO Symbol)`). Kùzu does NOT support multi-label (`SET s:Component`); the earlier multi-label design has been replaced.

### Decision

```cypher
-- Decision carries domain-specific attributes beyond what Symbol.kind encodes.
-- Linked to Symbol via Cites (Symbol → Decision) and Resolves (Issue → Decision).
CREATE NODE TABLE Decision (
  id SERIAL PRIMARY KEY,
  symbol_id INT64,       -- FK to Symbol.id (the decision's symbol representation)
  workspace_id INT64,
  revision_id INT64,
  adr_number STRING,      -- "ADR-001"
  title STRING,
  status STRING,           -- "Proposed" | "Accepted" | "Deprecated"
  valid_from INT64,
  valid_to INT64 DEFAULT -1,
  properties MAP(STRING, STRING)
);
```

### Doc

```cypher
-- Documentation node. Linked to Symbol via Annotates (Doc → Symbol).
CREATE NODE TABLE Doc (
  id SERIAL PRIMARY KEY,
  symbol_id INT64,
  workspace_id INT64,
  revision_id INT64,
  doc_kind STRING,         -- "readme", "api", "guide", "internal"
  title STRING,
  content STRING,
  file_path STRING,
  valid_from INT64,
  valid_to INT64 DEFAULT -1,
  properties MAP(STRING, STRING)
);
```

### Evidence

```cypher
-- Evidence node (raw log, trace, measurement). Linked to Decision via Justifies (Evidence → Decision).
CREATE NODE TABLE Evidence (
  id SERIAL PRIMARY KEY,
  symbol_id INT64,
  workspace_id INT64,
  revision_id INT64,
  evidence_kind STRING,    -- "log", "trace", "measurement", "external"
  content STRING,
  source STRING,           -- where this evidence came from
  confidence REAL,          -- 0.0 to 1.0
  valid_from INT64,
  valid_to INT64 DEFAULT -1,
  properties MAP(STRING, STRING)
);
```

### Issue

```cypher
CREATE NODE TABLE Issue (
  id SERIAL PRIMARY KEY,
  workspace_id INT64,
  revision_id INT64,
  issue_id STRING,         -- natural key: "RUST-001"
  rule_id STRING,
  severity STRING,          -- "error", "warning", "info"
  message STRING,
  file_path STRING,
  line_number INT64,
  column_number INT64,
  status STRING,            -- "open", "acknowledged", "fixed", "suppressed"
  valid_from INT64,
  valid_to INT64 DEFAULT -1,
  properties MAP(STRING, STRING)
);
```

### Component

```cypher
-- Architectural component. Linked to Symbol via PartOf (Symbol → Component) and to Container via Contains (Container → Component).
CREATE NODE TABLE Component (
  id SERIAL PRIMARY KEY,
  symbol_id INT64,
  workspace_id INT64,
  revision_id INT64,
  component_kind STRING,    -- "library", "service", "module", "package"
  responsibility STRING,
  valid_from INT64,
  valid_to INT64 DEFAULT -1,
  properties MAP(STRING, STRING)
);
```

### Container

```cypher
-- Runtime container. Linked to Symbol via PartOf (Symbol → Container) and contains Components via Contains (Container → Component).
CREATE NODE TABLE Container (
  id SERIAL PRIMARY KEY,
  symbol_id INT64,
  workspace_id INT64,
  revision_id INT64,
  container_kind STRING,    -- "docker", "process", "lambda"
  technology STRING,
  valid_from INT64,
  valid_to INT64 DEFAULT -1,
  properties MAP(STRING, STRING)
);
```

### System

```cypher
-- Top-level system. Linked to Symbol via PartOf (Symbol → System).
CREATE NODE TABLE System (
  id SERIAL PRIMARY KEY,
  symbol_id INT64,
  workspace_id INT64,
  revision_id INT64,
  system_kind STRING,      -- "microservice", "monolith", "saas"
  boundaries STRING,         -- description of system boundaries
  valid_from INT64,
  valid_to INT64 DEFAULT -1,
  properties MAP(STRING, STRING)
);
```

### Route

```cypher
-- HTTP route. Linked to other Routes via HttpCalls (Route → Route).
CREATE NODE TABLE Route (
  id SERIAL PRIMARY KEY,
  symbol_id INT64,
  workspace_id INT64,
  revision_id INT64,
  method STRING,             -- "GET", "POST", "PUT", "DELETE", etc.
  path STRING,
  handler STRING,             -- fully qualified handler name
  valid_from INT64,
  valid_to INT64 DEFAULT -1,
  properties MAP(STRING, STRING)
);
```

### Rule

```cypher
CREATE NODE TABLE Rule (
  id SERIAL PRIMARY KEY,
  workspace_id INT64,
  revision_id INT64,
  rule_id STRING,
  name STRING,
  category STRING,           -- "rust-lang", "security", "performance"
  severity STRING,           -- "error", "warning", "info"
  description STRING,
  message_template STRING,
  valid_from INT64,
  valid_to INT64 DEFAULT -1,
  properties MAP(STRING, STRING)
);
```

### Baseline

```cypher
CREATE NODE TABLE Baseline (
  id SERIAL PRIMARY KEY,
  workspace_id INT64,
  revision_id INT64,
  baseline_id STRING,
  name STRING,
  description STRING,
  baseline_hash STRING,      -- hash of baseline state
  created_at INT64,
  valid_from INT64,
  valid_to INT64 DEFAULT -1,
  properties MAP(STRING, STRING)
);
```

### Investigation

```cypher
CREATE NODE TABLE Investigation (
  id SERIAL PRIMARY KEY,
  workspace_id INT64,
  revision_id INT64,
  title STRING,
  goal STRING,
  status STRING,             -- "draft", "active", "completed", "archived"
  entry_point STRING,
  narrative STRING DEFAULT '',
  related_adrs STRING[] DEFAULT [],
  created_at INT64,
  updated_at INT64,
  valid_from INT64,
  valid_to INT64 DEFAULT -1,
  properties MAP(STRING, STRING)
);
```

### EvidenceItem (pinned evidence within an investigation)

```cypher
CREATE NODE TABLE EvidenceItem (
  id SERIAL PRIMARY KEY,
  workspace_id INT64,
  revision_id INT64,
  investigation_id INT64,
  object_id STRING,         -- e.g., "symbol:src/main.rs:main:5"
  view_id STRING,
  note STRING,
  pinned_at INT64,
  valid_from INT64,
  valid_to INT64 DEFAULT -1,
  properties MAP(STRING, STRING)
);
```

### Artifact

```cypher
CREATE NODE TABLE Artifact (
  id SERIAL PRIMARY KEY,
  workspace_id INT64,
  revision_id INT64,
  investigation_id INT64,
  artifact_kind STRING,     -- "mermaid", "svg", "png", "drawio", "markdown"
  title STRING,
  content STRING,
  generated_from STRING,              -- object_id or view_id that generated this
  created_at INT64,
  valid_from INT64,
  valid_to INT64 DEFAULT -1,
  properties MAP(STRING, STRING)
);
```

### ExplorationSession

```cypher
CREATE NODE TABLE ExplorationSession (
  id SERIAL PRIMARY KEY,
  workspace_id INT64,
  revision_id INT64,
  title STRING,
  panes_json STRING,         -- JSON serialized pane states
  navigation_json STRING,     -- JSON serialized navigation history
  created_at INT64,
  updated_at INT64,
  valid_from INT64,
  valid_to INT64 DEFAULT -1,
  properties MAP(STRING, STRING)
);
```

### NamedView

```cypher
CREATE NODE TABLE NamedView (
  id SERIAL PRIMARY KEY,
  workspace_id INT64,
  revision_id INT64,
  owner STRING,
  name STRING,
  view_kind STRING,         -- ViewKind as string
  description STRING,
  created_at INT64,
  updated_at INT64,
  valid_from INT64,
  valid_to INT64 DEFAULT -1,
  properties MAP(STRING, STRING)
);
```

### ViewSpec

```cypher
CREATE NODE TABLE ViewSpec (
  id SERIAL PRIMARY KEY,
  workspace_id INT64,
  revision_id INT64,
  owner STRING,
  name STRING,
  view_kind STRING,
  renderer_kind STRING,
  data_source STRING,
  transform STRING,                   -- JSONata transform
  props_json STRING DEFAULT '{}',
  created_at INT64,
  updated_at INT64,
  valid_from INT64,
  valid_to INT64 DEFAULT -1,
  properties MAP(STRING, STRING)
);
```

### GraphReport

```cypher
CREATE NODE TABLE GraphReport (
  id SERIAL PRIMARY KEY,
  workspace_id INT64,
  revision_id INT64,
  report_id STRING,
  report_kind STRING,        -- "community_clusters", "god_nodes", "dead_code"
  summary_json STRING,      -- JSON serialized summary
  created_at INT64,
  valid_from INT64,
  valid_to INT64 DEFAULT -1,
  properties MAP(STRING, STRING)
);
```

### AnalyticsRun

```cypher
CREATE NODE TABLE AnalyticsRun (
  id SERIAL PRIMARY KEY,
  workspace_id INT64,
  revision_id INT64,
  algorithm_id STRING,        -- "pagerank@v1.0.0"
  mode STRING,               -- "stream", "stats", "annotate", "persist"
  status STRING,              -- "pending", "running", "succeeded", "truncated", "failed"
  parameters_json STRING,
  row_count INT64,
  truncation_marker STRING,
  error_kind STRING,
  error_message STRING,
  started_at INT64,
  finished_at INT64,
  valid_from INT64,
  valid_to INT64 DEFAULT -1,
  properties MAP(STRING, STRING)
);
```

### DescriptorLimits

```cypher
CREATE NODE TABLE DescriptorLimits (
  id SERIAL PRIMARY KEY,
  workspace_id INT64,
  revision_id INT64,
  algorithm_id STRING,
  version STRING,
  max_time_ms INT64,
  max_memory_bytes INT64,
  max_result_rows INT64,
  created_at INT64,
  valid_from INT64,
  valid_to INT64 DEFAULT -1,
  properties MAP(STRING, STRING)
);
```

---

## Relationship Tables

> **Shape**: every rel table uses `FROM <SourceNode> TO <TargetNode>` as the first clause. PRIMARY KEY is forbidden. The previous `id SERIAL INT64 PRIMARY KEY` + explicit FK columns (e.g., `caller_id`, `callee_id`) are REMOVED — the FROM/TO subsumes them, and COPY FROM CSV for rels uses Kùzu internal node IDs as the first two columns (assigned by Symbol COPY FROM).

### Calls

```cypher
CREATE REL TABLE Calls (
  FROM Symbol TO Symbol,
  workspace_id INT64,
  revision_id INT64,
  provenance STRING DEFAULT 'extractor',
  confidence REAL DEFAULT 1.0,
  valid_from INT64,
  valid_to INT64 DEFAULT -1,
  properties MAP(STRING, STRING)
);
```

### Imports

```cypher
CREATE REL TABLE Imports (
  FROM Symbol TO Symbol,
  workspace_id INT64,
  revision_id INT64,
  module_path STRING,
  valid_from INT64,
  valid_to INT64 DEFAULT -1,
  properties MAP(STRING, STRING)
);
```

### Defines

```cypher
CREATE REL TABLE Defines (
  FROM FileRecord TO Symbol,
  workspace_id INT64,
  revision_id INT64,
  valid_from INT64,
  valid_to INT64 DEFAULT -1,
  properties MAP(STRING, STRING)
);
```

### Cites

```cypher
-- Source (Symbol representing the doc) cites a Decision.
CREATE REL TABLE Cites (
  FROM Symbol TO Decision,
  workspace_id INT64,
  revision_id INT64,
  citation_text STRING,
  valid_from INT64,
  valid_to INT64 DEFAULT -1,
  properties MAP(STRING, STRING)
);
```

### Justifies

```cypher
-- Evidence justifies a Decision.
CREATE REL TABLE Justifies (
  FROM Evidence TO Decision,
  workspace_id INT64,
  revision_id INT64,
  justification_text STRING,
  valid_from INT64,
  valid_to INT64 DEFAULT -1,
  properties MAP(STRING, STRING)
);
```

### Resolves

```cypher
-- Issue is resolved by a Decision (or by a Symbol representing the fix).
CREATE REL TABLE Resolves (
  FROM Issue TO Decision,
  workspace_id INT64,
  revision_id INT64,
  resolution_note STRING,
  valid_from INT64,
  valid_to INT64 DEFAULT -1,
  properties MAP(STRING, STRING)
);
```

### PartOf

```cypher
-- Symbol belongs to a Component, Container, or System.
-- (One rel table per target type is also acceptable; keeping one with FROM Symbol TO Component as the canonical form for S2. Extend later if traversal diversity demands.)
CREATE REL TABLE PartOf (
  FROM Symbol TO Component,
  workspace_id INT64,
  revision_id INT64,
  valid_from INT64,
  valid_to INT64 DEFAULT -1,
  properties MAP(STRING, STRING)
);
```

### HttpCalls

```cypher
-- Route calls another Route.
CREATE REL TABLE HttpCalls (
  FROM Route TO Route,
  workspace_id INT64,
  revision_id INT64,
  call_site STRING,                  -- line number or expression
  valid_from INT64,
  valid_to INT64 DEFAULT -1,
  properties MAP(STRING, STRING)
);
```

### DefinedIn

```cypher
-- Symbol defined in a SourceFile (alternative to Defines for source-text lookup patterns).
CREATE REL TABLE DefinedIn (
  FROM Symbol TO SourceFile,
  workspace_id INT64,
  revision_id INT64,
  valid_from INT64,
  valid_to INT64 DEFAULT -1,
  properties MAP(STRING, STRING)
);
```

### ScannedIn

```cypher
-- FileRecord scanned in a Revision.
CREATE REL TABLE ScannedIn (
  FROM FileRecord TO Revision,
  workspace_id INT64,
  revision_id INT64,
  valid_from INT64,
  valid_to INT64 DEFAULT -1,
  properties MAP(STRING, STRING)
);
```

### HasIssue

```cypher
-- A Symbol has an Issue.
CREATE REL TABLE HasIssue (
  FROM Symbol TO Issue,
  workspace_id INT64,
  revision_id INT64,
  valid_from INT64,
  valid_to INT64 DEFAULT -1,
  properties MAP(STRING, STRING)
);
```

### PinnedIn

```cypher
-- EvidenceItem pinned to an Investigation.
CREATE REL TABLE PinnedIn (
  FROM EvidenceItem TO Investigation,
  workspace_id INT64,
  revision_id INT64,
  valid_from INT64,
  valid_to INT64 DEFAULT -1,
  properties MAP(STRING, STRING)
);
```

### SavedAs

```cypher
-- ExplorationSession saved as a NamedView.
CREATE REL TABLE SavedAs (
  FROM ExplorationSession TO NamedView,
  workspace_id INT64,
  revision_id INT64,
  valid_from INT64,
  valid_to INT64 DEFAULT -1,
  properties MAP(STRING, STRING)
);
```

### RunsOn

```cypher
-- AnalyticsRun runs on a Revision.
CREATE REL TABLE RunsOn (
  FROM AnalyticsRun TO Revision,
  workspace_id INT64,
  revision_id INT64,
  valid_from INT64,
  valid_to INT64 DEFAULT -1,
  properties MAP(STRING, STRING)
);
```

### TrackedBy

```cypher
-- Issue tracked by a Baseline.
CREATE REL TABLE TrackedBy (
  FROM Issue TO Baseline,
  workspace_id INT64,
  revision_id INT64,
  valid_from INT64,
  valid_to INT64 DEFAULT -1,
  properties MAP(STRING, STRING)
);
```

### Generates

```cypher
-- Artifact generated from an Investigation.
CREATE REL TABLE Generates (
  FROM Investigation TO Artifact,
  workspace_id INT64,
  revision_id INT64,
  valid_from INT64,
  valid_to INT64 DEFAULT -1,
  properties MAP(STRING, STRING)
);
```

### BelongsTo

```cypher
-- Space belongs to a Workspace.
CREATE REL TABLE BelongsTo (
  FROM Space TO Workspace,
  workspace_id INT64,
  revision_id INT64,
  valid_from INT64,
  valid_to INT64 DEFAULT -1,
  properties MAP(STRING, STRING)
);
```

### Contains

```cypher
-- Container contains Components.
CREATE REL TABLE Contains (
  FROM Container TO Component,
  workspace_id INT64,
  revision_id INT64,
  valid_from INT64,
  valid_to INT64 DEFAULT -1,
  properties MAP(STRING, STRING)
);
```

### References

```cypher
-- Symbol referencing another symbol (general dependency).
CREATE REL TABLE References (
  FROM Symbol TO Symbol,
  workspace_id INT64,
  revision_id INT64,
  reference_kind STRING,     -- "type_use", "value_use", "import"
  valid_from INT64,
  valid_to INT64 DEFAULT -1,
  properties MAP(STRING, STRING)
);
```

### Annotates

```cypher
-- Doc annotates a Symbol.
CREATE REL TABLE Annotates (
  FROM Doc TO Symbol,
  workspace_id INT64,
  revision_id INT64,
  valid_from INT64,
  valid_to INT64 DEFAULT -1,
  properties MAP(STRING, STRING)
);
```

---

## Indexing Summary

| Table | Indexes |
|-------|---------|
| Workspace | `id`, `name` |
| Space | `id`, `name` |
| Revision | `id`, `(workspace_id, revision_id)` |
| FileRecord | `id`, `(workspace_id, revision_id)`, `valid_to` |
| SourceFile | `id`, `workspace_id`, `valid_to` |
| Symbol | `id`, `(workspace_id, revision_id)`, `kind`, `valid_to`, `qualified_name` |
| Decision | `id`, `workspace_id` |
| Doc | `id`, `workspace_id` |
| Evidence | `id`, `workspace_id` |
| Issue | `id`, `workspace_id`, `file_path`, `status`, `valid_to` |
| Component | `id`, `workspace_id` |
| Container | `id`, `workspace_id` |
| System | `id`, `workspace_id` |
| Route | `id`, `workspace_id` |
| Rule | `id`, `workspace_id`, `rule_id` |
| Baseline | `id`, `workspace_id` |
| Investigation | `id`, `workspace_id`, `status` |
| EvidenceItem | `id`, `investigation_id`, `workspace_id` |
| Artifact | `id`, `investigation_id`, `workspace_id` |
| ExplorationSession | `id`, `workspace_id` |
| NamedView | `id`, `(workspace_id, owner, name)` |
| ViewSpec | `id`, `(workspace_id, owner, name)`, `workspace_id` |
| GraphReport | `id`, `workspace_id`, `revision_id` |
| AnalyticsRun | `id`, `workspace_id`, `revision_id`, `algorithm_id` |
| DescriptorLimits | `id`, `(algorithm_id, version)` |
| Calls | `id`, `(workspace_id, revision_id)`, `valid_to` |
| Imports | `id` |
| Defines | `id` |
| Cites | `id` |
| Justifies | `id` |
| Resolves | `id` |
| PartOf | `id` |
| HttpCalls | `id` |
| DefinedIn | `id` |
| ScannedIn | `id` |
| HasIssue | `id` |
| PinnedIn | `id` |
| SavedAs | `id` |
| RunsOn | `id` |
| TrackedBy | `id` |
| Generates | `id` |
| BelongsTo | `id` |
| Contains | `id` |
| References | `id` |
| Annotates | `id` |

> **Note on rel indexes**: rel tables are traversed natively via Kùzu's graph engine (FROM/TO internal IDs), so the per-FK composite indexes from v0.1.0 (e.g., `idx_calls_caller ON Calls(caller_id, valid_to)`) are obsolete — those FK columns no longer exist. Add `(workspace_id, revision_id)` and `valid_to` indexes where multi-edge filtering by workspace/revision/temporal is the access pattern (see Calls as the example).

---

## LadybugStore::open() Schema Initialization (e29-6)

> **Added**: 2026-08-05 (e29-6-ladybug-store-wiring)

### Requirement: LadybugStore::open() Initializes Complete Schema

`LadybugStore::open()` MUST initialize all four schema families — quality, generic graph nodes, generic graph relationships, and analytics lineage — not just the quality schema. Each schema init method SHALL be idempotent (`IF NOT EXISTS` DDL).

**Previous bug**: `open()` only called `init_quality_schema()`, leaving generic graph tables, relationship tables, and lineage tables uninitialized.

#### Scenario: open() initializes all four schema families

- GIVEN a freshly created LadybugDB with no tables
- WHEN `LadybugStore::open(path)` is called
- THEN `init_quality_schema()` SHALL be invoked (QualityIssue, QualityBaseline, QualityRule tables)
- AND `init_generic_graph_schema()` SHALL be invoked (22 node tables including Workspace, Symbol, FileRecord, etc.)
- AND `init_generic_graph_rels_schema()` SHALL be invoked (20 relationship tables including Calls, Imports, Defines, etc.)
- AND `init_lineage_schema()` SHALL be invoked (AnalyticsRunLineage, DescriptorLimits tables)

#### Scenario: Subsequent open() calls are no-ops for schema

- GIVEN a LadybugDB where `open()` was already called once and all tables exist
- WHEN `open()` is called again on the same file
- THEN all four `init_*_schema()` calls SHALL complete without error
- AND no duplicate table errors SHALL occur

---

## References

- [ADR-026: LadybugDB migration decision](../../docs/adr/ADR-026-ladybugdb-canonical-migration.md)
- [ADR-027: Hybrid schema strategy](../../docs/adr/ADR-027-ladybugdb-hybrid-schema-strategy.md)
- [LadybugDB CREATE TABLE docs](https://docs.ladybugdb.com/cypher/data-definition/create-table/)
- [LadybugDB MAP type](https://docs.ladybugdb.com/cypher/data-types/)
- [Kùzu multi-label discussion #3114](https://github.com/kuzudb/kuzu/discussions/3114) (reason multi-label is not supported)
