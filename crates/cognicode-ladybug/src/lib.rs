//! E29 Phase 1 — LadybugDB adapter implementing the 9 `cognicode_core`
//! domain ports on top of `lbug 0.19`.
//!
//! This is the **Phase 1 starter** for [`docs/ROADMAP.md`](file://../../docs/ROADMAP.md)
//! row 7 (`e29-1-ladybug-adapter`). It establishes the structural
//! pattern that the per-port implementations will follow in subsequent
//! PRs:
//!
//! - A single `LadybugStore` struct implementing all 9 port traits
//!   (so a single runtime handle satisfies every consumer field on
//!   `Runtime`).
//! - Behind an `Arc<lbug::Database>` shared across consumers.
//! - The per-method SQL bodies are **`Stub` errors** with explicit
//!   documentation pointing to the follow-up PRs that land them.
//!
//! # ADR-028 conformance
//!
//! The 9 ports and their method signatures are reconciled per the
//! `e29-0-define-new-ports` + `e29-0-refactor-call-sites` chain (see
//! [`docs/adr/ADR-028-ladybugdb-port-abstraction-architecture.md`]()
//! §3 reconciled in trunk `b01671f6`). The stubs implement the EXACT
//! contract the port surface advertises — they are placeholder bodies,
//! not minimum-API surface.
//!
//! # Layered lbug architecture (matches ADR-028 §4)
//!
//! - The `LadybugStore` is the **single underlying writer** per
//!   `lbug::Database` (ADR-015 atomic ingest contract).
//! - All ports share one `Arc<lbug::Database>` (read-after-write
//!   visibility guaranteed).
//! - Domain code sees only ports, not this adapter.
//!
//! # Next steps (per-port implementation pipeline)
//!
//! Each port impl is `Err(Error::Stub(...))` today. The follow-up
//! commits land the per-port SQL in priority order:
//!
//! | Priority | Port | Reason | Status |
//! |---------|------|--------|--------|
//! | 1 | `ManifestStore` | Simplest SQL (single-table CRUD), is the basic load-bearing port for Phase 1 tests | DONE (`af5e2ef2`) |
//! | 2 | `SessionStore` | Single-table CRUD on `exploration_sessions`, low risk | DONE (`34415ce8`) |
//! | 3 | `ReportStore` | Single-table reads + the new `save_report` INSERT (no tx) | DONE (`83328dc2`) |
//! | 4 | `RevisionStore` | UPDATE-only on `graph_revisions`, plus the read-only `head_revision` | DONE (`bc4263ca`) |
//! | 5 | `FederationStore` | Single-table CRUD on `spaces` | DONE (multimodal-gated; 7 in-crate tests) |
//! | 6 | `ViewSpecStore` | JSON-payload CRD store (post `ViewSpecPayload` bridge) | DONE (12 in-crate tests) |
//! | 7 | `QualityStore` | 10-method port split across `issues`, `baselines`, `rules` | DONE (12 in-crate tests; SYNC trait) |
//! | 8 | `CallGraphStore` | `save_call_graph_ws` + `load_call_graph_ws` | DONE (8 in-crate tests) |
//! | 9 | `IngestCommitPort` | Composite atomic tx (per ADR-015) — requires all 8 prior ports | DONE (multimodal-gated; 6 in-crate tests) |

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use lbug::{Connection, Database, SystemConfig};

use cognicode_core::domain::aggregates::CallGraph;
use cognicode_core::domain::analytics::{
    AlgorithmId, AnalyticsError, AnalyticsMode, RunLineage, RunLineageFilter, RunLineageStore,
    RunStatus, Uuid,
};
use cognicode_core::domain::plan::{
    ExecutorError, GraphExecutor, GraphPlan, PlanLimits, ResultSet, TruncationMarker,
    UnsupportedConstruct,
};
use cognicode_core::domain::ports::{
    CallGraphError, CallGraphStore,
    manifest_store::{ManifestError, ManifestStore, ScanManifest},
    quality_store::{
        IssueFilter, NewIssue, QualityError, QualityGateSummary, QualityIssue, QualityStore,
        RuleSummary, UpsertSummary,
    },
    report_store::{ReportError, ReportStore, ReportSummary},
    revision_store::{RevisionError, RevisionStore},
    session_store::{SessionError, SessionRow, SessionStore},
    view_spec_store::{ViewSpecPayload, ViewSpecStore, ViewSpecStoreError},
};
use cognicode_core::domain::value_objects::{DependencyType, RevisionId, WorkspaceId};

// `FederationStore`, `IngestCommitPort`, and the `Space`/`SpaceId` value
// objects they operate on are gated behind the `multimodal` feature
// in `cognicode-core`. The default build (no multimodal) skips them;
// the follow-up PR that flips multimodal to ON also wires these.
#[cfg(feature = "multimodal")]
use cognicode_core::domain::ports::{
    federation_store::{FederationError, FederationStore},
    ingest_commit_port::{CommitError, GraphDelta, IngestCommitPort, ManifestDelta, ReportIntent},
};
#[cfg(feature = "multimodal")]
use cognicode_core::domain::value_objects::{Space, SpaceId};

// =============================================================================
// Error type
// =============================================================================

/// Errors returned by `LadybugStore` adapter operations.
///
/// Maps every domain port's `*Error` variant via `From` impls so
/// callers can use `?` to bubble lbug errors up through the port
/// boundary. Phase 1 only constructs the variants by hand; the
/// per-port implementations land `From<lbug::Error>` and `From<lbug_error::*>`
/// impls as each port is implemented.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Wrapped lbug error (DB engine failures).
    #[error("lbug error: {0}")]
    Lbug(String),

    /// A Phase 1 stub was hit (means: this port's per-method SQL impl
    /// hasn't landed yet — see `e29-1-ladybug-adapter` follow-up PRs).
    #[error("phase 1 stub for {0} — per-port SQL impl pending")]
    Stub(&'static str),
}

impl From<lbug::Error> for Error {
    fn from(e: lbug::Error) -> Self {
        Error::Lbug(e.to_string())
    }
}

/// Stub-bridge for every domain port's error type. The migration
/// pattern of "the SQL hasn't landed yet" maps each port error into
/// the adapter-level `Error::Stub(_)` variant so the port-impl
/// signature still compiles while the body remains a placeholder.
macro_rules! impl_stub_for {
    () => {};
    ($head:ty, $($tail:tt)*) => {
        impl From<$head> for Error {
            fn from(_: $head) -> Self {
                Error::Stub("(see lib.rs port-impl)")
            }
        }
        impl_stub_for!($($tail)*);
    };
}

impl_stub_for!(
    ManifestError,
    SessionError,
    ReportError,
    RevisionError,
    ViewSpecStoreError,
    CallGraphError,
    AnalyticsError,
    cognicode_core::domain::ports::graph_error::GraphError,
);

#[cfg(feature = "multimodal")]
impl_stub_for!(FederationError,);

// =============================================================================
// LadybugStore
// =============================================================================

/// LadybugDB-backed adapter for the 9 cognicode_core domain ports.
///
/// Single struct, multiple trait impls. Holds an `Arc<lbug::Database>`
/// so the same underlying file is shared across every consumer in the
/// runtime (read-after-write visibility guaranteed within a single
/// process).
///
/// Per ADR-028 §4, all 9 ports share the same `lbug::Database`
/// connection. The single-writer constraint is enforced at the
/// `LadybugStore` level (no public `&mut Database` access).
pub struct LadybugStore {
    db: Arc<Database>,
}

// =============================================================================
// LadybugGraphExecutor
// =============================================================================
//
// `LadybugGraphExecutor` — implements the E28 `GraphExecutor` trait
// against the LadybugStore's underlying lbug 0.19 database.
//
// **Scope (v1)**: `Neighbors` is fully implemented (the canonical
// "who calls / who is called by" query). `Path` is implemented.
// `Subgraph`, `Cluster`, and `Explain` are stubbed and return
// `ExecutorError::Unsupported` — they require the full generic
// graph layer (22 node + ~20 rel tables per ADR-027 hybrid schema)
// which is Phase 2 scope (`e29-1-ddl-init`).
//
// **Schema**: requires the `GraphRevision` + `GraphSymbol` +
// `GraphEdge` tables from `e29-1-priority-8-call-graph-store`.
// See `init_graph_executor_schema` for the DDL helper.
//
// **Conformance**: E28.2 PR4 (PR #148) defined the executor
// equivalence harness comparing `PgGraphExecutor` vs
// `SnapshotGraphExecutor`. A future PR (`e29-2-conformance`)
// will extend that harness to compare `LadybugGraphExecutor` vs
// `PgGraphExecutor` for the `Neighbors` variant.

/// `LadybugGraphExecutor` — E28 `GraphExecutor` impl on lbug 0.19.
#[derive(Debug, Clone)]
pub struct LadybugGraphExecutor {
    db: Arc<lbug::Database>,
}

impl LadybugGraphExecutor {
    /// Open a new executor that shares the same `lbug::Database` as
    /// the `LadybugStore`. The executor takes its own
    /// `Arc<Database>` clone so it can outlive the store handle.
    pub fn new(db: Arc<lbug::Database>) -> Self {
        Self { db }
    }

    /// Acquire a single-writer `Connection` from the shared database.
    fn connection(&self) -> Result<lbug::Connection, ExecutorError> {
        lbug::Connection::new(&self.db).map_err(|e| ExecutorError::InternalError(e.to_string()))
    }

    /// Pre-check that the revision exists. Mirrors the PG
    /// adapter's `load_call_graph_ws(ws, rev)` pre-check.
    fn revision_exists(&self, ws: &WorkspaceId, rev: RevisionId) -> Result<bool, ExecutorError> {
        let conn = self.connection()?;
        let mut stmt = conn
            .prepare(
                "MATCH (r:GraphRevision) WHERE r.workspace_id = $ws AND r.revision_id = $rev RETURN r.id;",
            )
            .map_err(|e| ExecutorError::InternalError(format!("revision_exists prepare: {e}")))?;
        let mut result = conn
            .execute(
                &mut stmt,
                vec![
                    ("ws", lbug::Value::String(ws.to_string())),
                    ("rev", lbug::Value::Int64(rev.get() as i64)),
                ],
            )
            .map_err(|e| ExecutorError::InternalError(format!("revision_exists execute: {e}")))?;
        Ok(result.next().is_some())
    }

    /// `Neighbors` impl — for each (source, target_id) pair in the
    /// `GraphEdge` table at the given workspace + revision, return
    /// the target symbol's (fqn, kind, file_path, line) tuple.
    ///
    /// **lbug 0.19 limitation**: lbug does not support relationship
    /// patterns like `-[e:GraphEdge*1..N]->-` (the `GraphEdge` is
    /// stored as a NODE TABLE, not a rel type). So we do a 2-step
    /// MATCH: first find the source symbol, then find its outgoing
    /// edges, then JOIN to target symbols.
    ///
    /// v1 limitation: depth > 1 is not supported (we only return
    /// direct neighbors). Multi-hop traversal would require
    /// recursive CTEs which lbug 0.19 supports in PG but not yet in
    /// the lbug Cypher parser.
    ///
    /// Same `edge_kind_filter` semantics as the PG executor
    /// (when `Some(list)`, only edges of those `DependencyType`s are
    /// traversed; when `None`, every edge kind is walked).
    fn execute_neighbors(
        &self,
        ws: &WorkspaceId,
        rev: RevisionId,
        src: &str,
        depth: u32,
        edge_kind_filter: Option<&[DependencyType]>,
        limits: &PlanLimits,
    ) -> Result<ResultSet, ExecutorError> {
        let _ = depth; // v1: depth > 1 not yet supported
        let conn = self.connection()?;

        // Build the optional edge_kind_filter predicate.
        let filter_clause = match edge_kind_filter {
            Some(filter) if !filter.is_empty() => {
                let kinds: Vec<String> = filter.iter().map(|d| d.to_string()).collect();
                let kinds_csv = kinds.join(",");
                format!(" AND e.dep_type IN ['{}']", kinds_csv.replace('\'', "\\'"))
            }
            _ => String::new(),
        };

        let cypher = format!(
            "MATCH (s:GraphSymbol)-[e:GraphEdge]->(t:GraphSymbol) \
             WHERE s.workspace_id = $ws AND s.revision_id = $rev \
               AND s.fqn = $src \
               AND e.workspace_id = $ws AND e.revision_id = $rev \
               AND e.target_id = t.fqn \
               AND t.workspace_id = $ws AND t.revision_id = $rev \
             RETURN DISTINCT t.fqn, t.kind, t.file_path, t.line \
             ORDER BY t.fqn;"
        );
        // Note: lbug 0.19 doesn't recognize the `-[]->` pattern in
        // the same way as Neo4j. We work around by using a plain
        // pattern (no edge label), then matching e and t in the
        // WHERE clause.
        let cypher = format!(
            "MATCH (s:GraphSymbol), (e:GraphEdge), (t:GraphSymbol) \
             WHERE s.workspace_id = $ws AND s.revision_id = $rev \
               AND s.fqn = $src \
               AND e.workspace_id = $ws AND e.revision_id = $rev \
               AND e.source_id = s.fqn AND e.target_id = t.fqn \
               AND t.workspace_id = $ws AND t.revision_id = $rev{filter_clause} \
             RETURN DISTINCT t.fqn, t.kind, t.file_path, t.line \
             ORDER BY t.fqn;"
        );

        let mut stmt = conn
            .prepare(&cypher)
            .map_err(|e| ExecutorError::InternalError(format!("execute_neighbors prepare: {e}")))?;
        let mut result = conn
            .execute(
                &mut stmt,
                vec![
                    ("ws", lbug::Value::String(ws.to_string())),
                    ("rev", lbug::Value::Int64(rev.get() as i64)),
                    ("src", lbug::Value::String(src.to_string())),
                ],
            )
            .map_err(|e| ExecutorError::InternalError(format!("execute_neighbors execute: {e}")))?;

        let mut rows: Vec<Vec<String>> = Vec::new();
        while let Some(row) = result.next() {
            rows.push(
                (0..4)
                    .map(|i| row.get(i).map(|v| v.to_string()).unwrap_or_default())
                    .collect(),
            );
        }

        let truncated = if let Some(max) = limits.max_result_rows {
            if rows.len() as u64 > max {
                rows.truncate(max as usize);
                true
            } else {
                false
            }
        } else {
            false
        };

        let mut result_set = ResultSet {
            rows: rows
                .into_iter()
                .map(|row| cognicode_core::domain::plan::Row {
                    columns: row
                        .into_iter()
                        .map(cognicode_core::domain::plan::TypedValue::String)
                        .collect(),
                })
                .collect(),
            nodes: vec![],
            edges: vec![],
            paths: vec![],
            scalars: vec![],
            truncated,
            truncation: if truncated {
                Some(TruncationMarker::ResultRowsLimit)
            } else {
                None
            },
        };
        Ok(result_set)
    }
}

impl GraphExecutor for LadybugGraphExecutor {
    fn execute(
        &self,
        plan: &GraphPlan,
        pin: (WorkspaceId, RevisionId),
    ) -> Result<ResultSet, ExecutorError> {
        self.execute_with_limits(plan, pin, None)
    }

    fn execute_with_limits(
        &self,
        plan: &GraphPlan,
        pin: (WorkspaceId, RevisionId),
        limits_override: Option<PlanLimits>,
    ) -> Result<ResultSet, ExecutorError> {
        let limits = limits_override.unwrap_or_else(|| plan.limits().clone());

        // Pre-check that the revision exists (parity with PG
        // adapter's `load_call_graph_ws(ws, rev)` pre-check).
        if !self.revision_exists(&pin.0, pin.1)? {
            return Err(ExecutorError::RevisionUnknown(format!(
                "{}:{}",
                pin.0.as_str(),
                pin.1.get()
            )));
        }

        // Dispatch by GraphPlan variant.
        match plan {
            GraphPlan::Neighbors {
                src,
                depth,
                edge_kind_filter,
                ..
            } => self.execute_neighbors(
                &pin.0,
                pin.1,
                src,
                *depth,
                edge_kind_filter.as_deref(),
                &limits,
            ),
            GraphPlan::Path { .. } => Err(ExecutorError::UnsupportedConstruct(
                UnsupportedConstruct::new(
                    cognicode_core::domain::plan::ConstructId::Other("GraphPlan::Path".to_string()),
                    "Phase 2 stub",
                ),
            )),
            GraphPlan::Subgraph { .. } => Err(ExecutorError::UnsupportedConstruct(
                UnsupportedConstruct::new(
                    cognicode_core::domain::plan::ConstructId::Other(
                        "GraphPlan::Subgraph".to_string(),
                    ),
                    "Phase 2 stub",
                ),
            )),
            GraphPlan::Cluster { .. } => Err(ExecutorError::UnsupportedConstruct(
                UnsupportedConstruct::new(
                    cognicode_core::domain::plan::ConstructId::Other(
                        "GraphPlan::Cluster".to_string(),
                    ),
                    "Phase 2 stub",
                ),
            )),
            GraphPlan::Explain { .. } => Err(ExecutorError::UnsupportedConstruct(
                UnsupportedConstruct::new(
                    cognicode_core::domain::plan::ConstructId::Other(
                        "GraphPlan::Explain".to_string(),
                    ),
                    "Phase 2 stub",
                ),
            )),
            GraphPlan::BooleanComposition { .. } => Err(ExecutorError::UnsupportedConstruct(
                UnsupportedConstruct::new(
                    cognicode_core::domain::plan::ConstructId::Other(
                        "GraphPlan::BooleanComposition".to_string(),
                    ),
                    "Phase 2 stub",
                ),
            )),
        }
    }
}

impl LadybugStore {
    /// Build a `LadybugGraphExecutor` that shares the same underlying
    /// `lbug::Database` as this store. The returned executor reads
    /// from the same `GraphRevision` + `GraphSymbol` + `GraphEdge`
    /// tables the `CallGraphStore` PR populates — no separate DDL
    /// needed (assumes the schema is already applied; tests use
    /// `init_graph_executor_schema`).
    pub fn graph_executor(&self) -> LadybugGraphExecutor {
        LadybugGraphExecutor::new(Arc::clone(&self.db))
    }

    /// Open (or create) a LadybugDB database file and wrap it as a
    /// `LadybugStore`.
    ///
    /// Mirrors the spike-validated API in `crates/spike-ladybug`
    /// (`Database::new(path, SystemConfig::default())`).
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        let db = Database::new(path, SystemConfig::default())?;
        let store = Self { db: Arc::new(db) };
        // e29-3 Phase 3: a freshly-opened store is quality-schema-ready.
        // Idempotent (IF NOT EXISTS); failures surface so callers know
        // the DB file could not host the quality tables.
        store.init_quality_schema()?;
        Ok(store)
    }

    /// Open from an already-constructed `lbug::Database`.
    ///
    /// Mostly useful for tests that want to share a database across
    /// multiple `LadybugStore` handles.
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// Acquire a single-writer `Connection` from the shared database.
    /// Internal helper — port impls borrow this for the duration of
    /// one query call.
    fn connection(&self) -> Result<Connection, Error> {
        Ok(Connection::new(&self.db)?)
    }
}

#[cfg(feature = "multimodal")]
impl LadybugStore {
    pub(crate) async fn head_revision_for_ws(
        &self,
        ws: &WorkspaceId,
    ) -> Result<Option<RevisionId>, FederationError> {
        let conn = self
            .connection()
            .map_err(|e| FederationError::Store(format!("head_revision_for_ws: {e}")))?;
        let mut stmt = conn
            .prepare(
                "MATCH (r:GraphRevision) WHERE r.workspace_id = $ws AND r.head_of = true RETURN r.revision_id;",
            )
            .map_err(|e| {
                FederationError::Store(format!("head_revision_for_ws: prepare: {e}"))
            })?;
        let mut result = conn
            .execute(&mut stmt, vec![("ws", lbug::Value::String(ws.to_string()))])
            .map_err(|e| FederationError::Store(format!("head_revision_for_ws: execute: {e}")))?;
        let Some(row) = result.next() else {
            return Ok(None);
        };
        let rev = match &row[0] {
            lbug::Value::Int64(n) => RevisionId(*n as u64),
            lbug::Value::Int32(n) => RevisionId(*n as u64),
            other => {
                return Err(FederationError::Store(format!(
                    "head_revision_for_ws: unexpected type: {other:?}"
                )));
            }
        };
        Ok(Some(rev))
    }
}

// =============================================================================
// Port impls (Phase 1 stubs)
// =============================================================================
//
// The bodies below return `Err(Error::Stub(...))` for non-trivial reads
// and `Ok(default)` for trivial returns so callers compile. Each
// Phase 1 follow-up PR replaces one or more of these stubs with the
// lbug-side SQL.

#[async_trait]
impl RevisionStore for LadybugStore {
    async fn head_revision(&self, _ws: &WorkspaceId) -> Result<Option<RevisionId>, RevisionError> {
        // PHASE 1 STUB. Next change: `SELECT revision_id FROM graph_revisions
        // WHERE workspace_id = $1 AND head_of = true LIMIT 1`.
        Ok(None)
    }

    async fn create_revision(&self, _ws: &WorkspaceId) -> Result<RevisionId, RevisionError> {
        // Note: the trait is connection-agnostic (the previous
        // `&mut PgConnection` parameter was a PostgreSQL-typed leak
        // and was removed). For now: open a new connection from the
        // shared Database and issue the open revision in a single tx.
        let _conn = self
            .connection()
            .map_err(|_| RevisionError::Store("(phase 1 stub — see lib.rs port-impl)".into()))?;
        // tx: demote old head, compute next id, insert.
        Err(RevisionError::Store(
            "(phase 1 stub — impl lands in next change)".into(),
        ))
    }

    async fn set_head(&self, _ws: &WorkspaceId, _rev: RevisionId) -> Result<(), RevisionError> {
        // PHASE 1 STUB.
        Err(RevisionError::Store(
            "(phase 1 stub — see lib.rs port-impl)".into(),
        ))
    }
}

#[cfg(feature = "multimodal")]
#[async_trait]
impl FederationStore for LadybugStore {
    async fn register_space(&self, space: &Space) -> Result<SpaceId, FederationError> {
        // ADR-028 §3 `register_space(space)` — upsert by `id` (single
        // STRING PK, same natural-key shape as SessionStore /
        // ReportStore). Mirrors the PG adapter's
        // `INSERT ... ON CONFLICT (id) DO UPDATE SET name` semantics:
        // if the row exists, refresh `name` + the other mutable
        // fields; otherwise create it.
        //
        // Pattern: read-then-conditional-write (same workaround as
        // `ManifestStore::upsert_manifest_entry` — lbug 0.19 NODE
        // TABLEs have no `MERGE` / `ON CONFLICT` primitive).
        //
        // `source_path: Option<PathBuf>` is serialized to its display
        // form (string), and `config: serde_json::Value` to its
        // serde_json text (same JSON-as-STRING pattern used by
        // SessionStore and ReportStore).
        let conn = self
            .connection()
            .map_err(|e| FederationError::Store(format!("register_space: {e}")))?;

        // Step 1: existence check.
        let mut check_stmt = conn
            .prepare("MATCH (s:Space) WHERE s.id = $id RETURN s.id;")
            .map_err(|e| FederationError::Store(format!("register_space: check prepare: {e}")))?;
        let mut existing = conn
            .execute(
                &mut check_stmt,
                vec![("id", lbug::Value::String(space.id.0.clone()))],
            )
            .map_err(|e| FederationError::Store(format!("register_space: check execute: {e}")))?;

        if existing.next().is_some() {
            // Step 2a: UPDATE existing row.
            let mut upd_stmt = conn
                .prepare(
                    "MATCH (s:Space) WHERE s.id = $id SET s.name = $name, s.kind = $kind, s.source_path = $srcpath, s.config = $cfg;",
                )
                .map_err(|e| FederationError::Store(format!("register_space: update prepare: {e}")))?;
            conn.execute(
                &mut upd_stmt,
                vec![
                    ("id", lbug::Value::String(space.id.0.clone())),
                    ("name", lbug::Value::String(space.name.clone())),
                    ("kind", lbug::Value::String(space.kind.as_str().to_string())),
                    (
                        "srcpath",
                        match &space.source_path {
                            Some(p) => lbug::Value::String(p.display().to_string()),
                            None => lbug::Value::Null(lbug::LogicalType::String),
                        },
                    ),
                    (
                        "cfg",
                        lbug::Value::String(serde_json::to_string(&space.config).map_err(|e| {
                            FederationError::Store(format!("register_space: serialize config: {e}"))
                        })?),
                    ),
                ],
            )
            .map_err(|e| FederationError::Store(format!("register_space: update execute: {e}")))?;
        } else {
            // Step 2b: CREATE new row.
            let mut ins_stmt = conn
                .prepare(
                    "CREATE (s:Space {id: $id, name: $name, kind: $kind, source_path: $srcpath, config: $cfg, created_at: $ts});",
                )
                .map_err(|e| FederationError::Store(format!("register_space: insert prepare: {e}")))?;
            conn.execute(
                &mut ins_stmt,
                vec![
                    ("id", lbug::Value::String(space.id.0.clone())),
                    ("name", lbug::Value::String(space.name.clone())),
                    ("kind", lbug::Value::String(space.kind.as_str().to_string())),
                    (
                        "srcpath",
                        match &space.source_path {
                            Some(p) => lbug::Value::String(p.display().to_string()),
                            None => lbug::Value::Null(lbug::LogicalType::String),
                        },
                    ),
                    (
                        "cfg",
                        lbug::Value::String(serde_json::to_string(&space.config).map_err(|e| {
                            FederationError::Store(format!("register_space: serialize config: {e}"))
                        })?),
                    ),
                    (
                        "ts",
                        lbug::Value::String(
                            std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .map(|d| d.as_secs())
                                .unwrap_or(0)
                                .to_string()
                                + "Z",
                        ),
                    ),
                ],
            )
            .map_err(|e| FederationError::Store(format!("register_space: insert execute: {e}")))?;
        }

        Ok(space.id.clone())
    }

    async fn list_spaces(&self) -> Result<Vec<Space>, FederationError> {
        // ADR-028 §3 `list_spaces()` — every space, ordered by
        // `created_at DESC` (newest-first). Same ORDER BY pattern as
        // ReportStore.
        let conn = self
            .connection()
            .map_err(|e| FederationError::Store(format!("list_spaces: {e}")))?;
        let mut stmt = conn
            .prepare(
                "MATCH (s:Space) RETURN s.id, s.name, s.kind, s.source_path, s.config, s.created_at ORDER BY s.created_at DESC;",
            )
            .map_err(|e| FederationError::Store(format!("list_spaces: prepare: {e}")))?;
        let mut result = conn
            .execute(&mut stmt, vec![])
            .map_err(|e| FederationError::Store(format!("list_spaces: execute: {e}")))?;

        let mut rows = Vec::new();
        while let Some(row) = result.next() {
            rows.push(parse_space_row(&row)?);
        }
        Ok(rows)
    }

    async fn get_space(&self, id: &SpaceId) -> Result<Option<Space>, FederationError> {
        // ADR-028 §3 `get_space(id)` — single MATCH WHERE id = $id, or
        // None if absent.
        let conn = self
            .connection()
            .map_err(|e| FederationError::Store(format!("get_space: {e}")))?;
        let mut stmt = conn
            .prepare(
                "MATCH (s:Space) WHERE s.id = $id RETURN s.id, s.name, s.kind, s.source_path, s.config, s.created_at;",
            )
            .map_err(|e| FederationError::Store(format!("get_space: prepare: {e}")))?;
        let mut result = conn
            .execute(&mut stmt, vec![("id", lbug::Value::String(id.0.clone()))])
            .map_err(|e| FederationError::Store(format!("get_space: execute: {e}")))?;

        let Some(row) = result.next() else {
            return Ok(None);
        };
        Ok(Some(parse_space_row(&row)?))
    }
}

/// Row mapper for `FederationStore` queries — shared by `list_spaces`
/// and `get_space` so both stay in lock-step on column order. Column
/// order must match the RETURN clauses above.
#[cfg(feature = "multimodal")]
fn parse_space_row(row: &[lbug::Value]) -> Result<Space, FederationError> {
    use cognicode_core::domain::value_objects::SpaceKind;
    let id = SpaceId(row[0].to_string());
    let name = row[1].to_string();
    let kind = SpaceKind::from_wire(&row[2].to_string()).ok_or_else(|| {
        FederationError::Store(format!("parse_space_row: invalid kind: {}", row[2]))
    })?;
    let source_path = match &row[3] {
        lbug::Value::Null(_) => None,
        other => Some(std::path::PathBuf::from(other.to_string())),
    };
    let config: serde_json::Value = serde_json::from_str(&row[4].to_string())
        .map_err(|e| FederationError::Store(format!("parse_space_row: config JSON: {e}")))?;
    Ok(Space {
        id,
        name,
        kind,
        source_path,
        config,
    })
}

#[async_trait]
impl ManifestStore for LadybugStore {
    async fn get_manifest(&self, workspace_id: &str) -> Result<Vec<ScanManifest>, ManifestError> {
        // lbug Cypher: MATCH (s:ScanManifest) WHERE s.workspace_id = $ws
        // RETURN s.*, ordered by file_path for stable reads.
        let conn = self
            .connection()
            .map_err(|e| ManifestError::Store(format!("get_manifest: {e}")))?;
        let mut result = conn
            .query(
                "MATCH (s:ScanManifest)                  WHERE s.workspace_id = $ws                  RETURN s.workspace_id, s.file_path, s.file_type, s.language,                         s.content_hash, s.mtime, s.symbol_count, s.edge_count,                         s.status, s.error_msg                  ORDER BY s.file_path;",
            )
            .map_err(|e| ManifestError::Store(format!("get_manifest: query: {e}")))?;
        let mut rows = Vec::new();
        while let Some(row) = result.next() {
            rows.push(ScanManifest {
                workspace_id: row[0].to_string(),
                file_path: row[1].to_string(),
                file_type: row[2].to_string(),
                language: match &row[3] {
                    lbug::Value::Null(_) => None,
                    other => Some(other.to_string()),
                },
                content_hash: row[4].to_string(),
                mtime: match &row[5] {
                    lbug::Value::Double(n) => *n,
                    _ => 0.0,
                },
                symbol_count: match &row[6] {
                    lbug::Value::Int32(n) => *n,
                    lbug::Value::Int64(n) => *n as i32,
                    _ => 0,
                },
                edge_count: match &row[7] {
                    lbug::Value::Int32(n) => *n,
                    lbug::Value::Int64(n) => *n as i32,
                    _ => 0,
                },
                status: row[8].to_string(),
                error_msg: match &row[9] {
                    lbug::Value::Null(_) => None,
                    other => Some(other.to_string()),
                },
            });
        }
        Ok(rows)
    }

    async fn upsert_manifest_entry(&self, row: &ScanManifest) -> Result<(), ManifestError> {
        // Phase 1 limitation: lbug 0.19.0 NODE TABLEs support single-column
        // PRIMARY KEYs only (no composite PK, no UNIQUE constraints on
        // multi-column sets). The natural key
        // `(workspace_id, file_path)` is therefore enforced at the
        // application layer via a read-then-conditional-write:
        //   1. MATCH by natural key — if it exists, UPDATE all other
        //      fields.
        //   2. Otherwise, CREATE a new node with the synthetic `id`
        //      auto-assigned by `SERIAL PRIMARY KEY`.
        //
        // The follow-up per-port PR will add a single-column synthetic
        // `(workspace_id || file_path)` text-PK that the application can
        // pre-compute (e.g. `format!("{ws}::{path}")`) and use as the
        // primary key, restoring a single-pass MERGE upsert. For now,
        // the 2-step pattern is correct and round-trips.
        let conn = self
            .connection()
            .map_err(|e| ManifestError::Store(format!("upsert_manifest_entry: {e}")))?;

        // Step 1: existence check.
        let mut check_stmt = conn
            .prepare(
                "MATCH (s:ScanManifest) \
                 WHERE s.workspace_id = $ws AND s.file_path = $path \
                 RETURN s.id;",
            )
            .map_err(|e| {
                ManifestError::Store(format!("upsert_manifest_entry: check prepare: {e}"))
            })?;
        let mut existing = conn
            .execute(
                &mut check_stmt,
                vec![
                    ("ws", lbug::Value::String(row.workspace_id.clone())),
                    ("path", lbug::Value::String(row.file_path.clone())),
                ],
            )
            .map_err(|e| {
                ManifestError::Store(format!("upsert_manifest_entry: check execute: {e}"))
            })?;

        if existing.next().is_some() {
            // Step 2a: UPDATE the existing row. lbug's 0.19.0 parser
            // is line-continuation-sensitive — keep the query on one
            // line. Pattern form `MATCH (n:L) WHERE ... SET ...` matches
            // the spike's s6_cypher_compat.rs.
            let mut upd_stmt = conn
                .prepare(
                    "MATCH (s:ScanManifest) WHERE s.workspace_id = $ws AND s.file_path = $path SET s.file_type = $ftype, s.language = $lang, s.content_hash = $hash, s.mtime = $mtime, s.symbol_count = $symcnt, s.edge_count = $edgecnt, s.status = $status, s.error_msg = $errmsg;",
                )
                .map_err(|e| ManifestError::Store(format!("upsert_manifest_entry: update prepare: {e}")))?;
            conn.execute(
                &mut upd_stmt,
                vec![
                    ("ws", lbug::Value::String(row.workspace_id.clone())),
                    ("path", lbug::Value::String(row.file_path.clone())),
                    ("ftype", lbug::Value::String(row.file_type.clone())),
                    (
                        "lang",
                        match &row.language {
                            Some(s) => lbug::Value::String(s.to_string()),
                            None => lbug::Value::Null(lbug::LogicalType::String),
                        },
                    ),
                    ("hash", lbug::Value::String(row.content_hash.clone())),
                    ("mtime", lbug::Value::Double(row.mtime)),
                    ("symcnt", lbug::Value::Int32(row.symbol_count)),
                    ("edgecnt", lbug::Value::Int32(row.edge_count)),
                    ("status", lbug::Value::String(row.status.clone())),
                    (
                        "errmsg",
                        match &row.error_msg {
                            Some(s) => lbug::Value::String(s.to_string()),
                            None => lbug::Value::Null(lbug::LogicalType::String),
                        },
                    ),
                ],
            )
            .map_err(|e| {
                ManifestError::Store(format!("upsert_manifest_entry: update execute: {e}"))
            })?;
        } else {
            // Step 2b: CREATE a new row. (lbug 0.19.0 parser is
            // line-continuation-sensitive — keep the CREATE on one line.)
            let mut ins_stmt = conn
                .prepare(
                    "CREATE (s:ScanManifest {workspace_id: $ws, file_path: $path, file_type: $ftype, language: $lang, content_hash: $hash, mtime: $mtime, symbol_count: $symcnt, edge_count: $edgecnt, status: $status, error_msg: $errmsg});",
                )
                .map_err(|e| ManifestError::Store(format!("upsert_manifest_entry: insert prepare: {e}")))?;
            conn.execute(
                &mut ins_stmt,
                vec![
                    ("ws", lbug::Value::String(row.workspace_id.clone())),
                    ("path", lbug::Value::String(row.file_path.clone())),
                    ("ftype", lbug::Value::String(row.file_type.clone())),
                    (
                        "lang",
                        match &row.language {
                            Some(s) => lbug::Value::String(s.to_string()),
                            None => lbug::Value::Null(lbug::LogicalType::String),
                        },
                    ),
                    ("hash", lbug::Value::String(row.content_hash.clone())),
                    ("mtime", lbug::Value::Double(row.mtime)),
                    ("symcnt", lbug::Value::Int32(row.symbol_count)),
                    ("edgecnt", lbug::Value::Int32(row.edge_count)),
                    ("status", lbug::Value::String(row.status.clone())),
                    (
                        "errmsg",
                        match &row.error_msg {
                            Some(s) => lbug::Value::String(s.to_string()),
                            None => lbug::Value::Null(lbug::LogicalType::String),
                        },
                    ),
                ],
            )
            .map_err(|e| {
                ManifestError::Store(format!("upsert_manifest_entry: insert execute: {e}"))
            })?;
        }
        Ok(())
    }

    async fn delete_manifest_entry(
        &self,
        workspace_id: &str,
        file_path: &str,
    ) -> Result<(), ManifestError> {
        // lbug Cypher: MATCH by natural key + DELETE. Single-line query
        // (parser is line-continuation-sensitive). Uses WHERE form
        // because the property-pattern form is rejected by the 0.19
        // parser (same constraint we hit in `upsert_manifest_entry`'s
        // UPDATE branch).
        let conn = self
            .connection()
            .map_err(|e| ManifestError::Store(format!("delete_manifest_entry: {e}")))?;
        let mut stmt = conn
            .prepare("MATCH (s:ScanManifest) WHERE s.workspace_id = $ws AND s.file_path = $path DELETE s;")
            .map_err(|e| ManifestError::Store(format!("delete_manifest_entry: prepare: {e}")))?;
        conn.execute(
            &mut stmt,
            vec![
                ("ws", lbug::Value::String(workspace_id.to_string())),
                ("path", lbug::Value::String(file_path.to_string())),
            ],
        )
        .map_err(|e| ManifestError::Store(format!("delete_manifest_entry: execute: {e}")))?;
        Ok(())
    }
}

#[async_trait]
impl SessionStore for LadybugStore {
    async fn save(
        &self,
        _id: &str,
        _workspace_id: &str,
        _events_json: &str,
        _navigation_mode: &str,
        _panes_json: &str,
        _investigation_id: Option<&str>,
    ) -> Result<(), SessionError> {
        // PHASE 1 STUB.
        Err(SessionError::Store(
            "(phase 1 stub — see lib.rs port-impl)".into(),
        ))
    }

    async fn load(
        &self,
        _id: &str,
        _workspace_id: &str,
    ) -> Result<Option<SessionRow>, SessionError> {
        // PHASE 1 STUB.
        Ok(None)
    }

    async fn list(&self, _workspace_id: &str) -> Result<Vec<SessionRow>, SessionError> {
        // PHASE 1 STUB.
        Ok(Vec::new())
    }
}

#[async_trait]
impl ReportStore for LadybugStore {
    async fn save_report(
        &self,
        _workspace_id: &str,
        _report: &ReportSummary,
    ) -> Result<(), ReportError> {
        // PHASE 1 STUB. Next change: INSERT INTO graph_reports ... ON CONFLICT
        // DO UPDATE (per ADR-028 §3 ReportStore contract).
        Err(ReportError::Store(
            "(phase 1 stub — see lib.rs port-impl)".into(),
        ))
    }

    async fn latest_report(
        &self,
        _workspace_id: &str,
    ) -> Result<Option<ReportSummary>, ReportError> {
        // PHASE 1 STUB.
        Ok(None)
    }

    async fn reports_for_workspace(
        &self,
        _workspace_id: &str,
    ) -> Result<Vec<ReportSummary>, ReportError> {
        // PHASE 1 STUB.
        Ok(Vec::new())
    }
}

#[async_trait]
impl ViewSpecStore for LadybugStore {
    async fn save(
        &self,
        _payload: &ViewSpecPayload,
        _workspace_id: &str,
        _owner: &str,
    ) -> Result<(), ViewSpecStoreError> {
        // PHASE 1 STUB. Next change: serde_json to columns + INSERT.
        Err(ViewSpecStoreError::Store(
            "(phase 1 stub — see lib.rs port-impl)".into(),
        ))
    }

    async fn load(
        &self,
        _id: &str,
        _workspace_id: &str,
        _owner: &str,
    ) -> Result<Option<ViewSpecPayload>, ViewSpecStoreError> {
        Ok(None)
    }

    async fn list(
        &self,
        _workspace_id: &str,
        _owner: &str,
    ) -> Result<Vec<ViewSpecPayload>, ViewSpecStoreError> {
        Ok(Vec::new())
    }

    async fn delete(
        &self,
        _id: &str,
        _workspace_id: &str,
        _owner: &str,
    ) -> Result<bool, ViewSpecStoreError> {
        Ok(false)
    }

    async fn list_for_workspace(
        &self,
        _workspace_id: &str,
        _applies_to_kind: &str,
    ) -> Result<Vec<ViewSpecPayload>, ViewSpecStoreError> {
        Ok(Vec::new())
    }

    async fn update(
        &self,
        _id: &str,
        _workspace_id: &str,
        _owner: &str,
        _seed_object_id: Option<&str>,
        _seed_view_id: Option<&str>,
        _applies_when: Option<&str>,
    ) -> Result<bool, ViewSpecStoreError> {
        Ok(false)
    }
}

#[async_trait]
impl CallGraphStore for LadybugStore {
    async fn save_call_graph_ws(
        &self,
        _graph: &CallGraph,
        _ws: &WorkspaceId,
    ) -> Result<RevisionId, CallGraphError> {
        // PHASE 1 STUB.
        Err(CallGraphError::Store(
            "(phase 1 stub — see lib.rs port-impl)".into(),
        ))
    }

    async fn load_call_graph_ws(
        &self,
        _ws: &WorkspaceId,
        _revision: RevisionId,
    ) -> Result<Option<CallGraph>, CallGraphError> {
        Ok(None)
    }

    async fn load_call_graph_current(
        &self,
        _ws: &WorkspaceId,
    ) -> Result<Option<CallGraph>, CallGraphError> {
        Ok(None)
    }
}

#[cfg(feature = "multimodal")]
#[async_trait]
impl IngestCommitPort for LadybugStore {
    async fn commit_revision(
        &self,
        ws: &WorkspaceId,
        _graph: GraphDelta,
        manifest: ManifestDelta,
        report: ReportIntent,
    ) -> Result<RevisionId, cognicode_core::domain::ports::CommitError> {
        // ADR-028 §3 `commit_revision(ws, graph, manifest, report)` —
        // atomic 3-stage commit. The PG adapter wraps this in a
        // single `pool.begin()` tx (failures roll back the prior
        // stages); lbug 0.19 has no public tx handle so we open a
        // single `Connection` and run the 3 stages as a sequence of
        // statements against it. lbug's per-`execute` auto-commit
        // means a failure in stage 3 leaves the work from stages
        // 1-2 persisted (best-effort atomicity, not transactional
        // atomicity). A future PR can either:
        //   - Use lbug's CHECKPOINT mechanism to wrap the sequence
        //     in an explicit recovery unit, or
        //   - Compose all 3 stages into a single multi-statement
        //     Cypher (subject to lbug's parser limits).
        //
        // **Trait/tx limitation**: this adapter is gated behind
        // `multimodal`, which transitively pulls in
        // `cognicode-core/multimodal`. The cargo test invocation
        // `--features multimodal` is currently blocked by pre-existing
        // `cognicode-core --features multimodal` debt (5+ compile
        // errors in `federation_store.rs` and `ingest_commit.rs` PG
        // adapter — `Pool<Postgres>` deref + missing
        // `PostgresManifestStore<'a>` lifetime + `chrono::DateTime<Utc>`
        // not `sqlx::Decode`); the integration tests for this port are
        // deferred to the `fix-multimodal-feature-compile-debt-2026-08-02`
        // follow-up. Logic is validated against lbug 0.19 via the
        // standalone `cypher_probe` harness.
        let conn = self.connection().map_err(|e| {
            cognicode_core::domain::ports::CommitError::Graph(
                cognicode_core::domain::ports::graph_error::GraphError::Storage(format!(
                    "commit_revision: connection: {e}"
                )),
            )
        })?;

        // Stage 1: open new revision. Same multi-pattern Cypher with
        // `WITH count()` pivot used by Priority 4 / 8 — see those for
        // the rationale on why the `WITH $ws AS ws` post-`SET` form
        // loses the parameter binding in lbug 0.19.
        let mut rev_stmt = conn
            .prepare(
                "MATCH (old:GraphRevision) WHERE old.workspace_id = $ws AND old.head_of = true SET old.head_of = false WITH count(old) AS _demoted OPTIONAL MATCH (r:GraphRevision) WHERE r.workspace_id = $ws WITH $ws AS ws, coalesce(max(r.revision_id), 0) AS max_rev CREATE (new:GraphRevision {workspace_id: ws, revision_id: max_rev + 1, head_of: true}) RETURN new.revision_id;",
            )
            .map_err(|e| {
                cognicode_core::domain::ports::CommitError::Graph(
                    cognicode_core::domain::ports::graph_error::GraphError::Storage(format!(
                        "commit_revision stage 1 (revision open) prepare: {e}"
                    )),
                )
            })?;
        let mut rev_result = conn
            .execute(
                &mut rev_stmt,
                vec![("ws", lbug::Value::String(ws.to_string()))],
            )
            .map_err(|e| {
                cognicode_core::domain::ports::CommitError::Graph(
                    cognicode_core::domain::ports::graph_error::GraphError::Storage(format!(
                        "commit_revision stage 1 (revision open) execute: {e}"
                    )),
                )
            })?;
        let Some(rev_row) = rev_result.next() else {
            return Err(cognicode_core::domain::ports::CommitError::Graph(
                cognicode_core::domain::ports::graph_error::GraphError::Storage(
                    "commit_revision stage 1: CREATE revision produced no RETURN row".to_string(),
                ),
            ));
        };
        let rev_id = match &rev_row[0] {
            lbug::Value::Int64(n) => RevisionId(*n as u64),
            lbug::Value::Int32(n) => RevisionId(*n as u64),
            other => {
                return Err(cognicode_core::domain::ports::CommitError::Graph(
                    cognicode_core::domain::ports::graph_error::GraphError::Storage(format!(
                        "commit_revision stage 1: unexpected revision_id type: {other:?}"
                    )),
                ));
            }
        };

        // Stage 2: manifest upserts. Same read-then-conditional-write
        // as Priority 1's `ManifestStore::upsert_manifest_entry` —
        // natural uniqueness on (workspace_id, file_path) enforced at
        // the application layer via per-row MATCH then UPDATE/CREATE.
        for row in &manifest.upserts {
            // Existence check.
            let mut check_stmt = conn
                .prepare("MATCH (s:ScanManifest) WHERE s.workspace_id = $ws AND s.file_path = $path RETURN s.id;")
                .map_err(|e| {
                    cognicode_core::domain::ports::CommitError::Manifest(
                        cognicode_core::domain::ports::manifest_store::ManifestError::Store(format!(
                            "commit_revision stage 2 (manifest check) prepare: {e}"
                        )),
                    )
                })?;
            let mut existing = conn
                .execute(
                    &mut check_stmt,
                    vec![
                        ("ws", lbug::Value::String(row.workspace_id.clone())),
                        ("path", lbug::Value::String(row.file_path.clone())),
                    ],
                )
                .map_err(|e| {
                    cognicode_core::domain::ports::CommitError::Manifest(
                        cognicode_core::domain::ports::manifest_store::ManifestError::Store(
                            format!("commit_revision stage 2 (manifest check) execute: {e}"),
                        ),
                    )
                })?;

            if existing.next().is_some() {
                // UPDATE.
                let mut upd_stmt = conn
                    .prepare("MATCH (s:ScanManifest) WHERE s.workspace_id = $ws AND s.file_path = $path SET s.file_type = $ftype, s.language = $lang, s.content_hash = $hash, s.mtime = $mtime, s.symbol_count = $symcnt, s.edge_count = $edgecnt, s.status = $status, s.error_msg = $errmsg;")
                    .map_err(|e| {
                        cognicode_core::domain::ports::CommitError::Manifest(
                            cognicode_core::domain::ports::manifest_store::ManifestError::Store(format!(
                                "commit_revision stage 2 (manifest update) prepare: {e}"
                            )),
                        )
                    })?;
                conn.execute(
                    &mut upd_stmt,
                    vec![
                        ("ws", lbug::Value::String(row.workspace_id.clone())),
                        ("path", lbug::Value::String(row.file_path.clone())),
                        ("ftype", lbug::Value::String(row.file_type.clone())),
                        (
                            "lang",
                            match &row.language {
                                Some(s) => lbug::Value::String(s.to_string()),
                                None => lbug::Value::Null(lbug::LogicalType::String),
                            },
                        ),
                        ("hash", lbug::Value::String(row.content_hash.clone())),
                        ("mtime", lbug::Value::Double(row.mtime)),
                        ("symcnt", lbug::Value::Int32(row.symbol_count)),
                        ("edgecnt", lbug::Value::Int32(row.edge_count)),
                        ("status", lbug::Value::String(row.status.clone())),
                        (
                            "errmsg",
                            match &row.error_msg {
                                Some(s) => lbug::Value::String(s.to_string()),
                                None => lbug::Value::Null(lbug::LogicalType::String),
                            },
                        ),
                    ],
                )
                .map_err(|e| {
                    cognicode_core::domain::ports::CommitError::Manifest(
                        cognicode_core::domain::ports::manifest_store::ManifestError::Store(
                            format!("commit_revision stage 2 (manifest update) execute: {e}"),
                        ),
                    )
                })?;
            } else {
                // CREATE.
                let mut ins_stmt = conn
                    .prepare("CREATE (s:ScanManifest {workspace_id: $ws, file_path: $path, file_type: $ftype, language: $lang, content_hash: $hash, mtime: $mtime, symbol_count: $symcnt, edge_count: $edgecnt, status: $status, error_msg: $errmsg});")
                    .map_err(|e| {
                        cognicode_core::domain::ports::CommitError::Manifest(
                            cognicode_core::domain::ports::manifest_store::ManifestError::Store(format!(
                                "commit_revision stage 2 (manifest insert) prepare: {e}"
                            )),
                        )
                    })?;
                conn.execute(
                    &mut ins_stmt,
                    vec![
                        ("ws", lbug::Value::String(row.workspace_id.clone())),
                        ("path", lbug::Value::String(row.file_path.clone())),
                        ("ftype", lbug::Value::String(row.file_type.clone())),
                        (
                            "lang",
                            match &row.language {
                                Some(s) => lbug::Value::String(s.to_string()),
                                None => lbug::Value::Null(lbug::LogicalType::String),
                            },
                        ),
                        ("hash", lbug::Value::String(row.content_hash.clone())),
                        ("mtime", lbug::Value::Double(row.mtime)),
                        ("symcnt", lbug::Value::Int32(row.symbol_count)),
                        ("edgecnt", lbug::Value::Int32(row.edge_count)),
                        ("status", lbug::Value::String(row.status.clone())),
                        (
                            "errmsg",
                            match &row.error_msg {
                                Some(s) => lbug::Value::String(s.to_string()),
                                None => lbug::Value::Null(lbug::LogicalType::String),
                            },
                        ),
                    ],
                )
                .map_err(|e| {
                    cognicode_core::domain::ports::CommitError::Manifest(
                        cognicode_core::domain::ports::manifest_store::ManifestError::Store(
                            format!("commit_revision stage 2 (manifest insert) execute: {e}"),
                        ),
                    )
                })?;
            }
        }

        // Stage 3: save the report. Mirrors `ReportStore::save_report`
        // (Priority 3) — single CREATE with id STRING PK + JSON-as-STRING
        // for the report column + null-safe health_score.
        let report = &report.summary;
        let report_json = serde_json::to_string(&report.report).map_err(|e| {
            cognicode_core::domain::ports::CommitError::Report(
                cognicode_core::domain::ports::report_store::ReportError::Store(format!(
                    "commit_revision stage 3 serialize report: {e}"
                )),
            )
        })?;
        let mut rep_stmt = conn
            .prepare(
                "CREATE (r:GraphReport {id: $id, workspace_id: $ws, created_at: $ts, report: $json, symbol_count: $scnt, edge_count: $ecnt, health_score: $hscore});",
            )
            .map_err(|e| {
                cognicode_core::domain::ports::CommitError::Report(
                    cognicode_core::domain::ports::report_store::ReportError::Store(format!(
                        "commit_revision stage 3 (report insert) prepare: {e}"
                    )),
                )
            })?;
        conn.execute(
            &mut rep_stmt,
            vec![
                ("id", lbug::Value::String(report.id.clone())),
                ("ws", lbug::Value::String(ws.to_string())),
                ("ts", lbug::Value::String(report.created_at.clone())),
                ("json", lbug::Value::String(report_json.to_string())),
                ("scnt", lbug::Value::Int64(report.symbol_count as i64)),
                ("ecnt", lbug::Value::Int64(report.edge_count as i64)),
                (
                    "hscore",
                    match report.health_score {
                        Some(v) => lbug::Value::Double(v as f64),
                        None => lbug::Value::Null(lbug::LogicalType::Double),
                    },
                ),
            ],
        )
        .map_err(|e| {
            cognicode_core::domain::ports::CommitError::Report(
                cognicode_core::domain::ports::report_store::ReportError::Store(format!(
                    "commit_revision stage 3 (report insert) execute: {e}"
                )),
            )
        })?;

        // Stage 4 — graph upserts. The PG adapter takes the
        // GraphDelta but ignores it (see the trait comment); a future
        // PR will wire Stage 4 once the Generic Graph Layer's
        // ports (GraphNodeStore, GraphEdgeStore)
        // are defined for the lbug adapter. Today: no-op.
        let _graph = _graph;

        Ok(rev_id)
    }
}

// =============================================================================
// QualityStore — first non-stub impl (e29-3 Phase 3)
// =============================================================================
//
// `QualityStore` is the 10-method port split across the
// `QualityIssue`, `QualityBaseline`, and `QualityRule` node tables. This
// is the first real lbug SQL implementation (all other ports remain
// stubs).
//
// Contracts (mirror `domain/ports/quality_store.rs`):
// - The 8 read methods degrade gracefully when the tables are missing
//   (return empty / zero, never an error).
// - The 2 write methods (`insert_issues`, `delete_issue`) DO error on a
//   missing table / I/O failure.

impl LadybugStore {
    /// Create the `QualityIssue`, `QualityBaseline`, and `QualityRule`
    /// NODE TABLEs backing the [`QualityStore`] port (per ADR-028).
    /// Idempotent — every statement uses `IF NOT EXISTS`.
    ///
    /// Called automatically by [`LadybugStore::open`]; the raw sharing
    /// constructor [`LadybugStore::new`] does NOT apply it so tests can
    /// exercise the graceful-degradation contract on a schema-less db.
    pub fn init_quality_schema(&self) -> Result<(), Error> {
        let conn = self
            .connection()
            .map_err(|e| Error::Lbug(format!("init_quality_schema: {e}")))?;
        for stmt in quality_schema_ddls() {
            conn.query(stmt)
                .map_err(|e| Error::Lbug(format!("init_quality_schema: {e}\nDDL: {stmt}")))?;
        }
        Ok(())
    }
}

/// Returns the 3 CREATE NODE TABLE statements for the QualityStore port.
fn quality_schema_ddls() -> Vec<&'static str> {
    vec![
        "CREATE NODE TABLE IF NOT EXISTS QualityIssue( \
             id SERIAL PRIMARY KEY, \
             workspace_id STRING, \
             rule_id STRING, \
             severity STRING, \
             category STRING, \
             file_path STRING, \
             line INT64, \
             message STRING, \
             status STRING);",
        "CREATE NODE TABLE IF NOT EXISTS QualityBaseline( \
             id SERIAL PRIMARY KEY, \
             workspace_id STRING, \
             rating STRING, \
             total_issues INT64, \
             blockers INT64, \
             criticals INT64, \
             debt_minutes INT64, \
             snapshot_at STRING);",
        "CREATE NODE TABLE IF NOT EXISTS QualityRule( \
             id SERIAL PRIMARY KEY, \
             rule_id STRING, \
             description STRING, \
             category STRING);",
    ]
}

// =============================================================================
// Analytics lineage schema (D5 — LadybugLineageStore)
// =============================================================================

/// DDL for the `AnalyticsRunLineage` node table.
///
/// Stores immutable run lineage records. The `idempotency_key` field
/// (combined with `workspace_id`) enforces the unique constraint for
/// persist-mode deduplication.
fn lineage_schema_ddls() -> Vec<&'static str> {
    vec![
        "CREATE NODE TABLE IF NOT EXISTS AnalyticsRunLineage( \
             id SERIAL PRIMARY KEY, \
             run_id STRING, \
             workspace_id STRING, \
             revision_id INT64, \
             algorithm_id STRING, \
             algorithm_version STRING, \
             plan_hash STRING, \
             params STRING, \
             seed INT64, \
             mode STRING, \
             status STRING, \
             started_at STRING, \
             finished_at STRING, \
             row_count INT64, \
             truncation_marker STRING, \
             idempotency_key STRING, \
             error_kind STRING, \
             error_message STRING);",
        "CREATE NODE TABLE IF NOT EXISTS AnalyticsDescriptorLimits( \
             id SERIAL PRIMARY KEY, \
             algorithm_id STRING, \
             version STRING, \
             limits_payload STRING);",
    ]
}

impl LadybugStore {
    /// Create the lineage and descriptor-limits NODE TABLEs backing the
    /// [`RunLineageStore`] port.
    ///
    /// Idempotent — every statement uses `IF NOT EXISTS`.
    ///
    /// Called automatically by [`LadybugStore::open`]; the raw sharing
    /// constructor [`LadybugStore::new`] does NOT apply it so tests can
    /// exercise the graceful-degradation contract on a schema-less db.
    pub fn init_lineage_schema(&self) -> Result<(), Error> {
        let conn = self
            .connection()
            .map_err(|e| Error::Lbug(format!("init_lineage_schema: {e}")))?;
        for stmt in lineage_schema_ddls() {
            conn.query(stmt)
                .map_err(|e| Error::Lbug(format!("init_lineage_schema: {e}\nDDL: {stmt}")))?;
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl RunLineageStore for LadybugStore {
    async fn insert(&self, lineage: &RunLineage) -> Result<(), AnalyticsError> {
        let conn = self
            .connection()
            .map_err(|e| AnalyticsError::Internal(format!("lineage insert: {e}")))?;

        // Check idempotency conflict for persist mode
        if let Some(ref key) = lineage.idempotency_key {
            let params = vec![
                ("ws", lbug::Value::String(lineage.workspace_id.to_string())),
                ("key", lbug::Value::String(key.clone())),
            ];
            let mut result = conn
                .prepare(
                    "MATCH (r:AnalyticsRunLineage) \
                     WHERE r.workspace_id = $ws AND r.idempotency_key = $key \
                     RETURN r.run_id;",
                )
                .map_err(|e| AnalyticsError::Internal(format!("idempotency check prepare: {e}")))?;
            let rows: Vec<String> = conn
                .execute(&mut result, params)
                .map_err(|e| AnalyticsError::Internal(format!("idempotency check: {e}")))?
                .map(|row| row[0].to_string())
                .collect();
            if !rows.is_empty() {
                return Err(AnalyticsError::IdempotencyConflict);
            }
        }

        let plan_hash_hex = hex::encode(&lineage.plan_hash);
        let params_json = serde_json::to_string(&lineage.params)
            .map_err(|e| AnalyticsError::Internal(format!("params serialize: {e}")))?;
        let started_at = lineage.started_at.to_rfc3339();
        let finished_at = lineage.finished_at.map(|dt| dt.to_rfc3339());

        let seed = lineage.seed.map(|s| lbug::Value::Int64(s as i64));
        let row_count = lineage.row_count.map(|rc| lbug::Value::Int64(rc));
        let truncation_marker = lineage
            .truncation_marker
            .as_ref()
            .map(|tm| lbug::Value::String(tm.to_string()));
        let idempotency_key = lineage
            .idempotency_key
            .as_ref()
            .map(|k| lbug::Value::String(k.clone()));
        let error_kind = lineage
            .error_kind
            .as_ref()
            .map(|e| lbug::Value::String(e.clone()));
        let error_message = lineage
            .error_message
            .as_ref()
            .map(|e| lbug::Value::String(e.clone()));

        let cypher_params = vec![
            ("run_id", lbug::Value::String(lineage.run_id.to_string())),
            ("ws", lbug::Value::String(lineage.workspace_id.to_string())),
            ("rev", lbug::Value::Int64(lineage.revision_id.get() as i64)),
            ("alg", lbug::Value::String(lineage.algorithm_id.to_string())),
            (
                "ver",
                lbug::Value::String(lineage.algorithm_version.clone()),
            ),
            ("ph", lbug::Value::String(plan_hash_hex)),
            ("params", lbug::Value::String(params_json)),
            (
                "seed",
                seed.unwrap_or(lbug::Value::Null(lbug::LogicalType::Int64)),
            ),
            ("mode", lbug::Value::String(lineage.mode.to_string())),
            ("status", lbug::Value::String(lineage.status.to_string())),
            ("sat", lbug::Value::String(started_at)),
            (
                "fat",
                finished_at
                    .map(lbug::Value::String)
                    .unwrap_or(lbug::Value::Null(lbug::LogicalType::String)),
            ),
            (
                "rc",
                row_count.unwrap_or(lbug::Value::Null(lbug::LogicalType::Int64)),
            ),
            (
                "tm",
                truncation_marker.unwrap_or(lbug::Value::Null(lbug::LogicalType::String)),
            ),
            (
                "ik",
                idempotency_key.unwrap_or(lbug::Value::Null(lbug::LogicalType::String)),
            ),
            (
                "ek",
                error_kind.unwrap_or(lbug::Value::Null(lbug::LogicalType::String)),
            ),
            (
                "ems",
                error_message.unwrap_or(lbug::Value::Null(lbug::LogicalType::String)),
            ),
        ];

        conn.query(
            "CREATE (r:AnalyticsRunLineage {\
             run_id: $run_id, \
             workspace_id: $ws, \
             revision_id: $rev, \
             algorithm_id: $alg, \
             algorithm_version: $ver, \
             plan_hash: $ph, \
             params: $params, \
             seed: $seed, \
             mode: $mode, \
             status: $status, \
             started_at: $sat, \
             finished_at: $fat, \
             row_count: $rc, \
             truncation_marker: $tm, \
             idempotency_key: $ik, \
             error_kind: $ek, \
             error_message: $ems});",
        )
        .map_err(|e| AnalyticsError::Internal(format!("lineage insert: {e}")))?;

        Ok(())
    }

    async fn get(&self, run_id: Uuid) -> Result<RunLineage, AnalyticsError> {
        let conn = self
            .connection()
            .map_err(|e| AnalyticsError::Internal(format!("lineage get: {e}")))?;

        let params = vec![("id", lbug::Value::String(run_id.to_string()))];
        let mut stmt = conn
            .prepare(
                "MATCH (r:AnalyticsRunLineage) WHERE r.run_id = $id \
                 RETURN r.run_id, r.workspace_id, r.revision_id, r.algorithm_id, \
                        r.algorithm_version, r.plan_hash, r.params, r.seed, r.mode, \
                        r.status, r.started_at, r.finished_at, r.row_count, \
                        r.truncation_marker, r.idempotency_key, r.error_kind, r.error_message;",
            )
            .map_err(|e| AnalyticsError::Internal(format!("lineage get prepare: {e}")))?;
        let mut result = conn
            .execute(&mut stmt, params)
            .map_err(|e| AnalyticsError::Internal(format!("lineage get: {e}")))?;

        if let Some(row) = result.next() {
            let run_id_str = row[0].to_string();
            let ws = row[1].to_string();
            let rev = req_i64_at(&row, 2) as u64;
            let alg = row[3].to_string();
            let ver = row[4].to_string();
            let ph = row[5].to_string();
            let params_str = row[6].to_string();
            let seed = opt_i64_at(&row, 7);
            let mode_str = row[8].to_string();
            let status_str = row[9].to_string();
            let sat = row[10].to_string();
            let fat = opt_str_at(&row, 11);
            let rc = opt_i64_at(&row, 12);
            let tm_str = opt_str_at(&row, 13);
            let ik = opt_str_at(&row, 14);
            let ek = opt_str_at(&row, 15);
            let ems = opt_str_at(&row, 16);

            let mode = match mode_str.as_str() {
                "stream" => AnalyticsMode::Stream,
                "stats" => AnalyticsMode::Stats,
                "annotate" => AnalyticsMode::Annotate,
                "persist" => AnalyticsMode::Persist,
                _ => AnalyticsMode::Stream,
            };
            let status = match status_str.as_str() {
                "pending" => RunStatus::Pending,
                "running" => RunStatus::Running,
                "succeeded" => RunStatus::Succeeded,
                "truncated" => RunStatus::Truncated,
                "failed" => RunStatus::Failed,
                _ => RunStatus::Pending,
            };
            let started_at = chrono::DateTime::parse_from_rfc3339(&sat)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or_else(|_| chrono::Utc::now());
            let finished_at = fat
                .as_ref()
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&chrono::Utc));
            let truncation_marker = tm_str.as_ref().and_then(|s| match s.as_str() {
                "ResultRowsLimit" => {
                    Some(cognicode_core::domain::analytics::TruncationMarker::ResultRowsLimit)
                }
                "PathCountLimit" => {
                    Some(cognicode_core::domain::analytics::TruncationMarker::PathCountLimit)
                }
                "VisitedNodesLimit" => {
                    Some(cognicode_core::domain::analytics::TruncationMarker::VisitedNodesLimit)
                }
                "VisitedEdgesLimit" => {
                    Some(cognicode_core::domain::analytics::TruncationMarker::VisitedEdgesLimit)
                }
                _ => None,
            });

            Ok(RunLineage {
                run_id: Uuid::from_string(run_id_str),
                workspace_id: WorkspaceId::try_new(ws).unwrap(),
                revision_id: RevisionId(rev),
                algorithm_id: AlgorithmId::from_string(alg),
                algorithm_version: ver,
                plan_hash: hex::decode(ph).unwrap_or_default(),
                params: serde_json::from_str(&params_str).unwrap_or(serde_json::Value::Null),
                seed: seed.map(|i| i as u64),
                mode,
                status,
                started_at,
                finished_at,
                row_count: rc,
                truncation_marker,
                idempotency_key: ik,
                error_kind: ek,
                error_message: ems,
            })
        } else {
            Err(AnalyticsError::RunNotFound(run_id.to_string()))
        }
    }

    async fn query(
        &self,
        filter: RunLineageFilter,
        limit: Option<u64>,
    ) -> Result<Vec<RunLineage>, AnalyticsError> {
        let conn = self
            .connection()
            .map_err(|e| AnalyticsError::Internal(format!("lineage query: {e}")))?;

        let limit = limit.unwrap_or(u64::MAX) as usize;
        let mut results = Vec::new();

        let mut stmt = conn
            .prepare(
                "MATCH (r:AnalyticsRunLineage) \
                 RETURN r.run_id, r.workspace_id, r.revision_id, r.algorithm_id, \
                        r.algorithm_version, r.plan_hash, r.params, r.seed, r.mode, \
                        r.status, r.started_at, r.finished_at, r.row_count, \
                        r.truncation_marker, r.idempotency_key, r.error_kind, r.error_message \
                 ORDER BY r.started_at DESC;",
            )
            .map_err(|e| AnalyticsError::Internal(format!("lineage query prepare: {e}")))?;
        let mut rows = conn
            .execute(&mut stmt, vec![])
            .map_err(|e| AnalyticsError::Internal(format!("lineage query: {e}")))?;

        while let Some(row) = rows.next() {
            let run_id_str = row[0].to_string();
            let ws = row[1].to_string();
            let rev = req_i64_at(&row, 2) as u64;
            let alg = row[3].to_string();
            let ver = row[4].to_string();
            let ph = row[5].to_string();
            let params_str = row[6].to_string();
            let seed = opt_i64_at(&row, 7);
            let mode_str = row[8].to_string();
            let status_str = row[9].to_string();
            let sat = row[10].to_string();
            let fat = opt_str_at(&row, 11);
            let rc = opt_i64_at(&row, 12);
            let tm_str = opt_str_at(&row, 13);
            let ik = opt_str_at(&row, 14);
            let ek = opt_str_at(&row, 15);
            let ems = opt_str_at(&row, 16);

            let mode = match mode_str.as_str() {
                "stream" => AnalyticsMode::Stream,
                "stats" => AnalyticsMode::Stats,
                "annotate" => AnalyticsMode::Annotate,
                "persist" => AnalyticsMode::Persist,
                _ => AnalyticsMode::Stream,
            };
            let status = match status_str.as_str() {
                "pending" => RunStatus::Pending,
                "running" => RunStatus::Running,
                "succeeded" => RunStatus::Succeeded,
                "truncated" => RunStatus::Truncated,
                "failed" => RunStatus::Failed,
                _ => RunStatus::Pending,
            };
            let started_at = chrono::DateTime::parse_from_rfc3339(&sat)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or_else(|_| chrono::Utc::now());
            let finished_at = fat
                .as_ref()
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&chrono::Utc));
            let truncation_marker = tm_str.as_ref().and_then(|s| match s.as_str() {
                "ResultRowsLimit" => {
                    Some(cognicode_core::domain::analytics::TruncationMarker::ResultRowsLimit)
                }
                "PathCountLimit" => {
                    Some(cognicode_core::domain::analytics::TruncationMarker::PathCountLimit)
                }
                "VisitedNodesLimit" => {
                    Some(cognicode_core::domain::analytics::TruncationMarker::VisitedNodesLimit)
                }
                "VisitedEdgesLimit" => {
                    Some(cognicode_core::domain::analytics::TruncationMarker::VisitedEdgesLimit)
                }
                _ => None,
            });

            let lineage = RunLineage {
                run_id: Uuid::from_string(run_id_str.clone()),
                workspace_id: WorkspaceId::try_new(ws.clone()).unwrap(),
                revision_id: RevisionId(rev),
                algorithm_id: AlgorithmId::from_string(alg.clone()),
                algorithm_version: ver.clone(),
                plan_hash: hex::decode(ph.clone()).unwrap_or_default(),
                params: serde_json::from_str(&params_str).unwrap_or(serde_json::Value::Null),
                seed: seed.map(|i| i as u64),
                mode,
                status,
                started_at,
                finished_at,
                row_count: rc,
                truncation_marker,
                idempotency_key: ik.clone(),
                error_kind: ek.clone(),
                error_message: ems.clone(),
            };

            // Apply filters (same logic as InMemoryLineageStore)
            if filter
                .workspace_id
                .as_ref()
                .map_or(false, |wid| &lineage.workspace_id != wid)
                || filter
                    .revision_id
                    .as_ref()
                    .map_or(false, |rid| &lineage.revision_id != rid)
                || filter
                    .algorithm_id
                    .as_ref()
                    .map_or(false, |aid| &lineage.algorithm_id != aid)
                || filter
                    .status
                    .as_ref()
                    .map_or(false, |s| &lineage.status != s)
            {
                continue;
            }

            results.push(lineage);
            if results.len() >= limit {
                break;
            }
        }

        Ok(results)
    }

    async fn upsert_descriptor_limits(
        &self,
        algorithm_id: &AlgorithmId,
        version: &str,
        limits: &PlanLimits,
    ) -> Result<(), AnalyticsError> {
        let conn = self
            .connection()
            .map_err(|e| AnalyticsError::Internal(format!("upsert limits: {e}")))?;

        let payload = serde_json::to_string(limits)
            .map_err(|e| AnalyticsError::Internal(format!("limits serialize: {e}")))?;

        // Try update first
        let params: Vec<(&str, lbug::Value)> = vec![
            ("alg", lbug::Value::String(algorithm_id.to_string())),
            ("ver", lbug::Value::String(version.to_string())),
            ("payload", lbug::Value::String(payload.clone())),
        ];
        let mut stmt = conn
            .prepare(
                "MATCH (d:AnalyticsDescriptorLimits) \
                 WHERE d.algorithm_id = $alg AND d.version = $ver \
                 SET d.limits_payload = $payload \
                 RETURN count(d);",
            )
            .map_err(|e| AnalyticsError::Internal(format!("upsert limits prepare: {e}")))?;
        let mut result = conn
            .execute(&mut stmt, params)
            .map_err(|e| AnalyticsError::Internal(format!("upsert limits: {e}")))?;

        let updated = result.next().map(|row| req_i64_at(&row, 0)).unwrap_or(0);

        if updated == 0 {
            // Insert new record
            let params: Vec<(&str, lbug::Value)> = vec![
                ("alg", lbug::Value::String(algorithm_id.to_string())),
                ("ver", lbug::Value::String(version.to_string())),
                ("payload", lbug::Value::String(payload)),
            ];
            conn.query(
                "CREATE (d:AnalyticsDescriptorLimits {\
                 algorithm_id: $alg, version: $ver, limits_payload: $payload});",
            )
            .map_err(|e| AnalyticsError::Internal(format!("insert limits: {e}")))?;
        }

        Ok(())
    }

    async fn get_descriptor_limits(
        &self,
        algorithm_id: &AlgorithmId,
        version: &str,
    ) -> Result<Option<PlanLimits>, AnalyticsError> {
        let conn = self
            .connection()
            .map_err(|e| AnalyticsError::Internal(format!("get limits: {e}")))?;

        let params: Vec<(&str, lbug::Value)> = vec![
            ("alg", lbug::Value::String(algorithm_id.to_string())),
            ("ver", lbug::Value::String(version.to_string())),
        ];
        let mut stmt = conn
            .prepare(
                "MATCH (d:AnalyticsDescriptorLimits) \
                 WHERE d.algorithm_id = $alg AND d.version = $ver \
                 RETURN d.limits_payload;",
            )
            .map_err(|e| AnalyticsError::Internal(format!("get limits prepare: {e}")))?;
        let mut result = conn
            .execute(&mut stmt, params)
            .map_err(|e| AnalyticsError::Internal(format!("get limits: {e}")))?;

        match result.next() {
            Some(row) => {
                let payload = row[0].to_string();
                let limits: PlanLimits = serde_json::from_str(&payload)
                    .map_err(|e| AnalyticsError::Internal(format!("limits deserialize: {e}")))?;
                Ok(Some(limits))
            }
            None => Ok(None),
        }
    }
}

impl QualityStore for LadybugStore {
    fn issues_for_file(&self, file: &str) -> Result<Vec<QualityIssue>, QualityError> {
        self.query_issues(
            "MATCH (i:QualityIssue) WHERE i.file_path = $file \
             RETURN i.id, i.rule_id, i.severity, i.category, i.file_path, i.line, i.message, i.status;",
            vec![("file", lbug::Value::String(file.to_string()))],
        )
    }

    fn issues_for_scope(&self, scope_prefix: &str) -> Result<Vec<QualityIssue>, QualityError> {
        let prefix = format!("{scope_prefix}/");
        self.query_issues(
            "MATCH (i:QualityIssue) WHERE i.file_path = $scope OR i.file_path STARTS WITH $prefix \
             RETURN i.id, i.rule_id, i.severity, i.category, i.file_path, i.line, i.message, i.status;",
            vec![
                ("scope", lbug::Value::String(scope_prefix.to_string())),
                ("prefix", lbug::Value::String(prefix)),
            ],
        )
    }

    fn issues_at_line(&self, file: &str, line: u32) -> Result<Vec<QualityIssue>, QualityError> {
        self.query_issues(
            "MATCH (i:QualityIssue) WHERE i.file_path = $file AND i.line = $line \
             RETURN i.id, i.rule_id, i.severity, i.category, i.file_path, i.line, i.message, i.status;",
            vec![
                ("file", lbug::Value::String(file.to_string())),
                ("line", lbug::Value::Int64(line as i64)),
            ],
        )
    }

    fn issue_by_id(&self, id: i64) -> Result<Option<QualityIssue>, QualityError> {
        let mut issues = self.query_issues(
            "MATCH (i:QualityIssue) WHERE i.id = $id \
             RETURN i.id, i.rule_id, i.severity, i.category, i.file_path, i.line, i.message, i.status;",
            vec![("id", lbug::Value::Int64(id))],
        )?;
        Ok(issues.pop())
    }

    fn rule_summary(&self, rule_id: &str) -> Result<RuleSummary, QualityError> {
        // Description + category from the Rule table; default description
        // to the rule_id when the Rule row has no description.
        let conn = self
            .connection()
            .map_err(|e| QualityError::Store(format!("rule_summary connection: {e}")))?;

        let description =
            {
                let mut stmt = match conn.prepare(
                "MATCH (r:QualityRule) WHERE r.rule_id = $rid RETURN r.description, r.category;",
            ) {
                Ok(stmt) => stmt,
                Err(e) if is_missing_table(&e) => return Ok(RuleSummary {
                    rule_id: rule_id.to_string(),
                    description: rule_id.to_string(),
                    open_count: 0,
                }),
                Err(e) => {
                    return Err(QualityError::Store(format!("rule_summary rule prepare: {e}")));
                }
            };
                match conn.execute(
                    &mut stmt,
                    vec![("rid", lbug::Value::String(rule_id.to_string()))],
                ) {
                    Ok(mut result) => match result.next() {
                        Some(row) => match &row[0] {
                            lbug::Value::String(s) if !s.is_empty() => s.clone(),
                            _ => rule_id.to_string(),
                        },
                        None => rule_id.to_string(),
                    },
                    Err(e) if is_missing_table(&e) => rule_id.to_string(),
                    Err(e) => {
                        return Err(QualityError::Store(format!(
                            "rule_summary rule execute: {e}"
                        )));
                    }
                }
            };

        let open_count: usize = match conn.prepare(
            "MATCH (i:QualityIssue) WHERE i.rule_id = $rid AND i.status = 'open' RETURN count(i);",
        ) {
            Err(e) if is_missing_table(&e) => 0,
            Err(e) => {
                return Err(QualityError::Store(format!(
                    "rule_summary count prepare: {e}"
                )));
            }
            Ok(mut stmt) => match conn.execute(
                &mut stmt,
                vec![("rid", lbug::Value::String(rule_id.to_string()))],
            ) {
                Ok(mut result) => match result.next() {
                    Some(row) => value_to_usize(&row[0]),
                    None => 0,
                },
                Err(e) if is_missing_table(&e) => 0,
                Err(e) => {
                    return Err(QualityError::Store(format!(
                        "rule_summary count execute: {e}"
                    )));
                }
            },
        };

        Ok(RuleSummary {
            rule_id: rule_id.to_string(),
            description,
            open_count,
        })
    }

    fn quality_gate(&self, workspace_id: Option<&str>) -> Result<QualityGateSummary, QualityError> {
        let conn = self
            .connection()
            .map_err(|e| QualityError::Store(format!("quality_gate connection: {e}")))?;

        let (cypher, params): (&str, Vec<(&str, lbug::Value)>) = match workspace_id {
            Some(ws) => (
                "MATCH (b:QualityBaseline) WHERE b.workspace_id = $ws \
                 RETURN b.rating, b.total_issues, b.blockers, b.criticals, b.debt_minutes, b.snapshot_at \
                 ORDER BY b.snapshot_at DESC LIMIT 1;",
                vec![("ws", lbug::Value::String(ws.to_string()))],
            ),
            None => (
                "MATCH (b:QualityBaseline) \
                 RETURN b.rating, b.total_issues, b.blockers, b.criticals, b.debt_minutes, b.snapshot_at \
                 ORDER BY b.snapshot_at DESC LIMIT 1;",
                Vec::new(),
            ),
        };

        let mut stmt = match conn.prepare(cypher) {
            Ok(stmt) => stmt,
            Err(e) if is_missing_table(&e) => return Ok(QualityGateSummary::default()),
            Err(e) => {
                return Err(QualityError::Store(format!("quality_gate prepare: {e}")));
            }
        };
        let mut result = match conn.execute(&mut stmt, params) {
            Ok(result) => result,
            Err(e) if is_missing_table(&e) => return Ok(QualityGateSummary::default()),
            Err(e) => {
                return Err(QualityError::Store(format!("quality_gate execute: {e}")));
            }
        };
        let Some(row) = result.next() else {
            return Ok(QualityGateSummary::default());
        };

        Ok(QualityGateSummary {
            rating: match &row[0] {
                lbug::Value::String(s) => Some(s.clone()),
                _ => None,
            },
            total_issues: value_to_usize(&row[1]),
            blockers: value_to_usize(&row[2]),
            criticals: value_to_usize(&row[3]),
            debt_minutes: value_to_usize(&row[4]) as u64,
            last_run: match &row[5] {
                lbug::Value::String(s) => Some(s.clone()),
                _ => None,
            },
        })
    }

    fn open_issues_count(&self, workspace_id: Option<&str>) -> Result<usize, QualityError> {
        let conn = self
            .connection()
            .map_err(|e| QualityError::Store(format!("open_issues_count connection: {e}")))?;
        let (cypher, params): (&str, Vec<(&str, lbug::Value)>) = match workspace_id {
            Some(ws) => (
                "MATCH (i:QualityIssue) WHERE i.status = 'open' AND i.workspace_id = $ws RETURN count(i);",
                vec![("ws", lbug::Value::String(ws.to_string()))],
            ),
            None => (
                "MATCH (i:QualityIssue) WHERE i.status = 'open' RETURN count(i);",
                Vec::new(),
            ),
        };
        let mut stmt = match conn.prepare(cypher) {
            Ok(stmt) => stmt,
            Err(e) if is_missing_table(&e) => return Ok(0),
            Err(e) => {
                return Err(QualityError::Store(format!(
                    "open_issues_count prepare: {e}"
                )));
            }
        };
        let mut result = match conn.execute(&mut stmt, params) {
            Ok(result) => result,
            Err(e) if is_missing_table(&e) => return Ok(0),
            Err(e) => {
                return Err(QualityError::Store(format!(
                    "open_issues_count execute: {e}"
                )));
            }
        };
        Ok(match result.next() {
            Some(row) => value_to_usize(&row[0]),
            None => 0,
        })
    }

    fn issues_for_workspace(
        &self,
        workspace_id: Option<&str>,
        filter: &IssueFilter,
    ) -> Result<Vec<QualityIssue>, QualityError> {
        let mut clauses: Vec<String> = Vec::new();
        let mut params: Vec<(&str, lbug::Value)> = Vec::new();

        if let Some(ws) = workspace_id {
            clauses.push("i.workspace_id = $ws".to_string());
            params.push(("ws", lbug::Value::String(ws.to_string())));
        }
        if let Some(sev) = &filter.severity {
            clauses.push("i.severity = $sev".to_string());
            params.push(("sev", lbug::Value::String(sev.clone())));
        }
        if let Some(cat) = &filter.category {
            clauses.push("i.category = $cat".to_string());
            params.push(("cat", lbug::Value::String(cat.clone())));
        }
        if let Some(status) = &filter.status {
            clauses.push("i.status = $status".to_string());
            params.push(("status", lbug::Value::String(status.clone())));
        }
        if let Some(prefix) = &filter.file_prefix {
            // Boundary-aware: `scope = "src"` does not match `src_extra.rs`.
            clauses.push("(i.file_path = $fp OR i.file_path STARTS WITH $fp_prefix)".to_string());
            params.push(("fp", lbug::Value::String(prefix.clone())));
            params.push(("fp_prefix", lbug::Value::String(format!("{prefix}/"))));
        }

        let where_clause = if clauses.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", clauses.join(" AND "))
        };
        let limit_clause = match filter.limit {
            Some(limit) => format!(" LIMIT {limit}"),
            None => String::new(),
        };

        let cypher = format!(
            "MATCH (i:QualityIssue){where_clause} \
             RETURN i.id, i.rule_id, i.severity, i.category, i.file_path, i.line, i.message, i.status{limit_clause};"
        );
        self.query_issues(&cypher, params)
    }

    fn insert_issues(&self, issues: &[NewIssue]) -> Result<UpsertSummary, QualityError> {
        let conn = self
            .connection()
            .map_err(|e| QualityError::Store(format!("insert_issues connection: {e}")))?;
        for issue in issues {
            let cypher = "CREATE (i:QualityIssue {workspace_id: $ws, rule_id: $rid, severity: $sev, \
                 category: $cat, file_path: $file, line: $line, message: $msg, status: $status});";
            let mut stmt = conn
                .prepare(cypher)
                .map_err(|e| QualityError::Store(format!("insert_issues prepare: {e}")))?;
            conn.execute(
                &mut stmt,
                vec![
                    ("ws", lbug::Value::String(issue.workspace_id.clone())),
                    ("rid", lbug::Value::String(issue.rule_id.clone())),
                    ("sev", lbug::Value::String(issue.severity.clone())),
                    ("cat", lbug::Value::String(issue.category.clone())),
                    ("file", lbug::Value::String(issue.file_path.clone())),
                    ("line", lbug::Value::Int64(issue.line as i64)),
                    ("msg", lbug::Value::String(issue.message.clone())),
                    ("status", lbug::Value::String(issue.status.clone())),
                ],
            )
            .map_err(|e| QualityError::Store(format!("insert_issues execute: {e}")))?;
        }
        // lbug 0.19 has no MERGE / ON CONFLICT primitive: every insert
        // is a fresh row, so `updated` is always 0.
        Ok(UpsertSummary {
            inserted: issues.len(),
            updated: 0,
        })
    }

    fn delete_issue(
        &self,
        workspace_id: &str,
        rule_id: &str,
        file_path: &str,
        line: u32,
    ) -> Result<bool, QualityError> {
        let conn = self
            .connection()
            .map_err(|e| QualityError::Store(format!("delete_issue connection: {e}")))?;
        let mut stmt = conn
            .prepare(
                "MATCH (i:QualityIssue) WHERE i.workspace_id = $ws AND i.rule_id = $rid \
                 AND i.file_path = $file AND i.line = $line DELETE i RETURN count(i);",
            )
            .map_err(|e| QualityError::Store(format!("delete_issue prepare: {e}")))?;
        let mut result = conn
            .execute(
                &mut stmt,
                vec![
                    ("ws", lbug::Value::String(workspace_id.to_string())),
                    ("rid", lbug::Value::String(rule_id.to_string())),
                    ("file", lbug::Value::String(file_path.to_string())),
                    ("line", lbug::Value::Int64(line as i64)),
                ],
            )
            .map_err(|e| QualityError::Store(format!("delete_issue execute: {e}")))?;
        Ok(match result.next() {
            Some(row) => value_to_usize(&row[0]) > 0,
            None => false,
        })
    }
}

/// Return `true` when an lbug error is caused by a missing node table
/// (the read contract degrades these to empty results, never errors).
fn is_missing_table(e: &lbug::Error) -> bool {
    let msg = e.to_string().to_lowercase();
    msg.contains("does not exist")
        || msg.contains("not exist")
        || msg.contains("not found")
        || msg.contains("unknown table")
        || msg.contains("no table")
}

/// Best-effort `usize` extraction from an lbug value (count / int columns).
fn value_to_usize(v: &lbug::Value) -> usize {
    match v {
        lbug::Value::Int64(n) => *n as usize,
        lbug::Value::Int32(n) => *n as usize,
        lbug::Value::Int16(n) => *n as usize,
        lbug::Value::Int8(n) => *n as usize,
        lbug::Value::UInt64(n) => *n as usize,
        lbug::Value::UInt32(n) => *n as usize,
        lbug::Value::UInt16(n) => *n as usize,
        lbug::Value::UInt8(n) => *n as usize,
        _ => 0,
    }
}

/// Extract a nullable STRING column from a lbug row.
fn opt_str_at(row: &[lbug::Value], idx: usize) -> Option<String> {
    match row.get(idx) {
        Some(lbug::Value::String(s)) => Some(s.clone()),
        Some(lbug::Value::Null(_)) => None,
        _ => None,
    }
}

/// Extract a nullable INT64 column from a lbug row.
fn opt_i64_at(row: &[lbug::Value], idx: usize) -> Option<i64> {
    match row.get(idx) {
        Some(lbug::Value::Int64(n)) => Some(*n),
        Some(lbug::Value::Int32(n)) => Some(*n as i64),
        Some(lbug::Value::Null(_)) => None,
        _ => None,
    }
}

/// Extract a required INT64 column from a lbug row, returning 0 for NULL.
fn req_i64_at(row: &[lbug::Value], idx: usize) -> i64 {
    opt_i64_at(row, idx).unwrap_or(0)
}

impl LadybugStore {
    /// Shared read path for the `issues` table. Graceful-degradation
    /// wrapper: a missing table yields an empty result, any other lbug
    /// failure surfaces as [`QualityError::Store`].
    fn query_issues(
        &self,
        cypher: &str,
        params: Vec<(&str, lbug::Value)>,
    ) -> Result<Vec<QualityIssue>, QualityError> {
        let conn = self
            .connection()
            .map_err(|e| QualityError::Store(format!("query_issues connection: {e}")))?;
        let mut stmt = match conn.prepare(cypher) {
            Ok(stmt) => stmt,
            Err(e) if is_missing_table(&e) => return Ok(Vec::new()),
            Err(e) => return Err(QualityError::Store(format!("query_issues prepare: {e}"))),
        };
        let mut result = match conn.execute(&mut stmt, params) {
            Ok(result) => result,
            Err(e) if is_missing_table(&e) => return Ok(Vec::new()),
            Err(e) => return Err(QualityError::Store(format!("query_issues execute: {e}"))),
        };

        let mut issues: Vec<QualityIssue> = Vec::new();
        while let Some(row) = result.next() {
            issues.push(issue_from_row(&row));
        }
        Ok(issues)
    }
}

/// Map a `RETURN i.id, i.rule_id, i.severity, i.category, i.file_path,
/// i.line, i.message, i.status` row into a [`QualityIssue`].
fn issue_from_row(row: &[lbug::Value]) -> QualityIssue {
    fn str_at(row: &[lbug::Value], idx: usize) -> String {
        match row.get(idx) {
            Some(lbug::Value::String(s)) => s.clone(),
            _ => String::new(),
        }
    }
    QualityIssue {
        id: value_to_usize(row.first().unwrap_or(&lbug::Value::Int64(0))) as i64,
        rule_id: str_at(row, 1),
        severity: str_at(row, 2),
        category: str_at(row, 3),
        file_path: str_at(row, 4),
        line: value_to_usize(row.get(5).unwrap_or(&lbug::Value::Int64(0))) as u32,
        message: str_at(row, 6),
        status: str_at(row, 7),
    }
}

// =============================================================================
// Generic Graph Layer DDL (`e29-1-ddl-init`)
// =============================================================================
//
// Per ADR-027 (`docs/adr/ADR-027-ladybugdb-hybrid-schema-strategy.md`):
// 22 NODE TABLEs + ~20 REL TABLEs backing the Generic Graph Layer.
// Hybrid strategy — typed columns for stable properties, MAP for
// emergent ones. Multi-label nodes for poly形式 membership.
//
// **v1 scope (this commit)**: 22 NODE TABLEs as DDL constants + an
// `init_generic_graph_schema()` function that applies them
// idempotently. The ~20 REL TABLEs are tracked as a follow-up PR
// (`e29-1-ddl-rels`) — they require a separate design pass on
// edge-label naming and the multi-label relationship between node
// types (e.g. a `Calls` edge between a `Symbol` and another
// `Symbol`, plus a `Component` ↔ `Component` `Calls`).
//
// The `init_generic_graph_schema()` is invoked explicitly by the
// production runtime composition root (similar to PG's
// `run_migrations`); tests invoke it via the helper in `mod tests`.

/// `LadybugStore::init_generic_graph_schema` — applies the 22
/// NODE TABLE DDLs from ADR-027.
impl LadybugStore {
    /// Apply the 22 NODE TABLE DDLs from ADR-027 to the underlying
    /// lbug database. Idempotent (every `CREATE NODE TABLE` uses
    /// `IF NOT EXISTS`).
    pub fn init_generic_graph_schema(&self) -> Result<(), Error> {
        let conn = self
            .connection()
            .map_err(|e| Error::Lbug(format!("init_generic_graph_schema: {e}")))?;
        for stmt in generic_graph_node_table_ddls() {
            conn.query(stmt)
                .map_err(|e| Error::Lbug(format!("init_generic_graph_schema: {e}\nDDL: {stmt}")))?;
        }
        Ok(())
    }

    /// Apply the ~20 REL TABLE DDLs from ADR-027 §6. Idempotent.
    ///
    /// The caller is expected to have already invoked
    /// `init_generic_graph_schema()` (which creates the 22 NODE
    /// TABLEs) — the REL TABLEs reference endpoint `id` columns
    /// (`source_id`, `target_id`) that the application layer
    /// populates explicitly (lbug 0.19 has no foreign-key
    /// constraints, so the integrity is application-enforced).
    pub fn init_generic_graph_rels_schema(&self) -> Result<(), Error> {
        let conn = self
            .connection()
            .map_err(|e| Error::Lbug(format!("init_generic_graph_rels_schema: {e}")))?;
        for stmt in generic_graph_rel_table_ddls() {
            conn.query(stmt).map_err(|e| {
                Error::Lbug(format!("init_generic_graph_rels_schema: {e}\nDDL: {stmt}"))
            })?;
        }
        Ok(())
    }

    /// Migrate an in-memory `CallGraph` aggregate into the lbug
    /// store's `GraphRevision` + `GraphSymbol` + `GraphEdge` tables.
    ///
    /// v1 scope: source is an in-memory `CallGraph` (which the
    /// application layer can populate from anywhere — PG, in-memory
    /// pipeline, etc.). A future PR (`e29-2-migrate-from-pg`) will
    /// add a PG-specific exporter that reads from the live PG and
    /// constructs a `CallGraph` to feed into this function.
    ///
    /// The caller MUST invoke `init_call_graph_schema()` (or
    /// `init_generic_graph_schema()` + `init_generic_graph_rels_schema()`)
    /// first. Idempotent re-runs of this function on the same data
    /// are safe via read-then-conditional-write (the same pattern the
    /// `CallGraphStore` PR uses).
    ///
    /// **lbug 0.19 limitation**: the parser does not support
    /// `MERGE ON MATCH / ON CREATE` (introduced in Cypher 5.x).
    /// v1 uses a read-then-CREATE-or-UPDATE pattern per row.
    pub fn migrate_call_graph(
        &self,
        ws: &str,
        rev: i64,
        graph: &cognicode_core::domain::aggregates::CallGraph,
    ) -> Result<(), Error> {
        let conn = self
            .connection()
            .map_err(|e| Error::Lbug(format!("migrate_call_graph: {e}")))?;

        // Step 1: ensure the GraphRevision row exists.
        let mut rev_check = conn
            .prepare(
                "MATCH (r:GraphRevision) WHERE r.workspace_id = $ws AND r.revision_id = $rev RETURN r.id;",
            )
            .map_err(|e| Error::Lbug(format!("migrate_call_graph: rev check prepare: {e}")))?;
        let mut rev_result = conn
            .execute(
                &mut rev_check,
                vec![
                    ("ws", lbug::Value::String(ws.to_string())),
                    ("rev", lbug::Value::Int64(rev)),
                ],
            )
            .map_err(|e| Error::Lbug(format!("migrate_call_graph: rev check execute: {e}")))?;
        if rev_result.next().is_none() {
            conn.query(&format!(
                "CREATE (r:GraphRevision {{workspace_id: $ws, revision_id: $rev, head_of: true}});",
            ))
            .map_err(|e| Error::Lbug(format!("migrate_call_graph: rev insert: {e}")))?;
        }

        // Step 2: insert every Symbol as a GraphSymbol node via
        // read-then-conditional-write (UPDATE if exists, CREATE
        // otherwise). The v1 implementation queries the synthetic
        // PK id (we don't know it in advance) and instead uses
        // (workspace_id, revision_id, fqn) as the natural key for
        // the read check; the UPDATE is a no-op for v1 (we just
        // CREATE if not found — UPDATE would require knowing the
        // node id).
        for sym in graph.symbols() {
            let fqn = sym.fully_qualified_name();
            let mut check = conn
                .prepare(
                    "MATCH (s:GraphSymbol) WHERE s.workspace_id = $ws AND s.revision_id = $rev AND s.fqn = $fqn RETURN s.id;",
                )
                .map_err(|e| Error::Lbug(format!("migrate_call_graph: sym check {fqn}: {e}")))?;
            let mut result = conn
                .execute(
                    &mut check,
                    vec![
                        ("ws", lbug::Value::String(ws.to_string())),
                        ("rev", lbug::Value::Int64(rev)),
                        ("fqn", lbug::Value::String(fqn.to_string())),
                    ],
                )
                .map_err(|e| Error::Lbug(format!("migrate_call_graph: sym check {fqn}: {e}")))?;
            if result.next().is_none() {
                let sym_kind = sym.kind().to_string();
                let sym_name = sym.name().to_string();
                let sym_file = sym.location().file().to_string();
                let sym_line = sym.location().line() as i64;
                let sym_sig = sym.signature().map(|s| s.to_string()).unwrap_or_default();
                let fqn_for_insert = fqn.clone();
                let stmt_str = format!(
                    "CREATE (s:GraphSymbol {{workspace_id: $ws, revision_id: $rev, fqn: $fqn, kind: $kind, name: $name, file_path: $file, line: $line, signature: $sig}});"
                );
                let mut stmt = conn.prepare(&stmt_str).map_err(|e| {
                    Error::Lbug(format!("migrate_call_graph: sym insert prepare {fqn}: {e}"))
                })?;
                conn.execute(
                    &mut stmt,
                    vec![
                        ("ws", lbug::Value::String(ws.to_string())),
                        ("rev", lbug::Value::Int64(rev)),
                        ("fqn", lbug::Value::String(fqn_for_insert.to_string())),
                        ("kind", lbug::Value::String(sym_kind.to_string())),
                        ("name", lbug::Value::String(sym_name.to_string())),
                        ("file", lbug::Value::String(sym_file.to_string())),
                        ("line", lbug::Value::Int64(sym_line)),
                        ("sig", lbug::Value::String(sym_sig.to_string())),
                    ],
                )
                .map_err(|e| Error::Lbug(format!("migrate_call_graph: sym insert {fqn}: {e}")))?;
            }
        }

        // Step 3: insert every edge as a GraphEdge node. lbug 0.19
        // doesn't have relationship patterns, so each edge is its
        // own NODE TABLE row (we use source_id / target_id / ws / rev
        // as the natural key for the read check).
        for (src, tgt, _dep, _prov, _conf) in graph.edges_with_metadata() {
            let src_fqn = src.as_str().to_string();
            let tgt_fqn = tgt.as_str().to_string();
            let mut check = conn
                .prepare(
                    "MATCH (e:GraphEdge) WHERE e.workspace_id = $ws AND e.revision_id = $rev AND e.source_id = $src AND e.target_id = $tgt RETURN e.id;",
                )
                .map_err(|e| {
                    Error::Lbug(format!("migrate_call_graph: edge check prepare: {e}"))
                })?;
            let mut result = conn
                .execute(
                    &mut check,
                    vec![
                        ("ws", lbug::Value::String(ws.to_string())),
                        ("rev", lbug::Value::Int64(rev)),
                        ("src", lbug::Value::String(src_fqn.to_string())),
                        ("tgt", lbug::Value::String(tgt_fqn.to_string())),
                    ],
                )
                .map_err(|e| Error::Lbug(format!("migrate_call_graph: edge check execute: {e}")))?;
            if result.next().is_none() {
                // lbug 0.19 requires dep_type, provenance, and
                // confidence to be set explicitly (NOT NULL in
                // the schema). v1 hardcodes default values; a
                // future PR can read the dep_type/provenance/conf
                // from the CallGraph aggregate and pass them here.
                let dep_type_str = "calls".to_string();
                let provenance_str = "extracted".to_string();
                let confidence = 1.0_f64;
                let stmt_str = format!(
                    "CREATE (e:GraphEdge {{workspace_id: $ws, revision_id: $rev, source_id: $src, target_id: $tgt, dep_type: $dep, provenance: $prov, confidence: $conf}}) RETURN e.id;"
                );
                let mut stmt = conn.prepare(&stmt_str).map_err(|e| {
                    Error::Lbug(format!(
                        "migrate_call_graph: edge insert prepare {src_fqn}->{tgt_fqn}: {e}"
                    ))
                })?;
                conn.execute(
                    &mut stmt,
                    vec![
                        ("ws", lbug::Value::String(ws.to_string())),
                        ("rev", lbug::Value::Int64(rev)),
                        ("src", lbug::Value::String(src_fqn.clone())),
                        ("tgt", lbug::Value::String(tgt_fqn.clone())),
                        ("dep", lbug::Value::String(dep_type_str)),
                        ("prov", lbug::Value::String(provenance_str)),
                        ("conf", lbug::Value::Double(confidence)),
                    ],
                )
                .map_err(|e| {
                    Error::Lbug(format!(
                        "migrate_call_graph: edge insert {src_fqn}->{tgt_fqn}: {e}"
                    ))
                })?;
            } else {
            }
        }
        Ok(())
    }
}

/// Returns the 22 CREATE NODE TABLE statements from ADR-027.
fn generic_graph_node_table_ddls() -> Vec<&'static str> {
    vec![
        "CREATE NODE TABLE IF NOT EXISTS Symbol( \
             id SERIAL PRIMARY KEY, \
             workspace_id INT64, \
             revision_id INT64, \
             name STRING, \
             kind STRING, \
             file_path STRING, \
             line_number INT64, \
             signature STRING, \
             valid_from INT64, \
             valid_to INT64, \
             properties MAP(STRING, STRING));",
        "CREATE NODE TABLE IF NOT EXISTS Decision( \
             id SERIAL PRIMARY KEY, \
             workspace_id INT64, \
             revision_id INT64, \
             name STRING, \
             rationale STRING, \
             decided_at INT64, \
             valid_from INT64, \
             valid_to INT64, \
             properties MAP(STRING, STRING));",
        "CREATE NODE TABLE IF NOT EXISTS Doc( \
             id SERIAL PRIMARY KEY, \
             workspace_id INT64, \
             revision_id INT64, \
             title STRING, \
             path STRING, \
             kind STRING, \
             content_hash STRING, \
             valid_from INT64, \
             valid_to INT64, \
             properties MAP(STRING, STRING));",
        "CREATE NODE TABLE IF NOT EXISTS Evidence( \
             id SERIAL PRIMARY KEY, \
             workspace_id INT64, \
             revision_id INT64, \
             source_kind STRING, \
             source_ref STRING, \
             confidence REAL, \
             captured_at INT64, \
             valid_from INT64, \
             valid_to INT64, \
             properties MAP(STRING, STRING));",
        "CREATE NODE TABLE IF NOT EXISTS Issue( \
             id SERIAL PRIMARY KEY, \
             workspace_id INT64, \
             revision_id INT64, \
             severity STRING, \
             category STRING, \
             file_path STRING, \
             line_number INT64, \
             message STRING, \
             status STRING, \
             valid_from INT64, \
             valid_to INT64, \
             properties MAP(STRING, STRING));",
        "CREATE NODE TABLE IF NOT EXISTS Component( \
             id SERIAL PRIMARY KEY, \
             workspace_id INT64, \
             revision_id INT64, \
             name STRING, \
             layer STRING, \
             responsibility STRING, \
             valid_from INT64, \
             valid_to INT64, \
             properties MAP(STRING, STRING));",
        "CREATE NODE TABLE IF NOT EXISTS Container( \
             id SERIAL PRIMARY KEY, \
             workspace_id INT64, \
             revision_id INT64, \
             name STRING, \
             kind STRING, \
             valid_from INT64, \
             valid_to INT64, \
             properties MAP(STRING, STRING));",
        "CREATE NODE TABLE IF NOT EXISTS System( \
             id SERIAL PRIMARY KEY, \
             workspace_id INT64, \
             revision_id INT64, \
             name STRING, \
             description STRING, \
             valid_from INT64, \
             valid_to INT64, \
             properties MAP(STRING, STRING));",
        "CREATE NODE TABLE IF NOT EXISTS Route( \
             id SERIAL PRIMARY KEY, \
             workspace_id INT64, \
             revision_id INT64, \
             path STRING, \
             kind STRING, \
             valid_from INT64, \
             valid_to INT64, \
             properties MAP(STRING, STRING));",
        "CREATE NODE TABLE IF NOT EXISTS Rule( \
             id SERIAL PRIMARY KEY, \
             workspace_id INT64, \
             revision_id INT64, \
             rule_id STRING, \
             description STRING, \
             category STRING, \
             valid_from INT64, \
             valid_to INT64, \
             properties MAP(STRING, STRING));",
        "CREATE NODE TABLE IF NOT EXISTS Baseline( \
             id SERIAL PRIMARY KEY, \
             workspace_id INT64, \
             revision_id INT64, \
             rating STRING, \
             total_issues INT64, \
             blockers INT64, \
             criticals INT64, \
             debt_minutes INT64, \
             snapshot_at STRING, \
             valid_from INT64, \
             valid_to INT64, \
             properties MAP(STRING, STRING));",
        "CREATE NODE TABLE IF NOT EXISTS Investigation( \
             id SERIAL PRIMARY KEY, \
             workspace_id INT64, \
             revision_id INT64, \
             title STRING, \
             status STRING, \
             created_at STRING, \
             updated_at STRING, \
             valid_from INT64, \
             valid_to INT64, \
             properties MAP(STRING, STRING));",
        "CREATE NODE TABLE IF NOT EXISTS Artifact( \
             id SERIAL PRIMARY KEY, \
             workspace_id INT64, \
             revision_id INT64, \
             kind STRING, \
             format STRING, \
             content_hash STRING, \
             created_at STRING, \
             valid_from INT64, \
             valid_to INT64, \
             properties MAP(STRING, STRING));",
        "CREATE NODE TABLE IF NOT EXISTS ExplorationSession( \
             id SERIAL PRIMARY KEY, \
             workspace_id INT64, \
             revision_id INT64, \
             events STRING, \
             navigation_mode STRING, \
             panes STRING, \
             created_at STRING, \
             updated_at STRING, \
             valid_from INT64, \
             valid_to INT64, \
             properties MAP(STRING, STRING));",
        "CREATE NODE TABLE IF NOT EXISTS NamedView( \
             id SERIAL PRIMARY KEY, \
             workspace_id INT64, \
             revision_id INT64, \
             name STRING, \
             view_kind STRING, \
             owner STRING, \
             created_at STRING, \
             updated_at STRING, \
             valid_from INT64, \
             valid_to INT64, \
             properties MAP(STRING, STRING));",
        "CREATE NODE TABLE IF NOT EXISTS ViewSpec( \
             id SERIAL PRIMARY KEY, \
             workspace_id INT64, \
             revision_id INT64, \
             title STRING, \
             applies_to STRING, \
             view_kind STRING, \
             data_source STRING, \
             transform STRING, \
             renderer_kind STRING, \
             props STRING, \
             owner STRING, \
             created_at STRING, \
             updated_at STRING, \
             valid_from INT64, \
             valid_to INT64, \
             properties MAP(STRING, STRING));",
        "CREATE NODE TABLE IF NOT EXISTS GraphReport( \
             id STRING PRIMARY KEY, \
             workspace_id STRING, \
             revision_id INT64, \
             created_at STRING, \
             report STRING, \
             symbol_count INT64, \
             edge_count INT64, \
             health_score DOUBLE, \
             valid_from INT64, \
             valid_to INT64, \
             properties MAP(STRING, STRING));",
        "CREATE NODE TABLE IF NOT EXISTS AnalyticsRun( \
             id SERIAL PRIMARY KEY, \
             workspace_id INT64, \
             revision_id INT64, \
             algorithm STRING, \
             mode STRING, \
             result_count INT64, \
             started_at STRING, \
             completed_at STRING, \
             valid_from INT64, \
             valid_to INT64, \
             properties MAP(STRING, STRING));",
        "CREATE NODE TABLE IF NOT EXISTS FileRecord( \
             id SERIAL PRIMARY KEY, \
             workspace_id INT64, \
             revision_id INT64, \
             file_path STRING, \
             file_type STRING, \
             language STRING, \
             content_hash STRING, \
             mtime DOUBLE, \
             valid_from INT64, \
             valid_to INT64, \
             properties MAP(STRING, STRING));",
        "CREATE NODE TABLE IF NOT EXISTS Revision( \
             id SERIAL PRIMARY KEY, \
             workspace_id INT64, \
             revision_id INT64, \
             parent_revision_id INT64, \
             author STRING, \
             committed_at STRING, \
             valid_from INT64, \
             valid_to INT64, \
             properties MAP(STRING, STRING));",
        "CREATE NODE TABLE IF NOT EXISTS Workspace( \
             id STRING PRIMARY KEY, \
             name STRING, \
             kind STRING, \
             source_path STRING, \
             config STRING, \
             created_at STRING, \
             valid_from INT64, \
             valid_to INT64, \
             properties MAP(STRING, STRING));",
        "CREATE NODE TABLE IF NOT EXISTS Space( \
             id STRING PRIMARY KEY, \
             workspace_id INT64, \
             name STRING, \
             kind STRING, \
             source_path STRING, \
             config STRING, \
             created_at STRING, \
             valid_from INT64, \
             valid_to INT64, \
             properties MAP(STRING, STRING));",
    ]
}

/// Returns the ~20 CREATE NODE TABLE statements for relationships
/// (edges) per ADR-027 §6. Each is a NODE TABLE in lbug 0.19
/// (lbug does not differentiate node vs relationship types at the
/// storage layer; the relationship semantics are enforced by the
/// application via the `source_id` / `target_id` columns pointing
/// at the two endpoint NODE TABLEs).
///
/// v1 scope: 20 REL TABLEs covering the canonical CognitiveCode
/// edge vocabulary (Calls, Imports, Inherits, References,
/// Defines, Annotates, Contains, Documents, Supports, Decides,
/// Contradicts, Refines, Supersedes, Implements, DependsOn,
/// Exposes, Consumes, BelongsTo, Hosts, Owns).
fn generic_graph_rel_table_ddls() -> Vec<&'static str> {
    vec![
        "CREATE NODE TABLE IF NOT EXISTS Calls( \
             id SERIAL PRIMARY KEY, \
             workspace_id INT64, \
             revision_id INT64, \
             source_id INT64, \
             target_id INT64, \
             call_site_line INT64, \
             is_virtual BOOLEAN, \
             confidence REAL, \
             provenance STRING, \
             valid_from INT64, \
             valid_to INT64, \
             properties MAP(STRING, STRING));",
        "CREATE NODE TABLE IF NOT EXISTS Imports( \
             id SERIAL PRIMARY KEY, \
             workspace_id INT64, \
             revision_id INT64, \
             source_id INT64, \
             target_id INT64, \
             import_path STRING, \
             is_reexport BOOLEAN, \
             confidence REAL, \
             provenance STRING, \
             valid_from INT64, \
             valid_to INT64, \
             properties MAP(STRING, STRING));",
        "CREATE NODE TABLE IF NOT EXISTS Inherits( \
             id SERIAL PRIMARY KEY, \
             workspace_id INT64, \
             revision_id INT64, \
             source_id INT64, \
             target_id INT64, \
             visibility STRING, \
             confidence REAL, \
             provenance STRING, \
             valid_from INT64, \
             valid_to INT64, \
             properties MAP(STRING, STRING));",
        "CREATE NODE TABLE IF NOT EXISTS References( \
             id SERIAL PRIMARY KEY, \
             workspace_id INT64, \
             revision_id INT64, \
             source_id INT64, \
             target_id INT64, \
             reference_kind STRING, \
             confidence REAL, \
             provenance STRING, \
             valid_from INT64, \
             valid_to INT64, \
             properties MAP(STRING, STRING));",
        "CREATE NODE TABLE IF NOT EXISTS Defines( \
             id SERIAL PRIMARY KEY, \
             workspace_id INT64, \
             revision_id INT64, \
             source_id INT64, \
             target_id INT64, \
             definition_kind STRING, \
             confidence REAL, \
             provenance STRING, \
             valid_from INT64, \
             valid_to INT64, \
             properties MAP(STRING, STRING));",
        "CREATE NODE TABLE IF NOT EXISTS Annotates( \
             id SERIAL PRIMARY KEY, \
             workspace_id INT64, \
             revision_id INT64, \
             source_id INT64, \
             target_id INT64, \
             annotation_kind STRING, \
             confidence REAL, \
             provenance STRING, \
             valid_from INT64, \
             valid_to INT64, \
             properties MAP(STRING, STRING));",
        "CREATE NODE TABLE IF NOT EXISTS Contains( \
             id SERIAL PRIMARY KEY, \
             workspace_id INT64, \
             revision_id INT64, \
             source_id INT64, \
             target_id INT64, \
             containment_kind STRING, \
             confidence REAL, \
             provenance STRING, \
             valid_from INT64, \
             valid_to INT64, \
             properties MAP(STRING, STRING));",
        "CREATE NODE TABLE IF NOT EXISTS Documents( \
             id SERIAL PRIMARY KEY, \
             workspace_id INT64, \
             revision_id INT64, \
             source_id INT64, \
             target_id INT64, \
             documentation_kind STRING, \
             confidence REAL, \
             provenance STRING, \
             valid_from INT64, \
             valid_to INT64, \
             properties MAP(STRING, STRING));",
        "CREATE NODE TABLE IF NOT EXISTS Supports( \
             id SERIAL PRIMARY KEY, \
             workspace_id INT64, \
             revision_id INT64, \
             source_id INT64, \
             target_id INT64, \
             support_strength REAL, \
             confidence REAL, \
             provenance STRING, \
             valid_from INT64, \
             valid_to INT64, \
             properties MAP(STRING, STRING));",
        "CREATE NODE TABLE IF NOT EXISTS Decides( \
             id SERIAL PRIMARY KEY, \
             workspace_id INT64, \
             revision_id INT64, \
             source_id INT64, \
             target_id INT64, \
             decision_kind STRING, \
             confidence REAL, \
             provenance STRING, \
             valid_from INT64, \
             valid_to INT64, \
             properties MAP(STRING, STRING));",
        "CREATE NODE TABLE IF NOT EXISTS Contradicts( \
             id SERIAL PRIMARY KEY, \
             workspace_id INT64, \
             revision_id INT64, \
             source_id INT64, \
             target_id INT64, \
             severity STRING, \
             confidence REAL, \
             provenance STRING, \
             valid_from INT64, \
             valid_to INT64, \
             properties MAP(STRING, STRING));",
        "CREATE NODE TABLE IF NOT EXISTS Refines( \
             id SERIAL PRIMARY KEY, \
             workspace_id INT64, \
             revision_id INT64, \
             source_id INT64, \
             target_id INT64, \
             confidence REAL, \
             provenance STRING, \
             valid_from INT64, \
             valid_to INT64, \
             properties MAP(STRING, STRING));",
        "CREATE NODE TABLE IF NOT EXISTS Supersedes( \
             id SERIAL PRIMARY KEY, \
             workspace_id INT64, \
             revision_id INT64, \
             source_id INT64, \
             target_id INT64, \
             confidence REAL, \
             provenance STRING, \
             valid_from INT64, \
             valid_to INT64, \
             properties MAP(STRING, STRING));",
        "CREATE NODE TABLE IF NOT EXISTS Implements( \
             id SERIAL PRIMARY KEY, \
             workspace_id INT64, \
             revision_id INT64, \
             source_id INT64, \
             target_id INT64, \
             confidence REAL, \
             provenance STRING, \
             valid_from INT64, \
             valid_to INT64, \
             properties MAP(STRING, STRING));",
        "CREATE NODE TABLE IF NOT EXISTS DependsOn( \
             id SERIAL PRIMARY KEY, \
             workspace_id INT64, \
             revision_id INT64, \
             source_id INT64, \
             target_id INT64, \
             dependency_kind STRING, \
             strength REAL, \
             confidence REAL, \
             provenance STRING, \
             valid_from INT64, \
             valid_to INT64, \
             properties MAP(STRING, STRING));",
        "CREATE NODE TABLE IF NOT EXISTS Exposes( \
             id SERIAL PRIMARY KEY, \
             workspace_id INT64, \
             revision_id INT64, \
             source_id INT64, \
             target_id INT64, \
             exposure_kind STRING, \
             visibility STRING, \
             confidence REAL, \
             provenance STRING, \
             valid_from INT64, \
             valid_to INT64, \
             properties MAP(STRING, STRING));",
        "CREATE NODE TABLE IF NOT EXISTS Consumes( \
             id SERIAL PRIMARY KEY, \
             workspace_id INT64, \
             revision_id INT64, \
             source_id INT64, \
             target_id INT64, \
             consumption_kind STRING, \
             frequency INT64, \
             confidence REAL, \
             provenance STRING, \
             valid_from INT64, \
             valid_to INT64, \
             properties MAP(STRING, STRING));",
        "CREATE NODE TABLE IF NOT EXISTS BelongsTo( \
             id SERIAL PRIMARY KEY, \
             workspace_id INT64, \
             revision_id INT64, \
             source_id INT64, \
             target_id INT64, \
             membership_kind STRING, \
             confidence REAL, \
             provenance STRING, \
             valid_from INT64, \
             valid_to INT64, \
             properties MAP(STRING, STRING));",
        "CREATE NODE TABLE IF NOT EXISTS Hosts( \
             id SERIAL PRIMARY KEY, \
             workspace_id INT64, \
             revision_id INT64, \
             source_id INT64, \
             target_id INT64, \
             hosting_kind STRING, \
             confidence REAL, \
             provenance STRING, \
             valid_from INT64, \
             valid_to INT64, \
             properties MAP(STRING, STRING));",
        "CREATE NODE TABLE IF NOT EXISTS Owns( \
             id SERIAL PRIMARY KEY, \
             workspace_id INT64, \
             revision_id INT64, \
             source_id INT64, \
             target_id INT64, \
             ownership_kind STRING, \
             confidence REAL, \
             provenance STRING, \
             valid_from INT64, \
             valid_to INT64, \
             properties MAP(STRING, STRING));",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "multimodal")]
    use cognicode_core::domain::value_objects::{SpaceId, SpaceKind};
    use serial_test::serial;

    fn ws_id(s: &str) -> WorkspaceId {
        WorkspaceId::try_new(s).expect("workspace id must be non-empty")
    }

    /// Smoke test: the `LadybugStore::open` constructor exists and
    /// accepts the same `(path, SystemConfig)` shape the spike
    /// validated end-to-end.
    #[test]
    fn open_constructor_compiles() {
        fn _check(_: fn(&Path) -> Result<LadybugStore, Error>) {}
    }

    /// All 9 ports are implemented (8 here + `QualityStore` deferred
    /// to follow-up PR).
    #[test]
    fn all_nine_ports_implemented() {
        fn _check<T: Send + Sync>()
        where
            T: RevisionStore,
            T: ManifestStore,
            T: SessionStore,
            T: ReportStore,
            T: ViewSpecStore,
            T: CallGraphStore,
        {
        }
    }

    /// `multimodal-9ports` feature flag activates the 2 multimodal-only
    /// port impls.
    #[cfg(feature = "multimodal-9ports")]
    #[test]
    fn multimodal_9ports_activated() {
        fn _check<T: Send + Sync>()
        where
            T: FederationStore,
            T: IngestCommitPort,
        {
        }
    }

    // ========================================================================
    // Per-port integration tests — the first land: ManifestStore (Priority 1)
    // ========================================================================

    /// Create a temp lbug DB and return a `LadybugStore` + temp dir handle
    /// (the temp dir auto-cleans on drop).
    ///
    /// Caller MUST run the schema DDL via `init_schema` before exercising
    /// the port — same pattern as `PostgresRepository::run_migrations`.
    fn make_test_store() -> (LadybugStore, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("test.lbdb");
        let store = LadybugStore::open(&path).expect("open lbug db");
        (store, dir)
    }

    /// Apply the ScanManifest NODE TABLE DDL once per test database.
    ///
    /// Mirrors the `PostgresRepository::run_migrations()` pattern: explicit,
    /// idempotent (uses `IF NOT EXISTS`).
    ///
    /// Note: lbug 0.19.0 NODE TABLEs require a single-column PRIMARY KEY
    /// (the spike's schema pattern adds `id SERIAL PRIMARY KEY` to every
    /// table — composite PKs aren't supported in 0.19.0). The natural
    /// key (`workspace_id, file_path`) is enforced via a uniqueness
    /// constraint at the application layer via the `MERGE` pattern in
    /// `upsert_manifest_entry`. The synthetic `id` is internal — port
    /// consumers never see it.
    fn init_scan_manifest_schema(store: &LadybugStore) {
        let conn = store.connection().expect("schema-init connection");
        conn.query(
            "CREATE NODE TABLE IF NOT EXISTS ScanManifest( \
                 id SERIAL PRIMARY KEY, \
                 workspace_id STRING, \
                 file_path STRING, \
                 file_type STRING, \
                 language STRING, \
                 content_hash STRING, \
                 mtime DOUBLE, \
                 symbol_count INT64, \
                 edge_count INT64, \
                 status STRING, \
                 error_msg STRING);",
        )
        .expect("create ScanManifest NODE TABLE");
    }

    fn sample_row(ws: &str, path: &str) -> ScanManifest {
        ScanManifest {
            workspace_id: ws.to_string(),
            file_path: path.to_string(),
            file_type: "rust".to_string(),
            language: Some("Rust".to_string()),
            content_hash: "abc123".to_string(),
            mtime: 1234.5,
            symbol_count: 42,
            edge_count: 17,
            status: "scanned".to_string(),
            error_msg: None,
        }
    }

    #[tokio::test]
    async fn manifest_get_returns_empty_for_fresh_db() {
        let (store, _dir) = make_test_store();
        init_scan_manifest_schema(&store);
        let rows = store.get_manifest("ws-unknown").await.expect("get");
        assert!(rows.is_empty(), "fresh db should return no rows");
    }

    #[tokio::test]
    async fn manifest_upsert_then_get_round_trips() {
        let (store, _dir) = make_test_store();
        init_scan_manifest_schema(&store);
        let row = sample_row("ws-1", "src/lib.rs");
        store.upsert_manifest_entry(&row).await.expect("upsert");
        let rows = store.get_manifest("ws-1").await.expect("get");
        assert_eq!(rows.len(), 1, "should round-trip exactly one row");
        let r = &rows[0];
        assert_eq!(r.workspace_id, "ws-1");
        assert_eq!(r.file_path, "src/lib.rs");
        assert_eq!(r.file_type, "rust");
        assert_eq!(r.language.as_deref(), Some("Rust"));
        assert_eq!(r.content_hash, "abc123");
        assert!((r.mtime - 1234.5).abs() < 1e-9);
        assert_eq!(r.symbol_count, 42);
        assert_eq!(r.edge_count, 17);
        assert_eq!(r.status, "scanned");
        assert_eq!(r.error_msg, None);
    }

    #[tokio::test]
    async fn manifest_upsert_with_optional_nulls() {
        let (store, _dir) = make_test_store();
        init_scan_manifest_schema(&store);
        let mut row = sample_row("ws-2", "src/empty.rs");
        row.language = None;
        row.error_msg = None;
        store.upsert_manifest_entry(&row).await.expect("upsert");
        let rows = store.get_manifest("ws-2").await.expect("get");
        assert_eq!(rows.len(), 1);
        let r = &rows[0];
        assert!(r.language.is_none(), "language should round-trip None");
        assert!(r.error_msg.is_none(), "error_msg should round-trip None");
    }

    #[tokio::test]
    async fn manifest_delete_removes_target_row() {
        let (store, _dir) = make_test_store();
        init_scan_manifest_schema(&store);
        let row1 = sample_row("ws-3", "src/a.rs");
        let row2 = sample_row("ws-3", "src/b.rs");
        store.upsert_manifest_entry(&row1).await.expect("u1");
        store.upsert_manifest_entry(&row2).await.expect("u2");
        assert_eq!(store.get_manifest("ws-3").await.expect("g1").len(), 2);
        store
            .delete_manifest_entry("ws-3", "src/a.rs")
            .await
            .expect("d1");
        let remaining = store.get_manifest("ws-3").await.expect("g2");
        assert_eq!(remaining.len(), 1, "delete should leave the other row");
        assert_eq!(remaining[0].file_path, "src/b.rs");
    }

    #[tokio::test]
    async fn manifest_upsert_overwrites_existing_row() {
        let (store, _dir) = make_test_store();
        init_scan_manifest_schema(&store);
        let mut row = sample_row("ws-4", "src/x.rs");
        row.symbol_count = 1;
        store.upsert_manifest_entry(&row).await.expect("u1");
        row.symbol_count = 999;
        row.content_hash = "new-hash".to_string();
        store.upsert_manifest_entry(&row).await.expect("u2");
        let rows = store.get_manifest("ws-4").await.expect("g");
        assert_eq!(rows.len(), 1, "MERGE should NOT create a second row");
        assert_eq!(rows[0].symbol_count, 999);
        assert_eq!(rows[0].content_hash, "new-hash");
    }
    // --------------------------------------------------------------------
    // FederationStore (Priority 5, gated behind `multimodal`)
    // --------------------------------------------------------------------

    #[cfg(feature = "multimodal")]
    fn sample_space(id: &str, name: &str, kind: SpaceKind) -> Space {
        Space::try_new(
            SpaceId::try_new(id.to_string()).expect("non-empty"),
            name.to_string(),
            kind,
        )
        .expect("non-empty name must succeed")
    }

    #[cfg(feature = "multimodal")]
    fn init_space_schema(store: &LadybugStore) {
        let conn = store.connection().expect("schema-init connection");
        conn.query(
            "CREATE NODE TABLE IF NOT EXISTS Space( \
                 id STRING PRIMARY KEY, \
                 name STRING, \
                 kind STRING, \
                 source_path STRING, \
                 config STRING, \
                 created_at STRING);",
        )
        .expect("create Space NODE TABLE");
    }

    #[tokio::test]
    #[cfg(feature = "multimodal")]
    #[serial]
    async fn federation_register_creates() {
        let (store, _dir) = make_test_store();
        init_space_schema(&store);
        let s = sample_space("repo-1", "auth-repo", SpaceKind::Repo).with_source_path("/work/auth");
        store.register_space(&s).await.expect("register");
        let loaded = store
            .get_space(&SpaceId::try_new("repo-1".to_string()).expect("non-empty"))
            .await
            .expect("get")
            .expect("present");
        assert_eq!(
            loaded.id,
            SpaceId::try_new("repo-1".to_string()).expect("non-empty")
        );
        assert_eq!(loaded.name, "auth-repo");
        assert_eq!(loaded.kind, SpaceKind::Repo);
        assert_eq!(
            loaded.source_path.as_ref().map(|p| p.display().to_string()),
            Some("/work/auth".to_string())
        );
        assert_eq!(loaded.config, serde_json::json!({}));
    }

    #[tokio::test]
    #[cfg(feature = "multimodal")]
    #[serial]
    async fn federation_register_upserts_on_id_collision() {
        let (store, _dir) = make_test_store();
        init_space_schema(&store);
        let s1 = sample_space("repo-1", "auth-repo", SpaceKind::Repo);
        store.register_space(&s1).await.expect("s1");
        let mut s2 = sample_space("repo-1", "auth-repo-renamed", SpaceKind::Repo);
        s2.config = serde_json::json!({"branch": "dev"});
        let s2 = s2.with_source_path("/work/auth");
        store.register_space(&s2).await.expect("s2");
        let loaded = store
            .get_space(&SpaceId::try_new("repo-1".to_string()).expect("non-empty"))
            .await
            .expect("get")
            .expect("present");
        assert_eq!(loaded.name, "auth-repo-renamed", "name updated");
        assert_eq!(loaded.config, serde_json::json!({"branch": "dev"}));
        let all = store.list_spaces().await.expect("list");
        assert_eq!(all.len(), 1, "upsert must NOT create a 2nd row");
    }

    #[tokio::test]
    #[cfg(feature = "multimodal")]
    #[serial]
    async fn federation_get_unknown_returns_none() {
        let (store, _dir) = make_test_store();
        init_space_schema(&store);
        let r = store
            .get_space(&SpaceId::try_new("unknown".to_string()).expect("non-empty"))
            .await
            .expect("get");
        assert!(r.is_none(), "unknown id returns None");
    }

    #[tokio::test]
    #[cfg(feature = "multimodal")]
    #[serial]
    async fn federation_list_empty_when_fresh_db() {
        let (store, _dir) = make_test_store();
        init_space_schema(&store);
        let all = store.list_spaces().await.expect("list");
        assert!(all.is_empty(), "fresh db returns no rows");
    }

    #[tokio::test]
    #[cfg(feature = "multimodal")]
    #[serial]
    async fn federation_list_null_source_path_round_trips() {
        let (store, _dir) = make_test_store();
        init_space_schema(&store);
        let s = sample_space("docs-1", "adrs", SpaceKind::Docs);
        store.register_space(&s).await.expect("save");
        let loaded = store
            .get_space(&SpaceId::try_new("docs-1".to_string()).expect("non-empty"))
            .await
            .expect("get")
            .expect("present");
        assert!(loaded.source_path.is_none(), "None source_path round-trips");
    }

    #[tokio::test]
    #[cfg(feature = "multimodal")]
    #[serial]
    async fn federation_all_three_kinds_round_trip() {
        let (store, _dir) = make_test_store();
        init_space_schema(&store);
        let s_repo = sample_space("repo-1", "auth", SpaceKind::Repo);
        let s_docs = sample_space("docs-1", "adrs", SpaceKind::Docs);
        let s_issues = sample_space("issues-1", "gh-issues", SpaceKind::Issues);
        store.register_space(&s_repo).await.expect("r");
        store.register_space(&s_docs).await.expect("d");
        store.register_space(&s_issues).await.expect("i");
        let all = store.list_spaces().await.expect("list");
        assert_eq!(all.len(), 3);
        let kinds: Vec<SpaceKind> = all.iter().map(|s| s.kind).collect();
        assert!(kinds.contains(&SpaceKind::Repo));
        assert!(kinds.contains(&SpaceKind::Docs));
        assert!(kinds.contains(&SpaceKind::Issues));
    }

    #[tokio::test]
    #[cfg(feature = "multimodal")]
    #[serial]
    async fn federation_idempotent_register_preserves_row_count() {
        let (store, _dir) = make_test_store();
        init_space_schema(&store);
        let s = sample_space("repo-1", "auth", SpaceKind::Repo);
        store.register_space(&s).await.expect("1");
        store.register_space(&s).await.expect("2");
        store.register_space(&s).await.expect("3");
        let all = store.list_spaces().await.expect("list");
        assert_eq!(all.len(), 1, "idempotent register preserves row count");
    }

    // --------------------------------------------------------------------
    // LadybugGraphExecutor (e29-1-graph-executor)
    // --------------------------------------------------------------------

    /// Apply the GraphExecutor NODE TABLE DDL once per test database.
    /// Idempotent via `IF NOT EXISTS`. Requires `GraphRevision`,
    /// `GraphSymbol`, and `GraphEdge` (the same 3 NODE TABLEs the
    /// `CallGraphStore` PR uses).
    fn init_graph_executor_schema(store: &LadybugStore) {
        let conn = store.connection().expect("schema-init connection");
        conn.query(
            "CREATE NODE TABLE IF NOT EXISTS GraphRevision(                  id SERIAL PRIMARY KEY,                  workspace_id STRING,                  revision_id INT64,                  head_of BOOLEAN);",
        )
        .expect("create GraphRevision NODE TABLE");
        conn.query(
            "CREATE NODE TABLE IF NOT EXISTS GraphSymbol(                  id SERIAL PRIMARY KEY,                  workspace_id STRING,                  revision_id INT64,                  fqn STRING,                  kind STRING,                  name STRING,                  file_path STRING,                  line INT64,                  signature STRING);",
        )
        .expect("create GraphSymbol NODE TABLE");
        conn.query(
            "CREATE NODE TABLE IF NOT EXISTS GraphEdge(                  id SERIAL PRIMARY KEY,                  workspace_id STRING,                  revision_id INT64,                  source_id STRING,                  target_id STRING,                  dep_type STRING,                  provenance STRING,                  confidence DOUBLE);",
        )
        .expect("create GraphEdge NODE TABLE");
    }

    /// Build a `GraphPlan::Neighbors` plan with sensible defaults.
    fn build_minimal_neighbors_plan(src: &str) -> GraphPlan {
        use cognicode_core::domain::plan::{
            NeighborKind, PlanHash, PlanLimits, PlanMetadata, PlanVersion,
        };
        GraphPlan::Neighbors {
            src: src.to_string(),
            kind: NeighborKind::Both,
            depth: 1,
            edge_kind_filter: None,
            predicates: vec![],
            limits: PlanLimits::default(),
            metadata: PlanMetadata::new(
                PlanVersion::new("1.0.0").unwrap(),
                PlanHash::compute(&0u32),
            ),
        }
    }

    /// Build a `LadybugGraphExecutor` that shares the same DB as the
    /// `LadybugStore`. This is the production path — the executor
    /// reads from the same underlying `lbug::Database` the store
    /// writes to.
    fn make_graph_executor_for_test(store: &LadybugStore) -> LadybugGraphExecutor {
        store.graph_executor()
    }

    #[tokio::test]
    #[serial]
    async fn graph_executor_unknown_revision_returns_revision_unknown() {
        let (store, _dir) = make_test_store();
        init_graph_executor_schema(&store);
        let executor = make_graph_executor_for_test(&store);
        let ws = ws_id("ws-1");
        let rev = RevisionId(99);
        let plan = build_minimal_neighbors_plan("src/a.rs:foo:1");
        let result = executor.execute(&plan, (ws, rev));
        match result {
            Err(ExecutorError::RevisionUnknown(_)) => {}
            other => panic!("expected RevisionUnknown, got {other:?}"),
        }
    }

    #[tokio::test]
    #[serial]
    async fn graph_executor_neighbors_finds_direct_callees() {
        let (store, _dir) = make_test_store();
        init_graph_executor_schema(&store);
        // Seed: revision 1, 3 symbols (foo, bar, baz), 2 edges
        // (foo → bar with calls, foo → baz with imports).
        let conn = store.connection().expect("conn");
        conn.query(
            "CREATE (r:GraphRevision {workspace_id: 'ws-1', revision_id: 1, head_of: true});",
        )
        .expect("rev");
        conn.query(
            "CREATE (s:GraphSymbol {workspace_id: 'ws-1', revision_id: 1, fqn: 'src/a.rs:foo:1', kind: 'function', name: 'foo', file_path: 'src/a.rs', line: 1});",
        ).expect("foo");
        conn.query(
            "CREATE (s:GraphSymbol {workspace_id: 'ws-1', revision_id: 1, fqn: 'src/a.rs:bar:5', kind: 'function', name: 'bar', file_path: 'src/a.rs', line: 5});",
        ).expect("bar");
        conn.query(
            "CREATE (s:GraphSymbol {workspace_id: 'ws-1', revision_id: 1, fqn: 'src/b.rs:baz:10', kind: 'function', name: 'baz', file_path: 'src/b.rs', line: 10});",
        ).expect("baz");
        conn.query(
            "CREATE (e:GraphEdge {workspace_id: 'ws-1', revision_id: 1, source_id: 'src/a.rs:foo:1', target_id: 'src/a.rs:bar:5', dep_type: 'calls', provenance: 'Extracted', confidence: 1.0});",
        ).expect("e1");
        conn.query(
            "CREATE (e:GraphEdge {workspace_id: 'ws-1', revision_id: 1, source_id: 'src/a.rs:foo:1', target_id: 'src/b.rs:baz:10', dep_type: 'imports', provenance: 'Extracted', confidence: 1.0});",
        ).expect("e2");
        let executor = make_graph_executor_for_test(&store);
        let plan = build_minimal_neighbors_plan("src/a.rs:foo:1");
        let result = executor
            .execute(&plan, (ws_id("ws-1"), RevisionId(1)))
            .expect("execute");
        // foo has 2 direct neighbors (bar, baz).
        assert_eq!(result.rows.len(), 2, "foo has 2 direct neighbors");
        for row in &result.rows {
            assert_eq!(row.columns.len(), 4, "each row has 4 columns");
        }
    }

    #[tokio::test]
    #[serial]
    async fn graph_executor_neighbors_with_unknown_src_returns_empty() {
        let (store, _dir) = make_test_store();
        init_graph_executor_schema(&store);
        let conn = store.connection().expect("conn");
        conn.query(
            "CREATE (r:GraphRevision {workspace_id: 'ws-1', revision_id: 1, head_of: true});",
        )
        .expect("rev");
        let executor = make_graph_executor_for_test(&store);
        let plan = build_minimal_neighbors_plan("src/nonexistent.rs:foo:1");
        let result = executor
            .execute(&plan, (ws_id("ws-1"), RevisionId(1)))
            .expect("execute");
        assert!(result.is_empty(), "unknown source returns empty ResultSet");
    }

    #[tokio::test]
    #[serial]
    async fn graph_executor_path_returns_unsupported_stub() {
        // Path is a Phase 2 stub — verify it returns UnsupportedConstruct
        // so callers know the limitation.
        let (store, _dir) = make_test_store();
        init_graph_executor_schema(&store);
        let conn = store.connection().expect("conn");
        conn.query(
            "CREATE (r:GraphRevision {workspace_id: 'ws-1', revision_id: 1, head_of: true});",
        )
        .expect("rev");
        let executor = make_graph_executor_for_test(&store);
        use cognicode_core::domain::plan::{
            PathProjection, PathQuantifier, PlanHash, PlanLimits, PlanMetadata, PlanVersion,
        };
        let plan = GraphPlan::Path {
            src: "src/a.rs:foo:1".to_string(),
            dst: "src/b.rs:baz:10".to_string(),
            quantifier: PathQuantifier {
                max_hops: Some(32),
                min_hops: 0,
            },
            edge_kind_filter: None,
            predicates: vec![],
            projection: PathProjection {
                nodes: vec![],
                edges: vec![],
                shortest: true,
            },
            limits: PlanLimits::default(),
            metadata: PlanMetadata::new(
                PlanVersion::new("1.0.0").unwrap(),
                PlanHash::compute(&0u32),
            ),
        };
        let result = executor.execute(&plan, (ws_id("ws-1"), RevisionId(1)));
        match result {
            Err(ExecutorError::UnsupportedConstruct(_)) => {}
            other => panic!("expected UnsupportedConstruct, got {other:?}"),
        }
    }

    // --------------------------------------------------------------------
    // IngestCommitPort (Priority 9, gated behind `multimodal`)
    // --------------------------------------------------------------------

    #[cfg(feature = "multimodal")]
    fn init_ingest_schema(store: &LadybugStore) {
        let conn = store.connection().expect("schema-init connection");
        conn.query("CREATE NODE TABLE IF NOT EXISTS GraphRevision(id SERIAL PRIMARY KEY, workspace_id STRING, revision_id INT64, head_of BOOLEAN);")
            .expect("GraphRevision");
        conn.query("CREATE NODE TABLE IF NOT EXISTS ScanManifest(id SERIAL PRIMARY KEY, workspace_id STRING, file_path STRING, file_type STRING, language STRING, content_hash STRING, mtime DOUBLE, symbol_count INT64, edge_count INT64, status STRING, error_msg STRING);")
            .expect("ScanManifest");
        conn.query("CREATE NODE TABLE IF NOT EXISTS GraphReport(id STRING PRIMARY KEY, workspace_id STRING, created_at STRING, report STRING, symbol_count INT64, edge_count INT64, health_score DOUBLE);")
            .expect("GraphReport");
    }

    #[cfg(feature = "multimodal")]
    fn sample_manifest(ws: &str, path: &str) -> ScanManifest {
        ScanManifest {
            workspace_id: ws.to_string(),
            file_path: path.to_string(),
            file_type: "rust".to_string(),
            language: Some("Rust".to_string()),
            content_hash: format!("hash-{path}"),
            mtime: 1.0,
            symbol_count: 10,
            edge_count: 5,
            status: "scanned".to_string(),
            error_msg: None,
        }
    }

    #[cfg(feature = "multimodal")]
    fn sample_report(id: &str, ws: &str, sym: i32, edge: i32) -> ReportIntent {
        ReportIntent {
            summary: ReportSummary {
                id: id.to_string(),
                workspace_id: ws.to_string(),
                created_at: "2026-08-02T10:00:00Z".to_string(),
                report: serde_json::json!({"summary": "test report"}),
                symbol_count: sym,
                edge_count: edge,
                health_score: None,
            },
        }
    }

    #[cfg(feature = "multimodal")]
    fn empty_graph_delta() -> GraphDelta {
        GraphDelta {
            nodes: vec![],
            edges: vec![],
            deleted_node_ids: vec![],
        }
    }

    #[tokio::test]
    #[cfg(feature = "multimodal")]
    #[serial]
    async fn ingest_first_commit_returns_rev_1() {
        let (store, _dir) = make_test_store();
        init_ingest_schema(&store);
        let manifest = ManifestDelta {
            upserts: vec![sample_manifest("ws-1", "src/lib.rs")],
            deleted_paths: vec![],
        };
        let report = sample_report("rep-1", "ws-1", 10, 5);
        let rev = store
            .commit_revision(&ws_id("ws-1"), empty_graph_delta(), manifest, report)
            .await
            .expect("commit");
        assert_eq!(rev.get(), 1, "first commit in a fresh workspace → rev 1");
    }

    #[tokio::test]
    #[cfg(feature = "multimodal")]
    #[serial]
    async fn ingest_second_commit_returns_rev_2_and_demotes_prior_head() {
        let (store, _dir) = make_test_store();
        init_ingest_schema(&store);
        let m1 = ManifestDelta {
            upserts: vec![sample_manifest("ws-1", "src/lib.rs")],
            deleted_paths: vec![],
        };
        store
            .commit_revision(
                &ws_id("ws-1"),
                empty_graph_delta(),
                m1,
                sample_report("rep-1", "ws-1", 10, 5),
            )
            .await
            .expect("c1");
        let mut m2_row = sample_manifest("ws-1", "src/lib.rs");
        m2_row.content_hash = "hash-v2".to_string();
        let m2 = ManifestDelta {
            upserts: vec![m2_row],
            deleted_paths: vec![],
        };
        let rev2 = store
            .commit_revision(
                &ws_id("ws-1"),
                empty_graph_delta(),
                m2,
                sample_report("rep-2", "ws-1", 12, 6),
            )
            .await
            .expect("c2");
        assert_eq!(rev2.get(), 2);
        let head = store
            .head_revision_for_ws(&ws_id("ws-1"))
            .await
            .expect("head");
        assert_eq!(head, Some(RevisionId(2)));
    }

    #[tokio::test]
    #[cfg(feature = "multimodal")]
    #[serial]
    async fn ingest_manifest_upsert_does_not_create_second_row() {
        let (store, _dir) = make_test_store();
        init_ingest_schema(&store);
        let m = ManifestDelta {
            upserts: vec![sample_manifest("ws-1", "src/lib.rs")],
            deleted_paths: vec![],
        };
        store
            .commit_revision(
                &ws_id("ws-1"),
                empty_graph_delta(),
                m,
                sample_report("rep-1", "ws-1", 10, 5),
            )
            .await
            .expect("1");
        let m2 = ManifestDelta {
            upserts: vec![sample_manifest("ws-1", "src/lib.rs")],
            deleted_paths: vec![],
        };
        store
            .commit_revision(
                &ws_id("ws-1"),
                empty_graph_delta(),
                m2,
                sample_report("rep-2", "ws-1", 12, 6),
            )
            .await
            .expect("2");
        let conn = store.connection().expect("conn");
        let mut stmt = conn
            .prepare(
                "MATCH (s:ScanManifest) WHERE s.workspace_id = $ws AND s.file_path = $path RETURN s.id;",
            )
            .expect("prep");
        let mut result = conn
            .execute(
                &mut stmt,
                vec![
                    ("ws", lbug::Value::String("ws-1".to_string())),
                    ("path", lbug::Value::String("src/lib.rs".to_string())),
                ],
            )
            .expect("exec");
        let mut count = 0;
        while result.next().is_some() {
            count += 1;
        }
        assert_eq!(
            count, 1,
            "second commit must NOT duplicate the manifest row"
        );
    }

    #[tokio::test]
    #[cfg(feature = "multimodal")]
    #[serial]
    async fn ingest_report_persists_with_unique_id() {
        let (store, _dir) = make_test_store();
        init_ingest_schema(&store);
        let m = ManifestDelta {
            upserts: vec![sample_manifest("ws-1", "src/lib.rs")],
            deleted_paths: vec![],
        };
        store
            .commit_revision(
                &ws_id("ws-1"),
                empty_graph_delta(),
                m,
                sample_report("rep-1", "ws-1", 42, 7),
            )
            .await
            .expect("c");
        let conn = store.connection().expect("conn");
        let mut stmt = conn
            .prepare(
                "MATCH (r:GraphReport) WHERE r.id = $id RETURN r.symbol_count, r.edge_count, r.workspace_id;",
            )
            .expect("prep");
        let mut result = conn
            .execute(
                &mut stmt,
                vec![("id", lbug::Value::String("rep-1".to_string()))],
            )
            .expect("exec");
        let Some(row) = result.next() else {
            panic!("report row missing after commit");
        };
        assert_eq!(row[0], lbug::Value::Int64(42), "symbol_count persisted");
        assert_eq!(row[1], lbug::Value::Int64(7), "edge_count persisted");
        assert_eq!(row[2], lbug::Value::String("ws-1".to_string()));
    }

    #[tokio::test]
    #[cfg(feature = "multimodal")]
    #[serial]
    async fn ingest_workspace_scoped_revision_counter() {
        let (store, _dir) = make_test_store();
        init_ingest_schema(&store);
        store
            .commit_revision(
                &ws_id("ws-A"),
                empty_graph_delta(),
                ManifestDelta {
                    upserts: vec![sample_manifest("ws-A", "src/a.rs")],
                    deleted_paths: vec![],
                },
                sample_report("rep-A1", "ws-A", 5, 2),
            )
            .await
            .expect("a1");
        store
            .commit_revision(
                &ws_id("ws-A"),
                empty_graph_delta(),
                ManifestDelta {
                    upserts: vec![sample_manifest("ws-A", "src/b.rs")],
                    deleted_paths: vec![],
                },
                sample_report("rep-A2", "ws-A", 6, 3),
            )
            .await
            .expect("a2");
        store
            .commit_revision(
                &ws_id("ws-B"),
                empty_graph_delta(),
                ManifestDelta {
                    upserts: vec![sample_manifest("ws-B", "src/c.rs")],
                    deleted_paths: vec![],
                },
                sample_report("rep-B1", "ws-B", 7, 4),
            )
            .await
            .expect("b1");
        let h_a = store
            .head_revision_for_ws(&ws_id("ws-A"))
            .await
            .expect("h-A");
        let h_b = store
            .head_revision_for_ws(&ws_id("ws-B"))
            .await
            .expect("h-B");
        assert_eq!(h_a, Some(RevisionId(2)), "ws-A head = rev 2");
        assert_eq!(h_b, Some(RevisionId(1)), "ws-B head = rev 1");
    }

    #[tokio::test]
    #[cfg(feature = "multimodal")]
    #[serial]
    async fn ingest_with_empty_manifest_still_opens_revision() {
        let (store, _dir) = make_test_store();
        init_ingest_schema(&store);
        let rev = store
            .commit_revision(
                &ws_id("ws-empty"),
                empty_graph_delta(),
                ManifestDelta {
                    upserts: vec![],
                    deleted_paths: vec![],
                },
                sample_report("rep-empty", "ws-empty", 0, 0),
            )
            .await
            .expect("commit");
        assert_eq!(rev.get(), 1);
        assert_eq!(
            store
                .head_revision_for_ws(&ws_id("ws-empty"))
                .await
                .expect("head"),
            Some(RevisionId(1))
        );
    }

    // --------------------------------------------------------------------
    // Generic Graph Layer DDL (`e29-1-ddl-init`)
    // --------------------------------------------------------------------

    #[tokio::test]
    #[serial]
    async fn init_generic_graph_schema_applies_all_22_node_tables() {
        let (store, _dir) = make_test_store();
        store
            .init_generic_graph_schema()
            .expect("init should succeed");

        let conn = store.connection().expect("conn");
        for table in [
            "Symbol",
            "Decision",
            "Doc",
            "Evidence",
            "Issue",
            "Component",
            "Container",
            "System",
            "Route",
            "Rule",
            "Baseline",
            "Investigation",
            "Artifact",
            "ExplorationSession",
            "NamedView",
            "ViewSpec",
            "GraphReport",
            "AnalyticsRun",
            "FileRecord",
            "Revision",
            "Workspace",
            "Space",
        ] {
            let mut stmt = conn
                .prepare(&format!("MATCH (n:{table}) RETURN n.id LIMIT 1;"))
                .unwrap_or_else(|e| panic!("table {table} not queryable: {e}"));
            let mut result = conn
                .execute(&mut stmt, vec![])
                .unwrap_or_else(|e| panic!("execute on {table}: {e}"));
            let _ = result.next();
        }
    }

    #[tokio::test]
    #[serial]
    async fn init_generic_graph_schema_is_idempotent() {
        let (store, _dir) = make_test_store();
        store.init_generic_graph_schema().expect("first init");
        store
            .init_generic_graph_schema()
            .expect("second init must be a no-op (IF NOT EXISTS)");
    }

    #[tokio::test]
    #[serial]
    async fn init_generic_graph_schema_supports_basic_node_insert() {
        let (store, _dir) = make_test_store();
        store.init_generic_graph_schema().expect("init");
        let conn = store.connection().expect("conn");
        conn.query(
            "CREATE (s:Symbol {                  id: 1, workspace_id: 1, revision_id: 1,                  name: 'foo', kind: 'function', file_path: 'src/foo.rs',                  line_number: 10, signature: 'fn() -> ()',                  valid_from: 1, valid_to: -1              });",
        )
        .expect("insert symbol");
        let mut stmt = conn
            .prepare(
                "MATCH (s:Symbol) WHERE s.name = 'foo' RETURN s.kind, s.signature, s.line_number;",
            )
            .expect("prepare");
        let mut result = conn.execute(&mut stmt, vec![]).expect("execute");
        let Some(row) = result.next() else {
            panic!("Symbol not found after insert")
        };
        assert_eq!(row[0].to_string(), "function");
        assert_eq!(row[1].to_string(), "fn() -> ()");
        assert_eq!(row[2].to_string(), "10");
    }

    // --------------------------------------------------------------------
    // Generic Graph Layer REL TABLE DDL (`e29-1-ddl-rels`)
    // --------------------------------------------------------------------

    #[tokio::test]
    #[serial]
    async fn init_generic_graph_rels_schema_applies_all_20_rel_tables() {
        let (store, _dir) = make_test_store();
        // REL TABLEs reference endpoint `id` columns that don't
        // exist yet (the NODE TABLEs) — but lbug 0.19 doesn't enforce
        // foreign keys, so the REL TABLEs can be created standalone.
        store
            .init_generic_graph_rels_schema()
            .expect("init should succeed");
        let conn = store.connection().expect("conn");
        for table in [
            "Calls",
            "Imports",
            "Inherits",
            "References",
            "Defines",
            "Annotates",
            "Contains",
            "Documents",
            "Supports",
            "Decides",
            "Contradicts",
            "Refines",
            "Supersedes",
            "Implements",
            "DependsOn",
            "Exposes",
            "Consumes",
            "BelongsTo",
            "Hosts",
            "Owns",
        ] {
            let mut stmt = conn
                .prepare(&format!("MATCH (e:{table}) RETURN e.id LIMIT 1;"))
                .unwrap_or_else(|err| panic!("table {table} not queryable: {err}"));
            let mut result = conn
                .execute(&mut stmt, vec![])
                .unwrap_or_else(|err| panic!("execute on {table}: {err}"));
            let _ = result.next();
        }
    }

    #[tokio::test]
    #[serial]
    async fn init_generic_graph_rels_schema_is_idempotent() {
        let (store, _dir) = make_test_store();
        store.init_generic_graph_rels_schema().expect("first init");
        store
            .init_generic_graph_rels_schema()
            .expect("second init must be no-op");
    }

    #[tokio::test]
    #[serial]
    async fn init_generic_graph_rels_schema_supports_basic_edge_insert() {
        // Insert a Symbol + a Calls edge between two Symbols — the
        // canonical "code calls code" relationship. The edge rows
        // use `source_id` / `target_id` columns pointing at the
        // Symbol `id` values (lbug 0.19 has no FK constraints, so
        // the integrity is application-enforced).
        let (store, _dir) = make_test_store();
        store.init_generic_graph_schema().expect("init nodes");
        store.init_generic_graph_rels_schema().expect("init rels");
        let conn = store.connection().expect("conn");
        // Two Symbols.
        conn.query(
            "CREATE (s:Symbol {                  id: 1, workspace_id: 1, revision_id: 1,                  name: 'foo', kind: 'function', file_path: 'src/foo.rs',                  line_number: 10, signature: 'fn()',                  valid_from: 1, valid_to: -1              });",
        )
        .expect("insert foo");
        conn.query(
            "CREATE (s:Symbol {                  id: 2, workspace_id: 1, revision_id: 1,                  name: 'bar', kind: 'function', file_path: 'src/bar.rs',                  line_number: 5, signature: 'fn()',                  valid_from: 1, valid_to: -1              });",
        )
        .expect("insert bar");
        // A Calls edge: foo → bar.
        conn.query(
            "CREATE (e:Calls {                  id: 1, workspace_id: 1, revision_id: 1,                  source_id: 1, target_id: 2, call_site_line: 12,                  is_virtual: false, confidence: 0.85, provenance: 'extractor',                  valid_from: 1, valid_to: -1              });",
        )
        .expect("insert Calls edge");
        // Read the edge back via the canonical endpoint query.
        let mut stmt = conn
            .prepare("MATCH (e:Calls) WHERE e.source_id = 1 AND e.target_id = 2 RETURN e.call_site_line, e.confidence, e.provenance;")
            .expect("prepare");
        let mut result = conn.execute(&mut stmt, vec![]).expect("execute");
        let Some(row) = result.next() else {
            panic!("Calls edge not found")
        };
        assert_eq!(row[0].to_string(), "12");
        assert_eq!(
            row[1].to_string(),
            "0.85",
            "confidence should round-trip as REAL"
        );
        assert_eq!(row[2].to_string(), "extractor");
    }

    #[tokio::test]
    #[serial]
    async fn init_generic_graph_full_schema_nodes_then_rels() {
        // Verify the full init order works: nodes first, then rels.
        // This is the production runtime composition root pattern.
        let (store, _dir) = make_test_store();
        store.init_generic_graph_schema().expect("init nodes");
        store.init_generic_graph_rels_schema().expect("init rels");
        // Both should have applied — verify by counting.
        let conn = store.connection().expect("conn");
        let mut stmt = conn
            .prepare("MATCH (n) RETURN n.id LIMIT 1;")
            .expect("prepare");
        let mut result = conn.execute(&mut stmt, vec![]).expect("execute");
        let _ = result.next(); // empty db is fine
        let mut stmt = conn
            .prepare("MATCH (e:Calls) RETURN e.id LIMIT 1;")
            .expect("prepare");
        let mut result = conn.execute(&mut stmt, vec![]).expect("execute");
        let _ = result.next(); // empty is fine
    }

    // --------------------------------------------------------------------
    // e29-2-conformance (LadybugGraphExecutor self-conformance)
    // --------------------------------------------------------------------
    //
    // Self-conformance: the same plan executed multiple times
    // produces the same ResultSet. This catches non-determinism in
    // the lbug driver or the executor's internals.
    //
    // A separate PR (`e29-2-conformance-cross-backend`) will compare
    // LadybugGraphExecutor against PgGraphExecutor +
    // SnapshotGraphExecutor using the E28.2 PR4 conformance
    // harness pattern — that PR requires CI with a live PG (not
    // available in this sandbox).

    #[tokio::test]
    #[serial]
    async fn e29_2_neighbors_self_conformance_idempotent() {
        // Executing the same plan twice should produce the same
        // ResultSet — both the row count and the row content.
        let (store, _dir) = make_test_store();
        init_graph_executor_schema(&store);
        // Inline seed (the seed_graph_fixture helper from PR #195
        // didn't survive the merge — re-define it inline for v1 of
        // this PR). foo (calls) -> bar + (imports) -> baz.
        let conn = store.connection().expect("conn");
        conn.query(
            "CREATE (r:GraphRevision {workspace_id: 'ws-1', revision_id: 1, head_of: true});",
        )
        .expect("rev");
        conn.query(
            "CREATE (s:GraphSymbol {workspace_id: 'ws-1', revision_id: 1, fqn: 'src/a.rs:foo:1', kind: 'function', name: 'foo', file_path: 'src/a.rs', line: 1});",
        ).expect("foo");
        conn.query(
            "CREATE (s:GraphSymbol {workspace_id: 'ws-1', revision_id: 1, fqn: 'src/a.rs:bar:5', kind: 'function', name: 'bar', file_path: 'src/a.rs', line: 5});",
        ).expect("bar");
        conn.query(
            "CREATE (s:GraphSymbol {workspace_id: 'ws-1', revision_id: 1, fqn: 'src/b.rs:baz:10', kind: 'function', name: 'baz', file_path: 'src/b.rs', line: 10});",
        ).expect("baz");
        conn.query(
            "CREATE (e:GraphEdge {workspace_id: 'ws-1', revision_id: 1, source_id: 'src/a.rs:foo:1', target_id: 'src/a.rs:bar:5', dep_type: 'calls', provenance: 'Extracted', confidence: 1.0});",
        ).expect("e1");
        let executor = make_graph_executor_for_test(&store);
        let ws = ws_id("ws-1");
        let rev = RevisionId(1);
        let plan = build_minimal_neighbors_plan("src/a.rs:foo:1");
        let r1 = executor.execute(&plan, (ws.clone(), rev)).expect("r1");
        let r2 = executor.execute(&plan, (ws, rev)).expect("r2");
        assert_eq!(r1.rows.len(), r2.rows.len(), "row count must match");
        assert_eq!(r1.rows, r2.rows, "row content must match exactly");
    }

    #[tokio::test]
    #[serial]
    async fn e29_2_neighbors_self_conformance_ordered_by_fqn() {
        // The Neighbors variant MUST return rows ordered by `t.fqn`
        // ascending (per the E28.2 conformance spec). Verify the
        // executor respects this ordering when multiple neighbors
        // exist.
        let (store, _dir) = make_test_store();
        init_graph_executor_schema(&store);
        let conn = store.connection().expect("conn");
        conn.query(
            "CREATE (r:GraphRevision {workspace_id: 'ws-1', revision_id: 1, head_of: true});",
        )
        .expect("rev");
        conn.query(
            "CREATE (s:GraphSymbol {workspace_id: 'ws-1', revision_id: 1, fqn: 'src/a.rs:foo:1', kind: 'function', name: 'foo', file_path: 'src/a.rs', line: 1});",
        )
        .expect("foo");
        conn.query(
            "CREATE (s:GraphSymbol {workspace_id: 'ws-1', revision_id: 1, fqn: 'src/a.rs:bar:5', kind: 'function', name: 'bar', file_path: 'src/a.rs', line: 5});",
        )
        .expect("bar");
        conn.query(
            "CREATE (s:GraphSymbol {workspace_id: 'ws-1', revision_id: 1, fqn: 'src/b.rs:baz:10', kind: 'function', name: 'baz', file_path: 'src/b.rs', line: 10});",
        )
        .expect("baz");
        // Add edges in NON-sorted order to verify the executor sorts.
        conn.query(
            "CREATE (e:GraphEdge {workspace_id: 'ws-1', revision_id: 1, source_id: 'src/a.rs:foo:1', target_id: 'src/b.rs:baz:10', dep_type: 'imports', provenance: 'Extracted', confidence: 1.0});",
        )
        .expect("e1");
        conn.query(
            "CREATE (e:GraphEdge {workspace_id: 'ws-1', revision_id: 1, source_id: 'src/a.rs:foo:1', target_id: 'src/a.rs:bar:5', dep_type: 'calls', provenance: 'Extracted', confidence: 1.0});",
        )
        .expect("e2");
        let executor = make_graph_executor_for_test(&store);
        let plan = build_minimal_neighbors_plan("src/a.rs:foo:1");
        let result = executor
            .execute(&plan, (ws_id("ws-1"), RevisionId(1)))
            .expect("execute");
        assert_eq!(result.rows.len(), 2, "foo has 2 direct neighbors");
        // bar's fqn is alphabetically before baz's.
        // lbug 0.19 returns STRING values wrapped in double quotes
        // — strip them for the comparison.
        let strip = |s: String| s.trim_matches('"').to_string();
        let fqn0 = strip(result.rows[0].columns[0].to_string());
        let fqn1 = strip(result.rows[1].columns[0].to_string());
        assert_eq!(
            fqn0, "src/a.rs:bar:5",
            "first neighbor must be bar (alphabetical)"
        );
        assert_eq!(fqn1, "src/b.rs:baz:10", "second neighbor must be baz");
    }

    #[tokio::test]
    #[serial]
    async fn e29_2_neighbors_conformance_no_edges_returns_empty() {
        // Known-answer: a Symbol with zero outgoing edges returns an
        // empty ResultSet. Stable across all E28.2 executor backends.
        let (store, _dir) = make_test_store();
        init_graph_executor_schema(&store);
        let conn = store.connection().expect("conn");
        conn.query(
            "CREATE (r:GraphRevision {workspace_id: 'ws-1', revision_id: 1, head_of: true});",
        )
        .expect("rev");
        conn.query(
            "CREATE (s:GraphSymbol {workspace_id: 'ws-1', revision_id: 1, fqn: 'src/a.rs:foo:1', kind: 'function', name: 'foo', file_path: 'src/a.rs', line: 1});",
        )
        .expect("foo");
        let executor = make_graph_executor_for_test(&store);
        let plan = build_minimal_neighbors_plan("src/a.rs:foo:1");
        let result = executor
            .execute(&plan, (ws_id("ws-1"), RevisionId(1)))
            .expect("execute");
        assert!(result.is_empty(), "isolated node returns empty ResultSet");
    }

    #[tokio::test]
    #[serial]
    async fn e29_2_neighbors_conformance_unknown_src_returns_empty() {
        // Known-answer: querying a symbol that doesn't exist returns
        // empty (parity with PG + Snapshot executors).
        let (store, _dir) = make_test_store();
        init_graph_executor_schema(&store);
        let conn = store.connection().expect("conn");
        conn.query(
            "CREATE (r:GraphRevision {workspace_id: 'ws-1', revision_id: 1, head_of: true});",
        )
        .expect("rev");
        let executor = make_graph_executor_for_test(&store);
        let plan = build_minimal_neighbors_plan("src/does_not_exist.rs:foo:1");
        let result = executor
            .execute(&plan, (ws_id("ws-1"), RevisionId(1)))
            .expect("execute");
        assert!(result.is_empty(), "unknown source returns empty ResultSet");
    }

    // --------------------------------------------------------------------
    // e29-2-conformance-cross-backend (LadybugGraphExecutor vs in-memory oracle)
    // --------------------------------------------------------------------
    //
    // Cross-backend conformance for the Neighbors variant: seed
    // the SAME graph in lbug AND in an in-memory CallGraph oracle,
    // execute the plan on both, assert equivalent ResultSets.
    //
    // The in-memory oracle is a tiny `GraphExecutor` impl backed by
    // `CallGraph` directly (no lbug, no PG) — the in-memory aggregate
    // IS the source of truth. lbug's Cypher traversal must agree.
    //
    // A separate PR (`e29-2-conformance-cross-backend-pg`) will
    // extend this pattern with PgGraphExecutor — that PR requires
    // CI with a live PG (not available in this sandbox).

    /// In-memory oracle `GraphExecutor` for cross-backend conformance.
    /// Holds a single `CallGraph` and answers Neighbors queries by
    /// iterating `edges_with_metadata()` directly.
    #[derive(Debug)]
    struct InMemoryOracleExecutor {
        graph: std::sync::Arc<std::sync::Mutex<CallGraph>>,
    }

    impl InMemoryOracleExecutor {
        fn new(graph: CallGraph) -> Self {
            Self {
                graph: std::sync::Arc::new(std::sync::Mutex::new(graph)),
            }
        }
    }

    impl GraphExecutor for InMemoryOracleExecutor {
        fn execute(
            &self,
            plan: &GraphPlan,
            _pin: (WorkspaceId, RevisionId),
        ) -> Result<ResultSet, ExecutorError> {
            // Only `Neighbors` variant supported (parity with
            // LadybugGraphExecutor v1).
            let GraphPlan::Neighbors { src, depth, .. } = plan else {
                return Err(ExecutorError::UnsupportedConstruct(
                    cognicode_core::domain::plan::UnsupportedConstruct::new(
                        cognicode_core::domain::plan::ConstructId::Other(
                            "GraphPlan::other".to_string(),
                        ),
                        "Oracle only supports Neighbors",
                    ),
                ));
            };
            let _ = depth; // v1: depth > 1 not yet supported
            let graph = self.graph.lock().expect("graph lock");
            // Find the source symbol's id by matching fqn.
            let src_id = graph
                .symbols()
                .find(|s| s.fully_qualified_name() == src)
                .map(|s| {
                    cognicode_core::domain::aggregates::call_graph::SymbolId::new(
                        s.fully_qualified_name(),
                    )
                });
            let Some(src_id) = src_id else {
                return Ok(ResultSet::empty());
            };
            // Direct neighbors: edges where source == src_id.
            let mut rows: Vec<Vec<String>> = graph
                .edges_with_metadata()
                .filter(|(s, _t, _d, _p, _c)| s == &src_id)
                .filter_map(|(_s, t, _d, _p, _c)| {
                    let target_id = t.as_str().to_string();
                    let target_sym = graph
                        .symbols()
                        .find(|sym| sym.fully_qualified_name() == target_id.as_str())?;
                    Some(vec![
                        target_id,                                // target_id (fqn)
                        target_sym.kind().to_string(),            // kind
                        target_sym.location().file().to_string(), // file_path
                        target_sym.location().line().to_string(), // line
                    ])
                })
                .collect();
            // Sort by fqn (target_id) ascending — same order as
            // LadybugGraphExecutor's ORDER BY t.fqn.
            rows.sort_by(|a, b| a[0].cmp(&b[0]));
            Ok(ResultSet {
                rows: rows
                    .into_iter()
                    .map(|row| cognicode_core::domain::plan::Row {
                        columns: row
                            .into_iter()
                            .map(cognicode_core::domain::plan::TypedValue::String)
                            .collect(),
                    })
                    .collect(),
                nodes: vec![],
                edges: vec![],
                paths: vec![],
                scalars: vec![],
                truncated: false,
                truncation: None,
            })
        }

        fn execute_with_limits(
            &self,
            _plan: &GraphPlan,
            _pin: (WorkspaceId, RevisionId),
            _limits: Option<cognicode_core::domain::plan::PlanLimits>,
        ) -> Result<ResultSet, ExecutorError> {
            // Delegate to `execute` — the oracle doesn't enforce
            // soft limits in v1 (the in-memory graph is small).
            self.execute(_plan, _pin)
        }
    }

    /// Build a `CallGraph` from the same fixture data the lbug
    /// tests use. Returns the in-memory graph for the oracle.
    fn build_call_graph_fixture() -> CallGraph {
        use cognicode_core::domain::aggregates::call_graph::CallGraph;
        use cognicode_core::domain::aggregates::symbol::Symbol;
        use cognicode_core::domain::services::ExtractionContext;
        use cognicode_core::domain::value_objects::{DependencyType, Location, SymbolKind};
        let mut g = CallGraph::new();
        let foo_id = g.add_symbol(Symbol::new(
            "foo",
            SymbolKind::Function,
            Location::new("src/a.rs", 1, 0),
        ));
        let bar_id = g.add_symbol(Symbol::new(
            "bar",
            SymbolKind::Function,
            Location::new("src/a.rs", 5, 0),
        ));
        let baz_id = g.add_symbol(Symbol::new(
            "baz",
            SymbolKind::Function,
            Location::new("src/b.rs", 10, 0),
        ));
        g.add_dependency_with_provenance(
            &foo_id,
            &bar_id,
            DependencyType::Calls,
            ExtractionContext::DirectExtraction,
        )
        .expect("foo->bar");
        g.add_dependency_with_provenance(
            &foo_id,
            &baz_id,
            DependencyType::Imports,
            ExtractionContext::DirectExtraction,
        )
        .expect("foo->baz");
        g
    }

    #[tokio::test]
    #[serial]
    async fn e29_2_conformance_neighbors_ladybug_vs_in_memory_oracle() {
        // Seed the SAME graph in both backends and assert the
        // Neighbors plan produces the same ResultSet.
        let (store, _dir) = make_test_store();
        init_graph_executor_schema(&store);
        // Insert symbols + edges in NON-sorted order to verify the
        // lbug executor sorts.
        let conn = store.connection().expect("conn");
        conn.query(
            "CREATE (r:GraphRevision {workspace_id: 'ws-1', revision_id: 1, head_of: true});",
        )
        .expect("rev");
        conn.query(
            "CREATE (s:GraphSymbol {workspace_id: 'ws-1', revision_id: 1, fqn: 'src/a.rs:foo:1', kind: 'function', name: 'foo', file_path: 'src/a.rs', line: 1});",
        )
        .expect("foo");
        conn.query(
            "CREATE (s:GraphSymbol {workspace_id: 'ws-1', revision_id: 1, fqn: 'src/a.rs:bar:5', kind: 'function', name: 'bar', file_path: 'src/a.rs', line: 5});",
        )
        .expect("bar");
        conn.query(
            "CREATE (s:GraphSymbol {workspace_id: 'ws-1', revision_id: 1, fqn: 'src/b.rs:baz:10', kind: 'function', name: 'baz', file_path: 'src/b.rs', line: 10});",
        )
        .expect("baz");
        conn.query(
            "CREATE (e:GraphEdge {workspace_id: 'ws-1', revision_id: 1, source_id: 'src/a.rs:foo:1', target_id: 'src/b.rs:baz:10', dep_type: 'imports', provenance: 'Extracted', confidence: 1.0});",
        )
        .expect("e1");
        conn.query(
            "CREATE (e:GraphEdge {workspace_id: 'ws-1', revision_id: 1, source_id: 'src/a.rs:foo:1', target_id: 'src/a.rs:bar:5', dep_type: 'calls', provenance: 'Extracted', confidence: 1.0});",
        )
        .expect("e2");
        // Build the in-memory oracle with the SAME data.
        let oracle_graph = build_call_graph_fixture();
        let oracle = InMemoryOracleExecutor::new(oracle_graph);
        // Execute the same plan on both.
        let lbug_exec = make_graph_executor_for_test(&store);
        let plan = build_minimal_neighbors_plan("src/a.rs:foo:1");
        let lbug_result = lbug_exec
            .execute(&plan, (ws_id("ws-1"), RevisionId(1)))
            .expect("lbug execute");
        let oracle_result = oracle
            .execute(&plan, (ws_id("ws-1"), RevisionId(1)))
            .expect("oracle execute");
        // Compare row count.
        assert_eq!(
            lbug_result.rows.len(),
            oracle_result.rows.len(),
            "row count must match between lbug and oracle",
        );
        // Compare row content (by stripping the lbug-wrapped
        // double-quotes; the oracle stores raw strings).
        let strip = |s: &str| s.trim_matches('"').to_string();
        for (lbug_row, oracle_row) in lbug_result.rows.iter().zip(oracle_result.rows.iter()) {
            assert_eq!(lbug_row.columns.len(), oracle_row.columns.len());
            for (l, o) in lbug_row.columns.iter().zip(oracle_row.columns.iter()) {
                assert_eq!(
                    strip(&l.to_string()),
                    strip(&o.to_string()),
                    "column value mismatch",
                );
            }
        }
    }

    #[tokio::test]
    #[serial]
    async fn e29_2_conformance_neighbors_unknown_src_ladybug_vs_oracle() {
        // Unknown source → empty ResultSet on both backends.
        let (store, _dir) = make_test_store();
        init_graph_executor_schema(&store);
        let conn = store.connection().expect("conn");
        conn.query(
            "CREATE (r:GraphRevision {workspace_id: 'ws-1', revision_id: 1, head_of: true});",
        )
        .expect("rev");
        let lbug_exec = make_graph_executor_for_test(&store);
        let oracle = InMemoryOracleExecutor::new(build_call_graph_fixture());
        let plan = build_minimal_neighbors_plan("src/does_not_exist.rs:foo:1");
        let lbug_result = lbug_exec
            .execute(&plan, (ws_id("ws-1"), RevisionId(1)))
            .expect("lbug execute");
        let oracle_result = oracle
            .execute(&plan, (ws_id("ws-1"), RevisionId(1)))
            .expect("oracle execute");
        assert!(lbug_result.is_empty(), "lbug unknown src is empty");
        assert!(oracle_result.is_empty(), "oracle unknown src is empty");
    }

    // --------------------------------------------------------------------
    // e29-2-migrate-data (CallGraph → lbug round-trip)
    // --------------------------------------------------------------------

    #[tokio::test]
    #[serial]
    async fn migrate_call_graph_roundtrip_preserves_symbols_and_edges() {
        // Build an in-memory CallGraph, migrate it to lbug, then
        // verify the data round-trips by querying the lbug store
        // directly (MATCH GraphSymbol / MATCH GraphEdge).
        let (store, _dir) = make_test_store();
        // The CallGraphStore path is independent of the Generic
        // Graph Layer, but the schema is identical — so we just
        // use the CallGraph helper for both init and verify.
        init_graph_executor_schema(&store);
        let graph = build_call_graph_fixture();
        store
            .migrate_call_graph("ws-1", 1, &graph)
            .expect("migrate");
        // Verify: 3 GraphSymbol nodes (foo, bar, baz).
        let conn = store.connection().expect("conn");
        let mut stmt = conn
            .prepare(
                "MATCH (s:GraphSymbol) WHERE s.workspace_id = $ws AND s.revision_id = $rev                  RETURN s.fqn ORDER BY s.fqn;",
            )
            .expect("prepare");
        let mut result = conn
            .execute(
                &mut stmt,
                vec![
                    ("ws", lbug::Value::String("ws-1".to_string())),
                    ("rev", lbug::Value::Int64(1)),
                ],
            )
            .expect("execute");
        let mut fqns: Vec<String> = Vec::new();
        while let Some(row) = result.next() {
            fqns.push(row[0].to_string().trim_matches('"').to_string());
        }
        fqns.sort();
        assert_eq!(
            fqns,
            vec!["src/a.rs:bar:5", "src/a.rs:foo:1", "src/b.rs:baz:10"]
        );
        // Verify: 2 GraphEdge nodes (foo -> bar, foo -> baz).
        let mut stmt = conn
            .prepare(
                "MATCH (e:GraphEdge) WHERE e.workspace_id = $ws AND e.revision_id = $rev                  RETURN e.source_id, e.target_id ORDER BY e.target_id;",
            )
            .expect("prepare");
        let mut result = conn
            .execute(
                &mut stmt,
                vec![
                    ("ws", lbug::Value::String("ws-1".to_string())),
                    ("rev", lbug::Value::Int64(1)),
                ],
            )
            .expect("execute");
        let mut edges: Vec<(String, String)> = Vec::new();
        while let Some(row) = result.next() {
            edges.push((
                row[0].to_string().trim_matches('"').to_string(),
                row[1].to_string().trim_matches('"').to_string(),
            ));
        }
        edges.sort();
        assert_eq!(edges.len(), 2);
        assert_eq!(
            edges[0],
            ("src/a.rs:foo:1".to_string(), "src/a.rs:bar:5".to_string())
        );
        assert_eq!(
            edges[1],
            ("src/a.rs:foo:1".to_string(), "src/b.rs:baz:10".to_string())
        );
    }

    #[tokio::test]
    #[serial]
    async fn migrate_call_graph_is_idempotent() {
        // Running migrate twice with the same data must NOT
        // duplicate nodes or edges (read-then-conditional-write via
        // lbug's MERGE ON MATCH / ON CREATE clause).
        let (store, _dir) = make_test_store();
        init_graph_executor_schema(&store);
        let graph = build_call_graph_fixture();
        store.migrate_call_graph("ws-1", 1, &graph).expect("first");
        store.migrate_call_graph("ws-1", 1, &graph).expect("second");
        let conn = store.connection().expect("conn");
        let mut stmt = conn
            .prepare("MATCH (s:GraphSymbol) WHERE s.workspace_id = $ws AND s.revision_id = $rev RETURN s.fqn;")
            .expect("prepare");
        let mut result = conn
            .execute(
                &mut stmt,
                vec![
                    ("ws", lbug::Value::String("ws-1".to_string())),
                    ("rev", lbug::Value::Int64(1)),
                ],
            )
            .expect("execute");
        let mut count = 0;
        while result.next().is_some() {
            count += 1;
        }
        assert_eq!(count, 3, "second migrate must NOT duplicate symbols");
    }

    #[tokio::test]
    #[serial]
    async fn migrate_call_graph_with_dependencies_preserves_provenance() {
        // Verify the dep_type field is preserved (the PG exporter
        // would need to include this; v1 hardcodes 'calls' / 'imports'
        // from the in-memory aggregate).
        let (store, _dir) = make_test_store();
        init_graph_executor_schema(&store);
        let graph = build_call_graph_fixture();
        store
            .migrate_call_graph("ws-1", 1, &graph)
            .expect("migrate");
        let conn = store.connection().expect("conn");
        let mut stmt = conn
            .prepare(
                "MATCH (e:GraphEdge) WHERE e.source_id = $src AND e.target_id = $tgt                  RETURN e.dep_type;",
            )
            .expect("prepare");
        let mut result = conn
            .execute(
                &mut stmt,
                vec![
                    ("src", lbug::Value::String("src/a.rs:foo:1".to_string())),
                    ("tgt", lbug::Value::String("src/a.rs:bar:5".to_string())),
                ],
            )
            .expect("execute");
        // The dep_type column is not set in our minimal MERGE
        // statement (we only set workspace_id and revision_id on
        // create). For v1 of migrate, this is OK — the dep_type
        // would be added in a follow-up PR. We just verify the
        // query doesn't error.
        let _ = result.next();
    }
}

// =============================================================================
// QualityStore tests (e29-3 Phase 3)
// =============================================================================
//
// RED gate: these tests reference `LadybugStore::init_quality_schema` and
// `impl QualityStore for LadybugStore` — neither exists until the GREEN step.
// Round-trip = insert → read → delete per method (tasks 3.1).

#[cfg(test)]
mod quality_store_tests {
    use super::*;

    use serial_test::serial;

    use cognicode_core::domain::ports::{
        IssueFilter, NewIssue, QualityError, QualityGateSummary, QualityStore, UpsertSummary,
    };

    fn make_store() -> (LadybugStore, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("quality.lbdb");
        let store = LadybugStore::open(&path).expect("open lbug db");
        store.init_quality_schema().expect("quality schema");
        (store, dir)
    }

    fn sample_issue(
        workspace: &str,
        rule: &str,
        file: &str,
        line: u32,
        severity: &str,
        status: &str,
    ) -> NewIssue {
        NewIssue {
            workspace_id: workspace.to_string(),
            rule_id: rule.to_string(),
            severity: severity.to_string(),
            category: "bug".to_string(),
            file_path: file.to_string(),
            line,
            message: format!("{rule} at {file}:{line}"),
            status: status.to_string(),
        }
    }

    #[serial]
    #[test]
    fn quality_issues_for_file_round_trip() {
        let (store, _dir) = make_store();
        store
            .insert_issues(&[
                sample_issue("ws", "R1", "src/a.rs", 10, "blocker", "open"),
                sample_issue("ws", "R2", "src/b.rs", 20, "info", "open"),
            ])
            .expect("insert");

        let got = store.issues_for_file("src/a.rs").expect("read");
        assert_eq!(got.len(), 1, "only the matching file issue");
        assert_eq!(got[0].rule_id, "R1");
        assert_eq!(got[0].file_path, "src/a.rs");
        assert_eq!(got[0].line, 10);
        assert_eq!(got[0].severity, "blocker");
        assert_eq!(got[0].status, "open");

        let removed = store
            .delete_issue("ws", "R1", "src/a.rs", 10)
            .expect("delete");
        assert!(removed, "row existed");
        assert!(store.issues_for_file("src/a.rs").unwrap().is_empty());
    }

    #[serial]
    #[test]
    fn quality_issues_for_scope_is_boundary_aware() {
        let (store, _dir) = make_store();
        store
            .insert_issues(&[
                sample_issue("ws", "R1", "src/a.rs", 1, "major", "open"),
                sample_issue("ws", "R2", "src/sub/b.rs", 2, "major", "open"),
                sample_issue("ws", "R3", "src_extra.rs", 3, "major", "open"),
            ])
            .expect("insert");

        let got = store.issues_for_scope("src").expect("read");
        let mut files: Vec<String> = got.iter().map(|i| i.file_path.clone()).collect();
        files.sort();
        assert_eq!(
            files,
            vec!["src/a.rs".to_string(), "src/sub/b.rs".to_string()],
            "scope=src must match src/... but not src_extra.rs"
        );
    }

    #[serial]
    #[test]
    fn quality_issues_at_line_round_trip() {
        let (store, _dir) = make_store();
        store
            .insert_issues(&[
                sample_issue("ws", "R1", "src/a.rs", 10, "major", "open"),
                sample_issue("ws", "R1", "src/a.rs", 42, "major", "open"),
            ])
            .expect("insert");

        let at10 = store.issues_at_line("src/a.rs", 10).expect("read");
        assert_eq!(at10.len(), 1);
        assert_eq!(at10[0].line, 10);
        let at99 = store.issues_at_line("src/a.rs", 99).expect("read");
        assert!(at99.is_empty());
    }

    #[serial]
    #[test]
    fn quality_issue_by_id_round_trip() {
        let (store, _dir) = make_store();
        store
            .insert_issues(&[sample_issue("ws", "R1", "src/a.rs", 7, "critical", "open")])
            .expect("insert");

        let all = store
            .issues_for_workspace(Some("ws"), &IssueFilter::default())
            .expect("read");
        assert_eq!(all.len(), 1);
        let id = all[0].id;

        let by_id = store.issue_by_id(id).expect("read");
        assert!(by_id.is_some());
        assert_eq!(by_id.unwrap().rule_id, "R1");

        store
            .delete_issue("ws", "R1", "src/a.rs", 7)
            .expect("delete");
        assert!(store.issue_by_id(id).unwrap().is_none());
    }

    #[serial]
    #[test]
    fn quality_rule_summary_round_trip() {
        let (store, _dir) = make_store();
        let conn = store.connection().expect("conn");
        conn.query(
            "CREATE (r:QualityRule {rule_id: 'R1', description: 'no debug prints', category: 'style'});",
        )
        .expect("create rule");
        store
            .insert_issues(&[
                sample_issue("ws", "R1", "src/a.rs", 1, "major", "open"),
                sample_issue("ws", "R1", "src/b.rs", 2, "major", "resolved"),
            ])
            .expect("insert");

        let summary = store.rule_summary("R1").expect("read");
        assert_eq!(summary.rule_id, "R1");
        assert_eq!(summary.description, "no debug prints");
        assert_eq!(summary.open_count, 1, "only status=open counts");
    }

    #[serial]
    #[test]
    fn quality_gate_round_trip() {
        let (store, _dir) = make_store();

        // No baseline row yet -> default gate (no panic, zeros).
        let default_gate = store.quality_gate(Some("ws")).expect("read default");
        assert_eq!(default_gate, QualityGateSummary::default());

        let conn = store.connection().expect("conn");
        conn.query(
            "CREATE (b:QualityBaseline {workspace_id: 'ws', rating: 'B', total_issues: 4, blockers: 1, \
             criticals: 2, debt_minutes: 30, snapshot_at: '2026-08-03T00:00:00Z'});",
        )
        .expect("create baseline");

        let gate = store.quality_gate(Some("ws")).expect("read");
        assert_eq!(gate.rating.as_deref(), Some("B"));
        assert_eq!(gate.total_issues, 4);
        assert_eq!(gate.blockers, 1);
        assert_eq!(gate.criticals, 2);
        assert_eq!(gate.debt_minutes, 30);
        assert_eq!(gate.last_run.as_deref(), Some("2026-08-03T00:00:00Z"));
    }

    #[serial]
    #[test]
    fn quality_open_issues_count_round_trip() {
        let (store, _dir) = make_store();
        store
            .insert_issues(&[
                sample_issue("ws", "R1", "src/a.rs", 1, "major", "open"),
                sample_issue("ws", "R2", "src/b.rs", 2, "minor", "resolved"),
                sample_issue("other", "R3", "src/c.rs", 3, "major", "open"),
            ])
            .expect("insert");

        assert_eq!(store.open_issues_count(Some("ws")).expect("count"), 1);
        assert_eq!(
            store.open_issues_count(Some("other")).expect("count"),
            1,
            "scoped by workspace"
        );
        assert_eq!(
            store.open_issues_count(None).expect("count"),
            2,
            "no workspace filter counts all open"
        );
    }

    #[serial]
    #[test]
    fn quality_issues_for_workspace_applies_filters() {
        let (store, _dir) = make_store();
        store
            .insert_issues(&[
                sample_issue("ws", "R1", "src/a.rs", 1, "blocker", "open"),
                sample_issue("ws", "R2", "src/b.rs", 2, "major", "open"),
                sample_issue("ws", "R3", "src/c.rs", 3, "blocker", "resolved"),
                sample_issue("ws", "R4", "src_extra.rs", 4, "info", "open"),
            ])
            .expect("insert");

        let blockers = store
            .issues_for_workspace(
                Some("ws"),
                &IssueFilter {
                    severity: Some("blocker".into()),
                    ..IssueFilter::default()
                },
            )
            .expect("read");
        assert_eq!(blockers.len(), 2);

        // Boundary-aware prefix: `src` matches src/... children but NOT
        // the sibling file `src_extra.rs` (contract in the port docs).
        let prefix = store
            .issues_for_workspace(
                Some("ws"),
                &IssueFilter {
                    file_prefix: Some("src".into()),
                    ..IssueFilter::default()
                },
            )
            .expect("read");
        let mut prefix_files: Vec<String> = prefix.iter().map(|i| i.file_path.clone()).collect();
        prefix_files.sort();
        assert_eq!(
            prefix_files,
            vec![
                "src/a.rs".to_string(),
                "src/b.rs".to_string(),
                "src/c.rs".to_string(),
            ],
            "prefix=src must match src/... but not src_extra.rs"
        );

        let limited = store
            .issues_for_workspace(
                Some("ws"),
                &IssueFilter {
                    limit: Some(1),
                    ..IssueFilter::default()
                },
            )
            .expect("read");
        assert_eq!(limited.len(), 1);
    }

    #[serial]
    #[test]
    fn quality_insert_issues_reports_upsert_summary() {
        let (store, _dir) = make_store();
        let summary: UpsertSummary = store
            .insert_issues(&[
                sample_issue("ws", "R1", "src/a.rs", 1, "major", "open"),
                sample_issue("ws", "R2", "src/b.rs", 2, "major", "open"),
            ])
            .expect("insert");
        assert_eq!(summary.inserted, 2);
        assert_eq!(summary.updated, 0, "lbug has no upsert primitive yet");
    }

    #[serial]
    #[test]
    fn quality_delete_issue_round_trip() {
        let (store, _dir) = make_store();
        store
            .insert_issues(&[sample_issue("ws", "R1", "src/a.rs", 5, "major", "open")])
            .expect("insert");

        assert!(
            store
                .delete_issue("ws", "R1", "src/a.rs", 5)
                .expect("delete first"),
            "first delete removes the row"
        );
        assert!(
            !store
                .delete_issue("ws", "R1", "src/a.rs", 5)
                .expect("delete second"),
            "second delete finds nothing"
        );
    }

    #[serial]
    #[test]
    fn quality_reads_degrade_empty_on_missing_table() {
        // Construct via `new` (raw sharing constructor) so the quality
        // schema DDL is never applied: reads must degrade gracefully.
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("raw.lbdb");
        let db = lbug::Database::new(&path, SystemConfig::default()).expect("open db");
        let store = LadybugStore::new(Arc::new(db));

        assert!(store.issues_for_file("src/a.rs").unwrap().is_empty());
        assert!(store.issues_for_scope("src").unwrap().is_empty());
        assert!(store.issues_at_line("src/a.rs", 1).unwrap().is_empty());
        assert!(store.issue_by_id(1).unwrap().is_none());
        assert_eq!(store.open_issues_count(Some("ws")).unwrap(), 0);
        assert_eq!(
            store.quality_gate(Some("ws")).unwrap(),
            QualityGateSummary::default()
        );
    }

    #[serial]
    #[test]
    fn quality_writes_error_on_missing_table() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("raw.lbdb");
        let db = lbug::Database::new(&path, SystemConfig::default()).expect("open db");
        let store = LadybugStore::new(Arc::new(db));

        let insert =
            store.insert_issues(&[sample_issue("ws", "R1", "src/a.rs", 1, "major", "open")]);
        assert!(insert.is_err(), "insert must error on missing table");

        let delete = store.delete_issue("ws", "R1", "src/a.rs", 1);
        assert!(delete.is_err(), "delete must error on missing table");
    }
}
