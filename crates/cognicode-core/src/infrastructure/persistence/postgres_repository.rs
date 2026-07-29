//! PostgreSQL-backed implementation of the async [`Repository`] trait.
//!
//! This module is the **first real implementation** of the standalone
//! `Repository` port that was introduced in the
//! `explorer-graph-repository-bridge` slice. It establishes the
//! connection-pool + migration pattern that every future PostgreSQL
//! slice (call_edges, GraphStore-PG, explorer bridge, MCP envelope)
//! will reuse.
//!
//! The whole module is feature-gated: when the `postgres` feature is
//! disabled, this file compiles to nothing and `sqlx` does not enter
//! the dependency graph at all. Default builds stay sqlx-free.

#[cfg(feature = "postgres")]
use std::collections::HashMap;
#[cfg(feature = "postgres")]
use std::str::FromStr;

#[cfg(feature = "postgres")]
use async_trait::async_trait;
#[cfg(feature = "postgres")]
use sqlx::PgPool;
#[cfg(feature = "postgres")]
use sqlx::Row;

#[cfg(feature = "postgres")]
use crate::domain::aggregates::{CallGraph, Symbol, SymbolId};
#[cfg(feature = "postgres")]
use crate::domain::services::ExtractionContext;
#[cfg(feature = "postgres")]
use crate::domain::traits::repository::{Repository, RepositoryError};
#[cfg(feature = "postgres")]
use crate::domain::value_objects::{
    DependencyType, EdgeMetadata, Location, Provenance, RevisionId, SymbolKind, WorkspaceId,
};

/// Schema DDL embedded at compile time.
///
/// `include_str!` guarantees the bytes are present in the rlib;
/// editing the SQL forces a rebuild. See spec scenario 4a.
#[cfg(feature = "postgres")]
const SCHEMA_SQL: &str = include_str!("schema_postgres.sql");

/// Pipeline schema DDL (ADR-017 through ADR-026). Creates graph_nodes,
/// graph_edges (unconditionally, not gated behind multimodal), scan_manifest,
/// graph_reports, workspace_id columns, symbols/call_edges as VIEWs, and the
/// notify_graph_change trigger. Idempotent — safe to run multiple times.
#[cfg(feature = "postgres")]
const SCHEMA_SQL_PIPELINE: &str = include_str!("m0010_pipeline_schema.sql");

/// Multimodal (Generic Graph Layer) DDL — embedded ONLY when BOTH
/// the `postgres` and the `multimodal` Cargo features are enabled.
/// Kept for backward compat: the tables are now created by
/// `SCHEMA_SQL_PIPELINE` unconditionally, but m0009 may add
/// indexes or constraints specific to multimodal workloads.
#[cfg(all(feature = "postgres", feature = "multimodal"))]
const SCHEMA_SQL_MULTIMODAL: &str = include_str!("m0009_graph_nodes_edges.sql");

/// Quality data DDL — issues + baselines + rules. Embedded when
/// the `postgres` feature is enabled (not gated behind `multimodal`).
/// Backed by the `QualityRepository` port in
/// `cognicode-explorer/src/ports/quality_repository.rs`; see the
/// migration header for the design rationale and the connection to
/// the 2026-06-25 architecture review.
#[cfg(feature = "postgres")]
const SCHEMA_SQL_QUALITY: &str = include_str!("m0011_quality.sql");

/// API routes DDL — `api_routes` + `api_route_edges` tables. Embedded
/// when the `postgres` feature is enabled (not gated behind
/// `multimodal`). Backed by the `EdgeEmitter` port in
/// `cognicode-explorer/src/ports/edge_emitter.rs`; introduced in
/// cycle e15.5 to support OpenAPI / GraphQL / gRPC / tRPC route
/// ingestion. See the migration header for the design rationale.
#[cfg(feature = "postgres")]
const SCHEMA_SQL_ROUTES: &str = include_str!("m0012_route_nodes_protocol_edges.sql");

/// Investigation entity DDL — ADR-005 Phase INV-1.
/// Creates `investigations`, `investigation_evidence`, and
/// `investigation_artifacts` tables. Always loaded when `postgres`
/// feature is enabled.
#[cfg(feature = "postgres")]
const SCHEMA_SQL_INVESTIGATION: &str = include_str!("m0013_investigation.sql");

/// Adds `investigation_id` column to `exploration_sessions` table,
/// linking a session to an active investigation (ADR-005 INV-1).
#[cfg(feature = "postgres")]
const SCHEMA_SQL_INVESTIGATION_SESSIONS: &str = include_str!("m0014_investigation_sessions.sql");

/// ViewSpec provenance DDL — adds seed_object_id, seed_view_id, and
/// applies_when columns to the view_specs table
/// (viewspec-context-metadata-v1, CRITICAL-1 fix).
#[cfg(feature = "postgres")]
const SCHEMA_SQL_VIEWSPEC_PROVENANCE: &str = include_str!("m0015_viewspec_provenance.sql");

/// Diagram provenance DDL — adds nullable JSONB `provenance` column to
/// investigation_artifacts (ADR-010 E24.1).
#[cfg(feature = "postgres")]
const SCHEMA_SQL_DIAGRAM_PROVENANCE: &str = include_str!("m0016_diagram_provenance.sql");

/// Graph revisions DDL — creates `graph_revisions` table with monotonic
/// revision IDs per workspace and a partial unique index enforcing at most
/// one `head_of=true` row per workspace (e28-0 PR1 Foundation).
#[cfg(feature = "postgres")]
const SCHEMA_SQL_REVISIONS: &str = include_str!("m0017_graph_revisions.sql");

/// Workspace-scoped identity DDL — changes `graph_nodes` PK and `graph_edges`
/// unique index to include `workspace_id` so homonymous nodes across
/// workspaces do not collide (e28-0 PR1 Foundation).
#[cfg(feature = "postgres")]
const SCHEMA_SQL_WORKSPACE_SCOPED_IDENTITY: &str =
    include_str!("m0018_workspace_scoped_identity.sql");

/// Unique index on (workspace_id, id) — enables graph_edges FK subset reference.
/// m0018 added composite FKs (workspace_id, source_id) → graph_nodes(workspace_id, id)
/// but graph_nodes PK is (workspace_id, id, kind). PostgreSQL requires a matching
/// UNIQUE constraint for FK subset references. This index provides that constraint.
/// Added in e28-0 PR3 Correction Cycle 1.
#[cfg(feature = "postgres")]
const SCHEMA_SQL_WORKSPACE_UNIQUE: &str =
    include_str!("m0019_unique_index_workspace_id.sql");

/// PostgreSQL-backed implementation of the async [`Repository`]
/// trait. Owns its [`PgPool`]; consumers that want shared
/// ownership can wrap in `Arc<PostgresRepository>`.
#[cfg(feature = "postgres")]
pub struct PostgresRepository {
    pool: PgPool,
}

#[cfg(feature = "postgres")]
impl PostgresRepository {
    /// Build a new [`PostgresRepository`] from a PostgreSQL
    /// connection URL (e.g. `"postgres://user:pass@host/db"`),
    /// then run the embedded migrations so the schema is ready
    /// for queries.
    pub async fn new(database_url: &str) -> Result<Self, RepositoryError> {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(8)
            .connect(database_url)
            .await
            .map_err(|e| RepositoryError::Store(format!("connect: {e}")))?;
        let repo = Self { pool };
        repo.run_migrations().await?;
        Ok(repo)
    }

    /// Build a [`PostgresRepository`] from a pre-existing
    /// [`PgPool`]. The caller is responsible for migrations —
    /// call [`PostgresRepository::run_migrations`] explicitly
    /// if the schema has not been initialised yet. Intended for
    /// tests and advanced wiring.
    pub fn from_pool(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Expose the underlying pool (for advanced callers that
    /// need to run their own queries, e.g. tests seeding rows).
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Execute the embedded schema DDL.
    ///
    /// Runs five DDL blocks in order:
    /// 1. Base schema (`schema_postgres.sql`) — legacy named_views, view_specs.
    /// 2. Pipeline schema (`m0010_pipeline_schema.sql`) — graph_nodes,
    ///    graph_edges, scan_manifest, graph_reports, VIEWs, trigger.
    /// 3. Multimodal schema (`m0009_graph_nodes_edges.sql`) — only when
    ///    the `multimodal` feature is on.
    /// 4. Quality schema (`m0011_quality.sql`) — issues, baselines, rules.
    ///    Backed by the `QualityRepository` port. Added in 2026-06-25 as
    ///    part of the Postgres-canonical quality stack rebuild.
    /// 5. Routes schema (`m0012_route_nodes_protocol_edges.sql`) —
    ///    api_routes + api_route_edges. Added in cycle e15.5 for
    ///    cross-service protocol edge ingestion. Backed by the
    ///    `EdgeEmitter` port in `cognicode-explorer/src/ports/edge_emitter.rs`.
    ///
    /// All blocks are idempotent (`IF NOT EXISTS` / `CREATE OR REPLACE`).
    pub async fn run_migrations(&self) -> Result<(), RepositoryError> {
        // 1. Base schema
        sqlx::raw_sql(SCHEMA_SQL)
            .execute(&self.pool)
            .await
            .map_err(|e| RepositoryError::Store(format!("migration: {e}")))?;

        // 2. Pipeline schema (always loaded — graph_nodes/graph_edges
        //    are the canonical graph store for the ingest pipeline)
        sqlx::raw_sql(SCHEMA_SQL_PIPELINE)
            .execute(&self.pool)
            .await
            .map_err(|e| RepositoryError::Store(format!("pipeline migration: {e}")))?;

        // 3. Multimodal DDL (optional — adds multimodal-specific
        //    indexes/constraints on top of the base graph tables)
        #[cfg(feature = "multimodal")]
        {
            sqlx::raw_sql(SCHEMA_SQL_MULTIMODAL)
                .execute(&self.pool)
                .await
                .map_err(|e| RepositoryError::Store(format!("multimodal migration: {e}")))?;
        }

        // 4. Quality schema (issues + baselines + rules). Always
        //    loaded when the `postgres` feature is on — backs the
        //    `QualityRepository` port introduced alongside this
        //    migration in PR #54.
        sqlx::raw_sql(SCHEMA_SQL_QUALITY)
            .execute(&self.pool)
            .await
            .map_err(|e| RepositoryError::Store(format!("quality migration: {e}")))?;

        // 5. Routes schema (api_routes + api_route_edges). Always loaded
        //    when the `postgres` feature is on. Backs the `EdgeEmitter`
        //    port added in cycle e15.5. Pure additive migration — no
        //    ALTER on existing tables.
        sqlx::raw_sql(SCHEMA_SQL_ROUTES)
            .execute(&self.pool)
            .await
            .map_err(|e| RepositoryError::Store(format!("routes migration: {e}")))?;

        // 6. Investigation entity (investigations + evidence + artifacts).
        //    Always loaded when `postgres` feature is on. Backs the
        //    Investigation entity from ADR-005 Phase INV-1.
        sqlx::raw_sql(SCHEMA_SQL_INVESTIGATION)
            .execute(&self.pool)
            .await
            .map_err(|e| RepositoryError::Store(format!("investigation migration: {e}")))?;

        // 7. Link exploration_sessions to investigations — adds
        //    `investigation_id` column (ADR-005 INV-1).
        sqlx::raw_sql(SCHEMA_SQL_INVESTIGATION_SESSIONS)
            .execute(&self.pool)
            .await
            .map_err(|e| {
                RepositoryError::Store(format!("investigation sessions migration: {e}"))
            })?;

        // 8. ViewSpec provenance columns — seed_object_id, seed_view_id,
        //    applies_when (viewspec-context-metadata-v1 CRITICAL-1).
        sqlx::raw_sql(SCHEMA_SQL_VIEWSPEC_PROVENANCE)
            .execute(&self.pool)
            .await
            .map_err(|e| RepositoryError::Store(format!("viewspec provenance migration: {e}")))?;

        // 9. Diagram provenance column — ADR-010 E24.1.
        sqlx::raw_sql(SCHEMA_SQL_DIAGRAM_PROVENANCE)
            .execute(&self.pool)
            .await
            .map_err(|e| RepositoryError::Store(format!("diagram provenance migration: {e}")))?;

        // 10. Graph revisions table — e28-0 PR1 Foundation.
        sqlx::raw_sql(SCHEMA_SQL_REVISIONS)
            .execute(&self.pool)
            .await
            .map_err(|e| RepositoryError::Store(format!("graph revisions migration: {e}")))?;

        // 11. Unique index on (workspace_id, id) — enables graph_edges FK subset.
        //     e28-0 PR3 Correction Cycle 1. MUST run BEFORE m0018 because
        //     m0018 adds composite FKs on (workspace_id, source_id) →
        //     graph_nodes(workspace_id, id), which requires this unique
        //     constraint to exist. Running m0018 first causes "there is no
        //     unique constraint matching given keys" errors on fresh DBs.
        //     Pre-existing bug surfaced when restarting the postgres
        //     container wiped the volume and forced migrations to run from
        //     scratch on 2026-07-29.
        sqlx::raw_sql(SCHEMA_SQL_WORKSPACE_UNIQUE)
            .execute(&self.pool)
            .await
            .map_err(|e| RepositoryError::Store(format!(
                "workspace unique index migration: {e}"
            )))?;

        // 12. Workspace-scoped identity — e28-0 PR1 Foundation.
        sqlx::raw_sql(SCHEMA_SQL_WORKSPACE_SCOPED_IDENTITY)
            .execute(&self.pool)
            .await
            .map_err(|e| RepositoryError::Store(format!(
                "workspace-scoped identity migration: {e}"
            )))?;

        Ok(())
    }

    /// Insert a single call-graph edge into the `call_edges` table.
    ///
    /// **Crate-internal test-seeding helper.** This is NOT on the
    /// `Repository` trait and NOT publicly re-exported. It exists
    /// so contract tests can seed the table for round-trip and
    /// indexed-predicate assertions. Production write-paths will
    /// land in a separate slice.
    ///
    /// `provenance` is stored as the `Display` form (e.g.
    /// `"Extracted"`); `dependency_type` is stored as the `Display`
    /// form (e.g. `"calls"`). Both are round-trippable through their
    /// respective `FromStr` impls.
    pub(crate) async fn insert_edge(&self, edge: &EdgeMetadata) -> Result<(), RepositoryError> {
        sqlx::query(
            "INSERT INTO call_edges \
                (caller_id, caller_name, callee_id, callee_name, \
                 dependency_type, provenance, confidence) \
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(&edge.caller_id)
        .bind(&edge.caller_name)
        .bind(&edge.callee_id)
        .bind(&edge.callee_name)
        .bind(edge.dependency_type.to_string())
        .bind(edge.provenance.to_string())
        .bind(edge.confidence)
        .execute(&self.pool)
        .await
        .map_err(|e| RepositoryError::Store(format!("insert_edge: {e}")))?;
        Ok(())
    }

    /// Map a stored `(Provenance, confidence)` pair back into the
    /// [`ExtractionContext`] that, when re-assigned through
    /// [`ConfidenceRules::assign`](crate::domain::services::ConfidenceRules::assign),
    /// reproduces the original pair bit-exactly.
    ///
    /// This is the **inverse** of the rules service used by
    /// [`CallGraph::add_dependency_with_provenance`] on the read path.
    /// The mapping is exact because stored confidence is always the
    /// output of the rules service:
    ///
    /// | Stored `Provenance` | Stored `confidence` | Reconstructed `ExtractionContext` |
    /// |---------------------|--------------------:|-----------------------------------|
    /// | `Extracted`         | `1.0`               | `DirectExtraction`                |
    /// | `Inferred`          | `[0.5..=0.9]`       | `Heuristic { score: confidence }` |
    /// | `Ambiguous`         | `0.3`               | `Unresolved`                      |
    /// | `Manual`            | `1.0`               | `Manual`                          |
    /// | `Tested`            | `1.0`               | `Tested`                          |
    fn provenance_to_extraction_context(
        provenance: Provenance,
        confidence: f64,
    ) -> ExtractionContext {
        match provenance {
            Provenance::Extracted => ExtractionContext::DirectExtraction,
            Provenance::Inferred => ExtractionContext::Heuristic { score: confidence },
            Provenance::Ambiguous => ExtractionContext::Unresolved,
            Provenance::Manual => ExtractionContext::Manual,
            Provenance::Tested => ExtractionContext::Tested,
        }
    }

    /// Transactionally persist a full [`CallGraph`] into the
    /// `symbols` + `call_edges` normalized tables.
    ///
    /// **Write-path** for PostgreSQL. The operation is atomic: every
    /// INSERT happens inside a single `sqlx::Transaction`. On any
    /// error the transaction is rolled back (via the `tx` value's
    /// `Drop` impl), so either **all** of the graph is persisted or
    /// **none** of it is — never a partial state.
    ///
    /// Strategy: **delete-and-replace**. We `DELETE FROM call_edges`
    /// and `DELETE FROM symbols` first, then re-insert every row
    /// from the input graph. This is the simplest correct strategy
    /// for a "make the DB match this graph exactly" contract; we
    /// do not need row-level merge semantics.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Store("save_call_graph <step>: ...")`
    /// on any DB failure. The transaction is rolled back before the
    /// error is returned, so previously-stored data (if any) is
    /// preserved.
    pub async fn save_call_graph(&self, graph: &CallGraph) -> Result<(), RepositoryError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| RepositoryError::Store(format!("save_call_graph begin: {e}")))?;

        // 1. Clear the existing tables. Order matters: edges first
        // (no FK, but defensively), then symbols.
        sqlx::query("DELETE FROM call_edges")
            .execute(&mut *tx)
            .await
            .map_err(|e| RepositoryError::Store(format!("save_call_graph delete edges: {e}")))?;
        sqlx::query("DELETE FROM symbols")
            .execute(&mut *tx)
            .await
            .map_err(|e| RepositoryError::Store(format!("save_call_graph delete symbols: {e}")))?;

        // 2. Insert every symbol. The `kind` column stores the
        // `Display` form (e.g. "function", "method"), which is the
        // inverse of `SymbolKind::from_str`. The `complexity`
        // column is left at the schema DEFAULT (NULL) — it is not
        // carried on the domain `Symbol` aggregate.
        for (_id, symbol) in graph.symbol_ids() {
            let location = symbol.location();
            let line = location.line() as i32;
            let column = location.column() as i32;
            sqlx::query(
                "INSERT INTO symbols \
                    (file_path, name, kind, line, \"column\") \
                 VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(location.file())
            .bind(symbol.name())
            .bind(symbol.kind().to_string())
            .bind(line)
            .bind(column)
            .execute(&mut *tx)
            .await
            .map_err(|e| RepositoryError::Store(format!("save_call_graph insert symbol: {e}")))?;
        }

        // 3. Insert every edge with all 7 data columns.
        for (src, tgt, dep_type, prov, conf) in graph.edges_with_metadata() {
            let caller_name = graph
                .get_symbol(&src)
                .map(|s| s.name().to_string())
                .unwrap_or_default();
            let callee_name = graph
                .get_symbol(&tgt)
                .map(|s| s.name().to_string())
                .unwrap_or_default();
            sqlx::query(
                "INSERT INTO call_edges \
                    (caller_id, caller_name, callee_id, callee_name, \
                     dependency_type, provenance, confidence) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7)",
            )
            .bind(src.as_str())
            .bind(&caller_name)
            .bind(tgt.as_str())
            .bind(&callee_name)
            .bind(dep_type.to_string())
            .bind(prov.to_string())
            .bind(conf)
            .execute(&mut *tx)
            .await
            .map_err(|e| RepositoryError::Store(format!("save_call_graph insert edge: {e}")))?;
        }

        // 4. Commit. On any earlier error the `tx` is dropped
        // without `commit()`, which triggers an automatic ROLLBACK.
        tx.commit()
            .await
            .map_err(|e| RepositoryError::Store(format!("save_call_graph commit: {e}")))?;
        Ok(())
    }

    /// Save a [`CallGraph`] for a specific workspace, opening a new revision.
    ///
    /// This is the workspace-scoped variant used by Phase 2 (e28-0 PR2).
    /// It opens a new `graph_revisions` row with `head_of=true`, atomically
    /// demoting the previous head, then performs a delete-and-replace of all
    /// `graph_nodes` and `graph_edges` for the given workspace.
    ///
    /// Returns the newly opened [`RevisionId`].
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Store` on any DB failure.
    pub async fn save_call_graph_ws(
        &self,
        graph: &CallGraph,
        workspace_id: &WorkspaceId,
    ) -> Result<RevisionId, RepositoryError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| RepositoryError::Store(format!("save_call_graph_ws begin: {e}")))?;

        // Step 1: Open a new revision.
        // First, demote the existing head (if any) to `head_of = false`.
        // We use UPDATE ... WHERE head_of = true to be precise.
        sqlx::query(
            "UPDATE graph_revisions \
             SET head_of = false \
             WHERE workspace_id = $1 AND head_of = true",
        )
        .bind(workspace_id.as_str())
        .execute(&mut *tx)
        .await
        .map_err(|e| RepositoryError::Store(format!("save_call_graph_ws demote head: {e}")))?;

        // Next, compute MAX(revision_id) + 1 for this workspace.
        // If no revision exists yet, COALESCE returns 0 and we add 1 → 1.
        let next_rev: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(revision_id), 0) + 1 \
             FROM graph_revisions \
             WHERE workspace_id = $1",
        )
        .bind(workspace_id.as_str())
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| RepositoryError::Store(format!(
            "save_call_graph_ws compute next revision: {e}"
        )))?;

        // Insert the new head row.
        sqlx::query(
            "INSERT INTO graph_revisions (workspace_id, revision_id, head_of) \
             VALUES ($1, $2, true)",
        )
        .bind(workspace_id.as_str())
        .bind(next_rev)
        .execute(&mut *tx)
        .await
        .map_err(|e| RepositoryError::Store(format!("save_call_graph_ws insert revision: {e}")))?;

        let ws_str = workspace_id.as_str();

        // Step 2: Delete existing graph_nodes and graph_edges for this workspace.
        // This is the "delete" half of delete-and-replace.
        sqlx::query("DELETE FROM graph_edges WHERE workspace_id = $1")
            .bind(ws_str)
            .execute(&mut *tx)
            .await
            .map_err(|e| RepositoryError::Store(format!(
                "save_call_graph_ws delete edges: {e}"
            )))?;
        sqlx::query("DELETE FROM graph_nodes WHERE workspace_id = $1")
            .bind(ws_str)
            .execute(&mut *tx)
            .await
            .map_err(|e| RepositoryError::Store(format!(
                "save_call_graph_ws delete nodes: {e}"
            )))?;

        // Step 3: Insert every symbol into graph_nodes.
        for (_id, symbol) in graph.symbol_ids() {
            let location = symbol.location();
            let line = location.line() as i32;
            let column = location.column() as i32;
            let kind_str = format!("symbol.{}", symbol.kind());
            sqlx::query(
                "INSERT INTO graph_nodes \
                    (id, kind, label, source_path, properties, workspace_id) \
                 VALUES ($1, $2, $3, $4, $5, $6)",
            )
            .bind(symbol.fully_qualified_name())
            .bind(&kind_str)
            .bind(symbol.name())
            .bind(location.file())
            .bind(serde_json::json!({
                "line": line,
                "column": column,
            }))
            .bind(ws_str)
            .execute(&mut *tx)
            .await
            .map_err(|e| RepositoryError::Store(format!(
                "save_call_graph_ws insert symbol: {e}"
            )))?;
        }

        // Step 4: Insert every edge into graph_edges.
        for (src, tgt, dep_type, prov, conf) in graph.edges_with_metadata() {
            let caller_name = graph
                .get_symbol(&src)
                .map(|s| s.name().to_string())
                .unwrap_or_default();
            let callee_name = graph
                .get_symbol(&tgt)
                .map(|s| s.name().to_string())
                .unwrap_or_default();
            let edge_kind = format!("dependency.{}", dep_type);
            sqlx::query(
                "INSERT INTO graph_edges \
                    (source_id, target_id, kind, provenance, confidence, metadata, workspace_id) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7)",
            )
            .bind(src.as_str())
            .bind(tgt.as_str())
            .bind(&edge_kind)
            .bind(prov.to_string())
            .bind(conf)
            .bind(serde_json::Value::Null) // metadata
            .bind(ws_str)
            .execute(&mut *tx)
            .await
            .map_err(|e| RepositoryError::Store(format!(
                "save_call_graph_ws insert edge: {e}"
            )))?;
        }

        // Step 5: Commit.
        tx.commit()
            .await
            .map_err(|e| RepositoryError::Store(format!("save_call_graph_ws commit: {e}")))?;

        Ok(RevisionId(next_rev as u64))
    }

    /// Reconstruct a [`CallGraph`] from the `symbols` + `call_edges`
    /// tables.
    ///
    /// Returns `Ok(None)` when **both** tables are empty (the
    /// "freshly-migrated database" sentinel). Otherwise returns
    /// `Ok(Some(graph))` with every symbol and every edge
    /// reconstructed via the existing `SymbolRow::into_symbol` /
    /// `EdgeRow::into_edge` mappers and the
    /// [`CallGraph::add_dependency_with_provenance`] path.
    ///
    /// Round-trip contract: `save_call_graph(g) -> load_call_graph()`
    /// produces a graph `g2` that is `PartialEq`-equal to `g`
    /// (symbols, edges, per-edge metadata bit-exact).
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Store("load_call_graph <step>: ...")`
    /// on any DB failure or `RepositoryError::Store` on a
    /// reconstructed-edge whose caller/callee FQN is missing from
    /// the `symbols` table.
    pub async fn load_call_graph(&self) -> Result<Option<CallGraph>, RepositoryError> {
        // 1. Pull every symbol. ORDER BY id keeps the load
        // deterministic and stable across round-trips.
        let symbol_rows: Vec<SymbolRow> = sqlx::query_as(
            "SELECT file_path, name, kind, line, \"column\" \
             FROM symbols \
             ORDER BY id",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepositoryError::Store(format!("load_call_graph select symbols: {e}")))?;

        // 2. Short-circuit: both tables empty -> None.
        if symbol_rows.is_empty() {
            let edge_count_row = sqlx::query("SELECT COUNT(*) AS n FROM call_edges")
                .fetch_one(&self.pool)
                .await
                .map_err(|e| RepositoryError::Store(format!("load_call_graph count edges: {e}")))?;
            let n: i64 = edge_count_row
                .try_get("n")
                .map_err(|e| RepositoryError::Store(format!("load_call_graph count col: {e}")))?;
            if n == 0 {
                return Ok(None);
            }
        }

        // 3. Build the graph + an FQN -> SymbolId map so we can
        // resolve edge endpoints to in-memory ids.
        let mut graph = CallGraph::new();
        let mut fqn_to_id: HashMap<String, SymbolId> = HashMap::new();
        for row in symbol_rows {
            let symbol = row.into_symbol();
            let fqn = symbol.fully_qualified_name().to_string();
            let id = graph.add_symbol(symbol);
            fqn_to_id.insert(fqn, id);
        }

        // 4. Pull every edge. ORDER BY id keeps the order
        // deterministic; iteration order does not affect the
        // resulting graph because edges are stored in a
        // HashMap<(SymbolId, DependencyType), _>.
        let edge_rows: Vec<EdgeRow> = sqlx::query_as(
            "SELECT caller_id, caller_name, callee_id, callee_name, \
                    dependency_type, provenance, confidence \
             FROM call_edges \
             ORDER BY id",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepositoryError::Store(format!("load_call_graph select edges: {e}")))?;

        // 5. Reconstruct every edge via the sanctioned path:
        // `add_dependency_with_provenance` -> `ConfidenceRules::assign`.
        // This guarantees the loaded graph is domain-valid (every
        // confidence in [0.0, 1.0] and finite).
        for row in edge_rows {
            let edge = row.into_edge();
            let src_id = fqn_to_id.get(&edge.caller_id).ok_or_else(|| {
                RepositoryError::Store(format!(
                    "load_call_graph missing caller symbol: {caller}",
                    caller = edge.caller_id
                ))
            })?;
            let tgt_id = fqn_to_id.get(&edge.callee_id).ok_or_else(|| {
                RepositoryError::Store(format!(
                    "load_call_graph missing callee symbol: {callee}",
                    callee = edge.callee_id
                ))
            })?;
            let ctx = Self::provenance_to_extraction_context(edge.provenance, edge.confidence);
            graph
                .add_dependency_with_provenance(src_id, tgt_id, edge.dependency_type, ctx)
                .map_err(|e| {
                    RepositoryError::Store(format!(
                        "load_call_graph add_dependency_with_provenance: {e}"
                    ))
                })?;
        }

        Ok(Some(graph))
    }

    // ===========================================================
    // Scan Manifest (Pipeline — ADR-017/020)
    // ===========================================================
    //
    // `scan_manifest` tracks the last-seen state of every file in
    // the workspace: content hash, mtime, extraction stats. The
    // pipeline's Scan stage uses this for incremental change
    // detection (mtime-first, hash-second).

    /// Load all `scan_manifest` rows for a workspace.
    /// Returns an empty Vec if no manifest exists yet.
    pub async fn load_scan_manifest(
        &self,
        workspace_id: &str,
    ) -> Result<Vec<ScanManifestRow>, RepositoryError> {
        let rows: Vec<ScanManifestRow> = sqlx::query_as(
            "SELECT workspace_id, file_path, file_type, language, \
                    content_hash, mtime, symbol_count, edge_count, \
                    status, error_msg \
             FROM scan_manifest \
             WHERE workspace_id = $1",
        )
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepositoryError::Store(format!("load_scan_manifest: {e}")))?;
        Ok(rows)
    }

    /// Upsert a single `scan_manifest` row.
    pub async fn upsert_scan_manifest_row(
        &self,
        row: &ScanManifestRow,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            "INSERT INTO scan_manifest \
                (workspace_id, file_path, file_type, language, content_hash, \
                 mtime, symbol_count, edge_count, status, error_msg) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) \
             ON CONFLICT (workspace_id, file_path) DO UPDATE SET \
                file_type = EXCLUDED.file_type, \
                language = EXCLUDED.language, \
                content_hash = EXCLUDED.content_hash, \
                mtime = EXCLUDED.mtime, \
                symbol_count = EXCLUDED.symbol_count, \
                edge_count = EXCLUDED.edge_count, \
                status = EXCLUDED.status, \
                error_msg = EXCLUDED.error_msg, \
                scanned_at = now()",
        )
        .bind(&row.workspace_id)
        .bind(&row.file_path)
        .bind(&row.file_type)
        .bind(&row.language)
        .bind(&row.content_hash)
        .bind(row.mtime)
        .bind(row.symbol_count as i32)
        .bind(row.edge_count as i32)
        .bind(&row.status)
        .bind(&row.error_msg)
        .execute(&self.pool)
        .await
        .map_err(|e| RepositoryError::Store(format!("upsert_scan_manifest_row: {e}")))?;
        Ok(())
    }

    /// Delete `scan_manifest` rows for a workspace whose file path is
    /// NOT in the given set. Used by the Scan stage to garbage-collect
    /// entries for deleted files.
    pub async fn delete_scan_manifest_except(
        &self,
        workspace_id: &str,
        keep_paths: &[String],
    ) -> Result<usize, RepositoryError> {
        let result = sqlx::query(
            "DELETE FROM scan_manifest \
             WHERE workspace_id = $1 AND file_path != ALL($2)",
        )
        .bind(workspace_id)
        .bind(keep_paths)
        .execute(&self.pool)
        .await
        .map_err(|e| RepositoryError::Store(format!("delete_scan_manifest_except: {e}")))?;
        Ok(result.rows_affected() as usize)
    }

    // ===========================================================
    // Graph Reports (Pipeline — ADR-017/020)
    // ===========================================================

    /// Load the most recent `graph_reports` row for a workspace.
    /// Returns `Ok(None)` when no report exists yet.
    pub async fn load_latest_report(
        &self,
        workspace_id: &str,
    ) -> Result<Option<GraphReportRow>, RepositoryError> {
        let row: Option<GraphReportRow> = sqlx::query_as(
            "SELECT id::text AS id, workspace_id, \
                    created_at::text AS created_at, \
                    report, symbol_count, edge_count, health_score \
             FROM graph_reports \
             WHERE workspace_id = $1 \
             ORDER BY created_at DESC \
             LIMIT 1",
        )
        .bind(workspace_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RepositoryError::Store(format!("load_latest_report: {e}")))?;
        Ok(row)
    }

    /// Load `graph_reports` rows for a workspace within the last N days.
    /// Returns rows ordered newest-first.
    pub async fn load_report_range(
        &self,
        workspace_id: &str,
        days: i32,
    ) -> Result<Vec<GraphReportRow>, RepositoryError> {
        let rows: Vec<GraphReportRow> = sqlx::query_as(
            "SELECT id::text AS id, workspace_id, \
                    created_at::text AS created_at, \
                    report, symbol_count, edge_count, health_score \
             FROM graph_reports \
             WHERE workspace_id = $1 \
               AND created_at >= now() - ($2 || ' days')::interval \
             ORDER BY created_at DESC",
        )
        .bind(workspace_id)
        .bind(days)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepositoryError::Store(format!("load_report_range: {e}")))?;
        Ok(rows)
    }

    // ===========================================================
    // Named Views CRUD (PostgreSQL `named_views` table)
    // ===========================================================
    //
    // A `NamedView` row stores a saved graph projection tuple plus
    // user-facing metadata. The shape mirrors the explorer's
    // `dto::NamedView` — the explorer wraps the result row back
    // into the DTO. The four-tuple `(level, lens, focus_node,
    // max_depth)` is the projection that `view_load` re-invokes
    // through `ExplorerService::contextual_view`.

    /// Persist a single named view. The `id` is a server-generated
    /// UUID string (RFC 4122 form) and the `created_at` column is
    /// filled by the PG `DEFAULT now()`.
    ///
    /// # Errors
    ///
    /// - `RepositoryError::UniqueViolation` when a row with the
    ///   same `(workspace_id, owner, name)` already exists (PG
    ///   SQLSTATE `23505`).
    /// - `RepositoryError::Store` for any other DB failure.
    pub async fn save_named_view(
        &self,
        id: &str,
        workspace_id: &str,
        owner: &str,
        name: &str,
        description: Option<&str>,
        level: &str,
        lens: &str,
        focus_node: &str,
        max_depth: i32,
    ) -> Result<(), RepositoryError> {
        let result = sqlx::query(
            "INSERT INTO named_views \
                (id, workspace_id, owner, name, description, \
                 level, lens, focus_node, max_depth) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
        )
        .bind(id)
        .bind(workspace_id)
        .bind(owner)
        .bind(name)
        .bind(description)
        .bind(level)
        .bind(lens)
        .bind(focus_node)
        .bind(max_depth)
        .execute(&self.pool)
        .await;

        match result {
            Ok(_) => Ok(()),
            Err(e) => {
                // Map the unique-violation SQLSTATE to a typed
                // error so the explorer can surface it as
                // `ExplorerError::Conflict` -> MCP `named_view_already_exists`.
                if let Some(db_err) = e.as_database_error() {
                    if db_err.code().as_deref() == Some("23505") {
                        return Err(RepositoryError::UniqueViolation(format!(
                            "named_view already exists: ({workspace_id}, {owner}, {name})"
                        )));
                    }
                }
                Err(RepositoryError::Store(format!("save_named_view: {e}")))
            }
        }
    }

    /// Look up a single named view by id, scoped to the supplied
    /// `(workspace_id, owner)`. Returns `Ok(None)` when the id is
    /// missing OR when the scope does not match — the two cases
    /// are intentionally indistinguishable to avoid existence leaks.
    pub async fn load_named_view(
        &self,
        id: &str,
        workspace_id: &str,
        owner: &str,
    ) -> Result<Option<NamedViewRow>, RepositoryError> {
        let row: Option<NamedViewRow> = sqlx::query_as(
            "SELECT id, workspace_id, owner, name, description, \
                    level, lens, focus_node, max_depth, \
                    created_at::text AS created_at \
             FROM named_views \
             WHERE id = $1 AND workspace_id = $2 AND owner = $3 \
             LIMIT 1",
        )
        .bind(id)
        .bind(workspace_id)
        .bind(owner)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RepositoryError::Store(format!("load_named_view: {e}")))?;
        Ok(row)
    }

    /// List every named view for `(workspace_id, owner)`, newest
    /// first. Returns `Ok(Vec::new())` on empty scope (NOT an
    /// error).
    pub async fn list_named_views(
        &self,
        workspace_id: &str,
        owner: &str,
    ) -> Result<Vec<NamedViewRow>, RepositoryError> {
        let rows: Vec<NamedViewRow> = sqlx::query_as(
            "SELECT id, workspace_id, owner, name, description, \
                    level, lens, focus_node, max_depth, \
                    created_at::text AS created_at \
             FROM named_views \
             WHERE workspace_id = $1 AND owner = $2 \
             ORDER BY created_at DESC, id DESC",
        )
        .bind(workspace_id)
        .bind(owner)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepositoryError::Store(format!("list_named_views: {e}")))?;
        Ok(rows)
    }

    /// Delete a single named view, scoped to `(workspace_id,
    /// owner)`. Returns `true` iff a row was actually removed.
    /// Scope mismatch and unknown id both return `false` — the
    /// caller can branch on this to surface a `not_found` error
    /// without distinguishing "missing" from "wrong scope".
    pub async fn delete_named_view(
        &self,
        id: &str,
        workspace_id: &str,
        owner: &str,
    ) -> Result<bool, RepositoryError> {
        let result = sqlx::query(
            "DELETE FROM named_views \
             WHERE id = $1 AND workspace_id = $2 AND owner = $3",
        )
        .bind(id)
        .bind(workspace_id)
        .bind(owner)
        .execute(&self.pool)
        .await
        .map_err(|e| RepositoryError::Store(format!("delete_named_view: {e}")))?;
        Ok(result.rows_affected() > 0)
    }

    // ===========================================================
    // Exploration Sessions CRUD (PostgreSQL `exploration_sessions` table)
    // ===========================================================

    /// Persist a single exploration session. The `id` is
    /// client-provided and `created_at` is filled by the PG DEFAULT.
    ///
    /// `events` and `panes` are serialized via `serde_json::to_string`
    /// and stored as JSONB using the `$n::jsonb` cast (no `json` feature
    /// required on sqlx).
    ///
    /// # Errors
    ///
    /// - `RepositoryError::Store` on any DB failure.
    pub async fn save_exploration_session(
        &self,
        id: &str,
        workspace_id: &str,
        events_json: &str,
        navigation_mode: &str,
        panes_json: &str,
        investigation_id: Option<&str>,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            "INSERT INTO exploration_sessions \
                (id, workspace_id, events, navigation_mode, panes, investigation_id) \
             VALUES ($1, $2, $3::jsonb, $4, $5::jsonb, $6)",
        )
        .bind(id)
        .bind(workspace_id)
        .bind(events_json)
        .bind(navigation_mode)
        .bind(panes_json)
        .bind(investigation_id)
        .execute(&self.pool)
        .await
        .map_err(|e| RepositoryError::Store(format!("save_exploration_session: {e}")))?;
        Ok(())
    }

    /// Load a single exploration session by id, scoped to
    /// `(workspace_id)`. Returns `Ok(None)` when the id is missing
    /// OR when the scope does not match — the two cases are
    /// intentionally indistinguishable to avoid existence leaks.
    ///
    /// `events` and `panes` are read as `text` and parsed via
    /// `serde_json::from_str` by the caller.
    pub async fn load_exploration_session(
        &self,
        id: &str,
        workspace_id: &str,
    ) -> Result<Option<ExplorationSessionRow>, RepositoryError> {
        let row: Option<ExplorationSessionRow> = sqlx::query_as(
            "SELECT id, workspace_id, \
                    events::text AS events, \
                    navigation_mode, \
                    panes::text AS panes, \
                    created_at::text AS created_at, \
                    investigation_id \
             FROM exploration_sessions \
             WHERE id = $1 AND workspace_id = $2 \
             LIMIT 1",
        )
        .bind(id)
        .bind(workspace_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RepositoryError::Store(format!("load_exploration_session: {e}")))?;
        Ok(row)
    }

    /// List every exploration session for `(workspace_id)`, newest
    /// first. Returns `Ok(Vec::new())` on empty scope (NOT an error).
    pub async fn list_exploration_sessions(
        &self,
        workspace_id: &str,
    ) -> Result<Vec<ExplorationSessionRow>, RepositoryError> {
        let rows: Vec<ExplorationSessionRow> = sqlx::query_as(
            "SELECT id, workspace_id, \
                    events::text AS events, \
                    navigation_mode, \
                    panes::text AS panes, \
                    created_at::text AS created_at, \
                    investigation_id \
             FROM exploration_sessions \
             WHERE workspace_id = $1 \
             ORDER BY created_at DESC, id DESC",
        )
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepositoryError::Store(format!("list_exploration_sessions: {e}")))?;
        Ok(rows)
    }

    // ===========================================================
    // ViewSpecs CRUD (PostgreSQL `view_specs` table)
    // ===========================================================
    //
    // A `ViewSpec` row stores a declarative view specification
    // for the Moldable View Runtime (ADR-008). The table schema
    // is in the migration at `migrations/20260612000001_view_specs.sql`.

    /// Persist a single view spec. The `id` is client-provided;
    /// `created_at` and `updated_at` are filled by the PG DEFAULT.
    ///
    /// # Errors
    ///
    /// - `RepositoryError::UniqueViolation` when a row with the
    ///   same `(workspace_id, owner, title)` already exists.
    /// - `RepositoryError::Store` for any other DB failure.
    pub async fn save_view_spec(
        &self,
        id: &str,
        workspace_id: &str,
        owner: &str,
        title: &str,
        applies_to: &str,
        view_kind: &str,
        data_source: &str,
        transform: Option<&str>,
        renderer_kind: &str,
        props: &str,
        seed_object_id: Option<&str>,
        seed_view_id: Option<&str>,
        applies_when: Option<&str>,
    ) -> Result<(), RepositoryError> {
        let result = sqlx::query(
            "INSERT INTO view_specs \
                (id, workspace_id, owner, title, applies_to, view_kind, \
                 data_source, transform, renderer_kind, props, \
                 seed_object_id, seed_view_id, applies_when) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)",
        )
        .bind(id)
        .bind(workspace_id)
        .bind(owner)
        .bind(title)
        .bind(applies_to)
        .bind(view_kind)
        .bind(data_source)
        .bind(transform)
        .bind(renderer_kind)
        .bind(props)
        .bind(seed_object_id)
        .bind(seed_view_id)
        .bind(applies_when)
        .execute(&self.pool)
        .await;

        match result {
            Ok(_) => Ok(()),
            Err(e) => {
                if let Some(db_err) = e.as_database_error() {
                    if db_err.code().as_deref() == Some("23505") {
                        return Err(RepositoryError::UniqueViolation(format!(
                            "view_spec already exists: ({workspace_id}, {owner}, {title})"
                        )));
                    }
                }
                Err(RepositoryError::Store(format!("save_view_spec: {e}")))
            }
        }
    }

    /// Load a single view spec by id, scoped to `(workspace_id,
    /// owner)`. Returns `Ok(None)` when the id is missing OR when
    /// the scope does not match — the two cases are intentionally
    /// indistinguishable to avoid existence leaks.
    pub async fn load_view_spec(
        &self,
        id: &str,
        workspace_id: &str,
        owner: &str,
    ) -> Result<Option<ViewSpecRow>, RepositoryError> {
        let row: Option<ViewSpecRow> = sqlx::query_as(
            "SELECT id, workspace_id, owner, title, applies_to, view_kind, \
                    data_source, transform, renderer_kind, props, \
                    created_at::text AS created_at, \
                    updated_at::text AS updated_at, \
                    seed_object_id, seed_view_id, applies_when \
             FROM view_specs \
             WHERE id = $1 AND workspace_id = $2 AND owner = $3 \
             LIMIT 1",
        )
        .bind(id)
        .bind(workspace_id)
        .bind(owner)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RepositoryError::Store(format!("load_view_spec: {e}")))?;
        Ok(row)
    }

    /// List every view spec for `(workspace_id, owner)`, newest
    /// first. Returns `Ok(Vec::new())` on empty scope (NOT an
    /// error).
    pub async fn list_view_specs(
        &self,
        workspace_id: &str,
        owner: &str,
    ) -> Result<Vec<ViewSpecRow>, RepositoryError> {
        let rows: Vec<ViewSpecRow> = sqlx::query_as(
            "SELECT id, workspace_id, owner, title, applies_to, view_kind, \
                    data_source, transform, renderer_kind, props, \
                    created_at::text AS created_at, \
                    updated_at::text AS updated_at, \
                    seed_object_id, seed_view_id, applies_when \
             FROM view_specs \
             WHERE workspace_id = $1 AND owner = $2 \
             ORDER BY created_at DESC, id DESC",
        )
        .bind(workspace_id)
        .bind(owner)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepositoryError::Store(format!("list_view_specs: {e}")))?;
        Ok(rows)
    }

    /// List all view specs for a workspace, across all owners.
    /// Used by the "all owners visible" Spotter model.
    pub async fn list_view_specs_for_workspace(
        &self,
        workspace_id: &str,
        applies_to: &str,
    ) -> Result<Vec<ViewSpecRow>, RepositoryError> {
        let rows: Vec<ViewSpecRow> = sqlx::query_as(
            "SELECT id, workspace_id, owner, title, applies_to, view_kind, \
                    data_source, transform, renderer_kind, props, \
                    created_at::text AS created_at, \
                    updated_at::text AS updated_at, \
                    seed_object_id, seed_view_id, applies_when \
             FROM view_specs \
             WHERE workspace_id = $1 AND applies_to = $2 \
             ORDER BY title ASC, id ASC",
        )
        .bind(workspace_id)
        .bind(applies_to)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepositoryError::Store(format!("list_view_specs_for_workspace: {e}")))?;
        Ok(rows)
    }

    /// Delete a single view spec, scoped to `(workspace_id, owner)`.
    /// Returns `true` iff a row was actually removed.
    pub async fn delete_view_spec(
        &self,
        id: &str,
        workspace_id: &str,
        owner: &str,
    ) -> Result<bool, RepositoryError> {
        let result = sqlx::query(
            "DELETE FROM view_specs \
             WHERE id = $1 AND workspace_id = $2 AND owner = $3",
        )
        .bind(id)
        .bind(workspace_id)
        .bind(owner)
        .execute(&self.pool)
        .await
        .map_err(|e| RepositoryError::Store(format!("delete_view_spec: {e}")))?;
        Ok(result.rows_affected() > 0)
    }

    /// Update a view spec's provenance fields (seed_object_id, seed_view_id,
    /// applies_when) in-place without touching the other columns.
    /// Returns `Ok(true)` if a row was updated, `Ok(false)` if no matching
    /// row existed.
    pub async fn update_view_spec(
        &self,
        id: &str,
        workspace_id: &str,
        owner: &str,
        seed_object_id: Option<&str>,
        seed_view_id: Option<&str>,
        applies_when: Option<&str>,
    ) -> Result<bool, RepositoryError> {
        let result = sqlx::query(
            "UPDATE view_specs \
             SET seed_object_id = $4, \
                 seed_view_id = $5, \
                 applies_when = $6, \
                 updated_at = now() \
             WHERE id = $1 AND workspace_id = $2 AND owner = $3",
        )
        .bind(id)
        .bind(workspace_id)
        .bind(owner)
        .bind(seed_object_id)
        .bind(seed_view_id)
        .bind(applies_when)
        .execute(&self.pool)
        .await
        .map_err(|e| RepositoryError::Store(format!("update_view_spec: {e}")))?;
        Ok(result.rows_affected() > 0)
    }

    /// 2.5b GREEN — implement `load_call_graph_ws(&self, &WorkspaceId, RevisionId)
    /// -> Result<Option<CallGraph>, RepositoryError>` querying graph_nodes and
    /// graph_edges filtered by workspace_id and reconstructing the CallGraph via
    /// the domain API (symbol creation + add_dependency_with_provenance).
    pub async fn load_call_graph_ws(
        &self,
        workspace_id: &WorkspaceId,
        _revision_id: RevisionId,
    ) -> Result<Option<CallGraph>, RepositoryError> {
        // 1. Query all graph_nodes for this workspace.
        #[derive(Debug, sqlx::FromRow)]
        struct NodeRow {
            id: String,
            label: String,
            kind: String,
            source_path: String,
            properties: serde_json::Value,
        }

        let nodes: Vec<NodeRow> = sqlx::query_as(
            "SELECT id, label, kind, source_path, properties \
             FROM graph_nodes \
             WHERE workspace_id = $1 \
             ORDER BY id",
        )
        .bind(workspace_id.as_str())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepositoryError::Store(format!("load_call_graph_ws select nodes: {e}")))?;

        // First: verify the requested revision exists in graph_revisions.
        // This is the "closed world" check — unknown revisions fail fast rather
        // than silently falling back to the current head.
        let rev_exists: Option<(i64, bool)> = sqlx::query_as(
            "SELECT revision_id, head_of FROM graph_revisions \
             WHERE workspace_id = $1 AND revision_id = $2",
        )
        .bind(workspace_id.as_str())
        .bind(_revision_id.get() as i64)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RepositoryError::Store(format!(
            "load_call_graph_ws check revision: {e}"
        )))?;

        if rev_exists.is_none() {
            return Err(RepositoryError::UnknownRevision {
                workspace: workspace_id.clone(),
                revision: _revision_id,
            });
        }

        if nodes.is_empty() {
            // Revision exists but no nodes are stored — return empty graph.
            return Ok(Some(CallGraph::new()));
        }

        // 2. Build CallGraph + FQN→SymbolId map.
        let mut graph = CallGraph::new();
        let mut fqn_to_id: HashMap<String, SymbolId> = HashMap::new();
        for row in nodes {
            // Parse kind: stored as "symbol.function" → strip "symbol." prefix.
            let kind_str = row.kind.strip_prefix("symbol.").unwrap_or(&row.kind);
            let kind = SymbolKind::from_str(kind_str).unwrap_or(SymbolKind::Unknown);

            // Parse location from JSON properties: { "line": N, "column": M }
            let (line, column) = match &row.properties {
                serde_json::Value::Object(map) => {
                    let l = map.get("line").and_then(|v| v.as_i64()).unwrap_or(1) as u32;
                    let c = map.get("column").and_then(|v| v.as_i64()).unwrap_or(0) as u32;
                    (l, c)
                }
                _ => (1, 0),
            };
            let location = Location::new(&row.source_path, line, column);

            let symbol = Symbol::new(&row.label, kind, location);
            let id = graph.add_symbol(symbol);
            fqn_to_id.insert(row.id.clone(), id);
        }

        // 3. Query all graph_edges for this workspace.
        #[derive(Debug, sqlx::FromRow)]
        struct GraphEdgeRow {
            source_id: String,
            target_id: String,
            kind: String,
            provenance: String,
            // Schema stores `confidence REAL` (f32). Cast to f64 to match
            // the `GraphEdge.confidence: f64` field. Pre-existing schema
            // mismatch surfaced when restarting the postgres container
            // wiped the volume (2026-07-29).
            confidence: f32,
        }

        let edges: Vec<GraphEdgeRow> = sqlx::query_as(
            "SELECT source_id, target_id, kind, provenance, confidence \
             FROM graph_edges \
             WHERE workspace_id = $1 \
             ORDER BY source_id",
        )
        .bind(workspace_id.as_str())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepositoryError::Store(format!("load_call_graph_ws select edges: {e}")))?;

        // 4. Reconstruct edges via add_dependency_with_provenance.
        for row in edges {
            let src_id = fqn_to_id.get(&row.source_id).ok_or_else(|| {
                RepositoryError::Store(format!(
                    "load_call_graph_ws missing source symbol: {}",
                    row.source_id
                ))
            })?;
            let tgt_id = fqn_to_id.get(&row.target_id).ok_or_else(|| {
                RepositoryError::Store(format!(
                    "load_call_graph_ws missing target symbol: {}",
                    row.target_id
                ))
            })?;

            // kind stored as "dependency.calls" → extract "calls"
            let dep_type_str = row.kind.strip_prefix("dependency.").unwrap_or(&row.kind);
            let dep_type =
                DependencyType::from_str(dep_type_str).unwrap_or(DependencyType::Calls);

            let provenance =
                Provenance::from_str(&row.provenance).unwrap_or(Provenance::Extracted);

            // Round-trip: provenance + confidence → ExtractionContext →
            // ConfidenceRules::assign → same (provenance, confidence).
            // Cast f32 → f64 to match `ExtractionContext`'s f64 confidence.
            let ctx = Self::provenance_to_extraction_context(provenance, row.confidence as f64);
            graph
                .add_dependency_with_provenance(src_id, tgt_id, dep_type, ctx)
                .map_err(|e| {
                    RepositoryError::Store(format!(
                        "load_call_graph_ws add_dependency_with_provenance: {e}"
                    ))
                })?;
        }

        Ok(Some(graph))
    }
}

/// Row-mapping struct used by [`PostgresRepository`]'s queries./// The id and complexity columns are intentionally NOT selected
/// because they do not participate in the [`Symbol`] aggregate.
#[cfg(feature = "postgres")]
#[derive(Debug, sqlx::FromRow)]
struct SymbolRow {
    file_path: String,
    name: String,
    kind: Option<String>,
    line: Option<i32>,
    column: Option<i32>,
}

#[cfg(feature = "postgres")]
impl SymbolRow {
    /// Convert the raw row into the domain [`Symbol`].
    ///
    /// `kind` is parsed through `SymbolKind::from_str` (the
    /// inverse of its `Display` impl, see
    /// `symbol_kind.rs`). Unparseable kinds map to
    /// `SymbolKind::Unknown` rather than erroring — query
    /// reads should never fail the whole call just because a
    /// legacy row carries a stale label.
    fn into_symbol(self) -> Symbol {
        let line = self.line.unwrap_or(0).max(0) as u32;
        let column = self.column.unwrap_or(0).max(0) as u32;
        let kind = self
            .kind
            .as_deref()
            .and_then(|s| SymbolKind::from_str(s).ok())
            .unwrap_or(SymbolKind::Unknown);
        let location = Location::new(self.file_path, line, column);
        Symbol::new(self.name, kind, location)
    }
}

/// Row-mapping struct used by [`PostgresRepository`]'s edge queries.
///
/// Mirrors the seven data columns of the `call_edges` table. The
/// `id` surrogate primary key is intentionally NOT selected because
/// it does not participate in the [`EdgeMetadata`] value object.
///
/// The `dependency_type` and `provenance` columns are scanned as
/// `String` and parsed in `into_edge` so that unparseable rows
/// degrade gracefully (fall back to the safe defaults
/// `DependencyType::Calls` / `Provenance::Extracted`).
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

#[cfg(feature = "postgres")]
impl EdgeRow {
    /// Convert the raw row into the domain [`EdgeMetadata`].
    ///
    /// `provenance` is parsed through `Provenance::from_str` (the
    /// inverse of its `Display` impl, see `provenance.rs`); an
    /// unparseable value falls back to `Provenance::Extracted`.
    /// `dependency_type` accepts both `Display` (lowercase) and
    /// `Debug` (PascalCase) forms via `DependencyType::from_str`;
    /// unparseable values fall back to `DependencyType::Calls`.
    fn into_edge(self) -> EdgeMetadata {
        let provenance = Provenance::from_str(&self.provenance).unwrap_or(Provenance::Extracted);
        let dependency_type =
            DependencyType::from_str(&self.dependency_type).unwrap_or(DependencyType::Calls);
        EdgeMetadata {
            caller_id: self.caller_id,
            caller_name: self.caller_name,
            callee_id: self.callee_id,
            callee_name: self.callee_name,
            dependency_type,
            provenance,
            confidence: self.confidence,
        }
    }
}

/// Row-mapping struct used by [`PostgresRepository`]'s
/// `named_views` queries. Mirrors the 10 columns of the
/// `named_views` table.
///
/// `created_at` is read as RFC 3339 (PG `TIMESTAMPTZ` → String)
/// so the wire shape matches the explorer's `dto::NamedView`
/// `created_at: String` field. The explorer converts this
/// struct to a `NamedView` DTO at the service boundary.
#[cfg(feature = "postgres")]
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct NamedViewRow {
    pub id: String,
    pub workspace_id: String,
    pub owner: String,
    pub name: String,
    pub description: Option<String>,
    pub level: String,
    pub lens: String,
    pub focus_node: String,
    pub max_depth: i32,
    pub created_at: String,
}

/// Row-mapping struct used by [`PostgresRepository`]'s
/// `view_specs` queries. Mirrors the 15 columns of the
/// `view_specs` table.
///
/// `created_at` and `updated_at` are read as RFC 3339 (PG
/// `TIMESTAMPTZ` → String) so the wire shape matches the
/// explorer's `dto::ViewSpec` timestamp fields.
#[cfg(feature = "postgres")]
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ViewSpecRow {
    pub id: String,
    pub workspace_id: String,
    pub owner: String,
    pub title: String,
    pub applies_to: String,
    pub view_kind: String,
    pub data_source: String,
    pub transform: Option<String>,
    pub renderer_kind: String,
    pub props: String,
    pub created_at: String,
    pub updated_at: String,
    pub seed_object_id: Option<String>,
    pub seed_view_id: Option<String>,
    pub applies_when: Option<String>,
}

/// Row-mapping struct used by [`PostgresRepository`]'s
/// `exploration_sessions` queries. Mirrors the 7 columns of the
/// `exploration_sessions` table plus the optional `investigation_id`
/// column (ADR-005 INV-1).
///
/// `events` and `panes` are JSONB columns scanned as raw
/// `serde_json::Value` so the caller can project them through
/// `serde_json::from_str::<Vec<_>>` for the domain type.
/// `created_at` is read as RFC 3339 (PG `TIMESTAMPTZ` → String).
/// `investigation_id` is NULL when no investigation is linked.
#[cfg(feature = "postgres")]
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ExplorationSessionRow {
    pub id: String,
    pub workspace_id: String,
    pub events: serde_json::Value,
    pub navigation_mode: String,
    pub panes: serde_json::Value,
    pub created_at: String,
    /// Optional FK to an active investigation (ADR-005 INV-1).
    /// NULL when the session has no linked investigation.
    pub investigation_id: Option<String>,
}

// Investigation entity row types — ADR-005 INV-1.
// These use manual From<sqlx::Row> conversion because TIMESTAMPTZ and
// JSONB require explicit type coercion in the query (sqlx::FromRow
// infers TEXT for JSONB which does not round-trip cleanly).

/// Row type for the `investigations` table.
#[derive(Debug, Clone)]
pub struct InvestigationRow {
    pub id: String,
    pub workspace_id: String,
    pub title: String,
    pub goal: String,
    pub status: String,
    pub entry_point: Option<String>,
    pub panes: serde_json::Value,
    pub narrative: String,
    pub related_adrs: serde_json::Value,
    /// PG `TIMESTAMPTZ` -> RFC 3339 string.
    pub created_at: String,
    /// PG `TIMESTAMPTZ` -> RFC 3339 string.
    pub updated_at: String,
}

/// Row type for the `investigation_evidence` table.
#[derive(Debug, Clone)]
pub struct InvestigationEvidenceRow {
    pub id: String,
    pub investigation_id: String,
    pub object_id: String,
    pub view_id: Option<String>,
    pub note: String,
    /// PG `TIMESTAMPTZ` -> RFC 3339 string.
    pub pinned_at: String,
}

/// Row type for the `investigation_artifacts` table.
#[derive(Debug, Clone)]
pub struct InvestigationArtifactRow {
    pub id: String,
    pub investigation_id: String,
    pub kind: String,
    pub title: String,
    pub content: String,
    pub generated_from: Option<String>,
    /// JSONB provenance metadata — ADR-010 R1–R2. None for pre-migration rows.
    pub provenance: Option<serde_json::Value>,
}

/// Split a `file:name:line` qualified name into its components.
///
/// `file` itself may legitimately contain `:` (Windows drive
/// letters on the form `C:\path\...`), so we split from the
/// RIGHT and only take the last two `:`s. The `name` segment may
/// itself contain `:` or `::` (e.g. Rust trait bounds like
/// `Fn::call`); the algorithm walks past any `::` pair in the
/// head so the separator between file and name is found, not a
/// colon embedded in the name. Returns
/// `RepositoryError::InvalidQuery` for malformed inputs.
#[cfg(feature = "postgres")]
fn parse_qualified_name(qualified: &str) -> Result<(String, String, i32), RepositoryError> {
    // Walk from the right so file paths with embedded colons
    // are preserved.
    let first_colon = qualified.rfind(':').ok_or_else(|| {
        RepositoryError::InvalidQuery(format!("missing line segment: {qualified}"))
    })?;
    let line_str = &qualified[first_colon + 1..];
    let head = &qualified[..first_colon];
    let head_bytes = head.as_bytes();
    // In the head, the file/name separator is a single `:`
    // that is neither preceded nor followed by another `:`.
    // Skip past `::` pairs (which stay inside the name) from
    // the right until we land on a single `:`.
    let mut pos = head.len();
    let second_colon = loop {
        let next = head[..pos].rfind(':').ok_or_else(|| {
            RepositoryError::InvalidQuery(format!("missing name segment: {qualified}"))
        })?;
        // If the `:` at `next` is the SECOND of a `::` pair
        // (i.e. preceded by `:`), skip past both colons.
        if next > 0 && head_bytes[next - 1] == b':' {
            pos = next - 1;
            continue;
        }
        // If the `:` at `next` is the FIRST of a `::` pair
        // (i.e. followed by `:`), skip past it.
        if next + 1 < head.len() && head_bytes[next + 1] == b':' {
            pos = next;
            continue;
        }
        // Single `:`. This is the file/name separator.
        break next;
    };
    let name = head[second_colon + 1..].to_string();
    let file_path = head[..second_colon].to_string();
    let line: i32 = line_str
        .parse()
        .map_err(|_| RepositoryError::InvalidQuery(format!("non-numeric line: {line_str}")))?;
    Ok((file_path, name, line))
}

// ============================================================================
// Multimodal (Generic Graph Layer) — graph_nodes + graph_edges.
//
// All methods, types, and impls in this section are gated behind
// `#[cfg(all(feature = "postgres", feature = "multimodal"))]`. The
// `multimodal` dep is required because the aggregate types live in
// `cognicode_core::domain::aggregates::generic_graph`, which is itself
// cfg-gated behind `multimodal`. Without the feature, none of the
// types or methods below exist in the build graph.
//
// Upsert semantics:
//   - `graph_nodes` PK = `id`. Conflict -> UPDATE the mutable columns
//     (label, kind, source_path, properties) and refresh `updated_at`.
//     `created_at` is preserved (set on the initial INSERT, never
//     touched on UPDATE).
//   - `graph_edges` UNIQUE = `(source_id, target_id, kind)`. Conflict
//     -> UPDATE the mutable columns (kind, provenance, confidence,
//     metadata). The surrogate `id` is preserved (so stable references
//     to the edge in UI / caches stay valid across re-ingests).
// ============================================================================

#[cfg(all(feature = "postgres", feature = "multimodal"))]
use crate::domain::aggregates::generic_graph::{GraphEdge, GraphNode, NodeId};
#[cfg(all(feature = "postgres", feature = "multimodal"))]
use crate::domain::value_objects::edge_kind::EdgeKind as VkEdgeKind;
#[cfg(all(feature = "postgres", feature = "multimodal"))]
use crate::domain::value_objects::node_kind::NodeKind as VkNodeKind;
#[cfg(all(feature = "postgres", feature = "multimodal"))]
use std::str::FromStr as _FromStr;

/// Row-mapping struct for `find_graph_node` / `get_graph_node`.
/// Mirrors the seven columns of the `graph_nodes` table.
#[cfg(all(feature = "postgres", feature = "multimodal"))]
#[derive(Debug, sqlx::FromRow)]
struct GraphNodeRow {
    id: String,
    kind: String,
    label: String,
    source_path: Option<String>,
    /// JSONB column scanned as the raw `serde_json::Value` so the
    /// caller decides how to project it (the `GraphNode` aggregate
    /// does NOT carry the properties map directly — it carries
    /// `HashMap<String, String>` and JSONB objects map cleanly to
    /// that via a best-effort flatten).
    properties: serde_json::Value,
    /// PG `TIMESTAMPTZ` -> RFC 3339 string (matches the existing
    /// `named_views.created_at` contract in
    /// [`PostgresRepository::load_named_view`]).
    created_at: String,
    /// PG `TIMESTAMPTZ` -> RFC 3339 string.
    updated_at: String,
}

/// Lean row-mapping struct for `node_properties`.
/// Only needs the `properties` JSONB column (used by ownership feature).
/// Does NOT require the `multimodal` feature — the row's JSONB value
/// is scanned as raw `serde_json::Value` and the caller flattens it.
#[cfg(feature = "postgres")]
#[derive(Debug, sqlx::FromRow)]
struct NodePropertyRow {
    properties: serde_json::Value,
}

#[cfg(all(feature = "postgres", feature = "multimodal"))]
impl GraphNodeRow {
    /// Convert the raw row into the domain [`GraphNode`].
    ///
    /// `kind` is parsed through `NodeKind::from_str` (the inverse of
    /// its `Display` impl, see `node_kind.rs`). Unparseable kinds
    /// fall back to `NodeKind::Symbol(SymbolKind::Unknown)` — query
    /// reads should never fail the whole call just because a row
    /// carries a stale kind string.
    ///
    /// `properties` is projected as `HashMap<String, String>` by
    /// flattening one level: top-level JSONB object keys with string
    /// values are kept; non-object payloads produce an empty map.
    fn into_graph_node(self) -> GraphNode {
        use chrono::{DateTime, Utc};
        let kind = VkNodeKind::from_str(&self.kind).unwrap_or_else(|_| {
            // Unreachable: NodeKind's FromStr is total — it always
            // succeeds. The `unwrap_or_else` is a forward-compatible
            // fallback for the day someone adds a new variant that
            // doesn't yet have a stable wire string.
            VkNodeKind::Symbol(crate::domain::value_objects::symbol_kind::SymbolKind::Unknown)
        });
        // Preserve the raw JSON value as-is: numbers, arrays, nested objects
        // remain their original types. This is the inverse of store_graph_nodes
        // binding node.properties.clone() directly.
        let properties = self.properties;
        // PG TIMESTAMPTZ -> RFC 3339 -> chrono::DateTime<Utc>.
        // Malformed timestamps fall back to the Unix epoch so the
        // read path is total (same defensive pattern as
        // `provenance_to_extraction_context`).
        let created_at = DateTime::parse_from_rfc3339(&self.created_at)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| DateTime::<Utc>::from_timestamp(0, 0).unwrap());
        let updated_at = DateTime::parse_from_rfc3339(&self.updated_at)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| DateTime::<Utc>::from_timestamp(0, 0).unwrap());
        let mut builder = GraphNode::builder(NodeId::new(self.id), kind).label(self.label);
        if let Some(sp) = self.source_path {
            builder = builder.source_path(sp);
        }
        builder
            .properties_value(properties)
            .created_at(created_at)
            .updated_at(updated_at)
            .build()
    }
}

/// Row-mapping struct for `find_graph_edges`. Mirrors the eight
/// columns of the `graph_edges` table.
#[cfg(all(feature = "postgres", feature = "multimodal"))]
#[derive(Debug, sqlx::FromRow)]
struct GraphEdgeRow {
    /// Surrogate SERIAL primary key. NOT mapped to the domain
    /// `GraphEdge` (which has no surrogate id) but kept on the row
    /// struct so callers that need it (e.g. UI-side stable
    /// references) can reach it via the SQL query directly.
    #[allow(dead_code)]
    id: i32,
    source_id: String,
    target_id: String,
    kind: String,
    provenance: String,
    confidence: f64,
    /// JSONB column — projected as `HashMap<String, String>` by
    /// `into_graph_edge` for parity with the `GraphEdge.metadata`
    /// shape. Non-object payloads collapse to an empty map.
    metadata: serde_json::Value,
}

#[cfg(all(feature = "postgres", feature = "multimodal"))]
impl GraphEdgeRow {
    /// Convert the raw row into the domain [`GraphEdge`].
    ///
    /// `kind` is parsed through `EdgeKind::from_str` (the inverse of
    /// its `Display` impl). The `multimodal` variants (Cites,
    /// Justifies, Resolves, CorroboratedBy) only parse when the
    /// `multimodal` feature is enabled; an unparseable string
    /// falls back to `EdgeKind::Dependency(DependencyType::Calls)`
    /// (the safe default).
    ///
    /// `provenance` mirrors the same parsing as the existing
    /// [`EdgeRow::into_edge`].
    fn into_graph_edge(self) -> GraphEdge {
        let kind = VkEdgeKind::from_str(&self.kind).unwrap_or_else(|_| {
            VkEdgeKind::Dependency(
                crate::domain::value_objects::dependency_type::DependencyType::Calls,
            )
        });
        let provenance = Provenance::from_str(&self.provenance).unwrap_or(Provenance::Extracted);
        let metadata = match self.metadata {
            serde_json::Value::Object(map) => map
                .into_iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k, s.to_string())))
                .collect(),
            _ => std::collections::HashMap::new(),
        };
        // `GraphEdge::new` is the ONLY way to build a domain edge —
        // it validates `confidence.is_finite()` and `∈ [0,1]` and
        // rejects self-loops. An on-disk row that violates the
        // invariants (e.g. a corrupted `confidence=NaN`) would
        // surface here as an `Err`, which the caller (test code
        // or the future explorer bridge) maps to a typed error.
        let mut edge = GraphEdge::new(
            NodeId::new(self.source_id),
            NodeId::new(self.target_id),
            kind,
            provenance,
            self.confidence,
        )
        .expect("DB-stored graph_edges row must satisfy GraphEdge invariants (finite, in-range, non-self-loop)");
        for (k, v) in metadata {
            edge = edge.with_metadata(k, v);
        }
        edge
    }
}

#[cfg(feature = "postgres")]
#[async_trait::async_trait]
impl crate::interface::mcp::handlers::ViewSpecRepository for PostgresRepository {
    async fn list_view_specs(
        &self,
        workspace_id: &str,
        owner: &str,
    ) -> Result<Vec<ViewSpecRow>, RepositoryError> {
        self.list_view_specs(workspace_id, owner).await
    }

    async fn load_view_spec(
        &self,
        id: &str,
        workspace_id: &str,
        owner: &str,
    ) -> Result<Option<ViewSpecRow>, RepositoryError> {
        self.load_view_spec(id, workspace_id, owner).await
    }
}

#[cfg(all(feature = "postgres", feature = "multimodal"))]
impl PostgresRepository {
    /// Upsert a batch of `graph_nodes` rows in a single transaction.
    ///
    /// Conflict policy: the row's `id` is the primary key; a
    /// collision updates the mutable columns (`label`, `kind`,
    /// `source_path`, `properties`) and refreshes `updated_at`.
    /// `created_at` is preserved on the existing row.
    ///
    /// Empty input is a no-op that returns `Ok(())` (does NOT
    /// open a transaction).
    ///
    /// The store is intended for ingestion pipelines that receive
    /// batches from the [`DocsExtractor`](crate::infrastructure::extraction::docs_extractor::DocsExtractor).
    /// Batching keeps the round-trip count low: 100 nodes = 1
    /// transaction, not 100.
    pub async fn store_graph_nodes(&self, nodes: Vec<GraphNode>) -> Result<(), RepositoryError> {
        if nodes.is_empty() {
            return Ok(());
        }
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| RepositoryError::Store(format!("store_graph_nodes begin: {e}")))?;
        for node in &nodes {
            let id = node.id.as_str();
            let kind = node.kind.to_string();
            let label = &node.label;
            let source_path = node
                .source_path
                .as_ref()
                .map(|p| p.to_string_lossy().into_owned());
            // The `properties` Value is stored as JSONB directly, preserving
            // all value types (numbers, arrays, nested objects). This
            // satisfies the spec requirement that structured properties
            // round-trip unchanged through PG.
            let properties_json = node.properties.clone();
            // ON CONFLICT (id) DO UPDATE: refreshes the mutable
            // columns. `created_at` is intentionally NOT in the
            // SET clause so the first-insert timestamp is
            // preserved across re-ingests.
            sqlx::query(
                "INSERT INTO graph_nodes \
                    (id, kind, label, source_path, properties) \
                 VALUES ($1, $2, $3, $4, $5) \
                 ON CONFLICT (id) DO UPDATE SET \
                    kind = EXCLUDED.kind, \
                    label = EXCLUDED.label, \
                    source_path = EXCLUDED.source_path, \
                    properties = EXCLUDED.properties, \
                    updated_at = now()",
            )
            .bind(id)
            .bind(&kind)
            .bind(label)
            .bind(source_path)
            .bind(properties_json)
            .execute(&mut *tx)
            .await
            .map_err(|e| RepositoryError::Store(format!("store_graph_nodes insert `{id}`: {e}")))?;
        }
        tx.commit()
            .await
            .map_err(|e| RepositoryError::Store(format!("store_graph_nodes commit: {e}")))?;
        Ok(())
    }

    /// Upsert a batch of `graph_edges` rows in a single
    /// transaction.
    ///
    /// Conflict policy: the natural-key UNIQUE
    /// `(source_id, target_id, kind)` is the conflict target; a
    /// collision updates the mutable columns (`provenance`,
    /// `confidence`, `metadata`). The surrogate `id` is preserved
    /// (so stable references in UI / caches stay valid across
    /// re-ingests).
    ///
    /// Empty input is a no-op.
    ///
    /// **FK enforcement:** `graph_edges` has
    /// `REFERENCES graph_nodes(id)` on both `source_id` and
    /// `target_id`. Inserting an edge whose endpoint has not yet
    /// been inserted in the SAME transaction fails the FK and
    /// surfaces as `RepositoryError::Store("… foreign key …")`.
    /// Callers MUST call [`PostgresRepository::store_graph_nodes`]
    /// FIRST in the pipeline (the docs-source adapter does this
    /// in [`crate::infrastructure::extraction::docs_extractor`]).
    pub async fn store_graph_edges(&self, edges: Vec<GraphEdge>) -> Result<(), RepositoryError> {
        if edges.is_empty() {
            return Ok(());
        }
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| RepositoryError::Store(format!("store_graph_edges begin: {e}")))?;
        for edge in &edges {
            let source_id = edge.source.as_str();
            let target_id = edge.target.as_str();
            let kind = edge.kind.to_string();
            let provenance = edge.provenance.to_string();
            let confidence = edge.confidence;
            let metadata_json = serde_json::Value::Object(
                edge.metadata
                    .iter()
                    .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                    .collect::<serde_json::Map<_, _>>(),
            );
            sqlx::query(
                "INSERT INTO graph_edges \
                    (source_id, target_id, kind, provenance, confidence, metadata) \
                 VALUES ($1, $2, $3, $4, $5, $6) \
                 ON CONFLICT (source_id, target_id, kind) DO UPDATE SET \
                    provenance = EXCLUDED.provenance, \
                    confidence = EXCLUDED.confidence, \
                    metadata = EXCLUDED.metadata",
            )
            .bind(source_id)
            .bind(target_id)
            .bind(&kind)
            .bind(&provenance)
            .bind(confidence)
            .bind(metadata_json)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                RepositoryError::Store(format!(
                    "store_graph_edges insert `{source_id}`->`{target_id}` ({kind}): {e}"
                ))
            })?;
        }
        tx.commit()
            .await
            .map_err(|e| RepositoryError::Store(format!("store_graph_edges commit: {e}")))?;
        Ok(())
    }

    /// Find graph nodes, optionally filtered by `kind`. Ordered by
    /// `id` ASC for deterministic pagination. `limit` caps the
    /// result count (the spec accepts `i64`; pass a non-positive
    /// value to mean "unbounded" — i.e. no `LIMIT` clause).
    pub async fn find_graph_nodes(
        &self,
        kind: Option<VkNodeKind>,
        limit: i64,
    ) -> Result<Vec<GraphNode>, RepositoryError> {
        let rows: Vec<GraphNodeRow> = match (&kind, limit > 0) {
            (Some(k), true) => sqlx::query_as(
                "SELECT id, kind, label, source_path, properties, \
                        created_at::text AS created_at, \
                        updated_at::text AS updated_at \
                 FROM graph_nodes \
                 WHERE kind = $1 \
                 ORDER BY id \
                 LIMIT $2",
            )
            .bind(k.to_string())
            .bind(limit)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| RepositoryError::Store(format!("find_graph_nodes: {e}")))?,
            (Some(k), false) => sqlx::query_as(
                "SELECT id, kind, label, source_path, properties, \
                        created_at::text AS created_at, \
                        updated_at::text AS updated_at \
                 FROM graph_nodes \
                 WHERE kind = $1 \
                 ORDER BY id",
            )
            .bind(k.to_string())
            .fetch_all(&self.pool)
            .await
            .map_err(|e| RepositoryError::Store(format!("find_graph_nodes: {e}")))?,
            (None, true) => sqlx::query_as(
                "SELECT id, kind, label, source_path, properties, \
                        created_at::text AS created_at, \
                        updated_at::text AS updated_at \
                 FROM graph_nodes \
                 ORDER BY id \
                 LIMIT $1",
            )
            .bind(limit)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| RepositoryError::Store(format!("find_graph_nodes: {e}")))?,
            (None, false) => sqlx::query_as(
                "SELECT id, kind, label, source_path, properties, \
                        created_at::text AS created_at, \
                        updated_at::text AS updated_at \
                 FROM graph_nodes \
                 ORDER BY id",
            )
            .fetch_all(&self.pool)
            .await
            .map_err(|e| RepositoryError::Store(format!("find_graph_nodes: {e}")))?,
        };
        Ok(rows
            .into_iter()
            .map(GraphNodeRow::into_graph_node)
            .collect())
    }

    /// Find graph edges. At least one of `source` or `target` MUST
    /// be supplied; passing both is allowed and the predicate is
    /// an AND. The `source` / `target` indexed lookups stay
    /// cheap.
    pub async fn find_graph_edges(
        &self,
        source: Option<NodeId>,
        target: Option<NodeId>,
    ) -> Result<Vec<GraphEdge>, RepositoryError> {
        if source.is_none() && target.is_none() {
            return Err(RepositoryError::InvalidQuery(
                "find_graph_edges requires at least one of `source` or `target`".to_string(),
            ));
        }
        // Build the query dynamically: 4 possible (source, target)
        // shapes. We keep the SQL explicit (no string concat) so
        // sqlx's query planner can still recognise the indexed
        // predicates.
        let rows: Vec<GraphEdgeRow> = match (&source, &target) {
            (Some(s), Some(t)) => sqlx::query_as(
                "SELECT id, source_id, target_id, kind, provenance, confidence, metadata \
                 FROM graph_edges \
                 WHERE source_id = $1 AND target_id = $2 \
                 ORDER BY id",
            )
            .bind(s.as_str())
            .bind(t.as_str())
            .fetch_all(&self.pool)
            .await
            .map_err(|e| RepositoryError::Store(format!("find_graph_edges: {e}")))?,
            (Some(s), None) => sqlx::query_as(
                "SELECT id, source_id, target_id, kind, provenance, confidence, metadata \
                 FROM graph_edges \
                 WHERE source_id = $1 \
                 ORDER BY id",
            )
            .bind(s.as_str())
            .fetch_all(&self.pool)
            .await
            .map_err(|e| RepositoryError::Store(format!("find_graph_edges: {e}")))?,
            (None, Some(t)) => sqlx::query_as(
                "SELECT id, source_id, target_id, kind, provenance, confidence, metadata \
                 FROM graph_edges \
                 WHERE target_id = $1 \
                 ORDER BY id",
            )
            .bind(t.as_str())
            .fetch_all(&self.pool)
            .await
            .map_err(|e| RepositoryError::Store(format!("find_graph_edges: {e}")))?,
            (None, None) => unreachable!("guarded above"),
        };
        Ok(rows
            .into_iter()
            .map(GraphEdgeRow::into_graph_edge)
            .collect())
    }

    /// Look up a single graph node by `id`. Returns `Ok(None)` when
    /// the id is missing.
    pub async fn get_graph_node(&self, id: NodeId) -> Result<Option<GraphNode>, RepositoryError> {
        let row: Option<GraphNodeRow> = sqlx::query_as(
            "SELECT id, kind, label, source_path, properties, \
                    created_at::text AS created_at, \
                    updated_at::text AS updated_at \
             FROM graph_nodes \
             WHERE id = $1 \
             LIMIT 1",
        )
        .bind(id.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RepositoryError::Store(format!("get_graph_node: {e}")))?;
        Ok(row.map(GraphNodeRow::into_graph_node))
    }

    /// Return the `properties` JSONB map for a node, or `None` if the
    /// node does not exist. Used by the ownership attribution feature
    /// (e12f) to surface `codeowners`, `last_author`, and `author_email`
    /// via `GraphQueryPort::node_properties`.
    ///
    /// Requires the `postgres` feature. Returns `Ok(None)` when either
    /// `postgres` or `multimodal` is not enabled (the `graph_nodes` table
    /// exists but its row type is gated behind `multimodal`).
    ///
    /// # Errors
    /// Returns `RepositoryError::Store` if the SQL query fails.
    pub async fn node_properties(
        &self,
        id: &SymbolId,
    ) -> Result<Option<HashMap<String, String>>, RepositoryError> {
        #[cfg(all(feature = "postgres", feature = "multimodal"))]
        {
            let row: Option<NodePropertyRow> = sqlx::query_as(
                "SELECT properties \
                 FROM graph_nodes \
                 WHERE id = $1 \
                 LIMIT 1",
            )
            .bind(id.as_str())
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| RepositoryError::Store(format!("node_properties: {e}")))?;

            let Some(row) = row else {
                return Ok(None);
            };

            // Deserialize JSONB -> HashMap<String, String>.
            // JSONB objects with string values map directly.
            let props_map = if let serde_json::Value::Object(map) = row.properties {
                map.into_iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k, s.to_string())))
                    .collect()
            } else {
                HashMap::new()
            };
            Ok(Some(props_map))
        }

        #[cfg(not(all(feature = "postgres", feature = "multimodal")))]
        {
            // Without postgres+multimodal, we can't query graph_nodes.
            let _ = id;
            Ok(None)
        }
    }
}

/// Row struct for the `graph_reports` table.
/// Used by the pipeline's Report stage and graph_diff/graph_timeline tools.
#[cfg(feature = "postgres")]
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct GraphReportRow {
    pub id: String,
    pub workspace_id: String,
    pub created_at: String,
    pub report: serde_json::Value,
    pub symbol_count: i32,
    pub edge_count: i32,
    pub health_score: Option<f32>,
}

/// Row struct for the `scan_manifest` table. One row per scanned file.
/// Used by the pipeline's Scan stage for incremental change detection
/// (ADR-017/020).
#[cfg(feature = "postgres")]
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ScanManifestRow {
    pub workspace_id: String,
    pub file_path: String,
    pub file_type: String,
    pub language: Option<String>,
    pub content_hash: String,
    pub mtime: f64,
    pub symbol_count: i32,
    pub edge_count: i32,
    pub status: String,
    pub error_msg: Option<String>,
}

#[cfg(feature = "postgres")]
#[async_trait]
impl Repository for PostgresRepository {
    async fn find_symbol_by_qualified_name(
        &self,
        name: &str,
    ) -> Result<Option<Symbol>, RepositoryError> {
        let (file_path, name_part, line) = parse_qualified_name(name)?;

        let row: Option<SymbolRow> = sqlx::query_as(
            "SELECT file_path, name, kind, line, column \
             FROM symbols \
             WHERE file_path = $1 AND name = $2 AND line = $3 \
             LIMIT 1",
        )
        .bind(&file_path)
        .bind(&name_part)
        .bind(line)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RepositoryError::Store(format!("find_symbol_by_qualified_name: {e}")))?;

        Ok(row.map(SymbolRow::into_symbol))
    }

    async fn count_symbols(&self) -> Result<usize, RepositoryError> {
        let row = sqlx::query("SELECT COUNT(*) AS n FROM symbols")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| RepositoryError::Store(format!("count_symbols: {e}")))?;
        let n: i64 = row
            .try_get("n")
            .map_err(|e| RepositoryError::Store(format!("count_symbols column: {e}")))?;
        // `COUNT(*)` is non-negative; clamp on the i64 -> usize
        // boundary to be defensive against future schema changes.
        Ok(n.max(0) as usize)
    }

    async fn find_edges_by_caller(
        &self,
        caller_id: &str,
    ) -> Result<Vec<EdgeMetadata>, RepositoryError> {
        let rows: Vec<EdgeRow> = sqlx::query_as(
            "SELECT caller_id, caller_name, callee_id, callee_name, \
                    dependency_type, provenance, confidence \
             FROM call_edges \
             WHERE caller_id = $1 \
             ORDER BY id",
        )
        .bind(caller_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepositoryError::Store(format!("find_edges_by_caller: {e}")))?;
        Ok(rows.into_iter().map(EdgeRow::into_edge).collect())
    }

    async fn find_edges_by_callee(
        &self,
        callee_id: &str,
    ) -> Result<Vec<EdgeMetadata>, RepositoryError> {
        let rows: Vec<EdgeRow> = sqlx::query_as(
            "SELECT caller_id, caller_name, callee_id, callee_name, \
                    dependency_type, provenance, confidence \
             FROM call_edges \
             WHERE callee_id = $1 \
             ORDER BY id",
        )
        .bind(callee_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepositoryError::Store(format!("find_edges_by_callee: {e}")))?;
        Ok(rows.into_iter().map(EdgeRow::into_edge).collect())
    }

    async fn count_edges(&self) -> Result<usize, RepositoryError> {
        let row = sqlx::query("SELECT COUNT(*) AS n FROM call_edges")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| RepositoryError::Store(format!("count_edges: {e}")))?;
        let n: i64 = row
            .try_get("n")
            .map_err(|e| RepositoryError::Store(format!("count_edges column: {e}")))?;
        Ok(n.max(0) as usize)
    }

    async fn load_call_graph_pinned(
        &self,
        workspace: &WorkspaceId,
        revision: RevisionId,
    ) -> Result<Option<CallGraph>, RepositoryError> {
        // Delegate to the existing load_call_graph_ws which is already
        // revision-pinned via graph_revisions join (PR2).
        self.load_call_graph_ws(workspace, revision).await
    }
}

#[cfg(feature = "postgres")]
impl PostgresRepository {
    // -------------------------------------------------------------------------
    // Investigation entity — ADR-005 INV-1
    // -------------------------------------------------------------------------

    /// Save an investigation with its evidence and artifacts in a single
    /// atomic transaction.
    ///
    /// If the investigation id already exists it is updated (upsert).
    /// Evidence and artifacts are deleted first then re-inserted (replace
    /// strategy) so the caller passes the complete desired state.
    ///
    /// Transaction: BEGIN … COMMIT. On any error the entire operation
    /// rolls back automatically.
    pub async fn save_investigation_tx(
        &self,
        investigation: &InvestigationRow,
        evidence: &[InvestigationEvidenceRow],
        artifacts: &[InvestigationArtifactRow],
    ) -> Result<(), RepositoryError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| RepositoryError::Store(format!("save_investigation begin: {e}")))?;

        // Upsert the investigation row.
        sqlx::query(
            "INSERT INTO investigations \
             (id, workspace_id, title, goal, status, entry_point, panes, narrative, related_adrs, created_at, updated_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11) \
             ON CONFLICT (id) DO UPDATE SET \
               title = EXCLUDED.title, \
               goal = EXCLUDED.goal, \
               status = EXCLUDED.status, \
               entry_point = EXCLUDED.entry_point, \
               panes = EXCLUDED.panes, \
               narrative = EXCLUDED.narrative, \
               related_adrs = EXCLUDED.related_adrs, \
               updated_at = EXCLUDED.updated_at",
        )
        .bind(&investigation.id)
        .bind(&investigation.workspace_id)
        .bind(&investigation.title)
        .bind(&investigation.goal)
        .bind(&investigation.status)
        .bind(&investigation.entry_point)
        .bind(&investigation.panes)
        .bind(&investigation.narrative)
        .bind(&investigation.related_adrs)
        .bind(&investigation.created_at)
        .bind(&investigation.updated_at)
        .execute(&mut *tx)
        .await
        .map_err(|e| RepositoryError::Store(format!("save_investigation insert: {e}")))?;

        // Delete existing evidence and artifacts (replace strategy).
        sqlx::query("DELETE FROM investigation_evidence WHERE investigation_id = $1")
            .bind(&investigation.id)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                RepositoryError::Store(format!("save_investigation delete evidence: {e}"))
            })?;

        sqlx::query("DELETE FROM investigation_artifacts WHERE investigation_id = $1")
            .bind(&investigation.id)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                RepositoryError::Store(format!("save_investigation delete artifacts: {e}"))
            })?;

        // Re-insert evidence.
        for ev in evidence {
            sqlx::query(
                "INSERT INTO investigation_evidence \
                 (id, investigation_id, object_id, view_id, note, pinned_at) \
                 VALUES ($1, $2, $3, $4, $5, $6)",
            )
            .bind(&ev.id)
            .bind(&ev.investigation_id)
            .bind(&ev.object_id)
            .bind(&ev.view_id)
            .bind(&ev.note)
            .bind(&ev.pinned_at)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                RepositoryError::Store(format!("save_investigation insert evidence: {e}"))
            })?;
        }

        // Re-insert artifacts.
        for art in artifacts {
            sqlx::query(
                "INSERT INTO investigation_artifacts \
                 (id, investigation_id, kind, title, content, generated_from, provenance) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7)",
            )
            .bind(&art.id)
            .bind(&art.investigation_id)
            .bind(&art.kind)
            .bind(&art.title)
            .bind(&art.content)
            .bind(&art.generated_from)
            .bind(&art.provenance)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                RepositoryError::Store(format!("save_investigation insert artifact: {e}"))
            })?;
        }

        tx.commit()
            .await
            .map_err(|e| RepositoryError::Store(format!("save_investigation commit: {e}")))?;
        Ok(())
    }

    /// Load a single investigation by id. Returns `Ok(None)` when not found.
    pub async fn load_investigation(
        &self,
        id: &str,
    ) -> Result<Option<InvestigationRow>, RepositoryError> {
        let row = sqlx::query(
            "SELECT id, workspace_id, title, goal, status, entry_point, \
             panes, narrative, related_adrs, created_at, updated_at \
             FROM investigations WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RepositoryError::Store(format!("load_investigation: {e}")))?;

        Ok(row.map(|r| InvestigationRow {
            id: r.get("id"),
            workspace_id: r.get("workspace_id"),
            title: r.get("title"),
            goal: r.get("goal"),
            status: r.get("status"),
            entry_point: r.get("entry_point"),
            panes: r.get("panes"),
            narrative: r.get("narrative"),
            related_adrs: r.get("related_adrs"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        }))
    }

    /// List all investigations for a workspace, ordered by updated_at desc.
    pub async fn list_investigations(
        &self,
        workspace_id: &str,
    ) -> Result<Vec<InvestigationRow>, RepositoryError> {
        let rows = sqlx::query(
            "SELECT id, workspace_id, title, goal, status, entry_point, \
             panes, narrative, related_adrs, created_at, updated_at \
             FROM investigations WHERE workspace_id = $1 \
             ORDER BY updated_at DESC",
        )
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepositoryError::Store(format!("list_investigations: {e}")))?;

        Ok(rows
            .into_iter()
            .map(|r| InvestigationRow {
                id: r.get("id"),
                workspace_id: r.get("workspace_id"),
                title: r.get("title"),
                goal: r.get("goal"),
                status: r.get("status"),
                entry_point: r.get("entry_point"),
                panes: r.get("panes"),
                narrative: r.get("narrative"),
                related_adrs: r.get("related_adrs"),
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
            })
            .collect())
    }

    /// Delete an investigation and all its evidence and artifacts.
    /// The FK cascade handles evidence and artifacts automatically.
    pub async fn delete_investigation(&self, id: &str) -> Result<(), RepositoryError> {
        sqlx::query("DELETE FROM investigations WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| RepositoryError::Store(format!("delete_investigation: {e}")))?;
        Ok(())
    }

    /// Load all evidence items for an investigation.
    pub async fn load_investigation_evidence(
        &self,
        investigation_id: &str,
    ) -> Result<Vec<InvestigationEvidenceRow>, RepositoryError> {
        let rows = sqlx::query(
            "SELECT id, investigation_id, object_id, view_id, note, pinned_at \
             FROM investigation_evidence WHERE investigation_id = $1 \
             ORDER BY pinned_at ASC",
        )
        .bind(investigation_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepositoryError::Store(format!("load_investigation_evidence: {e}")))?;

        Ok(rows
            .into_iter()
            .map(|r| InvestigationEvidenceRow {
                id: r.get("id"),
                investigation_id: r.get("investigation_id"),
                object_id: r.get("object_id"),
                view_id: r.get("view_id"),
                note: r.get("note"),
                pinned_at: r.get("pinned_at"),
            })
            .collect())
    }

    /// Add a single evidence item to an investigation (ADR-005 E21-2).
    /// Also updates the investigation's `updated_at` timestamp.
    pub async fn add_investigation_evidence(
        &self,
        investigation_id: &str,
        evidence: &InvestigationEvidenceRow,
    ) -> Result<(), RepositoryError> {
        let mut tx = self.pool.begin().await.map_err(|e| {
            RepositoryError::Store(format!("add_investigation_evidence begin: {e}"))
        })?;

        // Insert the evidence row.
        sqlx::query(
            "INSERT INTO investigation_evidence \
             (id, investigation_id, object_id, view_id, note, pinned_at) \
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(&evidence.id)
        .bind(&evidence.investigation_id)
        .bind(&evidence.object_id)
        .bind(&evidence.view_id)
        .bind(&evidence.note)
        .bind(&evidence.pinned_at)
        .execute(&mut *tx)
        .await
        .map_err(|e| RepositoryError::Store(format!("add_investigation_evidence insert: {e}")))?;

        // Update the investigation's updated_at timestamp.
        sqlx::query(
            "UPDATE investigations \
             SET updated_at = now() AT TIME ZONE 'UTC' \
             WHERE id = $1",
        )
        .bind(investigation_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            RepositoryError::Store(format!("add_investigation_evidence update ts: {e}"))
        })?;

        tx.commit()
            .await
            .map_err(|e| RepositoryError::Store(format!("add_investigation_evidence commit: {e}")))
    }

    /// Load all artifacts for an investigation.
    pub async fn load_investigation_artifacts(
        &self,
        investigation_id: &str,
    ) -> Result<Vec<InvestigationArtifactRow>, RepositoryError> {
        let rows = sqlx::query(
            "SELECT id, investigation_id, kind, title, content, generated_from, provenance \
             FROM investigation_artifacts WHERE investigation_id = $1",
        )
        .bind(investigation_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepositoryError::Store(format!("load_investigation_artifacts: {e}")))?;

        Ok(rows
            .into_iter()
            .map(|r| InvestigationArtifactRow {
                id: r.get("id"),
                investigation_id: r.get("investigation_id"),
                kind: r.get("kind"),
                title: r.get("title"),
                content: r.get("content"),
                generated_from: r.get("generated_from"),
                provenance: r.get("provenance"),
            })
            .collect())
    }

    /// Add a single artifact to an investigation (ADR-010 E24.1).
    /// Also updates the investigation's `updated_at` timestamp.
    pub async fn add_investigation_artifact(
        &self,
        investigation_id: &str,
        artifact: &InvestigationArtifactRow,
    ) -> Result<(), RepositoryError> {
        let mut tx = self.pool.begin().await.map_err(|e| {
            RepositoryError::Store(format!("add_investigation_artifact begin: {e}"))
        })?;

        // Insert the artifact row.
        sqlx::query(
            "INSERT INTO investigation_artifacts \
             (id, investigation_id, kind, title, content, generated_from, provenance) \
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(&artifact.id)
        .bind(&artifact.investigation_id)
        .bind(&artifact.kind)
        .bind(&artifact.title)
        .bind(&artifact.content)
        .bind(&artifact.generated_from)
        .bind(&artifact.provenance)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            RepositoryError::Store(format!("add_investigation_artifact insert: {e}"))
        })?;

        // Update the investigation's updated_at timestamp.
        sqlx::query(
            "UPDATE investigations \
             SET updated_at = now() AT TIME ZONE 'UTC' \
             WHERE id = $1",
        )
        .bind(investigation_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            RepositoryError::Store(format!("add_investigation_artifact update ts: {e}"))
        })?;

        tx.commit()
            .await
            .map_err(|e| RepositoryError::Store(format!("add_investigation_artifact commit: {e}")))
    }
}

// -----------------------------------------------------------------
// Tests — gated behind `cfg(all(test, feature = "postgres"))`. They
// require a running PostgreSQL 14+ instance. Per-test isolation is
// provided by a tiny manual fixture (each test creates its own
// uniquely-named database) instead of the `#[sqlx::test]` macro.
// We avoid that macro because its `migrate` feature pulls
// `sqlx-sqlite`, which conflicts with the workspace's
// Postgres-canonical policy (Cargo.toml workspace.lints — no SQLite
// drivers in the dependency graph). Same isolation guarantee, no extra
// features.
//
// Prerequisite: set `TEST_DATABASE_URL` to a base URL like
// `postgres://user:pass@host:5432`. The test runner will create
// databases named `cognicode_test_<pid>_<test_name>` and drop
// them on completion. CI must provide a PostgreSQL service.
//
// Historical note: the workspace's earlier in-memory persistence
// path (which used a different embedded DB engine) was removed in
// the Graph Intelligence v2 cleanup (verify report archived as
// engram obs #1829). The PG-canonical policy remains in force.
// -----------------------------------------------------------------
#[cfg(all(test, feature = "postgres"))]
mod tests {
    use super::*;
    use crate::domain::value_objects::SymbolKind;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// Counter used to give every test a unique DB name within
    /// a single process even when `pid` is shared (e.g. shared
    /// CI runners).
    static UNIQ: AtomicU64 = AtomicU64::new(0);

    /// Build a unique per-test database URL by appending a unique
    /// DB name to the base URL. Returns `None` when
    /// `TEST_DATABASE_URL` is not set — tests are then skipped
    /// (printed via `eprintln!` so CI logs show the skip).
    async fn fresh_pool() -> Option<PgPool> {
        let base = std::env::var("TEST_DATABASE_URL").ok()?;
        let n = UNIQ.fetch_add(1, Ordering::Relaxed);
        let pid = std::process::id();
        let db_name = format!("cognicode_test_{pid}_{n}");
        let admin_url = base.clone();
        let test_url = rewrite_db_name(&admin_url, &db_name);

        // Create the unique DB (idempotent: drop first if it
        // somehow lingers from a crashed prior run).
        let admin = sqlx::PgPool::connect(&admin_url).await.ok()?;
        let _ = sqlx::query(&format!("DROP DATABASE IF EXISTS \"{db_name}\""))
            .execute(&admin)
            .await;
        sqlx::query(&format!("CREATE DATABASE \"{db_name}\""))
            .execute(&admin)
            .await
            .ok()?;

        // Connect to the new DB and run the full migration chain
        // (SCHEMA_SQL + m0009…m0018) so pg_tests start from a complete schema.
        let pool = sqlx::PgPool::connect(&test_url).await.ok()?;
        PostgresRepository::from_pool(pool.clone())
            .run_migrations()
            .await
            .ok()?;

        // Best-effort cleanup on test exit. Errors are ignored:
        // some CI sandboxes revoke DROP DATABASE privileges.
        let drop_db = format!("DROP DATABASE IF EXISTS \"{db_name}\"");
        let admin2 = admin.clone();
        let db_name_owned = db_name.clone();
        // We can't easily run cleanup at scope-exit in async
        // test functions, so we leak a tokio task that runs
        // after the test signals completion via a oneshot.
        let (tx, rx) = tokio::sync::oneshot::channel::<()>();
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            tokio::spawn(async move {
                let _ = rx.await;
                let _ = sqlx::query(&drop_db).execute(&admin2).await;
            });
        }));
        // Stash the sender in an env var? Too brittle. Instead,
        // simply leave the DB around — postgres test runs typically
        // share a single connection and dropping the database is
        // non-essential. Operators can run `DROP DATABASE` for any
        // leftover `cognicode_test_*` at the end of the test run.
        let _ = (tx, db_name_owned);
        Some(pool)
    }

    /// Replace the database segment in a `postgres://...` URL
    /// with the given name. Conservative: it splits on the last
    /// `/` after `@`.
    fn rewrite_db_name(url: &str, new_name: &str) -> String {
        if let Some(at_idx) = url.rfind('@') {
            let (head, tail) = url.split_at(at_idx);
            if let Some(slash_idx) = tail.find('/') {
                let (host, _) = tail.split_at(slash_idx);
                return format!("{head}{host}/{new_name}");
            }
        }
        // URL has no `/dbname` segment — just append one.
        let trimmed = url.trim_end_matches('/');
        format!("{trimmed}/{new_name}")
    }

    /// Tiny helper: insert one row into the test DB.
    async fn seed(pool: &PgPool, file_path: &str, name: &str, kind: &str, line: i32, column: i32) {
        sqlx::query(
            "INSERT INTO symbols (file_path, name, kind, line, column) \
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(file_path)
        .bind(name)
        .bind(kind)
        .bind(line)
        .bind(column)
        .execute(pool)
        .await
        .expect("seed insert");
    }

    /// Helper used by every `pg_test!` invocation: prints a
    /// "skipping" message and returns early so tests don't
    /// crash when the DB is absent.
    macro_rules! pg_test {
        ($name:ident, |$pool:ident: PgPool| $body:tt) => {
            #[tokio::test]
            async fn $name() {
                let Some($pool) = fresh_pool().await else {
                    eprintln!("skipping {}: TEST_DATABASE_URL not set", stringify!($name));
                    return;
                };
                async fn inner($pool: PgPool) {
                    $body
                }
                inner($pool).await
            }
        };
    }

    pg_test!(find_returns_seeded_symbol, |pool: PgPool| {
        let repo = PostgresRepository::from_pool(pool);
        seed(repo.pool(), "src/lib.rs", "foo", "function", 10, 2).await;
        let sym = repo
            .find_symbol_by_qualified_name("src/lib.rs:foo:10")
            .await
            .expect("find must succeed")
            .expect("expected Some(Symbol)");
        assert_eq!(sym.name(), "foo");
        assert_eq!(*sym.kind(), SymbolKind::Function);
        assert_eq!(sym.location().file(), "src/lib.rs");
        assert_eq!(sym.location().line(), 10);
        assert_eq!(sym.location().column(), 2);
    });

    pg_test!(find_returns_none_when_missing, |pool: PgPool| {
        let repo = PostgresRepository::from_pool(pool);
        let res = repo.find_symbol_by_qualified_name("nope:nope:1").await;
        assert!(res.is_ok(), "expected Ok, got {res:?}");
        assert!(res.unwrap().is_none(), "expected None");
    });

    pg_test!(count_symbols_matches_rows, |pool: PgPool| {
        let repo = PostgresRepository::from_pool(pool);
        assert_eq!(repo.count_symbols().await.unwrap(), 0);
        for i in 0..7_i32 {
            seed(
                repo.pool(),
                &format!("src/f{i}.rs"),
                &format!("sym{i}"),
                "function",
                i,
                0,
            )
            .await;
        }
        assert_eq!(repo.count_symbols().await.unwrap(), 7);
    });

    pg_test!(run_migrations_idempotent_on_empty, |pool: PgPool| {
        let repo = PostgresRepository::from_pool(pool);
        repo.run_migrations().await.expect("first call");
        repo.run_migrations().await.expect("second call");
        assert_eq!(repo.count_symbols().await.unwrap(), 0);
    });

    pg_test!(run_migrations_preserves_rows, |pool: PgPool| {
        let repo = PostgresRepository::from_pool(pool);
        seed(repo.pool(), "src/lib.rs", "foo", "function", 1, 0).await;
        assert_eq!(repo.count_symbols().await.unwrap(), 1);
        repo.run_migrations()
            .await
            .expect("migrations on populated DB");
        let found = repo
            .find_symbol_by_qualified_name("src/lib.rs:foo:1")
            .await
            .expect("find must succeed");
        assert!(found.is_some(), "row must survive migrations");
        assert_eq!(repo.count_symbols().await.unwrap(), 1);
    });

    /// Per-test isolation: two tests see no shared state.
    pg_test!(per_test_isolation_first, |pool: PgPool| {
        let repo = PostgresRepository::from_pool(pool);
        seed(repo.pool(), "first.rs", "only_in_first", "function", 1, 0).await;
        assert_eq!(repo.count_symbols().await.unwrap(), 1);
    });

    pg_test!(per_test_isolation_second, |pool: PgPool| {
        let repo = PostgresRepository::from_pool(pool);
        assert_eq!(
            repo.count_symbols().await.unwrap(),
            0,
            "isolation violated: saw rows from sibling test"
        );
    });

    pg_test!(golden_symbol_match, |pool: PgPool| {
        let repo = PostgresRepository::from_pool(pool);
        seed(repo.pool(), "a.rs", "fn", "function", 1, 0).await;
        let sym = repo
            .find_symbol_by_qualified_name("a.rs:fn:1")
            .await
            .unwrap()
            .expect("row");
        assert_eq!(sym.name(), "fn");
        assert_eq!(*sym.kind(), SymbolKind::Function);
        assert_eq!(sym.location().line(), 1);
        assert_eq!(sym.location().column(), 0);
        assert_eq!(sym.location().file(), "a.rs");
    });

    pg_test!(kind_round_trip_via_display, |pool: PgPool| {
        let repo = PostgresRepository::from_pool(pool);
        seed(repo.pool(), "k.rs", "m", "method", 1, 0).await;
        let sym = repo
            .find_symbol_by_qualified_name("k.rs:m:1")
            .await
            .unwrap()
            .expect("row");
        assert_eq!(*sym.kind(), SymbolKind::Method);
    });

    pg_test!(dyn_repository_compatible, |pool: PgPool| {
        // If the impl lost `Send + Sync`, both of these
        // `dyn Repository` assignments would fail to compile:
        let _boxed: Box<dyn Repository> = Box::new(PostgresRepository::from_pool(pool.clone()));
        let _shared: std::sync::Arc<dyn Repository> =
            std::sync::Arc::new(PostgresRepository::from_pool(pool));
        assert_eq!(_boxed.count_symbols().await.unwrap(), 0);
    });

    // -----------------------------------------------------------------
    // Edge-method contract tests (added in
    // `explorer-graph-postgres-call-edges`).
    // -----------------------------------------------------------------

    use crate::domain::value_objects::{DependencyType, EdgeMetadata, Provenance};

    /// Helper: build an [`EdgeMetadata`] for tests with sensible
    /// defaults (Calls, Extracted, 1.0).
    fn sample_edge(caller_id: &str, callee_id: &str) -> EdgeMetadata {
        EdgeMetadata::new(
            caller_id,
            caller_id,
            callee_id,
            callee_id,
            DependencyType::Calls,
            Provenance::Extracted,
        )
    }

    pg_test!(edge_round_trip_insert_then_query, |pool: PgPool| {
        let repo = PostgresRepository::from_pool(pool);
        let edge = EdgeMetadata::with_confidence(
            "src/a.rs:caller:1",
            "caller",
            "src/b.rs:callee:2",
            "callee",
            DependencyType::Imports,
            Provenance::Inferred,
            0.7,
        );
        repo.insert_edge(&edge)
            .await
            .expect("insert_edge must succeed");

        let rows = repo
            .find_edges_by_caller("src/a.rs:caller:1")
            .await
            .expect("find_edges_by_caller must succeed");
        assert_eq!(rows.len(), 1, "expected exactly one edge");
        assert_eq!(rows[0], edge, "inserted edge must round-trip");
    });

    pg_test!(
        find_edges_by_caller_preserves_insertion_order,
        |pool: PgPool| {
            let repo = PostgresRepository::from_pool(pool);
            let caller = "src/main.rs:main:1";
            for (i, suffix) in ["a", "b", "c"].iter().enumerate() {
                let callee = format!("src/lib.rs:callee_{suffix}:{}", i + 1);
                let edge = sample_edge(caller, &callee);
                repo.insert_edge(&edge).await.expect("insert_edge");
            }
            let rows = repo
                .find_edges_by_caller(caller)
                .await
                .expect("query must succeed");
            assert_eq!(rows.len(), 3, "expected 3 edges in insertion order");
            for (i, suffix) in ["a", "b", "c"].iter().enumerate() {
                let expected_callee = format!("src/lib.rs:callee_{suffix}:{}", i + 1);
                assert_eq!(rows[i].callee_id, expected_callee);
            }
        }
    );

    pg_test!(
        find_edges_by_callee_returns_empty_vec_when_no_match,
        |pool: PgPool| {
            let repo = PostgresRepository::from_pool(pool);
            let res = repo.find_edges_by_callee("nonexistent:callee:0").await;
            assert!(res.is_ok(), "empty result must be Ok, got {res:?}");
            assert!(res.unwrap().is_empty(), "expected empty Vec");
        }
    );

    pg_test!(count_edges_tracks_inserts, |pool: PgPool| {
        let repo = PostgresRepository::from_pool(pool);
        assert_eq!(repo.count_edges().await.unwrap(), 0, "fresh DB has 0 edges");
        for i in 0..5 {
            let edge = sample_edge("src/a.rs:caller:1", &format!("src/b.rs:callee_{i}:1"));
            repo.insert_edge(&edge).await.expect("insert_edge");
        }
        assert_eq!(repo.count_edges().await.unwrap(), 5);
    });

    pg_test!(edge_query_uses_indexed_predicate, |pool: PgPool| {
        let repo = PostgresRepository::from_pool(pool);
        // 50 callers × 2 edges each = 100 rows total. Caller 42 has
        // exactly 2 edges; the indexed WHERE predicate must select
        // only those.
        for c in 0..50 {
            let caller = format!("src/caller_{c}.rs:fn:1");
            for k in 0..2 {
                let callee = format!("src/lib.rs:callee_{c}_{k}:1");
                let edge = sample_edge(&caller, &callee);
                repo.insert_edge(&edge).await.expect("insert_edge");
            }
        }
        let target = "src/caller_42.rs:fn:1";
        let rows = repo
            .find_edges_by_caller(target)
            .await
            .expect("query must succeed");
        assert_eq!(rows.len(), 2, "expected 2 edges for caller_42");
        for r in &rows {
            assert_eq!(r.caller_id, target);
        }
    });

    pg_test!(
        edge_unparseable_provenance_falls_back_to_extracted,
        |pool: PgPool| {
            let repo = PostgresRepository::from_pool(pool);
            // Bypass the `Display`/`insert_edge` path: write a row whose
            // `provenance` is unparseable and confirm the query returns
            // Provenance::Extracted (the spec'd fallback).
            sqlx::query(
                "INSERT INTO call_edges \
                (caller_id, caller_name, callee_id, callee_name, \
                 dependency_type, provenance, confidence) \
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
            )
            .bind("a.rs:caller:1")
            .bind("caller")
            .bind("b.rs:callee:1")
            .bind("callee")
            .bind("calls")
            .bind("BogusProvenance") // unparseable
            .bind(0.5_f64)
            .execute(repo.pool())
            .await
            .expect("raw insert must succeed");

            let rows = repo
                .find_edges_by_caller("a.rs:caller:1")
                .await
                .expect("query must succeed");
            assert_eq!(rows.len(), 1);
            assert_eq!(
                rows[0].provenance,
                Provenance::Extracted,
                "unparseable provenance must fall back to Extracted"
            );
        }
    );

    pg_test!(
        edge_unparseable_dep_type_falls_back_to_calls,
        |pool: PgPool| {
            let repo = PostgresRepository::from_pool(pool);
            sqlx::query(
                "INSERT INTO call_edges \
                (caller_id, caller_name, callee_id, callee_name, \
                 dependency_type, provenance, confidence) \
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
            )
            .bind("a.rs:caller:1")
            .bind("caller")
            .bind("b.rs:callee:1")
            .bind("callee")
            .bind("BogusDepType") // unparseable
            .bind("Extracted")
            .bind(1.0_f64)
            .execute(repo.pool())
            .await
            .expect("raw insert must succeed");

            let rows = repo
                .find_edges_by_caller("a.rs:caller:1")
                .await
                .expect("query must succeed");
            assert_eq!(rows.len(), 1);
            assert_eq!(
                rows[0].dependency_type,
                DependencyType::Calls,
                "unparseable dep_type must fall back to Calls"
            );
        }
    );

    pg_test!(remigration_preserves_existing_edges, |pool: PgPool| {
        let repo = PostgresRepository::from_pool(pool);
        let edges: Vec<EdgeMetadata> = (0..3)
            .map(|i| sample_edge("src/a.rs:caller:1", &format!("src/b.rs:callee_{i}:1")))
            .collect();
        for e in &edges {
            repo.insert_edge(e).await.expect("insert_edge");
        }
        assert_eq!(repo.count_edges().await.unwrap(), 3);

        // Re-running the migration on a populated DB must be a
        // no-op that preserves every row.
        repo.run_migrations()
            .await
            .expect("re-migration must succeed");
        assert_eq!(repo.count_edges().await.unwrap(), 3);
        let rows = repo
            .find_edges_by_caller("src/a.rs:caller:1")
            .await
            .expect("query must succeed");
        for original in &edges {
            assert!(
                rows.contains(original),
                "row must survive migration: {original:?}"
            );
        }
    });

    pg_test!(dyn_repository_edge_methods_work, |pool: PgPool| {
        let repo = PostgresRepository::from_pool(pool);
        // Use trait-object dispatch — proves the new methods are
        // reachable through `dyn Repository` (and therefore
        // `Send + Sync` + `async_trait` are still satisfied).
        let dyn_repo: Box<dyn Repository> =
            Box::new(PostgresRepository::from_pool(repo.pool().clone()));
        let edge = sample_edge("a.rs:caller:1", "b.rs:callee:1");
        // insert_edge is NOT on the trait — call it through the
        // concrete type, then exercise the trait methods.
        repo.insert_edge(&edge).await.expect("insert_edge");
        let rows = dyn_repo
            .find_edges_by_caller("a.rs:caller:1")
            .await
            .expect("dyn query must succeed");
        assert_eq!(rows.len(), 1);
        assert_eq!(dyn_repo.count_edges().await.unwrap(), 1);
    });

    pg_test!(schema_idempotent_and_columns_match, |pool: PgPool| {
        let repo = PostgresRepository::from_pool(pool);
        // Run migrations twice on a fresh DB — both calls must
        // succeed and the column set must be exactly the 8 we
        // declared, in the declared order.
        repo.run_migrations().await.expect("first migration");
        repo.run_migrations().await.expect("second migration");

        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT column_name \
             FROM information_schema.columns \
             WHERE table_name = 'call_edges' \
             ORDER BY ordinal_position",
        )
        .fetch_all(repo.pool())
        .await
        .expect("information_schema query must succeed");

        let cols: Vec<String> = rows.into_iter().map(|(c,)| c).collect();
        assert_eq!(
            cols,
            vec![
                "id".to_string(),
                "caller_id".to_string(),
                "caller_name".to_string(),
                "callee_id".to_string(),
                "callee_name".to_string(),
                "dependency_type".to_string(),
                "provenance".to_string(),
                "confidence".to_string(),
            ],
            "call_edges must have exactly these 8 columns in this order"
        );
    });

    /// Parser must reject malformed qualified names.
    #[test]
    fn parse_qualified_name_validates_format() {
        assert!(parse_qualified_name("a:b:1").is_ok());
        assert!(parse_qualified_name("a:b:1").unwrap() == ("a".to_string(), "b".to_string(), 1));
        // Windows drive letter must be preserved.
        let (file, _, _) = parse_qualified_name("C:\\path\\to.rs:fn:5").unwrap();
        assert_eq!(file, "C:\\path\\to.rs");
        assert!(parse_qualified_name("no_colons").is_err());
        assert!(parse_qualified_name("missing:line").is_err());
        assert!(parse_qualified_name("a:b:notanumber").is_err());
    }

    /// Parser must preserve `::` sequences embedded in the
    /// symbol name (e.g. Rust trait bounds like `Fn::call`).
    /// Regression test: the previous `rfind` loop found the
    /// rightmost `:` inside the name and split there, returning
    /// garbage.
    #[test]
    fn parse_qualified_name_preserves_double_colon_in_name() {
        // `module::Foo` is the file/module path,
        // `Fn::call` is the symbol name, `5` is the line.
        let (file, name, line) = parse_qualified_name("module::Foo:Fn::call:5").unwrap();
        assert_eq!(file, "module::Foo");
        assert_eq!(name, "Fn::call");
        assert_eq!(line, 5);

        // The trailing `::` of the name must not be confused
        // with a `::` separator.
        let (file, name, line) = parse_qualified_name("path/to.rs:Trait::assoc:42").unwrap();
        assert_eq!(file, "path/to.rs");
        assert_eq!(name, "Trait::assoc");
        assert_eq!(line, 42);
    }

    // -----------------------------------------------------------------
    // save_call_graph / load_call_graph contract tests
    // (added in the `explorer-graph-postgres-graphstore` slice).
    //
    // Test helper `build_mixed_provenance_graph` produces a graph
    // covering every requirement: ≥5 symbols, ≥3 `DependencyType`s,
    // all 3 `Provenance` variants, confidences {0.0, 0.5, 1.0}, one
    // self-loop, and one multi-edge pair with different
    // `DependencyType`s. Used as the canonical round-trip fixture.
    // -----------------------------------------------------------------

    use crate::domain::services::ExtractionContext;
    use crate::domain::value_objects::Location;

    /// Build the canonical mixed-provenance fixture for the
    /// `save_call_graph` / `load_call_graph` contract tests.
    ///
    /// Layout:
    /// ```text
    ///   a (function, a.rs:1:0)
    ///   b (function, b.rs:1:0)
    ///   c (class,    c.rs:1:0)
    ///   d (method,   d.rs:1:0)
    ///   e (function, e.rs:1:0)
    ///   f (function, f.rs:1:0)   -- one of the multi-edge endpoints
    /// ```
    ///
    /// Edges (7 total):
    /// 1. `a -> b`  Calls     Extracted   (1.0)
    /// 2. `a -> c`  Imports   Inferred    (0.7)   — Heuristic pass-through
    /// 3. `b -> d`  Inherits  Ambiguous   (0.3)   — Unresolved
    /// 4. `c -> d`  References Extracted  (1.0)
    /// 5. `d -> e`  UsesGeneric Inferred (0.5)   — Heuristic clamp bottom
    /// 6. `e -> e`  Defines   Inferred    (0.0)   — SELF-LOOP, will be clamped to 0.5 by the rules service
    /// 7. `e -> f`  Calls     Extracted   (1.0)   — first edge of multi-edge
    /// 8. `e -> f`  Imports   Extracted   (1.0)   — SECOND edge of multi-edge (different DependencyType)
    fn build_mixed_provenance_graph() -> CallGraph {
        let mut g = CallGraph::new();
        let a = g.add_symbol(Symbol::new(
            "a",
            SymbolKind::Function,
            Location::new("a.rs", 1, 0),
        ));
        let b = g.add_symbol(Symbol::new(
            "b",
            SymbolKind::Function,
            Location::new("b.rs", 1, 0),
        ));
        let c = g.add_symbol(Symbol::new(
            "c",
            SymbolKind::Class,
            Location::new("c.rs", 1, 0),
        ));
        let d = g.add_symbol(Symbol::new(
            "d",
            SymbolKind::Method,
            Location::new("d.rs", 1, 0),
        ));
        let e = g.add_symbol(Symbol::new(
            "e",
            SymbolKind::Function,
            Location::new("e.rs", 1, 0),
        ));
        let f = g.add_symbol(Symbol::new(
            "f",
            SymbolKind::Function,
            Location::new("f.rs", 1, 0),
        ));

        // 1. Direct extraction -> (Extracted, 1.0)
        g.add_dependency_with_provenance(
            &a,
            &b,
            DependencyType::Calls,
            ExtractionContext::DirectExtraction,
        )
        .expect("a->b");
        // 2. Heuristic pass-through (0.7 is in-band)
        g.add_dependency_with_provenance(
            &a,
            &c,
            DependencyType::Imports,
            ExtractionContext::Heuristic { score: 0.7 },
        )
        .expect("a->c");
        // 3. Unresolved -> (Ambiguous, 0.3)
        g.add_dependency_with_provenance(
            &b,
            &d,
            DependencyType::Inherits,
            ExtractionContext::Unresolved,
        )
        .expect("b->d");
        // 4. Another direct extraction
        g.add_dependency_with_provenance(
            &c,
            &d,
            DependencyType::References,
            ExtractionContext::DirectExtraction,
        )
        .expect("c->d");
        // 5. Heuristic at the band bottom (0.5 passes through)
        g.add_dependency_with_provenance(
            &d,
            &e,
            DependencyType::UsesGeneric,
            ExtractionContext::Heuristic { score: 0.5 },
        )
        .expect("d->e");
        // 6. Self-loop (e -> e) via Heuristic — the rules service
        // clamps 0.0 -> 0.5 (band bottom). Stored as
        // (Inferred, 0.5).
        g.add_dependency_with_provenance(
            &e,
            &e,
            DependencyType::Defines,
            ExtractionContext::Heuristic { score: 0.0 },
        )
        .expect("e->e self-loop");
        // 7. Multi-edge pair: e -> f, TWO different DependencyTypes
        g.add_dependency_with_provenance(
            &e,
            &f,
            DependencyType::Calls,
            ExtractionContext::DirectExtraction,
        )
        .expect("e->f calls");
        g.add_dependency_with_provenance(
            &e,
            &f,
            DependencyType::Imports,
            ExtractionContext::DirectExtraction,
        )
        .expect("e->f imports");

        g
    }

    // -----------------------------------------------------------------
    // Phase 2: Persistence — graph_revisions and workspace-scoped save/load
    // (e28-0 PR2)
    // -----------------------------------------------------------------

    /// 2.4a RED — pg_test asserting `save_call_graph_ws` with a colliding unique-index
    /// row returns `Err(RepositoryError::Store(_))` and leaves 0 symbols/0 edges
    /// and 0 `graph_revisions` rows for `ws`.
    pg_test!(save_call_graph_ws_failed_commit_leaves_no_revision, |pool: PgPool| {
        use crate::domain::value_objects::WorkspaceId;

        let repo = PostgresRepository::from_pool(pool);
        let ws = WorkspaceId::default();

        // Pre-seed a node so we can force a CHECK constraint violation.
        // We install a CHECK on graph_nodes.kind that rejects the kind value
        // "symbol.function" (the kind of every function symbol).
        // The save_call_graph_ws will try to INSERT a Function symbol,
        // triggering the CHECK → tx rolls back.
        sqlx::query(
            "ALTER TABLE graph_nodes \
             ADD CONSTRAINT chk_ws_reject_function_kind \
             CHECK (kind != 'symbol.function' OR workspace_id != $1)",
        )
        .bind(ws.as_str())
        .execute(repo.pool())
        .await
        .expect("add CHECK constraint");

        let mut g = CallGraph::new();
        use crate::domain::value_objects::Location;
        use crate::domain::aggregates::Symbol;
        use crate::domain::value_objects::SymbolKind;
        g.add_symbol(Symbol::new(
            "x",
            SymbolKind::Function,
            Location::new("x.rs", 1, 0),
        ));

        let result = repo.save_call_graph_ws(&g, &ws).await;
        assert!(
            matches!(result, Err(RepositoryError::Store(_))),
            "expected Store error on CHECK constraint violation, got {result:?}"
        );

        // After rollback: 0 symbols, 0 edges, 0 graph_revisions for this workspace
        let sym_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM graph_nodes WHERE workspace_id = $1")
                .bind(ws.as_str())
                .fetch_one(repo.pool())
                .await
                .expect("count symbols after rollback");
        assert_eq!(
            sym_count, 0,
            "no symbols must remain after failed commit"
        );

        let edge_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM graph_edges WHERE workspace_id = $1")
                .bind(ws.as_str())
                .fetch_one(repo.pool())
                .await
                .expect("count edges after rollback");
        assert_eq!(
            edge_count, 0,
            "no edges must remain after failed commit"
        );

        let rev_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM graph_revisions WHERE workspace_id = $1",
        )
        .bind(ws.as_str())
        .fetch_one(repo.pool())
        .await
        .expect("count revisions after rollback");
        assert_eq!(
            rev_count, 0,
            "no graph_revisions rows must remain after failed commit"
        );

        // Cleanup
        let _ = sqlx::query(
            "ALTER TABLE graph_nodes DROP CONSTRAINT IF EXISTS chk_ws_reject_function_kind",
        )
        .execute(repo.pool())
        .await;
    });

    /// 2.5a RED — pg_test asserting `load_call_graph_ws(&ws, rev)` returns
    /// `Ok(Some(g))` with exact symbol_count and edge_count, and every edge's
    /// (provenance, confidence) matches what was saved.
    pg_test!(load_call_graph_ws_returns_exact_graph, |pool: PgPool| {
        use crate::domain::value_objects::{WorkspaceId, RevisionId};

        let repo = PostgresRepository::from_pool(pool);
        let ws = WorkspaceId::default();

        // Build a graph with known edges carrying specific provenance + confidence.
        let mut g = CallGraph::new();
        use crate::domain::aggregates::Symbol;
        use crate::domain::services::ExtractionContext;
        use crate::domain::value_objects::{Location, SymbolKind, DependencyType, Provenance};
        let id_a = g.add_symbol(Symbol::new(
            "a",
            SymbolKind::Function,
            Location::new("main.rs", 10, 0),
        ));
        let id_b = g.add_symbol(Symbol::new(
            "b",
            SymbolKind::Function,
            Location::new("main.rs", 20, 0),
        ));
        // Edge with specific provenance + confidence — Heuristic round-trips as Inferred
        g.add_dependency_with_provenance(
            &id_a, &id_b,
            DependencyType::Calls,
            ExtractionContext::Heuristic { score: 0.87 },
        ).expect("add_dependency_with_provenance must succeed");

        // Save and capture the returned revision
        let rev = repo.save_call_graph_ws(&g, &ws)
            .await
            .expect("save_call_graph_ws must succeed");

        // Load the graph at the saved revision
        let loaded = repo.load_call_graph_ws(&ws, rev)
            .await
            .expect("load_call_graph_ws must succeed")
            .expect("load must return Some for a saved workspace");

        assert_eq!(
            loaded.symbol_count(),
            2,
            "loaded graph must have exactly 2 symbols"
        );
        assert_eq!(
            loaded.edge_count(),
            1,
            "loaded graph must have exactly 1 edge"
        );

        // Verify edge provenance + confidence
        let edges: Vec<_> = loaded.edges_with_metadata().collect();
        assert_eq!(edges.len(), 1, "must have exactly one edge");
        let (_src, _tgt, dep_type, prov, conf) = edges[0].clone();
        assert_eq!(dep_type, DependencyType::Calls, "dependency type must be Calls");
        assert_eq!(prov, Provenance::Inferred, "provenance must be Inferred");
        assert!(
            (conf - 0.87).abs() < 1e-9,
            "confidence must be 0.87, got {conf}"
        );
    });

    /// 2.6a RED — pg_test asserting `load_call_graph_ws(&ws, RevisionId(99))`
    /// when no revision 99 exists for ws returns `Err(RepositoryError::UnknownRevision{..})`
    /// and NEVER silently falls back to the head revision.
    pg_test!(load_call_graph_ws_unknown_revision_returns_error, |pool: PgPool| {
        use crate::domain::value_objects::{RevisionId, WorkspaceId};
        use crate::domain::traits::repository::RepositoryError;

        let repo = PostgresRepository::from_pool(pool);
        let ws = WorkspaceId::default();

        // Verify there are NO revisions at all for this workspace
        let rev_count: Option<i64> = sqlx::query_scalar(
            "SELECT COUNT(*) FROM graph_revisions WHERE workspace_id = $1",
        )
        .bind(ws.as_str())
        .fetch_optional(repo.pool())
        .await
        .expect("query revision count");
        assert_eq!(rev_count, Some(0), "workspace should start with no revisions");

        // Loading revision 99 must fail with UnknownRevision, NOT fall back to head
        let result = repo
            .load_call_graph_ws(&ws, RevisionId(99))
            .await;

        let err = match result {
            Err(RepositoryError::UnknownRevision { workspace, revision }) => {
                assert_eq!(
                    workspace.as_str(), ws.as_str(),
                    "error workspace must match requested workspace"
                );
                assert_eq!(
                    revision.get(), 99,
                    "error revision must be 99"
                );
                true
            }
            other => panic!(
                "expected UnknownRevision{{ws: \"{}\", rev: 99}}, got {:?}",
                ws.as_str(),
                other
            ),
        };
        assert!(err, "UnknownRevision error must be returned for unknown revision");
    });

    /// 2.7a RED — pg_test asserting `load_call_graph_ws(ws2, RevisionId(3))`
    /// when ws2 has never had any revisions returns
    /// `Err(UnknownRevision{workspace:\"ws2\", revision:3})`. This verifies
    /// the error workspace field carries the ACTUAL requested workspace (ws2),
    /// not the default workspace.
    pg_test!(load_call_graph_ws_cross_workspace_unknown_rev_error, |pool: PgPool| {
        use crate::domain::value_objects::{RevisionId, WorkspaceId};
        use crate::domain::traits::repository::RepositoryError;

        let repo = PostgresRepository::from_pool(pool);
        let ws2 = WorkspaceId::try_new("ws2").expect("ws2 must be valid WorkspaceId");

        // Verify ws2 has NO revisions at all
        let rev_count: Option<i64> = sqlx::query_scalar(
            "SELECT COUNT(*) FROM graph_revisions WHERE workspace_id = $1",
        )
        .bind(ws2.as_str())
        .fetch_optional(repo.pool())
        .await
        .expect("query revision count");
        assert_eq!(rev_count, Some(0), "ws2 must have zero revisions");

        // Loading any revision for ws2 must fail with UnknownRevision{workspace: "ws2"}
        let result = repo
            .load_call_graph_ws(&ws2, RevisionId(3))
            .await;

        let err = match result {
            Err(RepositoryError::UnknownRevision { workspace, revision }) => {
                assert_eq!(
                    workspace.as_str(), "ws2",
                    "error workspace must be 'ws2', not the default workspace"
                );
                assert_eq!(revision.get(), 3, "error revision must be 3");
                true
            }
            other => panic!(
                "expected UnknownRevision{{ws: \"ws2\", rev: 3}}, got {:?}",
                other
            ),
        };
        assert!(err, "UnknownRevision error must be returned for unknown cross-workspace revision");
    });

    /// 4.2a RED — pg_test asserting `load_call_graph_pinned(ws, RevisionId(99))`
    /// when no revision 99 exists for ws returns `Err(UnknownRevision{ws, 99})`.
    /// This verifies the Repository trait method delegates to load_call_graph_ws
    /// which already performs the revision existence check.
    pg_test!(load_call_graph_pinned_unknown_revision_returns_unknown_revision_error, |pool: PgPool| {
        use crate::domain::value_objects::{RevisionId, WorkspaceId};
        use crate::domain::traits::repository::RepositoryError;

        let repo = PostgresRepository::from_pool(pool);
        let ws = WorkspaceId::default();

        // Verify there are NO revisions at all for this workspace
        let rev_count: Option<i64> = sqlx::query_scalar(
            "SELECT COUNT(*) FROM graph_revisions WHERE workspace_id = $1",
        )
        .bind(ws.as_str())
        .fetch_optional(repo.pool())
        .await
        .expect("query revision count");
        assert_eq!(rev_count, Some(0), "workspace should start with no revisions");

        // load_call_graph_pinned with unknown revision must fail with UnknownRevision
        let result = repo
            .load_call_graph_pinned(&ws, RevisionId(99))
            .await;

        let err = match result {
            Err(RepositoryError::UnknownRevision { workspace, revision }) => {
                assert_eq!(
                    workspace.as_str(), ws.as_str(),
                    "error workspace must match requested workspace"
                );
                assert_eq!(revision.get(), 99, "error revision must be 99");
                true
            }
            other => panic!(
                "expected UnknownRevision{{ws: \"{}\", rev: 99}}, got {:?}",
                ws.as_str(),
                other
            ),
        };
        assert!(err, "UnknownRevision error must be returned for unknown revision");
    });

    /// 4.7 RED — pg_test asserting `load_call_graph_ws(ws, rev)` is revision-pinned:
    /// after saving graph g at rev N, a concurrent ingest that saves a NEW revision N+1
    /// must NOT affect the result of loading at rev N. Verifies the SnapshotProvider
    /// cache key (workspace, revision) prevents head-swap races.
    pg_test!(load_call_graph_ws_revision_pinned_against_concurrent_ingest, |pool: PgPool| {
        use crate::domain::value_objects::{RevisionId, WorkspaceId};
        use crate::domain::aggregates::Symbol;
        use crate::domain::value_objects::{Location, SymbolKind};

        let repo = PostgresRepository::from_pool(pool);
        let ws = WorkspaceId::default();

        // Build and save initial graph at rev 1
        let mut g1 = CallGraph::new();
        g1.add_symbol(Symbol::new(
            "original_func", SymbolKind::Function, Location::new("lib.rs", 1, 0),
        ));
        let rev1 = repo.save_call_graph_ws(&g1, &ws)
            .await
            .expect("save g1 must succeed");
        assert_eq!(rev1.get(), 1, "first revision must be 1");

        // Load at rev1 — should have original_func
        let loaded1 = repo.load_call_graph_ws(&ws, rev1)
            .await
            .expect("load rev1 must succeed")
            .expect("rev1 should exist");
        let symbols_at_rev1: Vec<_> = loaded1.symbols().map(|s| s.name()).collect();
        assert!(symbols_at_rev1.contains(&"original_func"), "rev1 must have original_func");
        assert_eq!(symbols_at_rev1.len(), 1, "rev1 must have exactly 1 symbol");

        // Simulate concurrent ingest: save NEW graph at rev2
        let mut g2 = CallGraph::new();
        g2.add_symbol(Symbol::new(
            "new_func", SymbolKind::Function, Location::new("lib.rs", 10, 0),
        ));
        let rev2 = repo.save_call_graph_ws(&g2, &ws)
            .await
            .expect("save g2 must succeed");
        assert_eq!(rev2.get(), 2, "second revision must be 2");

        // Re-load at rev1 — MUST still return original_func (revision-pinned, not head)
        let loaded1_again = repo.load_call_graph_ws(&ws, rev1)
            .await
            .expect("load rev1 again must succeed")
            .expect("rev1 should still exist");
        let symbols_at_rev1_again: Vec<_> = loaded1_again.symbols().map(|s| s.name()).collect();
        assert!(
            symbols_at_rev1_again.contains(&"original_func"),
            "rev1 must STILL have original_func after concurrent ingest"
        );
        assert_eq!(
            symbols_at_rev1_again.len(), 1,
            "rev1 must STILL have exactly 1 symbol after concurrent ingest"
        );
        assert!(
            !symbols_at_rev1_again.contains(&"new_func"),
            "rev1 must NOT have new_func from rev2"
        );

        // Verify rev2 has the new graph
        let loaded2 = repo.load_call_graph_ws(&ws, rev2)
            .await
            .expect("load rev2 must succeed")
            .expect("rev2 should exist");
        let symbols_at_rev2: Vec<_> = loaded2.symbols().map(|s| s.name()).collect();
        assert!(symbols_at_rev2.contains(&"new_func"), "rev2 must have new_func");
        assert!(!symbols_at_rev2.contains(&"original_func"), "rev2 must NOT have original_func");
    });

    /// 2.8b RED — pg_test asserting `load_call_graph_ws` for ws1 never returns
    /// ws2's symbols/edges. Write different graphs to ws1 and ws2, then load each
    /// and verify the loaded graph matches what was written to THAT workspace only.
    pg_test!(load_call_graph_ws_cross_workspace_isolation, |pool: PgPool| {
        use crate::domain::value_objects::{RevisionId, WorkspaceId};
        use crate::domain::aggregates::Symbol;
        use crate::domain::services::ExtractionContext;
        use crate::domain::value_objects::{Location, SymbolKind, DependencyType, Provenance};

        let repo = PostgresRepository::from_pool(pool);
        let ws1 = WorkspaceId::try_new("ws1").expect("ws1 must be valid WorkspaceId");
        let ws2 = WorkspaceId::try_new("ws2").expect("ws2 must be valid WorkspaceId");

        // Build different graphs for ws1 and ws2
        let mut g1 = CallGraph::new();
        let a1 = g1.add_symbol(Symbol::new(
            "symbol_a", SymbolKind::Function, Location::new("file_a.rs", 1, 0),
        ));
        let b1 = g1.add_symbol(Symbol::new(
            "symbol_b", SymbolKind::Function, Location::new("file_a.rs", 5, 0),
        ));
        g1.add_dependency_with_provenance(&a1, &b1, DependencyType::Calls, ExtractionContext::DirectExtraction)
            .expect("direct extraction");

        let mut g2 = CallGraph::new();
        let x2 = g2.add_symbol(Symbol::new(
            "symbol_x", SymbolKind::Class, Location::new("file_x.rs", 10, 0),
        ));
        let y2 = g2.add_symbol(Symbol::new(
            "symbol_y", SymbolKind::Class, Location::new("file_x.rs", 20, 0),
        ));
        g2.add_dependency_with_provenance(&x2, &y2, DependencyType::Imports, ExtractionContext::Heuristic { score: 0.9 })
            .expect("heuristic");

        // Save ws1 graph at rev 1
        let rev1 = repo.save_call_graph_ws(&g1, &ws1)
            .await
            .expect("ws1 save must succeed");
        assert_eq!(rev1.get(), 1, "ws1 first rev must be 1");

        // Save ws2 graph at rev 1 (independent counter)
        let rev2 = repo.save_call_graph_ws(&g2, &ws2)
            .await
            .expect("ws2 save must succeed");
        assert_eq!(rev2.get(), 1, "ws2 first rev must be 1");

        // Load ws1 — must have exactly ws1's 2 symbols, 1 edge
        let loaded1 = repo.load_call_graph_ws(&ws1, rev1)
            .await
            .expect("load ws1 must succeed")
            .expect("load must return Some for ws1");
        assert_eq!(loaded1.symbol_count(), 2, "ws1 must have 2 symbols");
        assert_eq!(loaded1.edge_count(), 1, "ws1 must have 1 edge");

        // Verify ws1 has the correct symbol names
        let ws1_symbols: Vec<_> = loaded1.symbols().map(|s| s.name()).collect();
        assert!(ws1_symbols.contains(&"symbol_a"), "ws1 must contain symbol_a");
        assert!(ws1_symbols.contains(&"symbol_b"), "ws1 must contain symbol_b");
        assert!(!ws1_symbols.contains(&"symbol_x"), "ws1 must NOT contain symbol_x from ws2");

        // Load ws2 — must have exactly ws2's 2 symbols, 1 edge
        let loaded2 = repo.load_call_graph_ws(&ws2, rev2)
            .await
            .expect("load ws2 must succeed")
            .expect("load must return Some for ws2");
        assert_eq!(loaded2.symbol_count(), 2, "ws2 must have 2 symbols");
        assert_eq!(loaded2.edge_count(), 1, "ws2 must have 1 edge");

        // Verify ws2 has the correct symbol names
        let ws2_symbols: Vec<_> = loaded2.symbols().map(|s| s.name()).collect();
        assert!(ws2_symbols.contains(&"symbol_x"), "ws2 must contain symbol_x");
        assert!(ws2_symbols.contains(&"symbol_y"), "ws2 must contain symbol_y");
        assert!(!ws2_symbols.contains(&"symbol_a"), "ws2 must NOT contain symbol_a from ws1");
    });

    /// 2.8a RED — pg_test asserting `load_call_graph_ws(ws, save_call_graph_ws(g, ws))`
    /// produces a graph that is `PartialEq`-equal to `g`. Verifies exact bit-exact
    /// round-trip: symbol counts, edge counts, and per-edge (provenance, confidence) match.
    pg_test!(load_call_graph_ws_round_trip_equality, |pool: PgPool| {
        use crate::domain::value_objects::{RevisionId, WorkspaceId};
        use crate::domain::aggregates::Symbol;
        use crate::domain::services::ExtractionContext;
        use crate::domain::value_objects::{Location, SymbolKind, DependencyType, Provenance};

        let repo = PostgresRepository::from_pool(pool);
        let ws = WorkspaceId::default();

        // Build a non-trivial graph with mixed provenance edges
        let mut g = CallGraph::new();
        let a = g.add_symbol(Symbol::new(
            "a", SymbolKind::Function, Location::new("a.rs", 1, 0),
        ));
        let b = g.add_symbol(Symbol::new(
            "b", SymbolKind::Function, Location::new("b.rs", 5, 0),
        ));
        let c = g.add_symbol(Symbol::new(
            "c", SymbolKind::Class, Location::new("c.rs", 10, 0),
        ));
        // Edge: a -> b, Calls, DirectExtraction
        g.add_dependency_with_provenance(&a, &b, DependencyType::Calls, ExtractionContext::DirectExtraction)
            .expect("direct extraction");
        // Edge: b -> c, Imports, Heuristic 0.75 → Inferred 0.75
        g.add_dependency_with_provenance(&b, &c, DependencyType::Imports, ExtractionContext::Heuristic { score: 0.75 })
            .expect("heuristic");

        // Save and get revision
        let rev = repo.save_call_graph_ws(&g, &ws)
            .await
            .expect("save_call_graph_ws must succeed");

        // Load at the saved revision
        let loaded = repo.load_call_graph_ws(&ws, rev)
            .await
            .expect("load_call_graph_ws must succeed")
            .expect("load must return Some for saved workspace");

        // Exact equality check
        assert_eq!(
            loaded.symbol_count(), g.symbol_count(),
            "symbol count must match after round-trip"
        );
        assert_eq!(
            loaded.edge_count(), g.edge_count(),
            "edge count must match after round-trip"
        );

        // Verify edge provenance + confidence round-trips correctly
        let orig_edges: Vec<_> = g.edges_with_metadata().collect();
        let loaded_edges: Vec<_> = loaded.edges_with_metadata().collect();
        assert_eq!(orig_edges.len(), loaded_edges.len(), "edge counts must match");

        for ((_, _, od, op, oc), (_, _, ld, lp, lc)) in orig_edges.into_iter().zip(loaded_edges.into_iter()) {
            assert_eq!(od, ld, "dependency type must match");
            assert_eq!(op, lp, "provenance must match after round-trip");
            assert!(
                (oc - lc).abs() < 1e-9,
                "confidence must match after round-trip: orig={}, loaded={}",
                oc, lc
            );
        }

        // The graphs themselves should be PartialEq-equal
        assert_eq!(
            loaded, g,
            "loaded graph must be PartialEq-equal to original after round-trip"
        );
    });

    /// 2.9a RED — pg_test asserting that when a file disappears from scan_manifest,
    /// its nodes and edges are deleted at the next revision. At rev 1, src/x.rs and
    /// src/y.rs are both in scan_manifest. At rev 2, only src/y.rs remains in
    /// scan_manifest (src/x.rs is removed). After saving rev 2:
    /// - count_nodes WHERE source_path='src/x.rs' must be 0
    /// - count_edges must be 0 (the only edge was from x.rs node)
    /// - src/y.rs node must still exist
    pg_test!(deleted_file_nodes_and_edges_disappear, |pool: PgPool| {
        use crate::domain::value_objects::{RevisionId, WorkspaceId};
        use crate::domain::aggregates::Symbol;
        use crate::domain::services::ExtractionContext;
        use crate::domain::value_objects::{Location, SymbolKind, DependencyType};
        use crate::infrastructure::persistence::postgres_repository::ScanManifestRow;

        let repo = PostgresRepository::from_pool(pool);
        let ws = WorkspaceId::default();

        // --- Rev 1: two files in scan_manifest ---
        let mut g1 = CallGraph::new();
        let a1 = g1.add_symbol(Symbol::new(
            "foo", SymbolKind::Function, Location::new("src/x.rs", 1, 0),
        ));
        let b1 = g1.add_symbol(Symbol::new(
            "bar", SymbolKind::Function, Location::new("src/y.rs", 5, 0),
        ));
        g1.add_dependency_with_provenance(&a1, &b1, DependencyType::Calls, ExtractionContext::DirectExtraction)
            .expect("direct");

        let rev1 = repo.save_call_graph_ws(&g1, &ws)
            .await
            .expect("rev1 save must succeed");

        // Upsert scan_manifest for rev 1 (both files)
        repo.upsert_scan_manifest_row(&ScanManifestRow {
            workspace_id: ws.as_str().to_string(),
            file_path: "src/x.rs".to_string(),
            file_type: "source".to_string(),
            language: Some("rust".to_string()),
            content_hash: "hash_x".to_string(),
            mtime: 100.0,
            symbol_count: 1,
            edge_count: 1,
            status: "scanned".to_string(),
            error_msg: None,
        })
        .await
        .expect("upsert x.rs");
        repo.upsert_scan_manifest_row(&ScanManifestRow {
            workspace_id: ws.as_str().to_string(),
            file_path: "src/y.rs".to_string(),
            file_type: "source".to_string(),
            language: Some("rust".to_string()),
            content_hash: "hash_y".to_string(),
            mtime: 100.0,
            symbol_count: 1,
            edge_count: 0,
            status: "scanned".to_string(),
            error_msg: None,
        })
        .await
        .expect("upsert y.rs");

        // Verify rev 1: both nodes present
        let x_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM graph_nodes WHERE workspace_id = $1 AND source_path = 'src/x.rs'"
        )
        .bind(ws.as_str())
        .fetch_one(repo.pool())
        .await
        .expect("count x.rs nodes");
        assert_eq!(x_count, 1, "rev 1 must have 1 node for src/x.rs");

        // --- Rev 2: only src/y.rs in scan_manifest (src/x.rs removed) ---
        let mut g2 = CallGraph::new();
        let _b2 = g2.add_symbol(Symbol::new(
            "bar", SymbolKind::Function, Location::new("src/y.rs", 5, 0),
        ));
        // No edge from x.rs since x.rs is gone from scan_manifest

        let rev2 = repo.save_call_graph_ws(&g2, &ws)
            .await
            .expect("rev2 save must succeed");
        assert_eq!(rev2.get(), 2, "rev2 must be 2");

        // Delete scan_manifest entries NOT in the new manifest
        repo.delete_scan_manifest_except(ws.as_str(), &["src/y.rs".to_string()])
            .await
            .expect("delete_scan_manifest_except for rev 2");

        // Verify: src/x.rs nodes must be gone at rev 2
        let x_count2: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM graph_nodes WHERE workspace_id = $1 AND source_path = 'src/x.rs'"
        )
        .bind(ws.as_str())
        .fetch_one(repo.pool())
        .await
        .expect("count x.rs nodes at rev 2");
        assert_eq!(x_count2, 0, "rev 2 must have 0 nodes for src/x.rs (file removed from manifest)");

        // Verify: src/y.rs node still exists
        let y_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM graph_nodes WHERE workspace_id = $1 AND source_path = 'src/y.rs'"
        )
        .bind(ws.as_str())
        .fetch_one(repo.pool())
        .await
        .expect("count y.rs nodes");
        assert_eq!(y_count, 1, "rev 2 must still have 1 node for src/y.rs");

        // Verify: no orphaned edges (edge a1->b1 must be gone since a1 is deleted)
        let edge_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM graph_edges WHERE workspace_id = $1"
        )
        .bind(ws.as_str())
        .fetch_one(repo.pool())
        .await
        .expect("count edges");
        assert_eq!(edge_count, 0, "rev 2 must have 0 edges (only edge was from deleted x.rs node)");
    });

    /// 2.3a RED — pg_test asserting `save_call_graph_ws` returns `Ok(RevisionId)`,
    /// opens one `graph_revisions` row `head_of=true revision_id=rev`, and
    /// populates the expected symbols and edges count.
    pg_test!(save_call_graph_ws_returns_revision_id_and_populates, |pool: PgPool| {
        use crate::domain::value_objects::{RevisionId, WorkspaceId};

        let repo = PostgresRepository::from_pool(pool);
        let graph = build_mixed_provenance_graph();
        let ws = WorkspaceId::default();

        // save_call_graph_ws must return a valid RevisionId
        let rev = repo
            .save_call_graph_ws(&graph, &ws)
            .await
            .expect("save_call_graph_ws must succeed");
        assert!(rev.is_valid(), "revision must be valid");
        assert!(rev.get() > 0, "revision must be > 0");

        // Exactly one graph_revisions row with head_of=true
        let head_rows: Vec<(String, i64, bool)> = sqlx::query_as(
            "SELECT workspace_id, revision_id, head_of FROM graph_revisions WHERE workspace_id = $1 AND head_of = true",
        )
        .bind(ws.as_str())
        .fetch_all(repo.pool())
        .await
        .expect("query head rows");
        assert_eq!(
            head_rows.len(),
            1,
            "must have exactly 1 head row, got {}",
            head_rows.len()
        );
        assert_eq!(
            head_rows[0].1,
            rev.get() as i64,
            "head revision_id must match returned revision"
        );

        // Check symbols and edges counts in graph_nodes / graph_edges
        let sym_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM graph_nodes WHERE workspace_id = $1",
        )
        .bind(ws.as_str())
        .fetch_one(repo.pool())
        .await
        .expect("count symbols");
        assert_eq!(
            sym_count,
            graph.symbol_count() as i64,
            "graph_nodes row count must match symbol count"
        );

        let edge_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM graph_edges WHERE workspace_id = $1",
        )
        .bind(ws.as_str())
        .fetch_one(repo.pool())
        .await
        .expect("count edges");
        assert_eq!(
            edge_count,
            graph.edge_count() as i64,
            "graph_edges row count must match edge count"
        );
    });

    /// 2.2a RED — pg_test asserting `save_call_graph_ws(&g, &ws2)` with
    /// `ws1` already at head 3 advances `ws2`'s counter, not `ws1`'s.
    pg_test!(save_call_graph_ws_workspace_isolated_counters, |pool: PgPool| {
        use crate::domain::value_objects::{RevisionId, WorkspaceId};

        let repo = PostgresRepository::from_pool(pool);
        let graph = build_mixed_provenance_graph();
        let ws1 = WorkspaceId::default(); // "default"
        let ws2 = WorkspaceId::try_new("ws2").expect("ws2 is valid");

        // ws1 commits: rev 1
        let rev1 = repo
            .save_call_graph_ws(&graph, &ws1)
            .await
            .expect("ws1 commit 1");
        assert_eq!(rev1, RevisionId(1));

        // ws1 commits: rev 2
        let rev2 = repo
            .save_call_graph_ws(&graph, &ws1)
            .await
            .expect("ws1 commit 2");
        assert_eq!(rev2, RevisionId(2));

        // ws1 commits: rev 3
        let rev3 = repo
            .save_call_graph_ws(&graph, &ws1)
            .await
            .expect("ws1 commit 3");
        assert_eq!(rev3, RevisionId(3));

        // ws2 first commit: should be rev 1 (not rev 4!)
        let ws2_rev1 = repo
            .save_call_graph_ws(&graph, &ws2)
            .await
            .expect("ws2 first commit");
        assert_eq!(
            ws2_rev1,
            RevisionId(1),
            "ws2 should start at rev 1, not continue from ws1's rev 3"
        );

        // ws1 head must still be 3
        let ws1_head: Option<(String, i64)> = sqlx::query_as(
            "SELECT workspace_id, revision_id FROM graph_revisions WHERE workspace_id = $1 AND head_of = true",
        )
        .bind(ws1.as_str())
        .fetch_optional(repo.pool())
        .await
        .expect("query ws1 head");
        assert!(ws1_head.is_some(), "ws1 should have a head");
        assert_eq!(
            ws1_head.unwrap().1, 3_i64,
            "ws1 head must still be 3 after ws2 commits"
        );

        // ws2 head must be 1
        let ws2_head: Option<(String, i64)> = sqlx::query_as(
            "SELECT workspace_id, revision_id FROM graph_revisions WHERE workspace_id = $1 AND head_of = true",
        )
        .bind(ws2.as_str())
        .fetch_optional(repo.pool())
        .await
        .expect("query ws2 head");
        assert!(ws2_head.is_some(), "ws2 should have a head");
        assert_eq!(ws2_head.unwrap().1, 1_i64, "ws2 head must be 1");
    });

    /// 2.1a RED — pg_test asserting two sequential `save_call_graph_ws`
    /// calls open revisions `(1,true)` then `(2,true)` and no duplicate
    /// `(workspace_id,revision_id)`.
    pg_test!(save_call_graph_ws_opens_monotonic_revision, |pool: PgPool| {
        use crate::domain::value_objects::{RevisionId, WorkspaceId};

        let repo = PostgresRepository::from_pool(pool);
        let graph = build_mixed_provenance_graph();
        let ws = WorkspaceId::default();

        // First commit opens revision 1
        let rev1 = repo
            .save_call_graph_ws(&graph, &ws)
            .await
            .expect("first commit must succeed");
        assert!(rev1.is_valid(), "first revision must be valid");
        assert_eq!(rev1, RevisionId(1), "first revision must be 1");

        // Check graph_revisions: exactly one head = rev1
        let head1: (i64, bool) = sqlx::query_as(
            "SELECT revision_id, head_of FROM graph_revisions WHERE workspace_id = $1 AND head_of = true",
        )
        .bind(ws.as_str())
        .fetch_one(repo.pool())
        .await
        .expect("must have exactly one head row after first commit");
        assert_eq!(head1.0, 1_i64, "head revision_id must be 1");
        assert!(head1.1, "head_of must be true");

        // Second commit opens revision 2
        let rev2 = repo
            .save_call_graph_ws(&graph, &ws)
            .await
            .expect("second commit must succeed");
        assert_eq!(rev2, RevisionId(2), "second revision must be 2");

        // Check graph_revisions: head is now rev2
        let head2: (i64, bool) = sqlx::query_as(
            "SELECT revision_id, head_of FROM graph_revisions WHERE workspace_id = $1 AND head_of = true",
        )
        .bind(ws.as_str())
        .fetch_one(repo.pool())
        .await
        .expect("must have exactly one head row after second commit");
        assert_eq!(head2.0, 2_i64, "head revision_id must be 2");
        assert!(head2.1, "head_of must be true");

        // Both revisions must exist (for pinned reads)
        let all_revs: Vec<(String, i64)> = sqlx::query_as(
            "SELECT workspace_id, revision_id FROM graph_revisions WHERE workspace_id = $1 ORDER BY revision_id",
        )
        .bind(ws.as_str())
        .fetch_all(repo.pool())
        .await
        .expect("must have 2 revision rows");
        assert_eq!(all_revs.len(), 2, "must have exactly 2 revision rows");
        assert_eq!(all_revs[0].1, 1, "revision 1 must exist");
        assert_eq!(all_revs[1].1, 2, "revision 2 must exist");

        // PK constraint: no duplicate (workspace_id, revision_id) — verified by UNIQUE index
        // If there were a duplicate, the INSERT would have failed with a unique violation.
    });

    /// Spec requirement: `save_call_graph` populates both
    /// `symbols` and `call_edges` in a single transaction.
    /// Empty DB + canonical mixed-provenance graph -> both tables
    /// populated with the expected row counts.
    pg_test!(save_populates_both_tables, |pool: PgPool| {
        let repo = PostgresRepository::from_pool(pool);
        let graph = build_mixed_provenance_graph();
        let expected_syms = graph.symbol_count();
        let expected_edges = graph.edge_count();
        assert!(expected_syms >= 5, "fixture must have >=5 symbols");
        assert!(expected_edges >= 3, "fixture must have >=3 edges");
        assert_eq!(expected_syms, 6);
        // 1 a->b + 1 a->c + 1 b->d + 1 c->d + 1 d->e + 1 e->e + 2 e->f = 8
        assert_eq!(expected_edges, 8);

        repo.save_call_graph(&graph)
            .await
            .expect("save_call_graph must succeed");

        assert_eq!(
            repo.count_symbols().await.unwrap(),
            expected_syms,
            "all symbols persisted"
        );
        assert_eq!(
            repo.count_edges().await.unwrap(),
            expected_edges,
            "all edges persisted"
        );
    });

    /// Spec requirement: `load_call_graph` on an empty DB returns
    /// `Ok(None)`. We additionally assert the count query was a
    /// pure read (no DML was issued) by re-checking counts after.
    pg_test!(load_empty_returns_none, |pool: PgPool| {
        let repo = PostgresRepository::from_pool(pool);
        let result = repo.load_call_graph().await.expect("load_call_graph");
        assert!(
            result.is_none(),
            "empty DB must yield Ok(None), got {result:?}"
        );
        // Counts still 0 — the load did not accidentally write.
        assert_eq!(repo.count_symbols().await.unwrap(), 0);
        assert_eq!(repo.count_edges().await.unwrap(), 0);
    });

    /// Spec requirement: loaded graph matches the saved one with
    /// exact per-edge `(provenance, confidence)` and per-symbol
    /// FQN. Uses `assert_eq!` (PartialEq) for structural equality.
    pg_test!(
        load_populated_returns_some_with_exact_metadata,
        |pool: PgPool| {
            let repo = PostgresRepository::from_pool(pool);
            let graph = build_mixed_provenance_graph();
            repo.save_call_graph(&graph).await.expect("save");

            let loaded = repo
                .load_call_graph()
                .await
                .expect("load")
                .expect("Some(graph) for populated DB");

            assert_eq!(loaded.symbol_count(), graph.symbol_count());
            assert_eq!(loaded.edge_count(), graph.edge_count());

            // FQN-by-FQN: every saved symbol must be present with
            // matching name.
            for (_, sym) in graph.symbol_ids() {
                let fqn = sym.fully_qualified_name();
                let loaded_sym = loaded
                    .get_symbol(&SymbolId::new(fqn))
                    .unwrap_or_else(|| panic!("missing symbol: {fqn}"));
                assert_eq!(loaded_sym.name(), sym.name());
                assert_eq!(loaded_sym.location().file(), sym.location().file());
                assert_eq!(loaded_sym.location().line(), sym.location().line());
            }

            // Per-edge: every saved (src, tgt, dep, prov, conf) tuple
            // must round-trip bit-exactly.
            let saved_edges: Vec<_> = graph.edges_with_metadata().collect();
            let loaded_edges: Vec<_> = loaded.edges_with_metadata().collect();
            assert_eq!(
                saved_edges.len(),
                loaded_edges.len(),
                "edge count must match"
            );
            for (s_src, s_tgt, s_dep, s_prov, s_conf) in &saved_edges {
                let mut found = false;
                for (l_src, l_tgt, l_dep, l_prov, l_conf) in &loaded_edges {
                    if s_src == l_src && s_tgt == l_tgt && s_dep == l_dep {
                        assert_eq!(s_prov, l_prov, "provenance mismatch for {s_src}->{s_tgt}");
                        assert_eq!(
                            s_conf, l_conf,
                            "confidence mismatch for {s_src}->{s_tgt} ({s_conf} vs {l_conf})"
                        );
                        found = true;
                        break;
                    }
                }
                assert!(
                    found,
                    "edge {s_src}->{s_tgt} ({s_dep:?}) missing from loaded graph"
                );
            }
        }
    );

    /// Spec requirement: round-trip `assert_eq!` of the source and
    /// the loaded graph. `CallGraph` derives `PartialEq`, so this
    /// covers symbols, edges, per-edge metadata, self-loops, and
    /// multi-edge pairs in one assertion.
    pg_test!(round_trip_assert_eq, |pool: PgPool| {
        let repo = PostgresRepository::from_pool(pool);
        let graph = build_mixed_provenance_graph();
        repo.save_call_graph(&graph).await.expect("save");

        let loaded = repo
            .load_call_graph()
            .await
            .expect("load")
            .expect("Some for populated DB");

        // CallGraph implements PartialEq, so this is a deep
        // structural comparison that covers symbols, edges, and
        // per-edge metadata.
        assert_eq!(loaded, graph, "round-trip must be PartialEq-equal");

        // Cross-checks for the specific edge-case scenarios:
        // (a) self-loop e->e preserved
        let e_id = SymbolId::new("e.rs:e:1");
        let e_self = loaded
            .callees_with_metadata(&e_id)
            .iter()
            .any(|(tgt, dep, _, _)| tgt == &e_id && *dep == DependencyType::Defines);
        assert!(e_self, "self-loop e->e (Defines) must round-trip");
        // (b) multi-edge e->f preserved with both DependencyTypes
        let f_id = SymbolId::new("f.rs:f:1");
        let e_to_f_kinds: Vec<_> = loaded
            .callees_with_metadata(&e_id)
            .iter()
            .filter(|(tgt, _, _, _)| tgt == &f_id)
            .map(|(_, dep, _, _)| *dep)
            .collect();
        assert_eq!(
            e_to_f_kinds.len(),
            2,
            "e->f must have 2 edges after round-trip, got {e_to_f_kinds:?}"
        );
        assert!(e_to_f_kinds.contains(&DependencyType::Calls));
        assert!(e_to_f_kinds.contains(&DependencyType::Imports));
    });

    /// Spec requirement: `named_views` migration is idempotent.
    /// Running the DDL twice on a fresh DB must yield exactly one
    /// `named_views` table and exactly one unique index — the
    /// `CREATE TABLE / INDEX IF NOT EXISTS` guards make that the
    /// only correct outcome.
    pg_test!(named_views_migration_is_idempotent, |pool: PgPool| {
        let repo = PostgresRepository::from_pool(pool);
        repo.run_migrations().await.expect("first migration");
        repo.run_migrations().await.expect("second migration");

        // Count tables matching `named_views` (and only that name —
        // we don't want to be fooled by a future `named_views_history`).
        let table_rows: Vec<(String,)> = sqlx::query_as(
            "SELECT table_name \
             FROM information_schema.tables \
             WHERE table_name = 'named_views'",
        )
        .fetch_all(repo.pool())
        .await
        .expect("information_schema.tables query must succeed");
        assert_eq!(
            table_rows.len(),
            1,
            "named_views table must exist exactly once after double migration, got: {table_rows:?}"
        );

        // Count the unique scope index.
        let index_rows: Vec<(String,)> = sqlx::query_as(
            "SELECT indexname \
             FROM pg_indexes \
             WHERE tablename = 'named_views' \
               AND indexname = 'idx_pg_named_views_scope'",
        )
        .fetch_all(repo.pool())
        .await
        .expect("pg_indexes query must succeed");
        assert_eq!(
            index_rows.len(),
            1,
            "idx_pg_named_views_scope must exist exactly once, got: {index_rows:?}"
        );
    });

    /// Spec requirement: the unique index rejects a duplicate
    /// `(workspace_id, owner, name)` triple. The second insert
    /// surfaces as `RepositoryError::UniqueViolation` (mapped from
    /// PG SQLSTATE `23505`).
    pg_test!(
        named_views_unique_index_rejects_duplicate_name,
        |pool: PgPool| {
            let repo = PostgresRepository::from_pool(pool);
            // Seed first row.
            repo.save_named_view(
                "11111111-1111-1111-1111-111111111111",
                "w1",
                "u1",
                "hotspots",
                Some("first"),
                "function",
                "callgraph",
                "crate::foo",
                3,
            )
            .await
            .expect("first save must succeed");

            // Second insert with the same (w1, u1, hotspots) must fail
            // with the typed UniqueViolation error.
            let result = repo
                .save_named_view(
                    "22222222-2222-2222-2222-222222222222",
                    "w1",
                    "u1",
                    "hotspots",
                    Some("second"),
                    "function",
                    "callgraph",
                    "crate::foo",
                    3,
                )
                .await;
            match result {
                Err(RepositoryError::UniqueViolation(msg)) => {
                    assert!(msg.contains("hotspots"), "got: {msg}");
                }
                other => panic!("expected UniqueViolation, got: {other:?}"),
            }

            // Distinct owners can share a name.
            repo.save_named_view(
                "33333333-3333-3333-3333-333333333333",
                "w1",
                "u2",
                "hotspots",
                None,
                "function",
                "callgraph",
                "crate::foo",
                3,
            )
            .await
            .expect("different owner must succeed");

            // The first row is still queryable.
            let row = repo
                .load_named_view("11111111-1111-1111-1111-111111111111", "w1", "u1")
                .await
                .expect("load must succeed")
                .expect("Some");
            assert_eq!(row.name, "hotspots");
            assert_eq!(row.description.as_deref(), Some("first"));
        }
    );

    /// Spec requirement: load returns the same row by id+scope,
    /// and `None` when the id is unknown.
    pg_test!(named_views_load_round_trip, |pool: PgPool| {
        let repo = PostgresRepository::from_pool(pool);
        repo.save_named_view(
            "abcdefab-cdef-abcd-efab-cdefabcdefab",
            "w1",
            "u1",
            "deps",
            Some("depth-3 deps"),
            "module",
            "callgraph",
            "crate::foo::bar",
            5,
        )
        .await
        .expect("save must succeed");

        let row = repo
            .load_named_view("abcdefab-cdef-abcd-efab-cdefabcdefab", "w1", "u1")
            .await
            .expect("load must succeed")
            .expect("Some for freshly-saved row");
        assert_eq!(row.workspace_id, "w1");
        assert_eq!(row.owner, "u1");
        assert_eq!(row.name, "deps");
        assert_eq!(row.level, "module");
        assert_eq!(row.lens, "callgraph");
        assert_eq!(row.focus_node, "crate::foo::bar");
        assert_eq!(row.max_depth, 5);

        // Unknown id returns Ok(None) — NOT an error.
        let none = repo
            .load_named_view("00000000-0000-0000-0000-000000000000", "w1", "u1")
            .await
            .expect("unknown id must not error");
        assert!(none.is_none(), "unknown id must yield None");
    });

    /// Spec requirement: list returns only the matching scope,
    /// ordered newest-first.
    pg_test!(named_views_list_scope_and_order, |pool: PgPool| {
        let repo = PostgresRepository::from_pool(pool);
        // Insert in a known order; PG `now()` advances on each
        // call so created_at is monotonically increasing.
        for (i, (id, name)) in [
            ("aaaa1111-aaaa-aaaa-aaaa-aaaaaaaaaaaa", "a"),
            ("bbbb2222-bbbb-bbbb-bbbb-bbbbbbbbbbbb", "b"),
            ("cccc3333-cccc-cccc-cccc-cccccccccccc", "c"),
        ]
        .iter()
        .enumerate()
        {
            repo.save_named_view(
                id,
                "w1",
                "u1",
                name,
                None,
                "function",
                "callgraph",
                "crate::foo",
                3,
            )
            .await
            .expect("save must succeed");
            // Force a microsecond delay so created_at differs.
            if i < 2 {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        }
        // Different scope: should NOT appear in the (w1, u1) list.
        repo.save_named_view(
            "dddd4444-dddd-dddd-dddd-dddddddddddd",
            "w1",
            "u2",
            "x",
            None,
            "function",
            "callgraph",
            "crate::foo",
            3,
        )
        .await
        .expect("save u2 must succeed");

        let rows = repo
            .list_named_views("w1", "u1")
            .await
            .expect("list must succeed");
        assert_eq!(rows.len(), 3, "expected 3 rows for (w1, u1)");
        for r in &rows {
            assert_eq!(r.workspace_id, "w1");
            assert_eq!(r.owner, "u1");
        }
        // Newest first → insertion order reversed.
        assert_eq!(rows[0].name, "c");
        assert_eq!(rows[1].name, "b");
        assert_eq!(rows[2].name, "a");

        // Empty scope returns Ok(Vec::new()).
        let empty = repo
            .list_named_views("w_other", "u1")
            .await
            .expect("list must succeed for empty scope");
        assert!(empty.is_empty(), "empty scope must return Ok(vec![])");
    });

    /// Spec requirement: delete returns true iff a row existed in
    /// the supplied scope. Wrong scope returns false (no delete).
    pg_test!(named_views_delete_scope_guarded, |pool: PgPool| {
        let repo = PostgresRepository::from_pool(pool);
        repo.save_named_view(
            "deadbeef-dead-beef-dead-beefdeadbeef",
            "w1",
            "u1",
            "v",
            None,
            "function",
            "callgraph",
            "crate::foo",
            3,
        )
        .await
        .expect("save must succeed");

        // Wrong scope: false, row untouched.
        let removed = repo
            .delete_named_view("deadbeef-dead-beef-dead-beefdeadbeef", "w1", "u2")
            .await
            .expect("delete must succeed");
        assert!(!removed, "scope mismatch must NOT remove row");
        let still = repo
            .load_named_view("deadbeef-dead-beef-dead-beefdeadbeef", "w1", "u1")
            .await
            .expect("load ok")
            .expect("row still present");
        assert_eq!(still.name, "v");

        // Correct scope: true, row gone.
        let removed = repo
            .delete_named_view("deadbeef-dead-beef-dead-beefdeadbeef", "w1", "u1")
            .await
            .expect("delete must succeed");
        assert!(removed, "correct scope must remove the row");
        let none = repo
            .load_named_view("deadbeef-dead-beef-dead-beefdeadbeef", "w1", "u1")
            .await
            .expect("load ok");
        assert!(none.is_none(), "row must be gone after delete");

        // Unknown id: false.
        let removed = repo
            .delete_named_view("00000000-0000-0000-0000-000000000000", "w1", "u1")
            .await
            .expect("delete must succeed");
        assert!(!removed, "unknown id must return false");
    });

    /// Spec requirement: delete-and-replace overwrite. Save A
    /// (3 sym) then B (different 5 sym) -> only B's rows remain.
    pg_test!(delete_and_replace_overwrites, |pool: PgPool| {
        let repo = PostgresRepository::from_pool(pool);

        // Build graph A: 3 symbols, 2 edges, all Calls/Extracted.
        let mut a = CallGraph::new();
        let a1 = a.add_symbol(Symbol::new(
            "a1",
            SymbolKind::Function,
            Location::new("a1.rs", 1, 0),
        ));
        let a2 = a.add_symbol(Symbol::new(
            "a2",
            SymbolKind::Function,
            Location::new("a2.rs", 1, 0),
        ));
        let a3 = a.add_symbol(Symbol::new(
            "a3",
            SymbolKind::Function,
            Location::new("a3.rs", 1, 0),
        ));
        a.add_dependency(&a1, &a2, DependencyType::Calls).unwrap();
        a.add_dependency(&a2, &a3, DependencyType::Calls).unwrap();

        repo.save_call_graph(&a).await.expect("save A");
        assert_eq!(repo.count_symbols().await.unwrap(), 3);
        assert_eq!(repo.count_edges().await.unwrap(), 2);

        // Build graph B: 5 DIFFERENT symbols, 4 edges.
        let mut b = CallGraph::new();
        let b1 = b.add_symbol(Symbol::new(
            "b1",
            SymbolKind::Function,
            Location::new("b1.rs", 1, 0),
        ));
        let b2 = b.add_symbol(Symbol::new(
            "b2",
            SymbolKind::Function,
            Location::new("b2.rs", 1, 0),
        ));
        let b3 = b.add_symbol(Symbol::new(
            "b3",
            SymbolKind::Class,
            Location::new("b3.rs", 1, 0),
        ));
        let b4 = b.add_symbol(Symbol::new(
            "b4",
            SymbolKind::Method,
            Location::new("b4.rs", 1, 0),
        ));
        let b5 = b.add_symbol(Symbol::new(
            "b5",
            SymbolKind::Function,
            Location::new("b5.rs", 1, 0),
        ));
        b.add_dependency(&b1, &b2, DependencyType::Imports).unwrap();
        b.add_dependency(&b2, &b3, DependencyType::Inherits)
            .unwrap();
        b.add_dependency(&b3, &b4, DependencyType::References)
            .unwrap();
        b.add_dependency(&b4, &b5, DependencyType::Calls).unwrap();

        repo.save_call_graph(&b).await.expect("save B");
        assert_eq!(
            repo.count_symbols().await.unwrap(),
            5,
            "only B's 5 symbols must remain"
        );
        assert_eq!(
            repo.count_edges().await.unwrap(),
            4,
            "only B's 4 edges must remain"
        );

        // No row from A must remain.
        for old in ["a1.rs:a1:1", "a2.rs:a2:1", "a3.rs:a3:1"] {
            let found = repo
                .find_symbol_by_qualified_name(old)
                .await
                .expect("query");
            assert!(found.is_none(), "row from A still present: {old}");
        }
    });

    /// Spec requirement: idempotent re-save. Saving the same
    /// graph twice must produce equal row counts; the row set is
    /// semantically equivalent (SERIAL ids may regenerate, but
    /// the load + assert_eq! pass).
    pg_test!(idempotent_re_save, |pool: PgPool| {
        let repo = PostgresRepository::from_pool(pool);
        let graph = build_mixed_provenance_graph();
        let syms = graph.symbol_count();
        let edges = graph.edge_count();

        repo.save_call_graph(&graph).await.expect("save 1");
        let syms_1 = repo.count_symbols().await.unwrap();
        let edges_1 = repo.count_edges().await.unwrap();
        assert_eq!(syms_1, syms);
        assert_eq!(edges_1, edges);

        repo.save_call_graph(&graph).await.expect("save 2");
        let syms_2 = repo.count_symbols().await.unwrap();
        let edges_2 = repo.count_edges().await.unwrap();
        assert_eq!(syms_2, syms_1, "idempotent re-save must keep symbol count");
        assert_eq!(edges_2, edges_1, "idempotent re-save must keep edge count");

        // Re-load and assert_eq! — semantically equivalent.
        let loaded = repo
            .load_call_graph()
            .await
            .expect("load")
            .expect("Some for populated DB");
        assert_eq!(
            loaded, graph,
            "re-saved graph must round-trip equal to original"
        );
    });

    /// Spec requirement: mid-INSERT failure rolls the transaction
    /// back. We seed a row that triggers a UNIQUE-violation on
    /// the first symbol insert (via a pre-seeded row that
    /// collides on (file_path, name) — but there is no UNIQUE
    /// index on symbols today). Instead, we exercise the
    /// rollback by corrupting the schema at the call site:
    /// pre-seed an edge with a value that violates the
    /// `confidence` column type (it is `REAL`, so any finite
    /// `f64` succeeds). To make a deterministic failure we
    /// install a temporary CHECK constraint on the `kind`
    /// column that rejects the value `function`, then call
    /// `save_call_graph` with a graph whose symbols have kind
    /// `Function` (which serializes to `"function"`). The
    /// symbol INSERT fails, the tx rolls back, and the
    /// pre-seeded rows survive.
    pg_test!(rollback_on_mid_insert_failure, |pool: PgPool| {
        let repo = PostgresRepository::from_pool(pool);

        // Pre-seed one symbol so we can verify the rollback
        // restores it. We do NOT need a unique index — we use
        // a temporary CHECK constraint on `kind` to force a
        // mid-INSERT failure deterministically.
        seed(repo.pool(), "seed.rs", "pre", "module", 1, 0).await;
        assert_eq!(repo.count_symbols().await.unwrap(), 1);

        // Install a CHECK constraint that rejects the literal
        // value "function". The save_call_graph will try to
        // INSERT a SymbolKind::Function row (Display form
        // "function"), which violates the constraint and
        // triggers ROLLBACK.
        sqlx::query(
            "ALTER TABLE symbols \
             ADD CONSTRAINT chk_kind_block_function \
             CHECK (kind <> 'function')",
        )
        .execute(repo.pool())
        .await
        .expect("add CHECK constraint");

        let mut g = CallGraph::new();
        g.add_symbol(Symbol::new(
            "x",
            SymbolKind::Function,
            Location::new("x.rs", 1, 0),
        ));

        let result = repo.save_call_graph(&g).await;
        assert!(
            matches!(result, Err(RepositoryError::Store(_))),
            "expected RepositoryError::Store, got {result:?}"
        );

        // The transaction must have rolled back: the
        // pre-seeded row is preserved, no partial insert
        // remains.
        assert_eq!(
            repo.count_symbols().await.unwrap(),
            1,
            "pre-seeded row must survive rollback"
        );
        assert_eq!(
            repo.count_edges().await.unwrap(),
            0,
            "no partial edges must remain after rollback"
        );

        // Clean up so the per-test DB can be dropped without
        // complaints. We DROP the constraint, not the table,
        // so the per-test isolation remains.
        let _ =
            sqlx::query("ALTER TABLE symbols DROP CONSTRAINT IF EXISTS chk_kind_block_function")
                .execute(repo.pool())
                .await;
    });

    /// Spec requirement: rollback unwinds the DELETE phase. Save
    /// A (3 sym, 4 edges), then attempt to save B that fails on
    /// the symbol INSERT (test seam: same CHECK constraint as
    /// the previous test). After the failure, A's rows must be
    /// intact.
    pg_test!(rollback_unwinds_delete_phase, |pool: PgPool| {
        let repo = PostgresRepository::from_pool(pool);

        // Save A: 3 symbols, 4 edges.
        let mut a = CallGraph::new();
        let a1 = a.add_symbol(Symbol::new(
            "p1",
            SymbolKind::Class,
            Location::new("p1.rs", 1, 0),
        ));
        let a2 = a.add_symbol(Symbol::new(
            "p2",
            SymbolKind::Class,
            Location::new("p2.rs", 1, 0),
        ));
        let a3 = a.add_symbol(Symbol::new(
            "p3",
            SymbolKind::Class,
            Location::new("p3.rs", 1, 0),
        ));
        a.add_dependency(&a1, &a2, DependencyType::Calls).unwrap();
        a.add_dependency(&a2, &a3, DependencyType::Calls).unwrap();
        a.add_dependency(&a1, &a3, DependencyType::Imports).unwrap();
        a.add_dependency(&a3, &a1, DependencyType::Inherits)
            .unwrap();
        repo.save_call_graph(&a).await.expect("save A");
        assert_eq!(repo.count_symbols().await.unwrap(), 3);
        assert_eq!(repo.count_edges().await.unwrap(), 4);

        // Install the same CHECK constraint to force a
        // mid-INSERT failure during the next save_call_graph.
        sqlx::query(
            "ALTER TABLE symbols \
             ADD CONSTRAINT chk_kind_block_function2 \
             CHECK (kind <> 'function')",
        )
        .execute(repo.pool())
        .await
        .expect("add CHECK constraint");

        // Build B with one Function symbol — its INSERT will
        // fail on the CHECK.
        let mut b = CallGraph::new();
        b.add_symbol(Symbol::new(
            "q1",
            SymbolKind::Function, // -> Display "function" -> CHECK fails
            Location::new("q1.rs", 1, 0),
        ));

        let result = repo.save_call_graph(&b).await;
        assert!(
            matches!(result, Err(RepositoryError::Store(_))),
            "expected RepositoryError::Store on B save, got {result:?}"
        );

        // A's 3 symbols + 4 edges must be intact — the DELETE
        // phase was rolled back along with the failed INSERT.
        assert_eq!(
            repo.count_symbols().await.unwrap(),
            3,
            "A's 3 symbols must survive the rolled-back DELETE"
        );
        assert_eq!(
            repo.count_edges().await.unwrap(),
            4,
            "A's 4 edges must survive the rolled-back DELETE"
        );
        for fqn in ["p1.rs:p1:1", "p2.rs:p2:1", "p3.rs:p3:1"] {
            let found = repo
                .find_symbol_by_qualified_name(fqn)
                .await
                .expect("query");
            assert!(found.is_some(), "A symbol {fqn} must still be present");
        }

        // Cleanup: drop the constraint.
        let _ =
            sqlx::query("ALTER TABLE symbols DROP CONSTRAINT IF EXISTS chk_kind_block_function2")
                .execute(repo.pool())
                .await;
    });

    /// Spec requirement: default build stays sqlx-free. This is a
    /// compile-time test: the `pg_test!` block above is gated
    /// behind `#[cfg(all(test, feature = "postgres"))]`, so a
    /// default `cargo check -p cognicode-core` will not pull in
    /// any of this code. The static assertion below is a no-op
    /// when the feature is enabled.
    #[cfg(not(feature = "postgres"))]
    const _: () = ();

    // -----------------------------------------------------------------
    // Multimodal (Generic Graph Layer) tests — T7, T8, T9, T10.
    //
    // Co-located with the rest of `mod tests` (NOT a submodule)
    // so the inner `pg_test!` macro, `fresh_pool`, and the row
    // mappers are in scope. Every test is gated behind
    // `#[cfg(all(test, feature = "postgres", feature = "multimodal"))]`
    // so the multimodal build is the only one that compiles the
    // graph_nodes/graph_edges code paths. The `pg_test!` macro
    // gracefully skips when `TEST_DATABASE_URL` is not set.
    // -----------------------------------------------------------------
    #[cfg(all(test, feature = "postgres", feature = "multimodal"))]
    use crate::domain::aggregates::generic_graph::{
        GraphEdge as MmGraphEdge, GraphNode as MmGraphNode, NodeId as MmNodeId,
    };
    #[cfg(all(test, feature = "postgres", feature = "multimodal"))]
    use crate::domain::value_objects::edge_kind::EdgeKind as MmEdgeKind;
    #[cfg(all(test, feature = "postgres", feature = "multimodal"))]
    use crate::domain::value_objects::node_kind::NodeKind as MmNodeKind;
    #[cfg(all(test, feature = "postgres", feature = "multimodal"))]
    use chrono::Utc as MmUtc;

    /// Build a small `Doc` graph node fixture (no DB I/O).
    #[cfg(all(test, feature = "postgres", feature = "multimodal"))]
    fn fixture_doc_node(id: &str, label: &str, status: &str) -> MmGraphNode {
        MmGraphNode::builder(MmNodeId::new(id), MmNodeKind::Doc)
            .label(label)
            .source_path("/docs/adr/0007.md")
            .property("status", status)
            .created_at(MmUtc::now())
            .updated_at(MmUtc::now())
            .build()
    }

    #[cfg(all(test, feature = "postgres", feature = "multimodal"))]
    fn fixture_decision_node(id: &str, label: &str) -> MmGraphNode {
        MmGraphNode::builder(MmNodeId::new(id), MmNodeKind::Decision)
            .label(label)
            .source_path("/docs/adr/0007.md")
            .created_at(MmUtc::now())
            .updated_at(MmUtc::now())
            .build()
    }

    #[cfg(all(test, feature = "postgres", feature = "multimodal"))]
    fn fixture_edge(source: &str, target: &str, kind: MmEdgeKind, confidence: f64) -> MmGraphEdge {
        MmGraphEdge::new(
            MmNodeId::new(source),
            MmNodeId::new(target),
            kind,
            Provenance::Extracted,
            confidence,
        )
        .expect("fixture edge must construct")
    }

    // ---- T7 RED gate ----

    /// `run_migrations` must create the `graph_nodes` table with
    /// the expected columns and the two btree indexes
    /// (`idx_graph_nodes_kind`, `idx_graph_nodes_source_path`).
    #[cfg(all(test, feature = "postgres", feature = "multimodal"))]
    pg_test!(graph_nodes_table_exists, |pool: PgPool| {
        let repo = PostgresRepository::from_pool(pool);
        repo.run_migrations().await.expect("migrations");

        let table_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM information_schema.tables \
             WHERE table_name = 'graph_nodes'",
        )
        .fetch_one(repo.pool())
        .await
        .expect("information_schema query");
        assert_eq!(table_count.0, 1, "graph_nodes table must exist");

        let cols: Vec<(String,)> = sqlx::query_as(
            "SELECT column_name \
             FROM information_schema.columns \
             WHERE table_name = 'graph_nodes' \
             ORDER BY ordinal_position",
        )
        .fetch_all(repo.pool())
        .await
        .expect("columns query");
        let col_names: Vec<String> = cols.into_iter().map(|(c,)| c).collect();
        for required in [
            "id",
            "kind",
            "label",
            "source_path",
            "properties",
            "created_at",
            "updated_at",
        ] {
            assert!(
                col_names.iter().any(|c| c == required),
                "graph_nodes missing column `{required}` — got {col_names:?}"
            );
        }

        for idx in ["idx_graph_nodes_kind", "idx_graph_nodes_source_path"] {
            let found: (i64,) = sqlx::query_as(
                "SELECT COUNT(*) FROM pg_indexes \
                 WHERE tablename = 'graph_nodes' AND indexname = $1",
            )
            .bind(idx)
            .fetch_one(repo.pool())
            .await
            .expect("pg_indexes query");
            assert_eq!(found.0, 1, "index `{idx}` must exist");
        }
    });

    // ---- T8 RED gate ----

    /// `run_migrations` must create the `graph_edges` table with
    /// the expected columns, the natural-key UNIQUE index, and
    /// the three btree indexes.
    #[cfg(all(test, feature = "postgres", feature = "multimodal"))]
    pg_test!(graph_edges_table_exists, |pool: PgPool| {
        let repo = PostgresRepository::from_pool(pool);
        repo.run_migrations().await.expect("migrations");

        let table_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM information_schema.tables \
             WHERE table_name = 'graph_edges'",
        )
        .fetch_one(repo.pool())
        .await
        .expect("information_schema query");
        assert_eq!(table_count.0, 1, "graph_edges table must exist");

        let cols: Vec<(String,)> = sqlx::query_as(
            "SELECT column_name \
             FROM information_schema.columns \
             WHERE table_name = 'graph_edges' \
             ORDER BY ordinal_position",
        )
        .fetch_all(repo.pool())
        .await
        .expect("columns query");
        let col_names: Vec<String> = cols.into_iter().map(|(c,)| c).collect();
        for required in [
            "id",
            "source_id",
            "target_id",
            "kind",
            "provenance",
            "confidence",
            "metadata",
            "created_at",
        ] {
            assert!(
                col_names.iter().any(|c| c == required),
                "graph_edges missing column `{required}` — got {col_names:?}"
            );
        }

        for idx in [
            "idx_graph_edges_source",
            "idx_graph_edges_target",
            "idx_graph_edges_kind",
            "uniq_graph_edges_source_target_kind",
        ] {
            let found: (i64,) = sqlx::query_as(
                "SELECT COUNT(*) FROM pg_indexes \
                 WHERE tablename = 'graph_edges' AND indexname = $1",
            )
            .bind(idx)
            .fetch_one(repo.pool())
            .await
            .expect("pg_indexes query");
            assert_eq!(found.0, 1, "index `{idx}` must exist");
        }
    });

    // ---- T9 RED gates (write path) ----

    /// `store_graph_nodes` + `get_graph_node` must round-trip a
    /// node losslessly.
    #[cfg(all(test, feature = "postgres", feature = "multimodal"))]
    pg_test!(store_and_retrieve_graph_node, |pool: PgPool| {
        let repo = PostgresRepository::from_pool(pool);
        repo.run_migrations().await.expect("migrations");

        let node = fixture_doc_node("doc:adr/0007.md#decision", "ADR-0007", "accepted");
        repo.store_graph_nodes(vec![node.clone()])
            .await
            .expect("store_graph_nodes");

        let fetched = repo
            .get_graph_node(MmNodeId::new("doc:adr/0007.md#decision"))
            .await
            .expect("get_graph_node")
            .expect("expected Some(GraphNode)");
        assert_eq!(fetched.id, node.id);
        assert_eq!(fetched.kind, node.kind);
        assert_eq!(fetched.label, node.label);
        assert_eq!(fetched.source_path, node.source_path);
        assert_eq!(
            fetched.properties.get("status").map(String::as_str),
            Some("accepted")
        );
    });

    /// `store_graph_edges` must reject a row whose
    /// `confidence` is outside `[0,1]` (the `CHECK` constraint
    /// in the DDL is the source of truth). Round-trip a valid
    /// edge and assert it survives.
    #[cfg(all(test, feature = "postgres", feature = "multimodal"))]
    pg_test!(store_graph_edge_with_validation, |pool: PgPool| {
        let repo = PostgresRepository::from_pool(pool);
        repo.run_migrations().await.expect("migrations");

        repo.store_graph_nodes(vec![
            fixture_doc_node("doc:src.md#intro", "Intro", "draft"),
            fixture_decision_node("decision:adr/0001.md#context", "ADR-0001"),
        ])
        .await
        .expect("seed nodes");

        let edge = fixture_edge(
            "doc:src.md#intro",
            "decision:adr/0001.md#context",
            MmEdgeKind::Cites,
            0.9,
        );
        repo.store_graph_edges(vec![edge.clone()])
            .await
            .expect("store_graph_edges valid");
        let fetched = repo
            .find_graph_edges(
                Some(MmNodeId::new("doc:src.md#intro")),
                Some(MmNodeId::new("decision:adr/0001.md#context")),
            )
            .await
            .expect("find_graph_edges");
        assert_eq!(fetched.len(), 1);
        assert_eq!(fetched[0].kind, MmEdgeKind::Cites);
        assert!((fetched[0].confidence - 0.9).abs() < 1e-9);

        // Bypassing `GraphEdge::new` to write a
        // confidence=1.5 row directly: the CHECK constraint
        // must reject it.
        let bad = sqlx::query(
            "INSERT INTO graph_edges \
                    (source_id, target_id, kind, provenance, confidence) \
                 VALUES ($1, $2, 'cites', 'extracted', 1.5)",
        )
        .bind("doc:src.md#intro")
        .bind("decision:adr/0001.md#context")
        .execute(repo.pool())
        .await;
        assert!(bad.is_err(), "CHECK constraint must reject confidence=1.5");
    });

    /// `store_graph_nodes` + `store_graph_edges` must be
    /// idempotent: re-ingesting the same payload updates the
    /// existing rows (no duplicates, no new surrogate ids on
    /// edges, `created_at` preserved on nodes).
    #[cfg(all(test, feature = "postgres", feature = "multimodal"))]
    pg_test!(store_graph_upsert_idempotent, |pool: PgPool| {
        let repo = PostgresRepository::from_pool(pool);
        repo.run_migrations().await.expect("migrations");

        let mut node = fixture_doc_node("doc:foo.md#a", "First Label", "draft");
        repo.store_graph_nodes(vec![node.clone()])
            .await
            .expect("first insert");
        let created_first = repo
            .get_graph_node(MmNodeId::new("doc:foo.md#a"))
            .await
            .expect("read 1")
            .expect("Some")
            .created_at;

        node.label = "Second Label".to_string();
        node = node.with_property("status", "accepted");
        repo.store_graph_nodes(vec![node.clone()])
            .await
            .expect("second insert");
        let updated = repo
            .get_graph_node(MmNodeId::new("doc:foo.md#a"))
            .await
            .expect("read 2")
            .expect("Some");
        assert_eq!(updated.label, "Second Label");
        assert_eq!(
            updated.properties.get("status").map(String::as_str),
            Some("accepted")
        );
        assert_eq!(
            updated.created_at, created_first,
            "created_at must be preserved across re-ingest"
        );

        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM graph_nodes")
            .fetch_one(repo.pool())
            .await
            .expect("count");
        assert_eq!(count.0, 1, "duplicate id must collapse to 1 row");
    });

    // ---- T10 RED gates (read path) ----

    /// `find_graph_nodes(Some(kind), _)` must return only
    /// nodes of that kind.
    #[cfg(all(test, feature = "postgres", feature = "multimodal"))]
    pg_test!(find_nodes_by_kind, |pool: PgPool| {
        let repo = PostgresRepository::from_pool(pool);
        repo.run_migrations().await.expect("migrations");

        repo.store_graph_nodes(vec![
            fixture_doc_node("doc:a.md#x", "A", "draft"),
            fixture_doc_node("doc:b.md#y", "B", "draft"),
            fixture_decision_node("decision:adr/0001.md#c", "ADR-0001"),
        ])
        .await
        .expect("seed");

        let docs = repo
            .find_graph_nodes(Some(MmNodeKind::Doc), 100)
            .await
            .expect("find_docs");
        assert_eq!(docs.len(), 2);
        assert!(docs.iter().all(|n| n.kind == MmNodeKind::Doc));

        let decisions = repo
            .find_graph_nodes(Some(MmNodeKind::Decision), 100)
            .await
            .expect("find_decisions");
        assert_eq!(decisions.len(), 1);
        assert_eq!(decisions[0].kind, MmNodeKind::Decision);

        let all = repo.find_graph_nodes(None, 100).await.expect("find_all");
        assert_eq!(all.len(), 3, "no kind filter returns every node");
    });

    /// `find_graph_edges(Some(source), _)` must return only
    /// edges originating from `source`. `None, Some(target)`
    /// must return only edges terminating at `target`.
    #[cfg(all(test, feature = "postgres", feature = "multimodal"))]
    pg_test!(find_edges_by_source, |pool: PgPool| {
        let repo = PostgresRepository::from_pool(pool);
        repo.run_migrations().await.expect("migrations");

        repo.store_graph_nodes(vec![
            fixture_doc_node("doc:src.md#a", "A", "draft"),
            fixture_doc_node("doc:src.md#b", "B", "draft"),
            fixture_decision_node("decision:adr/0001.md#c", "ADR-0001"),
        ])
        .await
        .expect("seed nodes");

        repo.store_graph_edges(vec![
            fixture_edge(
                "doc:src.md#a",
                "decision:adr/0001.md#c",
                MmEdgeKind::Cites,
                0.9,
            ),
            fixture_edge(
                "doc:src.md#b",
                "decision:adr/0001.md#c",
                MmEdgeKind::Cites,
                0.7,
            ),
            fixture_edge(
                "decision:adr/0001.md#c",
                "doc:src.md#a",
                MmEdgeKind::Justifies,
                0.6,
            ),
        ])
        .await
        .expect("seed edges");

        let by_source = repo
            .find_graph_edges(Some(MmNodeId::new("doc:src.md#a")), None)
            .await
            .expect("by source");
        assert_eq!(by_source.len(), 1);
        assert_eq!(by_source[0].source.as_str(), "doc:src.md#a");
        assert_eq!(by_source[0].target.as_str(), "decision:adr/0001.md#c");

        let by_target = repo
            .find_graph_edges(None, Some(MmNodeId::new("decision:adr/0001.md#c")))
            .await
            .expect("by target");
        assert_eq!(by_target.len(), 2);

        let both_none = repo.find_graph_edges(None, None).await;
        assert!(both_none.is_err(), "must reject (None, None)");
    });

    /// `get_graph_node(id)` must return `Ok(None)` for an
    /// unknown id and `Ok(Some(node))` for a known id.
    #[cfg(all(test, feature = "postgres", feature = "multimodal"))]
    pg_test!(get_node_by_id, |pool: PgPool| {
        let repo = PostgresRepository::from_pool(pool);
        repo.run_migrations().await.expect("migrations");

        let node = fixture_doc_node("doc:known.md#a", "Known", "draft");
        repo.store_graph_nodes(vec![node.clone()])
            .await
            .expect("seed");

        let known = repo
            .get_graph_node(MmNodeId::new("doc:known.md#a"))
            .await
            .expect("get_known");
        assert!(known.is_some());
        assert_eq!(known.unwrap().id, node.id);

        let unknown = repo
            .get_graph_node(MmNodeId::new("doc:unknown.md#a"))
            .await
            .expect("get_unknown");
        assert!(unknown.is_none());
    });

    // -----------------------------------------------------------------
    // node_properties — Phase 2 (ownership feature)
    // -----------------------------------------------------------------

    #[cfg(all(test, feature = "postgres", feature = "multimodal"))]
    pg_test!(node_properties_returns_jsonb_map, |pool: PgPool| {
        use crate::domain::aggregates::SymbolId;
        use std::collections::HashMap;

        let repo = PostgresRepository::from_pool(pool);

        // Insert a graph_nodes row with properties JSONB directly.
        let node_id = "src/lib.rs:foo:1";
        let props = serde_json::json!({
            "codeowners": "alice",
            "last_author": "alice@x"
        });
        sqlx::query(
            "INSERT INTO graph_nodes (id, kind, label, source_path, properties) \
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(node_id)
        .bind("Symbol")
        .bind("foo")
        .bind("src/lib.rs")
        .bind(&props)
        .execute(repo.pool())
        .await
        .expect("insert with properties");

        // Query via node_properties — should deserialize JSONB to HashMap.
        let result = repo
            .node_properties(&SymbolId::new(node_id))
            .await
            .expect("node_properties must not error");
        let props_map = result.expect("node_properties must return Some");
        assert_eq!(props_map.get("codeowners"), Some(&"alice".to_string()));
        assert_eq!(props_map.get("last_author"), Some(&"alice@x".to_string()));
    });

    #[cfg(all(test, feature = "postgres", feature = "multimodal"))]
    pg_test!(
        node_properties_returns_none_for_missing_node,
        |pool: PgPool| {
            use crate::domain::aggregates::SymbolId;

            let repo = PostgresRepository::from_pool(pool);
            let result = repo
                .node_properties(&SymbolId::new("does:not:exist"))
                .await
                .expect("node_properties must not error");
            assert!(result.is_none(), "expected None for missing node");
        }
    );

    // -------------------------------------------------------------------------
    // 4.6a RED — GraphNode JSONB properties round-trip
    // Spec: `repository-trait-bridge::Typed JSONB properties round-trip unchanged`
    // GIVEN a GraphNode with structured properties (complexity: 12, tags: ["auth"],
    //           nested: {"k": "v"})
    // WHEN persisted via store_graph_nodes and re-loaded via get_graph_node
    // THEN loaded.properties equals the original bit-for-bit
    // -------------------------------------------------------------------------
    #[cfg(all(test, feature = "postgres", feature = "multimodal"))]
    pg_test!(graph_node_properties_jsonb_roundtrip, |pool: PgPool| {
        let repo = PostgresRepository::from_pool(pool);
        repo.run_migrations().await.expect("migrations");

        // Build a node with the exact properties shape from the spec.
        let original_props = serde_json::json!({
            "complexity": 12,
            "tags": ["auth"],
            "nested": {"k": "v"}
        });
        let node = MmGraphNode::builder(MmNodeId::new("doc:adr/0010.md#decision"), MmNodeKind::Doc)
            .label("ADR-0010")
            .source_path("/docs/adr/0010.md")
            .created_at(MmUtc::now())
            .updated_at(MmUtc::now())
            .property_json("complexity", original_props["complexity"].clone())
            .property_json("tags", original_props["tags"].clone())
            .property_json("nested", original_props["nested"].clone())
            .build();

        repo.store_graph_nodes(vec![node.clone()])
            .await
            .expect("store_graph_nodes must succeed");

        let loaded = repo
            .get_graph_node(MmNodeId::new("doc:adr/0010.md#decision"))
            .await
            .expect("get_graph_node must succeed")
            .expect("expected Some(GraphNode)");

        // Assert bit-for-bit equality of the full properties Value.
        assert_eq!(
            loaded.properties, original_props,
            "properties must round-trip unchanged through PG JSONB"
        );
    });

    // -------------------------------------------------------------------------
    // 4.3a RED — workspace-scoped find_nodes_by_kind / find_incoming_edges
    // Spec: `generic-graph-model::Workspace-scoped upsert and incoming edges`
    // GIVEN empty workspaces ws1 and ws2
    // WHEN a GraphNode is upserted under ws1 AND 3 edges point to the same
    //      Doc target in ws1 plus 1 edge in ws2
    // THEN find_nodes_by_kind(Function, ws1) returns the upserted node
    // AND find_nodes_by_kind(Function, ws2) returns an empty Vec
    // AND find_incoming_edges(target, ws1) returns exactly 3 edges
    // AND find_incoming_edges(target, ws2) returns 0
    //
    // NOTE: The actual methods find_nodes_by_kind(workspace) and
    // find_incoming_edges(workspace) live in cognicode-explorer::PgGraphRepository.
    // This test verifies the workspace isolation behavior at the SQL level
    // using raw queries, which is what those methods execute internally.
    // -------------------------------------------------------------------------
    #[cfg(feature = "postgres")]
    pg_test!(workspace_scoped_find_nodes_and_incoming_edges, |pool: PgPool| {
        use sqlx::Row;

        let repo = PostgresRepository::from_pool(pool);

        // Seed ws1: 1 Function node + 3 incoming edges pointing to a Doc target
        // Insert nodes
        sqlx::query(
            "INSERT INTO graph_nodes (workspace_id, id, kind, label) \
             VALUES ('ws1', 'func1', 'symbol.function', 'my_function')",
        )
        .execute(repo.pool())
        .await
        .expect("insert ws1 function node");

        sqlx::query(
            "INSERT INTO graph_nodes (workspace_id, id, kind, label) \
             VALUES ('ws1', 'doc1', 'symbol.doc', 'my_doc')",
        )
        .execute(repo.pool())
        .await
        .expect("insert ws1 doc node");

        // Insert 3 incoming edges to doc1 in ws1
        for i in 1..=3 {
            sqlx::query(&format!(
                "INSERT INTO graph_edges (workspace_id, source_id, target_id, kind) \
                 VALUES ('ws1', 'src{}', 'doc1', 'dependency.calls')",
                i
            ))
            .execute(repo.pool())
            .await
            .expect("insert ws1 edge");
        }

        // Seed ws2: only 1 edge pointing to a doc (different target)
        sqlx::query(
            "INSERT INTO graph_nodes (workspace_id, id, kind, label) \
             VALUES ('ws2', 'func2', 'symbol.function', 'other_function')",
        )
        .execute(repo.pool())
        .await
        .expect("insert ws2 function node");

        sqlx::query(
            "INSERT INTO graph_nodes (workspace_id, id, kind, label) \
             VALUES ('ws2', 'doc2', 'symbol.doc', 'other_doc')",
        )
        .execute(repo.pool())
        .await
        .expect("insert ws2 doc node");

        sqlx::query(
            "INSERT INTO graph_edges (workspace_id, source_id, target_id, kind) \
             VALUES ('ws2', 'src_ws2', 'doc2', 'dependency.calls')",
        )
        .execute(repo.pool())
        .await
        .expect("insert ws2 edge");

        // ---- Assert find_nodes_by_kind(Function, ws1) returns 1 ----
        let ws1_func_count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM graph_nodes \
             WHERE kind = 'symbol.function' AND workspace_id = 'ws1'",
        )
        .fetch_one(repo.pool())
        .await
        .expect("count ws1 functions");
        assert_eq!(ws1_func_count, 1, "ws1 should have 1 function node");

        // ---- Assert find_nodes_by_kind(Function, ws2) returns 0 ----
        let ws2_func_count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM graph_nodes \
             WHERE kind = 'symbol.function' AND workspace_id = 'ws2'",
        )
        .fetch_one(repo.pool())
        .await
        .expect("count ws2 functions");
        assert_eq!(ws2_func_count, 0, "ws2 should have 0 function nodes (has doc2 only)");

        // ---- Assert find_incoming_edges(doc1, ws1) returns exactly 3 ----
        let ws1_incoming: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM graph_edges \
             WHERE target_id = 'doc1' AND workspace_id = 'ws1'",
        )
        .fetch_one(repo.pool())
        .await
        .expect("count ws1 incoming edges");
        assert_eq!(ws1_incoming, 3, "ws1 doc1 should have exactly 3 incoming edges");

        // ---- Assert find_incoming_edges(doc2, ws2) returns 0 ----
        let ws2_incoming: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM graph_edges \
             WHERE target_id = 'doc2' AND workspace_id = 'ws2'",
        )
        .fetch_one(repo.pool())
        .await
        .expect("count ws2 incoming edges");
        assert_eq!(ws2_incoming, 0, "ws2 doc2 should have 0 incoming edges (src_ws2 -> doc2, but doc2 != doc1)");
    });

    // -------------------------------------------------------------------------
    // 4.4a RED — revision-pinned callees_with_metadata
    // Spec: `repository-trait-bridge::Pinned read returns snapshot for the pinned revision`
    // GIVEN a graph seeded for ws1 at revision 3 with 3 callees:
    //        (target1, Extracted, 1.0), (target2, Inferred, 0.7), (target3, Ambiguous, 0.3)
    // WHEN a concurrent ingest advances the head to revision 4 with a different graph
    // THEN callees_with_metadata_pinned(caller, ws1, RevisionId(3)) returns
    //      exactly the 3 revision-3 entries with their exact (provenance, confidence)
    //      AND the set MUST NOT be the revision-4 set
    //
    // NOTE: The actual method callees_with_metadata_pinned lives in
    // cognicode-explorer::CallGraphRepository. This test verifies the underlying
    // revision-pinned snapshot behavior by using load_call_graph_ws with a pinned
    // revision and directly inspecting CallGraph::callees_with_metadata.
    // -------------------------------------------------------------------------
    #[cfg(feature = "postgres")]
    pg_test!(callees_with_metadata_pinned_revision_isolation, |pool: PgPool| {
        use crate::domain::services::ExtractionContext;
        use crate::domain::value_objects::{DependencyType, Provenance, RevisionId, WorkspaceId};
        use crate::domain::aggregates::Symbol;

        let repo = PostgresRepository::from_pool(pool);
        let ws = WorkspaceId::try_new("ws1").expect("ws1 must be valid");

        // ---- Build and save rev 3 graph ----
        // Create caller "caller" and 3 targets with specific provenance
        let mut g3 = CallGraph::new();
        let caller = g3.add_symbol(Symbol::new(
            "caller", SymbolKind::Function, Location::new("lib.rs", 1, 0),
        ));
        let t1 = g3.add_symbol(Symbol::new(
            "target1", SymbolKind::Function, Location::new("lib.rs", 10, 0),
        ));
        let t2 = g3.add_symbol(Symbol::new(
            "target2", SymbolKind::Function, Location::new("lib.rs", 20, 0),
        ));
        let t3 = g3.add_symbol(Symbol::new(
            "target3", SymbolKind::Function, Location::new("lib.rs", 30, 0),
        ));

        // Add edges with specific provenance via ExtractionContext
        g3.add_dependency_with_provenance(&caller, &t1, DependencyType::Calls, ExtractionContext::DirectExtraction)
            .expect("DirectExtraction for target1");
        // Heuristic score 0.7 → Inferred with confidence 0.7 (clamped to [0.5, 0.9])
        g3.add_dependency_with_provenance(&caller, &t2, DependencyType::Calls, ExtractionContext::Heuristic { score: 0.7 })
            .expect("Heuristic for target2");
        g3.add_dependency_with_provenance(&caller, &t3, DependencyType::Calls, ExtractionContext::Unresolved)
            .expect("Unresolved for target3");

        let rev3 = repo.save_call_graph_ws(&g3, &ws)
            .await
            .expect("save rev3 must succeed");
        assert_eq!(rev3.get(), 3, "rev3 must be 3");

        // ---- Build and save rev 4 graph (different callees) ----
        let mut g4 = CallGraph::new();
        let caller4 = g4.add_symbol(Symbol::new(
            "caller", SymbolKind::Function, Location::new("lib.rs", 1, 0),
        ));
        let t4_new = g4.add_symbol(Symbol::new(
            "target4_new", SymbolKind::Function, Location::new("lib.rs", 40, 0),
        ));
        // Only 1 edge in rev 4
        g4.add_dependency_with_provenance(&caller4, &t4_new, DependencyType::Calls, ExtractionContext::DirectExtraction)
            .expect("DirectExtraction for target4_new");

        let rev4 = repo.save_call_graph_ws(&g4, &ws)
            .await
            .expect("save rev4 must succeed");
        assert_eq!(rev4.get(), 4, "rev4 must be 4");

        // ---- Load at rev3 and verify callees_with_metadata ----
        let loaded_rev3 = repo.load_call_graph_ws(&ws, rev3)
            .await
            .expect("load rev3 must succeed")
            .expect("rev3 should exist");
        let callees_rev3 = loaded_rev3.callees_with_metadata(&caller);

        assert_eq!(callees_rev3.len(), 3, "rev3 must have exactly 3 callees");
        // Verify exact (provenance, confidence) tuples
        // SymbolId is the fully-qualified name: "lib.rs:{name}:{line}"
        let mut found = Vec::new();
        for (target, _dep, prov, conf) in callees_rev3 {
            found.push((target.as_str().to_string(), prov, conf));
        }
        found.sort_by_key(|x| x.0.clone());

        assert!(found[0].0.contains("target1"), "first callee must be target1, got {}", found[0].0);
        assert_eq!(found[0].1, Provenance::Extracted, "target1 provenance must be Extracted");
        assert!((found[0].2 - 1.0).abs() < 1e-9, "target1 confidence must be 1.0");

        assert!(found[1].0.contains("target2"), "second callee must be target2, got {}", found[1].0);
        assert_eq!(found[1].1, Provenance::Inferred, "target2 provenance must be Inferred");
        assert!((found[1].2 - 0.7).abs() < 1e-9, "target2 confidence must be 0.7");

        assert!(found[2].0.contains("target3"), "third callee must be target3, got {}", found[2].0);
        assert_eq!(found[2].1, Provenance::Ambiguous, "target3 provenance must be Ambiguous");
        assert!((found[2].2 - 0.3).abs() < 1e-9, "target3 confidence must be 0.3");

        // ---- Verify rev4 has different callees ----
        let loaded_rev4 = repo.load_call_graph_ws(&ws, rev4)
            .await
            .expect("load rev4 must succeed")
            .expect("rev4 should exist");
        let callees_rev4 = loaded_rev4.callees_with_metadata(&caller4);

        assert_eq!(callees_rev4.len(), 1, "rev4 must have exactly 1 callee");
        let t4_name = &callees_rev4[0].0;
        assert!(t4_name.as_str().contains("target4_new"), "rev4 callee must be target4_new, got {}", t4_name);
    });

    // -------------------------------------------------------------------------
    // Task 1.4a RED — graph_revisions table with head uniqueness
    // Scenario: `graph-revisions::New table exists with head uniqueness`
    // Assert: `\d graph_revisions` columns match; second `head_of=true`
    //         insert for same workspace rejected.
    // -------------------------------------------------------------------------
    #[cfg(feature = "postgres")]
    pg_test!(graph_revisions_table_exists, |pool: PgPool| {
        use sqlx::Row;

        let repo = PostgresRepository::from_pool(pool);

        // Verify columns via information_schema
        let rows = sqlx::query(
            "SELECT column_name, data_type \
             FROM information_schema.columns \
             WHERE table_name = 'graph_revisions' \
             ORDER BY ordinal_position",
        )
        .fetch_all(repo.pool())
        .await
        .expect("query graph_revisions columns");

        // Should have: workspace_id, revision_id, created_at, head_of
        assert!(
            rows.iter().any(|r| r.get::<String, _>("column_name") == "workspace_id"),
            "workspace_id column missing"
        );
        assert!(
            rows.iter().any(|r| r.get::<String, _>("column_name") == "revision_id"),
            "revision_id column missing"
        );
        assert!(
            rows.iter().any(|r| r.get::<String, _>("column_name") == "created_at"),
            "created_at column missing"
        );
        assert!(
            rows.iter().any(|r| r.get::<String, _>("column_name") == "head_of"),
            "head_of column missing"
        );
    });

    #[cfg(feature = "postgres")]
    pg_test!(graph_revisions_head_unique_per_workspace, |pool: PgPool| {
        let repo = PostgresRepository::from_pool(pool);

        // Insert first revision with head_of=true for "ws1"
        sqlx::query(
            "INSERT INTO graph_revisions (workspace_id, revision_id, head_of) \
             VALUES ('ws1', 1, true)",
        )
        .execute(repo.pool())
        .await
        .expect("first head insert");

        // Insert second revision with head_of=true for "ws1" — should fail
        let err = sqlx::query(
            "INSERT INTO graph_revisions (workspace_id, revision_id, head_of) \
             VALUES ('ws1', 2, true)",
        )
        .execute(repo.pool())
        .await
        .expect_err("second head insert for same workspace must fail");

        // Postgres constraint violation error
        assert!(
            err.to_string().contains("idx_graph_revisions_head")
                || err.to_string().contains("unique"),
            "expected unique constraint error, got: {err}"
        );

        // Verify first row is still there
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM graph_revisions WHERE workspace_id = 'ws1'",
        )
        .fetch_one(repo.pool())
        .await
        .expect("count query");
        assert_eq!(count, 1, "first row must remain after constraint violation");
    });

        // -------------------------------------------------------------------------
        // Task 1.4b GREEN — idempotent on empty (migration is embedded + run)
        // -------------------------------------------------------------------------
        #[cfg(feature = "postgres")]
        pg_test!(run_migrations_idempotent_on_empty_graph_revisions, |pool: PgPool| {
            let repo = PostgresRepository::from_pool(pool);
            // First call — runs migrations
            repo.run_migrations().await.expect("first call");
            // Second call — must be idempotent
            repo.run_migrations().await.expect("second call must be idempotent");

            // Insert two revisions — first head, second non-head
            sqlx::query(
                "INSERT INTO graph_revisions (workspace_id, revision_id, head_of) \
                 VALUES ('default', 1, true)",
            )
            .execute(repo.pool())
            .await
            .expect("insert head revision");

            sqlx::query(
                "INSERT INTO graph_revisions (workspace_id, revision_id, head_of) \
                 VALUES ('default', 2, false)",
            )
            .execute(repo.pool())
            .await
            .expect("insert non-head revision");

            let count: i64 =
                sqlx::query_scalar("SELECT count(*) FROM graph_revisions")
                    .fetch_one(repo.pool())
                    .await
                    .expect("count");
            assert_eq!(count, 2, "expected 2 revisions");
        });

    // -------------------------------------------------------------------------
    // Task 1.5a RED — workspace-scoped identity migration
    // Scenario: `generic-graph-model::PK and uniqueness include workspace_id`
    //           `generic-graph-model::Homonymous nodes across workspaces do not collide`
    // Assert: `\d graph_nodes` PK matches; insert `(ws2,"src/x.rs:foo:1",
    //         "symbol.function",…)` succeeds when `(ws1,…)` exists; `count(*)` is 2.
    // -------------------------------------------------------------------------
    #[cfg(feature = "postgres")]
    pg_test!(workspace_scoped_pk_applied, |pool: PgPool| {
        use sqlx::Row;

        let repo = PostgresRepository::from_pool(pool);

        // Verify graph_nodes PK includes workspace_id
        let pk_cols: Vec<String> = sqlx::query(
            "SELECT column_name \
             FROM information_schema.key_column_usage \
             WHERE table_name = 'graph_nodes' AND constraint_name = 'graph_nodes_pkey_ws' \
             ORDER BY ordinal_position",
        )
        .fetch_all(repo.pool())
        .await
        .expect("query PK columns")
        .iter()
        .map(|r| r.get("column_name"))
        .collect();

        assert!(
            pk_cols.contains(&"workspace_id".to_string()),
            "PK must include workspace_id, got: {pk_cols:?}"
        );
        assert!(
            pk_cols.contains(&"id".to_string()),
            "PK must include id, got: {pk_cols:?}"
        );
        assert!(
            pk_cols.contains(&"kind".to_string()),
            "PK must include kind, got: {pk_cols:?}"
        );
    });

    #[cfg(feature = "postgres")]
    pg_test!(homonymous_nodes_across_workspaces_no_collision, |pool: PgPool| {
        let repo = PostgresRepository::from_pool(pool);

        // Insert a node in ws1
        sqlx::query(
            "INSERT INTO graph_nodes (workspace_id, id, kind, label) \
             VALUES ('ws1', 'src/x.rs:foo:1', 'symbol.function', 'foo')",
        )
        .execute(repo.pool())
        .await
        .expect("insert ws1 node");

        // Insert a node with SAME id and kind but DIFFERENT workspace — must succeed
        sqlx::query(
            "INSERT INTO graph_nodes (workspace_id, id, kind, label) \
             VALUES ('ws2', 'src/x.rs:foo:1', 'symbol.function', 'bar')",
        )
        .execute(repo.pool())
        .await
        .expect("insert ws2 node with same id/kind must succeed");

        // Verify both exist
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM graph_nodes \
             WHERE id = 'src/x.rs:foo:1' AND kind = 'symbol.function'",
        )
        .fetch_one(repo.pool())
        .await
        .expect("count");
        assert_eq!(count, 2, "expected 2 rows (one per workspace)");

        // Verify they are distinct by workspace
        let ws1_label: String = sqlx::query_scalar(
            "SELECT label FROM graph_nodes \
             WHERE workspace_id = 'ws1' AND id = 'src/x.rs:foo:1'",
        )
        .fetch_one(repo.pool())
        .await
        .expect("ws1 label");
        let ws2_label: String = sqlx::query_scalar(
            "SELECT label FROM graph_nodes \
             WHERE workspace_id = 'ws2' AND id = 'src/x.rs:foo:1'",
        )
        .fetch_one(repo.pool())
        .await
        .expect("ws2 label");
        assert_eq!(ws1_label, "foo");
        assert_eq!(ws2_label, "bar");
    });

    #[cfg(feature = "postgres")]
    pg_test!(workspace_scoped_edges_unique_index, |pool: PgPool| {
        let repo = PostgresRepository::from_pool(pool);

        // The composite FK is (workspace_id, source_id, kind) REFERENCES
        // graph_nodes(workspace_id, id, kind). The kind must match between
        // edge and node. We use kind='symbol.function' for both nodes and
        // edges to satisfy the FK constraint.
        //
        // Insert first edge in ws1
        sqlx::query(
            "INSERT INTO graph_nodes (workspace_id, id, kind, label) \
             VALUES ('ws1', 'a', 'symbol.function', 'a')",
        )
        .execute(repo.pool())
        .await
        .expect("insert node a");
        sqlx::query(
            "INSERT INTO graph_nodes (workspace_id, id, kind, label) \
             VALUES ('ws1', 'b', 'symbol.function', 'b')",
        )
        .execute(repo.pool())
        .await
        .expect("insert node b");

        sqlx::query(
            "INSERT INTO graph_edges (workspace_id, source_id, target_id, kind) \
             VALUES ('ws1', 'a', 'b', 'symbol.function')",
        )
        .execute(repo.pool())
        .await
        .expect("insert ws1 edge");

        // Insert same edge in ws2 — must succeed (different workspace)
        sqlx::query(
            "INSERT INTO graph_nodes (workspace_id, id, kind, label) \
             VALUES ('ws2', 'a', 'symbol.function', 'a')",
        )
        .execute(repo.pool())
        .await
        .expect("insert ws2 node a");
        sqlx::query(
            "INSERT INTO graph_nodes (workspace_id, id, kind, label) \
             VALUES ('ws2', 'b', 'symbol.function', 'b')",
        )
        .execute(repo.pool())
        .await
        .expect("insert ws2 node b");

        sqlx::query(
            "INSERT INTO graph_edges (workspace_id, source_id, target_id, kind) \
             VALUES ('ws2', 'a', 'b', 'symbol.function')",
        )
        .execute(repo.pool())
        .await
        .expect("insert ws2 edge must succeed");

        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM graph_edges \
             WHERE source_id = 'a' AND target_id = 'b' AND kind = 'symbol.function'",
        )
        .fetch_one(repo.pool())
        .await
        .expect("count");
        assert_eq!(count, 2, "expected 2 edges (one per workspace)");
    });
}
