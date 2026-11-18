//! `codebase_map` publishes the basis of the graph it mapped, the same one
//! `graph_query` publishes.
//!
//! ## The defect
//!
//! The map said what the graph covers but not where the graph came from: a
//! reader could not tell which tree, which snapshot or which route (a build
//! or a cache) the counts and the hot symbols were taken from.
//!
//! ## What this file measures
//!
//! The real handler over a graph the real build route measured, read the way
//! the MCP wire carries it (the serialized output). The expected values are
//! written here from the tree and the manifest the build kept, not from the
//! code under test. The unmeasured and the replaced graphs must publish no
//! basis at all: a basis of some other graph would be a false statement.

use cognicode_core::application::graph_snapshot::{load_or_build_graph, source_digest};
use cognicode_core::application::services::analysis_service::AnalysisService;
use cognicode_core::domain::aggregates::call_graph::CallGraph;
use cognicode_core::domain::traits::graph_store::{GraphStore, StoreError};
use cognicode_core::domain::value_objects::file_manifest::FileManifest;
use cognicode_core::infrastructure::persistence::{CachedGraphStore, InMemoryGraphStore};
use cognicode_core::interface::mcp::handlers::HandlerContext;
use cognicode_core::interface::mcp::handlers::consolidated_handlers::{
    CodebaseMapInput, MapFormat, handle_codebase_map,
};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

fn write(path: &Path, contents: &str) {
    std::fs::create_dir_all(path.parent().expect("a file has a parent")).expect("directory");
    std::fs::write(path, contents).expect("the file is written");
}

/// A tree that is read whole: a function and the one that calls it.
fn readable_tree() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().to_path_buf();
    write(
        &root.join("src/lib.rs"),
        "pub fn helper() -> u32 {\n    1\n}\n\npub fn caller() -> u32 {\n    helper()\n}\n",
    );
    (dir, root)
}

/// Another tree with the same shape, so its graph is the same size as the
/// first one's and only the workspace tells them apart.
fn other_tree() -> (tempfile::TempDir, PathBuf) {
    readable_tree()
}

/// What runs on the service the moment the store hands the graph over.
type WhileItIsRead = Box<dyn FnOnce(&AnalysisService) + Send>;

/// The store the server reads through, except that another route replaces
/// the graph the service holds right after the reader took the old one:
/// the race between reading a graph and asking what was measured about it.
struct ReplacedWhileItIsRead {
    service: Arc<OnceLock<Arc<AnalysisService>>>,
    on_read: Mutex<Option<WhileItIsRead>>,
    rest: InMemoryGraphStore,
}

impl GraphStore for ReplacedWhileItIsRead {
    fn save_graph(&self, graph: &CallGraph) -> Result<(), StoreError> {
        self.rest.save_graph(graph)
    }

    fn load_graph(&self) -> Result<Option<CallGraph>, StoreError> {
        Ok(self.load_graph_shared()?.map(|graph| (*graph).clone()))
    }

    fn load_graph_shared(&self) -> Result<Option<Arc<CallGraph>>, StoreError> {
        let service = self.service.get().expect("the service is attached");
        let read = CachedGraphStore::new(service.graph_cache()).load_graph_shared()?;
        if let Some(replace) = self.on_read.lock().unwrap().take() {
            replace(service);
        }
        Ok(read)
    }

    fn save_manifest(&self, manifest: &FileManifest) -> Result<(), StoreError> {
        self.rest.save_manifest(manifest)
    }

    fn load_manifest(&self) -> Result<Option<FileManifest>, StoreError> {
        self.rest.load_manifest()
    }

    fn clear(&self) -> Result<(), StoreError> {
        self.rest.clear()
    }

    fn exists(&self) -> Result<bool, StoreError> {
        self.rest.exists()
    }
}

async fn map_of(ctx: &HandlerContext) -> Value {
    let output = handle_codebase_map(
        ctx,
        CodebaseMapInput {
            format: MapFormat::Compact,
        },
    )
    .await
    .expect("the map answers over a held graph");
    serde_json::to_value(output).expect("the answer is JSON")
}

/// The basis is the one of the graph that was read: its workspace, how it
/// was obtained and the digest of the manifest it was read against.
#[tokio::test]
async fn a_measured_graph_publishes_its_workspace_source_and_manifest_digest() {
    let (_tmp, root) = readable_tree();
    let ctx = HandlerContext::builder().with_working_dir(&root).build();
    let loaded =
        load_or_build_graph(ctx.analysis_service(), &root, None, None, false).expect("graph");

    let answer = map_of(&ctx).await;

    assert_eq!(answer["coverage"]["status"], "complete");
    let basis = &answer["basis"];
    assert_eq!(basis["graph_source"], "built");
    assert_eq!(
        basis["workspace"],
        root.canonicalize().unwrap().to_string_lossy().as_ref()
    );
    let manifest = loaded
        .manifest
        .as_ref()
        .expect("a build keeps its manifest");
    assert_eq!(
        basis["source_manifest_digest"],
        source_digest(manifest).as_str()
    );
}

/// A session that serves the graph it already holds keeps saying which
/// snapshot it is.
#[tokio::test]
async fn a_graph_the_session_already_held_keeps_its_snapshot() {
    let (_tmp, root) = readable_tree();
    let ctx = HandlerContext::builder().with_working_dir(&root).build();
    let first =
        load_or_build_graph(ctx.analysis_service(), &root, None, None, false).expect("graph");
    let manifest = first.manifest.clone().expect("manifest");
    load_or_build_graph(ctx.analysis_service(), &root, None, Some(&manifest), false)
        .expect("graph");

    let answer = map_of(&ctx).await;

    assert_eq!(
        answer["basis"]["source_manifest_digest"],
        source_digest(&manifest).as_str()
    );
}

/// A graph nobody measured has no basis to publish, and the answer says
/// the field is absent rather than inventing one.
#[tokio::test]
async fn a_graph_nobody_measured_publishes_no_basis() {
    let (_tmp, root) = readable_tree();
    // The graph is built and measured by the route that built it, and it
    // reaches the reader through a store: the service that reads it never
    // measured that graph, so it has nothing to publish about it.
    let builder = AnalysisService::new();
    builder
        .build_project_graph(&root)
        .expect("the graph is built");
    let store = InMemoryGraphStore::new();
    store
        .save_graph(&builder.get_project_graph())
        .expect("the graph is stored");
    let ctx = HandlerContext::builder()
        .with_working_dir(&root)
        .with_graph_store(store)
        .build();

    let answer = map_of(&ctx).await;

    assert_eq!(answer["coverage"]["status"], "unknown");
    assert!(
        answer.get("basis").is_none(),
        "no measure, no basis: {answer}"
    );
}

/// A tree with an ambiguous call, so the graph built from it is measured
/// with holes: the coverage published for a replacement tells which measure
/// was published.
fn tree_with_an_ambiguous_call() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().to_path_buf();
    write(
        &root.join("src/one.rs"),
        "pub fn helper() -> u32 {\n    1\n}\n",
    );
    write(
        &root.join("src/two.rs"),
        "pub fn helper() -> u32 {\n    2\n}\n",
    );
    write(
        &root.join("src/lib.rs"),
        "pub fn caller() -> u32 {\n    helper()\n}\n",
    );
    (dir, root)
}

/// A graph that replaced the measured one is described by the measure of
/// the replacement, not by the one taken for the graph it replaced.
#[tokio::test]
async fn a_graph_replaced_after_it_was_measured_publishes_the_replacement_measure() {
    let (_tmp, root) = readable_tree();
    let ctx = HandlerContext::builder().with_working_dir(&root).build();
    load_or_build_graph(ctx.analysis_service(), &root, None, None, false).expect("graph");

    // Another route replaces the graph the service holds, and measures the
    // graph it built. This one has an ambiguous call, so its measure has a
    // hole the measured graph did not have.
    let (_other, other_root) = tree_with_an_ambiguous_call();
    ctx.analysis_service()
        .build_project_graph(&other_root)
        .expect("the graph is built");

    let answer = map_of(&ctx).await;

    assert_eq!(
        answer["coverage"]["status"], "partial",
        "the coverage published is the one measured for the replacement: {answer}"
    );
    assert_eq!(
        answer["coverage"]["pending_references"]
            .as_array()
            .unwrap()
            .len(),
        1,
        "the hole of the replacement is the one published: {answer}"
    );
    assert!(
        answer.get("basis").is_none(),
        "a measure without the digest of the manifest it was read against \
         publishes no basis: {answer}"
    );
}

/// Another route measures and installs a graph of the same size while the
/// map holds the previous one. The map cannot say which graph it read, so
/// it must not publish the workspace of the one that was measured last.
#[tokio::test]
async fn a_graph_replaced_between_the_read_and_the_measure_publishes_no_basis() {
    let (_tmp, root) = readable_tree();
    let (_other, other_root) = other_tree();
    let service = Arc::new(OnceLock::new());
    let store = ReplacedWhileItIsRead {
        service: service.clone(),
        on_read: Mutex::new(None),
        rest: InMemoryGraphStore::new(),
    };
    let on_read: WhileItIsRead = Box::new(move |service| {
        load_or_build_graph(service, &other_root, None, None, false)
            .expect("the other tree is measured and installed");
    });
    *store.on_read.lock().unwrap() = Some(on_read);
    let ctx = HandlerContext::builder()
        .with_working_dir(&root)
        .with_graph_store(store)
        .build();
    service
        .set(ctx.analysis_service().clone())
        .unwrap_or_else(|_| panic!("the service is attached once"));
    load_or_build_graph(ctx.analysis_service(), &root, None, None, false).expect("graph");

    let answer = map_of(&ctx).await;

    assert_eq!(answer["coverage"]["status"], "unknown");
    assert!(
        answer.get("basis").is_none(),
        "the map read one graph and the measure belongs to another: {answer}"
    );
}
