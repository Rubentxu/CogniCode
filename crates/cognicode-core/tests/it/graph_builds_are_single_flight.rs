//! A cold graph is built once, and the building does not stop the server.
//!
//! ## The two gaps
//!
//! Every query tool calls `ensure_graph_built` before it reads the graph, and
//! `build_graph` calls the same decision (`load_or_build_graph`) directly.
//! When the session holds nothing, that decision parses the whole tree. Two
//! things were wrong with how the handlers asked for it:
//!
//! - **nothing coalesced the requests**: four agents asking four questions of
//!   the same cold workspace started four builds of the same tree, each one
//!   publishing its own graph over the others;
//! - **the build ran on the thread that serves the answers**: the handlers
//!   are `async`, the build is CPU and disk, and a build that never yields
//!   holds the runtime's thread for as long as it takes. A server on a small
//!   runtime answers nothing, not even a request that needs no graph.
//!
//! ## What this file pins
//!
//! - concurrent requests for the same cold tree share ONE build, and every
//!   one of them receives the graph that build published;
//! - a build that fails is not kept as an answer: the next request builds
//!   again, and sees the tree as it is by then;
//! - while a build is running, a request that needs no graph is answered.
//!
//! ## How the tests stay deterministic
//!
//! The build is held at the point where it publishes its graph, through a
//! `SharedGraph` double injected into the service. That double also counts
//! the publications, which is the observable "how many builds ran". Nothing
//! here sleeps to synchronise: the test releases the build when the service
//! says the other callers are parked behind it, and every wait carries its
//! own deadline, so a regression fails the test instead of hanging it.

use cognicode_core::application::analysis_basis::GraphSource;
use cognicode_core::application::graph_snapshot::load_or_build_graph;
use cognicode_core::application::ports::SharedGraph;
use cognicode_core::application::services::analysis_service::AnalysisService;
use cognicode_core::domain::aggregates::CallGraph;
use cognicode_core::domain::events::GraphEvent;
use cognicode_core::domain::value_objects::CheckpointId;
use cognicode_core::infrastructure::graph::GraphCache;
use cognicode_core::interface::mcp::handlers::{
    BuildGraphInput, HandlerContext, handle_build_graph, handle_get_complexity,
    handle_get_entry_points,
};
use cognicode_core::interface::mcp::schemas::{GetComplexityInput, GetEntryPointsInput};
use std::future::Future;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

/// How long a step that must happen is given to happen. The healthy path
/// takes milliseconds; the deadline only decides how long a regression
/// takes to be reported.
const DEADLINE: Duration = Duration::from_secs(10);

/// The graph cache of the service, with the publication held back until the
/// test lets it through.
///
/// A build publishes its graph once, as its last step. Holding that step
/// keeps a build "running" for exactly as long as the test needs, and the
/// number of times it was reached is the number of builds that ran.
struct HeldGraph {
    inner: GraphCache,
    publications: AtomicUsize,
    released: Mutex<bool>,
    release_signal: Condvar,
}

impl HeldGraph {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: GraphCache::new(),
            publications: AtomicUsize::new(0),
            released: Mutex::new(false),
            release_signal: Condvar::new(),
        })
    }

    /// Lets every held publication through, now and from here on.
    fn release(&self) {
        *self.released.lock().unwrap() = true;
        self.release_signal.notify_all();
    }

    /// How many builds reached their publication.
    fn publications(&self) -> usize {
        self.publications.load(Ordering::SeqCst)
    }
}

impl SharedGraph for HeldGraph {
    fn get(&self) -> Arc<CallGraph> {
        self.inner.get()
    }

    fn replace(&self, graph: CallGraph) {
        self.publications.fetch_add(1, Ordering::SeqCst);
        let released = self.released.lock().unwrap();
        let (_released, timeout) = self
            .release_signal
            .wait_timeout_while(released, DEADLINE * 3, |released| !*released)
            .unwrap();
        assert!(
            !timeout.timed_out(),
            "the test never released the build: the publication was held for too long"
        );
        self.inner.set(graph);
    }

    fn current_id(&self) -> Option<CheckpointId> {
        self.inner.current_id()
    }

    fn get_at(&self, id: CheckpointId) -> Option<Arc<CallGraph>> {
        self.inner.get_at(id)
    }

    fn subscribe(&self) -> tokio::sync::broadcast::Receiver<GraphEvent> {
        self.inner.subscribe()
    }
}

/// Spins until `condition` holds or the deadline passes; says which.
fn becomes_true_within_the_deadline(condition: impl Fn() -> bool) -> bool {
    let started = Instant::now();
    while !condition() {
        if started.elapsed() > DEADLINE {
            return false;
        }
        std::thread::yield_now();
    }
    true
}

/// A workspace with one caller and one callee, and the temporary root that
/// holds it and the cache.
fn workspace_with_root() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let root = tempfile::tempdir().expect("a temporary root");
    let ws = root.path().join("ws");
    std::fs::create_dir_all(&ws).expect("the workspace is created");
    std::fs::write(
        ws.join("a.rs"),
        "fn callee() -> u32 { 1 }\nfn caller() -> u32 { callee() }\n",
    )
    .expect("the source is written");
    let cache = root.path().join("cache");
    (root, ws, cache)
}

#[test]
fn concurrent_requests_for_a_cold_tree_share_one_build() {
    const CALLERS: usize = 4;
    let (_root, ws, cache) = workspace_with_root();
    let held = HeldGraph::new();
    let service = AnalysisService::with_graph_cache(held.clone());

    let (queued, results) = std::thread::scope(|scope| {
        let callers: Vec<_> = (0..CALLERS)
            .map(|_| scope.spawn(|| load_or_build_graph(&service, &ws, Some(&cache), None, false)))
            .collect();

        // One caller builds; the rest are parked behind it. Only then is the
        // build let go, so the callers cannot miss each other by timing.
        let queued =
            becomes_true_within_the_deadline(|| service.graph_builds().waiting(&ws) == CALLERS - 1);
        held.release();

        let results: Vec<_> = callers
            .into_iter()
            .map(|caller| caller.join().expect("the caller does not panic"))
            .collect();
        (queued, results)
    });

    assert!(
        queued,
        "{} callers asked for the same cold tree and only {} queued behind a build \
         ({} builds reached their publication)",
        CALLERS,
        service.graph_builds().waiting(&ws),
        held.publications()
    );
    assert_eq!(
        held.publications(),
        1,
        "the tree was parsed once, not once per caller"
    );
    assert_eq!(
        service.current_graph_revision(),
        1,
        "one graph was published, not one per caller"
    );

    let loaded: Vec<_> = results
        .into_iter()
        .map(|result| result.expect("every caller is answered"))
        .collect();
    for answer in &loaded {
        assert!(
            Arc::ptr_eq(&answer.graph, &loaded[0].graph),
            "every caller receives the graph that one build published"
        );
        assert_eq!(answer.source, GraphSource::Built);
        assert!(
            answer.graph.symbol_count() >= 2,
            "the shared graph is the built one, not an empty placeholder"
        );
    }
    assert_eq!(
        service.graph_builds().waiting(&ws),
        0,
        "the flight left the map once the build had answered"
    );
}

#[test]
fn a_failed_build_is_not_kept_as_an_answer() {
    const CALLERS: usize = 4;
    let root = tempfile::tempdir().expect("a temporary root");
    let ws = root.path().join("not-there-yet");
    let service = AnalysisService::new();

    let results: Vec<_> = std::thread::scope(|scope| {
        let callers: Vec<_> = (0..CALLERS)
            .map(|_| scope.spawn(|| load_or_build_graph(&service, &ws, None, None, false)))
            .collect();
        callers
            .into_iter()
            .map(|caller| caller.join().expect("the caller does not panic"))
            .collect()
    });
    assert!(
        results.iter().all(|result| result.is_err()),
        "a tree that is not there is a failure for every caller that asked"
    );

    // The tree appears. A failure that had been kept would be served again;
    // the next request builds what is there now.
    std::fs::create_dir_all(&ws).expect("the workspace is created");
    std::fs::write(ws.join("a.rs"), "fn only() -> u32 { 1 }\n").expect("the source is written");

    let answer = load_or_build_graph(&service, &ws, None, None, false)
        .expect("the tree exists now, so the build answers");
    assert_eq!(answer.source, GraphSource::Built);
    assert!(
        answer.graph.symbol_count() >= 1,
        "the answer describes the tree as it is now"
    );
}

/// Runs `heavy` on a one-thread runtime, with a request that needs no graph
/// asked beside it once the build is under way, and says whether that
/// request was answered while the build was still held.
///
/// The runtime has ONE thread on purpose: with several, a build that blocks
/// its thread would leave the others free to answer, and the test would
/// pass over exactly the defect it exists to catch.
fn a_light_request_is_answered_while_the_build_runs<H, F>(heavy: H)
where
    H: FnOnce(HandlerContext) -> F + Send + 'static,
    F: Future<Output = ()>,
{
    let (_root, ws, cache) = workspace_with_root();
    let held = HeldGraph::new();
    let ctx = HandlerContext::builder()
        .with_working_dir(ws.clone())
        .with_cache_root(cache)
        .with_analysis_service(AnalysisService::with_graph_cache(held.clone()))
        .build();
    let light_file = ws.join("a.rs").to_string_lossy().into_owned();

    let give_up = Arc::new(AtomicBool::new(false));
    let (answered_tx, answered_rx) = mpsc::channel();
    let (finished_tx, finished_rx) = mpsc::channel();

    let runtime = {
        let held = held.clone();
        let give_up = give_up.clone();
        std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("a runtime");
            runtime.block_on(async {
                let light_ctx = ctx.clone();
                let light = async {
                    // The light request is asked once a build has reached
                    // its publication: that is, while the build is running.
                    while held.publications() == 0 && !give_up.load(Ordering::SeqCst) {
                        tokio::task::yield_now().await;
                    }
                    let answer = handle_get_complexity(
                        &light_ctx,
                        GetComplexityInput {
                            file_path: light_file,
                            function_name: None,
                        },
                    )
                    .await;
                    let _ = answered_tx.send(answer.is_ok());
                };
                tokio::join!(heavy(ctx), light);
            });
            let _ = finished_tx.send(());
        })
    };

    let answered = answered_rx.recv_timeout(DEADLINE);

    // Whatever happened, the build is let go: a failed assertion below must
    // not leave a thread parked.
    give_up.store(true, Ordering::SeqCst);
    held.release();
    finished_rx
        .recv_timeout(DEADLINE)
        .expect("the heavy request finishes once the build is released");
    runtime.join().expect("the runtime thread does not panic");

    assert_eq!(
        answered,
        Ok(true),
        "a request that needs no graph is answered while a build is running; \
         the build held the only thread of the runtime instead"
    );
}

#[test]
fn build_graph_leaves_the_runtime_free_while_it_builds() {
    a_light_request_is_answered_while_the_build_runs(|ctx| async move {
        handle_build_graph(&ctx, BuildGraphInput { directory: None })
            .await
            .expect("the build answers");
    });
}

#[test]
fn a_query_tool_leaves_the_runtime_free_while_it_builds_the_graph() {
    a_light_request_is_answered_while_the_build_runs(|ctx| async move {
        handle_get_entry_points(&ctx, GetEntryPointsInput { compressed: false })
            .await
            .expect("the query answers");
    });
}
