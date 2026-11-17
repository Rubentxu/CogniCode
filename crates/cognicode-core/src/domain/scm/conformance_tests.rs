//! The SCM conformance suite run over the in-memory provider, and over
//! deliberately broken providers that it must reject. A suite that
//! passes every provider proves nothing: each control below breaks one
//! clause and the suite has to name it.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::conformance::{
    ProvisionedSource, RevisionGraph, ScmHarness, ScmProvider, WORKING_COPY_BASE,
    WorkingCopyDriver, clause, run_scm_conformance,
};
use super::fake::{FakeSource, FakeWorkingCopy};
use super::{
    Change, ChangeReader, ChangeSet, PathKey, ProviderCapabilities, RevisionHandle, RevisionReader,
    ScmError, SourceInventory, SourceRef, SourceRevision, WorkingCopyReader,
};

const ALL: ProviderCapabilities = ProviderCapabilities {
    rename_evidence: true,
    ancestor_queries: true,
    staged_index: true,
    dirty_tree: true,
};

fn fake_from(graph: &RevisionGraph, capabilities: ProviderCapabilities) -> FakeSource {
    let mut source = FakeSource::new("fake", capabilities);
    for revision in graph.revisions() {
        source = source.with_revision(revision.name, &revision.parents, &revision.files);
    }
    source.checked_out(WORKING_COPY_BASE)
}

fn references(graph: &RevisionGraph) -> BTreeMap<&'static str, String> {
    graph
        .revisions()
        .iter()
        .map(|revision| (revision.name, revision.name.to_string()))
        .collect()
}

/// Builds a [`FakeSource`], optionally wrapped into a flawed provider.
struct FakeHarness {
    capabilities: ProviderCapabilities,
    flaw: Option<Flaw>,
}

impl ScmHarness for FakeHarness {
    fn provision(&self, graph: &RevisionGraph) -> ProvisionedSource {
        let inner = fake_from(graph, self.capabilities);
        let copy = inner.working_copy();
        let (provider, working_copy): (Box<dyn ScmProvider>, Box<dyn WorkingCopyDriver>) =
            match self.flaw {
                None => (Box::new(inner), Box::new(copy)),
                Some(Flaw::IndexFollowsDisk) => (Box::new(inner), Box::new(IndexFollowsDisk(copy))),
                Some(flaw) => (
                    Box::new(Flawed {
                        inner,
                        flaw,
                        calls: AtomicUsize::new(0),
                    }),
                    Box::new(copy),
                ),
            };
        ProvisionedSource {
            provider,
            source: SourceRef::new("fake", "conformance").expect("a fixed source"),
            references: references(graph),
            working_copy: Some(working_copy),
            keep_alive: None,
        }
    }
}

// ----- conforming providers ------------------------------------------------

#[test]
fn a_provider_declaring_every_capability_conforms() {
    run_scm_conformance(&FakeHarness {
        capabilities: ALL,
        flaw: None,
    })
    .assert_conforms("FakeSource declaring every capability");
}

#[test]
fn a_provider_declaring_nothing_conforms_by_refusing_every_optional_reading() {
    run_scm_conformance(&FakeHarness {
        capabilities: ProviderCapabilities::NONE,
        flaw: None,
    })
    .assert_conforms("FakeSource declaring no capability");
}

#[test]
fn every_subset_of_capabilities_conforms() {
    for mask in 0u8..16 {
        let capabilities = ProviderCapabilities {
            rename_evidence: mask & 1 != 0,
            ancestor_queries: mask & 2 != 0,
            staged_index: mask & 4 != 0,
            dirty_tree: mask & 8 != 0,
        };
        run_scm_conformance(&FakeHarness {
            capabilities,
            flaw: None,
        })
        .assert_conforms(&format!("FakeSource declaring {capabilities:?}"));
    }
}

// ----- negative controls -----------------------------------------------------

/// One way a provider can break the contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Flaw {
    /// A diff to any head is computed against the revision named `head`.
    IgnoresHeadMovement,
    /// Every revision carries the digest of the first one.
    StaleRevisionDigest,
    /// Undeclared staged index answers an empty change set.
    EmptyStagedWhenUnsupported,
    /// Undeclared dirty tree answers an empty list.
    EmptyDirtyWhenUnsupported,
    /// Undeclared ancestry answers `true` and a diff.
    InventsAncestry,
    /// Declares rename evidence and reports a delete plus an add.
    NeverPairsRenames,
    /// Declares no rename evidence and reports a rename anyway.
    InventsRenames,
    /// An unrelated pair gets an empty diff instead of `MissingAncestor`.
    MissingAncestorIsEmptyDiff,
    /// `is_ancestor` holds when either side reaches the other.
    SymmetricAncestry,
    /// Ancestry only follows direct parents.
    DirectParentsOnly,
    /// Bytes of an untracked path are an empty vector.
    MissingContentIsEmpty,
    /// A source of another provider is served.
    ServesForeignSources,
    /// An unknown reference is a provider failure, not a named miss.
    UnknownReferenceIsFailure,
    /// The change set names its endpoints the wrong way round.
    SwapsEndpoints,
    /// Dirty paths are never reported.
    ForgetsDirtyPaths,
    /// The index follows the disk (see [`IndexFollowsDisk`]).
    IndexFollowsDisk,
    /// Two different revisions resolve to the same handle.
    CollapsesRevisions,
    /// The inventory leaves a tracked file out.
    InventoryDropsAFile,
    /// The bytes of a tracked file are not the committed ones.
    CorruptsBytes,
    /// The diff lists a path that did not change.
    ReportsUnchangedPaths,
    /// The diff of the same pair differs from one call to the next.
    DiffChangesBetweenCalls,
    /// A clean checkout reports a dirty path.
    ReportsDirtyWhenClean,
}

/// The table every control is run from: flaw, the clause the suite must
/// report, and the capabilities the flawed provider declares.
const CONTROLS: &[(Flaw, &str, ProviderCapabilities)] = &[
    (Flaw::IgnoresHeadMovement, clause::CHANGES_TRANSITIVE, ALL),
    (Flaw::StaleRevisionDigest, clause::RESOLVE_DIGEST, ALL),
    (
        Flaw::EmptyStagedWhenUnsupported,
        clause::STAGED_UNDECLARED,
        ProviderCapabilities::NONE,
    ),
    (
        Flaw::EmptyDirtyWhenUnsupported,
        clause::DIRTY_UNDECLARED,
        ProviderCapabilities::NONE,
    ),
    (
        Flaw::InventsAncestry,
        clause::ANCESTRY_UNDECLARED,
        ProviderCapabilities::NONE,
    ),
    (
        Flaw::InventsAncestry,
        clause::CHANGES_UNDECLARED,
        ProviderCapabilities::NONE,
    ),
    (Flaw::NeverPairsRenames, clause::RENAME_NOT_PAIRED, ALL),
    (
        Flaw::InventsRenames,
        clause::RENAME_INVENTED,
        ProviderCapabilities {
            rename_evidence: false,
            ..ALL
        },
    ),
    (
        Flaw::MissingAncestorIsEmptyDiff,
        clause::CHANGES_NEED_ANCESTRY,
        ALL,
    ),
    (Flaw::SymmetricAncestry, clause::ANCESTRY_TABLE, ALL),
    (Flaw::DirectParentsOnly, clause::ANCESTRY_TABLE, ALL),
    (Flaw::DirectParentsOnly, clause::CHANGES_TRANSITIVE, ALL),
    (Flaw::MissingContentIsEmpty, clause::CONTENT_MISSING, ALL),
    (Flaw::ServesForeignSources, clause::RESOLVE_FOREIGN, ALL),
    (
        Flaw::UnknownReferenceIsFailure,
        clause::RESOLVE_UNKNOWN,
        ALL,
    ),
    (Flaw::SwapsEndpoints, clause::CHANGES_ENDPOINTS, ALL),
    (Flaw::ForgetsDirtyPaths, clause::DIRTY_PATHS, ALL),
    (Flaw::IndexFollowsDisk, clause::STAGED_READS_INDEX, ALL),
    (Flaw::CollapsesRevisions, clause::RESOLVE_KNOWN, ALL),
    (Flaw::InventoryDropsAFile, clause::INVENTORY_EXACT, ALL),
    (Flaw::CorruptsBytes, clause::CONTENT_BYTES, ALL),
    (Flaw::ReportsUnchangedPaths, clause::CHANGES_CLASSIFIED, ALL),
    (
        Flaw::DiffChangesBetweenCalls,
        clause::CHANGES_DETERMINISTIC,
        ALL,
    ),
    (Flaw::ReportsDirtyWhenClean, clause::WORKCOPY_CLEAN, ALL),
];

#[test]
fn the_suite_rejects_every_deliberately_broken_provider() {
    for (flaw, expected_clause, capabilities) in CONTROLS {
        run_scm_conformance(&FakeHarness {
            capabilities: *capabilities,
            flaw: Some(*flaw),
        })
        .assert_detects(&format!("{flaw:?}"), expected_clause);
    }
}

#[test]
fn every_clause_the_suite_can_report_has_a_negative_control() {
    // `WORKCOPY_UNEXERCISED` validates the harness, not the provider.
    let reportable = [
        clause::RESOLVE_KNOWN,
        clause::RESOLVE_UNKNOWN,
        clause::RESOLVE_FOREIGN,
        clause::RESOLVE_DIGEST,
        clause::INVENTORY_EXACT,
        clause::CONTENT_BYTES,
        clause::CONTENT_MISSING,
        clause::CHANGES_CLASSIFIED,
        clause::CHANGES_DETERMINISTIC,
        clause::CHANGES_ENDPOINTS,
        clause::CHANGES_TRANSITIVE,
        clause::CHANGES_UNDECLARED,
        clause::CHANGES_NEED_ANCESTRY,
        clause::RENAME_NOT_PAIRED,
        clause::RENAME_INVENTED,
        clause::ANCESTRY_TABLE,
        clause::ANCESTRY_UNDECLARED,
        clause::STAGED_UNDECLARED,
        clause::DIRTY_UNDECLARED,
        clause::WORKCOPY_CLEAN,
        clause::STAGED_READS_INDEX,
        clause::DIRTY_PATHS,
    ];
    let uncontrolled: Vec<&str> = reportable
        .into_iter()
        .filter(|reportable| {
            !CONTROLS
                .iter()
                .any(|(_, controlled, _)| controlled == reportable)
        })
        .collect();
    assert!(
        uncontrolled.is_empty(),
        "no negative control for {uncontrolled:?}"
    );
}

#[test]
fn the_fixture_graph_has_parents_before_children_and_an_unrelated_history() {
    let graph = RevisionGraph::standard();
    let mut declared = Vec::new();
    for revision in graph.revisions() {
        assert!(
            revision.parents.iter().all(|p| declared.contains(p)),
            "{} names a parent declared later",
            revision.name
        );
        declared.push(revision.name);
    }
    assert!(graph.revision("orphan").parents.is_empty());
}

// ----- the flawed provider -------------------------------------------------------

struct Flawed {
    inner: FakeSource,
    flaw: Flaw,
    calls: AtomicUsize,
}

fn failure(detail: &str) -> ScmError {
    ScmError::ProviderFailure {
        detail: detail.to_string(),
    }
}

impl Flawed {
    fn is(&self, flaw: Flaw) -> bool {
        self.flaw == flaw
    }

    fn revision_named(&self, like: &SourceRevision, name: &str) -> SourceRevision {
        self.inner
            .resolve(like.source(), name)
            .expect("the fixture revision exists")
    }

    fn empty_changes(&self, base: &SourceRevision, head: &SourceRevision) -> ChangeSet {
        ChangeSet::explicit(base.handle().clone(), head.handle().clone(), Vec::new())
    }

    fn only_direct_parent(&self, ancestor: &SourceRevision, descendant: &SourceRevision) -> bool {
        matches!(
            (ancestor.handle().as_str(), descendant.handle().as_str()),
            ("base", "head") | ("head", "tip")
        )
    }
}

impl RevisionReader for Flawed {
    fn capabilities(&self) -> ProviderCapabilities {
        self.inner.capabilities()
    }

    fn resolve(&self, source: &SourceRef, reference: &str) -> Result<SourceRevision, ScmError> {
        if self.is(Flaw::ServesForeignSources) {
            let own = SourceRef::new("fake", source.project()).expect("a fixed source");
            return self.inner.resolve(&own, reference);
        }
        if self.is(Flaw::CollapsesRevisions) && reference == "tip" {
            return self.inner.resolve(source, "head");
        }
        let resolved = self.inner.resolve(source, reference);
        if self.is(Flaw::UnknownReferenceIsFailure) {
            return resolved.map_err(|error| match error {
                ScmError::ReferenceNotFound { .. } => failure("lookup failed"),
                other => other,
            });
        }
        if self.is(Flaw::StaleRevisionDigest) {
            let first = self.inner.resolve(source, "base")?;
            return resolved.map(|revision| {
                SourceRevision::new(
                    revision.source().clone(),
                    revision.handle().clone(),
                    first.content().clone(),
                )
            });
        }
        resolved
    }

    fn inventory(&self, revision: &SourceRevision) -> Result<SourceInventory, ScmError> {
        let inventory = self.inner.inventory(revision)?;
        if self.is(Flaw::InventoryDropsAFile) {
            let kept = inventory.entries()[1..].to_vec();
            return SourceInventory::new(revision.handle(), kept);
        }
        Ok(inventory)
    }

    fn content(&self, revision: &SourceRevision, path: &PathKey) -> Result<Vec<u8>, ScmError> {
        let read = self.inner.content(revision, path);
        if self.is(Flaw::CorruptsBytes) {
            return read.map(|mut bytes| {
                bytes.push(b'\n');
                bytes
            });
        }
        if self.is(Flaw::MissingContentIsEmpty) {
            return Ok(read.unwrap_or_default());
        }
        read
    }
}

impl ChangeReader for Flawed {
    fn changes_between(
        &self,
        base: &SourceRevision,
        head: &SourceRevision,
    ) -> Result<ChangeSet, ScmError> {
        match self.flaw {
            Flaw::IgnoresHeadMovement => {
                let stale_head = self.revision_named(head, "head");
                self.inner.changes_between(base, &stale_head)
            }
            Flaw::NeverPairsRenames => {
                let changes = self.inner.changes_between(base, head)?;
                Ok(ChangeSet::between_raw(
                    &self.inner.inventory(base)?,
                    &self.inner.inventory(head)?,
                )
                .with_same_endpoints_as(&changes))
            }
            Flaw::InventsRenames => {
                let raw = ChangeSet::between_raw(
                    &self.inner.inventory(base)?,
                    &self.inner.inventory(head)?,
                );
                let mut paired = raw;
                paired.apply_rename_pairing(&[(
                    PathKey::new("src/old_name.rs").expect("path"),
                    PathKey::new("src/new_name.rs").expect("path"),
                )]);
                Ok(paired)
            }
            Flaw::MissingAncestorIsEmptyDiff => {
                self.inner
                    .changes_between(base, head)
                    .or_else(|error| match error {
                        ScmError::MissingAncestor { .. } => Ok(self.empty_changes(base, head)),
                        other => Err(other),
                    })
            }
            Flaw::DirectParentsOnly => {
                if self.only_direct_parent(base, head) {
                    self.inner.changes_between(base, head)
                } else {
                    Err(ScmError::MissingAncestor {
                        base: base.handle().as_str().to_string(),
                        head: head.handle().as_str().to_string(),
                    })
                }
            }
            Flaw::InventsAncestry => Ok(self.empty_changes(base, head)),
            Flaw::ReportsUnchangedPaths => {
                let changes = self.inner.changes_between(base, head)?;
                let mut listed = changes.changes().to_vec();
                listed.push(Change::unknown(
                    PathKey::new("src/untouched.rs").expect("path"),
                ));
                Ok(ChangeSet::explicit(
                    changes.base().clone(),
                    changes.head().clone(),
                    listed,
                ))
            }
            Flaw::DiffChangesBetweenCalls => {
                let changes = self.inner.changes_between(base, head)?;
                if self.calls.fetch_add(1, Ordering::SeqCst).is_multiple_of(2) {
                    return Ok(changes);
                }
                let fewer = changes.changes()[1..].to_vec();
                Ok(ChangeSet::explicit(
                    changes.base().clone(),
                    changes.head().clone(),
                    fewer,
                ))
            }
            Flaw::SwapsEndpoints => {
                let changes = self.inner.changes_between(base, head)?;
                Ok(ChangeSet::explicit(
                    changes.head().clone(),
                    changes.base().clone(),
                    changes.changes().to_vec(),
                ))
            }
            _ => self.inner.changes_between(base, head),
        }
    }

    fn is_ancestor(
        &self,
        maybe_ancestor: &SourceRevision,
        maybe_descendant: &SourceRevision,
    ) -> Result<bool, ScmError> {
        match self.flaw {
            Flaw::InventsAncestry => Ok(true),
            Flaw::SymmetricAncestry => {
                Ok(self.inner.is_ancestor(maybe_ancestor, maybe_descendant)?
                    || self.inner.is_ancestor(maybe_descendant, maybe_ancestor)?)
            }
            Flaw::DirectParentsOnly => {
                Ok(self.only_direct_parent(maybe_ancestor, maybe_descendant))
            }
            _ => self.inner.is_ancestor(maybe_ancestor, maybe_descendant),
        }
    }
}

impl WorkingCopyReader for Flawed {
    fn staged_changes(&self, source: &SourceRef) -> Result<ChangeSet, ScmError> {
        if self.is(Flaw::EmptyStagedWhenUnsupported) {
            return Ok(ChangeSet::explicit(
                RevisionHandle::new("HEAD").expect("handle"),
                RevisionHandle::new("index").expect("handle"),
                Vec::new(),
            ));
        }
        self.inner.staged_changes(source)
    }

    fn dirty_paths(&self, source: &SourceRef) -> Result<Vec<PathKey>, ScmError> {
        if self.is(Flaw::EmptyDirtyWhenUnsupported) {
            return Ok(Vec::new());
        }
        if self.is(Flaw::ForgetsDirtyPaths) {
            return Ok(Vec::new());
        }
        if self.is(Flaw::ReportsDirtyWhenClean) {
            return Ok(vec![PathKey::new("src/lib.rs").expect("path")]);
        }
        self.inner.dirty_paths(source)
    }
}

/// A checkout whose index is rewritten whenever the disk is: the
/// provider then cannot tell staged bytes from edited ones.
struct IndexFollowsDisk(FakeWorkingCopy);

impl WorkingCopyDriver for IndexFollowsDisk {
    fn stage(&self, path: &str, bytes: &[u8]) {
        self.0.stage(path, bytes);
    }

    fn edit(&self, path: &str, bytes: &[u8]) {
        self.0.stage(path, bytes);
        self.0.edit(path, bytes);
    }
}

trait WithEndpoints {
    fn with_same_endpoints_as(self, other: &ChangeSet) -> ChangeSet;
}

impl WithEndpoints for ChangeSet {
    fn with_same_endpoints_as(self, other: &ChangeSet) -> ChangeSet {
        ChangeSet::explicit(
            other.base().clone(),
            other.head().clone(),
            self.changes().to_vec(),
        )
    }
}
