//! `get_implementors` answers from the graph the product builds, with no
//! language server: the parents a Kotlin, Groovy or C# declaration names
//! become `Inherits` edges in the project graph.
//!
//! The corpus is written to a temporary directory, so the test needs
//! neither network nor a process. A parent name that two declarations
//! share stays out of the graph and is reported as pending.

use cognicode_core::domain::traits::GraphStore;
use cognicode_core::infrastructure::graph::per_file_graph::PerFileGraphCache;
use cognicode_core::infrastructure::persistence::memory_graph_store::InMemoryGraphStore;
use cognicode_core::interface::mcp::handlers::HandlerContext;
use cognicode_core::interface::mcp::handlers::graph_query_handlers::{
    GetImplementorsInput, handle_get_implementors,
};

fn write(dir: &std::path::Path, relative: &str, text: &str) -> std::path::PathBuf {
    let path = dir.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, text).unwrap();
    path
}

/// Builds the project graph over `files` and asks `get_implementors` for
/// `parent`, the way the MCP tool does.
async fn implementors_of(files: &[std::path::PathBuf], parent: &str) -> Vec<String> {
    let cache = PerFileGraphCache::new();
    let paths: Vec<&std::path::Path> = files.iter().map(|p| p.as_path()).collect();
    let report = cache.merge_with_report(&paths);

    let store = InMemoryGraphStore::new();
    store.save_graph(&report.graph).unwrap();
    let ctx = HandlerContext::builder().with_graph_store(store).build();

    let mut names = handle_get_implementors(
        &ctx,
        GetImplementorsInput {
            trait_name: parent.to_string(),
        },
    )
    .await
    .expect("the parent is declared in the corpus")
    .implementors;
    names.sort();
    names
}

#[tokio::test]
async fn kotlin_implementors_are_found_across_files() {
    let dir = tempfile::tempdir().unwrap();
    let files = vec![
        write(dir.path(), "walker.kt", "interface Walker\n"),
        write(dir.path(), "animal.kt", "open class Animal\n"),
        write(
            dir.path(),
            "pets.kt",
            "class Dog : Animal(), Walker\nclass Cat : Walker\n",
        ),
    ];
    assert_eq!(implementors_of(&files, "Walker").await, ["Cat", "Dog"]);
    assert_eq!(implementors_of(&files, "Animal").await, ["Dog"]);
}

#[tokio::test]
async fn groovy_implementors_are_found_across_files() {
    let dir = tempfile::tempdir().unwrap();
    let files = vec![
        write(dir.path(), "Runner.groovy", "interface Runner {}\n"),
        write(dir.path(), "Base.groovy", "class Base {}\n"),
        write(
            dir.path(),
            "Impl.groovy",
            "class Fast extends Base implements Runner {}\nclass Slow implements Runner {}\n",
        ),
    ];
    assert_eq!(implementors_of(&files, "Runner").await, ["Fast", "Slow"]);
    assert_eq!(implementors_of(&files, "Base").await, ["Fast"]);
}

#[tokio::test]
async fn csharp_implementors_are_found_across_files() {
    let dir = tempfile::tempdir().unwrap();
    let files = vec![
        write(dir.path(), "IFlyer.cs", "interface IFlyer { }\n"),
        write(dir.path(), "Bird.cs", "class Bird : IFlyer { }\n"),
        write(dir.path(), "Plane.cs", "class Plane : IFlyer { }\n"),
    ];
    assert_eq!(implementors_of(&files, "IFlyer").await, ["Bird", "Plane"]);
}

#[tokio::test]
async fn a_parent_name_two_declarations_share_invents_no_edge() {
    let dir = tempfile::tempdir().unwrap();
    let files = vec![
        write(dir.path(), "a/Mover.kt", "interface Mover\n"),
        write(dir.path(), "b/Mover.kt", "interface Mover\n"),
        write(dir.path(), "c/Car.kt", "class Car : Mover\n"),
    ];
    assert!(implementors_of(&files, "Mover").await.is_empty());
}
