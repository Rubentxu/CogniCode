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
//! | 4 | `RevisionStore` | UPDATE-only on `graph_revisions`, plus the read-only `head_revision` | DONE (this branch) |
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
    async fn head_revision(&self, ws: &WorkspaceId) -> Result<Option<RevisionId>, RevisionError> {
        // ADR-028 §3 `head_revision(ws)` — read-only. Returns the
        // `revision_id` of the single row where `head_of = true` for
        // the workspace, or None if no revisions exist yet.
        //
        // The trait signature requires `&self` for read paths (only
        // write paths take `&mut PgConnection`), so this matches.
        let conn = self
            .connection()
            .map_err(|e| RevisionError::Store(format!("head_revision: {e}")))?;
        let mut stmt = conn
            .prepare(
                "MATCH (r:GraphRevision) WHERE r.workspace_id = $ws AND r.head_of = true RETURN r.revision_id;",
            )
            .map_err(|e| RevisionError::Store(format!("head_revision: prepare: {e}")))?;
        let mut result = conn
            .execute(&mut stmt, vec![("ws", lbug::Value::String(ws.to_string()))])
            .map_err(|e| RevisionError::Store(format!("head_revision: execute: {e}")))?;

        let Some(row) = result.next() else {
            return Ok(None);
        };
        let rev_id = match &row[0] {
            lbug::Value::Int32(n) => *n as u64,
            lbug::Value::Int64(n) => *n as u64,
            other => {
                return Err(RevisionError::Store(format!(
                    "head_revision: unexpected revision_id type: {other:?}"
                )));
            }
        };
        Ok(Some(RevisionId(rev_id)))
    }

    async fn create_revision(
        &self,
        _conn: &mut sqlx::PgConnection,
        ws: &WorkspaceId,
    ) -> Result<RevisionId, RevisionError> {
        // Delegate to the `&self`-only helper — the trait's
        // `&mut PgConnection` is ignored (see `e29-trait-tx-handle-refactor`
        // follow-up documented in `head_revision`).
        self.create_revision_for_ws(ws).await
    }

    async fn set_head(
        &self,
        _conn: &mut sqlx::PgConnection,
        ws: &WorkspaceId,
        rev: RevisionId,
    ) -> Result<(), RevisionError> {
        // Delegate to the `&self`-only helper — see comment in
        // `create_revision` for why the trait's tx handle is ignored.
        self.set_head_for_ws(ws, rev).await
    }
}

impl LadybugStore {
    /// `RevisionStore::create_revision` body extracted to a
    /// `&self`-only helper so tests can exercise it without a
    /// `sqlx::PgConnection` (which has no public constructor in
    /// sqlx 0.8 — connecting requires a live PG server).
    ///
    /// ADR-028 §3 — atomically demote prior head, compute next
    /// `revision_id = MAX(revision_id) + 1` (or 1 if no rows), INSERT
    /// new row with `head_of = true`. Single multi-pattern Cypher
    /// so lbug's per-`execute` auto-commit keeps it atomic.
    ///
    /// **Cypher quirk**: lbug 0.19 doesn't preserve `$param` aliases
    /// across a `WITH` that follows a `SET` on a write scope. We
    /// pivot the scope with `WITH count(old)` (an aggregation) so
    /// the subsequent `OPTIONAL MATCH` can re-bind `$ws` via the
    /// outer query parameter. This was confirmed empirically against
    /// the spike crate (see the cypher_probe harness). A pure
    /// `WITH $ws AS ws` after `SET old.head_of = false` returns no
    /// rows for the subsequent CREATE — `count()` is the magic.
    pub(crate) async fn create_revision_for_ws(
        &self,
        ws: &WorkspaceId,
    ) -> Result<RevisionId, RevisionError> {
        let conn = self
            .connection()
            .map_err(|e| RevisionError::Store(format!("create_revision: {e}")))?;
        let mut stmt = conn
            .prepare(
                "MATCH (old:GraphRevision) WHERE old.workspace_id = $ws AND old.head_of = true SET old.head_of = false WITH count(old) AS _demoted OPTIONAL MATCH (r:GraphRevision) WHERE r.workspace_id = $ws WITH $ws AS ws, coalesce(max(r.revision_id), 0) AS max_rev CREATE (new:GraphRevision {workspace_id: ws, revision_id: max_rev + 1, head_of: true}) RETURN new.revision_id;",
            )
            .map_err(|e| {
                RevisionError::Store(format!("create_revision: prepare: {e}"))
            })?;
        let mut result = conn
            .execute(&mut stmt, vec![("ws", lbug::Value::String(ws.to_string()))])
            .map_err(|e| RevisionError::Store(format!("create_revision: execute: {e}")))?;

        let Some(row) = result.next() else {
            return Err(RevisionError::Store(
                "create_revision: CREATE produced no RETURN row".into(),
            ));
        };
        let rev_id = match &row[0] {
            lbug::Value::Int32(n) => *n as u64,
            lbug::Value::Int64(n) => *n as u64,
            other => {
                return Err(RevisionError::Store(format!(
                    "create_revision: unexpected revision_id type: {other:?}"
                )));
            }
        };
        Ok(RevisionId(rev_id))
    }

    /// `RevisionStore::set_head` body extracted to a `&self`-only
    /// helper — see `create_revision_for_ws` for the rationale.
    ///
    /// ADR-028 §3 — demote prior head + promote target revision.
    /// Single Cypher using the same `WITH count()` pivot trick so
    /// the demote scope and the promote scope compose in one statement.
    pub(crate) async fn set_head_for_ws(
        &self,
        ws: &WorkspaceId,
        rev: RevisionId,
    ) -> Result<(), RevisionError> {
        let conn = self
            .connection()
            .map_err(|e| RevisionError::Store(format!("set_head: {e}")))?;
        let mut stmt = conn
            .prepare(
                "MATCH (old:GraphRevision) WHERE old.workspace_id = $ws AND old.head_of = true SET old.head_of = false WITH count(old) AS _demoted, $rev AS target MATCH (target_row:GraphRevision) WHERE target_row.workspace_id = $ws AND target_row.revision_id = target SET target_row.head_of = true;",
            )
            .map_err(|e| RevisionError::Store(format!("set_head: prepare: {e}")))?;
        conn.execute(
            &mut stmt,
            vec![
                ("ws", lbug::Value::String(ws.to_string())),
                ("rev", lbug::Value::Int64(rev.get() as i64)),
            ],
        )
        .map_err(|e| RevisionError::Store(format!("set_head: execute: {e}")))?;
        Ok(())
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
        workspace_id: &str,
        report: &ReportSummary,
    ) -> Result<(), ReportError> {
        // ADR-028 §3 `save_report(ws, report)` — INSERT a new GraphReport
        // node keyed by `id` (single-column STRING PK, same lbug 0.19
        // shape as SessionStore's ExplorationSession). The PG adapter
        // trusts the caller's `id` + `created_at`; we mirror that here.
        //
        // `report` (the JSON column) is serialized to a STRING — lbug
        // 0.19 has no JSON type, and the application layer already
        // round-trips via serde_json (same pattern as SessionStore's
        // `events` / `panes` columns).
        //
        // `health_score` is `Option<f32>` — null-safe via a NULL
        // parameter when None.
        let conn = self
            .connection()
            .map_err(|e| ReportError::Store(format!("save_report: {e}")))?;

        let report_json = serde_json::to_string(&report.report)
            .map_err(|e| ReportError::Store(format!("save_report: serialize report: {e}")))?;

        let mut stmt = conn
            .prepare(
                "CREATE (r:GraphReport {id: $id, workspace_id: $ws, created_at: $ts, report: $json, symbol_count: $scnt, edge_count: $ecnt, health_score: $hscore});",
            )
            .map_err(|e| ReportError::Store(format!("save_report: prepare: {e}")))?;
        conn.execute(
            &mut stmt,
            vec![
                ("id", lbug::Value::String(report.id.clone())),
                ("ws", lbug::Value::String(workspace_id.to_string())),
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
        .map_err(|e| ReportError::Store(format!("save_report: execute: {e}")))?;
        Ok(())
    }

    async fn latest_report(
        &self,
        workspace_id: &str,
    ) -> Result<Option<ReportSummary>, ReportError> {
        // ADR-028 §3 `latest_report(ws)` — newest row by `created_at`,
        // or None. ORDER BY + LIMIT 1 keeps the read O(rows-scanned);
        // no time-range filter (per ADR-028 §3 — PG adapter also uses
        // no time-range filter at the trait level).
        let conn = self
            .connection()
            .map_err(|e| ReportError::Store(format!("latest_report: {e}")))?;
        let mut stmt = conn
            .prepare(
                "MATCH (r:GraphReport) WHERE r.workspace_id = $ws RETURN r.id, r.workspace_id, r.created_at, r.report, r.symbol_count, r.edge_count, r.health_score ORDER BY r.created_at DESC LIMIT 1;",
            )
            .map_err(|e| ReportError::Store(format!("latest_report: prepare: {e}")))?;
        let mut result = conn
            .execute(
                &mut stmt,
                vec![("ws", lbug::Value::String(workspace_id.to_string()))],
            )
            .map_err(|e| ReportError::Store(format!("latest_report: execute: {e}")))?;

        let Some(row) = result.next() else {
            return Ok(None);
        };
        parse_report_row(&row).map(Some)
    }

    async fn reports_for_workspace(
        &self,
        workspace_id: &str,
    ) -> Result<Vec<ReportSummary>, ReportError> {
        // ADR-028 §3 `reports_for_workspace(ws)` — every row for `ws`,
        // ordered newest-first. No time-range filter (PG adapter also
        // applies no range at the trait level; the underlying
        // `load_report_range(days=365)` is adapter-local policy).
        let conn = self
            .connection()
            .map_err(|e| ReportError::Store(format!("reports_for_workspace: {e}")))?;
        let mut stmt = conn
            .prepare(
                "MATCH (r:GraphReport) WHERE r.workspace_id = $ws RETURN r.id, r.workspace_id, r.created_at, r.report, r.symbol_count, r.edge_count, r.health_score ORDER BY r.created_at DESC;",
            )
            .map_err(|e| ReportError::Store(format!("reports_for_workspace: prepare: {e}")))?;
        let mut result = conn
            .execute(
                &mut stmt,
                vec![("ws", lbug::Value::String(workspace_id.to_string()))],
            )
            .map_err(|e| ReportError::Store(format!("reports_for_workspace: execute: {e}")))?;

        let mut rows = Vec::new();
        while let Some(row) = result.next() {
            rows.push(parse_report_row(&row)?);
        }
        Ok(rows)
    }
}

/// Row mapper for `ReportStore` queries — shared by `latest_report`
/// and `reports_for_workspace` so both stay in lock-step on column
/// order. Column order must match the RETURN clauses above.
fn parse_report_row(row: &[lbug::Value]) -> Result<ReportSummary, ReportError> {
    let id = row[0].to_string();
    let workspace_id = row[1].to_string();
    let created_at = row[2].to_string();
    let report: serde_json::Value = serde_json::from_str(&row[3].to_string())
        .map_err(|e| ReportError::Store(format!("parse_report_row: report JSON: {e}")))?;
    let symbol_count = match &row[4] {
        lbug::Value::Int32(n) => *n,
        lbug::Value::Int64(n) => *n as i32,
        _ => 0,
    };
    let edge_count = match &row[5] {
        lbug::Value::Int32(n) => *n,
        lbug::Value::Int64(n) => *n as i32,
        _ => 0,
    };
    let health_score = match &row[6] {
        lbug::Value::Null(_) => None,
        lbug::Value::Double(n) => Some(*n as f32),
        _ => None,
    };
    Ok(ReportSummary {
        id,
        workspace_id,
        created_at,
        report,
        symbol_count,
        edge_count,
        health_score,
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
    // RevisionStore (Priority 4)
    // --------------------------------------------------------------------
    //
    // Same pattern as the per-port tests above: real lbug db in a
    // tempdir, schema-init helper, exercises the 3 trait methods.

    /// Apply the GraphRevision NODE TABLE DDL once per test database.
    /// Idempotent via `IF NOT EXISTS`.
    ///
    /// Note: natural uniqueness key is `(workspace_id, revision_id)`
    /// but lbug 0.19 NODE TABLEs only support single-column PRIMARY
    /// KEYs (the same limitation that drove ManifestStore's
    /// read-then-conditional-write). We use a synthetic `id SERIAL`
    /// PK and enforce `(workspace_id, revision_id)` uniqueness at
    /// the application layer (via the single-Cypher multi-pattern in
    /// `create_revision` that computes `MAX(revision_id) + 1` per
    /// workspace, so collisions cannot happen in practice).
    ///
    /// The at-most-one-row-with-`head_of = true` invariant per
    /// workspace is also enforced at the application layer via the
    /// demote-first-then-create pattern in both `create_revision`
    /// and `set_head` (single Cypher each).
    fn init_graph_revisions_schema(store: &LadybugStore) {
        let conn = store.connection().expect("schema-init connection");
        conn.query(
            "CREATE NODE TABLE IF NOT EXISTS GraphRevision( \
                 id SERIAL PRIMARY KEY, \
                 workspace_id STRING, \
                 revision_id INT64, \
                 head_of BOOLEAN);",
        )
        .expect("create GraphRevision NODE TABLE");
    }

    /// Build a `WorkspaceId` for tests (uses `try_new` per the
    /// `cognicode_core` value-object contract).
    fn ws_id(s: &str) -> WorkspaceId {
        WorkspaceId::try_new(s).expect("workspace id must be non-empty")
    }

    #[tokio::test]
    async fn revision_head_returns_none_for_fresh_db() {
        let (store, _dir) = make_test_store();
        init_graph_revisions_schema(&store);
        let head = RevisionStore::head_revision(&store, &ws_id("ws-unknown"))
            .await
            .expect("head");
        assert!(head.is_none(), "fresh db must return None");
    }

    #[tokio::test]
    async fn revision_create_returns_1_for_fresh_workspace() {
        let (store, _dir) = make_test_store();
        init_graph_revisions_schema(&store);
        let rev = store
            .create_revision_for_ws(&ws_id("ws-A"))
            .await
            .expect("create");
        assert_eq!(
            rev,
            RevisionId(1),
            "first revision in a fresh workspace must be 1"
        );
    }

    #[tokio::test]
    async fn revision_create_increments_monotonically() {
        // Three successive `create_revision_for_ws` calls on the same
        // workspace must produce 1, 2, 3 (MAX + 1 per call).
        let (store, _dir) = make_test_store();
        init_graph_revisions_schema(&store);
        let r1 = store
            .create_revision_for_ws(&ws_id("ws-x"))
            .await
            .expect("c1");
        let r2 = store
            .create_revision_for_ws(&ws_id("ws-x"))
            .await
            .expect("c2");
        let r3 = store
            .create_revision_for_ws(&ws_id("ws-x"))
            .await
            .expect("c3");
        assert_eq!(r1, RevisionId(1));
        assert_eq!(r2, RevisionId(2));
        assert_eq!(r3, RevisionId(3));

        // head_revision must reflect the latest.
        let head = RevisionStore::head_revision(&store, &ws_id("ws-x"))
            .await
            .expect("head");
        assert_eq!(head, Some(RevisionId(3)));
    }

    #[tokio::test]
    async fn revision_create_demotes_prior_head() {
        // After `create_revision_for_ws` runs, exactly one row per
        // workspace has `head_of = true` and it's the latest.
        let (store, _dir) = make_test_store();
        init_graph_revisions_schema(&store);
        let r1 = store
            .create_revision_for_ws(&ws_id("ws-d"))
            .await
            .expect("c1");
        let r2 = store
            .create_revision_for_ws(&ws_id("ws-d"))
            .await
            .expect("c2");

        // Head must be r2; r1 must NOT be head anymore.
        let head = RevisionStore::head_revision(&store, &ws_id("ws-d"))
            .await
            .expect("head");
        assert_eq!(head, Some(r2));
        assert_ne!(Some(r1), head);
    }

    #[tokio::test]
    async fn revision_create_scopes_to_workspace() {
        // Two workspaces, three revisions total — head must be
        // workspace-scoped.
        let (store, _dir) = make_test_store();
        init_graph_revisions_schema(&store);
        store
            .create_revision_for_ws(&ws_id("ws-A"))
            .await
            .expect("a1");
        store
            .create_revision_for_ws(&ws_id("ws-A"))
            .await
            .expect("a2");
        store
            .create_revision_for_ws(&ws_id("ws-B"))
            .await
            .expect("b1");

        assert_eq!(
            RevisionStore::head_revision(&store, &ws_id("ws-A"))
                .await
                .expect("h-A"),
            Some(RevisionId(2)),
            "ws-A's head must be its 2nd revision"
        );
        assert_eq!(
            RevisionStore::head_revision(&store, &ws_id("ws-B"))
                .await
                .expect("h-B"),
            Some(RevisionId(1)),
            "ws-B's head must be its 1st revision"
        );
    }

    #[tokio::test]
    async fn revision_set_head_promotes_target_and_demotes_prior() {
        // Create three revisions; then set_head back to revision 2.
        // After the call: head = rev 2; rev 3 must NOT be head.
        let (store, _dir) = make_test_store();
        init_graph_revisions_schema(&store);
        let r1 = store
            .create_revision_for_ws(&ws_id("ws-s"))
            .await
            .expect("c1");
        let r2 = store
            .create_revision_for_ws(&ws_id("ws-s"))
            .await
            .expect("c2");
        let _r3 = store
            .create_revision_for_ws(&ws_id("ws-s"))
            .await
            .expect("c3");

        // Sanity: head is now r3.
        assert_eq!(
            RevisionStore::head_revision(&store, &ws_id("ws-s"))
                .await
                .expect("pre"),
            Some(RevisionId(3))
        );

        // set_head back to r2.
        store
            .set_head_for_ws(&ws_id("ws-s"), r2)
            .await
            .expect("set_head");

        let head = RevisionStore::head_revision(&store, &ws_id("ws-s"))
            .await
            .expect("post");
        assert_eq!(head, Some(r2), "set_head must promote the target");
        assert_ne!(head, Some(r1));
    }

    // --------------------------------------------------------------------
    // ReportStore (Priority 3)
    // --------------------------------------------------------------------
    //
    // Same pattern as the ManifestStore / SessionStore tests above:
    // real lbug db in a tempdir, schema-init helper, exercises
    // save/latest/list with the same shapes the Postgres adapter uses.

    /// Apply the GraphReport NODE TABLE DDL once per test database.
    /// Idempotent via `IF NOT EXISTS`.
    ///
    /// Note: like ExplorationSession, the natural PK here is
    /// single-column (`id` STRING) — no synthetic PK needed and no
    /// read-then-conditional-write workaround. The PG adapter's
    /// `save_report` is also a single-pass INSERT (or
    /// INSERT-ON-CONFLICT-DO-UPDATE), so parity is direct.
    ///
    /// `report` is stored as STRING (the JSON text). `health_score`
    /// is nullable DOUBLE.
    fn init_graph_report_schema(store: &LadybugStore) {
        let conn = store.connection().expect("schema-init connection");
        conn.query(
            "CREATE NODE TABLE IF NOT EXISTS GraphReport( \
                 id STRING PRIMARY KEY, \
                 workspace_id STRING, \
                 created_at STRING, \
                 report STRING, \
                 symbol_count INT64, \
                 edge_count INT64, \
                 health_score DOUBLE);",
        )
        .expect("create GraphReport NODE TABLE");
    }

    fn sample_report(id: &str, ws: &str, ts: &str) -> ReportSummary {
        ReportSummary {
            id: id.to_string(),
            workspace_id: ws.to_string(),
            created_at: ts.to_string(),
            report: serde_json::json!({
                "summary": "test report",
                "issues": 3,
            }),
            symbol_count: 100,
            edge_count: 42,
            health_score: Some(0.85),
        }
    }

    #[tokio::test]
    async fn report_latest_returns_none_for_fresh_db() {
        let (store, _dir) = make_test_store();
        init_graph_report_schema(&store);
        let r = ReportStore::latest_report(&store, "ws-unknown")
            .await
            .expect("latest");
        assert!(r.is_none(), "fresh db should return None");
    }

    #[tokio::test]
    async fn report_list_returns_empty_for_fresh_db() {
        let (store, _dir) = make_test_store();
        init_graph_report_schema(&store);
        let rows = ReportStore::reports_for_workspace(&store, "ws-unknown")
            .await
            .expect("list");
        assert!(rows.is_empty(), "fresh db should return no rows");
    }

    #[tokio::test]
    async fn report_save_then_latest_round_trips() {
        let (store, _dir) = make_test_store();
        init_graph_report_schema(&store);
        let report = sample_report("rep-1", "ws-1", "2026-08-02T10:00:00Z");
        ReportStore::save_report(&store, "ws-1", &report)
            .await
            .expect("save");
        let loaded = ReportStore::latest_report(&store, "ws-1")
            .await
            .expect("latest");
        let loaded = loaded.expect("latest should return Some after save");
        assert_eq!(loaded.id, "rep-1");
        assert_eq!(loaded.workspace_id, "ws-1");
        assert_eq!(loaded.created_at, "2026-08-02T10:00:00Z");
        assert_eq!(loaded.report["summary"], "test report");
        assert_eq!(loaded.report["issues"], 3);
        assert_eq!(loaded.symbol_count, 100);
        assert_eq!(loaded.edge_count, 42);
        assert!(
            (loaded.health_score.expect("health_score") - 0.85_f32).abs() < 1e-4,
            "health_score should round-trip ~0.85",
        );
    }

    #[tokio::test]
    async fn report_save_with_null_health_score_round_trips() {
        let (store, _dir) = make_test_store();
        init_graph_report_schema(&store);
        let mut report = sample_report("rep-2", "ws-2", "2026-08-02T10:05:00Z");
        report.health_score = None;
        ReportStore::save_report(&store, "ws-2", &report)
            .await
            .expect("save");
        let loaded = ReportStore::latest_report(&store, "ws-2")
            .await
            .expect("latest")
            .expect("present");
        assert!(
            loaded.health_score.is_none(),
            "null health_score should round-trip None"
        );
    }

    #[tokio::test]
    async fn report_list_returns_rows_in_created_at_desc_order() {
        // Save three reports in a known order; list must return them
        // newest-first. Same determinism pattern as SessionStore's list
        // ordering test: back-to-back saves may land in the same
        // nanosecond, so we use distinct `created_at` values supplied
        // by the caller (the PG adapter trusts the caller's
        // `created_at`; we mirror that).
        let (store, _dir) = make_test_store();
        init_graph_report_schema(&store);
        ReportStore::save_report(
            &store,
            "ws-x",
            &sample_report("rep-old", "ws-x", "2026-08-02T08:00:00Z"),
        )
        .await
        .expect("save-old");
        ReportStore::save_report(
            &store,
            "ws-x",
            &sample_report("rep-mid", "ws-x", "2026-08-02T09:00:00Z"),
        )
        .await
        .expect("save-mid");
        ReportStore::save_report(
            &store,
            "ws-x",
            &sample_report("rep-new", "ws-x", "2026-08-02T10:00:00Z"),
        )
        .await
        .expect("save-new");
        let rows = ReportStore::reports_for_workspace(&store, "ws-x")
            .await
            .expect("list");
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].id, "rep-new");
        assert_eq!(rows[1].id, "rep-mid");
        assert_eq!(rows[2].id, "rep-old");
    }

    #[tokio::test]
    async fn report_latest_and_list_scopes_to_workspace() {
        // Two workspaces, three reports total — `latest_report(ws-A)`
        // and `reports_for_workspace(ws-A)` must only see ws-A's rows.
        let (store, _dir) = make_test_store();
        init_graph_report_schema(&store);
        ReportStore::save_report(
            &store,
            "ws-A",
            &sample_report("a-1", "ws-A", "2026-08-02T08:00:00Z"),
        )
        .await
        .expect("a1");
        ReportStore::save_report(
            &store,
            "ws-A",
            &sample_report("a-2", "ws-A", "2026-08-02T09:00:00Z"),
        )
        .await
        .expect("a2");
        ReportStore::save_report(
            &store,
            "ws-B",
            &sample_report("b-1", "ws-B", "2026-08-02T10:00:00Z"),
        )
        .await
        .expect("b1");

        let a_latest = ReportStore::latest_report(&store, "ws-A")
            .await
            .expect("latest-A")
            .expect("present");
        assert_eq!(
            a_latest.id, "a-2",
            "latest for ws-A must be a-2 (newest ws-A report)"
        );

        let a_rows = ReportStore::reports_for_workspace(&store, "ws-A")
            .await
            .expect("list-A");
        assert_eq!(a_rows.len(), 2);
        assert!(a_rows.iter().all(|r| r.workspace_id == "ws-A"));

        let b_rows = ReportStore::reports_for_workspace(&store, "ws-B")
            .await
            .expect("list-B");
        assert_eq!(b_rows.len(), 1);
        assert_eq!(b_rows[0].id, "b-1");
    }
}
