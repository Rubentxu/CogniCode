//! The owner of an `impl` keeps the path of the type it implements.
//!
//! ## The defect
//!
//! The route that builds a graph per file —no project-wide index— judges a
//! qualified call against the declarations of the file itself. A file that
//! implements a type owns the calls written through that type, even when
//! the type is declared in another file, and that ownership was decided by
//! the LAST segment of the path: `impl a::Widget` was owned as `Widget`,
//! and the call `b::Widget::new(7)` was read as `Widget::new(7)`.
//!
//! Two types of the same name in two modules then collide: the file
//! publishes an edge to the method of `a::Widget` for a call that names
//! the method of `b::Widget`. Before the ownership rule existed the call
//! was refused, so the rule added a false edge.
//!
//! ## What is measured
//!
//! Three fixtures, each in its own project:
//!
//! 1. The defect: a file that implements `impl a::Widget` and writes
//!    `b::Widget::new(7)` publishes no edge to its own `new`.
//! 2. The control: the same file writing `a::Widget::new(7)` publishes it.
//! 3. The bare owner: a file that writes `use crate::a::Widget;`,
//!    implements `impl Widget` and writes `b::Widget::new(7)` publishes no
//!    edge either —the owner takes the path the file binds the name to—,
//!    while the call written `Widget::new(7)` publishes it.
//!
//! The control is what makes the first measurement mean something: an
//! implementation that forbade every edge written through an `impl` would
//! pass the defect's measurement and lose every true one.
//!
//! ## Why the route without a project index
//!
//! The evidence is the route that judges a call against the file's own
//! declarations, because that is the route that owns the calls written
//! through the types the file implements. The route with the index judges
//! against the project, where two types of the same name in two modules
//! are one name: that route cannot tell them apart, and this change does
//! not pretend it can.

use cognicode_core::domain::aggregates::call_graph::CallGraph;
use cognicode_core::infrastructure::graph::per_file_graph::PerFileGraphCache;
use std::path::{Path, PathBuf};

/// The crate of the fixture, with the modules the fixture's types live in.
const LIB_RS: &str = "\
pub mod a;
pub mod b;
pub mod other;
";

/// A type in the module `a`.
const A_RS: &str = "\
pub struct Widget(pub u64);
";

/// A type of the same name in the module `b`, with a method of its own.
const B_RS: &str = "\
pub struct Widget(pub u64);

impl Widget {
    pub fn new(value: u64) -> Self {
        Widget(value)
    }
}
";

/// A file that implements the type of the module `a` and calls the type of
/// the module `b`.
const OTHER_CROSSES_MODULES_RS: &str = "\
use crate::a;
use crate::b;

impl a::Widget {
    pub fn new(value: u64) -> Self {
        a::Widget(value)
    }
}

pub fn make() -> u64 {
    b::Widget::new(7).0
}
";

/// The same file, calling the type it implements.
const OTHER_CALLS_ITS_OWN_RS: &str = "\
use crate::a;
use crate::b;

impl a::Widget {
    pub fn new(value: u64) -> Self {
        a::Widget(value)
    }
}

pub fn make() -> u64 {
    a::Widget::new(7).0
}
";

/// A file that implements the type it binds by name, and calls the type of
/// the module `b` and the type it binds.
const OTHER_BARE_OWNER_RS: &str = "\
use crate::a::Widget;
use crate::b;

impl Widget {
    pub fn new(value: u64) -> Self {
        Widget(value)
    }
}

pub fn make() -> u64 {
    Widget::new(7).0
}

pub fn make_other() -> u64 {
    b::Widget::new(7).0
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

/// The graph the route without a project index builds for one file.
fn file_route(dir: &Path, file: &Path) -> CallGraph {
    let cache = PerFileGraphCache::with_project_dir(dir.to_string_lossy().into_owned());
    let (graph, _pending) = cache
        .get_or_build(file)
        .expect("the file is read and its graph is built");
    (*graph).clone()
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

/// The defect: the call names the method of ANOTHER type, and the file must
/// not publish an edge to the method of the type it implements.
#[test]
fn a_call_through_another_module_does_not_join_the_impl_of_its_own_type() {
    let (dir, files) = project(&[
        ("lib.rs", LIB_RS),
        ("a.rs", A_RS),
        ("b.rs", B_RS),
        ("other.rs", OTHER_CROSSES_MODULES_RS),
    ]);
    let graph = file_route(dir.path(), &files[3]);

    assert_eq!(
        callers_of(&graph, "new"),
        Vec::<String>::new(),
        "`b::Widget::new(7)` names the method of `b::Widget`: the file's own `new` is the \
         method of `a::Widget`, and the two types are not the same one"
    );
}

/// The control: the same file, calling the type it implements. Without this
/// measurement, an implementation that refused every edge written through
/// an `impl` would pass the defect's one.
#[test]
fn a_call_through_the_module_of_the_type_joins_the_impl_of_it() {
    let (dir, files) = project(&[
        ("lib.rs", LIB_RS),
        ("a.rs", A_RS),
        ("b.rs", B_RS),
        ("other.rs", OTHER_CALLS_ITS_OWN_RS),
    ]);
    let graph = file_route(dir.path(), &files[3]);

    assert_eq!(
        callers_of(&graph, "new"),
        vec!["make".to_string()],
        "`a::Widget::new(7)` names the method of the type the file implements"
    );
}

/// The bare owner takes the path the file binds the name to: the file
/// writes `use crate::a::Widget;`, so the owner of `impl Widget` is
/// `crate::a::Widget`, and the call through `b::Widget` names another type.
#[test]
fn a_bare_owner_takes_the_path_the_file_binds_the_name_to() {
    let (dir, files) = project(&[
        ("lib.rs", LIB_RS),
        ("a.rs", A_RS),
        ("b.rs", B_RS),
        ("other.rs", OTHER_BARE_OWNER_RS),
    ]);
    let graph = file_route(dir.path(), &files[3]);

    assert_eq!(
        callers_of(&graph, "new"),
        vec!["make".to_string()],
        "the owner of `impl Widget` is the `crate::a::Widget` the file binds the name to: \
         `Widget::new(7)` names its method and `b::Widget::new(7)` names another type's"
    );
}
