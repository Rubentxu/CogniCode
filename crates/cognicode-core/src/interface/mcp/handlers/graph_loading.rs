//! Where a handler gets the graph of a tree: off the runtime that serves it.
//!
//! ## Why it has its own home
//!
//! Deciding the graph reads the snapshot and, when it cannot answer for the
//! tree, parses every file of it: seconds of CPU and disk that never yield.
//! The handlers are `async`, so doing that on the task that serves them holds
//! the runtime's thread for as long as it takes, and a server on a small
//! runtime answers nothing meanwhile —not even a request that needs no
//! graph.
//!
//! The decision itself is the application's `load_or_build_graph`, which also
//! coalesces concurrent requests for the same tree into one build. This
//! module only moves it off the runtime, and every handler that needs a graph
//! goes through it: a handler that called the application directly would
//! bring the stall back.

use std::path::{Path, PathBuf};

use crate::application::graph_snapshot::{LoadedGraph, load_or_build_graph};

use super::{HandlerContext, HandlerError, HandlerResult};

/// The graph of `directory`, decided on a thread that may block.
pub(super) async fn load_graph_off_the_runtime(
    ctx: &HandlerContext,
    directory: PathBuf,
) -> HandlerResult<LoadedGraph> {
    let service = ctx.analysis_service.clone();
    let cache_root = ctx.cache_root().map(Path::to_path_buf);
    let session_manifest = ctx.get_graph_store().load_manifest().ok().flatten();
    tokio::task::spawn_blocking(move || {
        load_or_build_graph(
            &service,
            &directory,
            cache_root.as_deref(),
            session_manifest.as_ref(),
            false,
        )
    })
    .await
    .map_err(|e| HandlerError::Internal(format!("spawn_blocking panicked: {e}")))?
    .map_err(HandlerError::App)
}
