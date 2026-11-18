use super::*;
use crate::domain::dependency_inventory::units::{SourceFile, UnitInput};

const REGISTRY: &str = "registry+https://github.com/rust-lang/crates.io-index";
const SUM: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";

fn file(path: &str, text: &str) -> SourceFile {
    SourceFile {
        path: path.into(),
        bytes: text.as_bytes().to_vec(),
    }
}

fn unit(manifest: &str, lock: Option<&str>) -> UnitInput {
    UnitInput {
        root: String::new(),
        lock: lock.map(|text| file("Cargo.lock", text)),
        manifests: vec![file("Cargo.toml", manifest)],
    }
}

fn manifest(dependencies: &str) -> String {
    format!("[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n{dependencies}")
}

fn registry_package(name: &str, version: &str, dependencies: &[&str]) -> String {
    let edges = if dependencies.is_empty() {
        String::new()
    } else {
        let list: Vec<String> = dependencies.iter().map(|d| format!("\"{d}\"")).collect();
        format!("dependencies = [{}]\n", list.join(", "))
    };
    format!(
        "\n[[package]]\nname = \"{name}\"\nversion = \"{version}\"\nsource = \"{REGISTRY}\"\nchecksum = \"{SUM}\"\n{edges}"
    )
}

fn app_package(dependencies: &[&str]) -> String {
    let list: Vec<String> = dependencies.iter().map(|d| format!("\"{d}\"")).collect();
    format!(
        "\n[[package]]\nname = \"app\"\nversion = \"0.1.0\"\ndependencies = [{}]\n",
        list.join(", ")
    )
}

fn lock(packages: &[String]) -> String {
    format!("version = 3\n{}", packages.concat())
}

fn kinds(found: &WorkspaceInventory) -> Vec<DiscrepancyKind> {
    found.discrepancies.iter().map(|d| d.kind).collect()
}

fn package<'a>(found: &'a WorkspaceInventory, id: &str) -> &'a ResolvedPackage {
    found
        .packages
        .iter()
        .find(|package| package.id == id)
        .unwrap_or_else(|| panic!("no package {id}"))
}

fn consistent_unit() -> WorkspaceInventory {
    inventory_unit(&unit(
        &manifest("[dependencies]\nserde = \"1\"\n"),
        Some(&lock(&[
            app_package(&["serde"]),
            registry_package("serde", "1.0.5", &[]),
        ])),
    ))
}

#[test]
fn a_lockfile_that_agrees_is_consistent_and_anchored_to_its_bytes() {
    let found = consistent_unit();

    assert_eq!(found.freshness, LockFreshness::Consistent);
    assert!(found.discrepancies.is_empty());
    let anchor = found.lockfile.expect("anchored");
    assert_eq!(anchor.path, "Cargo.lock");
    assert_eq!(anchor.digest.len(), 64);
    assert_eq!(found.lock_format_version, Some(3));
    assert_eq!(found.root, ".");
}

#[test]
fn a_resolved_package_carries_identity_integrity_and_who_declared_it() {
    let found = consistent_unit();

    let serde = package(&found, "serde@1.0.5");
    assert_eq!(serde.purl.as_deref(), Some("pkg:cargo/serde@1.0.5"));
    assert_eq!(serde.integrity, Integrity::Checksummed);
    assert_eq!(serde.checksum.as_deref(), Some(SUM));
    assert_eq!(serde.declared_by[0].member, "app");
    assert!(serde.direct_kinds.contains(&DependencyKind::Normal));
    let app = package(&found, "app@0.1.0");
    assert!(app.workspace_member);
    assert_eq!(app.purl, None, "a path package has no Package URL");
    assert_eq!(app.integrity, Integrity::Unpinned);
}

#[test]
fn a_declaration_the_lockfile_does_not_list_makes_it_stale() {
    let found = inventory_unit(&unit(
        &manifest("[dependencies]\nserde = \"1\"\nitoa = \"1\"\n"),
        Some(&lock(&[
            app_package(&["serde"]),
            registry_package("serde", "1.0.5", &[]),
        ])),
    ));

    assert_eq!(found.freshness, LockFreshness::Stale);
    assert_eq!(kinds(&found), vec![DiscrepancyKind::DeclaredNotLocked]);
    assert_eq!(found.unresolved_declarations[0].declaration.package, "itoa");
}

#[test]
fn a_locked_version_outside_the_requirement_makes_it_stale() {
    let found = inventory_unit(&unit(
        &manifest("[dependencies]\nserde = \"2\"\n"),
        Some(&lock(&[
            app_package(&["serde"]),
            registry_package("serde", "1.0.5", &[]),
        ])),
    ));

    assert_eq!(found.freshness, LockFreshness::Stale);
    assert!(kinds(&found).contains(&DiscrepancyKind::RequirementUnsatisfied));
}

#[test]
fn a_member_with_no_package_in_the_lockfile_makes_it_stale() {
    let found = inventory_unit(&unit(
        &manifest("[dependencies]\nserde = \"1\"\n"),
        Some(&lock(&[registry_package("serde", "1.0.5", &[])])),
    ));

    assert_eq!(found.freshness, LockFreshness::Stale);
    assert!(kinds(&found).contains(&DiscrepancyKind::MemberNotLocked));
    assert_eq!(found.unresolved_declarations.len(), 1);
}

#[test]
fn a_member_version_that_differs_from_the_lock_makes_it_stale() {
    let bumped = "[package]\nname = \"app\"\nversion = \"0.2.0\"\n";
    let found = inventory_unit(&unit(bumped, Some(&lock(&[app_package(&[])]))));

    assert_eq!(found.freshness, LockFreshness::Stale);
    assert!(kinds(&found).contains(&DiscrepancyKind::MemberVersionMismatch));
}

#[test]
fn a_lock_edge_no_declaration_explains_is_reported() {
    let found = inventory_unit(&unit(
        &manifest(""),
        Some(&lock(&[
            app_package(&["serde"]),
            registry_package("serde", "1.0.5", &[]),
        ])),
    ));

    assert_eq!(found.freshness, LockFreshness::Stale);
    assert!(kinds(&found).contains(&DiscrepancyKind::LockedNotDeclared));
}

#[test]
fn a_locked_package_no_member_reaches_is_an_orphan() {
    let found = inventory_unit(&unit(
        &manifest(""),
        Some(&lock(&[
            app_package(&[]),
            registry_package("serde", "1.0.5", &[]),
        ])),
    ));

    assert_eq!(found.freshness, LockFreshness::Stale);
    let orphans: Vec<&str> = found
        .discrepancies
        .iter()
        .filter(|d| d.kind == DiscrepancyKind::OrphanLockedPackage)
        .map(|d| d.subject.as_str())
        .collect();
    assert_eq!(orphans, vec!["serde@1.0.5"]);
}

#[test]
fn an_edge_to_a_package_the_lockfile_lacks_is_dangling() {
    let found = inventory_unit(&unit(
        &manifest("[dependencies]\nserde = \"1\"\n"),
        Some(&lock(&[
            app_package(&["serde"]),
            registry_package("serde", "1.0.5", &["ghost"]),
        ])),
    ));

    assert!(kinds(&found).contains(&DiscrepancyKind::DanglingLockEdge));
    assert_eq!(found.freshness, LockFreshness::Stale);
}

#[test]
fn manifests_without_a_lockfile_resolve_nothing_and_say_so() {
    let found = inventory_unit(&unit(&manifest("[dependencies]\nserde = \"1\"\n"), None));

    assert_eq!(found.freshness, LockFreshness::NoLockfile);
    assert!(found.packages.is_empty());
    assert!(found.lockfile.is_none());
    assert_eq!(found.unresolved_declarations.len(), 1);
    let resolved = found
        .layers
        .iter()
        .find(|layer| layer.layer == DependencyLayer::Resolved)
        .expect("resolved layer");
    assert_eq!(resolved.status, CoverageStatus::NotCollected);
}

#[test]
fn a_lockfile_without_a_manifest_is_reported_as_such() {
    let found = inventory_unit(&UnitInput {
        root: String::new(),
        lock: Some(file(
            "Cargo.lock",
            &lock(&[registry_package("serde", "1.0.5", &[])]),
        )),
        manifests: Vec::new(),
    });

    assert_eq!(found.freshness, LockFreshness::NoManifest);
    assert_eq!(found.packages.len(), 1);
}

#[test]
fn an_unreadable_lockfile_is_partial_and_never_a_missing_one() {
    let found = inventory_unit(&unit(
        &manifest("[dependencies]\nserde = \"1\"\n"),
        Some("this is = not [ toml"),
    ));

    assert_eq!(found.freshness, LockFreshness::Partial);
    assert_eq!(kinds(&found), vec![DiscrepancyKind::UnreadableFile]);
    assert!(found.lockfile.is_some(), "the bytes are still anchored");
}

#[test]
fn a_git_package_is_pinned_by_its_commit_and_identified_by_its_repository() {
    let git = format!(
        "\n[[package]]\nname = \"fork\"\nversion = \"0.4.0\"\nsource = \"git+https://example.invalid/fork.git?branch=main#{COMMIT}\"\n"
    );
    let found = inventory_unit(&unit(
        &manifest(
            "[dependencies]\nfork = { git = \"https://example.invalid/fork.git\", branch = \"main\" }\n",
        ),
        Some(&lock(&[app_package(&["fork"]), git])),
    ));

    assert_eq!(
        found.freshness,
        LockFreshness::Consistent,
        "{:?}",
        found.discrepancies
    );
    let fork = package(&found, "fork@0.4.0");
    assert_eq!(fork.integrity, Integrity::PinnedCommit);
    assert_eq!(fork.checksum, None);
    let purl = fork.purl.as_deref().expect("a git package has a purl");
    assert!(purl.starts_with("pkg:cargo/fork@0.4.0?vcs_url="), "{purl}");
    assert!(purl.contains(COMMIT), "{purl}");
}

#[test]
fn a_path_dependency_resolves_to_a_package_without_identity() {
    let found = inventory_unit(&unit(
        &manifest("[dependencies]\nlocal = { path = \"local\" }\n"),
        Some(&lock(&[
            app_package(&["local"]),
            "\n[[package]]\nname = \"local\"\nversion = \"0.0.1\"\n".to_string(),
        ])),
    ));

    assert_eq!(
        found.freshness,
        LockFreshness::Consistent,
        "{:?}",
        found.discrepancies
    );
    assert_eq!(package(&found, "local@0.0.1").purl, None);
}

#[test]
fn two_versions_of_one_crate_are_both_kept_and_listed() {
    let found = inventory_unit(&unit(
        &manifest(
            "[dependencies]\nbitflags = \"2\"\nold = { package = \"bitflags\", version = \"1\" }\n",
        ),
        Some(&lock(&[
            app_package(&["bitflags 1.3.2", "bitflags 2.1.0"]),
            registry_package("bitflags", "1.3.2", &[]),
            registry_package("bitflags", "2.1.0", &[]),
        ])),
    ));

    assert_eq!(
        found.freshness,
        LockFreshness::Consistent,
        "{:?}",
        found.discrepancies
    );
    assert_eq!(found.multiple_versions.len(), 1);
    assert_eq!(found.multiple_versions[0].versions, vec!["1.3.2", "2.1.0"]);
    assert_eq!(
        package(&found, "bitflags@1.3.2").declared_by[0]
            .declaration
            .key,
        "old"
    );
}

#[test]
fn the_kind_a_dependency_is_reached_through_follows_the_member_edge() {
    let found = inventory_unit(&unit(
        &manifest(
            "[dependencies]\nserde = \"1\"\n\n[dev-dependencies]\nitoa = \"1\"\n\n[build-dependencies]\ncc = \"1\"\n",
        ),
        Some(&lock(&[
            app_package(&["cc", "itoa", "serde"]),
            registry_package("cc", "1.0.1", &["shlex"]),
            registry_package("itoa", "1.0.1", &[]),
            registry_package("serde", "1.0.5", &[]),
            registry_package("shlex", "1.0.0", &[]),
        ])),
    ));

    assert_eq!(
        found.freshness,
        LockFreshness::Consistent,
        "{:?}",
        found.discrepancies
    );
    let via = |id: &str| {
        package(&found, id)
            .reachable_via
            .iter()
            .copied()
            .collect::<Vec<_>>()
    };
    assert_eq!(via("serde@1.0.5"), vec![DependencyKind::Normal]);
    assert_eq!(via("itoa@1.0.1"), vec![DependencyKind::Development]);
    assert_eq!(via("shlex@1.0.0"), vec![DependencyKind::Build]);
}

#[test]
fn the_resolved_digest_depends_on_the_graph_and_not_on_formatting() {
    let a = consistent_unit();
    let reformatted = inventory_unit(&unit(
        &manifest("[dependencies]\nserde = \"1\"\n"),
        Some(&format!(
            "# a comment\n{}",
            lock(&[
                app_package(&["serde"]),
                registry_package("serde", "1.0.5", &[])
            ])
        )),
    ));
    let bumped = inventory_unit(&unit(
        &manifest("[dependencies]\nserde = \"1\"\n"),
        Some(&lock(&[
            app_package(&["serde"]),
            registry_package("serde", "1.0.6", &[]),
        ])),
    ));

    assert_eq!(a.resolved_digest, reformatted.resolved_digest);
    assert_ne!(a.lockfile, reformatted.lockfile, "the lock bytes differ");
    assert_ne!(a.resolved_digest, bumped.resolved_digest);
}

#[test]
fn the_observed_and_distributed_layers_are_never_claimed() {
    let found = consistent_unit();

    for layer in [DependencyLayer::Observed, DependencyLayer::Distributed] {
        let row = found
            .layers
            .iter()
            .find(|row| row.layer == layer)
            .expect("layer");
        assert_eq!(row.status, CoverageStatus::NotCollected);
        assert!(!row.note.is_empty());
    }
}
