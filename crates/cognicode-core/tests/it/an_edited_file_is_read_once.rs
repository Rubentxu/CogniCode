//! A file that was edited is read once, and the index and the graph hear the
//! same reading.
//!
//! ## The gap
//!
//! `reparse_on_edit` answers one question: "this file changed, bring what you
//! know about it up to date". It used to answer it with two readings of the
//! same bytes. The first fed the search index with the generic reader of the
//! language; the second was the graph rebuild, which forgot the file and read
//! it again with the reader the graph uses. Two readings cost twice, and they
//! can disagree: a language whose own reader knows declarations the generic
//! one does not (a Terraform block, an Ansible task) was searchable under one
//! set of symbols and navigable under another.
//!
//! The index also published the place of every symbol it learned this way as
//! `source`, the placeholder of a reader that was never told the path, so a
//! result could not be opened by whoever asked.
//!
//! ## What this file pins
//!
//! - the edited file is read ONCE by the reader the graph uses, and the files
//!   that did not change are not read at all;
//! - the index is fed by that same reading: a symbol only the graph's reader
//!   declares is found by search, and it carries the real path of the file;
//! - the rebuild still says it was partial: the file read a moment before the
//!   build is one of the files interpreted since the graph it replaces;
//! - the graph that comes out is the graph of a full rebuild of the same tree
//!   —the same symbols and the same relations, and, compared apart, in the
//!   same order;
//! - a file the graph cannot read (a syntax error) still gets the answer it
//!   always got: the call succeeds, and the file is not published.
//!
//! ## How the reads are counted
//!
//! The reader is a seam of the service (`with_facts_reader`). The double wraps
//! the real reader, counts the calls by path, and declares one symbol more
//! than the source has: it stands for any symbol that only the graph's reader
//! knows. Nothing here sleeps, and every call carries its own deadline.

#![cfg(feature = "persistence")]

use cognicode_core::application::services::analysis_service::AnalysisService;
use cognicode_core::domain::aggregates::{CallGraph, Symbol};
use cognicode_core::domain::value_objects::{Location, SymbolKind};
use cognicode_core::infrastructure::graph::semantic_layer::read_file_facts;
use cognicode_core::interface::mcp::handlers::aix_handlers::handle_reparse_on_edit;
use cognicode_core::interface::mcp::handlers::consolidated_handlers::handle_smart_search;
use cognicode_core::interface::mcp::handlers::{
    BuildGraphInput, HandlerContext, handle_build_graph,
};
use cognicode_core::interface::mcp::schemas::{ReparseOnEditInput, SmartSearchInput};
use serde_json::Value;
use std::collections::{BTreeSet, HashMap};
use std::future::Future;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// How long a call that must answer is given to answer. The healthy path takes
/// milliseconds; the deadline only decides how long a regression takes to be
/// reported.
const DEADLINE: Duration = Duration::from_secs(30);

/// The symbol only the graph's reader declares, by file.
fn declared_only_by_the_graph(file: &str) -> String {
    format!("declared_only_by_the_graph_reader_in_{file}")
}

/// Runs `call` under the deadline, so a hang is a failure and not a stall.
async fn within_the_deadline<T>(call: impl Future<Output = T>) -> T {
    tokio::time::timeout(DEADLINE, call)
        .await
        .expect("the call did not answer within the deadline")
}

/// The reads the service made, by path.
#[derive(Clone, Default)]
struct Reads(Arc<Mutex<HashMap<String, usize>>>);

impl Reads {
    /// Forgets what was counted: what follows is what one call did.
    fn forget(&self) {
        self.0.lock().unwrap().clear();
    }

    /// How many times the files whose path ends with `suffix` were read.
    fn of(&self, suffix: &str) -> usize {
        self.0
            .lock()
            .unwrap()
            .iter()
            .filter(|(path, _)| path.ends_with(suffix))
            .map(|(_, count)| count)
            .sum()
    }

    /// How many reads there were in all.
    fn total(&self) -> usize {
        self.0.lock().unwrap().values().sum()
    }
}

/// A service whose reader counts its calls and declares one symbol more than
/// the source has.
fn counting_service(reads: &Reads) -> AnalysisService {
    let reads = reads.clone();
    AnalysisService::new().with_facts_reader(move |language, source, path| {
        *reads.0.lock().unwrap().entry(path.to_string()).or_default() += 1;
        let mut facts = read_file_facts(language, source, path)?;
        let file = Path::new(path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(path);
        facts.symbols.push(Symbol::new(
            declared_only_by_the_graph(file),
            SymbolKind::Function,
            Location::new(path, 1, 1),
        ));
        Ok(facts)
    })
}

/// A workspace of `files` Rust files, built into a graph and ready to be
/// edited.
async fn analysed_workspace(
    files: usize,
    service: Option<AnalysisService>,
) -> (tempfile::TempDir, HandlerContext) {
    let workspace = tempfile::tempdir().expect("a temporary workspace");
    let src = workspace.path().join("src");
    std::fs::create_dir_all(&src).expect("the directory is created");
    for index in 0..files {
        std::fs::write(
            src.join(format!("file_{index}.rs")),
            format!("fn funcion_{index}() {{}}\nfn otra_{index}() {{ funcion_{index}() }}\n"),
        )
        .expect("the source is written");
    }

    let builder = HandlerContext::builder().with_working_dir(workspace.path().to_path_buf());
    let ctx = match service {
        Some(service) => builder.with_analysis_service(service).build(),
        None => builder.build(),
    };
    let build = within_the_deadline(handle_build_graph(
        &ctx,
        BuildGraphInput { directory: None },
    ))
    .await
    .expect("build_graph answers");
    assert_eq!(
        build.status, "complete",
        "the graph has to be complete first: otherwise the manifest is not saved and \
         the files are treated as new instead of as modified"
    );
    (workspace, ctx)
}

/// What the tool publishes, as the client sees it.
async fn reparse(ctx: &HandlerContext, files: &[&str]) -> Value {
    let output = within_the_deadline(handle_reparse_on_edit(
        ctx,
        ReparseOnEditInput {
            file_paths: files.iter().map(|file| file.to_string()).collect(),
            edit_ranges: None,
        },
    ))
    .await
    .expect("reparse_on_edit answers");
    serde_json::to_value(&output).expect("the output is JSON")
}

/// The symbols of the graph, by identity.
fn symbols_of(graph: &CallGraph) -> Vec<String> {
    graph
        .symbols()
        .map(|symbol| symbol.fully_qualified_name().to_string())
        .collect()
}

/// The relations of the graph as `(source, target, kind)`.
fn relations_of(graph: &CallGraph) -> Vec<(String, String, String)> {
    graph
        .all_dependencies()
        .map(|(source, target, kind)| {
            (
                source.as_str().to_string(),
                target.as_str().to_string(),
                format!("{kind:?}"),
            )
        })
        .collect()
}

fn as_set<T: Ord + Clone>(items: &[T]) -> BTreeSet<T> {
    items.iter().cloned().collect()
}

#[tokio::test]
async fn an_edited_file_is_read_once_and_the_index_hears_the_same_reading() {
    let reads = Reads::default();
    let (workspace, ctx) = analysed_workspace(4, Some(counting_service(&reads))).await;

    std::fs::write(
        workspace.path().join("src/file_1.rs"),
        "fn renamed_1() {}\nfn otra_1() { renamed_1() }\nfn another_1() {}\n",
    )
    .expect("the file is edited");
    reads.forget();

    let published = reparse(&ctx, &["src/file_1.rs"]).await;

    assert_eq!(
        reads.of("src/file_1.rs"),
        1,
        "the edited file was read by the graph's reader {} times; reading the same \
         bytes twice is the cost of editing one file",
        reads.of("src/file_1.rs")
    );
    assert_eq!(
        reads.total(),
        1,
        "the files that did not change were read again: {:?}",
        reads.0.lock().unwrap()
    );
    assert_eq!(
        published["cost"]["rebuild_kind"], "partial",
        "one file was interpreted and the rest reused: {published}"
    );

    // The index heard the reading the graph used.
    let marker = declared_only_by_the_graph("file_1.rs");
    let answer = within_the_deadline(handle_smart_search(
        &ctx,
        SmartSearchInput {
            query: marker.clone(),
            limit: Some(5),
        },
    ))
    .await
    .expect("smart_search answers");
    let hit = answer
        .results
        .iter()
        .find(|result| result.name == marker)
        .unwrap_or_else(|| panic!("search does not know {marker}: {:?}", answer.results));
    assert_eq!(
        hit.source, "semantic",
        "the symbol the graph's reader declared reached the graph but not the index: \
         the index was fed by another reading of the same file"
    );
    let file = hit.file.as_deref().unwrap_or_default();
    assert!(
        file.ends_with("src/file_1.rs"),
        "the index published {file:?} as the place of the symbol; a place nobody can \
         open is not a place"
    );
}

#[tokio::test]
async fn the_graph_after_the_edit_is_the_graph_of_a_full_rebuild() {
    let (workspace, ctx) = analysed_workspace(5, None).await;

    // One file gains a declaration and a call to another file, and one is
    // renamed, so the rebuild has relations to keep, to change and to add.
    std::fs::write(
        workspace.path().join("src/file_2.rs"),
        "fn funcion_2() { funcion_3() }\nfn nueva_2() { funcion_2() }\n",
    )
    .expect("the file is edited");
    reparse(&ctx, &["src/file_2.rs"]).await;

    let after_the_edit = ctx.analysis_service().get_project_graph();
    let fresh = AnalysisService::new();
    fresh
        .build_project_graph(workspace.path())
        .expect("the full rebuild answers");
    let rebuilt = fresh.get_project_graph();

    let (edited_symbols, rebuilt_symbols) = (symbols_of(&after_the_edit), symbols_of(&rebuilt));
    assert_eq!(
        as_set(&edited_symbols),
        as_set(&rebuilt_symbols),
        "the edit left a graph with other symbols than a full rebuild of the same tree"
    );
    let (edited_relations, rebuilt_relations) =
        (relations_of(&after_the_edit), relations_of(&rebuilt));
    assert_eq!(
        as_set(&edited_relations),
        as_set(&rebuilt_relations),
        "the edit left a graph with other relations than a full rebuild of the same tree"
    );
    assert!(
        !rebuilt_relations.is_empty(),
        "the corpus has relations to compare"
    );

    // The content is the same. The order is another question, and it is asked
    // apart: a graph that lists the same things in another order is a
    // different defect from a graph that lists other things.
    assert_eq!(
        edited_symbols, rebuilt_symbols,
        "same symbols, published in another order than a full rebuild"
    );
    assert_eq!(
        edited_relations, rebuilt_relations,
        "same relations, published in another order than a full rebuild"
    );
}

#[tokio::test]
async fn a_file_the_graph_cannot_read_is_not_published_and_the_call_still_answers() {
    let (workspace, ctx) = analysed_workspace(3, None).await;

    std::fs::write(
        workspace.path().join("src/file_1.rs"),
        "fn broken( {\n    let = ;\n",
    )
    .expect("the file is broken");
    let published = reparse(&ctx, &["src/file_1.rs"]).await;
    assert!(
        published["cost"]["rebuild_kind"].is_string(),
        "the call says what the rebuild was: {published}"
    );

    let names: Vec<String> = ctx
        .analysis_service()
        .get_project_graph()
        .symbols()
        .map(|symbol| symbol.name().to_string())
        .collect();
    assert!(
        !names
            .iter()
            .any(|name| name == "funcion_1" || name == "otra_1"),
        "a file the reader could not read whole kept publishing what it used to declare: \
         {names:?}"
    );
    assert!(
        names.iter().any(|name| name == "funcion_0")
            && names.iter().any(|name| name == "funcion_2"),
        "the files that were not touched are still published: {names:?}"
    );
}
