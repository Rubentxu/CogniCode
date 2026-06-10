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
    DependencyType, EdgeMetadata, Location, Provenance, SymbolKind,
};

/// Schema DDL embedded at compile time.
///
/// `include_str!` guarantees the bytes are present in the rlib;
/// editing the SQL forces a rebuild. See spec scenario 4a.
#[cfg(feature = "postgres")]
const SCHEMA_SQL: &str = include_str!("schema_postgres.sql");

/// Multimodal (Generic Graph Layer) DDL — embedded ONLY when BOTH
/// the `postgres` and the `multimodal` Cargo features are enabled.
/// The DDL creates the `graph_nodes` + `graph_edges` tables, the
/// three btree indexes on `graph_edges` (`source_id`,
/// `target_id`, `kind`), the natural-key UNIQUE index on
/// `(source_id, target_id, kind)`, and the two btree indexes on
/// `graph_nodes` (`kind`, `source_path`).
///
/// Splitting the DDL into its own file is the only way to gate an
/// `include_str!` behind a Cargo feature. See
/// `m0009_graph_nodes_edges.sql` for the design notes.
#[cfg(all(feature = "postgres", feature = "multimodal"))]
const SCHEMA_SQL_MULTIMODAL: &str = include_str!("m0009_graph_nodes_edges.sql");

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
    /// Idempotent: every statement uses `IF NOT EXISTS`, so
    /// calling this on a freshly-initialised database is a
    /// no-op that still succeeds. Existing rows are preserved
    /// (no DROP, no schema-altering DDL is performed).
    ///
    /// When the `multimodal` feature is enabled, the Generic Graph
    /// Layer DDL (`m0009_graph_nodes_edges.sql`) is executed AFTER
    /// the base schema, so the `graph_nodes` / `graph_edges`
    /// tables are guaranteed to exist before the multimodal
    /// write-paths are used.
    pub async fn run_migrations(&self) -> Result<(), RepositoryError> {
        sqlx::query(SCHEMA_SQL)
            .execute(&self.pool)
            .await
            .map_err(|e| RepositoryError::Store(format!("migration: {e}")))?;
        // Multimodal DDL is applied after the base schema so the
        // order is deterministic. The `include_str!` constant
        // only exists when the `multimodal` feature is on, so
        // this block is compiled out in the default build.
        #[cfg(feature = "multimodal")]
        {
            sqlx::query(SCHEMA_SQL_MULTIMODAL)
                .execute(&self.pool)
                .await
                .map_err(|e| {
                    RepositoryError::Store(format!("multimodal migration: {e}"))
                })?;
        }
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
    fn provenance_to_extraction_context(
        provenance: Provenance,
        confidence: f64,
    ) -> ExtractionContext {
        match provenance {
            Provenance::Extracted => ExtractionContext::DirectExtraction,
            Provenance::Inferred => ExtractionContext::Heuristic { score: confidence },
            Provenance::Ambiguous => ExtractionContext::Unresolved,
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
    pub async fn save_call_graph(
        &self,
        graph: &CallGraph,
    ) -> Result<(), RepositoryError> {
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
                    (file_path, name, kind, line, column) \
                 VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(location.file())
            .bind(symbol.name())
            .bind(symbol.kind().to_string())
            .bind(line)
            .bind(column)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                RepositoryError::Store(format!("save_call_graph insert symbol: {e}"))
            })?;
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
            "SELECT file_path, name, kind, line, column \
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
                .map_err(|e| {
                    RepositoryError::Store(format!("load_call_graph count edges: {e}"))
                })?;
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
}

/// Row-mapping struct used by [`PostgresRepository`]'s queries.
/// The id and complexity columns are intentionally NOT selected
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

/// Split a `file:name:line` qualified name into its components.
///
/// `file` itself may legitimately contain `:` (Windows drive
/// letters on the form `C:\path\...`), so we split from the
/// RIGHT and only take the last two `:`s. Returns
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
    let second_colon = head.rfind(':').ok_or_else(|| {
        RepositoryError::InvalidQuery(format!("missing name segment: {qualified}"))
    })?;
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
use crate::domain::value_objects::node_kind::NodeKind as VkNodeKind;
#[cfg(all(feature = "postgres", feature = "multimodal"))]
use crate::domain::value_objects::edge_kind::EdgeKind as VkEdgeKind;
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
        let properties = match self.properties {
            serde_json::Value::Object(map) => map
                .into_iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k, s.to_string())))
                .collect(),
            _ => std::collections::HashMap::new(),
        };
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
            .properties(properties)
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
            VkEdgeKind::Dependency(crate::domain::value_objects::dependency_type::DependencyType::Calls)
        });
        let provenance =
            Provenance::from_str(&self.provenance).unwrap_or(Provenance::Extracted);
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
    pub async fn store_graph_nodes(
        &self,
        nodes: Vec<GraphNode>,
    ) -> Result<(), RepositoryError> {
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
            // The `properties` map is projected to a JSONB object:
            // every key in `node.properties` becomes a top-level
            // string-typed key. The shape is intentional — the
            // spec'd `DocsExtractor` payload (e.g.
            // `{"status": "accepted", "date": "2026-01-02"}`) is a
            // flat string map and round-trips losslessly.
            let properties_json = serde_json::Value::Object(
                node.properties
                    .iter()
                    .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                    .collect::<serde_json::Map<_, _>>(),
            );
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
            .map_err(|e| {
                RepositoryError::Store(format!("store_graph_nodes insert `{id}`: {e}"))
            })?;
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
    pub async fn store_graph_edges(
        &self,
        edges: Vec<GraphEdge>,
    ) -> Result<(), RepositoryError> {
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
        Ok(rows.into_iter().map(GraphNodeRow::into_graph_node).collect())
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
    pub async fn get_graph_node(
        &self,
        id: NodeId,
    ) -> Result<Option<GraphNode>, RepositoryError> {
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
}

// -----------------------------------------------------------------
// Tests — gated behind `cfg(all(test, feature = "postgres"))`. They
// require a running PostgreSQL 14+ instance. Per-test isolation is
// provided by a tiny manual fixture (each test creates its own
// uniquely-named database) instead of the `#[sqlx::test]` macro.
// We avoid that macro because its `migrate` feature pulls
// `sqlx-sqlite`, which conflicts with the workspace's `rusqlite`.
// Same isolation guarantee, no extra features.
//
// Prerequisite: set `TEST_DATABASE_URL` to a base URL like
// `postgres://user:pass@host:5432`. The test runner will create
// databases named `cognicode_test_<pid>_<test_name>` and drop
// them on completion. CI must provide a PostgreSQL service.
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

        // Connect to the new DB and run our migrations.
        let pool = sqlx::PgPool::connect(&test_url).await.ok()?;
        sqlx::query(SCHEMA_SQL).execute(&pool).await.ok()?;

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
        let c = g.add_symbol(Symbol::new("c", SymbolKind::Class, Location::new("c.rs", 1, 0)));
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
    pg_test!(load_populated_returns_some_with_exact_metadata, |pool: PgPool| {
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
                    assert_eq!(
                        s_prov, l_prov,
                        "provenance mismatch for {s_src}->{s_tgt}"
                    );
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
    });

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
    pg_test!(named_views_unique_index_rejects_duplicate_name, |pool: PgPool| {
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
            .load_named_view(
                "11111111-1111-1111-1111-111111111111",
                "w1",
                "u1",
            )
            .await
            .expect("load must succeed")
            .expect("Some");
        assert_eq!(row.name, "hotspots");
        assert_eq!(row.description.as_deref(), Some("first"));
    });

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
        b.add_dependency(&b2, &b3, DependencyType::Inherits).unwrap();
        b.add_dependency(&b3, &b4, DependencyType::References).unwrap();
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
        assert_eq!(
            syms_2, syms_1,
            "idempotent re-save must keep symbol count"
        );
        assert_eq!(
            edges_2, edges_1,
            "idempotent re-save must keep edge count"
        );

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
        let _ = sqlx::query("ALTER TABLE symbols DROP CONSTRAINT IF EXISTS chk_kind_block_function")
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
        a.add_dependency(&a3, &a1, DependencyType::Inherits).unwrap();
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
        let _ = sqlx::query(
            "ALTER TABLE symbols DROP CONSTRAINT IF EXISTS chk_kind_block_function2",
        )
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
    fn fixture_edge(
        source: &str,
        target: &str,
        kind: MmEdgeKind,
        confidence: f64,
    ) -> MmGraphEdge {
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
    pg_test!(
        store_graph_edge_with_validation,
        |pool: PgPool| {
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
            assert!(
                bad.is_err(),
                "CHECK constraint must reject confidence=1.5"
            );
        }
    );

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

        let all = repo
            .find_graph_nodes(None, 100)
            .await
            .expect("find_all");
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
            fixture_edge("doc:src.md#a", "decision:adr/0001.md#c", MmEdgeKind::Cites, 0.9),
            fixture_edge("doc:src.md#b", "decision:adr/0001.md#c", MmEdgeKind::Cites, 0.7),
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

        let both_none = repo
            .find_graph_edges(None, None)
            .await;
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
}
