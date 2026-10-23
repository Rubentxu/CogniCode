//! The product observes one set of files.
//!
//! The inventory walks them, the watcher watches them, and the endpoint that
//! announces the watcher names them. Three consumers of one decision: while
//! they disagree, one of them is answering a question nobody asked, and the
//! person asking believes the answer.
//!
//! Two things decide what the product observes. The first is what the file
//! is: the registry of languages the product can read says so, and a file it
//! cannot name is a file it does not read. The second is where the file
//! lives: the product does not look inside build output, caches or
//! dependencies, and it respects what the repository itself excludes.

use std::path::{Path, PathBuf};

use cognicode_core::application::ingest::observation::Observation;
use cognicode_core::application::ingest::scan::{
    classify_file, noise_directories, observed_extensions, walk_files,
};
use cognicode_core::application::ingest::types::FileType;
use cognicode_core::infrastructure::parser::Language;

/// Writes a file, creating the directories that lead to it.
fn write(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("the parent directory is creatable");
    }
    std::fs::write(path, contents).expect("the file is writable");
}

/// A workspace with the files a real one has: sources, documents, a
/// configuration, a dependency tree, a build tree, and two places the
/// repository itself excludes.
fn workspace() -> tempfile::TempDir {
    let directory = tempfile::TempDir::new().expect("the workspace is creatable");
    let root = directory.path();

    write(&root.join("src/main.rs"), "fn main() {}");
    write(&root.join("src/parser.hs"), "main = return ()");
    write(&root.join("docs/guide.md"), "# Guide");
    write(&root.join("config/settings.toml"), "key = 1");
    write(&root.join("scripts/run.sh"), "echo hello");
    write(&root.join("image.png"), "not really a png");

    write(
        &root.join("node_modules/left-pad/index.js"),
        "module.exports = 1;",
    );
    write(&root.join("target/debug/build.rs"), "fn main() {}");
    write(&root.join("build/output.py"), "print(1)");
    write(&root.join(".venv/lib/kept.txt"), "kept");

    write(&root.join(".gitignore"), "excluded/\n");
    write(&root.join("excluded/secret.rs"), "fn secret() {}");

    write(&root.join("nested/.gitignore"), "notes.txt\n");
    write(&root.join("nested/notes.txt"), "text");

    directory
}

/// The inventory and the watcher look at the same files.
#[test]
fn the_inventory_and_the_watcher_look_at_the_same_files() {
    let workspace = workspace();
    let observation = Observation::of(workspace.path());

    let walked = walk_files(workspace.path());
    assert!(
        !walked.is_empty(),
        "the fixture has to be walked for this to mean anything"
    );

    let expected: Vec<PathBuf> = walked
        .iter()
        .filter(|path| classify_file(path).0 != FileType::Other)
        .cloned()
        .collect();
    let observed: Vec<PathBuf> = walked
        .iter()
        .filter(|path| observation.observes(path))
        .cloned()
        .collect();

    assert_eq!(
        observed, expected,
        "the watcher and the inventory disagree about which files the product reads"
    );
}

/// The files the product reads are the ones the registry names.
#[test]
fn the_product_reads_the_languages_the_registry_names() {
    let workspace = workspace();
    let observation = Observation::of(workspace.path());

    for extension in ["hs", "scala", "sh", "jl", "erl", "zig", "dart", "ex", "ps1"] {
        let path = workspace.path().join(format!("sources/module.{extension}"));
        assert!(
            observation.observes(&path),
            "the registry reads a language whose extension is `{extension}`, and the watcher does not look at it"
        );
    }
}

/// What lives in a build tree, a cache or a dependency tree is not the
/// project the product describes.
#[test]
fn the_watcher_does_not_look_inside_the_places_the_product_does_not_read() {
    let workspace = workspace();
    let observation = Observation::of(workspace.path());

    for relative in [
        "node_modules/left-pad/index.js",
        "target/debug/build.rs",
        "build/output.py",
        ".venv/lib/kept.txt",
    ] {
        let path = workspace.path().join(relative);
        assert!(
            !observation.observes(&path),
            "the product does not read `{relative}`, and the watcher looks at it anyway"
        );
    }
}

/// What the repository excludes is excluded from the watch too.
#[test]
fn the_watcher_respects_what_the_repository_excludes() {
    let workspace = workspace();
    let observation = Observation::of(workspace.path());

    for relative in ["excluded/secret.rs", "nested/notes.txt"] {
        let path = workspace.path().join(relative);
        assert!(
            !observation.observes(&path),
            "the repository excludes `{relative}`, and the watcher looks at it anyway"
        );
    }
}

/// A file the product cannot name is not a file it reads.
#[test]
fn a_file_the_product_cannot_name_is_not_observed() {
    let workspace = workspace();
    let observation = Observation::of(workspace.path());

    assert!(!observation.observes(&workspace.path().join("image.png")));
}

/// The announced extensions are the ones the product reads.
#[test]
fn the_announced_extensions_are_the_extensions_the_product_reads() {
    let announced = observed_extensions();

    for extension in announced {
        let path = PathBuf::from(format!("file.{extension}"));
        assert_ne!(
            classify_file(&path).0,
            FileType::Other,
            "the endpoint announces `{extension}`, which the product cannot name"
        );
    }

    for language in Language::all_languages() {
        for extension in language.extensions() {
            assert!(
                announced.contains(extension),
                "the registry reads `{extension}` and the endpoint does not announce it"
            );
        }
    }

    for extension in ["md", "mdx", "qmd", "txt", "rst", "toml", "ini"] {
        assert!(
            announced.contains(&extension),
            "the product reads `{extension}` and the endpoint does not announce it"
        );
    }

    assert!(
        !announced.contains(&"png"),
        "the product does not read images, and the endpoint announces them"
    );
}

/// The directories the product does not read are named where they are kept.
#[test]
fn the_directories_the_product_does_not_read_are_named() {
    let directories = noise_directories();

    for name in ["node_modules", "target", "build", "dist"] {
        assert!(
            directories.contains(&name),
            "the product does not read `{name}`, and nobody says so"
        );
    }
}
