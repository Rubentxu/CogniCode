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
//!
//! Two facts the manifest is the only place that holds are read here: a
//! dependency that only exists for one platform is declared under a target
//! section, and it is a dependency all the same; and a dependency that names
//! a path outside the workspace names a crate that lives elsewhere, however
//! it is declared.

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
/// `tree-sitter` is written `tree_sitter` in the code that uses it. That is
/// the name the manifest gives it, and it is the name a path spells unless
/// the dependency renames its library (`[lib] name = "…"`), which the
/// manifest of the caller does not say.
pub fn external_crates(manifest: &Path) -> HashSet<String> {
    let Some(text) = read_manifest(manifest) else {
        return HashSet::new();
    };
    let Ok(parsed) = text.parse::<toml::Value>() else {
        return HashSet::new();
    };
    let (root, workspace) = workspace_of(manifest);
    let directory = manifest.parent();
    let mut names = HashSet::new();
    for table in dependency_tables(&parsed) {
        for (name, spec) in table {
            if is_local(name, spec, &workspace, root.as_deref(), directory) {
                continue;
            }
            names.insert(name.replace('-', "_"));
        }
    }
    names
}

/// Every table of a manifest that declares dependencies.
///
/// The three sections of the package, and the same three under each target:
/// a dependency that only exists for one platform is a dependency all the
/// same.
fn dependency_tables(parsed: &toml::Value) -> Vec<&toml::Table> {
    const SECTIONS: [&str; 3] = ["dependencies", "dev-dependencies", "build-dependencies"];
    let mut tables = Vec::new();
    for section in SECTIONS {
        if let Some(table) = parsed.get(section).and_then(toml::Value::as_table) {
            tables.push(table);
        }
    }
    if let Some(targets) = parsed.get("target").and_then(toml::Value::as_table) {
        for (_, target) in targets {
            let Some(target) = target.as_table() else {
                continue;
            };
            for section in SECTIONS {
                if let Some(table) = target.get(section).and_then(toml::Value::as_table) {
                    tables.push(table);
                }
            }
        }
    }
    tables
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
/// A dependency that names a path is a crate that lives here — as long as it
/// lives inside the workspace. One that lives outside it is somebody else's
/// crate, however it is declared: a vendored copy, a checkout of another
/// project, a submodule.
///
/// One that inherits from the workspace says so, and the workspace says
/// which: the inherited entry is the one that knows the path.
///
/// Without a workspace there is nothing to compare the path with, and the
/// manifest cannot tell a sibling crate from a vendored one. The dependency
/// is then taken for the project's own, as it was before.
fn is_local(
    name: &str,
    spec: &toml::Value,
    workspace: &toml::Table,
    root: Option<&Path>,
    directory: Option<&Path>,
) -> bool {
    let Some(table) = spec.as_table() else {
        return false;
    };
    if let Some(path) = table.get("path").and_then(toml::Value::as_str) {
        return lives_inside_the_root(path, directory, root);
    }
    if table.get("workspace").and_then(toml::Value::as_bool) != Some(true) {
        return false;
    }
    let Some(inherited) = workspace.get(name).and_then(toml::Value::as_table) else {
        return false;
    };
    match inherited.get("path").and_then(toml::Value::as_str) {
        Some(path) => lives_inside_the_root(path, root, root),
        None => false,
    }
}

/// Whether a path dependency lives inside the workspace.
///
/// A crate under the workspace root is a crate of this project; a crate
/// outside it is somebody else's.
fn lives_inside_the_root(path: &str, base: Option<&Path>, root: Option<&Path>) -> bool {
    let (Some(base), Some(root)) = (base, root) else {
        return true;
    };
    normalized(&base.join(path)).starts_with(normalized(root))
}

/// A path without the `.` and `..` components.
///
/// The components are resolved without touching the file system: a path that
/// is compared with another one has to be the same path, and the file system
/// is not consulted to know where a crate lives.
fn normalized(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
}

/// The workspace the package belongs to, and the dependencies it declares for
/// the packages that inherit them.
///
/// A package that inherits a dependency does not say where it comes from;
/// the workspace does, and that is what tells a sibling crate from a crate
/// pulled from a registry.
fn workspace_of(manifest: &Path) -> (Option<PathBuf>, toml::Table) {
    let mut directory = manifest.parent();
    while let Some(current) = directory {
        let path = current.join("Cargo.toml");
        if path.is_file()
            && let Some(text) = read_manifest(&path)
            && let Ok(parsed) = text.parse::<toml::Value>()
            && let Some(workspace) = parsed.get("workspace").and_then(toml::Value::as_table)
        {
            let dependencies = workspace
                .get("dependencies")
                .and_then(toml::Value::as_table)
                .cloned()
                .unwrap_or_default();
            return (Some(current.to_path_buf()), dependencies);
        }
        directory = current.parent();
    }
    (None, toml::Table::new())
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
             serde_json = \"1\"\ntree-sitter = { path = \"vendor/tree-sitter\" }\n",
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
            "the workspace's `tree-sitter` names a path inside the workspace: it is a crate of \
             this workspace: {crates:?}"
        );
    }

    /// A dependency that only exists for one platform is a dependency all
    /// the same.
    #[test]
    fn a_dependency_of_a_target_is_a_crate_from_outside() {
        let (_dir, file) = package(
            "[package]\nname = \"thing\"\n\n[target.'cfg(unix)'.dependencies]\n\
             serde_json = \"1\"\n",
        );
        let manifest = manifest_of(file.parent().expect("the file is in a directory"))
            .expect("the package has a manifest");
        let crates = external_crates(&manifest);

        assert!(
            crates.contains("serde_json"),
            "the package pulls `serde_json` from a registry for one platform: {crates:?}"
        );
    }

    /// A crate that lives outside the workspace is somebody else's, however
    /// the manifest declares it.
    #[test]
    fn a_path_dependency_outside_the_workspace_is_a_crate_from_outside() {
        let workspace = tempfile::tempdir().expect("a temporary workspace");
        let vendor = tempfile::tempdir().expect("a temporary vendor");
        let vendor_name = vendor
            .path()
            .file_name()
            .expect("the vendor has a name")
            .to_string_lossy()
            .to_string();
        std::fs::write(
            workspace.path().join("Cargo.toml"),
            "[workspace]\nmembers = [\"core\"]\n",
        )
        .expect("the workspace manifest is written");
        let package = workspace.path().join("core");
        std::fs::create_dir_all(package.join("src")).expect("the package is created");
        std::fs::write(
            package.join("Cargo.toml"),
            format!(
                "[package]\nname = \"core\"\n\n[dependencies]\n\
                 serde_json = {{ path = \"../../{vendor_name}\" }}\n"
            ),
        )
        .expect("the package manifest is written");
        let file = package.join("src").join("lib.rs");
        std::fs::write(&file, "pub fn run() {}\n").expect("the source is written");

        let manifest = manifest_of(file.parent().expect("the file is in a directory"))
            .expect("the package has a manifest");
        let crates = external_crates(&manifest);

        assert!(
            crates.contains("serde_json"),
            "the dependency names a path outside the workspace: it is a crate from outside it: \
             {crates:?}"
        );
    }

    /// A crate that lives inside the workspace is the project's own.
    #[test]
    fn a_path_dependency_inside_the_workspace_is_a_crate_of_this_workspace() {
        let workspace = tempfile::tempdir().expect("a temporary workspace");
        std::fs::write(
            workspace.path().join("Cargo.toml"),
            "[workspace]\nmembers = [\"core\"]\n",
        )
        .expect("the workspace manifest is written");
        let package = workspace.path().join("core");
        std::fs::create_dir_all(package.join("src")).expect("the package is created");
        std::fs::write(
            package.join("Cargo.toml"),
            "[package]\nname = \"core\"\n\n[dependencies]\n\
             graph-algos = { path = \"../algos\" }\n",
        )
        .expect("the package manifest is written");
        let file = package.join("src").join("lib.rs");
        std::fs::write(&file, "pub fn run() {}\n").expect("the source is written");

        let manifest = manifest_of(file.parent().expect("the file is in a directory"))
            .expect("the package has a manifest");
        let crates = external_crates(&manifest);

        assert!(
            !crates.contains("graph_algos"),
            "the dependency names a path inside the workspace: it is a crate of this workspace: \
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
