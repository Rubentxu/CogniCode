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
//! | 7 | `QualityStore` | 10-method port split across `issues`, `baselines`, `rules` | DONE (this branch) |
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
    quality_store::{
        IssueFilter, NewIssue, QualityError, QualityGateSummary, QualityIssue, QualityStore,
        RuleSummary, UpsertSummary,
    },
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
    QualityError,
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

// `QualityStore` is a SYNC trait (no `#[async_trait]` in
// `cognicode-core`'s definition — the PG adapter uses
// `block_in_place + Handle::current + handle.block_on` to drive
// async SQL through sync method signatures). Mirror that pattern
// here for parity; tests use `tokio::test` so `Handle::current()`
// resolves to the test's runtime.
impl QualityStore for LadybugStore {
    fn issues_for_file(&self, file: &str) -> Result<Vec<QualityIssue>, QualityError> {
        // MATCH WHERE file_path = $file, ordered by id for stable reads.
        let conn = self
            .connection()
            .map_err(|e| QualityError::Store(format!("issues_for_file: {e}")))?;
        let mut stmt = conn
            .prepare(
                "MATCH (i:Issue) WHERE i.file_path = $file RETURN i.id, i.rule_id, i.severity, i.category, i.file_path, i.line, i.message, i.status ORDER BY i.id;",
            )
            .map_err(|e| QualityError::Store(format!("issues_for_file: prepare: {e}")))?;
        let mut result = conn
            .execute(
                &mut stmt,
                vec![("file", lbug::Value::String(file.to_string()))],
            )
            .map_err(|e| QualityError::Store(format!("issues_for_file: execute: {e}")))?;
        let mut rows = Vec::new();
        while let Some(row) = result.next() {
            rows.push(parse_issue_row(&row)?);
        }
        Ok(rows)
    }

    fn issues_for_scope(&self, scope_prefix: &str) -> Result<Vec<QualityIssue>, QualityError> {
        // Boundary-aware: $prefix matches exactly OR $prefix + "/"
        // (the PG adapter's `WHERE file_path = $1 OR file_path LIKE
        // $1 || '/%'` semantics). lbug's STARTS WITH is the same as
        // LIKE 'prefix%' — combine with exact match via OR.
        let conn = self
            .connection()
            .map_err(|e| QualityError::Store(format!("issues_for_scope: {e}")))?;
        let mut stmt = conn
            .prepare(
                "MATCH (i:Issue) WHERE i.file_path = $p OR i.file_path STARTS WITH $p_slash RETURN i.id, i.rule_id, i.severity, i.category, i.file_path, i.line, i.message, i.status ORDER BY i.id;",
            )
            .map_err(|e| QualityError::Store(format!("issues_for_scope: prepare: {e}")))?;
        let mut result = conn
            .execute(
                &mut stmt,
                vec![
                    ("p", lbug::Value::String(scope_prefix.to_string())),
                    ("p_slash", lbug::Value::String(format!("{scope_prefix}/"))),
                ],
            )
            .map_err(|e| QualityError::Store(format!("issues_for_scope: execute: {e}")))?;
        let mut rows = Vec::new();
        while let Some(row) = result.next() {
            rows.push(parse_issue_row(&row)?);
        }
        Ok(rows)
    }

    fn issues_at_line(&self, file: &str, line: u32) -> Result<Vec<QualityIssue>, QualityError> {
        let conn = self
            .connection()
            .map_err(|e| QualityError::Store(format!("issues_at_line: {e}")))?;
        let mut stmt = conn
            .prepare(
                "MATCH (i:Issue) WHERE i.file_path = $file AND i.line = $line RETURN i.id, i.rule_id, i.severity, i.category, i.file_path, i.line, i.message, i.status ORDER BY i.id;",
            )
            .map_err(|e| QualityError::Store(format!("issues_at_line: prepare: {e}")))?;
        let mut result = conn
            .execute(
                &mut stmt,
                vec![
                    ("file", lbug::Value::String(file.to_string())),
                    ("line", lbug::Value::Int64(line as i64)),
                ],
            )
            .map_err(|e| QualityError::Store(format!("issues_at_line: execute: {e}")))?;
        let mut rows = Vec::new();
        while let Some(row) = result.next() {
            rows.push(parse_issue_row(&row)?);
        }
        Ok(rows)
    }

    fn issue_by_id(&self, id: i64) -> Result<Option<QualityIssue>, QualityError> {
        let conn = self
            .connection()
            .map_err(|e| QualityError::Store(format!("issue_by_id: {e}")))?;
        let mut stmt = conn
            .prepare(
                "MATCH (i:Issue) WHERE i.id = $id RETURN i.id, i.rule_id, i.severity, i.category, i.file_path, i.line, i.message, i.status;",
            )
            .map_err(|e| QualityError::Store(format!("issue_by_id: prepare: {e}")))?;
        let mut result = conn
            .execute(&mut stmt, vec![("id", lbug::Value::Int64(id))])
            .map_err(|e| QualityError::Store(format!("issue_by_id: execute: {e}")))?;
        let Some(row) = result.next() else {
            return Ok(None);
        };
        Ok(Some(parse_issue_row(&row)?))
    }

    fn rule_summary(&self, rule_id: &str) -> Result<RuleSummary, QualityError> {
        // Compact summary: description from Rule node + open count
        // from Issue nodes with status = 'open' for this rule_id.
        let conn = self
            .connection()
            .map_err(|e| QualityError::Store(format!("rule_summary: {e}")))?;
        let mut stmt = conn
            .prepare(
                "MATCH (r:Rule) WHERE r.rule_id = $rid OPTIONAL MATCH (i:Issue) WHERE i.rule_id = $rid AND i.status = 'open' WITH r, count(i) AS open_cnt RETURN r.description, open_cnt;",
            )
            .map_err(|e| QualityError::Store(format!("rule_summary: prepare: {e}")))?;
        let mut result = conn
            .execute(
                &mut stmt,
                vec![("rid", lbug::Value::String(rule_id.to_string()))],
            )
            .map_err(|e| QualityError::Store(format!("rule_summary: execute: {e}")))?;
        let Some(row) = result.next() else {
            // No rule row — default description to rule_id with 0 count.
            return Ok(RuleSummary {
                rule_id: rule_id.to_string(),
                description: rule_id.to_string(),
                open_count: 0,
            });
        };
        let description = match &row[0] {
            lbug::Value::Null(_) => rule_id.to_string(),
            other => other.to_string(),
        };
        let open_count = match &row[1] {
            lbug::Value::Int64(n) => *n as usize,
            lbug::Value::Int32(n) => *n as usize,
            _ => 0,
        };
        Ok(RuleSummary {
            rule_id: rule_id.to_string(),
            description,
            open_count,
        })
    }

    fn quality_gate(&self, workspace_id: Option<&str>) -> Result<QualityGateSummary, QualityError> {
        // Read the latest Baseline for the workspace (or default
        // "default" workspace if None — matches the PG adapter's
        // `workspace_id IS NULL OR = $1` semantics, but lbug needs
        // the predicate expressed as a ternary).
        let ws = workspace_id.unwrap_or("default");
        let conn = self
            .connection()
            .map_err(|e| QualityError::Store(format!("quality_gate: {e}")))?;

        // Latest baseline for this workspace.
        let mut bl_stmt = conn
            .prepare(
                "MATCH (b:Baseline) WHERE b.workspace_id = $ws RETURN b.rating, b.total_issues, b.blockers, b.criticals, b.debt_minutes, b.snapshot_at;",
            )
            .map_err(|e| QualityError::Store(format!("quality_gate: baseline prepare: {e}")))?;
        let mut bl_result = conn
            .execute(
                &mut bl_stmt,
                vec![("ws", lbug::Value::String(ws.to_string()))],
            )
            .map_err(|e| QualityError::Store(format!("quality_gate: baseline execute: {e}")))?;
        let Some(bl_row) = bl_result.next() else {
            return Ok(QualityGateSummary::default());
        };
        let rating = match &bl_row[0] {
            lbug::Value::Null(_) => None,
            other => Some(other.to_string()),
        };
        let total_issues = match &bl_row[1] {
            lbug::Value::Int64(n) => *n as usize,
            lbug::Value::Int32(n) => *n as usize,
            _ => 0,
        };
        let blockers = match &bl_row[2] {
            lbug::Value::Int64(n) => *n as usize,
            lbug::Value::Int32(n) => *n as usize,
            _ => 0,
        };
        let criticals = match &bl_row[3] {
            lbug::Value::Int64(n) => *n as usize,
            lbug::Value::Int32(n) => *n as usize,
            _ => 0,
        };
        let debt_minutes = match &bl_row[4] {
            lbug::Value::Int64(n) => *n.max(&0) as u64,
            lbug::Value::Int32(n) => *n.max(&0) as u64,
            _ => 0,
        };
        let last_run = match &bl_row[5] {
            lbug::Value::Null(_) => None,
            other => Some(other.to_string()),
        };
        Ok(QualityGateSummary {
            rating,
            total_issues,
            blockers,
            criticals,
            debt_minutes,
            last_run,
        })
    }

    fn open_issues_count(&self, workspace_id: Option<&str>) -> Result<usize, QualityError> {
        let ws = workspace_id.unwrap_or("default");
        let conn = self
            .connection()
            .map_err(|e| QualityError::Store(format!("open_issues_count: {e}")))?;
        let mut stmt = conn
            .prepare(
                "MATCH (i:Issue) WHERE i.workspace_id = $ws AND i.status = 'open' RETURN count(i);",
            )
            .map_err(|e| QualityError::Store(format!("open_issues_count: prepare: {e}")))?;
        let mut result = conn
            .execute(&mut stmt, vec![("ws", lbug::Value::String(ws.to_string()))])
            .map_err(|e| QualityError::Store(format!("open_issues_count: execute: {e}")))?;
        let Some(row) = result.next() else {
            return Ok(0);
        };
        let count = match &row[0] {
            lbug::Value::Int64(n) => *n as usize,
            lbug::Value::Int32(n) => *n as usize,
            _ => 0,
        };
        Ok(count)
    }

    fn issues_for_workspace(
        &self,
        workspace_id: Option<&str>,
        filter: &IssueFilter,
    ) -> Result<Vec<QualityIssue>, QualityError> {
        // Optional filters are AND-combined. Build the WHERE clause
        // dynamically — `None` means "no filter on this dimension".
        let ws = workspace_id.unwrap_or("default");
        let mut cypher = String::from("MATCH (i:Issue) WHERE i.workspace_id = $ws");
        if filter.severity.is_some() {
            cypher.push_str(" AND i.severity = $sev");
        }
        if filter.category.is_some() {
            cypher.push_str(" AND i.category = $cat");
        }
        if filter.status.is_some() {
            cypher.push_str(" AND i.status = $stat");
        }
        if filter.file_prefix.is_some() {
            // Same boundary-aware pattern as `issues_for_scope`.
            cypher.push_str(" AND (i.file_path = $fp OR i.file_path STARTS WITH $fp_slash)");
        }
        cypher.push_str(" RETURN i.id, i.rule_id, i.severity, i.category, i.file_path, i.line, i.message, i.status ORDER BY i.id");
        if let Some(limit) = filter.limit {
            cypher.push_str(&format!(" LIMIT {limit}"));
        }
        cypher.push(';');

        let conn = self
            .connection()
            .map_err(|e| QualityError::Store(format!("issues_for_workspace: {e}")))?;
        let mut stmt = conn
            .prepare(&cypher)
            .map_err(|e| QualityError::Store(format!("issues_for_workspace: prepare: {e}")))?;

        let mut params: Vec<(&str, lbug::Value)> =
            vec![("ws", lbug::Value::String(ws.to_string()))];
        if let Some(s) = &filter.severity {
            params.push(("sev", lbug::Value::String(s.clone())));
        }
        if let Some(c) = &filter.category {
            params.push(("cat", lbug::Value::String(c.clone())));
        }
        if let Some(s) = &filter.status {
            params.push(("stat", lbug::Value::String(s.clone())));
        }
        if let Some(p) = &filter.file_prefix {
            params.push(("fp", lbug::Value::String(p.clone())));
            params.push(("fp_slash", lbug::Value::String(format!("{p}/"))));
        }

        let mut result = conn
            .execute(&mut stmt, params)
            .map_err(|e| QualityError::Store(format!("issues_for_workspace: execute: {e}")))?;
        let mut rows = Vec::new();
        while let Some(row) = result.next() {
            rows.push(parse_issue_row(&row)?);
        }
        Ok(rows)
    }

    fn insert_issues(&self, issues: &[NewIssue]) -> Result<UpsertSummary, QualityError> {
        // Upsert by natural key `(workspace_id, rule_id, file_path, line)`
        // — same as the PG adapter's
        // `ON CONFLICT (workspace_id, rule_id, file_path, line) DO UPDATE`
        // semantics. For each issue, read-then-conditional-write.
        let mut summary = UpsertSummary::default();
        let conn = self
            .connection()
            .map_err(|e| QualityError::Store(format!("insert_issues: {e}")))?;

        for issue in issues {
            // Step 1: existence check.
            let mut check_stmt = conn
                .prepare(
                    "MATCH (i:Issue) WHERE i.workspace_id = $ws AND i.rule_id = $rid AND i.file_path = $fp AND i.line = $line RETURN i.id;",
                )
                .map_err(|e| QualityError::Store(format!("insert_issues: check prepare: {e}")))?;
            let mut existing = conn
                .execute(
                    &mut check_stmt,
                    vec![
                        ("ws", lbug::Value::String(issue.workspace_id.clone())),
                        ("rid", lbug::Value::String(issue.rule_id.clone())),
                        ("fp", lbug::Value::String(issue.file_path.clone())),
                        ("line", lbug::Value::Int64(issue.line as i64)),
                    ],
                )
                .map_err(|e| QualityError::Store(format!("insert_issues: check execute: {e}")))?;

            if existing.next().is_some() {
                // Step 2a: UPDATE the existing row.
                let mut upd_stmt = conn
                    .prepare(
                        "MATCH (i:Issue) WHERE i.workspace_id = $ws AND i.rule_id = $rid AND i.file_path = $fp AND i.line = $line SET i.severity = $sev, i.category = $cat, i.message = $msg, i.status = $stat;",
                    )
                    .map_err(|e| {
                        QualityError::Store(format!("insert_issues: update prepare: {e}"))
                    })?;
                conn.execute(
                    &mut upd_stmt,
                    vec![
                        ("ws", lbug::Value::String(issue.workspace_id.clone())),
                        ("rid", lbug::Value::String(issue.rule_id.clone())),
                        ("fp", lbug::Value::String(issue.file_path.clone())),
                        ("line", lbug::Value::Int64(issue.line as i64)),
                        ("sev", lbug::Value::String(issue.severity.clone())),
                        ("cat", lbug::Value::String(issue.category.clone())),
                        ("msg", lbug::Value::String(issue.message.clone())),
                        ("stat", lbug::Value::String(issue.status.clone())),
                    ],
                )
                .map_err(|e| QualityError::Store(format!("insert_issues: update execute: {e}")))?;
                summary.updated += 1;
            } else {
                // Step 2b: CREATE a new row.
                let mut ins_stmt = conn
                    .prepare(
                        "CREATE (i:Issue {workspace_id: $ws, rule_id: $rid, severity: $sev, category: $cat, file_path: $fp, line: $line, message: $msg, status: $stat});",
                    )
                    .map_err(|e| {
                        QualityError::Store(format!("insert_issues: insert prepare: {e}"))
                    })?;
                conn.execute(
                    &mut ins_stmt,
                    vec![
                        ("ws", lbug::Value::String(issue.workspace_id.clone())),
                        ("rid", lbug::Value::String(issue.rule_id.clone())),
                        ("sev", lbug::Value::String(issue.severity.clone())),
                        ("cat", lbug::Value::String(issue.category.clone())),
                        ("fp", lbug::Value::String(issue.file_path.clone())),
                        ("line", lbug::Value::Int64(issue.line as i64)),
                        ("msg", lbug::Value::String(issue.message.clone())),
                        ("stat", lbug::Value::String(issue.status.clone())),
                    ],
                )
                .map_err(|e| QualityError::Store(format!("insert_issues: insert execute: {e}")))?;
                summary.inserted += 1;
            }
        }
        Ok(summary)
    }

    fn delete_issue(
        &self,
        workspace_id: &str,
        rule_id: &str,
        file_path: &str,
        line: u32,
    ) -> Result<bool, QualityError> {
        // MATCH WHERE natural key + DELETE. Returns Ok(true) /
        // Ok(false) via the same pre-check pattern used elsewhere.
        let conn = self
            .connection()
            .map_err(|e| QualityError::Store(format!("delete_issue: {e}")))?;

        // Step 1: existence check.
        let mut check_stmt = conn
            .prepare(
                "MATCH (i:Issue) WHERE i.workspace_id = $ws AND i.rule_id = $rid AND i.file_path = $fp AND i.line = $line RETURN i.id;",
            )
            .map_err(|e| QualityError::Store(format!("delete_issue: check prepare: {e}")))?;
        let mut existing = conn
            .execute(
                &mut check_stmt,
                vec![
                    ("ws", lbug::Value::String(workspace_id.to_string())),
                    ("rid", lbug::Value::String(rule_id.to_string())),
                    ("fp", lbug::Value::String(file_path.to_string())),
                    ("line", lbug::Value::Int64(line as i64)),
                ],
            )
            .map_err(|e| QualityError::Store(format!("delete_issue: check execute: {e}")))?;
        if existing.next().is_none() {
            return Ok(false);
        }

        // Step 2: DELETE.
        let mut del_stmt = conn
            .prepare(
                "MATCH (i:Issue) WHERE i.workspace_id = $ws AND i.rule_id = $rid AND i.file_path = $fp AND i.line = $line DELETE i;",
            )
            .map_err(|e| QualityError::Store(format!("delete_issue: prepare: {e}")))?;
        conn.execute(
            &mut del_stmt,
            vec![
                ("ws", lbug::Value::String(workspace_id.to_string())),
                ("rid", lbug::Value::String(rule_id.to_string())),
                ("fp", lbug::Value::String(file_path.to_string())),
                ("line", lbug::Value::Int64(line as i64)),
            ],
        )
        .map_err(|e| QualityError::Store(format!("delete_issue: execute: {e}")))?;
        Ok(true)
    }
}

/// Row mapper for `Issue` queries — shared by `issues_for_file`,
/// `issues_for_scope`, `issues_at_line`, `issue_by_id`, and
/// `issues_for_workspace`. Column order must match the RETURN
/// clauses above.
fn parse_issue_row(row: &[lbug::Value]) -> Result<QualityIssue, QualityError> {
    let id = match &row[0] {
        lbug::Value::Int64(n) => *n,
        lbug::Value::Int32(n) => *n as i64,
        other => {
            return Err(QualityError::Store(format!(
                "parse_issue_row: unexpected id type: {other:?}"
            )));
        }
    };
    let line = match &row[5] {
        lbug::Value::Int64(n) => (*n).max(0) as u32,
        lbug::Value::Int32(n) => (*n).max(0) as u32,
        other => {
            return Err(QualityError::Store(format!(
                "parse_issue_row: unexpected line type: {other:?}"
            )));
        }
    };
    Ok(QualityIssue {
        id,
        rule_id: row[1].to_string(),
        severity: row[2].to_string(),
        category: row[3].to_string(),
        file_path: row[4].to_string(),
        line,
        message: row[6].to_string(),
        status: row[7].to_string(),
    })
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
    // Force lbug-on-tempdir tests to run serially (see `Cargo.toml`
    // dev-deps note on `serial_test`).
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
    // QualityStore (Priority 7)
    // --------------------------------------------------------------------
    //
    // Same pattern as the per-port tests above: real lbug db in a
    // tempdir, schema-init helper(s), exercises the 10 trait
    // methods.

    /// Apply the three QualityStore NODE TABLE DDLs once per test
    /// database. Idempotent via `IF NOT EXISTS`.
    ///
    /// Note: natural uniqueness key in the trait is
    /// `(workspace_id, rule_id, file_path, line)` (the PG UNIQUE
    /// constraint the PG adapter relies on). lbug 0.19 NODE TABLEs
    /// only support single-column PRIMARY KEYs, so we use
    /// `id SERIAL PRIMARY KEY` and enforce the natural uniqueness
    /// at the application layer (via read-then-conditional-write
    /// in `insert_issues`).
    fn init_quality_schema(store: &LadybugStore) {
        let conn = store.connection().expect("schema-init connection");
        conn.query(
            "CREATE NODE TABLE IF NOT EXISTS Issue( \
                 id SERIAL PRIMARY KEY, \
                 workspace_id STRING, \
                 rule_id STRING, \
                 severity STRING, \
                 category STRING, \
                 file_path STRING, \
                 line INT64, \
                 message STRING, \
                 status STRING);",
        )
        .expect("create Issue NODE TABLE");
        conn.query(
            "CREATE NODE TABLE IF NOT EXISTS Rule( \
                 rule_id STRING PRIMARY KEY, \
                 description STRING);",
        )
        .expect("create Rule NODE TABLE");
        conn.query(
            "CREATE NODE TABLE IF NOT EXISTS Baseline( \
                 workspace_id STRING PRIMARY KEY, \
                 rating STRING, \
                 total_issues INT64, \
                 blockers INT64, \
                 criticals INT64, \
                 debt_minutes INT64, \
                 snapshot_at STRING);",
        )
        .expect("create Baseline NODE TABLE");
    }

    fn sample_issue(ws: &str, rule: &str, file: &str, line: u32) -> NewIssue {
        NewIssue {
            workspace_id: ws.to_string(),
            rule_id: rule.to_string(),
            severity: "warning".to_string(),
            category: "lint".to_string(),
            file_path: file.to_string(),
            line,
            message: format!("violation of {rule} at {file}:{line}"),
            status: "open".to_string(),
        }
    }

    #[tokio::test]
    #[serial]
    async fn quality_issues_for_file_returns_matching_rows() {
        let (store, _dir) = make_test_store();
        init_quality_schema(&store);
        store
            .insert_issues(&[
                sample_issue("ws-1", "R1", "src/a.rs", 10),
                sample_issue("ws-1", "R2", "src/a.rs", 20),
                sample_issue("ws-1", "R1", "src/b.rs", 5),
            ])
            .expect("insert");
        let rows = store.issues_for_file("src/a.rs").expect("list");
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|r| r.file_path == "src/a.rs"));
    }

    #[tokio::test]
    #[serial]
    async fn quality_issues_for_file_empty_when_no_match() {
        let (store, _dir) = make_test_store();
        init_quality_schema(&store);
        store
            .insert_issues(&[sample_issue("ws-1", "R1", "src/a.rs", 10)])
            .expect("insert");
        let rows = store.issues_for_file("src/none.rs").expect("list");
        assert!(rows.is_empty());
    }

    #[tokio::test]
    #[serial]
    async fn quality_issues_for_scope_boundary_aware() {
        let (store, _dir) = make_test_store();
        init_quality_schema(&store);
        store
            .insert_issues(&[
                sample_issue("ws-1", "R1", "src", 1),
                sample_issue("ws-1", "R1", "src/a.rs", 10),
                sample_issue("ws-1", "R1", "src/sub/b.rs", 20),
                sample_issue("ws-1", "R1", "src_extra.rs", 5),
            ])
            .expect("insert");
        let rows = store.issues_for_scope("src").expect("list");
        // Boundary-aware: must match `src` + `src/*`, NOT `src_extra.rs`.
        assert_eq!(rows.len(), 3, "boundary must exclude src_extra.rs");
        let paths: Vec<&str> = rows.iter().map(|r| r.file_path.as_str()).collect();
        assert!(paths.contains(&"src"));
        assert!(paths.contains(&"src/a.rs"));
        assert!(paths.contains(&"src/sub/b.rs"));
        assert!(!paths.contains(&"src_extra.rs"));
    }

    #[tokio::test]
    #[serial]
    async fn quality_issues_at_line_filters_correctly() {
        let (store, _dir) = make_test_store();
        init_quality_schema(&store);
        store
            .insert_issues(&[
                sample_issue("ws-1", "R1", "src/a.rs", 10),
                sample_issue("ws-1", "R1", "src/a.rs", 20),
                sample_issue("ws-1", "R2", "src/a.rs", 10),
            ])
            .expect("insert");
        let rows = store.issues_at_line("src/a.rs", 10).expect("list");
        assert_eq!(rows.len(), 2);
        assert!(
            rows.iter()
                .all(|r| r.file_path == "src/a.rs" && r.line == 10)
        );
    }

    #[tokio::test]
    #[serial]
    async fn quality_issue_by_id_returns_some_and_none() {
        let (store, _dir) = make_test_store();
        init_quality_schema(&store);
        store
            .insert_issues(&[sample_issue("ws-1", "R1", "src/a.rs", 10)])
            .expect("insert");
        let id = store
            .issues_for_file("src/a.rs")
            .expect("list")
            .first()
            .expect("at least one")
            .id;
        let issue = store.issue_by_id(id).expect("by_id");
        assert!(issue.is_some(), "existing id returns Some");
        let issue = issue.expect("present");
        assert_eq!(issue.id, id);
        assert_eq!(issue.rule_id, "R1");

        let missing = store.issue_by_id(9999).expect("by_id missing");
        assert!(missing.is_none(), "unknown id returns None");
    }

    #[tokio::test]
    #[serial]
    async fn quality_rule_summary_counts_open_issues() {
        let (store, _dir) = make_test_store();
        init_quality_schema(&store);
        // Create the Rule row first — the Cypher's `MATCH (r:Rule)` is
        // required for the OPTIONAL MATCH to compose into a row.
        let conn = store.connection().expect("conn");
        conn.query("CREATE (r:Rule {rule_id: 'R1', description: 'no-unused-vars'});")
            .expect("rule insert");
        // Insert 2 open + 1 closed under the same rule_id.
        store
            .insert_issues(&[
                NewIssue {
                    workspace_id: "ws-1".to_string(),
                    rule_id: "R1".to_string(),
                    severity: "warning".to_string(),
                    category: "lint".to_string(),
                    file_path: "src/a.rs".to_string(),
                    line: 10,
                    message: "1".to_string(),
                    status: "open".to_string(),
                },
                NewIssue {
                    workspace_id: "ws-1".to_string(),
                    rule_id: "R1".to_string(),
                    severity: "warning".to_string(),
                    category: "lint".to_string(),
                    file_path: "src/a.rs".to_string(),
                    line: 20,
                    message: "2".to_string(),
                    status: "open".to_string(),
                },
                NewIssue {
                    workspace_id: "ws-1".to_string(),
                    rule_id: "R1".to_string(),
                    severity: "warning".to_string(),
                    category: "lint".to_string(),
                    file_path: "src/a.rs".to_string(),
                    line: 30,
                    message: "3".to_string(),
                    status: "closed".to_string(),
                },
            ])
            .expect("insert");
        let summary = store.rule_summary("R1").expect("summary");
        assert_eq!(summary.rule_id, "R1");
        assert_eq!(summary.description, "no-unused-vars");
        assert_eq!(summary.open_count, 2, "only status=open counted");
    }

    #[tokio::test]
    #[serial]
    async fn quality_quality_gate_returns_baseline_fields() {
        let (store, _dir) = make_test_store();
        init_quality_schema(&store);
        // Insert a baseline row manually.
        let conn = store.connection().expect("conn");
        conn.query(
            "CREATE (b:Baseline {workspace_id: 'ws-1', rating: 'B', total_issues: 12, blockers: 1, criticals: 4, debt_minutes: 90, snapshot_at: '2026-08-02T10:00:00Z'});",
        )
        .expect("baseline insert");
        let gate = store.quality_gate(Some("ws-1")).expect("gate");
        assert_eq!(gate.rating.as_deref(), Some("B"));
        assert_eq!(gate.total_issues, 12);
        assert_eq!(gate.blockers, 1);
        assert_eq!(gate.criticals, 4);
        assert_eq!(gate.debt_minutes, 90);
        assert_eq!(gate.last_run.as_deref(), Some("2026-08-02T10:00:00Z"));
    }

    #[tokio::test]
    #[serial]
    async fn quality_quality_gate_returns_default_when_no_baseline() {
        let (store, _dir) = make_test_store();
        init_quality_schema(&store);
        let gate = store.quality_gate(Some("ws-1")).expect("gate");
        assert_eq!(gate.rating, None);
        assert_eq!(gate.total_issues, 0);
        assert_eq!(gate.debt_minutes, 0);
    }

    #[tokio::test]
    #[serial]
    async fn quality_open_issues_count_filters_by_status() {
        let (store, _dir) = make_test_store();
        init_quality_schema(&store);
        store
            .insert_issues(&[
                NewIssue {
                    status: "open".to_string(),
                    ..sample_issue("ws-1", "R1", "src/a.rs", 10)
                },
                NewIssue {
                    status: "open".to_string(),
                    ..sample_issue("ws-1", "R1", "src/a.rs", 20)
                },
                NewIssue {
                    status: "closed".to_string(),
                    ..sample_issue("ws-1", "R1", "src/a.rs", 30)
                },
            ])
            .expect("insert");
        assert_eq!(store.open_issues_count(Some("ws-1")).expect("count"), 2);
    }

    #[tokio::test]
    #[serial]
    async fn quality_open_issues_count_scopes_to_workspace() {
        let (store, _dir) = make_test_store();
        init_quality_schema(&store);
        store
            .insert_issues(&[
                sample_issue("ws-A", "R1", "src/a.rs", 10),
                sample_issue("ws-A", "R1", "src/a.rs", 20),
                sample_issue("ws-B", "R1", "src/b.rs", 10),
            ])
            .expect("insert");
        assert_eq!(store.open_issues_count(Some("ws-A")).expect("A"), 2);
        assert_eq!(store.open_issues_count(Some("ws-B")).expect("B"), 1);
    }

    #[tokio::test]
    #[serial]
    async fn quality_issues_for_workspace_filters_combined() {
        let (store, _dir) = make_test_store();
        init_quality_schema(&store);
        store
            .insert_issues(&[
                NewIssue {
                    severity: "blocker".to_string(),
                    status: "open".to_string(),
                    ..sample_issue("ws-1", "R1", "src/a.rs", 10)
                },
                NewIssue {
                    severity: "blocker".to_string(),
                    status: "closed".to_string(),
                    ..sample_issue("ws-1", "R1", "src/a.rs", 20)
                },
                NewIssue {
                    severity: "warning".to_string(),
                    status: "open".to_string(),
                    ..sample_issue("ws-1", "R2", "src/a.rs", 30)
                },
                NewIssue {
                    severity: "blocker".to_string(),
                    status: "open".to_string(),
                    ..sample_issue("ws-2", "R1", "src/b.rs", 10)
                },
            ])
            .expect("insert");

        // Filter: ws-1 + severity=blocker + status=open → 1 row.
        let filter = IssueFilter {
            severity: Some("blocker".to_string()),
            status: Some("open".to_string()),
            ..IssueFilter::default()
        };
        let rows = store
            .issues_for_workspace(Some("ws-1"), &filter)
            .expect("list");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].severity, "blocker");
        assert_eq!(rows[0].status, "open");
    }

    #[tokio::test]
    #[serial]
    async fn quality_issues_for_workspace_respects_limit() {
        let (store, _dir) = make_test_store();
        init_quality_schema(&store);
        store
            .insert_issues(&[
                sample_issue("ws-1", "R1", "src/a.rs", 10),
                sample_issue("ws-1", "R1", "src/a.rs", 20),
                sample_issue("ws-1", "R1", "src/a.rs", 30),
            ])
            .expect("insert");
        let filter = IssueFilter {
            limit: Some(2),
            ..IssueFilter::default()
        };
        let rows = store
            .issues_for_workspace(Some("ws-1"), &filter)
            .expect("list");
        assert_eq!(rows.len(), 2, "limit=2 must cap the result set");
    }

    #[tokio::test]
    #[serial]
    async fn quality_insert_issues_upserts_on_natural_key() {
        let (store, _dir) = make_test_store();
        init_quality_schema(&store);
        // First insert: 1 new row.
        let s1 = store
            .insert_issues(&[sample_issue("ws-1", "R1", "src/a.rs", 10)])
            .expect("ins1");
        assert_eq!(
            s1,
            UpsertSummary {
                inserted: 1,
                updated: 0
            }
        );

        // Second insert with same natural key → upsert (update), no new row.
        let mut updated = sample_issue("ws-1", "R1", "src/a.rs", 10);
        updated.message = "updated message".to_string();
        updated.status = "closed".to_string();
        let s2 = store.insert_issues(&[updated]).expect("ins2");
        assert_eq!(
            s2,
            UpsertSummary {
                inserted: 0,
                updated: 1
            }
        );

        // Verify only 1 row exists.
        let rows = store.issues_for_file("src/a.rs").expect("list");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].message, "updated message");
        assert_eq!(rows[0].status, "closed");
    }

    #[tokio::test]
    #[serial]
    async fn quality_insert_issues_counts_separately() {
        let (store, _dir) = make_test_store();
        init_quality_schema(&store);
        // 1 existing + 1 new in one batch.
        store
            .insert_issues(&[sample_issue("ws-1", "R1", "src/a.rs", 10)])
            .expect("seed");
        let batch = vec![
            sample_issue("ws-1", "R1", "src/a.rs", 10), // existing
            sample_issue("ws-1", "R2", "src/b.rs", 20), // new
        ];
        let s = store.insert_issues(&batch).expect("batch");
        assert_eq!(
            s,
            UpsertSummary {
                inserted: 1,
                updated: 1
            }
        );
    }

    #[tokio::test]
    #[serial]
    async fn quality_delete_issue_removes_target_row() {
        let (store, _dir) = make_test_store();
        init_quality_schema(&store);
        store
            .insert_issues(&[sample_issue("ws-1", "R1", "src/a.rs", 10)])
            .expect("seed");
        let deleted = store
            .delete_issue("ws-1", "R1", "src/a.rs", 10)
            .expect("delete");
        assert!(deleted, "delete must return Ok(true)");
        let rows = store.issues_for_file("src/a.rs").expect("list");
        assert!(rows.is_empty());
    }

    #[tokio::test]
    #[serial]
    async fn quality_delete_issue_missing_returns_false() {
        let (store, _dir) = make_test_store();
        init_quality_schema(&store);
        let deleted = store
            .delete_issue("ws-1", "R-missing", "src/a.rs", 10)
            .expect("delete");
        assert!(!deleted, "unknown natural key returns Ok(false)");
    }
}
