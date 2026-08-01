//! In-Memory Bridge: load a [`CallGraph`] from PostgreSQL at binary
//! startup and hand it to the explorer in a `Arc<CallGraph>`.
//!
//! ## Design rationale
//!
//! The explorer has zero trait or adapter changes for the PG path —
//! the existing [`CallGraphRepository`](crate::adapters::CallGraphRepository)
//! adapter (a pure in-memory wrapper around `CallGraph`) is reused
//! once the graph is loaded. The `PgPool` lives only for the duration
//! of [`open_graph_from_postgres`] and is dropped on return; the
//! explorer holds nothing but the loaded `Arc<CallGraph>`.
//!
//! The bridge depends on the explorer-only [`anyhow::Error`] error
//! type (it is consumed by both binaries' `main()` functions, which
//! already return `anyhow::Result`). This keeps `cognicode-core`'s
//! [`RepositoryError`] from leaking into the explorer crate, and
//! avoids adding a `From<RepositoryError> for ExplorerError`
//! conversion that would couple the two crates.
//!
//! ## Failure semantics
//!
//! `open_graph_from_postgres` is fail-fast: any connect or load
//! error becomes a prefixed `anyhow` error and propagates. The
//! caller (binary `main()`) is expected to exit non-zero on `Err`.
//! No retry, no silent fallback to SQLite — when the operator
//! passes `--postgres`, they get a hard error on failure.

#[cfg(feature = "postgres")]
use std::sync::Arc;

#[cfg(feature = "postgres")]
use cognicode_core::domain::aggregates::CallGraph;
#[cfg(feature = "postgres")]
use cognicode_core::domain::ports::CallGraphStore;
#[cfg(feature = "postgres")]
use cognicode_core::domain::ports::PostgresCallGraphStore;
#[cfg(feature = "postgres")]
use cognicode_core::domain::ports::{PostgresRevisionStore, RevisionStore};
#[cfg(feature = "postgres")]
use cognicode_core::domain::value_objects::{RevisionId, WorkspaceId};
#[cfg(feature = "postgres")]
use cognicode_core::infrastructure::persistence::PostgresRepository;

/// Connect to PostgreSQL, run the embedded migrations, and load the
/// full call graph into memory. Returns an `Arc<CallGraph>` ready to
/// be wrapped in [`CallGraphRepository`](crate::adapters::CallGraphRepository).
///
/// - Populated DB (symbols or edges present) → `Ok(Arc<CallGraph>)`
///   with every row reconstructed through
///   `add_dependency_with_provenance` (so per-edge metadata is
///   domain-valid and round-trips bit-exact).
/// - Empty DB (both tables empty) → `Ok(Arc::new(CallGraph::new()))`.
///   The explorer serves an empty workspace; nothing errors.
/// - Connect failure or load failure → `Err(anyhow)` with a prefixed
///   message: `"open_graph_from_postgres: connect: …"` or
///   `"open_graph_from_postgres: load: …"`. The underlying
///   `PgPool` is dropped before the error is constructed.
#[cfg(feature = "postgres")]
pub async fn open_graph_from_postgres(
    database_url: &str,
    workspace_root: &std::path::Path,
) -> anyhow::Result<Arc<CallGraph>> {
    let (graph, _repo) = open_graph_with_repo(database_url, workspace_root).await?;
    Ok(graph)
}

/// Connect to PostgreSQL, run the embedded migrations, load the
/// full call graph into memory, AND return the live
/// `Arc<PostgresRepository>` so the caller can wire the
/// `named_views` CRUD methods into the explorer service.
///
/// The repo is kept alive for the lifetime of the explorer so the
/// `view_save` / `view_load` / `view_list` / `view_delete` MCP
/// tools can hit the live PG connection. The `Arc<CallGraph>` is
/// the in-memory read-only copy used by the impact_* tools.
///
/// - Failure modes are identical to [`open_graph_from_postgres`].
/// - The `Arc<PostgresRepository>` is returned even for an empty
///   DB — the table is migrated, and named-view CRUD is
///   reachable from the first request.
#[cfg(feature = "postgres")]
pub async fn open_graph_with_repo(
    database_url: &str,
    workspace_root: &std::path::Path,
) -> anyhow::Result<(Arc<CallGraph>, Arc<PostgresRepository>)> {
    let repo = PostgresRepository::new(database_url)
        .await
        .map_err(|e| anyhow::anyhow!("open_graph_from_postgres: connect: {e}"))?;

    let workspace_id = cognicode_core::application::ingest::workspace_id_for_path(workspace_root);
    let workspace = WorkspaceId::try_new(workspace_id.clone())
        .map_err(|e| anyhow::anyhow!("open_graph_from_postgres: workspace id: {e}"))?;

    // Pinned-revision is now routed through `RevisionStore::head_revision`
    // (e29-0-define-new-ports PR1 trait). The pg-side adapter
    // `PostgresRevisionStore::head_revision` issues the same `SELECT
    // MAX(revision_id) FROM graph_revisions ... HEAD_OF = true` query
    // that the inline `with_pool_async` SQL below used to issue; we
    // keep the connection pool inside the adapter instead of
    // exposing it to this bridge.
    let repo_arc: Arc<PostgresRepository> = Arc::new(repo);
    let rev_store: Arc<dyn RevisionStore> =
        Arc::new(cognicode_core::domain::ports::PostgresRevisionStore::new(
            repo_arc.with_pool(|p| p.clone()),
        ));
    let head_rev: Option<RevisionId> = rev_store
        .head_revision(&workspace)
        .await
        .map_err(|e| anyhow::anyhow!("open_graph_from_postgres: head revision: {e}"))?;

    // The `load_call_graph_ws` / `load_call_graph_current_ws` calls
    // go through the new `CallGraphStore` port (e29-0-refactor-call-sites).
    // The underlying `PostgresRepository` is kept alive so callers can
    // still hold the `Arc<PostgresRepository>` for migration + view-spec
    // CRUD paths that are not yet ported (e29-1-ladybug-adapter will
    // obsolete the raw `PostgresRepository` usage entirely).
    let cg_store: Arc<dyn CallGraphStore> = Arc::new(PostgresCallGraphStore::new(repo_arc.clone()));
    let graph = if let Some(rev) = head_rev {
        cg_store
            .load_call_graph_ws(&workspace, rev)
            .await
            .map_err(|e| anyhow::anyhow!("open_graph_from_postgres: load: {e}"))?
    } else {
        cg_store
            .load_call_graph_current(&workspace)
            .await
            .map_err(|e| anyhow::anyhow!("open_graph_from_postgres: load current workspace: {e}"))?
    };

    let graph = match graph {
        Some(g) => Arc::new(g),
        None => Arc::new(CallGraph::new()),
    };
    Ok((graph, repo_arc))
}
