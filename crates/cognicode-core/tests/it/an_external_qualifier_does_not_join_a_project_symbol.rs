//! A call qualified by a type the project does not declare is not a call to
//! the project's symbol of the same name.
//!
//! ## The defect
//!
//! The parser reduced every callee to its last segment: `Duration::from_millis`
//! became `from_millis`, and the qualifier was gone before the resolver ran.
//! The resolver then looked the bare name up in the project's symbol index,
//! and when the project declared exactly one `from_millis` — a method of its
//! own `EventTime` — every call to the standard library's one joined it.
//!
//! The graph then published, as the most called symbols of the project,
//! names that belong to the standard library (`from_millis`, `new`,
//! `default`, `from`, `with_capacity`): symbols whose caller count is the
//! number of times the *language* was used, not the number of times the
//! *project* was called.
//!
//! ## What is measured
//!
//! A project with one declaration of `from_millis` (on `EventTime`), three
//! calls to `Duration::from_millis` and one call to `EventTime::from_millis`.
//! The declaration has ONE caller. The three calls to the other type are
//! published as unresolved — the count the product already keeps for a name
//! the project does not declare — and no edge is invented for them.
//!
//! The same question is asked of both routes that build a graph (the
//! project route, and the per-file route that merges files one by one),
//! because two routes answering the same question differently is the defect
//! the routes' own consistency test exists to catch.
//!
//! ## What must keep working
//!
//! A qualifier is judged only when it is evidently someone else's: a type
//! name (capitalised) the project never declares. A lowercase qualifier is a
//! module path (`event::from_millis`), `Self` is the enclosing type, a
//! qualifier the project does declare resolves as before, and an
//! unqualified call is not touched. Each of those has its own test below:
//! the fix must not trade a false join for a lost true one.

use cognicode_core::application::services::analysis_service::AnalysisService;
use cognicode_core::domain::aggregates::call_graph::CallGraph;
use cognicode_core::infrastructure::graph::per_file_graph::PerFileGraphCache;
use std::path::{Path, PathBuf};

const EVENT_RS: &str = "\
pub struct EventTime(pub u64);

impl EventTime {
    pub fn from_millis(ms: u64) -> Self {
        EventTime(ms)
    }
}
";

/// Three calls to the standard library's `Duration::from_millis`, and one
/// call to the project's own `EventTime::from_millis`.
const LIB_RS: &str = "\
use std::time::Duration;

pub mod event;
use event::EventTime;

pub fn wait_a() -> Duration {
    Duration::from_millis(10)
}

pub fn wait_b() -> Duration {
    Duration::from_millis(20)
}

pub fn wait_c() -> Duration {
    Duration::from_millis(30)
}

pub fn stamp() -> EventTime {
    EventTime::from_millis(9)
}
";

/// A temporary project with the given files, and the paths of the files.
fn project(files: &[(&str, &str)]) -> (tempfile::TempDir, Vec<PathBuf>) {
    let dir = tempfile::tempdir().expect("a temporary project");
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

#[test]
fn the_project_route_does_not_join_a_std_call_to_the_project_symbol() {
    let (dir, _files) = project(&[("lib.rs", LIB_RS), ("event.rs", EVENT_RS)]);
    let (graph, unresolved) = project_route(dir.path());

    assert_eq!(
        callers_of(&graph, "from_millis"),
        vec!["stamp".to_string()],
        "only `stamp` calls the project's `from_millis`: the three `Duration::from_millis` \
         belong to the standard library"
    );
    assert!(
        unresolved >= 3,
        "the three calls to the other type are published as unresolved, not dropped \
         and not joined: the coverage says {unresolved}"
    );
}

#[test]
fn the_per_file_route_does_not_join_a_std_call_to_the_project_symbol() {
    let (dir, files) = project(&[("lib.rs", LIB_RS), ("event.rs", EVENT_RS)]);
    let graph = per_file_route(dir.path(), &files);

    assert_eq!(
        callers_of(&graph, "from_millis"),
        vec!["stamp".to_string()],
        "the per-file route answers like the project route"
    );
}

/// A qualifier the project declares resolves as before: the fix is about
/// names the project does not own, not about qualified calls.
#[test]
fn a_qualifier_the_project_declares_still_resolves() {
    // Only the project's own type is called: nothing here is the standard
    // library's, so this holds before the fix and after it.
    let lib = "\
pub mod event;
use event::EventTime;

pub fn stamp() -> EventTime {
    EventTime::from_millis(9)
}
";
    let (dir, _files) = project(&[("lib.rs", lib), ("event.rs", EVENT_RS)]);
    let (graph, _) = project_route(dir.path());

    assert_eq!(
        callers_of(&graph, "from_millis"),
        vec!["stamp".to_string()],
        "`EventTime::from_millis` is a call to the project's declaration"
    );
}

/// A lowercase qualifier is a module path, and a module is not a symbol:
/// it is never judged external.
#[test]
fn a_module_path_qualifier_still_resolves() {
    let lib = "\
pub mod event;

pub fn stamp() -> event::EventTime {
    event::build(9)
}
";
    let event = "\
pub struct EventTime(pub u64);

pub fn build(ms: u64) -> EventTime {
    EventTime(ms)
}
";
    let (dir, _files) = project(&[("lib.rs", lib), ("event.rs", event)]);
    let (graph, _) = project_route(dir.path());

    assert_eq!(
        callers_of(&graph, "build"),
        vec!["stamp".to_string()],
        "`event::build` is a call through a module path: it resolves to the project's `build`"
    );
}

/// `Self` is the enclosing type, which the project declares.
#[test]
fn a_self_qualifier_still_resolves() {
    let event = "\
pub struct EventTime(pub u64);

impl EventTime {
    pub fn from_millis(ms: u64) -> Self {
        EventTime(ms)
    }

    pub fn zero() -> Self {
        Self::from_millis(0)
    }
}
";
    let (dir, _files) = project(&[("event.rs", event)]);
    let (graph, _) = project_route(dir.path());

    assert_eq!(
        callers_of(&graph, "from_millis"),
        vec!["zero".to_string()],
        "`Self::from_millis` is a call to the declaration of the type it is written in"
    );
}

/// An unqualified call is not touched: it is how a project calls its own
/// free functions.
#[test]
fn an_unqualified_call_still_resolves() {
    let lib = "\
pub fn helper() {}

pub fn run() {
    helper();
}
";
    let (dir, _files) = project(&[("lib.rs", lib)]);
    let (graph, _) = project_route(dir.path());

    assert_eq!(callers_of(&graph, "helper"), vec!["run".to_string()]);
}

/// An `impl` of a project trait for a standard-library type does not make
/// that type the project's: `Duration` is still somebody else's, and the
/// project declares no `Duration` of its own.
#[test]
fn an_impl_for_a_std_type_does_not_make_the_type_the_projects() {
    let lib = "\
use std::time::Duration;

pub mod event;
use event::EventTime;

pub trait Describe {
    fn render(&self) -> u8 {
        1
    }
}

impl Describe for Duration {}

pub fn wait() -> Duration {
    Duration::from_millis(10)
}

pub fn stamp() -> EventTime {
    EventTime::from_millis(9)
}
";
    let (dir, files) = project(&[("lib.rs", lib), ("event.rs", EVENT_RS)]);

    let (graph, _) = project_route(dir.path());
    assert_eq!(
        callers_of(&graph, "from_millis"),
        vec!["stamp".to_string()],
        "the project route: an impl for `Duration` does not make `Duration` the project's"
    );

    let graph = per_file_route(dir.path(), &files);
    assert_eq!(
        callers_of(&graph, "from_millis"),
        vec!["stamp".to_string()],
        "the per-file route answers the same"
    );
}

/// A call qualified by a trait the project declares is a call into the
/// project: the judgment must not lose it.
#[test]
fn a_trait_qualifier_the_project_declares_still_resolves() {
    let lib = "\
pub trait Describe {
    fn render(&self) -> u8 {
        1
    }
}

pub struct Thing;

impl Describe for Thing {}

pub fn run(thing: &Thing) -> u8 {
    Describe::render(thing)
}
";
    let (dir, files) = project(&[("lib.rs", lib)]);

    let (graph, _) = project_route(dir.path());
    assert_eq!(
        callers_of(&graph, "render"),
        vec!["run".to_string()],
        "the project route: `Describe::render` names the project's trait"
    );

    let graph = per_file_route(dir.path(), &files);
    assert_eq!(
        callers_of(&graph, "render"),
        vec!["run".to_string()],
        "the per-file route answers the same"
    );
}

/// A call qualified by an enum the project declares is a call into the
/// project, for the same reason.
#[test]
fn an_enum_qualifier_the_project_declares_still_resolves() {
    let lib = "\
pub enum Shape {
    Unit,
    Pair(u8, u8),
}

impl Shape {
    pub fn unit() -> Shape {
        Shape::Unit
    }
}

pub fn make() -> Shape {
    Shape::unit()
}
";
    let (dir, files) = project(&[("lib.rs", lib)]);

    let (graph, _) = project_route(dir.path());
    assert_eq!(
        callers_of(&graph, "unit"),
        vec!["make".to_string()],
        "the project route: `Shape::unit` names the project's enum"
    );

    let graph = per_file_route(dir.path(), &files);
    assert_eq!(
        callers_of(&graph, "unit"),
        vec!["make".to_string()],
        "the per-file route answers the same"
    );
}
