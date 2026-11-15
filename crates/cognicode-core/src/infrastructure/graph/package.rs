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

use crate::domain::findings::digest::sha256_raw;
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
    /// The manifests that could not be judged, and why.
    ///
    /// A manifest that could not be read answers exactly what a manifest
    /// that declares nothing answers, and the two are not the same answer:
    /// the first one is a hole of the coverage, because the judgment of
    /// the crate-path rule could not be made for that package.
    holes: Vec<(PathBuf, ManifestHole)>,
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
                        // A manifest that could not be judged says nothing
                        // about the crates of the package, and the hole it
                        // leaves is kept: the judgment of the crate-path rule
                        // could not be made for that package, and the edge it
                        // may hold stays in the graph.
                        let crates = match external_crates(&manifest) {
                            Ok(crates) => Arc::new(crates),
                            Err(hole) => {
                                self.holes.push((manifest.clone(), hole));
                                Arc::new(HashSet::new())
                            }
                        };
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

    /// Whether the crate a path starts at is one the package that writes the
    /// call pulls from outside the project.
    ///
    /// The package that writes the call is the one that says: a call in
    /// `crates/core` is judged by what `crates/core` declares, and not by
    /// what another package of the workspace declares.
    ///
    /// A file that belongs to no package, and a package whose manifest
    /// cannot be read, is a package that says nothing: the call is not
    /// judged, and the edge it may hold is kept. So is a file no manifest
    /// governs: a `Cargo.toml` speaks about the Rust files of its package.
    pub fn is_external_crate(&mut self, head: &str, caller_file: Option<&Path>) -> bool {
        let Some(file) = caller_file else {
            return false;
        };
        if !is_governed_by_a_manifest(file) {
            return false;
        }
        let Some(directory) = file.parent() else {
            return false;
        };
        self.external_crates_of(directory).contains(head)
    }

    /// How many manifests have been read.
    ///
    /// The answer the cache holds is one manifest per package: a project of
    /// many files in one package reads it once.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn manifests_read(&self) -> usize {
        self.by_manifest.len()
    }

    /// The manifests that could not be judged, and why.
    ///
    /// The list is asked of the owner of the reading, so the routes that
    /// judge do not compose it on their own: two answers waiting to drift
    /// apart is what this project calls a hole in the hole.
    pub fn holes(&self) -> &[(PathBuf, ManifestHole)] {
        &self.holes
    }
}

/// Why a manifest could not be judged.
///
/// A manifest that cannot be read and a manifest whose text is not a
/// manifest are two different failures, and the coverage says which: the
/// first one is a file that could not be read, the second one is a file that
/// was read and is not what it says to be.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestHole {
    /// The manifest could not be read.
    Unreadable(String),
    /// The manifest was read, but its text is not a manifest.
    Unparseable(String),
}

impl ManifestHole {
    /// The sentence that says what happened, naming the package.
    ///
    /// The package is named here because the manifest that could not be read
    /// is the only thing that says which package it belongs to, and a
    /// coverage that counts a hole without naming what is missing is a
    /// count the reader cannot act on.
    pub fn detail(&self, package: &str) -> String {
        match self {
            ManifestHole::Unreadable(reason) => {
                format!("the manifest of the package `{package}` could not be read: {reason}")
            }
            ManifestHole::Unparseable(reason) => {
                format!("the manifest of the package `{package}` is not a manifest: {reason}")
            }
        }
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

/// The file that is a manifest, and the hash of its bytes.
///
/// The hash says whether the manifest is the same one another build read: it
/// is the same rule the source files follow, over the same bytes. A manifest
/// that changed is a manifest that says something else, and what it says is
/// what the answer to a call written through a crate depends on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestFile {
    /// The path of the manifest, as the build spells it.
    pub path: PathBuf,
    /// The hash of the bytes of the manifest.
    pub hash: [u8; 32],
}

/// The manifests that govern the files of a package: the manifest of the
/// package, and the manifest of the workspace that package inherits from.
///
/// What a package pulls from outside the project is declared by its own
/// manifest, and the dependencies it inherits are declared by the manifest of
/// the workspace. So both are what decides the answer, and both are hashed:
/// a workspace that changes the path of an inherited dependency changes the
/// answer of every package that inherits it, without changing a byte of
/// theirs.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GoverningManifests {
    /// The manifests that govern the files, by path.
    pub manifests: Vec<ManifestFile>,
}

/// The manifests that govern the files of a package, and the hash of their
/// bytes.
///
/// `None` when the manifest cannot be read: a manifest nobody can read says
/// nothing, and a hash of nothing would be a hash of the absence of a
/// manifest.
///
/// The manifest of the package and the manifest of the workspace that
/// inherits from it are two readings of two files, and a project has one
/// package per directory and one workspace for all of them: the caller that
/// reads the manifest of every directory of a project reads the manifest of
/// every package of it twice.
pub fn governing_manifests_of(manifest: &Path) -> Option<GoverningManifests> {
    let mut manifests = vec![manifest_with_hash(manifest)?];
    if let Some(workspace) = workspace_of(manifest).0
        && workspace != manifest
    {
        manifests.push(manifest_with_hash(&workspace)?);
    }
    Some(GoverningManifests { manifests })
}

/// A manifest, and the hash of its bytes.
///
/// `None` when the manifest cannot be read: a manifest nobody can read says
/// nothing, and a hash of nothing would be a hash of the absence of a
/// manifest.
fn manifest_with_hash(manifest: &Path) -> Option<ManifestFile> {
    let bytes = std::fs::read(manifest).ok()?;
    Some(ManifestFile {
        path: manifest.to_path_buf(),
        hash: sha256_raw(&bytes),
    })
}

/// The crates a package pulls from outside the project.
///
/// The names are the ones a path spells: a dependency declared as
/// `tree-sitter` is written `tree_sitter` in the code that uses it. That is
/// the name the manifest gives it, and it is the name a path spells unless
/// the dependency renames its library (`[lib] name = "…"`), which the
/// manifest of the caller does not say.
///
/// A manifest that cannot be read, or whose text is not a manifest, is not
/// an answer: it is the hole that says the judgment could not be made, and
/// it comes back as such instead of as a list of no crates — which is what a
/// manifest that declares nothing answers.
pub fn external_crates(manifest: &Path) -> Result<HashSet<String>, ManifestHole> {
    let text = match read_manifest(manifest) {
        Ok(text) => text,
        Err(error) => {
            return Err(ManifestHole::Unreadable(format!(
                "{}: {error}",
                manifest.display()
            )));
        }
    };
    let Ok(parsed) = text.parse::<toml::Value>() else {
        return Err(ManifestHole::Unparseable(format!(
            "{} is not a manifest",
            manifest.display()
        )));
    };
    let (workspace_manifest, workspace) = workspace_of(manifest);
    let root = workspace_manifest
        .as_ref()
        .and_then(|manifest| manifest.parent().map(Path::to_path_buf));
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
    Ok(names)
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
///
/// The first value is the manifest of the workspace, not its directory: the
/// manifest is the file whose bytes decide what the packages that inherit
/// from it pull.
fn workspace_of(manifest: &Path) -> (Option<PathBuf>, toml::Table) {
    let mut directory = manifest.parent();
    while let Some(current) = directory {
        let path = current.join("Cargo.toml");
        if path.is_file()
            && let Ok(text) = read_manifest(&path)
            && let Ok(parsed) = text.parse::<toml::Value>()
            && let Some(workspace) = parsed.get("workspace").and_then(toml::Value::as_table)
        {
            let dependencies = workspace
                .get("dependencies")
                .and_then(toml::Value::as_table)
                .cloned()
                .unwrap_or_default();
            return (Some(path), dependencies);
        }
        directory = current.parent();
    }
    (None, toml::Table::new())
}

/// The text of a manifest, when it can be read.
fn read_manifest(manifest: &Path) -> Result<String, std::io::Error> {
    std::fs::read_to_string(manifest)
}

#[cfg(test)]
mod tests {
    use super::{ManifestHole, Manifests, external_crates, is_governed_by_a_manifest, manifest_of};
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
        let crates = external_crates(&manifest).expect("the manifest can be read");

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
        let crates = external_crates(&manifest).expect("the manifest can be read");

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
        let crates = external_crates(&manifest).expect("the manifest can be read");

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
        let crates = external_crates(&manifest).expect("the manifest can be read");

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
        let crates = external_crates(&manifest).expect("the manifest can be read");

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
        let crates = external_crates(&manifest).expect("the manifest can be read");

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

    /// A manifest that cannot be judged is a hole, not a list of crates.
    ///
    /// The two answers look the same —no crate from outside— and they are
    /// not the same answer: one is what the manifest says, and the other is
    /// what nobody could know. The second one has to be counted, because the
    /// judgment of the crate-path rule does not run for that package.
    #[test]
    fn a_manifest_that_cannot_be_judged_is_a_hole() {
        let dir = tempfile::tempdir().expect("a temporary project");
        let package = dir.path().join("pkg");
        std::fs::create_dir_all(package.join("src")).expect("the source directory is created");
        let file = package.join("src/lib.rs");
        std::fs::write(&file, "pub fn run() {}\n").expect("the source is written");
        let manifest = package.join("Cargo.toml");
        std::fs::write(&manifest, "[package\nname = \"thing\"\n").expect("the manifest is written");

        let mut manifests = Manifests::default();
        let crates = manifests.external_crates_of(&package.join("src"));

        assert!(
            crates.is_empty(),
            "a manifest that could not be judged declares no crate from outside: {crates:?}"
        );
        let holes = manifests.holes();
        assert_eq!(holes.len(), 1, "the hole is counted: {holes:?}");
        assert!(
            holes[0].0.ends_with("pkg/Cargo.toml"),
            "the hole names the manifest: {holes:?}"
        );
        assert!(
            matches!(holes[0].1, ManifestHole::Unparseable(_)),
            "the manifest was read and was not a manifest: {holes:?}"
        );
    }

    /// A manifest that can be read is not a hole.
    #[test]
    fn a_manifest_that_can_be_read_is_not_a_hole() {
        let (_dir, file) =
            package("[package]\nname = \"thing\"\n\n[dependencies]\nserde_json = \"1\"\n");
        let directory = file
            .parent()
            .expect("the file is in a directory")
            .to_path_buf();
        let mut manifests = Manifests::default();

        manifests.external_crates_of(&directory);

        assert!(
            manifests.holes().is_empty(),
            "the manifest was read and there is no hole: {:?}",
            manifests.holes()
        );
    }
}
