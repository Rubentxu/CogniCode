//! What a package declares about the crates it pulls.
//!
//! A path that starts with a crate the package pulls from a registry names
//! something outside the project: the package declares it, so the project
//! does not own it, and a call written through it is not a call to the
//! project's own declaration of the same name.
//!
//! A dependency that names a path is a crate that lives here — a sibling of
//! the package in this workspace — and the project does own it:
//! `cognicode_graph_algos::page_rank` is as much the project's as
//! `crate::…` is. The manifest is the only place that says which of the two
//! a dependency is, and it is read once per package.
//!
//! What a package declares is not the same as what it may name: a dependency
//! declared under another name (`serde = { package = "serde" }`) is spelled
//! with the name the manifest gives it, and that is the name a path spells
//! too.

use crate::domain::value_objects::Language;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// What the manifests of a project declare, read once each.
///
/// The answer is asked once per call and the manifests do not change while
/// the build runs, so they are read once and kept. Reading a manifest is
/// not cheap — a package that inherits its dependencies reads the manifest
/// of the workspace too — and a project has one directory per source
/// directory and one manifest per package: keeping the answer per
/// directory alone would read the same manifest once per directory.
#[derive(Default)]
pub struct Manifests {
    /// The crates the package of each directory pulls from outside.
    by_directory: HashMap<PathBuf, Arc<HashSet<String>>>,
    /// The crates each manifest declares.
    by_manifest: HashMap<PathBuf, Arc<HashSet<String>>>,
}

impl Manifests {
    /// The crates the package that owns a directory pulls from outside the
    /// project.
    ///
    /// A directory that belongs to no package belongs to a package that
    /// says nothing, and a package that says nothing declares no crate
    /// from outside.
    pub fn external_crates_of(&mut self, directory: &Path) -> Arc<HashSet<String>> {
        if let Some(crates) = self.by_directory.get(directory) {
            return crates.clone();
        }
        let crates = match manifest_of(directory) {
            Some(manifest) => {
                let known = self.by_manifest.get(&manifest).cloned();
                match known {
                    Some(crates) => crates,
                    None => {
                        let crates = Arc::new(external_crates(&manifest));
                        self.by_manifest.insert(manifest.clone(), crates.clone());
                        crates
                    }
                }
            }
            None => Arc::new(HashSet::new()),
        };
        self.by_directory
            .insert(directory.to_path_buf(), crates.clone());
        crates
    }
}

/// Whether a manifest governs a file.
///
/// A `Cargo.toml` is the manifest of a Rust package, and it governs the Rust
/// files of that package. A file of another language that happens to live
/// inside the package is not one of them: a Java `log::info` is not the
/// `log` crate the Rust package pulls, and a manifest that says nothing
/// about it must not judge it.
pub fn is_governed_by_a_manifest(file: &Path) -> bool {
    let Some(extension) = file.extension().and_then(|extension| extension.to_str()) else {
        return false;
    };
    Language::Rust.extensions().contains(&extension)
}

/// The crates a package pulls from outside the project.
///
/// The names are the ones a path spells: a dependency declared as
/// `tree-sitter` is written `tree_sitter` in the code that uses it.
pub fn external_crates(manifest: &Path) -> HashSet<String> {
    let Some(text) = read_manifest(manifest) else {
        return HashSet::new();
    };
    let Ok(parsed) = text.parse::<toml::Value>() else {
        return HashSet::new();
    };
    let workspace = workspace_dependencies(manifest);
    let mut names = HashSet::new();
    for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
        let Some(table) = parsed.get(section).and_then(toml::Value::as_table) else {
            continue;
        };
        for (name, spec) in table {
            if is_local(name, spec, &workspace) {
                continue;
            }
            names.insert(name.replace('-', "_"));
        }
    }
    names
}

/// The manifest that governs a directory, when the directory belongs to a
/// Rust package.
///
/// The nearest one is the package's own: a file under `crates/core/src`
/// belongs to the package that declares `crates/core/Cargo.toml`, and not
/// to the workspace that declares the crates above it.
pub fn manifest_of(directory: &Path) -> Option<PathBuf> {
    let mut current = Some(directory);
    while let Some(dir) = current {
        let candidate = dir.join("Cargo.toml");
        if candidate.is_file() {
            return Some(candidate);
        }
        current = dir.parent();
    }
    None
}

/// The crates the package that owns a directory pulls from outside the
/// project.
///
/// A directory that belongs to no package belongs to a package that says
/// nothing, and a package that says nothing is one that declares no crate
/// from outside.
///
/// This is the question without a memory behind it: a caller that asks it
/// once is answered by reading the manifest. A caller that asks it many
/// times keeps the answer in [`Manifests`].
pub fn external_crates_of(directory: &Path) -> HashSet<String> {
    let mut manifests = Manifests::default();
    let crates = manifests.external_crates_of(directory);
    crates.as_ref().clone()
}

/// Whether a dependency is a crate of this workspace.
///
/// A dependency that names a path is a crate that lives here. One that
/// inherits from the workspace says so, and the workspace says which: the
/// inherited entry is the one that knows the path.
fn is_local(name: &str, spec: &toml::Value, workspace: &toml::Table) -> bool {
    let Some(table) = spec.as_table() else {
        return false;
    };
    if table.contains_key("path") {
        return true;
    }
    if table.get("workspace").and_then(toml::Value::as_bool) != Some(true) {
        return false;
    }
    workspace
        .get(name)
        .and_then(toml::Value::as_table)
        .is_some_and(|inherited| inherited.contains_key("path"))
}

/// The dependencies the workspace declares for the packages that inherit
/// them.
///
/// A package that inherits a dependency does not say where it comes from;
/// the workspace does, and that is what tells a sibling crate from a crate
/// pulled from a registry.
fn workspace_dependencies(manifest: &Path) -> toml::Table {
    let mut directory = manifest.parent();
    while let Some(current) = directory {
        let path = current.join("Cargo.toml");
        if path.is_file()
            && let Some(text) = read_manifest(&path)
            && let Ok(parsed) = text.parse::<toml::Value>()
            && let Some(workspace) = parsed.get("workspace").and_then(toml::Value::as_table)
        {
            return workspace
                .get("dependencies")
                .and_then(toml::Value::as_table)
                .cloned()
                .unwrap_or_default();
        }
        directory = current.parent();
    }
    toml::Table::new()
}

/// The text of a manifest, when it can be read.
fn read_manifest(manifest: &Path) -> Option<String> {
    std::fs::read_to_string(manifest).ok()
}

#[cfg(test)]
mod tests {
    use super::{Manifests, external_crates, is_governed_by_a_manifest, manifest_of};
    use std::path::{Path, PathBuf};

    /// A package with the given manifest, and the path of its source file.
    fn package(manifest: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("a temporary package");
        std::fs::write(dir.path().join("Cargo.toml"), manifest).expect("the manifest is written");
        let src = dir.path().join("src");
        std::fs::create_dir_all(&src).expect("the source directory is created");
        let file = src.join("lib.rs");
        std::fs::write(&file, "pub fn run() {}\n").expect("the source is written");
        (dir, file)
    }

    #[test]
    fn a_registry_dependency_is_a_crate_from_outside() {
        let (_dir, file) =
            package("[package]\nname = \"thing\"\n\n[dependencies]\nserde_json = \"1\"\n");
        let manifest = manifest_of(file.parent().expect("the file is in a directory"))
            .expect("the package has a manifest");
        let crates = external_crates(&manifest);

        assert!(
            crates.contains("serde_json"),
            "the package pulls `serde_json` from a registry: {crates:?}"
        );
    }

    #[test]
    fn a_dependency_that_names_a_path_is_a_crate_of_this_workspace() {
        let (_dir, file) = package(
            "[package]\nname = \"thing\"\n\n[dependencies]\ngraph-algos = { path = \"../algos\" }\n",
        );
        let manifest = manifest_of(file.parent().expect("the file is in a directory"))
            .expect("the package has a manifest");
        let crates = external_crates(&manifest);

        assert!(
            !crates.contains("graph_algos"),
            "the dependency names a path: it is a crate of this workspace, not a crate from \
             outside it: {crates:?}"
        );
    }

    #[test]
    fn a_dependency_that_inherits_from_the_workspace_is_where_the_workspace_says() {
        let dir = tempfile::tempdir().expect("a temporary workspace");
        let root = dir.path().join("Cargo.toml");
        std::fs::write(
            &root,
            "[workspace]\nmembers = [\"core\"]\n\n[workspace.dependencies]\n\
             serde_json = \"1\"\ntree-sitter = { path = \"../tree-sitter\" }\n",
        )
        .expect("the workspace manifest is written");
        let package = dir.path().join("core");
        std::fs::create_dir_all(package.join("src")).expect("the package is created");
        std::fs::write(
            package.join("Cargo.toml"),
            "[package]\nname = \"core\"\n\n[dependencies]\n\
             serde_json = { workspace = true }\ntree-sitter = { workspace = true }\n",
        )
        .expect("the package manifest is written");
        let file = package.join("src").join("lib.rs");
        std::fs::write(&file, "pub fn run() {}\n").expect("the source is written");

        let manifest = manifest_of(file.parent().expect("the file is in a directory"))
            .expect("the package has a manifest");
        let crates = external_crates(&manifest);

        assert!(
            crates.contains("serde_json"),
            "the workspace pulls `serde_json` from a registry: {crates:?}"
        );
        assert!(
            !crates.contains("tree_sitter"),
            "the workspace's `tree-sitter` names a path: it is a crate of this workspace: \
             {crates:?}"
        );
    }

    #[test]
    fn a_file_without_a_package_has_no_manifest() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let file = dir.path().join("lib.rs");
        std::fs::write(&file, "pub fn run() {}\n").expect("the source is written");

        assert!(
            manifest_of(file.parent().expect("the file is in a directory")).is_none(),
            "a file that belongs to no package is governed by no manifest"
        );
    }

    /// A manifest governs the Rust files of its package, and no other.
    #[test]
    fn a_manifest_governs_the_rust_files_of_its_package() {
        assert!(
            is_governed_by_a_manifest(Path::new("crates/core/src/lib.rs")),
            "the Rust file of the package is one the manifest governs"
        );
        assert!(
            !is_governed_by_a_manifest(Path::new("crates/core/src/Main.java")),
            "the Java file that lives inside the package is not one the manifest governs: the \
             `log` crate the package pulls is not the Java `log`"
        );
        assert!(
            !is_governed_by_a_manifest(Path::new("crates/core/src/lib")),
            "a file without an extension is not one the manifest governs"
        );
    }

    /// The manifests are read once and kept: the same package says the same
    /// thing twice.
    #[test]
    fn the_manifests_are_read_once_and_kept() {
        let (_dir, file) =
            package("[package]\nname = \"thing\"\n\n[dependencies]\nserde_json = \"1\"\n");
        let directory = file
            .parent()
            .expect("the file is in a directory")
            .to_path_buf();
        let mut manifests = Manifests::default();

        let first = manifests.external_crates_of(&directory);
        let second = manifests.external_crates_of(&directory);

        assert_eq!(first, second, "the same package says the same thing twice");
        assert!(
            first.contains("serde_json"),
            "the package pulls `serde_json` from a registry: {first:?}"
        );
    }

    /// A directory that belongs to no package declares no crate.
    #[test]
    fn a_directory_without_a_package_declares_no_crate() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let mut manifests = Manifests::default();

        assert!(
            manifests.external_crates_of(dir.path()).is_empty(),
            "a directory that belongs to no package is governed by no manifest"
        );
    }
}
