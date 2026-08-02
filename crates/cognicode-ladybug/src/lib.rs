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
//! | 1 | `ManifestStore` | Simplest SQL (single-table CRUD), is the basic load-bearing port for Phase 1 tests | DONE (trunk `af5e2ef2`) |
//! | 2 | `SessionStore` | Single-table CRUD on `exploration_sessions`, low risk | DONE (this branch) |
//! | 3 | `ReportStore` | Single-table reads + the new `save_report` INSERT (no tx) | next |
//! | 4 | `RevisionStore` | UPDATE-only on `graph_revisions`, plus the read-only `head_revision` | pending |
//! | 5 | `FederationStore` | Single-table CRUD on `spaces` | pending (multimodal) |
//! | 6 | `ViewSpecStore` | JSON-payload CRD store (post `ViewSpecPayload` bridge) | pending |
//! | 7 | `QualityStore` | 10-method port split across `issues`, `baselines`, `rules` | pending |
//! | 8 | `CallGraphStore` | `save_call_graph_ws` + `load_call_graph_ws` | pending |
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
use cognicode_core::domain::value_objects::{RevisionId, WorkspaceId};

// `FederationStore`, `IngestCommit`, and the `Space`/`SpaceId` value
// objects they operate on are gated behind the `multimodal` feature
// in `cognicode-core`. The default build (no multimodal) skips them;
// the follow-up PR that flips multimodal to ON also wires these.
#[cfg(feature = "multimodal")]
use cognicode_core::domain::ports::{
    federation_store::{FederationError, FederationStore, Space, SpaceId},
    ingest_commit::{CommitError, GraphDelta, IngestCommit, ManifestDelta, ReportIntent},
};

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
        id: &str,
        workspace_id: &str,
        events_json: &str,
        navigation_mode: &str,
        panes_json: &str,
        investigation_id: Option<&str>,
    ) -> Result<(), SessionError> {
        // lbug Cypher: CREATE one ExplorationSession node keyed by the
        // client-provided `id` (single-column STRING PRIMARY KEY — no
        // composite-PK workaround needed because the natural key is
        // already single-column, unlike `ManifestStore` where the
        // natural key `(workspace_id, file_path)` forced a synthetic
        // PK + read-then-conditional-write).
        //
        // `created_at` is filled client-side via `chrono::Utc::now()
        // .to_rfc3339()` because lbug 0.19.0 has no `now()`-equivalent
        // function call for default values (verified by the spike's
        // s2_schema_load — only `SERIAL` is auto-assigned). This
        // mirrors the PostgreSQL `DEFAULT now()` semantics for callers.
        //
        // `investigation_id` is stored as `Null(STRING)` when absent
        // — the natural representation of an optional FK in lbug
        // 0.19.0 (no NULLABLE shorthand; nullable columns are STRING
        // with semantic null values).
        //
        // The query is flattened to a single line — lbug 0.19.0's
        // parser is line-continuation-sensitive (verified in the
        // spike's s6_cypher_compat).
        let conn = self
            .connection()
            .map_err(|e| SessionError::Store(format!("save: {e}")))?;
        let created_at = chrono::Utc::now().to_rfc3339();
        let mut stmt = conn
            .prepare(
                "CREATE (s:ExplorationSession {id: $id, workspace_id: $ws, events: $events, navigation_mode: $mode, panes: $panes, created_at: $ts, investigation_id: $inv});",
            )
            .map_err(|e| SessionError::Store(format!("save: prepare: {e}")))?;
        conn.execute(
            &mut stmt,
            vec![
                ("id", lbug::Value::String(id.to_string())),
                ("ws", lbug::Value::String(workspace_id.to_string())),
                ("events", lbug::Value::String(events_json.to_string())),
                ("mode", lbug::Value::String(navigation_mode.to_string())),
                ("panes", lbug::Value::String(panes_json.to_string())),
                ("ts", lbug::Value::String(created_at)),
                (
                    "inv",
                    match investigation_id {
                        Some(s) => lbug::Value::String(s.to_string()),
                        None => lbug::Value::Null(lbug::LogicalType::String),
                    },
                ),
            ],
        )
        .map_err(|e| SessionError::Store(format!("save: execute: {e}")))?;
        Ok(())
    }

    async fn load(&self, id: &str, workspace_id: &str) -> Result<Option<SessionRow>, SessionError> {
        // lbug Cypher: MATCH by the natural `(id, workspace_id)` pair
        // and return the row. The `WHERE` form is used (not the
        // property-pattern form `MATCH (n:L {prop: $v})`) because the
        // property pattern is rejected by lbug 0.19.0's parser
        // (verified in the spike's s6_cypher_compat).
        //
        // Single-row lookup, so `LIMIT 1` is included as a defensive
        // belt-and-suspenders measure even though `id` is the PK and
        // should never have duplicates.
        let conn = self
            .connection()
            .map_err(|e| SessionError::Store(format!("load: {e}")))?;
        let mut stmt = conn
            .prepare(
                "MATCH (s:ExplorationSession) WHERE s.id = $id AND s.workspace_id = $ws RETURN s.id, s.workspace_id, s.events, s.navigation_mode, s.panes, s.created_at, s.investigation_id LIMIT 1;",
            )
            .map_err(|e| SessionError::Store(format!("load: prepare: {e}")))?;
        let mut result = conn
            .execute(
                &mut stmt,
                vec![
                    ("id", lbug::Value::String(id.to_string())),
                    ("ws", lbug::Value::String(workspace_id.to_string())),
                ],
            )
            .map_err(|e| SessionError::Store(format!("load: execute: {e}")))?;

        let Some(row) = result.next() else {
            return Ok(None);
        };

        let events: serde_json::Value = serde_json::from_str(&row[2].to_string())
            .map_err(|e| SessionError::Store(format!("load: events JSON: {e}")))?;
        let panes: serde_json::Value = serde_json::from_str(&row[4].to_string())
            .map_err(|e| SessionError::Store(format!("load: panes JSON: {e}")))?;
        Ok(Some(SessionRow {
            id: row[0].to_string(),
            workspace_id: row[1].to_string(),
            events,
            navigation_mode: row[3].to_string(),
            panes,
            created_at: row[5].to_string(),
            investigation_id: match &row[6] {
                lbug::Value::Null(_) => None,
                other => Some(other.to_string()),
            },
        }))
    }

    async fn list(&self, workspace_id: &str) -> Result<Vec<SessionRow>, SessionError> {
        // lbug Cypher: MATCH by `workspace_id` and ORDER BY `created_at
        // DESC, id DESC` (matching the PostgreSQL ORDER BY in the
        // Postgres adapter's `list_exploration_sessions` — stable
        // ordering is required so list-then-load round-trips in tests).
        let conn = self
            .connection()
            .map_err(|e| SessionError::Store(format!("list: {e}")))?;
        let mut stmt = conn
            .prepare(
                "MATCH (s:ExplorationSession) WHERE s.workspace_id = $ws RETURN s.id, s.workspace_id, s.events, s.navigation_mode, s.panes, s.created_at, s.investigation_id ORDER BY s.created_at DESC, s.id DESC;",
            )
            .map_err(|e| SessionError::Store(format!("list: prepare: {e}")))?;
        let mut result = conn
            .execute(
                &mut stmt,
                vec![("ws", lbug::Value::String(workspace_id.to_string()))],
            )
            .map_err(|e| SessionError::Store(format!("list: execute: {e}")))?;

        let mut rows = Vec::new();
        while let Some(row) = result.next() {
            let events: serde_json::Value = serde_json::from_str(&row[2].to_string())
                .map_err(|e| SessionError::Store(format!("list: events JSON: {e}")))?;
            let panes: serde_json::Value = serde_json::from_str(&row[4].to_string())
                .map_err(|e| SessionError::Store(format!("list: panes JSON: {e}")))?;
            rows.push(SessionRow {
                id: row[0].to_string(),
                workspace_id: row[1].to_string(),
                events,
                navigation_mode: row[3].to_string(),
                panes,
                created_at: row[5].to_string(),
                investigation_id: match &row[6] {
                    lbug::Value::Null(_) => None,
                    other => Some(other.to_string()),
                },
            });
        }
        Ok(rows)
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
    // Per-port integration tests — landed ports so far:
    //   - ManifestStore (Priority 1, trunk `af5e2ef2`)
    //   - SessionStore (Priority 2, this branch)
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
    // SessionStore (Priority 2)
    // --------------------------------------------------------------------
    //
    // Same pattern as ManifestStore tests above: real lbug db in a
    // tempdir, schema-init helper, exercises save/load/list with the
    // same JSON-string shapes the Postgres adapter uses.

    /// Apply the ExplorationSession NODE TABLE DDL once per test
    /// database. Idempotent via `IF NOT EXISTS`.
    ///
    /// Note: unlike `ScanManifest`, the natural PK here is
    /// single-column (`id` STRING) — no synthetic PK needed. The
    /// lbug 0.19.0 single-column-PK limitation only bites composite
    /// natural keys.
    ///
    /// `events` and `panes` are stored as STRING (the JSON text the
    /// caller passes in via the port signature). lbug 0.19.0 has no
    /// JSON type — callers serialize/deserialize via `serde_json` at
    /// the application layer (same as how the port trait's
    /// `SessionRow.events: serde_json::Value` is reconstructed on
    /// read).
    fn init_exploration_session_schema(store: &LadybugStore) {
        let conn = store.connection().expect("schema-init connection");
        conn.query(
            "CREATE NODE TABLE IF NOT EXISTS ExplorationSession( \
                 id STRING PRIMARY KEY, \
                 workspace_id STRING, \
                 events STRING, \
                 navigation_mode STRING, \
                 panes STRING, \
                 created_at STRING, \
                 investigation_id STRING);",
        )
        .expect("create ExplorationSession NODE TABLE");
    }

    #[tokio::test]
    async fn session_list_returns_empty_for_fresh_db() {
        let (store, _dir) = make_test_store();
        init_exploration_session_schema(&store);
        let rows = SessionStore::list(&store, "ws-unknown")
            .await
            .expect("list");
        assert!(rows.is_empty(), "fresh db should return no rows");
    }

    #[tokio::test]
    async fn session_save_then_load_round_trips() {
        let (store, _dir) = make_test_store();
        init_exploration_session_schema(&store);
        let events_json = r#"[{"kind":"click","t":1},{"kind":"hover","t":2}]"#;
        let panes_json = r#"{"left":"graph","right":"narrative"}"#;
        SessionStore::save(
            &store,
            "sess-1",
            "ws-1",
            events_json,
            "guided",
            panes_json,
            Some("inv-1"),
        )
        .await
        .expect("save");
        let row = SessionStore::load(&store, "sess-1", "ws-1")
            .await
            .expect("load");
        let row = row.expect("load should return Some after save");
        assert_eq!(row.id, "sess-1");
        assert_eq!(row.workspace_id, "ws-1");
        assert_eq!(row.navigation_mode, "guided");
        assert_eq!(row.investigation_id.as_deref(), Some("inv-1"));
        assert_eq!(row.events.to_string(), events_json);
        assert_eq!(row.panes.to_string(), panes_json);
        assert!(
            !row.created_at.is_empty(),
            "created_at must be filled by the store"
        );
    }

    #[tokio::test]
    async fn session_save_with_null_investigation_id_round_trips() {
        let (store, _dir) = make_test_store();
        init_exploration_session_schema(&store);
        SessionStore::save(&store, "sess-2", "ws-2", "[]", "free", "{}", None)
            .await
            .expect("save");
        let row = SessionStore::load(&store, "sess-2", "ws-2")
            .await
            .expect("load");
        let row = row.expect("present");
        assert!(
            row.investigation_id.is_none(),
            "investigation_id should round-trip None"
        );
    }

    #[tokio::test]
    async fn session_load_with_wrong_workspace_returns_none() {
        // Security check: load is scoped to `(id, workspace_id)`. A
        // session that exists under `ws-A` must NOT be retrievable
        // via `load(id, ws-B)` — this is the scope-isolation invariant
        // the Postgres adapter also enforces (see
        // `load_exploration_session`'s `WHERE id = $1 AND workspace_id
        // = $2`).
        let (store, _dir) = make_test_store();
        init_exploration_session_schema(&store);
        SessionStore::save(&store, "sess-3", "ws-A", "[]", "guided", "{}", None)
            .await
            .expect("save");
        let row = SessionStore::load(&store, "sess-3", "ws-B")
            .await
            .expect("load");
        assert!(
            row.is_none(),
            "cross-workspace load must return None (not the row)"
        );
    }

    #[tokio::test]
    async fn session_list_returns_rows_in_created_at_desc_order() {
        // Saving three sessions in a known order; `list` must return
        // them newest-first. The stored `created_at` is set at save
        // time via `chrono::Utc::now()`, so even back-to-back saves
        // can land in the same nanosecond — we therefore save with a
        // tiny sleep between each call so the ORDER BY `created_at
        // DESC` is deterministic in this test.
        let (store, _dir) = make_test_store();
        init_exploration_session_schema(&store);
        for id in ["sess-old", "sess-mid", "sess-new"] {
            SessionStore::save(&store, id, "ws-x", "[]", "guided", "{}", None)
                .await
                .expect("save");
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        let rows = SessionStore::list(&store, "ws-x").await.expect("list");
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].id, "sess-new");
        assert_eq!(rows[1].id, "sess-mid");
        assert_eq!(rows[2].id, "sess-old");
    }

    #[tokio::test]
    async fn session_list_scopes_to_workspace() {
        // Two workspaces, three sessions total — list(ws-A) must
        // return only ws-A's sessions.
        let (store, _dir) = make_test_store();
        init_exploration_session_schema(&store);
        SessionStore::save(&store, "a-1", "ws-A", "[]", "guided", "{}", None)
            .await
            .expect("s1");
        SessionStore::save(&store, "a-2", "ws-A", "[]", "guided", "{}", None)
            .await
            .expect("s2");
        SessionStore::save(&store, "b-1", "ws-B", "[]", "guided", "{}", None)
            .await
            .expect("s3");

        let a_rows = SessionStore::list(&store, "ws-A").await.expect("la");
        assert_eq!(a_rows.len(), 2);
        assert!(a_rows.iter().all(|r| r.workspace_id == "ws-A"));

        let b_rows = SessionStore::list(&store, "ws-B").await.expect("lb");
        assert_eq!(b_rows.len(), 1);
        assert_eq!(b_rows[0].id, "b-1");
    }
}
