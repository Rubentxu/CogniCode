//! `get_implementors` against versioned corpora, with no language server.
//!
//! Each directory under `sandbox/fixtures/<lang>` carries an
//! `inherits.golden` written by hand: one `child : parent` pair per line,
//! for every parent the sources declare inside the corpus. The test builds
//! the project graph the product builds and requires `get_implementors` to
//! return exactly the children the golden lists, for every parent. A parent
//! the corpus does not declare (`Serializable`, `IDisposable`) has no
//! declaration to point at, so nothing may be reported for it.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use cognicode_core::domain::traits::GraphStore;
use cognicode_core::infrastructure::graph::per_file_graph::PerFileGraphCache;
use cognicode_core::infrastructure::persistence::memory_graph_store::InMemoryGraphStore;
use cognicode_core::interface::mcp::handlers::HandlerContext;
use cognicode_core::interface::mcp::handlers::graph_query_handlers::{
    GetImplementorsInput, handle_get_implementors,
};

fn corpus_dir(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../sandbox/fixtures")
        .join(name)
        .canonicalize()
        .expect("the fixture directory is versioned")
}

/// Source files of the corpus, sorted, filtered by extension.
fn sources(dir: &Path, extension: &str) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some(extension))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no .{extension} files in {dir:?}");
    files
}

/// `parent -> children`, from the hand-written golden.
fn golden(dir: &Path) -> BTreeMap<String, BTreeSet<String>> {
    let text = std::fs::read_to_string(dir.join("inherits.golden")).unwrap();
    let mut pairs: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for line in text.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (child, parent) = line.split_once(" : ").expect("`child : parent` line");
        pairs
            .entry(parent.to_string())
            .or_default()
            .insert(child.to_string());
    }
    assert!(!pairs.is_empty(), "empty golden in {dir:?}");
    pairs
}

fn context_for(files: &[PathBuf]) -> HandlerContext {
    let cache = PerFileGraphCache::new();
    let paths: Vec<&Path> = files.iter().map(|p| p.as_path()).collect();
    let report = cache.merge_with_report(&paths);
    let store = InMemoryGraphStore::new();
    store.save_graph(&report.graph).unwrap();
    HandlerContext::builder().with_graph_store(store).build()
}

async fn implementors(ctx: &HandlerContext, parent: &str) -> BTreeSet<String> {
    handle_get_implementors(
        ctx,
        GetImplementorsInput {
            trait_name: parent.to_string(),
        },
    )
    .await
    .map(|out| out.implementors.into_iter().collect())
    .unwrap_or_default()
}

async fn check(name: &str, extension: &str, external_parents: &[&str]) {
    let dir = corpus_dir(name);
    let expected = golden(&dir);
    let ctx = context_for(&sources(&dir, extension));

    for (parent, children) in &expected {
        assert_eq!(
            &implementors(&ctx, parent).await,
            children,
            "{name}: implementors of {parent}"
        );
    }
    // A child that nothing extends must not turn up as anyone's parent.
    let children: BTreeSet<&String> = expected.values().flatten().collect();
    for child in children.into_iter().filter(|c| !expected.contains_key(*c)) {
        assert!(
            implementors(&ctx, child).await.is_empty(),
            "{name}: {child} has no implementors"
        );
    }
    for external in external_parents {
        assert!(
            implementors(&ctx, external).await.is_empty(),
            "{name}: {external} is not declared in the corpus"
        );
    }
}

#[tokio::test]
async fn kotlin_golden_without_lsp() {
    check("kotlin", "kt", &[]).await;
}

#[tokio::test]
async fn groovy_golden_without_lsp() {
    check("groovy", "groovy", &["Serializable"]).await;
}

#[tokio::test]
async fn csharp_golden_without_lsp() {
    check("csharp", "cs", &["IDisposable"]).await;
}
