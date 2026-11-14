//! A call written on a value names a method of the receiver's type.
//!
//! ## The defect
//!
//! A call written through a value (`values.len()`) carries no type in its
//! text, so the resolver had nothing to judge and joined the call to the
//! one project symbol that shares the bare name. The standard library's
//! `Vec::len` and the project's own `Holder::len` are spelled the same, so
//! every call on a `Vec` published the project's `len` call count as the
//! standard library's.
//!
//! ## What is measured
//!
//! A package with a type of its own and a method, and one file that calls
//! the method on a value of that type and calls the standard library's
//! method of the same name on a `Vec`. The project's method keeps its
//! caller; the standard library's call is not one of them.
//!
//! ## What must keep working
//!
//! - a call on a value of the project's own type keeps its edge;
//! - the three routes answer the same question the same way.

use cognicode_core::application::services::analysis_service::AnalysisService;
use cognicode_core::domain::aggregates::call_graph::CallGraph;
use cognicode_core::domain::value_objects::TraversalDirection;
use cognicode_core::infrastructure::graph::per_file_graph::PerFileGraphCache;
use cognicode_core::infrastructure::graph::{GraphStrategy, OnDemandStrategy};
use std::path::{Path, PathBuf};

/// A package with no dependencies: the type is the project's own.
const MANIFEST: &str = "\
[package]
name = \"fixture\"
";

/// A type of the project, with a method the standard library also has.
const HOLDER_RS: &str = "\
/// A type of the project, with a method the standard library also has.
pub struct Holder {
    pub value: u32,
}

impl Holder {
    /// The project's own method, spelled like the standard library's.
    pub fn len(&self) -> usize {
        1
    }
}
";

/// A file that calls the method on a value of the project's type, and calls
/// the standard library's method of the same name on a `Vec`.
const CALLER_RS: &str = "\
use crate::holder::Holder;

/// A call on a value of the project's own type.
pub fn project_len(holder: &Holder) -> usize {
    holder.len()
}

/// A call on the standard library's type, spelled the same.
pub fn standard_len(values: &Vec<u8>) -> usize {
    values.len()
}
";

/// A temporary package with the given manifest and files, and the paths of
/// the files.
fn package(manifest: &str, files: &[(&str, &str)]) -> (tempfile::TempDir, Vec<PathBuf>) {
    let dir = tempfile::tempdir().expect("a temporary package");
    std::fs::write(dir.path().join("Cargo.toml"), manifest).expect("the manifest is written");
    let src = dir.path().join("src");
    std::fs::create_dir_all(&src).expect("the source directory is created");
    let paths = files
        .iter()
        .map(|(name, content)| {
            let path = src.join(name);
            std::fs::write(&path, content).expect("the fixture is written");
            path
        })
        .collect();
    (dir, paths)
}

/// The package with the type, the method and the two calls.
fn package_with_a_value_receiver() -> (tempfile::TempDir, Vec<PathBuf>) {
    package(
        MANIFEST,
        &[
            ("lib.rs", "pub mod holder;\npub mod caller;\n"),
            ("holder.rs", HOLDER_RS),
            ("caller.rs", CALLER_RS),
        ],
    )
}

/// The graph of the project route, and what it published as unresolved.
fn project_route(dir: &Path) -> (CallGraph, usize) {
    let service = AnalysisService::new();
    service
        .build_project_graph(dir)
        .expect("the project graph is built");
    let unresolved = service
        .get_coverage_metrics()
        .expect("a built graph publishes its coverage")
        .unresolved_edges;
    ((*service.get_project_graph()).clone(), unresolved)
}

/// The graph of the per-file route.
fn per_file_route(dir: &Path, files: &[PathBuf]) -> CallGraph {
    let cache = PerFileGraphCache::with_project_dir(dir.to_string_lossy().into_owned());
    let paths: Vec<&Path> = files.iter().map(PathBuf::as_path).collect();
    cache.merge_with_report(&paths).graph
}

/// The names of the symbols that call the one declaration of `name`.
fn callers_of(graph: &CallGraph, name: &str) -> Vec<String> {
    let declared: Vec<_> = graph
        .symbol_ids()
        .filter(|(_, symbol)| symbol.name() == name)
        .map(|(id, _)| id.clone())
        .collect();
    assert_eq!(
        declared.len(),
        1,
        "the fixture declares `{name}` exactly once, and the graph holds {declared:?}"
    );
    let mut callers: Vec<String> = graph
        .callers(&declared[0])
        .iter()
        .map(|id| {
            graph
                .get_symbol(id)
                .map(|symbol| symbol.name().to_string())
                .unwrap_or_else(|| id.as_str().to_string())
        })
        .collect();
    callers.sort();
    callers
}

/// The names the on-demand route publishes as the callees of `symbol`.
fn on_demand_callees(strategy: &OnDemandStrategy, symbol: &str) -> Vec<String> {
    let result = strategy.build_subgraph(symbol, 1, TraversalDirection::Callees);
    let mut names: Vec<String> = result
        .entries
        .iter()
        .map(|entry| entry.symbol.name().to_string())
        .collect();
    names.sort();
    names
}

#[test]
fn a_standard_method_is_not_the_projects_own_call() {
    let (dir, _files) = package_with_a_value_receiver();
    let (graph, unresolved) = project_route(dir.path());

    assert_eq!(
        callers_of(&graph, "len"),
        vec!["project_len".to_string()],
        "`values.len()` is the standard library's method, and the package declares no `Vec`: the \\
         project's own `len` keeps only the call that is written on its own type"
    );
    assert!(
        unresolved >= 1,
        "the call is published as unresolved, not dropped and not joined: the coverage says \\
         {unresolved}"
    );
}

#[test]
fn the_per_file_route_does_not_join_a_standard_method_to_the_project_symbol() {
    let (dir, files) = package_with_a_value_receiver();
    let graph = per_file_route(dir.path(), &files);

    assert_eq!(
        callers_of(&graph, "len"),
        vec!["project_len".to_string()],
        "the per-file route answers like the project route"
    );
}

#[test]
fn the_on_demand_route_does_not_join_a_standard_method_to_the_project_symbol() {
    let (dir, _files) = package_with_a_value_receiver();
    let mut strategy = OnDemandStrategy::new();
    strategy
        .build_index(dir.path())
        .expect("the project is indexed");

    assert!(
        on_demand_callees(&strategy, "standard_len").is_empty(),
        "`standard_len` calls the standard library's `len`, and the package declares no `Vec`: \\
         the on-demand route must not publish the project's own `len` as its callee"
    );
}

/// A call on a value of the project's own type keeps its edge: the receiver
/// is the project's, and the method is the project's.
#[test]
fn a_call_on_the_projects_own_type_keeps_its_edge() {
    let (dir, files) = package_with_a_value_receiver();

    let (graph, _) = project_route(dir.path());
    assert!(
        callers_of(&graph, "len").contains(&"project_len".to_string()),
        "the project route: `holder.len()` is the project's own method"
    );

    let graph = per_file_route(dir.path(), &files);
    assert!(
        callers_of(&graph, "len").contains(&"project_len".to_string()),
        "the per-file route answers the same"
    );

    let mut strategy = OnDemandStrategy::new();
    strategy
        .build_index(dir.path())
        .expect("the project is indexed");
    assert_eq!(
        on_demand_callees(&strategy, "project_len"),
        vec!["len".to_string()],
        "the on-demand route answers the same"
    );
}
