//! The semantic index of the tree, populated off the runtime that serves it.
//!
//! ## The defect
//!
//! `ensure_semantic_indexed` populated the index synchronously: a walk of
//! the tree and an extraction of the symbols of every file of it, inside
//! the `async` handler, on the thread of the runtime that serves it.
//! While the population ran, the runtime answered nobody — not even a
//! call that needs no disk — and the deadline that was supposed to cut
//! the call short could not fire: a deadline is observed when the future
//! is polled again, and the poll was inside the population until it
//! finished. The answer then arrived after the deadline that should have
//! replaced it, and the work that was done was thrown away.
//!
//! ## What this module does
//!
//! The population moves to the blocking pool of the runtime, the same way
//! the walk of the content search and the build of the graph do. The
//! handler yields while the index is populated, so the runtime keeps
//! serving and the dispatch deadline can fire.
//!
//! What the deadline does **not** do is cancel the population: a task in
//! the blocking pool cannot be aborted. When the deadline fires, the
//! caller is answered with an error and the index is populated all the
//! same — the next call finds it ready. The deadline bounds the wait of
//! the caller, not the work.
//!
//! The window that covers this work is the one of the `graph` category:
//! the tools that populate the index need the graph as well, and the
//! build of the graph is the more expensive of the two.

use std::path::PathBuf;
use std::sync::Arc;

use super::{EnsureResult, HandlerContext, HandlerError, HandlerResult};
use crate::infrastructure::semantic::semantic_search::SemanticSearchService;

/// The semantic index of the tree, populated if it is not populated yet.
///
/// The index is populated once per session: the second call finds it
/// ready and answers without touching the tree.
pub(super) async fn ensure_semantic_indexed(ctx: &HandlerContext) -> HandlerResult<EnsureResult> {
    ensure_semantic_indexed_with_services(
        ctx.semantic_search.clone(),
        ctx.working_dir().to_path_buf(),
    )
    .await
}

/// The same work, with the service and the directory the caller names.
pub(super) async fn ensure_semantic_indexed_with_services(
    semantic_search: Arc<SemanticSearchService>,
    working_dir: PathBuf,
) -> HandlerResult<EnsureResult> {
    if !semantic_search.index().is_empty() {
        return Ok(EnsureResult::already_present(semantic_search.index().len()));
    }
    let start = std::time::Instant::now();
    let service = Arc::clone(&semantic_search);
    tokio::task::spawn_blocking(move || service.populate_from_directory(&working_dir))
        .await
        .map_err(|e| {
            HandlerError::Internal(format!(
                "the population of the semantic index panicked: {e}"
            ))
        })?
        .map_err(HandlerError::Internal)?;
    let elapsed = start.elapsed().as_millis() as u64;
    Ok(EnsureResult::auto_built(
        semantic_search.index().len(),
        elapsed,
    ))
}
