//! The vocabulary of a resolved dependency inventory.
//!
//! One inventory describes one Cargo workspace unit at one revision. Four
//! layers are kept apart and never merged into a single "dependency" notion:
//!
//! * **declared**: what a manifest asks for (requirement, kind, target,
//!   features);
//! * **resolved**: what the lockfile pins (version, source, checksum);
//! * **observed**: what the source code imports (not collected here);
//! * **distributed**: what a built artifact actually ships (not collected
//!   here: it needs the artifact or its SBOM, which a repository revision
//!   does not hold).
//!
//! A manifest is not a distributed package, a PURL is not a digest, and a
//! layer that was not collected says so in [`LayerStatus`] instead of being
//! silently absent.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

use super::declared::{DeclaredDependency, DependencyKind};
use super::source::PackageSource;

/// The four layers a dependency can be observed in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyLayer {
    /// What a manifest asks for.
    Declared,
    /// What the lockfile pins.
    Resolved,
    /// What the source code imports.
    Observed,
    /// What a built artifact ships.
    Distributed,
}

/// How completely a layer was collected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoverageStatus {
    /// Every input the layer reads was read.
    Collected,
    /// Some inputs were missing or unreadable; the layer is incomplete.
    Partial,
    /// The layer was not read at all.
    NotCollected,
}

/// The state of one layer in one inventory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayerStatus {
    /// The layer.
    pub layer: DependencyLayer,
    /// How completely it was collected.
    pub status: CoverageStatus,
    /// Why, in one sentence.
    pub note: String,
}

/// What guards the bytes of a resolved package.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Integrity {
    /// The lockfile records a registry checksum (SHA-256).
    Checksummed,
    /// A Git commit pins the content; the lockfile records no checksum.
    PinnedCommit,
    /// Neither a checksum nor a commit: a path package, or a registry
    /// package the lockfile records no checksum for.
    Unpinned,
}

/// A workspace member's declaration of a dependency.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct DeclaredUse {
    /// The member that declares it.
    pub member: String,
    /// The manifest the declaration is in.
    pub manifest: String,
    /// The declaration.
    pub declaration: DeclaredDependency,
}

/// One package the lockfile resolves.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedPackage {
    /// `name@version`, suffixed with `#<source>` only when two packages
    /// share the same name and version. Stable inside one inventory.
    pub id: String,
    /// The package name.
    pub name: String,
    /// The locked version.
    pub version: String,
    /// Where the package comes from.
    pub source: PackageSource,
    /// Package URL identity, where the source has a published identity. An
    /// identity only: not a digest and not provenance.
    pub purl: Option<String>,
    /// The registry checksum (SHA-256 hex) the lockfile records.
    pub checksum: Option<String>,
    /// What guards the package bytes.
    pub integrity: Integrity,
    /// Whether the package is a member of the workspace.
    pub workspace_member: bool,
    /// The declarations by workspace members that resolve to this package.
    pub declared_by: Vec<DeclaredUse>,
    /// The kinds of the member declarations that resolve to this package.
    pub direct_kinds: BTreeSet<DependencyKind>,
    /// The kinds of the member declarations from which the package is
    /// reachable through lockfile edges. An upper bound: a lockfile does
    /// not say the kind of an edge between two non-member packages, so a
    /// build-only dependency of a registry crate appears under the kind of
    /// the member edge that reached the crate.
    pub reachable_via: BTreeSet<DependencyKind>,
    /// The ids of the packages this one depends on, sorted.
    pub dependencies: Vec<String>,
}

/// A member of the workspace unit.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct MemberSummary {
    /// The package name.
    pub name: String,
    /// The version, when it could be determined.
    pub version: Option<String>,
    /// The manifest path.
    pub manifest: String,
}

/// A file the inventory was computed from, with the digest of its bytes.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FileAnchor {
    /// The repository-relative path.
    pub path: String,
    /// The SHA-256 of the bytes.
    pub digest: String,
}

/// Whether the lockfile agrees with the manifests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LockFreshness {
    /// Every declaration, member and edge agrees and every requirement was
    /// checked.
    Consistent,
    /// At least one discrepancy shows the lockfile needs an update.
    Stale,
    /// No discrepancy found, but some requirement or file could not be
    /// checked: consistency is not vouched for.
    Partial,
    /// The unit has manifests and no lockfile.
    NoLockfile,
    /// The unit has a lockfile and no readable manifest.
    NoManifest,
}

/// The kinds of disagreement between layers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiscrepancyKind {
    /// A member has no `[[package]]` in the lockfile.
    MemberNotLocked,
    /// A member's version differs from the locked one.
    MemberVersionMismatch,
    /// A member declares a dependency its lock entry does not list.
    DeclaredNotLocked,
    /// A member's lock entry lists a dependency no declaration explains.
    LockedNotDeclared,
    /// The locked version is outside the declared requirement.
    RequirementUnsatisfied,
    /// The requirement is in a grammar the reader does not cover.
    RequirementUnverifiable,
    /// A locked package no member reaches.
    OrphanLockedPackage,
    /// A lock edge names a package the lockfile does not contain.
    DanglingLockEdge,
    /// A manifest or lockfile could not be parsed.
    UnreadableFile,
}

impl DiscrepancyKind {
    /// Whether this kind proves the lockfile is out of date (as opposed to
    /// saying the check could not be completed).
    pub fn proves_stale(self) -> bool {
        !matches!(self, Self::RequirementUnverifiable | Self::UnreadableFile)
    }
}

/// One disagreement between layers, with the subject it is about.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Discrepancy {
    /// What kind of disagreement.
    pub kind: DiscrepancyKind,
    /// The member, package or file it is about.
    pub subject: String,
    /// What exactly disagrees.
    pub detail: String,
}

/// A crate present in more than one version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultipleVersions {
    /// The crate name.
    pub name: String,
    /// The locked versions, sorted.
    pub versions: Vec<String>,
}

/// The inventory of one Cargo workspace unit at one revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceInventory {
    /// The unit root directory, relative to the repository (`.` for the root).
    pub root: String,
    /// The lockfile the resolved layer was read from, with its digest. This
    /// anchors every resolved fact to those bytes.
    pub lockfile: Option<FileAnchor>,
    /// The lockfile format version.
    pub lock_format_version: Option<i64>,
    /// The manifests the declared layer was read from, with their digests.
    pub manifests: Vec<FileAnchor>,
    /// The workspace members.
    pub members: Vec<MemberSummary>,
    /// Whether the lockfile agrees with the manifests.
    pub freshness: LockFreshness,
    /// Every disagreement found.
    pub discrepancies: Vec<Discrepancy>,
    /// Crates locked in more than one version.
    pub multiple_versions: Vec<MultipleVersions>,
    /// The resolved packages, sorted by id.
    pub packages: Vec<ResolvedPackage>,
    /// Member declarations nothing resolves (no lockfile, or not locked).
    pub unresolved_declarations: Vec<DeclaredUse>,
    /// The state of the four layers.
    pub layers: Vec<LayerStatus>,
    /// A digest of the resolved packages and their edges: equal digests mean
    /// equal resolved graphs, whatever the revision handles.
    pub resolved_digest: String,
}
