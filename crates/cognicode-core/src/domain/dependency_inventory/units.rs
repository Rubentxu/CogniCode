//! Which manifests and lockfile belong together.
//!
//! A repository can hold several Cargo workspaces and standalone packages. A
//! unit is what one `Cargo.lock` describes: a workspace root with its
//! members, or a package that is its own root. Planning is a pure function of
//! the paths and bytes it is given; the caller decides which files to read.

use std::collections::{BTreeMap, BTreeSet};

use super::cargo_manifest::CargoManifest;
use super::declared::DeclaredOrigin;

/// A file of the revision: a path relative to the repository root and its
/// bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFile {
    /// The repository-relative path.
    pub path: String,
    /// The bytes.
    pub bytes: Vec<u8>,
}

/// The files of one workspace unit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitInput {
    /// The unit root directory, relative to the repository; empty for the
    /// repository root.
    pub root: String,
    /// The `Cargo.lock` at the unit root, if the revision tracks one.
    pub lock: Option<SourceFile>,
    /// The root manifest and the member manifests, sorted by path.
    pub manifests: Vec<SourceFile>,
}

/// The directory of a repository-relative path (empty for the root).
pub fn directory_of(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(directory, _)| directory)
}

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn under(root: &str, directory: &str) -> bool {
    root.is_empty() || directory == root || directory.starts_with(&format!("{root}/"))
}

fn relative<'a>(root: &str, directory: &'a str) -> &'a str {
    if root.is_empty() {
        directory
    } else {
        directory
            .strip_prefix(root)
            .map_or(directory, |rest| rest.trim_start_matches('/'))
    }
}

fn segment_matches(pattern: &[u8], text: &[u8]) -> bool {
    match (pattern.split_first(), text.split_first()) {
        (None, None) => true,
        (Some((b'*', rest)), _) => {
            segment_matches(rest, text)
                || text
                    .split_first()
                    .is_some_and(|(_, tail)| segment_matches(pattern, tail))
        }
        (Some((b'?', rest)), Some((_, tail))) => segment_matches(rest, tail),
        (Some((expected, rest)), Some((found, tail))) if expected == found => {
            segment_matches(rest, tail)
        }
        _ => false,
    }
}

fn path_matches(pattern: &[&str], path: &[&str]) -> bool {
    match pattern.split_first() {
        None => path.is_empty(),
        Some((&"**", rest)) => {
            path_matches(rest, path)
                || path
                    .split_first()
                    .is_some_and(|(_, tail)| path_matches(pattern, tail))
        }
        Some((segment, rest)) => path.split_first().is_some_and(|(head, tail)| {
            segment_matches(segment.as_bytes(), head.as_bytes()) && path_matches(rest, tail)
        }),
    }
}

/// Whether a workspace `members`/`exclude` glob selects `directory` (both
/// relative to the workspace root). Supports `*`, `?` and `**`.
fn glob_selects(pattern: &str, directory: &str) -> bool {
    let pattern: Vec<&str> = pattern
        .trim_start_matches("./")
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".")
        .collect();
    let path: Vec<&str> = directory
        .split('/')
        .filter(|part| !part.is_empty())
        .collect();
    path_matches(&pattern, &path)
}

struct Manifests {
    by_directory: BTreeMap<String, (SourceFile, Option<CargoManifest>)>,
}

impl Manifests {
    fn workspace_roots(&self) -> Vec<String> {
        let mut roots: Vec<String> = self
            .by_directory
            .iter()
            .filter(|(_, (_, manifest))| manifest.as_ref().is_some_and(|m| m.workspace.is_some()))
            .map(|(directory, _)| directory.clone())
            .collect();
        // Deepest first: a nested workspace claims its members before the
        // workspace that contains its directory can.
        roots.sort_by_key(|root| {
            std::cmp::Reverse(root.split('/').filter(|p| !p.is_empty()).count())
        });
        roots
    }

    fn members_of(&self, root: &str, claimed: &BTreeSet<String>) -> BTreeSet<String> {
        let Some(manifest) = self.by_directory.get(root).and_then(|(_, m)| m.as_ref()) else {
            return BTreeSet::from([root.to_string()]);
        };
        let table = manifest.workspace.clone().unwrap_or_default();
        let excluded = |directory: &str| {
            table
                .exclude
                .iter()
                .any(|pattern| glob_selects(pattern, relative(root, directory)))
        };
        let free = |directory: &str| {
            under(root, directory) && !claimed.contains(directory) && !excluded(directory)
        };
        let mut members = BTreeSet::from([root.to_string()]);
        for directory in self.by_directory.keys() {
            let selected = table
                .members
                .iter()
                .any(|pattern| glob_selects(pattern, relative(root, directory)));
            if selected && free(directory) {
                members.insert(directory.clone());
            }
        }
        // A path dependency inside the workspace directory is a member too.
        loop {
            let mut added = Vec::new();
            for member in &members {
                let Some((_, Some(member_manifest))) = self.by_directory.get(member) else {
                    continue;
                };
                for declaration in member_manifest.declared(member, Some((manifest, root))) {
                    let DeclaredOrigin::Path { path } = declaration.origin else {
                        continue;
                    };
                    if self.by_directory.contains_key(&path)
                        && !members.contains(&path)
                        && free(&path)
                    {
                        added.push(path);
                    }
                }
            }
            if added.is_empty() {
                break;
            }
            members.extend(added);
        }
        members
    }
}

/// Groups `Cargo.toml` and `Cargo.lock` files into workspace units.
///
/// Files with any other name are ignored. Every manifest lands in exactly
/// one unit, including a manifest that does not parse (the inventory
/// reports it as unreadable instead of dropping it); a lockfile with no
/// manifest beside it is a unit of its own.
pub fn plan_units(files: Vec<SourceFile>) -> Vec<UnitInput> {
    let mut manifests = Manifests {
        by_directory: BTreeMap::new(),
    };
    let mut locks: BTreeMap<String, SourceFile> = BTreeMap::new();
    for file in files {
        let directory = directory_of(&file.path).to_string();
        match file_name(&file.path) {
            "Cargo.toml" => {
                let parsed = CargoManifest::parse(&file.bytes).ok();
                manifests.by_directory.insert(directory, (file, parsed));
            }
            "Cargo.lock" => {
                locks.insert(directory, file);
            }
            _ => {}
        }
    }

    let mut claimed: BTreeSet<String> = BTreeSet::new();
    let mut groups: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for root in manifests.workspace_roots() {
        if claimed.contains(&root) {
            continue;
        }
        let members = manifests.members_of(&root, &claimed);
        claimed.extend(members.iter().cloned());
        groups.insert(root, members);
    }
    for directory in manifests.by_directory.keys() {
        if !claimed.contains(directory) {
            groups.insert(directory.clone(), BTreeSet::from([directory.clone()]));
        }
    }
    for directory in locks.keys() {
        groups.entry(directory.clone()).or_default();
    }

    groups
        .into_iter()
        .map(|(root, members)| UnitInput {
            lock: locks.get(&root).cloned(),
            manifests: members
                .iter()
                .filter_map(|member| manifests.by_directory.get(member))
                .map(|(file, _)| file.clone())
                .collect(),
            root,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str, text: &str) -> SourceFile {
        SourceFile {
            path: path.to_string(),
            bytes: text.as_bytes().to_vec(),
        }
    }

    fn package(name: &str) -> String {
        format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\n")
    }

    fn roots(units: &[UnitInput]) -> Vec<&str> {
        units.iter().map(|unit| unit.root.as_str()).collect()
    }

    fn manifest_paths(unit: &UnitInput) -> Vec<&str> {
        unit.manifests
            .iter()
            .map(|file| file.path.as_str())
            .collect()
    }

    #[test]
    fn a_workspace_groups_its_members_under_the_root_lockfile() {
        let units = plan_units(vec![
            file(
                "Cargo.toml",
                "[workspace]\nmembers = [\"crates/*\"]\nexclude = [\"crates/skipped\"]\n",
            ),
            file("Cargo.lock", "version = 4\n"),
            file("crates/a/Cargo.toml", &package("a")),
            file("crates/b/Cargo.toml", &package("b")),
            file("crates/skipped/Cargo.toml", &package("skipped")),
        ]);
        assert_eq!(roots(&units), vec!["", "crates/skipped"]);
        assert_eq!(
            manifest_paths(&units[0]),
            vec!["Cargo.toml", "crates/a/Cargo.toml", "crates/b/Cargo.toml"]
        );
        assert!(units[0].lock.is_some());
        assert!(units[1].lock.is_none());
    }

    #[test]
    fn a_path_dependency_inside_the_workspace_directory_is_a_member() {
        let units = plan_units(vec![
            file("Cargo.toml", "[workspace]\nmembers = [\"app\"]\n"),
            file(
                "app/Cargo.toml",
                "[package]\nname = \"app\"\nversion = \"0.1.0\"\n[dependencies]\nhelper = { path = \"../tools/helper\" }\n",
            ),
            file("tools/helper/Cargo.toml", &package("helper")),
        ]);
        assert_eq!(roots(&units), vec![""]);
        assert_eq!(manifest_paths(&units[0]).len(), 3);
    }

    #[test]
    fn standalone_packages_and_nested_workspaces_are_their_own_units() {
        let units = plan_units(vec![
            file("Cargo.toml", "[workspace]\nmembers = [\"a\"]\n"),
            file("a/Cargo.toml", &package("a")),
            file("fixtures/lone/Cargo.toml", &package("lone")),
            file("fixtures/lone/Cargo.lock", "version = 4\n"),
            file("fixtures/ws/Cargo.toml", "[workspace]\nmembers = [\"m\"]\n"),
            file("fixtures/ws/m/Cargo.toml", &package("m")),
        ]);
        assert_eq!(roots(&units), vec!["", "fixtures/lone", "fixtures/ws"]);
        assert_eq!(
            manifest_paths(&units[2]),
            vec!["fixtures/ws/Cargo.toml", "fixtures/ws/m/Cargo.toml"]
        );
        assert!(units[1].lock.is_some());
    }

    #[test]
    fn a_lockfile_without_a_manifest_is_a_unit_and_a_broken_manifest_is_kept() {
        let units = plan_units(vec![
            file("orphan/Cargo.lock", "version = 4\n"),
            file("broken/Cargo.toml", "[package\n"),
        ]);
        assert_eq!(roots(&units), vec!["broken", "orphan"]);
        assert_eq!(manifest_paths(&units[0]), vec!["broken/Cargo.toml"]);
        assert!(units[1].manifests.is_empty());
        assert!(units[1].lock.is_some());
    }

    #[test]
    fn member_globs_support_star_question_mark_and_double_star() {
        assert!(glob_selects("crates/*", "crates/a"));
        assert!(!glob_selects("crates/*", "crates/a/b"));
        assert!(glob_selects("crates/**", "crates/a/b"));
        assert!(glob_selects("crate-?", "crate-1"));
        assert!(glob_selects("./app", "app"));
        assert!(!glob_selects("app", "application"));
    }

    #[test]
    fn files_that_are_not_cargo_files_are_ignored() {
        assert!(plan_units(vec![file("src/lib.rs", "fn a() {}")]).is_empty());
    }
}
