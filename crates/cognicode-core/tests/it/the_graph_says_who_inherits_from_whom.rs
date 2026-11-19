//! The graph has to say who inherits from whom.
//!
//! ## The defect
//!
//! The route that builds the project graph —the one the change pack reads—
//! published no inheritance at all. A class that extends another, a type
//! that implements a trait: none of them produced a relation, so the graph
//! of a project with an inheritance hierarchy described a flat set of
//! declarations that had nothing to do with each other.
//!
//! ## What is measured
//!
//! One project per language, each with a declaration that names a parent:
//!
//! 1. Rust: `impl Renderer for Html` names the trait `Renderer` as the
//!    parent of the type `Html`. The block declares nothing, so the
//!    relation hangs on the type — not on the block.
//! 2. Python: `class Dog(Animal)`.
//! 3. TypeScript: `class Dog extends Animal`.
//! 4. Java: `class Dog extends Animal`.
//!
//! And one control: a declaration that names no parent publishes no
//! inheritance edge. Without it, a route that published an inheritance
//! edge for everything would pass the four measurements above.
//!
//! ## The two routes
//!
//! The measurement is made on the project route, because that is the route
//! the change pack reads, and on the per-file route, because a route that
//! says something different from the other one is two answers to the same
//! question.

use cognicode_core::application::services::analysis_service::AnalysisService;
use cognicode_core::domain::aggregates::call_graph::CallGraph;
use cognicode_core::domain::value_objects::DependencyType;
use cognicode_core::infrastructure::graph::per_file_graph::PerFileGraphCache;
use std::path::{Path, PathBuf};

/// A project with the given files, and the paths of the files.
fn project(files: &[(&str, &str)]) -> (tempfile::TempDir, Vec<PathBuf>) {
    let dir = tempfile::tempdir().expect("a temporary project");
    let src = dir.path().join("src");
    std::fs::create_dir_all(&src).expect("the source directory is created");
    let paths = files
        .iter()
        .map(|(name, content)| {
            let path = src.join(name);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).expect("the directory is created");
            }
            std::fs::write(&path, content).expect("the fixture is written");
            path
        })
        .collect();
    (dir, paths)
}

/// The inheritance edges of a graph, as `source -> target`.
///
/// The names are the ones the graph holds, and the edges are sorted: the
/// order the edges come in is not part of the answer.
fn inherits_edges(graph: &CallGraph) -> Vec<String> {
    let mut edges: Vec<String> = graph
        .all_dependencies()
        .filter(|(_, _, kind)| **kind == DependencyType::Inherits)
        .map(|(source, target, _)| {
            let name_of = |id: &cognicode_core::domain::aggregates::call_graph::SymbolId| {
                graph
                    .get_symbol(id)
                    .map(|symbol| symbol.name().to_string())
                    .unwrap_or_else(|| id.as_str().to_string())
            };
            format!("{} -> {}", name_of(source), name_of(target))
        })
        .collect();
    edges.sort();
    edges
}

/// The graph the project route builds for a project.
fn project_route(root: &Path) -> CallGraph {
    let service = AnalysisService::new();
    service
        .build_project_graph_filtered(root, &[root])
        .expect("the project graph builds")
        .graph
}

/// The graph the per-file route builds for the files of a project.
///
/// The route merges the files with an index over all of them, so a parent
/// that lives in another file of the project is found.
fn file_route(root: &Path, files: &[&Path]) -> CallGraph {
    let cache = PerFileGraphCache::with_project_dir(root.to_string_lossy().into_owned());
    cache.merge_with_report(files).graph
}

/// A Rust type that implements a trait.
///
/// The trait lives in another file: a relation names two declarations, and
/// the two of them do not have to live together.
const RUST_TRAIT_RS: &str = "\
pub trait Renderer {
    fn render(&self) -> String;
}
";

/// The Rust type, and the block that implements the trait.
const RUST_TYPE_RS: &str = "\
pub struct Html;

impl Renderer for Html {
    fn render(&self) -> String {
        String::new()
    }
}
";

/// An impl in a separate file from its owner type.
const RUST_IMPL_IN_OTHER_FILE_RS: &str = "\
impl Renderer for Html {
    fn render(&self) -> String { String::new() }
}
";

/// A Rust type that implements no trait.
const RUST_PLAIN_RS: &str = "\
pub struct Plain;
";

/// A Python class and the class that extends it.
const PYTHON_MODEL_PY: &str = "\
class Animal:
    pass


class Dog(Animal):
    pass
";

/// A TypeScript class and the class that extends it.
const TYPESCRIPT_MODEL_TS: &str = "\
class Animal {}

class Dog extends Animal {}
";

/// A Java class and the class that extends it.
const JAVA_MODEL_JAVA: &str = "\
class Animal {}

class Dog extends Animal {}
";

/// Rust: the trait a type implements is the parent of the type.
#[test]
fn the_project_route_says_which_type_implements_which_trait() {
    let (dir, _files) = project(&[("traits.rs", RUST_TRAIT_RS), ("page.rs", RUST_TYPE_RS)]);

    let edges = inherits_edges(&project_route(dir.path()));

    assert_eq!(
        edges,
        vec!["Html -> Renderer".to_string()],
        "`impl Renderer for Html` names the trait as the parent of the type"
    );
}

/// A Rust impl resolves its owner by the unique project-wide declaration
/// with that name; the declaration remains the only source of the edge.
#[test]
fn the_project_route_resolves_an_impl_owner_declared_in_another_file() {
    let (dir, files) = project(&[
        ("traits.rs", RUST_TRAIT_RS),
        ("types.rs", "pub struct Html;"),
        ("impls.rs", RUST_IMPL_IN_OTHER_FILE_RS),
    ]);

    assert_eq!(
        inherits_edges(&file_route(dir.path(), &[&files[0], &files[1], &files[2]])),
        vec!["Html -> Renderer".to_string()],
        "the impl owner `Html` resolves to its unique project declaration, then `Renderer` resolves as its parent"
    );
}

/// A missing or ambiguous impl owner is not treated as a declaration.
#[test]
fn the_project_route_does_not_invent_an_impl_owner_for_unresolved_or_ambiguous_names() {
    for files in [
        vec![
            ("traits.rs", RUST_TRAIT_RS),
            (
                "impls.rs",
                "impl Renderer for Ghost { fn render(&self) -> String { String::new() } }",
            ),
        ],
        vec![
            ("traits.rs", RUST_TRAIT_RS),
            ("types_a.rs", "pub struct Html;"),
            ("types_b.rs", "pub struct Html;"),
            ("impls.rs", RUST_IMPL_IN_OTHER_FILE_RS),
        ],
    ] {
        let (dir, files) = project(&files);
        let paths: Vec<&Path> = files.iter().map(PathBuf::as_path).collect();
        assert!(
            inherits_edges(&file_route(dir.path(), &paths)).is_empty(),
            "an unresolved or ambiguous owner cannot anchor an inheritance relation"
        );
    }
}

/// Python: the class a class extends is the parent of the class.
#[test]
fn the_project_route_says_which_class_extends_which_class() {
    let (dir, _files) = project(&[("models.py", PYTHON_MODEL_PY)]);

    let edges = inherits_edges(&project_route(dir.path()));

    assert_eq!(
        edges,
        vec!["Dog -> Animal".to_string()],
        "`class Dog(Animal)` names the parent of the class"
    );
}

/// TypeScript: the class a class extends is the parent of the class.
#[test]
fn the_project_route_says_which_typescript_class_extends_which() {
    let (dir, _files) = project(&[("models.ts", TYPESCRIPT_MODEL_TS)]);

    let edges = inherits_edges(&project_route(dir.path()));

    assert_eq!(
        edges,
        vec!["Dog -> Animal".to_string()],
        "`class Dog extends Animal` names the parent of the class"
    );
}

/// Java: the class a class extends is the parent of the class.
#[test]
fn the_project_route_says_which_java_class_extends_which() {
    let (dir, _files) = project(&[("Models.java", JAVA_MODEL_JAVA)]);

    let edges = inherits_edges(&project_route(dir.path()));

    assert_eq!(
        edges,
        vec!["Dog -> Animal".to_string()],
        "`class Dog extends Animal` names the parent of the class"
    );
}

/// The control: a declaration that names no parent publishes no inheritance.
///
/// Without this measurement, a route that published an inheritance edge for
/// every declaration would pass the four measurements above.
#[test]
fn a_declaration_that_names_no_parent_publishes_no_inheritance() {
    let (dir, _files) = project(&[("plain.rs", RUST_PLAIN_RS)]);

    let edges = inherits_edges(&project_route(dir.path()));

    assert!(
        edges.is_empty(),
        "a type that implements no trait names no parent: {edges:?}"
    );
}

/// The per-file route says the same thing as the project route.
///
/// The two routes are two answers to the same question, and a consumer that
/// gets a different graph depending on which interface it asked cannot
/// compare its answer with anything.
#[test]
fn the_two_routes_say_the_same_thing_about_inheritance() {
    let (dir, files) = project(&[("traits.rs", RUST_TRAIT_RS), ("page.rs", RUST_TYPE_RS)]);

    let project_edges = inherits_edges(&project_route(dir.path()));
    let file_edges = inherits_edges(&file_route(dir.path(), &[&files[0], &files[1]]));

    assert_eq!(
        project_edges, file_edges,
        "the project route and the per-file route published different inheritance \
         relations for the same tree: {project_edges:?} vs {file_edges:?}"
    );
    assert!(
        !project_edges.is_empty(),
        "a test that compares empty answers proves nothing: {project_edges:?}"
    );
}

/// A parent that lives in another directory of the project is a parent.
///
/// The declaration that names a parent is the trait, and the trait lives
/// where the code that declares it lives —which is not necessarily the
/// directory of the code that implements it. A rule that only looked at the
/// writer's own directory would leave the relation unpublished, and the two
/// routes would answer differently about the same file.
#[test]
fn a_parent_in_another_directory_is_a_parent() {
    let (dir, files) = project(&[
        ("traits/mod.rs", RUST_TRAIT_RS),
        ("models/page.rs", RUST_TYPE_RS),
    ]);

    let project_edges = inherits_edges(&project_route(dir.path()));
    let file_edges = inherits_edges(&file_route(dir.path(), &[&files[0], &files[1]]));

    assert_eq!(
        project_edges,
        vec!["Html -> Renderer".to_string()],
        "the trait lives in another directory of the project, and the relation \
         does not stop at the writer's own directory"
    );
    assert_eq!(
        project_edges, file_edges,
        "the project route and the per-file route published different inheritance \
         relations for the same tree: {project_edges:?} vs {file_edges:?}"
    );
}
