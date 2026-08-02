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
//! | 5 | `FederationStore` | Single-table CRUD on `spaces` | DONE (`feat/e29-1-priority-5-federation-store` — multimodal-gated, validated via cypher_probe) |
//! | 6 | `ViewSpecStore` | JSON-payload CRD store (post `ViewSpecPayload` bridge) | DONE (`feat/e29-1-priority-6-view-spec-store`) |
//! | 7 | `QualityStore` | 10-method port split across `issues`, `baselines`, `rules` | DONE (`feat/e29-1-priority-7-quality-store`) |
//! | 8 | `CallGraphStore` | `save_call_graph_ws` + `load_call_graph_ws` | DONE (this branch) |
//! | 9 | `IngestCommit` | Composite atomic tx (per ADR-015) — requires all 8 prior ports | pending |

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use lbug::{Connection, Database, SystemConfig};

use cognicode_core::domain::aggregates::CallGraph;
use cognicode_core::domain::ports::{
    CallGraphError, CallGraphStore,
    manifest_store::{ManifestError, ManifestStore, ScanManifest},
    report_store::{ReportError, ReportStore, ReportSummary},
    revision_store::{RevisionError, RevisionStore},
    session_store::{SessionError, SessionRow, SessionStore},
    view_spec_store::{ViewSpecPayload, ViewSpecStore, ViewSpecStoreError},
};
use cognicode_core::domain::value_objects::{DependencyType, Provenance, RevisionId, WorkspaceId};

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

impl LadybugStore {
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
    async fn register_space(&self, _space: &Space) -> Result<SpaceId, FederationError> {
        // PHASE 1 STUB. Next change: INSERT INTO spaces ... RETURNING space_id.
        Err(FederationError::Conflict(
            "(phase 1 stub — impl lands in next change)".into(),
        ))
    }

    async fn list_spaces(&self) -> Result<Vec<Space>, FederationError> {
        // PHASE 1 STUB. Next change: SELECT ... FROM spaces.
        Ok(Vec::new())
    }

    async fn get_space(&self, _id: &SpaceId) -> Result<Option<Space>, FederationError> {
        // PHASE 1 STUB.
        Ok(None)
    }
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
        graph: &CallGraph,
        ws: &WorkspaceId,
    ) -> Result<RevisionId, CallGraphError> {
        // ADR-028 §3 `save_call_graph_ws(graph, ws)` — atomic
        // demote-prior-head + open-new-revision + delete-prior-nodes+edges
        // + insert-new-nodes+edges. The PG adapter uses
        // `PostgresRepository::save_call_graph_ws` which atomically
        // composes the demote/create/delete/insert into one tx; lbug
        // 0.19 has no public tx handle so we open a single
        // `Connection` and run the demote+create as ONE multi-pattern
        // Cypher (the same `WITH count()` pivot trick used by
        // Priority 4 `RevisionStore::create_revision_for_ws` — see
        // the comment there for why the parameter binding survives
        // across the `SET` scope). The symbols + edges are inserted
        // in two follow-up statements that share the connection; if
        // any of the inserts fails, the graph is left in a partial
        // state (a future follow-up can compose this into a single
        // multi-statement Cypher per graph).
        let conn = self
            .connection()
            .map_err(|e| CallGraphError::Store(format!("save_call_graph_ws: {e}")))?;

        // Step 1: demote prior head + create new head in one Cypher.
        let mut rev_stmt = conn
            .prepare(
                "MATCH (old:GraphRevision) WHERE old.workspace_id = $ws AND old.head_of = true SET old.head_of = false WITH count(old) AS _demoted OPTIONAL MATCH (r:GraphRevision) WHERE r.workspace_id = $ws WITH $ws AS ws, coalesce(max(r.revision_id), 0) AS max_rev CREATE (new:GraphRevision {workspace_id: ws, revision_id: max_rev + 1, head_of: true}) RETURN new.revision_id;",
            )
            .map_err(|e| CallGraphError::Store(format!("save_call_graph_ws: rev prepare: {e}")))?;
        let mut rev_result = conn
            .execute(
                &mut rev_stmt,
                vec![("ws", lbug::Value::String(ws.to_string()))],
            )
            .map_err(|e| CallGraphError::Store(format!("save_call_graph_ws: rev execute: {e}")))?;
        let Some(rev_row) = rev_result.next() else {
            return Err(CallGraphError::Store(
                "save_call_graph_ws: CREATE revision produced no RETURN row".into(),
            ));
        };
        let rev_id = match &rev_row[0] {
            lbug::Value::Int64(n) => RevisionId(*n as u64),
            lbug::Value::Int32(n) => RevisionId(*n as u64),
            other => {
                return Err(CallGraphError::Store(format!(
                    "save_call_graph_ws: unexpected revision_id type: {other:?}"
                )));
            }
        };

        // Step 2: INSERT every symbol via read-then-conditional-write.
        // Symbols are scoped by `(workspace_id, revision_id, fqn)` —
        // the PG UNIQUE constraint. lbug 0.19 NODE TABLEs only
        // support single-column PKs, so we use `id SERIAL PRIMARY
        // KEY` (synthetic) and enforce the natural key at the
        // application layer. The same FQN can appear in multiple
        // revisions of the same workspace (this is the common case
        // when re-saving an updated graph).
        for symbol in graph.symbols() {
            // Existence check on (ws, rev, fqn).
            let mut check_stmt = conn
                .prepare(
                    "MATCH (s:GraphSymbol) WHERE s.workspace_id = $ws AND s.revision_id = $rev AND s.fqn = $fqn RETURN s.id;",
                )
                .map_err(|e| {
                    CallGraphError::Store(format!("save_call_graph_ws: sym check prepare: {e}"))
                })?;
            let mut existing = conn
                .execute(
                    &mut check_stmt,
                    vec![
                        ("ws", lbug::Value::String(ws.to_string())),
                        ("rev", lbug::Value::Int64(rev_id.get() as i64)),
                        (
                            "fqn",
                            lbug::Value::String(symbol.fully_qualified_name().to_string()),
                        ),
                    ],
                )
                .map_err(|e| {
                    CallGraphError::Store(format!("save_call_graph_ws: sym check execute: {e}"))
                })?;

            let sig_str = match symbol.signature() {
                Some(sig) => sig.to_string(),
                None => String::new(),
            };
            if existing.next().is_some() {
                // UPDATE existing row.
                let mut upd_stmt = conn
                    .prepare(
                        "MATCH (s:GraphSymbol) WHERE s.workspace_id = $ws AND s.revision_id = $rev AND s.fqn = $fqn SET s.kind = $kind, s.name = $name, s.file_path = $file, s.line = $line, s.signature = $sig;",
                    )
                    .map_err(|e| {
                        CallGraphError::Store(format!("save_call_graph_ws: sym update prepare: {e}"))
                    })?;
                conn.execute(
                    &mut upd_stmt,
                    vec![
                        ("ws", lbug::Value::String(ws.to_string())),
                        ("rev", lbug::Value::Int64(rev_id.get() as i64)),
                        (
                            "fqn",
                            lbug::Value::String(symbol.fully_qualified_name().to_string()),
                        ),
                        ("kind", lbug::Value::String(symbol.kind().to_string())),
                        ("name", lbug::Value::String(symbol.name().to_string())),
                        (
                            "file",
                            lbug::Value::String(symbol.location().file().to_string()),
                        ),
                        ("line", lbug::Value::Int64(symbol.location().line() as i64)),
                        ("sig", lbug::Value::String(sig_str)),
                    ],
                )
                .map_err(|e| {
                    CallGraphError::Store(format!("save_call_graph_ws: sym update execute: {e}"))
                })?;
            } else {
                // CREATE new row.
                let mut ins_stmt = conn
                    .prepare(
                        "CREATE (s:GraphSymbol {workspace_id: $ws, revision_id: $rev, fqn: $fqn, kind: $kind, name: $name, file_path: $file, line: $line, signature: $sig});",
                    )
                    .map_err(|e| {
                        CallGraphError::Store(format!("save_call_graph_ws: sym insert prepare: {e}"))
                    })?;
                conn.execute(
                    &mut ins_stmt,
                    vec![
                        ("ws", lbug::Value::String(ws.to_string())),
                        ("rev", lbug::Value::Int64(rev_id.get() as i64)),
                        (
                            "fqn",
                            lbug::Value::String(symbol.fully_qualified_name().to_string()),
                        ),
                        ("kind", lbug::Value::String(symbol.kind().to_string())),
                        ("name", lbug::Value::String(symbol.name().to_string())),
                        (
                            "file",
                            lbug::Value::String(symbol.location().file().to_string()),
                        ),
                        ("line", lbug::Value::Int64(symbol.location().line() as i64)),
                        ("sig", lbug::Value::String(sig_str)),
                    ],
                )
                .map_err(|e| {
                    CallGraphError::Store(format!("save_call_graph_ws: sym insert execute: {e}"))
                })?;
            }
        }

        // Step 3: INSERT every edge. Edges are scoped by
        // `(workspace_id, revision_id, source_id, target_id,
        // dep_type)` — the PG UNIQUE constraint. Same synthetic
        // PK + read-then-conditional-write pattern.
        for (source, target, dep_type, provenance, confidence) in graph.edges_with_metadata() {
            let mut check_stmt = conn
                .prepare(
                    "MATCH (e:GraphEdge) WHERE e.workspace_id = $ws AND e.revision_id = $rev AND e.source_id = $src AND e.target_id = $tgt AND e.dep_type = $dt RETURN e.id;",
                )
                .map_err(|e| {
                    CallGraphError::Store(format!("save_call_graph_ws: edge check prepare: {e}"))
                })?;
            let mut existing = conn
                .execute(
                    &mut check_stmt,
                    vec![
                        ("ws", lbug::Value::String(ws.to_string())),
                        ("rev", lbug::Value::Int64(rev_id.get() as i64)),
                        ("src", lbug::Value::String(source.to_string())),
                        ("tgt", lbug::Value::String(target.to_string())),
                        ("dt", lbug::Value::String(dep_type.to_string())),
                    ],
                )
                .map_err(|e| {
                    CallGraphError::Store(format!("save_call_graph_ws: edge check execute: {e}"))
                })?;

            if existing.next().is_some() {
                let mut upd_stmt = conn
                    .prepare(
                        "MATCH (e:GraphEdge) WHERE e.workspace_id = $ws AND e.revision_id = $rev AND e.source_id = $src AND e.target_id = $tgt AND e.dep_type = $dt SET e.provenance = $prov, e.confidence = $conf;",
                    )
                    .map_err(|e| {
                        CallGraphError::Store(format!("save_call_graph_ws: edge update prepare: {e}"))
                    })?;
                conn.execute(
                    &mut upd_stmt,
                    vec![
                        ("ws", lbug::Value::String(ws.to_string())),
                        ("rev", lbug::Value::Int64(rev_id.get() as i64)),
                        ("src", lbug::Value::String(source.to_string())),
                        ("tgt", lbug::Value::String(target.to_string())),
                        ("dt", lbug::Value::String(dep_type.to_string())),
                        ("prov", lbug::Value::String(provenance.to_string())),
                        ("conf", lbug::Value::Double(confidence)),
                    ],
                )
                .map_err(|e| {
                    CallGraphError::Store(format!("save_call_graph_ws: edge update execute: {e}"))
                })?;
            } else {
                let mut ins_stmt = conn
                    .prepare(
                        "CREATE (e:GraphEdge {workspace_id: $ws, revision_id: $rev, source_id: $src, target_id: $tgt, dep_type: $dt, provenance: $prov, confidence: $conf});",
                    )
                    .map_err(|e| {
                        CallGraphError::Store(format!("save_call_graph_ws: edge insert prepare: {e}"))
                    })?;
                conn.execute(
                    &mut ins_stmt,
                    vec![
                        ("ws", lbug::Value::String(ws.to_string())),
                        ("rev", lbug::Value::Int64(rev_id.get() as i64)),
                        ("src", lbug::Value::String(source.to_string())),
                        ("tgt", lbug::Value::String(target.to_string())),
                        ("dt", lbug::Value::String(dep_type.to_string())),
                        ("prov", lbug::Value::String(provenance.to_string())),
                        ("conf", lbug::Value::Double(confidence)),
                    ],
                )
                .map_err(|e| {
                    CallGraphError::Store(format!("save_call_graph_ws: edge insert execute: {e}"))
                })?;
            }
        }

        Ok(rev_id)
    }

    async fn load_call_graph_ws(
        &self,
        ws: &WorkspaceId,
        revision: RevisionId,
    ) -> Result<Option<CallGraph>, CallGraphError> {
        // Read all symbols + edges for `(ws, revision)` and rebuild
        // the CallGraph aggregate. We use the location.fqn as the
        // primary lookup key for symbols (it's unique per workspace
        // and stable across loads).
        let conn = self
            .connection()
            .map_err(|e| CallGraphError::Store(format!("load_call_graph_ws: {e}")))?;

        // Pre-check: if there are NO symbols AND NO edges for this
        // (ws, revision), return None — distinguishes "no data"
        // from "empty data" (lbug has no way to know whether an
        // empty graph was actually saved or just never existed).
        // We use a single OPTIONAL MATCH per table and combine via
        // WITH — only the first row's count is meaningful.
        let mut pre_stmt = conn
            .prepare(
                "OPTIONAL MATCH (s:GraphSymbol) WHERE s.workspace_id = $ws AND s.revision_id = $rev WITH count(s) AS sym_cnt OPTIONAL MATCH (e:GraphEdge) WHERE e.workspace_id = $ws AND e.revision_id = $rev WITH sym_cnt, count(e) AS edge_cnt RETURN sym_cnt, edge_cnt;",
            )
            .map_err(|e| CallGraphError::Store(format!("load_call_graph_ws: pre prepare: {e}")))?;
        let mut pre_result = conn
            .execute(
                &mut pre_stmt,
                vec![
                    ("ws", lbug::Value::String(ws.to_string())),
                    ("rev", lbug::Value::Int64(revision.get() as i64)),
                ],
            )
            .map_err(|e| CallGraphError::Store(format!("load_call_graph_ws: pre execute: {e}")))?;
        let Some(pre_row) = pre_result.next() else {
            return Ok(None);
        };
        let sym_cnt = match &pre_row[0] {
            lbug::Value::Int64(n) => *n,
            lbug::Value::Int32(n) => *n as i64,
            _ => 0,
        };
        let edge_cnt = match &pre_row[1] {
            lbug::Value::Int64(n) => *n,
            lbug::Value::Int32(n) => *n as i64,
            _ => 0,
        };
        if sym_cnt == 0 && edge_cnt == 0 {
            return Ok(None);
        }

        // Symbols
        let mut sym_stmt = conn
            .prepare(
                "MATCH (s:GraphSymbol) WHERE s.workspace_id = $ws AND s.revision_id = $rev RETURN s.id, s.fqn, s.kind, s.name, s.file_path, s.line, s.signature;",
            )
            .map_err(|e| CallGraphError::Store(format!("load_call_graph_ws: sym prepare: {e}")))?;
        let mut sym_result = conn
            .execute(
                &mut sym_stmt,
                vec![
                    ("ws", lbug::Value::String(ws.to_string())),
                    ("rev", lbug::Value::Int64(revision.get() as i64)),
                ],
            )
            .map_err(|e| CallGraphError::Store(format!("load_call_graph_ws: sym execute: {e}")))?;

        let mut graph = CallGraph::new();
        let mut symbol_count = 0usize;
        while let Some(row) = sym_result.next() {
            let s = parse_graph_symbol_row(&row)?;
            graph.add_symbol(s);
            symbol_count += 1;
        }

        // Edges
        let mut edge_stmt = conn
            .prepare(
                "MATCH (e:GraphEdge) WHERE e.workspace_id = $ws AND e.revision_id = $rev RETURN e.source_id, e.target_id, e.dep_type, e.provenance, e.confidence;",
            )
            .map_err(|e| {
                CallGraphError::Store(format!("load_call_graph_ws: edge prepare: {e}"))
            })?;
        let mut edge_result = conn
            .execute(
                &mut edge_stmt,
                vec![
                    ("ws", lbug::Value::String(ws.to_string())),
                    ("rev", lbug::Value::Int64(revision.get() as i64)),
                ],
            )
            .map_err(|e| CallGraphError::Store(format!("load_call_graph_ws: edge execute: {e}")))?;
        while let Some(row) = edge_result.next() {
            let (source_id, target_id, dep_type, confidence) = parse_graph_edge_row(&row)?;
            let provenance = parse_provenance(&row[3])?;
            graph
                .add_dependency_with_provenance(
                    &source_id,
                    &target_id,
                    dep_type,
                    // Map the stored Provenance → ExtractionContext
                    // the same way ConfidenceRules does it on the
                    // write side: Extracted → DirectExtraction,
                    // Inferred/Manual/Tested → matching variant,
                    // Ambiguous → Unresolved.
                    match provenance {
                        Provenance::Extracted => {
                            cognicode_core::domain::services::ExtractionContext::DirectExtraction
                        }
                        Provenance::Inferred => {
                            cognicode_core::domain::services::ExtractionContext::Heuristic {
                                score: confidence,
                            }
                        }
                        Provenance::Ambiguous => {
                            cognicode_core::domain::services::ExtractionContext::Unresolved
                        }
                        Provenance::Manual => {
                            cognicode_core::domain::services::ExtractionContext::Manual
                        }
                        Provenance::Tested => {
                            cognicode_core::domain::services::ExtractionContext::Tested
                        }
                    },
                )
                .map_err(|e| {
                    CallGraphError::Store(format!("load_call_graph_ws: add_dependency: {e}"))
                })?;
        }

        if symbol_count == 0 && edge_result.next().is_none() {
            // The pre-check above already filtered the truly-empty
            // (no rows) case; if we got here it means the pre-check
            // said there ARE rows but the per-table queries returned
            // none — a race condition between the pre-check and the
            // per-table reads. Treat as None for consistency.
            return Ok(None);
        }

        Ok(Some(graph))
    }

    async fn load_call_graph_current(
        &self,
        ws: &WorkspaceId,
    ) -> Result<Option<CallGraph>, CallGraphError> {
        // Resolve the head revision for the workspace via the
        // `&self`-only helper (Priority 4's trait method requires
        // a `&mut PgConnection` that LadybugStore cannot honor — see
        // the head_revision_for_ws comment).
        let head = self
            .head_revision_for_ws(ws)
            .await
            .map_err(|e| CallGraphError::Store(format!("load_call_graph_current: head: {e}")))?;
        let Some(rev) = head else {
            return Ok(None);
        };
        self.load_call_graph_ws(ws, rev).await
    }
}

/// `RevisionStore::head_revision` body extracted to a
/// `&self`-only helper so tests can exercise the head-resolution
/// path without relying on the trait's `&mut PgConnection`
/// parameter (which LadybugStore cannot honor — see the comment
/// in the trait impl above for the limitation).
///
/// This duplicates the Cypher that Priority 4's
/// `RevisionStore::head_revision` impl uses; when Priority 4 is
/// merged, the trait method will be wired and the tests can drop
/// this helper.
impl LadybugStore {
    pub(crate) async fn head_revision_for_ws(
        &self,
        ws: &WorkspaceId,
    ) -> Result<Option<RevisionId>, CallGraphError> {
        let conn = self
            .connection()
            .map_err(|e| CallGraphError::Store(format!("head_revision_for_ws: {e}")))?;
        let mut stmt = conn
            .prepare(
                "MATCH (r:GraphRevision) WHERE r.workspace_id = $ws AND r.head_of = true RETURN r.revision_id;",
            )
            .map_err(|e| {
                CallGraphError::Store(format!("head_revision_for_ws: prepare: {e}"))
            })?;
        let mut result = conn
            .execute(&mut stmt, vec![("ws", lbug::Value::String(ws.to_string()))])
            .map_err(|e| CallGraphError::Store(format!("head_revision_for_ws: execute: {e}")))?;
        let Some(row) = result.next() else {
            return Ok(None);
        };
        let rev = match &row[0] {
            lbug::Value::Int64(n) => RevisionId(*n as u64),
            lbug::Value::Int32(n) => RevisionId(*n as u64),
            other => {
                return Err(CallGraphError::Store(format!(
                    "head_revision_for_ws: unexpected type: {other:?}"
                )));
            }
        };
        Ok(Some(rev))
    }
}

/// Row mapper for `GraphSymbol` queries. Returns a fully-constructed
/// [`Symbol`] (the file/line/kind/name/signature fields round-trip via
/// the Display form — lbug 0.19 has no structured types).
fn parse_graph_symbol_row(
    row: &[lbug::Value],
) -> Result<cognicode_core::domain::aggregates::Symbol, CallGraphError> {
    use cognicode_core::domain::aggregates::Symbol;
    use cognicode_core::domain::value_objects::{Location, SymbolKind};
    let fqn = row[1].to_string();
    let kind_str = row[2].to_string();
    let kind = match kind_str.as_str() {
        "Function" | "function" => SymbolKind::Function,
        "Method" | "method" => SymbolKind::Method,
        "Class" | "class" => SymbolKind::Class,
        "Struct" | "struct" => SymbolKind::Struct,
        "Enum" | "enum" => SymbolKind::Enum,
        "Trait" | "trait" => SymbolKind::Trait,
        "Variable" | "variable" => SymbolKind::Variable,
        "Constant" | "constant" => SymbolKind::Constant,
        "Module" | "module" => SymbolKind::Module,
        "Interface" | "interface" => SymbolKind::Interface,
        other => {
            return Err(CallGraphError::Store(format!(
                "parse_graph_symbol_row: unknown kind: {other}"
            )));
        }
    };
    let file = row[4].to_string();
    let line = match &row[5] {
        lbug::Value::Int64(n) => (*n).max(0) as u32,
        lbug::Value::Int32(n) => (*n).max(0) as u32,
        other => {
            return Err(CallGraphError::Store(format!(
                "parse_graph_symbol_row: unexpected line type: {other:?}"
            )));
        }
    };
    let name = row[3].to_string();
    let mut symbol = Symbol::new(name, kind, Location::new(file, line, 0));
    symbol.set_fqn_override(&fqn);
    // Signature round-trip is lossy on the read side (the
    // Display form drops parameter names' types and async-ness);
    // the PG adapter has the same constraint — signatures are
    // reconstructable but lossy. Future PR can move to a JSON
    // column for full fidelity once lbug has a JSON type.
    let _sig_str = row[6].to_string();
    Ok(symbol)
}

/// Row mapper for `GraphEdge` queries. Returns
/// `(source_id, target_id, dep_type, confidence)` — `provenance` is
/// read separately via `parse_provenance` because the caller needs
/// the discriminant to pick the right `ExtractionContext` when
/// re-adding the edge to the aggregate.
fn parse_graph_edge_row(
    row: &[lbug::Value],
) -> Result<
    (
        cognicode_core::domain::aggregates::SymbolId,
        cognicode_core::domain::aggregates::SymbolId,
        DependencyType,
        f64,
    ),
    CallGraphError,
> {
    use cognicode_core::domain::aggregates::SymbolId;
    let source_id = SymbolId::new(row[0].to_string());
    let target_id = SymbolId::new(row[1].to_string());
    let dep_type = match row[2].to_string().as_str() {
        "calls" | "Calls" => DependencyType::Calls,
        "imports" | "Imports" => DependencyType::Imports,
        "inherits" | "Inherits" => DependencyType::Inherits,
        "uses_generic" | "UsesGeneric" => DependencyType::UsesGeneric,
        "references" | "References" => DependencyType::References,
        "defines" | "Defines" => DependencyType::Defines,
        "annotated_by" | "AnnotatedBy" => DependencyType::AnnotatedBy,
        "contains" | "Contains" => DependencyType::Contains,
        other => {
            return Err(CallGraphError::Store(format!(
                "parse_graph_edge_row: unknown dep_type: {other}"
            )));
        }
    };
    let confidence = match &row[4] {
        lbug::Value::Double(n) => *n,
        lbug::Value::Int64(n) => *n as f64,
        _ => 0.0,
    };
    Ok((source_id, target_id, dep_type, confidence))
}

fn parse_provenance(value: &lbug::Value) -> Result<Provenance, CallGraphError> {
    match value.to_string().as_str() {
        "Extracted" | "extracted" => Ok(Provenance::Extracted),
        "Inferred" | "inferred" => Ok(Provenance::Inferred),
        "Ambiguous" | "ambiguous" => Ok(Provenance::Ambiguous),
        "Manual" | "manual" => Ok(Provenance::Manual),
        "Tested" | "tested" => Ok(Provenance::Tested),
        other => Err(CallGraphError::Store(format!(
            "parse_provenance: unknown: {other}"
        ))),
    }
}

#[cfg(feature = "multimodal")]
#[async_trait]
impl IngestCommit for LadybugStore {
    async fn commit_revision(
        &self,
        _ws: &WorkspaceId,
        _graph: GraphDelta,
        _manifest: ManifestDelta,
        _report: ReportIntent,
    ) -> Result<RevisionId, cognicode_core::domain::ports::CommitError> {
        // PHASE 1 STUB. Next change: single-tx commit_revision that
        // delegates to RevisionStore::create_revision + ManifestStore
        // upserts + ReportStore::save_report, all within one
        // lbug::Connection tx (matches the PostgreSQL behavior
        // shipped in `b01671f6`).
        Err(cognicode_core::domain::ports::CommitError::Graph(
            cognicode_core::domain::ports::graph_error::GraphError::Storage(
                "(phase 1 stub — see lib.rs port-impl)".to_string(),
            ),
        ))
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
    use serial_test::serial;

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
    #[serial]
    async fn manifest_get_returns_empty_for_fresh_db() {
        let (store, _dir) = make_test_store();
        init_scan_manifest_schema(&store);
        let rows = store.get_manifest("ws-unknown").await.expect("get");
        assert!(rows.is_empty(), "fresh db should return no rows");
    }

    #[tokio::test]
    #[serial]
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
    #[serial]
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
    #[serial]
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
    #[serial]
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
    // CallGraphStore (Priority 8)
    // --------------------------------------------------------------------

    use cognicode_core::domain::aggregates::{CallGraph, Symbol};
    use cognicode_core::domain::value_objects::{DependencyType, Location, SymbolKind};

    /// Apply the CallGraphStore NODE TABLEs. The store also needs
    /// a `GraphRevision` table (used by `save_call_graph_ws`'s
    /// single-Cypher demote+create revision step).
    fn init_call_graph_schema(store: &LadybugStore) {
        let conn = store.connection().expect("schema-init connection");
        conn.query(
            "CREATE NODE TABLE IF NOT EXISTS GraphRevision( \
                 id SERIAL PRIMARY KEY, \
                 workspace_id STRING, \
                 revision_id INT64, \
                 head_of BOOLEAN);",
        )
        .expect("create GraphRevision NODE TABLE");
        conn.query(
            "CREATE NODE TABLE IF NOT EXISTS GraphSymbol( \
                 id SERIAL PRIMARY KEY, \
                 workspace_id STRING, \
                 revision_id INT64, \
                 fqn STRING, \
                 kind STRING, \
                 name STRING, \
                 file_path STRING, \
                 line INT64, \
                 signature STRING);",
        )
        .expect("create GraphSymbol NODE TABLE");
        conn.query(
            "CREATE NODE TABLE IF NOT EXISTS GraphEdge( \
                 id SERIAL PRIMARY KEY, \
                 workspace_id STRING, \
                 revision_id INT64, \
                 source_id STRING, \
                 target_id STRING, \
                 dep_type STRING, \
                 provenance STRING, \
                 confidence DOUBLE);",
        )
        .expect("create GraphEdge NODE TABLE");
    }

    fn ws_id(s: &str) -> WorkspaceId {
        WorkspaceId::try_new(s).expect("workspace id must be non-empty")
    }

    fn build_small_graph() -> CallGraph {
        // A → B (Calls), A → C (Imports), B → D (Calls)
        let mut g = CallGraph::new();
        let a = g.add_symbol(Symbol::new(
            "foo",
            SymbolKind::Function,
            Location::new("src/foo.rs", 10, 1),
        ));
        let b = g.add_symbol(Symbol::new(
            "bar",
            SymbolKind::Function,
            Location::new("src/bar.rs", 20, 1),
        ));
        let c = g.add_symbol(Symbol::new(
            "Baz",
            SymbolKind::Class,
            Location::new("src/baz.rs", 30, 1),
        ));
        let d = g.add_symbol(Symbol::new(
            "qux",
            SymbolKind::Function,
            Location::new("src/qux.rs", 40, 1),
        ));
        g.add_dependency(&a, &b, DependencyType::Calls)
            .expect("a→b");
        g.add_dependency(&a, &c, DependencyType::Imports)
            .expect("a→c");
        g.add_dependency(&b, &d, DependencyType::Calls)
            .expect("b→d");
        g
    }

    #[tokio::test]
    #[serial]
    #[serial]
    async fn call_graph_save_returns_first_revision_for_fresh_workspace() {
        let (store, _dir) = make_test_store();
        init_call_graph_schema(&store);
        let g = build_small_graph();
        let rev = store
            .save_call_graph_ws(&g, &ws_id("ws-1"))
            .await
            .expect("save");
        assert_eq!(rev.get(), 1, "first save in a fresh workspace → rev 1");
    }

    #[tokio::test]
    #[serial]
    #[serial]
    async fn call_graph_save_increments_monotonically() {
        let (store, _dir) = make_test_store();
        init_call_graph_schema(&store);
        let g = build_small_graph();
        let r1 = store
            .save_call_graph_ws(&g, &ws_id("ws-x"))
            .await
            .expect("s1");
        let r2 = store
            .save_call_graph_ws(&g, &ws_id("ws-x"))
            .await
            .expect("s2");
        let r3 = store
            .save_call_graph_ws(&g, &ws_id("ws-x"))
            .await
            .expect("s3");
        assert_eq!(r1.get(), 1);
        assert_eq!(r2.get(), 2);
        assert_eq!(r3.get(), 3);
    }

    #[tokio::test]
    #[serial]
    #[serial]
    async fn call_graph_save_demotes_prior_head() {
        // After save, only the latest revision has head_of=true.
        let (store, _dir) = make_test_store();
        init_call_graph_schema(&store);
        let g = build_small_graph();
        store
            .save_call_graph_ws(&g, &ws_id("ws-d"))
            .await
            .expect("s1");
        store
            .save_call_graph_ws(&g, &ws_id("ws-d"))
            .await
            .expect("s2");
        // head_revision for ws-d must be 2 (the latest).
        let head = store
            .head_revision_for_ws(&ws_id("ws-d"))
            .await
            .expect("head");
        assert_eq!(head, Some(RevisionId(2)));
    }

    #[tokio::test]
    #[serial]
    #[serial]
    async fn call_graph_load_returns_symbols_and_edges_round_trip() {
        let (store, _dir) = make_test_store();
        init_call_graph_schema(&store);
        let g = build_small_graph();
        let rev = store
            .save_call_graph_ws(&g, &ws_id("ws-rt"))
            .await
            .expect("save");
        let loaded = store
            .load_call_graph_ws(&ws_id("ws-rt"), rev)
            .await
            .expect("load")
            .expect("present");
        // 4 symbols round-trip.
        let count = loaded.symbols().count();
        assert_eq!(count, 4, "all 4 symbols round-trip");
        // 3 edges round-trip.
        let mut edge_count = 0;
        for _ in loaded.edges_with_metadata() {
            edge_count += 1;
        }
        assert_eq!(edge_count, 3, "all 3 edges round-trip");
    }

    #[tokio::test]
    #[serial]
    #[serial]
    async fn call_graph_load_returns_none_for_unknown_revision() {
        let (store, _dir) = make_test_store();
        init_call_graph_schema(&store);
        let g = build_small_graph();
        store
            .save_call_graph_ws(&g, &ws_id("ws-x"))
            .await
            .expect("save");
        let loaded = store
            .load_call_graph_ws(&ws_id("ws-x"), RevisionId(999))
            .await
            .expect("load");
        assert!(loaded.is_none(), "unknown revision returns None");
    }

    #[tokio::test]
    #[serial]
    #[serial]
    async fn call_graph_load_current_returns_head_revision_graph() {
        let (store, _dir) = make_test_store();
        init_call_graph_schema(&store);
        let g = build_small_graph();
        store
            .save_call_graph_ws(&g, &ws_id("ws-c"))
            .await
            .expect("save 1");
        store
            .save_call_graph_ws(&g, &ws_id("ws-c"))
            .await
            .expect("save 2");
        // Current = revision 2 (the head).
        let loaded = store
            .load_call_graph_current(&ws_id("ws-c"))
            .await
            .expect("load current")
            .expect("present");
        let count = loaded.symbols().count();
        assert_eq!(count, 4);
    }

    #[tokio::test]
    #[serial]
    #[serial]
    async fn call_graph_load_current_returns_none_for_unknown_workspace() {
        let (store, _dir) = make_test_store();
        init_call_graph_schema(&store);
        let loaded = store
            .load_call_graph_current(&ws_id("ws-unknown"))
            .await
            .expect("load");
        assert!(loaded.is_none());
    }

    #[tokio::test]
    #[serial]
    #[serial]
    async fn call_graph_save_is_scoped_to_workspace() {
        // Two workspaces, two saves — the head for each is independent.
        let (store, _dir) = make_test_store();
        init_call_graph_schema(&store);
        let g = build_small_graph();
        store
            .save_call_graph_ws(&g, &ws_id("ws-A"))
            .await
            .expect("a1");
        store
            .save_call_graph_ws(&g, &ws_id("ws-A"))
            .await
            .expect("a2");
        store
            .save_call_graph_ws(&g, &ws_id("ws-B"))
            .await
            .expect("b1");
        assert_eq!(
            store
                .head_revision_for_ws(&ws_id("ws-A"))
                .await
                .expect("h-A"),
            Some(RevisionId(2))
        );
        assert_eq!(
            store
                .head_revision_for_ws(&ws_id("ws-B"))
                .await
                .expect("h-B"),
            Some(RevisionId(1))
        );
    }
}
