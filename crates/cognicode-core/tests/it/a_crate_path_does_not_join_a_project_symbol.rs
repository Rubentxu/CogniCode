//! A call written through a crate the project pulls is not a call to the
//! project's symbol of the same name.
//!
//! ## The defect
//!
//! The type qualifier (`Duration::from_millis`) is judged: a capitalised
//! name the project never declares belongs to somebody else. A crate path is
//! the same fact written the other way round — `serde_json::to_value` names
//! a function of a crate the package pulls — and it was not judged at all,
//! because the parser only read a qualifier when the segment before the
//! name was spelled like a type.
//!
//! The project's own `to_value` and the one of `serde_json` are spelled the
//! same, so every `serde_json::to_value(…)` in the repository joined the
//! project's declaration. The graph then published, as one of the most
//! called symbols of the project, a function of a dependency, with the
//! dependency's call count as the project's.
//!
//! ## What is measured
//!
//! A package that pulls `serde_json` from a registry, a project function
//! named `to_value`, and one call written `serde_json::to_value(1)`. The
//! declaration has no callers: the call is not the project's, and it is
//! published as unresolved rather than dropped or joined.
//!
//! The same question is asked of every route that builds a graph, because
//! two routes answering the same question differently is the defect the
//! routes' own consistency test exists to catch.
//!
//! ## What must keep working
//!
//! The rule is about the crates a package pulls from *outside* the project,
//! and it says nothing else:
//!
//! - a path that starts at a module of the project (`event::build`) still
//!   resolves;
//! - a path that starts at a crate of this workspace — one the package
//!   declares with a path — still resolves, because that crate is the
//!   project's own;
//! - a call without a path, and a call through a type, are not touched.

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

/// The same package, with `serde_json` declared as a crate that lives here.
const LOCAL_MANIFEST: &str = "\
[package]
name = \"fixture\"

[dependencies]
serde_json = { path = \"../serde_json\" }
";

/// The same package, with `serde_json` declared for one platform only.
const TARGET_MANIFEST: &str = "\
[package]
name = \"fixture\"

[target.'cfg(unix)'.dependencies]
serde_json = \"1\"
";

/// A package that pulls `log` from a registry and declares a module of its
/// own that carries the same name.
const SHADOWED_MANIFEST: &str = "\
[package]
name = \"fixture\"

[dependencies]
log = \"0.4\"
";

const LIB_RS: &str = "\
pub mod event;

/// The project's own function, whose name a crate of a registry also has.
pub fn to_value() -> String {
    String::new()
}

/// A call through the crate the package pulls.
pub fn from_the_crate() -> String {
    serde_json::to_value(1).to_string()
}

/// A call through a module of the project.
pub fn from_the_module() -> u32 {
    event::build()
}
";

const EVENT_RS: &str = "\
pub fn build() -> u32 {
    2
}
";

/// A module of the project that carries the name of one of its dependencies.
const SHADOWED_LIB_RS: &str = "\
/// A module of the project, with the name of a crate the package pulls.
pub mod log {
    pub fn info() {}
}

/// A call through the module of the project.
pub fn from_the_module() {
    log::info();
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

/// A workspace with one member, and a crate that lives outside it.
///
/// The member declares the outside crate with a path: the manifest says
/// where it lives, and that is what the rule asks.
fn workspace_with_a_crate_from_outside(
    files: &[(&str, &str)],
) -> (tempfile::TempDir, tempfile::TempDir, Vec<PathBuf>) {
    let workspace = tempfile::tempdir().expect("a temporary workspace");
    let outside = tempfile::tempdir().expect("a crate that lives outside");
    let outside_name = outside
        .path()
        .file_name()
        .expect("the crate has a name")
        .to_string_lossy()
        .to_string();
    std::fs::write(
        workspace.path().join("Cargo.toml"),
        "[workspace]\nmembers = [\"core\"]\n",
    )
    .expect("the workspace manifest is written");
    let member = workspace.path().join("core");
    std::fs::create_dir_all(member.join("src")).expect("the member is created");
    std::fs::write(
        member.join("Cargo.toml"),
        format!(
            "[package]\nname = \"core\"\n\n[dependencies]\n\
             serde_json = {{ path = \"../../{outside_name}\" }}\n"
        ),
    )
    .expect("the member manifest is written");
    let paths = files
        .iter()
        .map(|(name, content)| {
            let path = member.join("src").join(name);
            std::fs::write(&path, content).expect("the fixture is written");
            path
        })
        .collect();
    (workspace, outside, paths)
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
fn the_project_route_does_not_join_a_crate_path_call_to_the_project_symbol() {
    let (dir, _files) = package(MANIFEST, &[("lib.rs", LIB_RS), ("event.rs", EVENT_RS)]);
    let (graph, unresolved) = project_route(dir.path());

    assert_eq!(
        callers_of(&graph, "to_value"),
        Vec::<String>::new(),
        "the package pulls `serde_json` from a registry: `serde_json::to_value` is not a call \
         to the project's own `to_value`"
    );
    assert!(
        unresolved >= 1,
        "the call is published as unresolved, not dropped and not joined: the coverage says \
         {unresolved}"
    );
}

#[test]
fn the_per_file_route_does_not_join_a_crate_path_call_to_the_project_symbol() {
    let (dir, files) = package(MANIFEST, &[("lib.rs", LIB_RS), ("event.rs", EVENT_RS)]);
    let graph = per_file_route(dir.path(), &files);

    assert_eq!(
        callers_of(&graph, "to_value"),
        Vec::<String>::new(),
        "the per-file route answers like the project route"
    );
}

#[test]
fn the_on_demand_route_does_not_join_a_crate_path_call_to_the_project_symbol() {
    let (dir, _files) = package(MANIFEST, &[("lib.rs", LIB_RS), ("event.rs", EVENT_RS)]);
    let mut strategy = OnDemandStrategy::new();
    strategy
        .build_index(dir.path())
        .expect("the project is indexed");

    assert!(
        on_demand_callees(&strategy, "from_the_crate").is_empty(),
        "`from_the_crate` calls `serde_json::to_value`, and the package pulls `serde_json` \
         from a registry: the on-demand route must not publish the project's own `to_value` \
         as its callee"
    );
}

/// A path that starts at a module of the project is the project's own: the
/// module is not a crate the package pulls from outside.
#[test]
fn a_module_path_still_resolves() {
    let (dir, files) = package(MANIFEST, &[("lib.rs", LIB_RS), ("event.rs", EVENT_RS)]);

    let (graph, _) = project_route(dir.path());
    assert_eq!(
        callers_of(&graph, "build"),
        vec!["from_the_module".to_string()],
        "the project route: `event::build` names a module of the project"
    );

    let graph = per_file_route(dir.path(), &files);
    assert_eq!(
        callers_of(&graph, "build"),
        vec!["from_the_module".to_string()],
        "the per-file route answers the same"
    );

    let mut strategy = OnDemandStrategy::new();
    strategy
        .build_index(dir.path())
        .expect("the project is indexed");
    assert_eq!(
        on_demand_callees(&strategy, "from_the_module"),
        vec!["build".to_string()],
        "the on-demand route answers the same"
    );
}

/// A crate declared with a path is a crate of this workspace, and the
/// project owns it: the rule is about the crates a package pulls from
/// *outside*, and it must not lose a true edge to avoid a false one.
#[test]
fn a_crate_of_this_workspace_still_resolves() {
    let (dir, files) = package(
        LOCAL_MANIFEST,
        &[("lib.rs", LIB_RS), ("event.rs", EVENT_RS)],
    );

    let (graph, _) = project_route(dir.path());
    assert_eq!(
        callers_of(&graph, "to_value"),
        vec!["from_the_crate".to_string()],
        "the project route: the package declares `serde_json` with a path, so it is a crate \
         of this workspace"
    );

    let graph = per_file_route(dir.path(), &files);
    assert_eq!(
        callers_of(&graph, "to_value"),
        vec!["from_the_crate".to_string()],
        "the per-file route answers the same"
    );
}

/// A dependency that only exists for one platform is a dependency all the
/// same: the rule reads it like any other.
#[test]
fn a_dependency_of_a_target_is_judged_too() {
    let (dir, files) = package(
        TARGET_MANIFEST,
        &[("lib.rs", LIB_RS), ("event.rs", EVENT_RS)],
    );

    let (graph, _) = project_route(dir.path());
    assert_eq!(
        callers_of(&graph, "to_value"),
        Vec::<String>::new(),
        "the package pulls `serde_json` from a registry for one platform: \
         `serde_json::to_value` is not a call to the project's own `to_value`"
    );

    let graph = per_file_route(dir.path(), &files);
    assert_eq!(
        callers_of(&graph, "to_value"),
        Vec::<String>::new(),
        "the per-file route answers the same"
    );
}

/// A crate that lives outside the workspace is somebody else's, however the
/// manifest declares it.
#[test]
fn a_crate_that_lives_outside_the_workspace_is_judged_too() {
    let (workspace, _outside, files) =
        workspace_with_a_crate_from_outside(&[("lib.rs", LIB_RS), ("event.rs", EVENT_RS)]);

    let (graph, _) = project_route(workspace.path());
    assert_eq!(
        callers_of(&graph, "to_value"),
        Vec::<String>::new(),
        "the package declares `serde_json` with a path that leaves the workspace: it is a crate \
         from outside it, and `serde_json::to_value` is not a call to the project's own \
         `to_value`"
    );

    let graph = per_file_route(workspace.path(), &files);
    assert_eq!(
        callers_of(&graph, "to_value"),
        Vec::<String>::new(),
        "the per-file route answers the same"
    );
}

/// A module of the project that carries the name of one of its dependencies
/// keeps its edge: the call is the project's own.
#[test]
fn a_module_of_the_project_that_shadows_a_dependency_keeps_its_edge() {
    let (dir, files) = package(SHADOWED_MANIFEST, &[("lib.rs", SHADOWED_LIB_RS)]);

    let (graph, _) = project_route(dir.path());
    assert_eq!(
        callers_of(&graph, "info"),
        vec!["from_the_module".to_string()],
        "the file declares `pub mod log;`, so `log::info()` names the project's own module, not \
         the `log` crate the package pulls"
    );

    let graph = per_file_route(dir.path(), &files);
    assert_eq!(
        callers_of(&graph, "info"),
        vec!["from_the_module".to_string()],
        "the per-file route answers the same"
    );
}
