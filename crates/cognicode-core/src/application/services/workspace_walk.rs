//! The walks of the workspace, off the runtime that serves them.
//!
//! ## The defect
//!
//! `handle_search_content` and `handle_list_files` called the
//! application synchronously: a walk of the tree —and, for the content
//! search, a read of every file of it— inside an `async` handler, on
//! the thread of the runtime that serves it. The listing collects the
//! entries of the tree; the content search is the one that reads the
//! files. While the walk ran, the runtime answered nobody
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
//!
//! The retrieval of the candidates is the exception, and it is the
//! exception because its work has a bound of its own: the walk it does
//! stops when the budget of the call is reached, and the answer says that
//! it stopped (`application/services/verification.rs` owns that bound).

use crate::application::dto::{
    ListFilesRequest, ListFilesResult, SearchContentRequest, SearchContentResult,
};
use crate::application::error::{AppError, AppResult};
use crate::application::services::file_operations::FileOperationsService;
use std::sync::Arc;

/// Runs the work off the thread that awaits it.
///
/// The walk of the tree is blocking I/O: the thread that awaits the answer
/// has other calls to serve, and the work belongs to the blocking pool of
/// the runtime.
///
/// When there is no runtime to send it to — a consumer that awaits a
/// published route with another executor — the work happens where it
/// always happened. There is no pool to move it to, and `spawn_blocking`
/// outside a runtime is a panic, not a slow answer.
pub(crate) async fn off_the_runtime<T: Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
) -> Result<T, tokio::task::JoinError> {
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => handle.spawn_blocking(work).await,
        Err(_) => Ok(work()),
    }
}

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
        off_the_runtime(move || service.search_content(input))
            .await
            .map_err(|e| AppError::InternalError(format!("the content search panicked: {e}")))?
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
        off_the_runtime(move || service.list_files(input))
            .await
            .map_err(|e| {
                AppError::InternalError(format!("the listing of the tree panicked: {e}"))
            })?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::future::Future;
    use std::task::{Context, Poll};

    /// Awaits a future that does not yield.
    ///
    /// The work of this module does not yield when there is no runtime to
    /// send it to, and this is how a future that does not yield is
    /// awaited: one poll, and the answer is there.
    fn block_on_once<F: Future>(future: F) -> F::Output {
        let mut future = Box::pin(future);
        let waker = std::task::Waker::noop();
        let mut context = Context::from_waker(waker);
        match future.as_mut().poll(&mut context) {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("the future yielded, and this waits for one that does not"),
        }
    }

    /// With a runtime, the work runs on another thread.
    ///
    /// The thread that awaits the answer has other calls to serve, and the
    /// walk of the tree is not one of them.
    #[tokio::test(flavor = "current_thread")]
    async fn the_work_runs_on_another_thread_when_there_is_a_runtime() {
        let caller = std::thread::current().id();
        let worker = off_the_runtime(|| std::thread::current().id())
            .await
            .expect("the work ran");

        assert_ne!(
            caller, worker,
            "the work ran on the thread that awaits it: {caller:?}"
        );
    }

    /// Without a runtime, the work runs here, and the answer still comes.
    ///
    /// A plain test has no runtime: `spawn_blocking` outside one is a
    /// panic, and a published route that a consumer awaits with another
    /// executor must answer all the same.
    #[test]
    fn the_work_runs_here_when_there_is_no_runtime_to_send_it_to() {
        assert!(
            tokio::runtime::Handle::try_current().is_err(),
            "this test has no runtime, and the case it measures is that one"
        );

        let value = block_on_once(off_the_runtime(|| 7_u32)).expect("the work ran");
        assert_eq!(value, 7, "the answer came: {value}");
    }
}
