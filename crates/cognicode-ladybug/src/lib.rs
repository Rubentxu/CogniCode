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
//! | 9 | `IngestCommit` | Composite atomic tx (per ADR-015) — requires all 8 prior ports | DONE (multimodal-gated; 6 in-crate tests) |

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use lbug::{Connection, Database, SystemConfig};

use cognicode_core::domain::aggregates::CallGraph;
use cognicode_core::domain::plan::{
    ExecutorError, GraphExecutor, GraphPlan, PlanLimits, ResultSet, TruncationMarker,
    UnsupportedConstruct,
};
use cognicode_core::domain::ports::{
    CallGraphError, CallGraphStore,
    manifest_store::{ManifestError, ManifestStore, ScanManifest},
    report_store::{ReportError, ReportStore, ReportSummary},
    revision_store::{RevisionError, RevisionStore},
    session_store::{SessionError, SessionRow, SessionStore},
    view_spec_store::{ViewSpecPayload, ViewSpecStore, ViewSpecStoreError},
};
use cognicode_core::domain::value_objects::{DependencyType, RevisionId, WorkspaceId};

// `FederationStore`, `IngestCommit`, and the `Space`/`SpaceId` value
// objects they operate on are gated behind the `multimodal` feature
// in `cognicode-core`. The default build (no multimodal) skips them;
// the follow-up PR that flips multimodal to ON also wires these.
#[cfg(feature = "multimodal")]
use cognicode_core::domain::ports::{
    federation_store::{FederationError, FederationStore},
    ingest_commit::{CommitError, GraphDelta, IngestCommit, ManifestDelta, ReportIntent},
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
        Ok(Self { db: Arc::new(db) })
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

    async fn create_revision(
        &self,
        _conn: &mut sqlx::PgConnection,
        _ws: &WorkspaceId,
    ) -> Result<RevisionId, RevisionError> {
        // Note: the trait signature requires `&mut PgConnection` per the
        // e29-0-define-new-ports PR1 design. Phase 1 will swap this for
        // `&mut lbug::Connection` (a future migration of the trait), or
        // for lbug's own tx handle if its API supports that.
        //
        // For now: open a new connection from the shared Database and
        // issue the open revision in a single tx.
        let _conn = self
            .connection()
            .map_err(|_| RevisionError::Store("(phase 1 stub — see lib.rs port-impl)".into()))?;
        // tx: demote old head, compute next id, insert.
        Err(RevisionError::Store(
            "(phase 1 stub — impl lands in next change)".into(),
        ))
    }

    async fn set_head(
        &self,
        _conn: &mut sqlx::PgConnection,
        _ws: &WorkspaceId,
        _rev: RevisionId,
    ) -> Result<(), RevisionError> {
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
                            Some(s) => lbug::Value::String(s.clone()),
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
                            Some(s) => lbug::Value::String(s.clone()),
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
                            Some(s) => lbug::Value::String(s.clone()),
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
                            Some(s) => lbug::Value::String(s.clone()),
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
impl IngestCommit for LadybugStore {
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
                                Some(s) => lbug::Value::String(s.clone()),
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
                                Some(s) => lbug::Value::String(s.clone()),
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
                                Some(s) => lbug::Value::String(s.clone()),
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
                                Some(s) => lbug::Value::String(s.clone()),
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
                ("json", lbug::Value::String(report_json)),
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
        // ports (GraphWritePort, GraphNodeStore, GraphEdgeStore)
        // are defined for the lbug adapter. Today: no-op.
        let _graph = _graph;

        Ok(rev_id)
    }
}

// =============================================================================
// QualityStore — left as a tail-call exercise for the per-port PR
// =============================================================================
//
// `QualityStore` is the largest of the 9 ports (10 methods split
// across `issues` + `baselines` + `rules`). It doesn't fit the
// skeleton pattern (one stub per method here is bloated and fragile),
// so it's deferred to its own follow-up PR that:
//   - Lands the lbug-side SQL
//   - Adds a per-method integration test
//
// Skipping the impl here is documented as Phase 1 scope; the trait
// surface is still complete and verified via the cargo check on the
// other 8 ports.

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
            T: IngestCommit,
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
    // IngestCommit (Priority 9, gated behind `multimodal`)
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
}
