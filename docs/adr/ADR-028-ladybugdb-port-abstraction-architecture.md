# ADR-028: Port abstraction architecture for LadybugDB migration

**Status**: ACCEPTED (with reconciled Phase 0 surface per the audit findings in `sddk/audit-e29-0/code-vs-design-audit.md`, 2026-08-01)
**Date**: 2026-07-30 (original) / 2026-08-01 (reconciled per Phase 0 trunk merge `f179a116`)
**Deciders**: User, OpenCode orchestrator

## Context

The LadybugDB migration requires that every PostgreSQL access be abstracted behind a domain port. Today, ~30 call sites directly use `sqlx::query` or `PostgresRepository` concrete methods without going through traits. Without port abstraction, migrating to LadybugDB would require rewriting 30+ call sites.

The existing hexagonal architecture has:
- `domain/` — domain logic, value objects, aggregates
- `domain/ports/` — driven port traits (GraphRepository, GraphError, NodePropertyReader)
- `domain/traits/` — additional port traits (Repository, GraphQueryPort, GraphStore, SearchProvider, etc.)
- `infrastructure/` — concrete adapters (PostgresRepository, PgGraphExecutor)
- `application/` — use cases depending on ports

### Current port inventory

**Ports that exist and are clean (no PG leaks):**

| Port | Location | Methods | Status |
|------|----------|---------|--------|
| `Repository` | `domain/traits/repository.rs` | `find_symbol_by_qualified_name`, `count_symbols`, `find_edges_by_caller`, `find_edges_by_callee`, `count_edges`, `load_call_graph_pinned` | ⚠️ Doc leak: `UniqueViolation` mentions "PostgreSQL SQLSTATE 23505" |
| `GraphRepository` | `domain/ports/graph_repository.rs` | `search`, `find_nodes_by_kind`, `get_node`, `find_outgoing_edges`, `edges_by_kind`, `rationale_subgraph` | ⚠️ Doc leak: `raw_rank` mentions "ts_rank_cd value" |
| `GraphQueryPort` | `domain/traits/graph_query_port.rs` | `callers`, `callees`, `fan_in`, `fan_out`, `callers_with_metadata`, `callees_with_metadata`, `dependencies_with_metadata`, `traverse_callees`, `traverse_callers` | ✅ Clean |
| `GraphExecutor` | `domain/plan/executor.rs` | `execute`, `execute_with_limits` | ✅ Clean, perfect |
| `GraphStore` | `domain/traits/graph_store.rs` | `save_graph`, `load_graph`, `save_manifest`, `load_manifest`, `clear`, `exists`, `current_checkpoint_id`, `checkpoint_at` | ✅ Clean |
| `IacRepository` | `domain/traits/iac_repository.rs` | IaC resource read | ✅ Clean |
| `SearchProvider` | `domain/traits/search_provider.rs` | `search`, `replace`, `find_similar`, `validate_query` | ✅ Clean |
| `SourceExtractor` | `domain/traits/source_extractor.rs` | `extract` | ✅ Clean |
| `InvestigationStore` | `domain/investigation_store.rs` | `save`, `load`, `list`, `delete`, `add_evidence`, `add_artifact` | ✅ Clean |
| `RunLineageStore` | `domain/analytics/lineage.rs` | `insert`, `get`, `query`, `upsert_descriptor_limits`, `get_descriptor_limits` | ✅ Clean |

**Ports that need cleanup:**

| Port | Issue | Fix |
|------|-------|-----|
| `ViewSpecRepository` | Located in `interface/mcp/handlers/mod.rs:338` | Move to `domain/ports/view_spec_repository.rs` |
| `RepositoryError::UniqueViolation` | Doc comment mentions "PostgreSQL unique-violation (SQLSTATE 23505)" | Change doc to: "A unique constraint was violated" |
| `SearchPage.raw_rank` | Doc comment mentions "ts_rank_cd value" | Change doc to: "The raw relevance score as a positive float. Format is backend-defined." |

**Ports that do NOT exist (PostgreSQL accessed directly via sqlx or concrete PostgresRepository):**

| Table | Port needed | Methods |
|-------|-----------|---------|
| `scan_manifest` | `ManifestStore` | `upsert_manifest_entry`, `delete_manifest_entry`, `get_manifest` |
| `graph_revisions` | `RevisionStore` | `create_revision`, `set_head`, `head_revision` |
| `issues`, `baselines`, `rules` | `QualityStore` | `issues_for_file`, `issues_for_workspace`, `upsert_issue`, `latest_baseline`, `save_baseline` |
| `exploration_sessions` | `SessionStore` | `save_session`, `load_session`, `list_sessions` |
| `named_views`, `view_specs` | `ViewStore` | `save_view_spec`, `list_view_specs`, `delete_view_spec`, `save_named_view` |
| `graph_reports` | `ReportStore` | `save_report`, `latest_report`, `reports_for_workspace` |
| `spaces` | `FederationStore` | `register_space`, `list_spaces`, `get_space` |
| (composite) | `IngestCommit` | `commit_revision` (atomically publishes graph delta + manifest delta + revision + report outbox + lineage) |
| `call_graph_ws` (call-graph aggregate) | `CallGraphStore` | `save_call_graph_ws`, `load_call_graph_ws`, `load_call_graph_current` — added by `e29-0-refactor-call-sites` (not in the original 8-table list above; emerged when the refactor survey found 2 production consumers reaching `PostgresRepository::save_call_graph_ws` directly) |

## Decision

### 1. Clean existing ports

**RepositoryError::UniqueViolation** — remove "PostgreSQL unique-violation (SQLSTATE 23505)" from doc. Keep the error variant, change doc to: "A unique constraint was violated (e.g., duplicate name for the same workspace + owner)."

**SearchPage.raw_rank** — remove "ts_rank_cd value" from doc. Change to: "The raw relevance score as a positive float. Format is backend-defined."

### 2. Move ViewSpecRepository

Move `ViewSpecRepository` from `interface/mcp/handlers/mod.rs:338` to `domain/ports/view_spec_repository.rs`. Update all references in `interface/mcp/handlers/consolidated_handlers.rs` and `infrastructure/persistence/postgres_repository.rs`.

### 3. Define new port traits

All new ports use `#[async_trait]` and return domain types only. Error types are domain-defined enums in `domain/errors/`.

```rust
// In domain/errors/mod.rs
pub enum IngestError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("constraint violation: {0}")]
    ConstraintViolation(String),
    #[error("transaction failed: {0}")]
    TransactionFailed(String),
}

pub enum QualityError {
    #[error("issue not found: {0}")]
    NotFound(String),
    #[error("invalid status: {0}")]
    InvalidStatus(String),
}

pub enum SessionError {
    #[error("session not found: {0}")]
    NotFound(String),
    #[error("serialization error: {0}")]
    Serialization(String),
}

pub enum ViewError {
    #[error("view not found: {0}")]
    NotFound(String),
    #[error("duplicate name: {0}")]
    DuplicateName(String),
}

pub enum ReportError {
    #[error("report not found: {0}")]
    NotFound(String),
}

pub enum FederationError {
    #[error("space not found: {0}")]
    NotFound(String),
    #[error("space conflict: {0}")]
    Conflict(String),
}

pub enum CommitError {
    #[error("revision unknown: workspace {workspace} r{revision}")]
    RevisionUnknown { workspace: WorkspaceId, revision: RevisionId },
    #[error("manifest inconsistent: {0}")]
    ManifestInconsistent(String),
    #[error("commit failed: {0}")]
    CommitFailed(String),
}
```

#### ManifestStore

> **Implemented in `crates/cognicode-core/src/domain/ports/manifest_store.rs`** ✓ reconciled to match actual code.

```rust
use async_trait::async_trait;

#[async_trait]
pub trait ManifestStore: Send + Sync {
    /// Load every `scan_manifest` row for a workspace.
    /// Returns an empty `Vec` if the workspace has never been scanned.
    async fn get_manifest(&self, workspace_id: &str) -> Result<Vec<ScanManifest>, ManifestError>;

    /// Upsert a single `scan_manifest` row.
    async fn upsert_manifest_entry(&self, row: &ScanManifest) -> Result<(), ManifestError>;

    /// Delete a single `scan_manifest` row by `(workspace_id, file_path)`.
    ///
    /// **PHASE 0: stub** in `PostgresManifestStore::delete_manifest_entry` —
    /// no-op until the next change wires the per-row DELETE SQL through
    /// the port (the caller in `application/ingest/service.rs` preserves
    /// the eventual batch semantics via a per-file loop).
    async fn delete_manifest_entry(
        &self,
        workspace_id: &str,
        file_path: &str,
    ) -> Result<(), ManifestError>;
}
```

> **Note**: the original ADR §3 sketched `delete_manifest_entry(&self, ws, path: &str) -> Result<(), IngestError>` where `ws: WorkspaceId`; the current code uses `workspace_id: &str` for symmetry with `SessionStore` and `ReportStore`. The domain type stays the same; only the parameter style differs. Trivial rename if `WorkspaceId` everywhere is preferred (out of scope — keep the change small).

#### RevisionStore

> **Implemented in `crates/cognicode-core/src/domain/ports/revision_store.rs`** ✓ split into read (no-tx) and write (requires tx) shapes.

```rust
#[async_trait]
pub trait RevisionStore: Send + Sync {
    /// **Read-only.** Return the current head revision id for a workspace, if any.
    /// (Was `&mut PgConnection` in the original ADR-suggested shape; split out of `IngestCommit::commit_revision` so the open can be tx-aware while reads are not.)
    async fn head_revision(&self, ws: &WorkspaceId) -> Result<Option<RevisionId>, RevisionError>;

    /// **Write** — opens a new revision, demoting the prior head atomically inside the caller's tx.
    async fn create_revision(
        &self,
        conn: &mut sqlx::PgConnection,
        ws: &WorkspaceId,
    ) -> Result<RevisionId, RevisionError>;

    /// **Write** — promotes `rev` to head, demoting the prior head inside the caller's tx.
    async fn set_head(
        &self,
        conn: &mut sqlx::PgConnection,
        ws: &WorkspaceId,
        rev: RevisionId,
    ) -> Result<(), RevisionError>;
}
```

> **Note**: the e29-0-define-new-ports PR1 trait took a single-tx-required shape (all 3 methods took `&mut PgConnection`); e29-0-refactor-call-sites survey revealed the asymmetry (read callers in `postgres_bridge.rs` had no good way to route a no-tx read through the port) and split `head_revision` out as a tx-free read. Documented here for the audit trail.

#### QualityStore

> **Implemented in `crates/cognicode-core/src/domain/ports/quality_store.rs`** ✓ unified `QualityRepository` + `QualityWritePort` into a single 10-method trait (W1 decision in `e29-0-define-new-ports/verify-report.md`).

```rust
#[async_trait]
pub trait QualityStore: Send + Sync {
    // 8 read methods:
    async fn issues_for_file(&self, file: &str) -> Result<Vec<QualityIssue>, QualityError>;
    async fn issues_for_scope(&self, scope_prefix: &str) -> Result<Vec<QualityIssue>, QualityError>;
    async fn issues_at_line(&self, file: &str, line: u32) -> Result<Vec<QualityIssue>, QualityError>;
    async fn issue_by_id(&self, id: i64) -> Result<Option<QualityIssue>, QualityError>;
    async fn rule_summary(&self, rule_id: &str) -> Result<RuleSummary, QualityError>;
    async fn quality_gate(&self, workspace_id: Option<&str>) -> Result<QualityGateSummary, QualityError>;
    async fn open_issues_count(&self, workspace_id: Option<&str>) -> Result<usize, QualityError>;
    async fn issues_for_workspace(
        &self,
        workspace_id: Option<&str>,
        filter: &IssueFilter,
    ) -> Result<Vec<QualityIssue>, QualityError>;

    // 2 write methods:
    async fn insert_issues(&self, issues: &[NewIssue]) -> Result<UpsertSummary, QualityError>;
    async fn delete_issue(
        &self,
        workspace_id: &str,
        rule_id: &str,
        file_path: &str,
        line: u32,
    ) -> Result<bool, QualityError>;
}
```

> **Domain types introduced** (relocated from `cognicode-explorer` with `e29-0-define-new-ports PR2`):
> - `QualityIssue`, `RuleSummary`, `QualityGateSummary`, `IssueFilter`, `NewIssue`, `UpsertSummary`
> - Error type is `QualityError { Store(String), Conflict(String), NotFound(i64) }` (vs. ADR's `QualityError { NotFound(String), InvalidStatus(String) }` — the implementation picked a string-keyed store/conflict model rather than the Strongly-typed variant model).

#### SessionStore

> **Implemented in `crates/cognicode-core/src/domain/ports/session_store.rs`** ✓ works with `&str` rather than `WorkspaceId` (the ADR's id-based signature would have required moving the `WorkspaceId` value-object into the domain layer, which we deferred).

```rust
#[async_trait]
pub trait SessionStore: Send + Sync {
    async fn save(&self, session: &SessionRow) -> Result<(), SessionError>;
    async fn load(&self, id: &str, workspace_id: &str) -> Result<Option<SessionRow>, SessionError>;
    async fn list(&self, workspace_id: &str) -> Result<Vec<SessionRow>, SessionError>;
}
```

#### ViewSpecStore (formerly `ViewStore`)

> **Implemented in `crates/cognicode-core/src/domain/ports/view_spec_store.rs`** ✓ names shortened to `save`/`load`/`list`/`delete` (the explorer-side bridge in `cognicode-explorer/src/view_spec_payload.rs` does the `ViewSpec ↔ ViewSpecPayload` translation).

```rust
#[async_trait]
pub trait ViewSpecStore: Send + Sync {
    async fn save(&self, payload: &ViewSpecPayload, workspace_id: &str, owner: &str)
        -> Result<(), ViewSpecStoreError>;
    async fn load(&self, id: &str, workspace_id: &str, owner: &str)
        -> Result<Option<ViewSpecPayload>, ViewSpecStoreError>;
    async fn list(&self, workspace_id: &str, owner: &str)
        -> Result<Vec<ViewSpecPayload>, ViewSpecStoreError>;
    async fn delete(&self, id: &str, workspace_id: &str, owner: &str)
        -> Result<bool, ViewSpecStoreError>;

    /// (2 extra methods added by e29-0-refactor-call-sites PR2 + the
    /// postgres_bridge.rs landscape.)
    async fn list_for_workspace(&self, workspace_id: &str, applies_to_kind: &str)
        -> Result<Vec<ViewSpecPayload>, ViewSpecStoreError>;
    async fn update(&self, id: &str, workspace_id: &str, owner: &str,
        seed_object_id: Option<&str>, seed_view_id: Option<&str>, applies_when: Option<&str>)
        -> Result<bool, ViewSpecStoreError>;
}
```

> **About `save_named_view`**: the ADR-028 sketch imagined a single `ViewStore` trait covering both `view_specs` and `named_views` tables — but the codebase already had a separate [`NamedViewStore`](crates/cognicode-core/src/domain/ports/named_view_store.rs) port for the latter (with `save_named_view`/`load_named_view`/`delete_named_view` etc.), pre-dating ADR-028 by several releases. The ADR is reconciled: the `named_views` table stays on its dedicated `NamedViewStore` port; `ViewSpecStore` covers only `view_specs`. The `save_named_view` semantics the ADR referenced now lives at `crate::domain::ports::NamedViewStore::save_named_view`.

#### ReportStore

> **Implemented in `crates/cognicode-core/src/domain/ports/report_store.rs`** ✓ `save_report` added by this round (was previously missing — Phase 0 placeholder). `reports_for_workspace` replaces the old `load_range(workspace, days)` (the days parameter was an exploratory knob; the ADR's contract has no time-range filter, so the Phase 0 adapter uses a 365-day default internally and a future PR can lift that into a query parameter if unbounded reads are ever needed).

```rust
#[async_trait]
pub trait ReportStore: Send + Sync {
    /// Persist a new report row.
    /// **PHASE 0 stub in `PostgresReportStore::save_report`** — the
    /// full SQL (`INSERT INTO graph_reports ... ON CONFLICT DO UPDATE`)
    /// lands once the ingest state's report-publish step is wired
    /// (sibling change to `IngestCommit::commit_revision`'s Phase 1
    /// atomicity work).
    async fn save_report(
        &self,
        workspace_id: &str,
        report: &ReportSummary,
    ) -> Result<(), ReportError>;

    async fn latest_report(&self, workspace_id: &str)
        -> Result<Option<ReportSummary>, ReportError>;
    async fn reports_for_workspace(&self, workspace_id: &str)
        -> Result<Vec<ReportSummary>, ReportError>;
}
```

#### FederationStore

```rust
#[async_trait]
pub trait FederationStore: Send + Sync {
    async fn register_space(&self, space: &Space) -> Result<SpaceId, FederationError>;
    async fn list_spaces(&self) -> Result<Vec<Space>, FederationError>;
    async fn get_space(&self, id: &SpaceId) -> Result<Option<Space>, FederationError>;
}
```

> **Implemented in `crates/cognicode-core/src/domain/ports/federation_store.rs`** ✓ exact match. Reserved as a placeholder — no `spaces` call sites reachable from Phase 0's composition root; the trait is wired in `Runtime` once Phase 1 (`e29-1-ladybug-adapter`) needs it.

#### CallGraphStore

> **Implemented in `crates/cognicode-core/src/domain/ports/call_graph_store.rs`** ✓ added by `e29-0-refactor-call-sites` (the survey found 2 production consumers reaching `PostgresRepository::save_call_graph_ws` directly).

```rust
#[async_trait]
pub trait CallGraphStore: Send + Sync {
    async fn save_call_graph_ws(
        &self,
        graph: &CallGraph,
        ws: &WorkspaceId,
    ) -> Result<RevisionId, CallGraphError>;
    async fn load_call_graph_ws(
        &self,
        ws: &WorkspaceId,
        revision: RevisionId,
    ) -> Result<Option<CallGraph>, CallGraphError>;
    async fn load_call_graph_current(
        &self,
        ws: &WorkspaceId,
    ) -> Result<Option<CallGraph>, CallGraphError>;
}
```

> **Why a dedicated port (not extending `RevisionStore` or `GraphWritePort`)**:
> - `RevisionStore` is the workspace-scoped revision head *counter* (3 methods: `create_revision`, `set_head`, `head_revision`). It manipulates the `graph_revisions` table; no `CallGraph` aggregate is involved.
> - `GraphWritePort` is the generic graph layer (the `graph_nodes` + `graph_edges` tables populated by the docs extractor). It carries no workspace/revision concept.
> - The canonical call-graph WS round-trip is its own aggregate domain concept — a workspace's edges + nodes at a specific revision — and warrants its own port.

#### IngestCommit (composite atomic transaction)

```rust
/// Atomic publication of a complete ingest revision.
/// Implements ADR-019: one transaction = one revision.
#[async_trait]
pub trait IngestCommit: Send + Sync {
    async fn commit_revision(
        &self,
        ws: &WorkspaceId,
        graph_delta: GraphDelta,
        manifest_delta: ManifestDelta,
        report_outbox: Option<ReportIntent>,
    ) -> Result<RevisionId, CommitError>;
}
```

> **PHASE 0: stub** — `PostgresIngestCommit::commit_revision` delegates
> per-stage to `RevisionStore::create_revision` + `ManifestStore::upsert_manifest_entry` + `ReportStore::load_latest`.
> True atomic transaction semantics land with Phase 1's `LadybugStore`
> per ADR-028 §4 (single-writer constraint on `lbug::Database`).

## Scope explicitly NOT covered

The ingest pipeline in `crates/cognicode-core/src/application/ingest/{resolve,cluster,pg_upsert_stage,service,report_stage}.rs`
still uses raw `sqlx::query` directly in 5 files. This is by design:

- The original ADR-028 §1 enumeration covered **8 tables** (the read-side
  surface + the table-level writes), not the multi-stage ingest pipeline.
- Phase 1's `LadybugStore` replaces the entire ingest stack with a single
  `ladybug::Database` write + native `lbug::Transaction`; routing the
  current PG-side ingest through ports would be wasted churn.
- When `e29-1-ladybug-adapter` lands, those 5 files are deleted, not ported.

The single raw-`sqlx` site that **is** ported (Phase 0 scope) is
`crates/cognicode-explorer/src/postgres_bridge.rs:88-105` — the
`SELECT MAX(revision_id) FROM graph_revisions ... HEAD_OF = true` query
now routes through `RevisionStore::head_revision` (was the only
in-scope read-side `postgres::query` per the `e29-0-refactor-call-sites`
survey).


## Consequences

### Positive

- LadybugDB adapter is swappable with PostgreSQL adapter (both implement same traits)
- Unit testing uses in-memory trait implementations
- Port abstraction makes the migration bounded: only adapters change, not domain or application code
- Clear separation between domain (what) and infrastructure (how)
- ADR-019 atomic ingest commits are expressed as a single port trait

### Negative

- 7 new port traits plus 6 new error types
- 30+ call sites must be refactored from `sqlx::query` or `PostgresRepository::method` to `repo.method()` where `repo: Arc<dyn SomePort>`
- `ViewSpecRepository` must be relocated from interface to domain/ports

### Mitigations

- Refactor ports first (Phase 0), before any LadybugDB work
- In-memory test adapters already exist for several ports
- Feature flag keeps both adapters compileable during transition
- New error types are simple enums with `thiserror`

## Phase 0 reconciliation notes

The trait surface above is the **reconciled** state as of Phase 0 trunk merge
(`main` @ f179a116, 2026-08-01). Method-name drift, surface extensions, and
gap-filling captured here were either documented as design decisions (e.g. the
QualityStore / QualityWritePort unification, the RevisionStore tx split) or
out-of-scope gaps (e.g. the 5 raw-`sqlx` files in `application/ingest/`).

The next decision on the Phase 0 surface:
- Decide whether the **3 still-missing concrete SQL pieces** stay as Phase 0
  stubs until the ingest-state work drives them, or get a small follow-up
  PR that completes the SQL:
  - `PostgresManifestStore::delete_manifest_entry` (per-row DELETE)
  - `PostgresReportStore::save_report` (INSERT … ON CONFLICT)
  - `PostgresIngestCommit::commit_revision` (per-stage transaction)
- Either decision is OK; the current stubs preserve the public surface so
  callers compile against the ADR-028 contract.

## References

- [ADR-026](./ADR-026-ladybugdb-canonical-migration.md)
- [ADR-027](./ADR-027-ladybugdb-hybrid-schema-strategy.md)
- [GraphExecutor port spec](../specs/graph-executor-port/spec.md)
- [Executor equivalence conformance spec](../specs/executor-equivalence-conformance/spec.md)
- [LadybugDB spike validation spec](../specs/ladybug-spike-validation/spec.md)
- `sddk/audit-e29-0/code-vs-design-audit.md` (local-only audit that drove this reconciliation pass)
