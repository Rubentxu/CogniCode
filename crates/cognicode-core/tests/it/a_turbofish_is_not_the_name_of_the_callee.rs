//! A call written with a turbofish calls the method, not the type
//! argument.
//!
//! ## The defect
//!
//! The name of a callee was the last segment of its text, and in
//! `x.parse::<u32>()` the last segment is the generic argument list: the
//! call was published as `<u32>`, a name no declaration can carry. The
//! call was counted as unresolved even when the project declares the
//! method, and the relationship the project owns was missing from the
//! graph.
//!
//! ## What is measured
//!
//! A project with a method that takes a type argument, and one call
//! written with a turbofish. The call joins the project's declaration,
//! on the project route and on the per-file route.
//!
//! ## What must keep working
//!
//! - a path with generic arguments and no turbofish (`Vec::<u8>::new`)
//!   still names the method;
//! - the type qualifier of such a path is still the type (`Vec`).

use cognicode_core::application::services::analysis_service::AnalysisService;
use cognicode_core::domain::aggregates::call_graph::CallGraph;
use cognicode_core::infrastructure::graph::per_file_graph::PerFileGraphCache;
use std::path::{Path, PathBuf};

/// A package with no dependencies: the call is the project's own.
const MANIFEST: &str = "\
[package]
name = \"fixture\"
";

/// A type of the project, with a method that takes a type argument.
const HOLDER_RS: &str = "\
/// A type of the project, with a method that takes a type argument.
pub struct Holder {
    pub value: u32,
}

impl Holder {
    /// A method the project declares.
    pub fn parse<T: Into<u32>>(&self) -> T {
        self.value.into()
    }
}
";

/// A call written with a turbofish.
const CALLER_RS: &str = "\
use crate::holder::Holder;

/// A call written with a turbofish: the callee is `parse`, not `<u32>`.
pub fn parse_it(holder: &Holder) -> u32 {
    holder.parse::<u32>()
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

/// The package with the type, the method and the call.
fn package_with_a_turbofish() -> (tempfile::TempDir, Vec<PathBuf>) {
    package(
        MANIFEST,
        &[
            ("lib.rs", "pub mod holder;\npub mod caller;\n"),
            ("holder.rs", HOLDER_RS),
            ("caller.rs", CALLER_RS),
        ],
    )
}

/// The graph of the project route.
fn project_route(dir: &Path) -> CallGraph {
    let service = AnalysisService::new();
    service
        .build_project_graph(dir)
        .expect("the project graph is built");
    (*service.get_project_graph()).clone()
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

#[test]
fn the_project_route_resolves_a_call_written_with_a_turbofish() {
    let (dir, _files) = package_with_a_turbofish();
    let graph = project_route(dir.path());

    assert_eq!(
        callers_of(&graph, "parse"),
        vec!["parse_it".to_string()],
        "`holder.parse::<u32>()` calls the project's own `parse`"
    );
}

#[test]
fn the_per_file_route_resolves_a_call_written_with_a_turbofish() {
    let (dir, files) = package_with_a_turbofish();
    let graph = per_file_route(dir.path(), &files);

    assert_eq!(
        callers_of(&graph, "parse"),
        vec!["parse_it".to_string()],
        "the per-file route answers like the project route"
    );
}
