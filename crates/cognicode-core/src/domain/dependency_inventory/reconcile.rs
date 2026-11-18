//! Reconciles the declared layer (manifests) with the resolved layer
//! (lockfile) of one workspace unit.
//!
//! The result is a pure function of the bytes of the unit's files. It never
//! asks Cargo anything: a disagreement between a manifest and the lockfile is
//! reported as a [`Discrepancy`], and the freshness verdict is only
//! `Consistent` when every check could actually be made.

use std::collections::{BTreeMap, BTreeSet};

use super::cargo_lock::{CargoLock, LockDependencyRef, LockedPackage};
use super::cargo_manifest::CargoManifest;
use super::declared::{DeclaredDependency, DeclaredOrigin, DependencyKind};
use super::model::{
    CoverageStatus, DeclaredUse, DependencyLayer, Discrepancy, DiscrepancyKind, FileAnchor,
    Integrity, LayerStatus, LockFreshness, MemberSummary, MultipleVersions, ResolvedPackage,
    WorkspaceInventory,
};
use super::purl::cargo_purl;
use super::source::PackageSource;
use super::units::{SourceFile, UnitInput, directory_of};
use super::version_req::{RequirementMatch, requirement_matches};
use crate::domain::scm::ContentDigest;

/// A workspace member with the declarations it makes.
struct Member {
    name: String,
    version: Option<String>,
    manifest: String,
    declarations: Vec<DeclaredDependency>,
}

/// What a declaration resolved to in the lockfile.
enum Resolution {
    Targets(Vec<usize>),
    NotLocked,
    Unsatisfied(Vec<String>),
    Unverifiable(Vec<usize>),
}

fn anchor(file: &SourceFile) -> FileAnchor {
    FileAnchor {
        path: file.path.clone(),
        digest: ContentDigest::from_bytes(&file.bytes).as_hex().to_string(),
    }
}

fn display_root(root: &str) -> String {
    if root.is_empty() {
        ".".into()
    } else {
        root.into()
    }
}

fn discrepancy(kind: DiscrepancyKind, subject: &str, detail: String) -> Discrepancy {
    Discrepancy {
        kind,
        subject: subject.to_string(),
        detail,
    }
}

/// Parsed inputs of a unit.
struct Declared {
    members: Vec<Member>,
    anchors: Vec<FileAnchor>,
    problems: Vec<Discrepancy>,
}

fn read_manifests(unit: &UnitInput) -> Declared {
    let mut parsed: Vec<(&SourceFile, CargoManifest)> = Vec::new();
    let mut problems = Vec::new();
    for file in &unit.manifests {
        match CargoManifest::parse(&file.bytes) {
            Ok(manifest) => parsed.push((file, manifest)),
            Err(error) => problems.push(discrepancy(
                DiscrepancyKind::UnreadableFile,
                &file.path,
                error.to_string(),
            )),
        }
    }
    let root = parsed
        .iter()
        .find(|(file, manifest)| {
            directory_of(&file.path) == unit.root && manifest.workspace.is_some()
        })
        .map(|(_, manifest)| manifest.clone());
    let mut members = Vec::new();
    for (file, manifest) in &parsed {
        let Some(package) = &manifest.package else {
            continue;
        };
        let directory = directory_of(&file.path);
        members.push(Member {
            name: package.name.clone(),
            version: manifest.package_version(root.as_ref()),
            manifest: file.path.clone(),
            declarations: manifest.declared(
                directory,
                root.as_ref().map(|root| (root, unit.root.as_str())),
            ),
        });
    }
    members.sort_by(|a, b| a.manifest.cmp(&b.manifest));
    Declared {
        members,
        anchors: unit.manifests.iter().map(anchor).collect(),
        problems,
    }
}

/// Stable ids for the locked packages: `name@version`, disambiguated by
/// source only for the packages that would otherwise collide.
fn package_ids(packages: &[LockedPackage]) -> Vec<String> {
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    for package in packages {
        *seen
            .entry(format!("{}@{}", package.name, package.version))
            .or_default() += 1;
    }
    let mut counters: BTreeMap<String, usize> = BTreeMap::new();
    packages
        .iter()
        .map(|package| {
            let base = format!("{}@{}", package.name, package.version);
            if seen[&base] == 1 {
                return base;
            }
            let source = package
                .source
                .lock_string()
                .unwrap_or_else(|| "path".to_string());
            let candidate = format!("{base}#{source}");
            let counter = counters.entry(candidate.clone()).or_default();
            *counter += 1;
            if *counter == 1 {
                candidate
            } else {
                format!("{candidate}#{counter}")
            }
        })
        .collect()
}

/// The packages a lock edge points at.
fn resolve_reference(packages: &[LockedPackage], reference: &LockDependencyRef) -> Vec<usize> {
    packages
        .iter()
        .enumerate()
        .filter(|(_, package)| {
            package.name == reference.name
                && reference
                    .version
                    .as_ref()
                    .is_none_or(|version| *version == package.version)
                && reference
                    .source
                    .as_ref()
                    .is_none_or(|source| package.source.lock_string().as_deref() == Some(source))
        })
        .map(|(index, _)| index)
        .collect()
}

fn origin_matches(origin: &DeclaredOrigin, source: &PackageSource) -> bool {
    matches!(
        (origin, source),
        (DeclaredOrigin::Path { .. }, PackageSource::Path)
            | (DeclaredOrigin::Git { .. }, PackageSource::Git { .. })
            | (
                DeclaredOrigin::Registry { .. },
                PackageSource::Registry { .. }
            )
    )
}

fn resolve_declaration(
    declaration: &DeclaredDependency,
    candidates: &[usize],
    packages: &[LockedPackage],
) -> Resolution {
    let same_origin: Vec<usize> = candidates
        .iter()
        .copied()
        .filter(|&index| {
            packages[index].name == declaration.package
                && origin_matches(&declaration.origin, &packages[index].source)
        })
        .collect();
    if same_origin.is_empty() {
        return Resolution::NotLocked;
    }
    let (Some(requirement), DeclaredOrigin::Registry { .. }) =
        (&declaration.requirement, &declaration.origin)
    else {
        return Resolution::Targets(same_origin);
    };
    let verdict = |index: usize| requirement_matches(requirement, &packages[index].version);
    let satisfied: Vec<usize> = same_origin
        .iter()
        .copied()
        .filter(|&index| verdict(index) == RequirementMatch::Satisfied)
        .collect();
    if !satisfied.is_empty() {
        return Resolution::Targets(satisfied);
    }
    if same_origin
        .iter()
        .any(|&index| verdict(index) == RequirementMatch::Unverifiable)
    {
        return Resolution::Unverifiable(same_origin);
    }
    Resolution::Unsatisfied(
        same_origin
            .iter()
            .map(|&index| packages[index].version.clone())
            .collect(),
    )
}

fn describe(declaration: &DeclaredDependency) -> String {
    let target = declaration
        .target
        .as_ref()
        .map(|target| format!(" for {target}"))
        .unwrap_or_default();
    format!(
        "`{}` ({}{target})",
        declaration.key,
        declaration.kind.table()
    )
}

fn integrity(package: &LockedPackage) -> Integrity {
    match (&package.checksum, &package.source) {
        (Some(_), _) => Integrity::Checksummed,
        (None, PackageSource::Git { .. }) => Integrity::PinnedCommit,
        (None, _) => Integrity::Unpinned,
    }
}

/// The lock entry that stands for a member: a path package of that name.
fn member_entry(member: &Member, packages: &[LockedPackage]) -> Option<usize> {
    let named: Vec<usize> = packages
        .iter()
        .enumerate()
        .filter(|(_, package)| package.name == member.name && package.source == PackageSource::Path)
        .map(|(index, _)| index)
        .collect();
    let by_version = named.iter().copied().find(|&index| {
        member
            .version
            .as_ref()
            .is_some_and(|version| *version == packages[index].version)
    });
    by_version.or_else(|| named.first().copied())
}

/// Everything resolved about a unit that has a lockfile.
struct Resolved {
    packages: Vec<ResolvedPackage>,
    unresolved: Vec<DeclaredUse>,
    discrepancies: Vec<Discrepancy>,
}

fn resolve_unit(lock: &CargoLock, members: &[Member]) -> Resolved {
    let ids = package_ids(&lock.packages);
    let edges: Vec<Vec<usize>> = lock
        .packages
        .iter()
        .map(|package| {
            package
                .dependencies
                .iter()
                .flat_map(|reference| resolve_reference(&lock.packages, reference))
                .collect()
        })
        .collect();
    let mut discrepancies = Vec::new();
    for (index, package) in lock.packages.iter().enumerate() {
        for reference in &package.dependencies {
            if resolve_reference(&lock.packages, reference).is_empty() {
                discrepancies.push(discrepancy(
                    DiscrepancyKind::DanglingLockEdge,
                    &ids[index],
                    format!(
                        "depends on `{}` which the lockfile does not contain",
                        reference.name
                    ),
                ));
            }
        }
    }

    let mut declared_by: Vec<Vec<DeclaredUse>> = vec![Vec::new(); lock.packages.len()];
    let mut direct_kinds: Vec<BTreeSet<DependencyKind>> =
        vec![BTreeSet::new(); lock.packages.len()];
    let mut member_edges: BTreeMap<usize, Vec<(DependencyKind, usize)>> = BTreeMap::new();
    let mut member_indexes: BTreeSet<usize> = BTreeSet::new();
    let mut unresolved = Vec::new();

    for member in members {
        let uses = |declaration: &DeclaredDependency| DeclaredUse {
            member: member.name.clone(),
            manifest: member.manifest.clone(),
            declaration: declaration.clone(),
        };
        let Some(entry) = member_entry(member, &lock.packages) else {
            discrepancies.push(discrepancy(
                DiscrepancyKind::MemberNotLocked,
                &member.name,
                format!("`{}` has no package in the lockfile", member.manifest),
            ));
            unresolved.extend(member.declarations.iter().map(uses));
            continue;
        };
        member_indexes.insert(entry);
        if let Some(declared) = &member.version
            && *declared != lock.packages[entry].version
        {
            discrepancies.push(discrepancy(
                DiscrepancyKind::MemberVersionMismatch,
                &member.name,
                format!(
                    "the manifest says {declared}, the lockfile says {}",
                    lock.packages[entry].version
                ),
            ));
        }
        let candidates = &edges[entry];
        let mut explained: BTreeSet<usize> = BTreeSet::new();
        for declaration in &member.declarations {
            let (targets, unverifiable) =
                match resolve_declaration(declaration, candidates, &lock.packages) {
                    Resolution::Targets(targets) => (targets, false),
                    Resolution::Unverifiable(targets) => (targets, true),
                    Resolution::NotLocked => {
                        discrepancies.push(discrepancy(
                            DiscrepancyKind::DeclaredNotLocked,
                            &member.name,
                            format!(
                                "declares {} and the lockfile does not list it",
                                describe(declaration)
                            ),
                        ));
                        unresolved.push(uses(declaration));
                        continue;
                    }
                    Resolution::Unsatisfied(locked) => {
                        discrepancies.push(discrepancy(
                            DiscrepancyKind::RequirementUnsatisfied,
                            &member.name,
                            format!(
                                "{} requires `{}` and the lockfile has {}",
                                describe(declaration),
                                declaration.requirement.as_deref().unwrap_or_default(),
                                locked.join(", ")
                            ),
                        ));
                        unresolved.push(uses(declaration));
                        continue;
                    }
                };
            if unverifiable {
                discrepancies.push(discrepancy(
                    DiscrepancyKind::RequirementUnverifiable,
                    &member.name,
                    format!(
                        "the requirement `{}` of {} is not in a grammar this reader covers",
                        declaration.requirement.as_deref().unwrap_or_default(),
                        describe(declaration)
                    ),
                ));
            }
            for target in targets {
                explained.insert(target);
                declared_by[target].push(uses(declaration));
                direct_kinds[target].insert(declaration.kind);
                member_edges
                    .entry(entry)
                    .or_default()
                    .push((declaration.kind, target));
            }
        }
        for &target in candidates {
            let named = member
                .declarations
                .iter()
                .any(|declaration| declaration.package == lock.packages[target].name);
            if !named && !explained.contains(&target) {
                discrepancies.push(discrepancy(
                    DiscrepancyKind::LockedNotDeclared,
                    &member.name,
                    format!(
                        "the lockfile lists `{}` and no declaration of the manifest names it",
                        ids[target]
                    ),
                ));
            }
        }
    }

    let reachable_via = propagate(&edges, &member_edges, &member_indexes);
    let reached = reach_all(&edges, &member_indexes);
    for (index, id) in ids.iter().enumerate() {
        if !member_indexes.contains(&index) && !reached.contains(&index) && !members.is_empty() {
            discrepancies.push(discrepancy(
                DiscrepancyKind::OrphanLockedPackage,
                id,
                "no workspace member reaches this locked package".to_string(),
            ));
        }
    }

    let mut packages: Vec<ResolvedPackage> = lock
        .packages
        .iter()
        .enumerate()
        .map(|(index, package)| {
            let mut dependencies: Vec<String> = edges[index]
                .iter()
                .map(|&target| ids[target].clone())
                .collect();
            dependencies.sort();
            dependencies.dedup();
            let mut uses = declared_by[index].clone();
            uses.sort();
            uses.dedup();
            ResolvedPackage {
                id: ids[index].clone(),
                name: package.name.clone(),
                version: package.version.clone(),
                source: package.source.clone(),
                purl: cargo_purl(&package.name, &package.version, &package.source),
                checksum: package.checksum.clone(),
                integrity: integrity(package),
                workspace_member: member_indexes.contains(&index),
                declared_by: uses,
                direct_kinds: direct_kinds[index].clone(),
                reachable_via: reachable_via[index].clone(),
                dependencies,
            }
        })
        .collect();
    packages.sort_by(|a, b| a.id.cmp(&b.id));
    discrepancies.sort();
    discrepancies.dedup();
    unresolved.sort();
    unresolved.dedup();
    Resolved {
        packages,
        unresolved,
        discrepancies,
    }
}

/// Every package reachable from the members through any lock edge.
fn reach_all(edges: &[Vec<usize>], members: &BTreeSet<usize>) -> BTreeSet<usize> {
    let mut seen: BTreeSet<usize> = BTreeSet::new();
    let mut stack: Vec<usize> = members.iter().copied().collect();
    while let Some(node) = stack.pop() {
        for &next in &edges[node] {
            if seen.insert(next) {
                stack.push(next);
            }
        }
    }
    seen
}

/// For each package, the kinds of the member declarations that reach it.
///
/// A member reached as a dependency contributes only the edges its manifest
/// declares outside `[dev-dependencies]`: Cargo ignores the dev-dependencies
/// of a package that is not the one being built, and the lockfile cannot
/// say which of a member's edges were dev ones.
fn propagate(
    edges: &[Vec<usize>],
    member_edges: &BTreeMap<usize, Vec<(DependencyKind, usize)>>,
    member_indexes: &BTreeSet<usize>,
) -> Vec<BTreeSet<DependencyKind>> {
    let mut reachable: Vec<BTreeSet<DependencyKind>> = vec![BTreeSet::new(); edges.len()];
    let mut stack: Vec<(usize, DependencyKind)> = member_edges
        .values()
        .flatten()
        .map(|&(kind, target)| (target, kind))
        .collect();
    while let Some((node, carried)) = stack.pop() {
        if !reachable[node].insert(carried) {
            continue;
        }
        if member_indexes.contains(&node) {
            for &(kind, next) in member_edges.get(&node).into_iter().flatten() {
                match kind {
                    DependencyKind::Development => {}
                    DependencyKind::Build => stack.push((next, DependencyKind::Build)),
                    DependencyKind::Normal => stack.push((next, carried)),
                }
            }
        } else {
            stack.extend(edges[node].iter().map(|&next| (next, carried)));
        }
    }
    reachable
}

fn multiple_versions(packages: &[ResolvedPackage]) -> Vec<MultipleVersions> {
    let mut by_name: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for package in packages {
        by_name
            .entry(&package.name)
            .or_default()
            .insert(&package.version);
    }
    by_name
        .into_iter()
        .filter(|(_, versions)| versions.len() > 1)
        .map(|(name, versions)| MultipleVersions {
            name: name.to_string(),
            versions: versions.into_iter().map(str::to_string).collect(),
        })
        .collect()
}

fn resolved_digest(packages: &[ResolvedPackage]) -> String {
    let essence: Vec<_> = packages
        .iter()
        .map(|package| {
            (
                &package.id,
                package.source.lock_string(),
                &package.checksum,
                &package.dependencies,
            )
        })
        .collect();
    let bytes = serde_json::to_vec(&essence).expect("plain strings always serialise");
    ContentDigest::from_bytes(&bytes).as_hex().to_string()
}

fn layers(
    declared_complete: bool,
    has_members: bool,
    lock: Option<&CargoLock>,
    lock_unreadable: bool,
) -> Vec<LayerStatus> {
    let declared = match (has_members, declared_complete) {
        (true, true) => (
            CoverageStatus::Collected,
            "every manifest of the unit was read",
        ),
        (true, false) => (
            CoverageStatus::Partial,
            "some manifest of the unit could not be parsed",
        ),
        (false, _) => (
            CoverageStatus::NotCollected,
            "the unit has no readable package manifest",
        ),
    };
    let resolved = match (lock, lock_unreadable) {
        (Some(_), _) => (
            CoverageStatus::Collected,
            "read from Cargo.lock: versions, sources, checksums and edges; resolved features and the kind or target of edges between non-member packages are not in a lockfile",
        ),
        (None, true) => (
            CoverageStatus::NotCollected,
            "Cargo.lock could not be parsed",
        ),
        (None, false) => (
            CoverageStatus::NotCollected,
            "the revision tracks no Cargo.lock for this unit",
        ),
    };
    let rows = [
        (DependencyLayer::Declared, declared),
        (DependencyLayer::Resolved, resolved),
        (
            DependencyLayer::Observed,
            (
                CoverageStatus::NotCollected,
                "source imports are not correlated with declared or locked packages by this inventory",
            ),
        ),
        (
            DependencyLayer::Distributed,
            (
                CoverageStatus::NotCollected,
                "what a build ships needs the built artifact or its SBOM; a repository revision holds neither",
            ),
        ),
    ];
    rows.into_iter()
        .map(|(layer, (status, note))| LayerStatus {
            layer,
            status,
            note: note.to_string(),
        })
        .collect()
}

/// Builds the inventory of one workspace unit from the bytes of its files.
pub fn inventory_unit(unit: &UnitInput) -> WorkspaceInventory {
    let declared = read_manifests(unit);
    let mut discrepancies = declared.problems.clone();
    let (lock, lock_unreadable) = match &unit.lock {
        Some(file) => match CargoLock::parse(&file.bytes) {
            Ok(lock) => (Some(lock), false),
            Err(error) => {
                discrepancies.push(discrepancy(
                    DiscrepancyKind::UnreadableFile,
                    &file.path,
                    error.to_string(),
                ));
                (None, true)
            }
        },
        None => (None, false),
    };

    let (packages, unresolved) = match &lock {
        Some(lock) => {
            let resolved = resolve_unit(lock, &declared.members);
            discrepancies.extend(resolved.discrepancies);
            (resolved.packages, resolved.unresolved)
        }
        None => {
            let uses = declared
                .members
                .iter()
                .flat_map(|member| {
                    member.declarations.iter().map(|declaration| DeclaredUse {
                        member: member.name.clone(),
                        manifest: member.manifest.clone(),
                        declaration: declaration.clone(),
                    })
                })
                .collect();
            (Vec::new(), uses)
        }
    };
    discrepancies.sort();
    discrepancies.dedup();

    let freshness = if unit.lock.is_none() {
        LockFreshness::NoLockfile
    } else if declared.members.is_empty() && declared.problems.is_empty() {
        LockFreshness::NoManifest
    } else if discrepancies.iter().any(|found| found.kind.proves_stale()) {
        LockFreshness::Stale
    } else if lock_unreadable || !discrepancies.is_empty() {
        LockFreshness::Partial
    } else {
        LockFreshness::Consistent
    };
    let layer_rows = layers(
        declared.problems.is_empty(),
        !declared.members.is_empty(),
        lock.as_ref(),
        lock_unreadable,
    );
    WorkspaceInventory {
        root: display_root(&unit.root),
        lockfile: unit.lock.as_ref().map(anchor),
        lock_format_version: lock.as_ref().and_then(|lock| lock.format_version),
        manifests: declared.anchors,
        members: declared
            .members
            .iter()
            .map(|member| MemberSummary {
                name: member.name.clone(),
                version: member.version.clone(),
                manifest: member.manifest.clone(),
            })
            .collect(),
        freshness,
        discrepancies,
        multiple_versions: multiple_versions(&packages),
        resolved_digest: resolved_digest(&packages),
        packages,
        unresolved_declarations: unresolved,
        layers: layer_rows,
    }
}

#[cfg(test)]
mod tests;
