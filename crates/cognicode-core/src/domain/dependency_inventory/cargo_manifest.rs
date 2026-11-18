//! A reader of `Cargo.toml`: the DECLARED layer of a Cargo workspace.
//!
//! Pure and offline: it reads the bytes of one manifest and nothing else. It
//! follows no `path`, runs no build script and expands no macro; whatever the
//! manifest leaves to resolution (the version a requirement picks, the
//! features a build turns on) is not in its output.

use std::collections::BTreeMap;

use super::declared::{DeclaredDependency, DeclaredOrigin, DependencyKind};

/// The `[package]` identity of a manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestPackage {
    /// The crate name.
    pub name: String,
    /// The version; `None` when it is inherited and the manifest that
    /// defines it was not given.
    pub version: Option<String>,
    /// Whether the version is `version.workspace = true`.
    pub version_inherited: bool,
}

/// The `[workspace]` table of a manifest.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WorkspaceTable {
    /// `members`, as written (globs included).
    pub members: Vec<String>,
    /// `exclude`, as written.
    pub exclude: Vec<String>,
    /// `[workspace.package] version`, which members may inherit.
    pub package_version: Option<String>,
    /// `[workspace.dependencies]`, as raw declarations.
    pub dependencies: BTreeMap<String, RawDependency>,
}

/// A declaration as written, before workspace inheritance and path
/// normalisation.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RawDependency {
    /// `package = "..."`.
    pub package: Option<String>,
    /// The version requirement.
    pub requirement: Option<String>,
    /// `path = "..."`, relative to the manifest that wrote it.
    pub path: Option<String>,
    /// `git = "..."`.
    pub git: Option<String>,
    /// `branch`, `tag` or `rev` as `branch:main`, `tag:v1`, `rev:abc`.
    pub git_reference: Option<String>,
    /// `registry = "..."`.
    pub registry: Option<String>,
    /// `features = [...]`.
    pub features: Vec<String>,
    /// `default-features`; the manifest default is `true`.
    pub default_features: Option<bool>,
    /// `optional = true`.
    pub optional: bool,
    /// `workspace = true`: inherit from `[workspace.dependencies]`.
    pub inherited: bool,
}

/// A raw declaration with the table it was declared in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestDependency {
    /// The key in the table.
    pub key: String,
    /// The table.
    pub kind: DependencyKind,
    /// `cfg(...)` or a target triple, for `[target.<...>.<kind>]`.
    pub target: Option<String>,
    /// The declaration.
    pub raw: RawDependency,
}

/// A parsed `Cargo.toml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CargoManifest {
    /// `[package]`, absent in a virtual workspace manifest.
    pub package: Option<ManifestPackage>,
    /// `[workspace]`, absent in a plain package manifest.
    pub workspace: Option<WorkspaceTable>,
    /// Every dependency declaration, target-specific ones included.
    pub dependencies: Vec<ManifestDependency>,
}

/// Why a manifest could not be read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ManifestParseError {
    /// The bytes are not UTF-8 TOML.
    #[error("Cargo.toml is not valid TOML: {0}")]
    Toml(String),
}

/// Each kind with the spellings Cargo accepts for its table.
const KINDS: [(DependencyKind, &[&str]); 3] = [
    (DependencyKind::Normal, &["dependencies"]),
    (
        DependencyKind::Development,
        &["dev-dependencies", "dev_dependencies"],
    ),
    (
        DependencyKind::Build,
        &["build-dependencies", "build_dependencies"],
    ),
];

fn strings(value: Option<&toml::Value>) -> Vec<String> {
    value
        .and_then(toml::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(toml::Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn raw_dependency(value: &toml::Value) -> RawDependency {
    if let Some(requirement) = value.as_str() {
        return RawDependency {
            requirement: Some(requirement.to_string()),
            ..RawDependency::default()
        };
    }
    let text = |key: &str| {
        value
            .get(key)
            .and_then(toml::Value::as_str)
            .map(str::to_string)
    };
    let flag = |key: &str| value.get(key).and_then(toml::Value::as_bool);
    let git_reference = ["branch", "tag", "rev"]
        .iter()
        .find_map(|key| text(key).map(|found| format!("{key}:{found}")));
    RawDependency {
        package: text("package"),
        requirement: text("version"),
        path: text("path"),
        git: text("git"),
        git_reference,
        registry: text("registry"),
        features: strings(value.get("features")),
        default_features: flag("default-features").or_else(|| flag("default_features")),
        optional: flag("optional").unwrap_or(false),
        inherited: flag("workspace").unwrap_or(false),
    }
}

fn collect_tables(table: &toml::Table, target: Option<&str>, out: &mut Vec<ManifestDependency>) {
    for (kind, names) in KINDS {
        for name in names {
            let Some(entries) = table.get(*name).and_then(toml::Value::as_table) else {
                continue;
            };
            for (key, value) in entries {
                out.push(ManifestDependency {
                    key: key.clone(),
                    kind,
                    target: target.map(str::to_string),
                    raw: raw_dependency(value),
                });
            }
        }
    }
}

fn workspace_table(workspace: &toml::Table) -> WorkspaceTable {
    let dependencies = workspace
        .get("dependencies")
        .and_then(toml::Value::as_table)
        .map(|entries| {
            entries
                .iter()
                .map(|(key, value)| (key.clone(), raw_dependency(value)))
                .collect()
        })
        .unwrap_or_default();
    WorkspaceTable {
        members: strings(workspace.get("members")),
        exclude: strings(workspace.get("exclude")),
        package_version: workspace
            .get("package")
            .and_then(|package| package.get("version"))
            .and_then(toml::Value::as_str)
            .map(str::to_string),
        dependencies,
    }
}

impl CargoManifest {
    /// Reads a manifest from its bytes.
    pub fn parse(bytes: &[u8]) -> Result<Self, ManifestParseError> {
        let text = std::str::from_utf8(bytes)
            .map_err(|error| ManifestParseError::Toml(error.to_string()))?;
        let document: toml::Table =
            toml::from_str(text).map_err(|error| ManifestParseError::Toml(error.to_string()))?;

        let package = document
            .get("package")
            .and_then(toml::Value::as_table)
            .and_then(|package| {
                let name = package.get("name")?.as_str()?.to_string();
                let version_entry = package.get("version");
                let version_inherited = version_entry
                    .and_then(|entry| entry.get("workspace"))
                    .and_then(toml::Value::as_bool)
                    .unwrap_or(false);
                Some(ManifestPackage {
                    name,
                    version: version_entry
                        .and_then(toml::Value::as_str)
                        .map(str::to_string),
                    version_inherited,
                })
            });
        let workspace = document
            .get("workspace")
            .and_then(toml::Value::as_table)
            .map(workspace_table);

        let mut dependencies = Vec::new();
        collect_tables(&document, None, &mut dependencies);
        if let Some(targets) = document.get("target").and_then(toml::Value::as_table) {
            for (target, tables) in targets {
                if let Some(tables) = tables.as_table() {
                    collect_tables(tables, Some(target), &mut dependencies);
                }
            }
        }
        Ok(Self {
            package,
            workspace,
            dependencies,
        })
    }

    /// The package version, with `version.workspace = true` resolved against
    /// `root` (the workspace root manifest) when it is given.
    pub fn package_version(&self, root: Option<&CargoManifest>) -> Option<String> {
        let package = self.package.as_ref()?;
        if package.version_inherited {
            return root
                .and_then(|root| root.workspace.as_ref())
                .and_then(|workspace| workspace.package_version.clone());
        }
        package.version.clone()
    }

    /// The declarations with workspace inheritance applied and path
    /// dependencies normalised to repository-relative directories.
    ///
    /// `directory` is the directory of this manifest and `root` the
    /// workspace root manifest with its own directory, both relative to the
    /// repository root. A `path` that leaves the repository becomes a
    /// `Path` origin with an empty directory, which no member matches.
    pub fn declared(
        &self,
        directory: &str,
        root: Option<(&CargoManifest, &str)>,
    ) -> Vec<DeclaredDependency> {
        self.dependencies
            .iter()
            .map(|entry| declare(entry, directory, root))
            .collect()
    }
}

fn declare(
    entry: &ManifestDependency,
    directory: &str,
    root: Option<(&CargoManifest, &str)>,
) -> DeclaredDependency {
    let local = &entry.raw;
    let inherited = if local.inherited {
        root.and_then(|(manifest, root_directory)| {
            let table = manifest.workspace.as_ref()?;
            table
                .dependencies
                .get(&entry.key)
                .map(|raw| (raw, root_directory))
        })
    } else {
        None
    };
    // Where `path` is relative to: the manifest that wrote it.
    let (base, path_directory) = match inherited {
        Some((raw, root_directory)) => (raw, root_directory),
        None => (local, directory),
    };
    let mut features = base.features.clone();
    if inherited.is_some() {
        features.extend(local.features.iter().cloned());
    }
    features.sort();
    features.dedup();
    let origin = if let Some(path) = &base.path {
        DeclaredOrigin::Path {
            path: join_normalised(path_directory, path).unwrap_or_default(),
        }
    } else if let Some(url) = &base.git {
        DeclaredOrigin::Git {
            url: url.clone(),
            reference: base.git_reference.clone(),
        }
    } else {
        DeclaredOrigin::Registry {
            registry: base.registry.clone(),
        }
    };
    DeclaredDependency {
        key: entry.key.clone(),
        package: base
            .package
            .clone()
            .or_else(|| local.package.clone())
            .unwrap_or_else(|| entry.key.clone()),
        kind: entry.kind,
        target: entry.target.clone(),
        requirement: base.requirement.clone(),
        origin,
        features,
        default_features: base.default_features.unwrap_or(true)
            && local.default_features.unwrap_or(true),
        optional: local.optional,
    }
}

/// `base/relative` with `.` and `..` resolved; `None` when the result would
/// leave the repository root.
pub fn join_normalised(base: &str, relative: &str) -> Option<String> {
    let mut parts: Vec<&str> = base
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".")
        .collect();
    for part in relative.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            other => parts.push(other),
        }
    }
    Some(parts.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORKSPACE_ROOT: &str = r#"
[workspace]
members = ["app", "crates/*"]
exclude = ["crates/skipped"]

[workspace.package]
version = "0.7.0"

[workspace.dependencies]
smallvec = { version = "1", features = ["union"] }
util = { path = "util" }
"#;

    const MEMBER: &str = r#"
[package]
name = "app"
version.workspace = true

[dependencies]
bitflags = "2"
old_bitflags = { package = "bitflags", version = "1" }
smallvec = { workspace = true, features = ["write"] }
util = { workspace = true }
gitdep = { git = "https://example.test/gitdep", rev = "abc1234" }
memchr = { version = "2", optional = true, default-features = false, features = ["std"] }

[dev-dependencies]
itoa = "1"

[build-dependencies]
autocfg = "1"

[target.'cfg(unix)'.dependencies]
libc = "0.2"

[target.'cfg(windows)'.build-dependencies]
winres = "0.1"
"#;

    fn find<'a>(items: &'a [DeclaredDependency], key: &str) -> &'a DeclaredDependency {
        items
            .iter()
            .find(|item| item.key == key)
            .unwrap_or_else(|| panic!("no declaration `{key}`"))
    }

    #[test]
    fn kinds_and_targets_follow_the_table_that_declares_them() {
        let manifest = CargoManifest::parse(MEMBER.as_bytes()).expect("a manifest");
        let declared = manifest.declared("app", None);
        assert_eq!(find(&declared, "bitflags").kind, DependencyKind::Normal);
        assert_eq!(find(&declared, "itoa").kind, DependencyKind::Development);
        assert_eq!(find(&declared, "autocfg").kind, DependencyKind::Build);
        let libc = find(&declared, "libc");
        assert_eq!(libc.kind, DependencyKind::Normal);
        assert_eq!(libc.target.as_deref(), Some("cfg(unix)"));
        let winres = find(&declared, "winres");
        assert_eq!(winres.kind, DependencyKind::Build);
        assert_eq!(winres.target.as_deref(), Some("cfg(windows)"));
    }

    #[test]
    fn a_renamed_dependency_resolves_to_its_package_name() {
        let manifest = CargoManifest::parse(MEMBER.as_bytes()).expect("a manifest");
        let declared = manifest.declared("app", None);
        let renamed = find(&declared, "old_bitflags");
        assert_eq!(renamed.package, "bitflags");
        assert_eq!(renamed.requirement.as_deref(), Some("1"));
    }

    #[test]
    fn features_optionality_and_git_references_are_kept() {
        let manifest = CargoManifest::parse(MEMBER.as_bytes()).expect("a manifest");
        let declared = manifest.declared("app", None);
        let memchr = find(&declared, "memchr");
        assert!(memchr.optional);
        assert!(!memchr.default_features);
        assert_eq!(memchr.features, vec!["std".to_string()]);
        let git = find(&declared, "gitdep");
        assert_eq!(
            git.origin,
            DeclaredOrigin::Git {
                url: "https://example.test/gitdep".to_string(),
                reference: Some("rev:abc1234".to_string()),
            }
        );
    }

    #[test]
    fn workspace_inheritance_merges_requirement_features_and_path() {
        let root = CargoManifest::parse(WORKSPACE_ROOT.as_bytes()).expect("a root manifest");
        let member = CargoManifest::parse(MEMBER.as_bytes()).expect("a manifest");
        let declared = member.declared("app", Some((&root, "")));
        let smallvec = find(&declared, "smallvec");
        assert_eq!(smallvec.requirement.as_deref(), Some("1"));
        assert_eq!(
            smallvec.features,
            vec!["union".to_string(), "write".to_string()]
        );
        // The inherited path is relative to the workspace root, not to `app`.
        assert_eq!(
            find(&declared, "util").origin,
            DeclaredOrigin::Path {
                path: "util".to_string()
            }
        );
        assert_eq!(
            member.package_version(Some(&root)).as_deref(),
            Some("0.7.0")
        );
        assert_eq!(member.package_version(None), None);
    }

    #[test]
    fn a_virtual_workspace_manifest_has_members_and_no_package() {
        let root = CargoManifest::parse(WORKSPACE_ROOT.as_bytes()).expect("a root manifest");
        assert!(root.package.is_none());
        let workspace = root.workspace.expect("a workspace table");
        assert_eq!(workspace.members, vec!["app", "crates/*"]);
        assert_eq!(workspace.exclude, vec!["crates/skipped"]);
    }

    #[test]
    fn path_dependencies_are_normalised_against_the_declaring_manifest() {
        let manifest = CargoManifest::parse(
            b"[package]\nname = \"a\"\nversion = \"1.0.0\"\n[dependencies]\nb = { path = \"../b\" }\nz = { path = \"../../../z\" }\n",
        )
        .expect("a manifest");
        let declared = manifest.declared("crates/a", None);
        assert_eq!(
            find(&declared, "b").origin,
            DeclaredOrigin::Path {
                path: "crates/b".to_string()
            }
        );
        assert_eq!(
            find(&declared, "z").origin,
            DeclaredOrigin::Path {
                path: String::new()
            },
            "a path that leaves the repository matches no member"
        );
    }

    #[test]
    fn a_broken_manifest_is_an_error() {
        assert!(matches!(
            CargoManifest::parse(b"[package\nname"),
            Err(ManifestParseError::Toml(_))
        ));
    }
}
