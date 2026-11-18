//! The offline Cargo reader against `cargo metadata --locked --offline`.
//!
//! The fixture workspace has development, build, target-specific, renamed,
//! workspace-inherited and path dependencies, and two versions of one crate.
//! The golden is the normalised output of `cargo metadata` on it, produced by
//! `sandbox/fixtures/dependency-inventory/cargo_metadata_golden.py` outside
//! the test suite: these tests never launch cargo.
//!
//! What is compared: the members, the packages with their versions and
//! sources, the edges of the resolve graph, the declarations of every member
//! (requirement, kind, target, features, rename) and the kinds and targets of
//! the edges that start in a member.
//!
//! What is not compared, and why: `cargo metadata` publishes no checksum, so
//! the lockfile checksums are asserted separately; it resolves features, which
//! a lockfile does not record; and a Git source cannot be compared natively
//! offline.

use cognicode_core::domain::dependency_inventory::{
    DeclaredUse, DependencyKind, LockFreshness, PackageSource, SourceFile, UnitInput,
    WorkspaceInventory, inventory_unit,
};
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::Path;

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../sandbox/fixtures/dependency-inventory"
);

fn read(path: &Path) -> Vec<u8> {
    std::fs::read(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

fn golden() -> Value {
    let path = Path::new(FIXTURE).join("cargo-workspace.metadata.golden.json");
    serde_json::from_slice(&read(&path)).expect("the golden is JSON")
}

fn inventory() -> WorkspaceInventory {
    let root = Path::new(FIXTURE).join("cargo-workspace");
    let file = |relative: &str| SourceFile {
        path: relative.to_string(),
        bytes: read(&root.join(relative)),
    };
    inventory_unit(&UnitInput {
        root: String::new(),
        lock: Some(file("Cargo.lock")),
        manifests: vec![
            file("Cargo.toml"),
            file("app/Cargo.toml"),
            file("util/Cargo.toml"),
        ],
    })
}

fn source_label(source: &PackageSource) -> String {
    source.lock_string().unwrap_or_else(|| "path".to_string())
}

fn kind_label(kind: DependencyKind) -> &'static str {
    match kind {
        DependencyKind::Normal => "normal",
        DependencyKind::Development => "dev",
        DependencyKind::Build => "build",
    }
}

/// `cargo metadata` writes every requirement as a comparator list, and a
/// dependency without a version requirement as `*`.
fn caret(requirement: Option<&str>) -> String {
    let Some(requirement) = requirement else {
        return "*".to_string();
    };
    if requirement.starts_with(|c: char| c.is_ascii_digit()) {
        format!("^{requirement}")
    } else {
        requirement.to_string()
    }
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .expect("an array")
        .iter()
        .map(|item| item.as_str().expect("a string").to_string())
        .collect()
}

#[test]
fn the_fixture_lockfile_agrees_with_its_manifests() {
    let found = inventory();

    assert_eq!(
        found.freshness,
        LockFreshness::Consistent,
        "{:?}",
        found.discrepancies
    );
    assert_eq!(found.lock_format_version, Some(4));
}

#[test]
fn the_members_are_the_workspace_members_of_cargo_metadata() {
    let found = inventory();

    let members: Vec<String> = found
        .members
        .iter()
        .map(|member| {
            format!(
                "{}@{}",
                member.name,
                member.version.as_deref().unwrap_or("?")
            )
        })
        .collect();
    assert_eq!(members, strings(&golden()["workspace_members"]));
}

#[test]
fn the_packages_have_the_versions_and_sources_cargo_metadata_resolves() {
    let found = inventory();
    let golden = golden();

    let expected: BTreeSet<(String, String)> = golden["packages"]
        .as_array()
        .expect("packages")
        .iter()
        .map(|package| {
            (
                package["id"].as_str().expect("id").to_string(),
                package["source"].as_str().expect("source").to_string(),
            )
        })
        .collect();
    let read: BTreeSet<(String, String)> = found
        .packages
        .iter()
        .map(|package| (package.id.clone(), source_label(&package.source)))
        .collect();
    assert_eq!(read, expected);
}

#[test]
fn the_edges_are_the_edges_of_the_cargo_metadata_resolve_graph() {
    let found = inventory();
    let golden = golden();

    let expected: BTreeSet<(String, String)> = golden["edges"]
        .as_array()
        .expect("edges")
        .iter()
        .map(|edge| {
            (
                edge["from"].as_str().expect("from").to_string(),
                edge["to"].as_str().expect("to").to_string(),
            )
        })
        .collect();
    let read: BTreeSet<(String, String)> = found
        .packages
        .iter()
        .flat_map(|package| {
            package
                .dependencies
                .iter()
                .map(|to| (package.id.clone(), to.clone()))
        })
        .collect();
    assert_eq!(read, expected);
}

#[test]
fn the_kind_and_target_of_each_member_edge_are_those_of_cargo_metadata() {
    let found = inventory();
    let golden = golden();
    let members: BTreeSet<&str> = found.members.iter().map(|m| m.name.as_str()).collect();

    let mut compared = 0;
    for edge in golden["edges"].as_array().expect("edges") {
        let from = edge["from"].as_str().expect("from");
        let member = from.split('@').next().expect("name");
        if !members.contains(member) {
            continue;
        }
        let expected: BTreeSet<(String, Option<String>)> = edge["kinds"]
            .as_array()
            .expect("kinds")
            .iter()
            .map(|kind| {
                (
                    kind["kind"].as_str().expect("kind").to_string(),
                    kind["target"].as_str().map(str::to_string),
                )
            })
            .collect();
        let target = found
            .packages
            .iter()
            .find(|package| package.id == edge["to"].as_str().expect("to"))
            .expect("the target is resolved");
        let read: BTreeSet<(String, Option<String>)> = target
            .declared_by
            .iter()
            .filter(|usage| usage.member == member)
            .map(|usage| {
                (
                    kind_label(usage.declaration.kind).to_string(),
                    usage.declaration.target.clone(),
                )
            })
            .collect();
        assert_eq!(read, expected, "{from} -> {}", target.id);
        compared += 1;
    }
    assert_eq!(
        compared, 9,
        "every edge that starts in a member was compared"
    );
}

#[test]
fn every_member_declaration_is_what_cargo_metadata_reports() {
    let found = inventory();
    let golden = golden();

    for package in golden["packages"].as_array().expect("packages") {
        let id = package["id"].as_str().expect("id");
        let member = id.split('@').next().expect("name");
        if !found.members.iter().any(|m| m.name == member) {
            continue;
        }
        let expected: BTreeSet<String> = package["declared"]
            .as_array()
            .expect("declared")
            .iter()
            .map(|declared| {
                format!(
                    "{} as {} {} {} {:?} {:?}",
                    declared["package"].as_str().expect("package"),
                    declared["rename"].as_str().unwrap_or("-"),
                    declared["requirement"].as_str().expect("requirement"),
                    declared["kind"].as_str().expect("kind"),
                    declared["target"].as_str(),
                    strings(&declared["features"]),
                )
            })
            .collect();
        let read: BTreeSet<String> = found
            .packages
            .iter()
            .flat_map(|candidate| &candidate.declared_by)
            .filter(|usage: &&DeclaredUse| usage.member == member)
            .map(|usage| {
                let declaration = &usage.declaration;
                let mut features = declaration.features.clone();
                features.sort();
                format!(
                    "{} as {} {} {} {:?} {:?}",
                    declaration.package,
                    if declaration.key == declaration.package {
                        "-"
                    } else {
                        &declaration.key
                    },
                    caret(declaration.requirement.as_deref()),
                    kind_label(declaration.kind),
                    declaration.target.as_deref(),
                    features,
                )
            })
            .collect();
        assert_eq!(read, expected, "declarations of {member}");
    }
}

#[test]
fn both_versions_of_a_crate_are_kept_apart() {
    let found = inventory();

    let versions: Vec<&str> = found
        .multiple_versions
        .iter()
        .flat_map(|crate_| crate_.versions.iter().map(String::as_str))
        .collect();
    assert_eq!(found.multiple_versions[0].name, "bitflags");
    assert_eq!(versions, vec!["1.3.2", "2.13.2"]);
}

#[test]
fn every_registry_package_carries_the_lockfile_checksum_and_a_package_url() {
    let found = inventory();

    for package in &found.packages {
        match package.source {
            PackageSource::Registry { .. } => {
                assert_eq!(
                    package.checksum.as_deref().map(str::len),
                    Some(64),
                    "{}",
                    package.id
                );
                assert_eq!(
                    package.purl.as_deref(),
                    Some(format!("pkg:cargo/{}@{}", package.name, package.version).as_str())
                );
            }
            _ => assert!(package.purl.is_none() && package.checksum.is_none()),
        }
    }
}
