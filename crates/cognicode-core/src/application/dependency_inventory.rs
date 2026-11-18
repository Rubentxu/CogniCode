//! The use case behind the dependency inventory: read the Cargo files of a
//! revision, reconcile them, and answer the three questions a consumer asks
//! of it (the inventory, who depends on a package, what changed since a
//! base revision).
//!
//! The CLI and the MCP server reach this module through one goal, so both
//! surfaces get the same answer and the same limits. Reading is offline and
//! bounded: only the bytes of tracked `Cargo.toml` and `Cargo.lock` files
//! are read, through the revision reader, and Cargo is never launched.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::application::artifact_sbom::{DistributionAnswer, DistributionQuery, distribute};
use crate::domain::dependency_inventory::sbom::SbomLimits;
use crate::domain::dependency_inventory::{
    DependentsReport, Discrepancy, DiscrepancyKind, InventoryDelta, LockFreshness, SourceFile,
    WorkspaceInventory, dependents_of, diff_inventories, diff_packages, inventory_unit, plan_units,
};
use crate::domain::scm::{BlobRead, PathKey, RevisionReader, ScmError, SourceRef, SourceRevision};

/// How much of a repository the reader is willing to read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InventoryLimits {
    /// The largest single Cargo file read, in bytes.
    pub max_file_bytes: u64,
    /// The most Cargo files read from one revision.
    pub max_files: usize,
    /// The bounds of an SBOM document.
    pub sbom: SbomLimits,
}

impl Default for InventoryLimits {
    fn default() -> Self {
        Self {
            max_file_bytes: 8 * 1024 * 1024,
            max_files: 2_000,
            sbom: SbomLimits::default(),
        }
    }
}

/// What a consumer asks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryQuery {
    /// The revision to inventory (`HEAD`, a branch, a commit).
    pub revision: String,
    /// A revision to compare against: adds the delta of the resolved layer.
    pub base: Option<String>,
    /// A package (`name` or `name@version`) whose transitive dependents are
    /// wanted.
    pub dependents: Option<String>,
    /// Restricts the answer to the units that contain or lie under this
    /// repository-relative path.
    pub path: Option<String>,
    /// An SBOM to compare the resolved inventory with: fills the
    /// distributed layer as a claim of its producer.
    pub distribution: Option<DistributionQuery>,
}

/// A resolved revision, as the answer anchors itself to it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevisionAnchor {
    /// The reference the caller supplied.
    pub requested_ref: String,
    /// The immutable revision handle it resolved to.
    pub revision: String,
    /// The digest of all tracked content of the revision.
    pub content_digest: String,
}

/// A Cargo file the reader could not use.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkippedFile {
    /// The repository-relative path.
    pub path: String,
    /// Why it was not read.
    pub reason: String,
}

/// What of the revision was read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceCoverage {
    /// The Cargo files the revision tracks.
    pub cargo_files_tracked: usize,
    /// The Cargo files that were read.
    pub cargo_files_read: usize,
    /// The ones that were not.
    pub skipped: Vec<SkippedFile>,
    /// Whether every tracked Cargo file was read.
    pub complete: bool,
}

/// The dependents of a package in one unit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnitDependents {
    /// The unit root.
    pub root: String,
    /// The dependents, or `None` when the unit does not resolve the package.
    pub report: Option<DependentsReport>,
}

/// How a unit differs between the base and the head revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnitChange {
    /// The unit exists only at the head.
    Added,
    /// The unit exists only at the base.
    Removed,
    /// The unit exists on both sides with a different resolved graph.
    Changed,
    /// The unit exists on both sides with the same resolved graph.
    Unchanged,
}

/// The change of one unit's resolved layer between two revisions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnitDelta {
    /// The unit root.
    pub root: String,
    /// How the unit differs.
    pub change: UnitChange,
    /// The lockfile digest at the base, when the unit had one.
    pub base_lock_digest: Option<String>,
    /// The lockfile digest at the head, when the unit has one.
    pub head_lock_digest: Option<String>,
    /// Whether the lockfile agreed with the manifests at the base.
    pub base_freshness: Option<LockFreshness>,
    /// Whether the lockfile agrees with the manifests at the head.
    pub head_freshness: Option<LockFreshness>,
    /// The packages added, removed and updated.
    pub delta: InventoryDelta,
}

/// The comparison with a base revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InventoryComparison {
    /// The base revision.
    pub base: RevisionAnchor,
    /// What was read of the base.
    pub base_coverage: SourceCoverage,
    /// One entry per unit that exists on either side.
    pub units: Vec<UnitDelta>,
}

/// The answer to an [`InventoryQuery`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DependencyInventoryAnswer {
    /// The revision the inventory describes.
    pub revision: RevisionAnchor,
    /// What of that revision was read.
    pub coverage: SourceCoverage,
    /// The ecosystems that were inventoried.
    pub ecosystems: Vec<String>,
    /// One inventory per workspace unit, by root.
    pub units: Vec<WorkspaceInventory>,
    /// The dependents of the queried package, when one was asked for.
    pub dependents: Option<Vec<UnitDependents>>,
    /// The comparison with the base revision, when one was asked for.
    pub comparison: Option<InventoryComparison>,
    /// The comparison with an SBOM, when one was supplied.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distributed: Option<DistributionAnswer>,
}

/// A revision's units with what was read to build them.
struct Collected {
    anchor: RevisionAnchor,
    coverage: SourceCoverage,
    units: Vec<WorkspaceInventory>,
}

fn is_cargo_file(path: &str) -> bool {
    matches!(path.rsplit('/').next(), Some("Cargo.toml" | "Cargo.lock"))
}

fn read_cargo_files(
    reader: &dyn RevisionReader,
    revision: &SourceRevision,
    limits: &InventoryLimits,
) -> Result<(Vec<SourceFile>, SourceCoverage), ScmError> {
    let inventory = reader.inventory(revision)?;
    let tracked: Vec<PathKey> = inventory
        .entries()
        .iter()
        .filter(|entry| is_cargo_file(entry.path.as_str()))
        .map(|entry| entry.path.clone())
        .collect();
    let mut skipped = Vec::new();
    let (wanted, beyond) = tracked.split_at(tracked.len().min(limits.max_files));
    skipped.extend(beyond.iter().map(|path| SkippedFile {
        path: path.as_str().to_string(),
        reason: format!("more than {} Cargo files are tracked", limits.max_files),
    }));
    let answers = reader.contents(revision, wanted, limits.max_file_bytes);
    let mut files = Vec::new();
    for (path, answer) in wanted.iter().zip(answers) {
        match answer {
            Ok(BlobRead::Bytes(bytes)) => files.push(SourceFile {
                path: path.as_str().to_string(),
                bytes,
            }),
            Ok(BlobRead::TooLarge { bytes }) => skipped.push(SkippedFile {
                path: path.as_str().to_string(),
                reason: format!(
                    "{bytes} bytes exceed the {} byte limit",
                    limits.max_file_bytes
                ),
            }),
            Err(error) => skipped.push(SkippedFile {
                path: path.as_str().to_string(),
                reason: error.to_string(),
            }),
        }
    }
    let coverage = SourceCoverage {
        cargo_files_tracked: tracked.len(),
        cargo_files_read: files.len(),
        complete: skipped.is_empty(),
        skipped,
    };
    Ok((files, coverage))
}

/// A lockfile that could not be read must not look like a missing one.
fn account_for_skipped(units: &mut [WorkspaceInventory], skipped: &[SkippedFile]) {
    for unit in units {
        let lock_path = if unit.root == "." {
            "Cargo.lock".to_string()
        } else {
            format!("{}/Cargo.lock", unit.root)
        };
        let Some(skip) = skipped.iter().find(|skip| skip.path == lock_path) else {
            continue;
        };
        unit.discrepancies.push(Discrepancy {
            kind: DiscrepancyKind::UnreadableFile,
            subject: lock_path,
            detail: skip.reason.clone(),
        });
        if unit.freshness == LockFreshness::NoLockfile {
            unit.freshness = LockFreshness::Partial;
        }
    }
}

fn collect(
    reader: &dyn RevisionReader,
    source: &SourceRef,
    reference: &str,
    limits: &InventoryLimits,
) -> Result<Collected, ScmError> {
    let revision = reader.resolve(source, reference)?;
    let (files, coverage) = read_cargo_files(reader, &revision, limits)?;
    let mut units: Vec<WorkspaceInventory> = plan_units(files).iter().map(inventory_unit).collect();
    account_for_skipped(&mut units, &coverage.skipped);
    units.sort_by(|a, b| a.root.cmp(&b.root));
    Ok(Collected {
        anchor: RevisionAnchor {
            requested_ref: reference.to_string(),
            revision: revision.handle().as_str().to_string(),
            content_digest: revision.content().as_hex().to_string(),
        },
        coverage,
        units,
    })
}

/// Whether a unit rooted at `root` is in the scope of the repository path
/// `scope`: the unit contains the path or lies under it.
fn in_scope(root: &str, scope: &str) -> bool {
    let scope = scope.trim_start_matches("./").trim_end_matches('/');
    if scope.is_empty() || scope == "." || root == "." {
        return true;
    }
    scope == root
        || scope.starts_with(&format!("{root}/"))
        || root.starts_with(&format!("{scope}/"))
}

fn compare_units(base: &[WorkspaceInventory], head: &[WorkspaceInventory]) -> Vec<UnitDelta> {
    let before: BTreeMap<&str, &WorkspaceInventory> =
        base.iter().map(|unit| (unit.root.as_str(), unit)).collect();
    let after: BTreeMap<&str, &WorkspaceInventory> =
        head.iter().map(|unit| (unit.root.as_str(), unit)).collect();
    let roots: std::collections::BTreeSet<&str> =
        before.keys().chain(after.keys()).copied().collect();
    roots
        .into_iter()
        .map(|root| {
            let (old, new) = (before.get(root), after.get(root));
            let (change, delta) = match (old, new) {
                (Some(old), Some(new)) => {
                    let delta = diff_inventories(old, new);
                    let change = if delta.unchanged {
                        UnitChange::Unchanged
                    } else {
                        UnitChange::Changed
                    };
                    (change, delta)
                }
                (None, Some(new)) => (UnitChange::Added, diff_packages(&[], &new.packages)),
                (Some(old), None) => (UnitChange::Removed, diff_packages(&old.packages, &[])),
                (None, None) => unreachable!("a root comes from one of the two sides"),
            };
            UnitDelta {
                root: root.to_string(),
                change,
                base_lock_digest: old
                    .and_then(|unit| unit.lockfile.as_ref())
                    .map(|f| f.digest.clone()),
                head_lock_digest: new
                    .and_then(|unit| unit.lockfile.as_ref())
                    .map(|f| f.digest.clone()),
                base_freshness: old.map(|unit| unit.freshness),
                head_freshness: new.map(|unit| unit.freshness),
                delta,
            }
        })
        .collect()
}

/// Answers `query` over the revisions the reader serves.
pub fn answer(
    reader: &dyn RevisionReader,
    source: &SourceRef,
    query: &InventoryQuery,
    limits: &InventoryLimits,
) -> Result<DependencyInventoryAnswer, ScmError> {
    let head = collect(reader, source, &query.revision, limits)?;
    let base = query
        .base
        .as_deref()
        .map(|reference| collect(reader, source, reference, limits))
        .transpose()?;
    let keep = |unit: &WorkspaceInventory| {
        query
            .path
            .as_deref()
            .is_none_or(|scope| in_scope(&unit.root, scope))
    };
    let comparison = base.map(|base| {
        let base_units: Vec<_> = base.units.iter().filter(|u| keep(u)).cloned().collect();
        let head_units: Vec<_> = head.units.iter().filter(|u| keep(u)).cloned().collect();
        InventoryComparison {
            base: base.anchor,
            base_coverage: base.coverage,
            units: compare_units(&base_units, &head_units),
        }
    });
    let mut units: Vec<WorkspaceInventory> = head.units.into_iter().filter(|u| keep(u)).collect();
    let distributed = query
        .distribution
        .as_ref()
        .map(|asked| distribute(&mut units, asked, &limits.sbom));
    let dependents = query.dependents.as_deref().map(|package| {
        units
            .iter()
            .map(|unit| UnitDependents {
                root: unit.root.clone(),
                report: dependents_of(unit, package),
            })
            .collect()
    });
    Ok(DependencyInventoryAnswer {
        revision: head.anchor,
        coverage: head.coverage,
        ecosystems: vec!["cargo".to_string()],
        units,
        dependents,
        comparison,
        distributed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::scm::{
        ContentDigest, InventoryEntry, ProviderCapabilities, RevisionHandle, SourceInventory,
    };
    use std::collections::HashMap;

    /// Revisions held in memory: reference -> (path -> bytes).
    struct Memory(HashMap<String, BTreeMap<String, Vec<u8>>>);

    impl Memory {
        fn revision(&self, source: &SourceRef, reference: &str) -> Option<SourceRevision> {
            let files = self.0.get(reference)?;
            let handle = RevisionHandle::new(&format!("rev-{reference}")).expect("handle");
            let entries = self.entries(&handle, files);
            Some(SourceRevision::new(
                source.clone(),
                handle,
                entries.content_digest(),
            ))
        }

        fn entries(
            &self,
            handle: &RevisionHandle,
            files: &BTreeMap<String, Vec<u8>>,
        ) -> SourceInventory {
            let entries = files
                .iter()
                .map(|(path, bytes)| InventoryEntry {
                    path: PathKey::new(path).expect("path"),
                    content: ContentDigest::from_bytes(bytes),
                })
                .collect();
            SourceInventory::new(handle, entries).expect("inventory")
        }
    }

    impl RevisionReader for Memory {
        fn capabilities(&self) -> ProviderCapabilities {
            ProviderCapabilities::NONE
        }
        fn resolve(&self, source: &SourceRef, reference: &str) -> Result<SourceRevision, ScmError> {
            self.revision(source, reference)
                .ok_or_else(|| ScmError::ReferenceNotFound {
                    provider: "memory".into(),
                    reference: reference.into(),
                })
        }
        fn inventory(&self, revision: &SourceRevision) -> Result<SourceInventory, ScmError> {
            let reference = revision.handle().as_str().trim_start_matches("rev-");
            Ok(self.entries(revision.handle(), &self.0[reference]))
        }
        fn content(&self, revision: &SourceRevision, path: &PathKey) -> Result<Vec<u8>, ScmError> {
            let reference = revision.handle().as_str().trim_start_matches("rev-");
            self.0[reference]
                .get(path.as_str())
                .cloned()
                .ok_or_else(|| ScmError::ContentUnavailable {
                    path: path.as_str().into(),
                })
        }
    }

    const MANIFEST: &str =
        "[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n[dependencies]\nserde = \"1\"\n";

    fn lock(version: &str) -> String {
        format!(
            "version = 3\n\n[[package]]\nname = \"app\"\nversion = \"0.1.0\"\ndependencies = [\"serde\"]\n\n[[package]]\nname = \"serde\"\nversion = \"{version}\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"{}\"\n",
            "a".repeat(64)
        )
    }

    fn files(version: &str) -> BTreeMap<String, Vec<u8>> {
        BTreeMap::from([
            ("Cargo.toml".to_string(), MANIFEST.as_bytes().to_vec()),
            ("Cargo.lock".to_string(), lock(version).into_bytes()),
            ("README.md".to_string(), b"not a cargo file".to_vec()),
        ])
    }

    fn source() -> SourceRef {
        SourceRef::new("memory", "repo").expect("source")
    }

    fn query(revision: &str) -> InventoryQuery {
        InventoryQuery {
            revision: revision.into(),
            base: None,
            dependents: None,
            path: None,
            distribution: None,
        }
    }

    #[test]
    fn the_inventory_is_anchored_to_the_revision_and_to_the_lock_digest() {
        let reader = Memory(HashMap::from([("head".to_string(), files("1.0.1"))]));

        let answer = answer(
            &reader,
            &source(),
            &query("head"),
            &InventoryLimits::default(),
        )
        .expect("answer");

        assert_eq!(answer.revision.revision, "rev-head");
        assert_eq!(answer.coverage.cargo_files_tracked, 2);
        assert!(answer.coverage.complete);
        let lock_digest = &answer.units[0].lockfile.as_ref().expect("lock").digest;
        assert_eq!(
            *lock_digest,
            ContentDigest::from_bytes(lock("1.0.1").as_bytes()).as_hex()
        );
    }

    #[test]
    fn an_oversized_lockfile_is_reported_and_never_read_as_a_missing_one() {
        let reader = Memory(HashMap::from([("head".to_string(), files("1.0.1"))]));
        let limits = InventoryLimits {
            max_file_bytes: 100,
            max_files: 10,
            ..InventoryLimits::default()
        };

        let answer = answer(&reader, &source(), &query("head"), &limits).expect("answer");

        assert!(!answer.coverage.complete);
        assert_eq!(answer.coverage.skipped[0].path, "Cargo.lock");
        assert_ne!(answer.units[0].freshness, LockFreshness::NoLockfile);
        assert!(
            answer.units[0]
                .discrepancies
                .iter()
                .any(|d| d.kind == DiscrepancyKind::UnreadableFile)
        );
    }

    #[test]
    fn a_base_revision_adds_the_delta_of_the_resolved_layer() {
        let reader = Memory(HashMap::from([
            ("base".to_string(), files("1.0.1")),
            ("head".to_string(), files("1.0.2")),
        ]));
        let ask = InventoryQuery {
            base: Some("base".into()),
            ..query("head")
        };

        let answer = answer(&reader, &source(), &ask, &InventoryLimits::default()).expect("answer");

        let comparison = answer.comparison.expect("comparison");
        assert_eq!(comparison.base.revision, "rev-base");
        assert_eq!(comparison.units[0].change, UnitChange::Changed);
        assert_ne!(
            comparison.units[0].base_lock_digest,
            comparison.units[0].head_lock_digest
        );
        assert_eq!(comparison.units[0].delta.updated[0].name, "serde");
    }

    #[test]
    fn a_unit_that_appears_at_the_head_is_added_with_all_its_packages() {
        let reader = Memory(HashMap::from([
            ("base".to_string(), BTreeMap::new()),
            ("head".to_string(), files("1.0.1")),
        ]));
        let ask = InventoryQuery {
            base: Some("base".into()),
            ..query("head")
        };

        let answer = answer(&reader, &source(), &ask, &InventoryLimits::default()).expect("answer");

        let unit = &answer.comparison.expect("comparison").units[0];
        assert_eq!(unit.change, UnitChange::Added);
        assert_eq!(unit.delta.added.len(), 2);
    }

    #[test]
    fn dependents_are_asked_per_unit_and_an_unknown_package_is_not_an_empty_list() {
        let reader = Memory(HashMap::from([("head".to_string(), files("1.0.1"))]));
        let ask = InventoryQuery {
            dependents: Some("serde".into()),
            ..query("head")
        };
        let known = answer(&reader, &source(), &ask, &InventoryLimits::default()).expect("answer");
        let unknown = answer(
            &reader,
            &source(),
            &InventoryQuery {
                dependents: Some("missing".into()),
                ..query("head")
            },
            &InventoryLimits::default(),
        )
        .expect("answer");

        let report = known.dependents.expect("asked")[0]
            .report
            .clone()
            .expect("known");
        assert_eq!(report.affected_members, vec!["app@0.1.0"]);
        assert!(unknown.dependents.expect("asked")[0].report.is_none());
    }

    #[test]
    fn a_path_scope_keeps_the_units_that_contain_or_lie_under_it() {
        assert!(in_scope(".", "any/where"));
        assert!(in_scope("tools/a", "tools/a/src"));
        assert!(in_scope("tools/a", "tools"));
        assert!(!in_scope("tools/a", "tools/b"));
    }

    #[test]
    fn an_unknown_reference_is_an_error_not_an_empty_inventory() {
        let reader = Memory(HashMap::new());

        let result = answer(
            &reader,
            &source(),
            &query("nope"),
            &InventoryLimits::default(),
        );

        assert!(matches!(result, Err(ScmError::ReferenceNotFound { .. })));
    }
}
