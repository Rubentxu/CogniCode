//! The hole of the file route is visible.
//!
//! ## The defect
//!
//! Two routes build a graph: the route WITH a project-wide index (the
//! project scope) and the route WITHOUT one (the file scope, where the
//! file is the whole scope).
//!
//! The route without an index refuses some calls that the route with an
//! index publishes. The concrete case is a file that implements a TRAIT
//! for a type that lives in another file:
//!
//! ```ignore
//! // other.rs
//! use crate::widget::Widget;
//!
//! pub trait Describe {
//!     fn render(&self) -> u64;
//! }
//!
//! impl Describe for Widget {
//!     fn render(&self) -> u64 { 0 }
//! }
//!
//! pub fn show(w: &Widget) -> u64 {
//!     Widget::render(w)          // written through the TYPE
//! }
//! ```
//!
//! The owner of a trait impl is the TRAIT (`Describe`), so the file route
//! refuses the call `Widget::render(w)`: the qualifier `Widget` is not
//! the owner `Describe`. The route with the index publishes it, because
//! the project declares `Widget`. Two routes, two answers about the same
//! call.
//!
//! The worse half of the defect is that the refusal leaves no trace. The
//! refused call published no edge AND no pending reference: the edge
//! disappeared and the coverage did not mention it. A hole nobody counts
//! is a hole nobody can fix.
//!
//! ## What is measured
//!
//! 1. The hole: the file route does NOT publish the edge to `render` — it
//!    cannot prove the call is the file's own, and publishing would repeat
//!    the defect TASK-268 removed, where a standard-library call was
//!    joined to a project symbol — AND the pending references of that
//!    file name the call: its target, its file and its line.
//! 2. The control: a call whose name the file never mentions leaves NO
//!    pending reference. It is somebody else's call, not a hole.
//! 3. The route WITH the index is not changed: the project route still
//!    publishes the call, because the project declares the type.
//!
//! The control is what makes the first measurement mean something: a rule
//! that reported every refusal would pass the hole's measurement and
//! count the whole world outside the project as a hole.

use cognicode_core::application::services::analysis_service::AnalysisService;
use cognicode_core::domain::aggregates::call_graph::CallGraph;
use cognicode_core::domain::value_objects::{PendingClass, PendingReference};
use cognicode_core::infrastructure::graph::per_file_graph::PerFileGraphCache;
use std::path::{Path, PathBuf};

/// The crate of the fixture, with the modules the fixture's types live in.
const LIB_RS: &str = "\
pub mod widget;
pub mod other;
pub mod ghost;
";

/// The type the trait is implemented for. It lives in ANOTHER file, which
/// is what the file route cannot see.
const WIDGET_RS: &str = "\
pub struct Widget(pub u64);
";

/// The file that implements the trait and writes the call through the
/// type.
const OTHER_RS: &str = "\
use crate::widget::Widget;

pub trait Describe {
    fn render(&self) -> u64;
}

impl Describe for Widget {
    fn render(&self) -> u64 { 0 }
}

pub fn show(w: &Widget) -> u64 {
    Widget::render(w)
}
";

/// The control: a file that mentions neither the type nor the method.
const GHOST_RS: &str = "\
use std::time::Duration;

pub fn wait() -> Duration {
    Duration::from_millis(10)
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

/// The graph the route without a project index builds for one file, and the
/// references that file states and nobody tied.
fn file_route(dir: &Path, file: &Path) -> (CallGraph, Vec<PendingReference>) {
    let cache = PerFileGraphCache::with_project_dir(dir.to_string_lossy().into_owned());
    let (graph, pending) = cache
        .get_or_build(file)
        .expect("the file is read and its graph is built");
    ((*graph).clone(), pending)
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

/// The line a piece of the fixture is written on, counting from one.
fn line_of(source: &str, needle: &str) -> u32 {
    source
        .lines()
        .position(|line| line.contains(needle))
        .expect("the fixture writes that line") as u32
        + 1
}

/// The hole: the file route refuses the call, and the refusal is visible.
///
/// The file cannot prove the call is its own: the owner of the trait impl
/// is the trait, and the call is written through the type — whose own
/// inherent method would win over the trait's. So the edge is not
/// published. But the file DOES declare the name the call names (the
/// method of its own impl), so the refusal is a hole in what the graph
/// knows, and it leaves a reference behind.
#[test]
fn the_refused_call_of_the_file_route_leaves_a_reference_behind() {
    let (dir, files) = project(&[
        ("lib.rs", LIB_RS),
        ("widget.rs", WIDGET_RS),
        ("other.rs", OTHER_RS),
    ]);
    let (graph, pending) = file_route(dir.path(), &files[2]);

    assert_eq!(
        callers_of(&graph, "render"),
        Vec::<String>::new(),
        "the file route cannot prove the call is the file's own: the type lives in another file \
         and its own method would win over the trait's"
    );

    let reference = pending
        .iter()
        .find(|reference| reference.target == "render")
        .unwrap_or_else(|| {
            panic!(
                "the refused call leaves no trace: the pending references of the file are \
                 {pending:?}, and the hole nobody counts is a hole nobody can fix"
            )
        });
    assert_eq!(
        reference.class,
        PendingClass::Unresolved,
        "the far end of the reference could not be identified: the wire says `unresolved` and \
         nothing else"
    );
    assert_eq!(
        reference.origin_file,
        files[2].to_string_lossy().to_string(),
        "the reference says which file writes the call"
    );
    assert_eq!(
        reference.origin_line,
        Some(line_of(OTHER_RS, "Widget::render(w)")),
        "the reference says which line the call is written on"
    );
}

/// The control: a call the file never mentions is somebody else's call.
///
/// The file declares neither the type nor the method, so nothing of the
/// call can be the file's own: there is no hole, and no reference behind.
#[test]
fn a_call_the_file_never_mentions_leaves_no_reference_behind() {
    let (dir, files) = project(&[("lib.rs", LIB_RS), ("ghost.rs", GHOST_RS)]);
    let (_graph, pending) = file_route(dir.path(), &files[1]);

    assert!(
        pending.is_empty(),
        "the call names something the file never declares: it is somebody else's call and not \
         a hole, so it leaves no reference behind. The pending references are {pending:?}"
    );
}

/// The route WITH the index is not changed: the project declares the type,
/// so the project route publishes the call.
#[test]
fn the_project_route_still_publishes_the_call() {
    let (dir, _files) = project(&[
        ("lib.rs", LIB_RS),
        ("widget.rs", WIDGET_RS),
        ("other.rs", OTHER_RS),
    ]);

    let service = AnalysisService::new();
    service
        .build_project_graph(dir.path())
        .expect("the project graph is built");
    let graph = service.get_project_graph();

    assert_eq!(
        callers_of(&graph, "render"),
        vec!["show".to_string()],
        "the project declares `Widget`: the call is the project's, and the project route \
         publishes it"
    );
}
