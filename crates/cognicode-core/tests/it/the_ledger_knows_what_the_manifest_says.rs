//! The reuse ledger knows what the manifest says.
//!
//! ## The defect
//!
//! The answer to a call written through a crate (`serde_json::to_value(1)`)
//! depends on what the package of the caller declares: the manifest is the
//! only place that says the crate comes from outside the project. The reuse
//! ledger did not track the manifest, so it refused to keep the answer of
//! any call written through a crate path: every one of them was answered
//! again on every build, and a build that touched a single file paid for
//! the whole project.
//!
//! ## What is measured
//!
//! A package with a manifest and two calls — one written through a crate the
//! package pulls, one written without a path — and two builds of the same
//! tree.
//!
//! - the answer of the call written through the crate is kept;
//! - a manifest that starts declaring the crate invalidates the answer that
//!   was kept, and the call is answered again;
//! - the manifest of one package invalidates nothing of another.
//!
//! ## What must keep working
//!
//! - a manifest that changed invalidates the answers of the files it
//!   governs, and only of those;
//! - the graph published in warm is the graph published in cold.

use cognicode_core::application::services::analysis_service::AnalysisService;
use cognicode_core::domain::aggregates::call_graph::CallGraph;
use std::collections::BTreeSet;
use std::path::PathBuf;

/// A package that pulls `serde_json` from a registry.
const MANIFEST: &str = "\
[package]
name = \"fixture\"

[dependencies]
serde_json = \"1\"
";

/// The same package, pulling nothing.
const BARE_MANIFEST: &str = "\
[package]
name = \"fixture\"
";

/// A package that declares a function whose name a crate of a registry also
/// has, and one more that a call names without a path.
const LIB_RS: &str = "\
/// The project's own function, whose name a crate of a registry also has.
pub fn to_value() -> u32 {
    1
}

/// The project's own function, called without a path.
pub fn own() -> u32 {
    1
}
";

/// A call written through the crate the package pulls.
const CALLER_RS: &str = "\
/// A call through the crate the package pulls.
pub fn from_the_crate() -> u32 {
    serde_json::to_value(1)
}
";

/// A call written without a path.
const OTHER_RS: &str = "\
/// A call written without a path.
pub fn calls_own() -> u32 {
    own()
}
";

/// The first package of the tree: it declares the function the crate path
/// spells.
const A_LIB_RS: &str = "\
/// The project's own function, whose name a crate of a registry also has.
pub fn to_value() -> u32 {
    1
}
";

/// The second package of the tree: it declares the function the ordinary
/// call names.
const B_LIB_RS: &str = "\
/// The project's own function, called without a path.
pub fn own() -> u32 {
    1
}
";

/// A package with the given manifest and files, and the path of the tree.
fn package(manifest: &str, files: &[(&str, &str)]) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("a temporary package");
    let root = dir.path().to_path_buf();
    std::fs::write(root.join("Cargo.toml"), manifest).expect("the manifest is written");
    let src = root.join("src");
    std::fs::create_dir_all(&src).expect("the source directory is created");
    for (name, content) in files {
        std::fs::write(src.join(name), content).expect("the fixture is written");
    }
    (dir, root)
}

/// Two packages in one tree: one calls through a crate, the other calls
/// without a path.
///
/// Each package has its own manifest, and each declares a name the other
/// does not: a name declared twice would be a homonym, and a homonym is a
/// refusal the test is not about.
fn two_packages() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("a temporary tree");
    let root = dir.path().to_path_buf();
    for (package, library) in [("a", A_LIB_RS), ("b", B_LIB_RS)] {
        let src = root.join(package).join("src");
        std::fs::create_dir_all(&src).expect("the source directory is created");
        std::fs::write(root.join(package).join("Cargo.toml"), BARE_MANIFEST)
            .expect("the manifest is written");
        std::fs::write(src.join("lib.rs"), library).expect("the fixture is written");
    }
    std::fs::write(root.join("a/src/caller.rs"), CALLER_RS).expect("the fixture is written");
    std::fs::write(root.join("b/src/other.rs"), OTHER_RS).expect("the fixture is written");
    (dir, root)
}

/// The edges of the graph, as `(origin, destination, kind)`.
fn edges(graph: &CallGraph) -> BTreeSet<(String, String, String)> {
    graph
        .all_dependencies()
        .map(|(from, to, kind)| {
            (
                from.as_str().to_string(),
                to.as_str().to_string(),
                format!("{kind:?}"),
            )
        })
        .collect()
}

/// The identity the graph gives to the declaration of `name` in the file
/// whose path ends with `file`.
///
/// The identity is read from the graph and not written by hand: a test that
/// fixes the shape of the identity fails when the shape changes and not
/// when the relation changes.
fn id_of(graph: &CallGraph, file: &str, name: &str) -> String {
    graph
        .symbols()
        .find(|symbol| symbol.location().file().ends_with(file) && symbol.name() == name)
        .map(|symbol| symbol.fully_qualified_name().to_string())
        .unwrap_or_else(|| panic!("the graph declares `{name}` in a file ending with `{file}`"))
}

/// The answer of a call written through a crate is kept when neither the
/// file nor the manifest changed.
///
/// The call is answered by what the manifest says, and the manifest did not
/// change: the answer of the last build is the answer of this one.
#[test]
fn a_call_written_through_a_crate_keeps_its_answer() {
    let (_tmp, root) = package(
        MANIFEST,
        &[
            ("lib.rs", LIB_RS),
            ("caller.rs", CALLER_RS),
            ("other.rs", OTHER_RS),
        ],
    );
    let service = AnalysisService::new();

    service.build_project_graph(&root).expect("the cold build");
    let cold = service
        .get_reuse_metrics()
        .expect("the build publishes how much it reused");
    assert_eq!(
        cold.relationships_reused, 0,
        "a cold build keeps nothing; got {cold:?}"
    );
    assert_eq!(
        cold.relationships_resolved, 2,
        "the corpus states two calls, one through a crate and one without a path; got {cold:?}"
    );

    service.build_project_graph(&root).expect("the warm build");
    let warm = service
        .get_reuse_metrics()
        .expect("the build publishes how much it reused");

    assert_eq!(
        warm.relationships_resolved, 0,
        "nothing changed, so no answer has to be computed again; got {warm:?}"
    );
    assert_eq!(
        warm.relationships_reused, 2,
        "the call written through the crate keeps its answer too; got {warm:?}"
    );
}

/// A build that touches one file keeps the answers of the files that did
/// not change.
///
/// The ledger exists for that: a consumer that changed one file paid for
/// the whole project otherwise. The calls written through a crate were the
/// ones that paid — they were answered again on every build, whatever the
/// build touched.
#[test]
fn a_build_that_touches_one_file_keeps_the_answers_of_the_others() {
    let (_tmp, root) = package(
        MANIFEST,
        &[
            ("lib.rs", LIB_RS),
            ("caller.rs", CALLER_RS),
            ("other.rs", OTHER_RS),
        ],
    );
    let service = AnalysisService::new();

    service.build_project_graph(&root).expect("the cold build");

    // One file changes: the body of the function that calls `own`, and one
    // declaration more. The call written through the crate lives in another
    // file, and the manifest did not change.
    std::fs::write(
        root.join("src/other.rs"),
        "/// A call written without a path.\npub fn calls_own() -> u32 {\n    own()\n}\n\n\
         /// A declaration that is new to the project.\npub fn another() -> u32 {\n    2\n}\n",
    )
    .expect("the file is rewritten");
    service.build_project_graph(&root).expect("the warm build");

    let warm = service
        .get_reuse_metrics()
        .expect("the build publishes how much it reused");
    assert_eq!(
        warm.relationships_reused, 1,
        "the answer of the call written through the crate is kept; got {warm:?}"
    );
    assert_eq!(
        warm.relationships_resolved, 1,
        "only the file that changed is answered again; got {warm:?}"
    );
}

/// A manifest that starts declaring the crate invalidates the answer that
/// was kept.
///
/// The answer depended on what the package declared, and the package now
/// declares something else: the kept answer is an answer about a package
/// that no longer exists.
#[test]
fn a_manifest_that_changes_invalidates_the_answer_it_depends_on() {
    let (_tmp, root) = package(
        BARE_MANIFEST,
        &[("lib.rs", LIB_RS), ("caller.rs", CALLER_RS)],
    );
    let service = AnalysisService::new();

    service.build_project_graph(&root).expect("the cold build");
    let graph = service.get_project_graph();
    let from_the_crate = id_of(&graph, "caller.rs", "from_the_crate");
    let to_value = id_of(&graph, "lib.rs", "to_value");
    assert!(
        edges(&graph).contains(&(
            from_the_crate.clone(),
            to_value.clone(),
            "Calls".to_string()
        )),
        "the manifest declares nothing, so the call is the project's own"
    );

    // Nothing changed: the answer is kept.
    service.build_project_graph(&root).expect("the warm build");
    let warm = service
        .get_reuse_metrics()
        .expect("the build publishes how much it reused");
    assert_eq!(
        warm.relationships_reused, 1,
        "the answer of the call is kept; got {warm:?}"
    );
    assert_eq!(
        warm.relationships_resolved, 0,
        "nothing has to be answered again; got {warm:?}"
    );

    // The manifest now declares the crate: the call names a function of
    // somebody else's.
    std::fs::write(root.join("Cargo.toml"), MANIFEST).expect("the manifest is rewritten");
    service.build_project_graph(&root).expect("the warm build");
    let after = service
        .get_reuse_metrics()
        .expect("the build publishes how much it reused");
    assert_eq!(
        after.relationships_resolved, 1,
        "the answer that was kept depended on what the manifest said; got {after:?}"
    );
    assert_eq!(
        after.relationships_reused, 0,
        "the answer that was kept does not hold any more; got {after:?}"
    );

    let graph = service.get_project_graph();
    assert!(
        !edges(&graph).contains(&(from_the_crate, to_value, "Calls".to_string())),
        "the call names a function of a crate the package pulls"
    );
}

/// The manifest of one package invalidates nothing of another.
///
/// A manifest says what its own package pulls, and says nothing about the
/// package next door. A ledger that invalidated every answer of the
/// project when one manifest changed would keep nothing, and would pay the
/// whole project on every build.
#[test]
fn the_manifest_of_one_package_invalidates_nothing_of_another() {
    let (_tmp, root) = two_packages();
    let service = AnalysisService::new();

    service.build_project_graph(&root).expect("the cold build");
    let cold = service
        .get_reuse_metrics()
        .expect("the build publishes how much it reused");
    assert_eq!(
        cold.relationships_resolved, 2,
        "the two packages state one call each; got {cold:?}"
    );

    // Nothing changed: both packages keep their answers.
    service.build_project_graph(&root).expect("the warm build");
    let warm = service
        .get_reuse_metrics()
        .expect("the build publishes how much it reused");
    assert_eq!(
        warm.relationships_reused, 2,
        "both packages keep their answers; got {warm:?}"
    );
    assert_eq!(
        warm.relationships_resolved, 0,
        "nothing has to be answered again; got {warm:?}"
    );

    // The manifest of the first package declares the crate.
    std::fs::write(root.join("a/Cargo.toml"), MANIFEST).expect("the manifest is rewritten");
    service.build_project_graph(&root).expect("the warm build");
    let after = service
        .get_reuse_metrics()
        .expect("the build publishes how much it reused");
    assert_eq!(
        after.relationships_resolved, 1,
        "the call of the package whose manifest changed is answered again; got {after:?}"
    );
    assert_eq!(
        after.relationships_reused, 1,
        "the call of the other package keeps its answer; got {after:?}"
    );
}
