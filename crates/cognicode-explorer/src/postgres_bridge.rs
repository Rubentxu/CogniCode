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
pub async fn open_graph_from_postgres(database_url: &str) -> anyhow::Result<Arc<CallGraph>> {
    let (graph, _repo) = open_graph_with_repo(database_url).await?;
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
) -> anyhow::Result<(Arc<CallGraph>, Arc<PostgresRepository>)> {
    let repo = PostgresRepository::new(database_url)
        .await
        .map_err(|e| anyhow::anyhow!("open_graph_from_postgres: connect: {e}"))?;

    let graph = repo
        .load_call_graph()
        .await
        .map_err(|e| anyhow::anyhow!("open_graph_from_postgres: load: {e}"))?;

    let graph = match graph {
        Some(g) => Arc::new(g),
        None => Arc::new(CallGraph::new()),
    };
    let repo = Arc::new(repo);
    Ok((graph, repo))
}
