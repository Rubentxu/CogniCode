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
//!
//! ## The second defect: a refusal that carries its motive
//!
//! The hole of TASK-319 is measured, and it is measured with a rule that
//! says "the file declares the name the call names -> the refusal is a
//! hole". That rule is too wide, because it does not know WHY the route
//! refused:
//!
//! - The file cannot prove the call is somebody else's when the call is
//!   written through a TYPE the file does not declare: the type may live
//!   in another file of the project. THAT is a hole.
//! - The file CAN prove the call is somebody else's when the head of the
//!   path is a crate outside the project: `serde_json::to_value(1)` names
//!   a function of a crate the package pulls, and no file of the project
//!   can change that. That is NOT a hole, and the file route used to
//!   count it as one whenever the file happened to declare a symbol of
//!   the same name.
//! - The file CAN prove it too when the RECEIVER belongs to somebody
//!   else: the file states the type of the receiver, and a type it does
//!   not bind is the standard library's or a dependency's.
//!
//! Two routes, two counts about the same call: the project route proved
//! the call was foreign and left no reference behind, and the file route
//! left one. The motive of the refusal has to leave the judgment for the
//! two routes to agree.

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

/// The manifest of the package of the fixture: it pulls `serde_json` from
/// a registry, so the crate is outside the project.
///
/// The manifest is what makes the crate-path rule judgeable: a package
/// that says nothing declares no crate from outside, and the call
/// written through `serde_json` would not be judged at all.
const CRATE_MANIFEST: &str = "\
[package]
name = \"fixture\"

[dependencies]
serde_json = \"1\"
";

/// A file that declares its OWN `to_value` and calls the crate's.
///
/// The file declares the name the call names, so the rule of TASK-319
/// ("the file declares the name the call names -> the refusal is a hole")
/// would call this a hole. The head of the path says otherwise: the call
/// names a function of a crate outside the project, and the route can
/// PROVE it.
const CRATE_RS: &str = "\
pub fn to_value(value: u64) -> u64 {
    value
}

pub fn encode(value: u64) -> u64 {
    serde_json::to_value(value)
}
";

/// A file that declares its OWN `len` and calls somebody else's `len`.
///
/// The receiver is a value whose type the file does not bind: `Vec` is
/// the standard library's, and the file says so by not binding it. The
/// file declares the name the call names, and the rule of TASK-319 alone
/// would call this a hole; the receiver says otherwise.
const RECEIVER_RS: &str = "\
pub fn len() -> usize {
    0
}

pub fn run(values: &Vec<u8>) -> usize {
    values.len()
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

/// A temporary project whose package pulls `serde_json` from outside, and
/// the paths of the files.
fn project_with_crate(files: &[(&str, &str)]) -> (tempfile::TempDir, Vec<PathBuf>) {
    let (dir, paths) = project(files);
    std::fs::write(dir.path().join("Cargo.toml"), CRATE_MANIFEST)
        .expect("the manifest of the package is written");
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

/// A call written through a crate from outside the project is NOT a hole.
///
/// The file declares its own `to_value` and calls the crate's
/// `to_value`. The head of the path is a crate outside the project, and
/// the route can PROVE the call is not the file's: no file of the project
/// can change what `serde_json::to_value` names. So the refusal leaves
/// no reference behind, exactly as it does in the project route.
///
/// The file declares the name the call names, and that is what makes this
/// measurement mean something: the rule of TASK-319 alone would call it a
/// hole, and the two routes would count the same call differently.
#[test]
fn a_call_written_through_a_crate_from_outside_leaves_no_reference_behind() {
    let (dir, files) = project_with_crate(&[("lib.rs", CRATE_RS)]);
    let (graph, pending) = file_route(dir.path(), &files[0]);

    assert!(
        pending.is_empty(),
        "the head of the path is a crate outside the project: the route can prove the call is \
         not the file's, so the refusal leaves no reference behind. The pending references are \
         {pending:?}"
    );
    assert_eq!(
        callers_of(&graph, "to_value"),
        Vec::<String>::new(),
        "the call names the crate's `to_value` and not the file's own: the file route does not \
         join it"
    );

    // The project route answers the same: the crate is outside the
    // project, and the call is not the project's either.
    let service = AnalysisService::new();
    service
        .build_project_graph(dir.path())
        .expect("the project graph is built");

    assert_eq!(
        callers_of(&service.get_project_graph(), "to_value"),
        Vec::<String>::new(),
        "two routes, one answer: the project route does not join it either"
    );
}

/// A call written on a receiver that belongs to somebody else is NOT a
/// hole either.
///
/// The file declares its own `len` and calls the standard library's
/// `len`. The receiver is the proof: the file states the type of the
/// value the call is written on, and it does not bind that type — so the
/// type is the standard library's, and the route can PROVE the call is
/// not the file's.
#[test]
fn a_call_on_a_receiver_that_belongs_to_somebody_else_leaves_no_reference_behind() {
    let (dir, files) = project(&[("lib.rs", RECEIVER_RS)]);
    let (graph, pending) = file_route(dir.path(), &files[0]);

    assert!(
        pending.is_empty(),
        "the receiver belongs to somebody else and the route can prove it: the refusal leaves \
         no reference behind. The pending references are {pending:?}"
    );
    assert_eq!(
        callers_of(&graph, "len"),
        Vec::<String>::new(),
        "the call names the standard library's `len` and not the file's own: the file route does \
         not join it"
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
