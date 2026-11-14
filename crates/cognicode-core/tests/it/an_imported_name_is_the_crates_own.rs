//! A name a file imports from a crate from outside is the crate's name.
//!
//! ## The defect
//!
//! The rule that judges a call written through a crate path
//! (`serde_json::to_value(1)`) only fired on the path. A file that writes
//! `use serde_json::to_value;` and then calls `to_value(1)` says the same
//! thing with the import instead of the path, and the import was not read:
//! the call was judged by its bare name, and it joined the project's own
//! `to_value` — the same false edge, in the imported form.
//!
//! ## What is measured
//!
//! A package that pulls `serde_json` from a registry, a project function
//! named `to_value`, and one call written `to_value(1)` in a file that
//! imports the name from the crate. The declaration has no callers: the
//! call is the crate's, and it is published as unresolved rather than
//! dropped or joined.
//!
//! ## What must keep working
//!
//! - an import from the project (`use crate::own::to_value;`) keeps its
//!   edge: the call still joins the project's declaration;
//! - an alias (`use serde_json::to_value as tv;`) is the crate's name too;
//! - the three routes answer the same question the same way.

use cognicode_core::application::services::analysis_service::AnalysisService;
use cognicode_core::domain::aggregates::call_graph::CallGraph;
use cognicode_core::domain::value_objects::TraversalDirection;
use cognicode_core::infrastructure::graph::per_file_graph::PerFileGraphCache;
use cognicode_core::infrastructure::graph::{GraphStrategy, OnDemandStrategy};
use std::path::{Path, PathBuf};

/// A package that pulls `serde_json` from a registry.
const MANIFEST: &str = "\
[package]
name = \"fixture\"

[dependencies]
serde_json = \"1\"
";

/// The project's own function, whose name the crate also has.
const OWN_RS: &str = "\
/// The project's own function, whose name the crate also has.
pub fn to_value() -> String {
    String::new()
}
";

/// A file that imports a name from the crate the package pulls.
const IMPORTED_RS: &str = "\
use serde_json::to_value;

/// A call to the crate's function, written with the name it imports.
pub fn from_the_crate() -> String {
    to_value(&1).to_string()
}
";

/// A file that imports a name from the crate under another name.
const ALIASED_RS: &str = "\
use serde_json::to_value as tv;

/// A call to the crate's function, written with the name it imports.
pub fn from_the_crate() -> String {
    tv(&1).to_string()
}
";

/// The project's own function, whose name the alias binds.
const ALIASED_OWN_RS: &str = "\
/// The project's own function, whose name the alias binds.
pub fn tv() -> String {
    String::new()
}
";

/// A file that imports a name from the project.
const PROJECT_RS: &str = "\
use crate::own::to_value;

/// A call to the project's own function, written with the name it imports.
pub fn from_the_project() -> String {
    to_value()
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

/// A package with the project's own function and one caller of it.
fn package_with_a_caller(caller: &str) -> (tempfile::TempDir, Vec<PathBuf>) {
    package(
        MANIFEST,
        &[
            ("lib.rs", "pub mod own;\npub mod caller;\n"),
            ("own.rs", OWN_RS),
            ("caller.rs", caller),
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
fn an_imported_name_is_not_the_projects_own_call() {
    let (dir, _files) = package_with_a_caller(IMPORTED_RS);
    let (graph, unresolved) = project_route(dir.path());

    assert_eq!(
        callers_of(&graph, "to_value"),
        Vec::<String>::new(),
        "the package pulls `serde_json` from a registry, and the file imports the name from \
         it: `to_value` is the crate's function, not the project's own"
    );
    assert!(
        unresolved >= 1,
        "the call is published as unresolved, not dropped and not joined: the coverage says \
         {unresolved}"
    );
}

#[test]
fn the_per_file_route_does_not_join_an_imported_name_to_the_project_symbol() {
    let (dir, files) = package_with_a_caller(IMPORTED_RS);
    let graph = per_file_route(dir.path(), &files);

    assert_eq!(
        callers_of(&graph, "to_value"),
        Vec::<String>::new(),
        "the per-file route answers like the project route"
    );
}

#[test]
fn the_on_demand_route_does_not_join_an_imported_name_to_the_project_symbol() {
    let (dir, _files) = package_with_a_caller(IMPORTED_RS);
    let mut strategy = OnDemandStrategy::new();
    strategy
        .build_index(dir.path())
        .expect("the project is indexed");

    assert!(
        on_demand_callees(&strategy, "from_the_crate").is_empty(),
        "`from_the_crate` calls the crate's `to_value`, and the package pulls `serde_json` \
         from a registry: the on-demand route must not publish the project's own `to_value` \
         as its callee"
    );
}

/// An alias is the same case: the name the file calls is the crate's, and
/// the collision happens with the name the alias binds.
#[test]
fn an_aliased_name_is_not_the_projects_own_call() {
    let (dir, files) = package(
        MANIFEST,
        &[
            ("lib.rs", "pub mod own;\npub mod caller;\n"),
            ("own.rs", ALIASED_OWN_RS),
            ("caller.rs", ALIASED_RS),
        ],
    );

    let (graph, _) = project_route(dir.path());
    assert_eq!(
        callers_of(&graph, "tv"),
        Vec::<String>::new(),
        "the project route: the alias names the crate's function too"
    );

    let graph = per_file_route(dir.path(), &files);
    assert_eq!(
        callers_of(&graph, "tv"),
        Vec::<String>::new(),
        "the per-file route answers the same"
    );
}

/// An import from the project keeps its edge: the name is the project's
/// own, and the call joins the project's declaration.
#[test]
fn an_import_from_the_project_keeps_its_edge() {
    let (dir, files) = package_with_a_caller(PROJECT_RS);

    let (graph, _) = project_route(dir.path());
    assert_eq!(
        callers_of(&graph, "to_value"),
        vec!["from_the_project".to_string()],
        "the project route: `use crate::own::to_value;` names the project's own function"
    );

    let graph = per_file_route(dir.path(), &files);
    assert_eq!(
        callers_of(&graph, "to_value"),
        vec!["from_the_project".to_string()],
        "the per-file route answers the same"
    );

    let mut strategy = OnDemandStrategy::new();
    strategy
        .build_index(dir.path())
        .expect("the project is indexed");
    assert_eq!(
        on_demand_callees(&strategy, "from_the_project"),
        vec!["to_value".to_string()],
        "the on-demand route answers the same"
    );
}
