//! The walks of the workspace, off the runtime that serves them.
//!
//! ## The defect
//!
//! `handle_search_content` and `handle_list_files` called the
//! application synchronously: a walk of the tree and a read of every
//! file of it, inside an `async` handler, on the thread of the runtime
//! that serves it. While the walk ran, the runtime answered nobody
//! else — not even a call that needs no disk — and the deadline that
//! was supposed to cut the call short could not fire: a deadline is
//! observed when the future is polled again, and the poll was inside
//! the walk until it finished. The answer then arrived after the
//! deadline that should have replaced it, and the work that was done
//! was thrown away.
//!
//! ## What this module does
//!
//! The walk moves to the blocking pool of the runtime. The handler
//! yields while the walk runs, so the runtime keeps serving and the
//! dispatch deadline can fire.
//!
//! What the deadline does **not** do is cancel the walk: a task in the
//! blocking pool cannot be aborted. When the deadline fires, the
//! caller is answered with an error and the walk keeps reading files
//! until it ends. The deadline bounds the wait of the caller, not the
//! work.

use crate::application::dto::{
    ListFilesRequest, ListFilesResult, RetrieveAndVerifyRequest, RetrieveAndVerifyResult,
    SearchContentRequest, SearchContentResult,
};
use crate::application::error::{AppError, AppResult};
use crate::application::services::file_operations::FileOperationsService;
use crate::application::services::verification::VERIFICATION_BUDGET;
use std::sync::Arc;
use std::time::Instant;

impl FileOperationsService {
    /// The content search, off the runtime.
    ///
    /// The search reads every file of the tree: it is the work of the
    /// `file` category, and its deadline is chosen for it.
    pub(crate) async fn search_content_off_the_runtime(
        self: &Arc<Self>,
        input: SearchContentRequest,
    ) -> AppResult<SearchContentResult> {
        let service = Arc::clone(self);
        tokio::task::spawn_blocking(move || service.search_content(input))
            .await
            .map_err(|e| AppError::InternalError(format!("the content search panicked: {e}")))?
    }

    /// The retrieval of the candidates and their verification, off the
    /// runtime.
    ///
    /// The walk of the tree is the part that occupies the runtime, and it
    /// is the part that moves: the verification that follows waits for a
    /// subprocess, so it yields while it waits.
    ///
    /// The budget starts before the walk, because the walk is part of the
    /// work the budget bounds.
    pub(crate) async fn retrieve_and_verify_off_the_runtime(
        self: &Arc<Self>,
        input: RetrieveAndVerifyRequest,
    ) -> AppResult<RetrieveAndVerifyResult> {
        let started = Instant::now();
        let service = Arc::clone(self);
        let retrieval = tokio::task::spawn_blocking(move || service.retrieve_candidates(&input))
            .await
            .map_err(|e| {
                AppError::InternalError(format!("the retrieval of the candidates panicked: {e}"))
            })??;
        Ok(self
            .verify_retrieval(retrieval, VERIFICATION_BUDGET, started)
            .await)
    }

    /// The listing of the tree, off the runtime.
    ///
    /// The listing walks the same tree the content search walks, and it
    /// answers the same question the deadline of the `file` category
    /// covers.
    pub(crate) async fn list_files_off_the_runtime(
        self: &Arc<Self>,
        input: ListFilesRequest,
    ) -> AppResult<ListFilesResult> {
        let service = Arc::clone(self);
        tokio::task::spawn_blocking(move || service.list_files(input))
            .await
            .map_err(|e| {
                AppError::InternalError(format!("the listing of the tree panicked: {e}"))
            })?
    }
}
