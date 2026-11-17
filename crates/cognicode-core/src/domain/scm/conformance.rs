//! The reusable conformance suite of the SCM ports.
//!
//! [`run_scm_conformance`] drives one provider through the contract of
//! [`RevisionReader`], [`ChangeReader`] and [`WorkingCopyReader`] over a
//! fixed, pure-data [`RevisionGraph`]. The provider is built by an
//! [`ScmHarness`], the only adapter-specific piece: a fake materializes
//! the graph in memory, the Git adapter materializes it in a throwaway
//! repository, a future provider materializes it however it stores
//! history.
//!
//! The suite does not presuppose Git. It speaks of opaque references,
//! revisions with parents, files and digests, and it reads what the
//! provider DECLARES in [`ProviderCapabilities`] to decide what to
//! demand:
//!
//! * a declared reading must answer exactly; an undeclared one must
//!   answer [`ScmError::CapabilityUnsupported`], never an empty result
//!   that looks like "nothing changed";
//! * a provider that declares rename evidence must pair an exact-content
//!   move into one rename, and one that does not must never report a
//!   rename or a copy;
//! * a diff without recorded ancestry, and bytes that are not there,
//!   are named failures.
//!
//! Behaviour that belongs to one provider (Git's untracked files,
//! submodules, shallow clones) stays in that provider's own tests.
//! Compiled for unit tests and under the `test-support` feature.

use std::any::Any;
use std::collections::BTreeMap;

use crate::domain::conformance::ConformanceReport;

use super::changeset::{Change, ChangeKind, ChangeSet};
use super::error::ScmError;
use super::fake::FakeWorkingCopy;
use super::identity::{ContentDigest, PathKey, SourceRef, SourceRevision};
use super::ports::{ChangeReader, ProviderCapabilities, RevisionReader, WorkingCopyReader};

/// Identifiers of the clauses the suite checks. A negative control
/// names the clause it expects the suite to report.
pub mod clause {
    pub const RESOLVE_KNOWN: &str = "scm.resolve.known-reference";
    pub const RESOLVE_UNKNOWN: &str = "scm.resolve.unknown-reference";
    pub const RESOLVE_FOREIGN: &str = "scm.resolve.foreign-provider";
    pub const RESOLVE_DIGEST: &str = "scm.resolve.digest-matches-inventory";
    pub const INVENTORY_EXACT: &str = "scm.inventory.tracked-content";
    pub const CONTENT_BYTES: &str = "scm.content.bytes";
    pub const CONTENT_MISSING: &str = "scm.content.missing-is-unavailable";
    pub const CHANGES_CLASSIFIED: &str = "scm.changes.classification";
    pub const CHANGES_DETERMINISTIC: &str = "scm.changes.deterministic";
    pub const CHANGES_ENDPOINTS: &str = "scm.changes.names-its-endpoints";
    pub const CHANGES_TRANSITIVE: &str = "scm.changes.spans-intermediate-revisions";
    pub const CHANGES_UNDECLARED: &str = "scm.changes.undeclared-is-unsupported";
    pub const CHANGES_NEED_ANCESTRY: &str = "scm.changes.require-ancestry";
    pub const RENAME_NOT_PAIRED: &str = "scm.rename.declared-pairs-moves";
    pub const RENAME_INVENTED: &str = "scm.rename.undeclared-never-invented";
    pub const ANCESTRY_TABLE: &str = "scm.ancestry.truth-table";
    pub const ANCESTRY_UNDECLARED: &str = "scm.ancestry.undeclared-is-unsupported";
    pub const STAGED_UNDECLARED: &str = "scm.staged.undeclared-is-unsupported";
    pub const DIRTY_UNDECLARED: &str = "scm.dirty.undeclared-is-unsupported";
    pub const WORKCOPY_CLEAN: &str = "scm.working-copy.clean-is-empty";
    pub const STAGED_READS_INDEX: &str = "scm.staged.reads-the-index";
    pub const DIRTY_PATHS: &str = "scm.dirty.lists-edited-paths";
    pub const WORKCOPY_UNEXERCISED: &str = "scm.working-copy.declared-is-exercisable";
}

/// The revision the working copy of a provisioned provider sits on.
pub const WORKING_COPY_BASE: &str = "head";

const LIB_V1: &[u8] = b"pub fn answer() -> u32 {\n    41\n}\n";
const LIB_V2: &[u8] = b"pub fn answer() -> u32 {\n    42\n}\n";
const MOVED: &[u8] = b"//! A module that changes its path and nothing else.\npub struct Ledger { pub entries: Vec<String> }\n";
const REMOVED: &[u8] = b"# Notes\nThese notes are deleted by the next revision.\n";
const ADDED: &[u8] = b"pub const GREETING: &str = \"hello\";\n";
const UNTOUCHED: &[u8] = b"pub mod constants { pub const LIMIT: usize = 16; }\n";
const TIP_ONLY: &[u8] = b"pub fn only_at_the_tip() {}\n";
const ORPHAN_README: &[u8] = b"# A history with nothing in common\n";
/// What a test stages over `src/lib.rs`.
pub const STAGED_BYTES: &[u8] = b"pub fn answer() -> u32 {\n    43\n}\n";
/// What a test writes over `src/lib.rs` without staging it.
pub const EDITED_BYTES: &[u8] = b"pub fn answer() -> u32 {\n    44\n}\n";

const EDITED_PATH: &str = "src/lib.rs";
const MOVED_FROM: &str = "src/old_name.rs";
const MOVED_TO: &str = "src/new_name.rs";

/// One revision of the fixture: a name, its direct parents and its
/// tracked files.
#[derive(Debug, Clone)]
pub struct GraphRevision {
    pub name: &'static str,
    pub parents: Vec<&'static str>,
    pub files: Vec<(&'static str, &'static [u8])>,
}

/// A revision graph as pure data. A harness turns it into whatever its
/// provider stores.
#[derive(Debug, Clone)]
pub struct RevisionGraph {
    revisions: Vec<GraphRevision>,
}

impl RevisionGraph {
    /// The fixture every provider is tested over:
    ///
    /// * `base` holds four files;
    /// * `head` (child of `base`) modifies one, adds one, deletes one
    ///   and moves one to a new path with identical bytes;
    /// * `tip` (child of `head`) adds a file, so `base` reaches `tip`
    ///   only through `head`;
    /// * `orphan` has no relation to the others.
    ///
    /// Parents are listed before their children.
    pub fn standard() -> Self {
        let base = vec![
            ("src/lib.rs", LIB_V1),
            (MOVED_FROM, MOVED),
            ("docs/removed.md", REMOVED),
            ("src/untouched.rs", UNTOUCHED),
        ];
        let head = vec![
            ("src/lib.rs", LIB_V2),
            (MOVED_TO, MOVED),
            ("src/added.rs", ADDED),
            ("src/untouched.rs", UNTOUCHED),
        ];
        let mut tip = head.clone();
        tip.push(("src/tip.rs", TIP_ONLY));
        Self {
            revisions: vec![
                GraphRevision {
                    name: "base",
                    parents: vec![],
                    files: base,
                },
                GraphRevision {
                    name: "head",
                    parents: vec!["base"],
                    files: head,
                },
                GraphRevision {
                    name: "tip",
                    parents: vec!["head"],
                    files: tip,
                },
                GraphRevision {
                    name: "orphan",
                    parents: vec![],
                    files: vec![("README.md", ORPHAN_README)],
                },
            ],
        }
    }

    /// The revisions, parents before children.
    pub fn revisions(&self) -> &[GraphRevision] {
        &self.revisions
    }

    /// The revision named `name`.
    pub fn revision(&self, name: &str) -> &GraphRevision {
        self.revisions
            .iter()
            .find(|revision| revision.name == name)
            .unwrap_or_else(|| panic!("the fixture has no revision {name:?}"))
    }
}

/// What the suite needs from a provider: the three ports in one object.
pub trait ScmProvider: RevisionReader + ChangeReader + WorkingCopyReader {}

impl<T: RevisionReader + ChangeReader + WorkingCopyReader> ScmProvider for T {}

/// Edits the uncommitted state of a provisioned provider from outside
/// it, the way a developer edits a checkout.
pub trait WorkingCopyDriver {
    /// Writes `bytes` to `path` and stages it.
    fn stage(&self, path: &str, bytes: &[u8]);
    /// Writes `bytes` to `path` without staging it.
    fn edit(&self, path: &str, bytes: &[u8]);
}

impl WorkingCopyDriver for FakeWorkingCopy {
    fn stage(&self, path: &str, bytes: &[u8]) {
        FakeWorkingCopy::stage(self, path, bytes);
    }

    fn edit(&self, path: &str, bytes: &[u8]) {
        FakeWorkingCopy::edit(self, path, bytes);
    }
}

/// A provider holding the fixture, ready to be tested.
pub struct ProvisionedSource {
    pub provider: Box<dyn ScmProvider>,
    pub source: SourceRef,
    /// The reference that resolves to each revision of the graph, by
    /// the revision's name in the graph.
    pub references: BTreeMap<&'static str, String>,
    /// Present when the provider has a working copy the suite can edit.
    pub working_copy: Option<Box<dyn WorkingCopyDriver>>,
    /// Whatever must outlive the provider (a temporary directory).
    pub keep_alive: Option<Box<dyn Any>>,
}

/// Builds a fresh provider holding exactly `graph`, with its working
/// copy clean on [`WORKING_COPY_BASE`]. Called more than once per run:
/// each call must return an independent provider.
pub trait ScmHarness {
    fn provision(&self, graph: &RevisionGraph) -> ProvisionedSource;
}

/// Runs every clause of the SCM contract against the provider the
/// harness builds, and reports each clause it breaks.
pub fn run_scm_conformance(harness: &dyn ScmHarness) -> ConformanceReport {
    let graph = RevisionGraph::standard();
    let mut report = ConformanceReport::new();
    let world = harness.provision(&graph);
    if let Some(revisions) = resolve_every_revision(&world, &graph, &mut report) {
        check_resolution_edges(&world, &mut report);
        check_inventories_and_content(&world, &graph, &revisions, &mut report);
        check_changes(&world, &revisions, &mut report);
        check_ancestry(&world, &revisions, &mut report);
    }
    report.absorb(check_working_copy(harness.provision(&graph)));
    report
}

type Revisions = BTreeMap<&'static str, SourceRevision>;

fn path(value: &str) -> PathKey {
    PathKey::new(value).unwrap_or_else(|error| panic!("fixture path {value:?}: {error}"))
}

fn digest(bytes: &[u8]) -> ContentDigest {
    ContentDigest::from_bytes(bytes)
}

fn resolve_every_revision(
    world: &ProvisionedSource,
    graph: &RevisionGraph,
    report: &mut ConformanceReport,
) -> Option<Revisions> {
    let mut revisions = Revisions::new();
    for fixture in graph.revisions() {
        let reference = &world.references[fixture.name];
        match world.provider.resolve(&world.source, reference) {
            Ok(revision) => {
                let again = world.provider.resolve(&world.source, reference);
                report.require(
                    clause::RESOLVE_KNOWN,
                    again.as_ref() == Ok(&revision),
                    || format!("resolving {reference:?} twice gave {revision:?} then {again:?}"),
                );
                let inventory = world.provider.inventory(&revision);
                report.require(
                    clause::RESOLVE_DIGEST,
                    matches!(&inventory, Ok(inv) if &inv.content_digest() == revision.content()),
                    || {
                        format!(
                            "the digest of {reference:?} is not the digest of its inventory {inventory:?}"
                        )
                    },
                );
                revisions.insert(fixture.name, revision);
            }
            Err(error) => {
                report.violated(
                    clause::RESOLVE_KNOWN,
                    format!("reference {reference:?} of a known revision: {error:?}"),
                );
                return None;
            }
        }
    }
    let handles: std::collections::BTreeSet<_> =
        revisions.values().map(|r| r.handle().clone()).collect();
    report.require(
        clause::RESOLVE_KNOWN,
        handles.len() == revisions.len(),
        || "two different revisions resolved to the same handle".to_string(),
    );
    Some(revisions)
}

fn check_resolution_edges(world: &ProvisionedSource, report: &mut ConformanceReport) {
    let provider = world.source.provider().as_str();
    let asked = "no-such-reference-in-this-history";
    match world.provider.resolve(&world.source, asked) {
        Err(ScmError::ReferenceNotFound {
            provider: named_provider,
            reference,
        }) => report.require(
            clause::RESOLVE_UNKNOWN,
            named_provider == provider && reference == asked,
            || format!("not-found named provider {named_provider:?} and reference {reference:?}"),
        ),
        other => report.violated(
            clause::RESOLVE_UNKNOWN,
            format!("an unknown reference gave {other:?}"),
        ),
    }

    let foreign = SourceRef::new("foreign-provider", world.source.project())
        .expect("a fixed source is well formed");
    let known = &world.references["base"];
    match world.provider.resolve(&foreign, known) {
        Err(ScmError::ReferenceNotFound {
            provider: named_provider,
            ..
        }) => report.require(
            clause::RESOLVE_FOREIGN,
            named_provider == "foreign-provider",
            || format!("a foreign provider was refused as {named_provider:?}"),
        ),
        other => report.violated(
            clause::RESOLVE_FOREIGN,
            format!("a source of another provider was served: {other:?}"),
        ),
    }
}

fn check_inventories_and_content(
    world: &ProvisionedSource,
    graph: &RevisionGraph,
    revisions: &Revisions,
    report: &mut ConformanceReport,
) {
    for fixture in graph.revisions() {
        let revision = &revisions[fixture.name];
        let expected: BTreeMap<PathKey, ContentDigest> = fixture
            .files
            .iter()
            .map(|(file, bytes)| (path(file), digest(bytes)))
            .collect();
        match world.provider.inventory(revision) {
            Ok(inventory) => {
                let actual: BTreeMap<PathKey, ContentDigest> = inventory
                    .entries()
                    .iter()
                    .map(|entry| (entry.path.clone(), entry.content.clone()))
                    .collect();
                report.expect_eq(
                    clause::INVENTORY_EXACT,
                    &format!("tracked content of {}", fixture.name),
                    &actual,
                    &expected,
                );
                report.expect_eq(
                    clause::INVENTORY_EXACT,
                    &format!("revision named by the inventory of {}", fixture.name),
                    inventory.revision_handle(),
                    revision.handle(),
                );
            }
            Err(error) => report.violated(
                clause::INVENTORY_EXACT,
                format!("inventory of {}: {error:?}", fixture.name),
            ),
        }
        for (file, bytes) in &fixture.files {
            let read = world.provider.content(revision, &path(file));
            report.require(clause::CONTENT_BYTES, read.as_deref() == Ok(*bytes), || {
                format!("bytes of {file} at {}: {read:?}", fixture.name)
            });
        }
    }
    let head = &revisions["head"];
    for missing in ["docs/removed.md", "no/such/file.rs"] {
        let read = world.provider.content(head, &path(missing));
        report.require(
            clause::CONTENT_MISSING,
            matches!(&read, Err(ScmError::ContentUnavailable { path }) if path == missing),
            || format!("bytes of {missing} that head does not track: {read:?}"),
        );
    }
}

/// A change reduced to what the contract promises, so providers that
/// order or represent the same facts differently still compare equal.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Observed {
    path: String,
    kind: String,
    before: Option<ContentDigest>,
    after: Option<ContentDigest>,
}

fn observe(change: &Change) -> Observed {
    let kind = match change.kind() {
        ChangeKind::Added => "added".to_string(),
        ChangeKind::Deleted => "deleted".to_string(),
        ChangeKind::Modified => "modified".to_string(),
        ChangeKind::Renamed { from } => format!("renamed from {}", from.as_str()),
        ChangeKind::Copied { from } => format!("copied from {}", from.as_str()),
        ChangeKind::Unknown => "unknown".to_string(),
    };
    Observed {
        path: change.path().as_str().to_string(),
        kind,
        before: change.before().cloned(),
        after: change.after().cloned(),
    }
}

fn observed_sorted(changes: &ChangeSet) -> Vec<Observed> {
    let mut observed: Vec<Observed> = changes.changes().iter().map(observe).collect();
    observed.sort();
    observed
}

fn seen(path: &str, kind: &str, before: Option<&[u8]>, after: Option<&[u8]>) -> Observed {
    Observed {
        path: path.to_string(),
        kind: kind.to_string(),
        before: before.map(digest),
        after: after.map(digest),
    }
}

fn is_the_move(observed: &Observed) -> bool {
    observed.path == MOVED_FROM || observed.path == MOVED_TO
}

fn check_changes(world: &ProvisionedSource, revisions: &Revisions, report: &mut ConformanceReport) {
    let declared = world.provider.capabilities();
    let (base, head, tip) = (&revisions["base"], &revisions["head"], &revisions["tip"]);
    let between = world.provider.changes_between(base, head);
    if !declared.ancestor_queries {
        report.require(
            clause::CHANGES_UNDECLARED,
            between
                == Err(ScmError::CapabilityUnsupported {
                    capability: "ancestor_queries",
                }),
            || format!("a diff without declared ancestry answered {between:?}"),
        );
        return;
    }
    let changes = match between {
        Ok(changes) => changes,
        Err(error) => {
            report.violated(
                clause::CHANGES_CLASSIFIED,
                format!("a diff along recorded ancestry failed: {error:?}"),
            );
            check_unrelated_pairs_are_refused(world, revisions, report);
            return;
        }
    };

    report.expect_eq(
        clause::CHANGES_ENDPOINTS,
        "base of the change set",
        changes.base(),
        base.handle(),
    );
    report.expect_eq(
        clause::CHANGES_ENDPOINTS,
        "head of the change set",
        changes.head(),
        head.handle(),
    );
    let again = world.provider.changes_between(base, head);
    report.require(
        clause::CHANGES_DETERMINISTIC,
        again.as_ref() == Ok(&changes),
        || format!("the same diff twice gave {changes:?} then {again:?}"),
    );

    let (moved, others): (Vec<_>, Vec<_>) =
        observed_sorted(&changes).into_iter().partition(is_the_move);
    let expected_others = vec![
        seen("docs/removed.md", "deleted", Some(REMOVED), None),
        seen("src/added.rs", "added", None, Some(ADDED)),
        seen("src/lib.rs", "modified", Some(LIB_V1), Some(LIB_V2)),
    ];
    report.expect_eq(
        clause::CHANGES_CLASSIFIED,
        "added, deleted and modified paths (unchanged paths absent)",
        &others,
        &expected_others,
    );
    check_the_move(declared, &moved, report);

    let spanning = world.provider.changes_between(base, tip);
    report.require(
        clause::CHANGES_TRANSITIVE,
        matches!(&spanning, Ok(set) if observed_sorted(set)
            .contains(&seen("src/tip.rs", "added", None, Some(TIP_ONLY)))),
        || format!("a diff across an intermediate revision: {spanning:?}"),
    );
    check_unrelated_pairs_are_refused(world, revisions, report);
}

fn check_the_move(
    declared: ProviderCapabilities,
    moved: &[Observed],
    report: &mut ConformanceReport,
) {
    let renamed = seen(
        MOVED_TO,
        &format!("renamed from {MOVED_FROM}"),
        Some(MOVED),
        Some(MOVED),
    );
    let as_delete_and_add = vec![
        seen(MOVED_TO, "added", None, Some(MOVED)),
        seen(MOVED_FROM, "deleted", Some(MOVED), None),
    ];
    let claims_rename = moved
        .iter()
        .any(|o| o.kind.starts_with("renamed") || o.kind.starts_with("copied"));
    if declared.rename_evidence {
        report.require(
            clause::RENAME_NOT_PAIRED,
            moved == [renamed.clone()],
            || format!("a move declared as pairable: expected {renamed:?}, got {moved:?}"),
        );
    } else if claims_rename {
        report.violated(
            clause::RENAME_INVENTED,
            format!("rename evidence is not declared, yet the diff says {moved:?}"),
        );
    } else {
        report.require(
            clause::CHANGES_CLASSIFIED,
            moved == as_delete_and_add.as_slice(),
            || format!("an unpaired move: expected {as_delete_and_add:?}, got {moved:?}"),
        );
    }
}

fn check_unrelated_pairs_are_refused(
    world: &ProvisionedSource,
    revisions: &Revisions,
    report: &mut ConformanceReport,
) {
    let unrelated = [("orphan", "tip"), ("head", "base"), ("tip", "base")];
    for (from, to) in unrelated {
        let (base, head) = (&revisions[from], &revisions[to]);
        let result = world.provider.changes_between(base, head);
        let named_failure = matches!(&result,
            Err(ScmError::MissingAncestor { base: b, head: h })
                if b == base.handle().as_str() && h == head.handle().as_str());
        report.require(clause::CHANGES_NEED_ANCESTRY, named_failure, || {
            format!("a diff from {from} to {to}, which is not along ancestry: {result:?}")
        });
    }
}

fn check_ancestry(
    world: &ProvisionedSource,
    revisions: &Revisions,
    report: &mut ConformanceReport,
) {
    let table = [
        ("base", "head", true),
        ("base", "tip", true),
        ("head", "tip", true),
        ("head", "base", false),
        ("tip", "base", false),
        ("orphan", "tip", false),
        ("base", "orphan", false),
    ];
    for (ancestor, descendant, expected) in table {
        let answer = world
            .provider
            .is_ancestor(&revisions[ancestor], &revisions[descendant]);
        if world.provider.capabilities().ancestor_queries {
            report.require(clause::ANCESTRY_TABLE, answer == Ok(expected), || {
                format!(
                    "is {ancestor} an ancestor of {descendant}: expected {expected}, got {answer:?}"
                )
            });
        } else {
            report.require(
                clause::ANCESTRY_UNDECLARED,
                answer
                    == Err(ScmError::CapabilityUnsupported {
                        capability: "ancestor_queries",
                    }),
                || format!("an undeclared ancestry query answered {answer:?}"),
            );
        }
    }
}

fn check_working_copy(world: ProvisionedSource) -> ConformanceReport {
    let mut report = ConformanceReport::new();
    let declared = world.provider.capabilities();
    let source = &world.source;
    let staged = world.provider.staged_changes(source);
    let dirty = world.provider.dirty_paths(source);

    if declared.staged_index {
        report.require(
            clause::WORKCOPY_CLEAN,
            matches!(&staged, Ok(set) if set.changes().is_empty()),
            || format!("a clean checkout reported staged changes {staged:?}"),
        );
    } else {
        report.require(
            clause::STAGED_UNDECLARED,
            staged
                == Err(ScmError::CapabilityUnsupported {
                    capability: "staged_index",
                }),
            || format!("an undeclared staged index answered {staged:?}"),
        );
    }
    if declared.dirty_tree {
        report.require(
            clause::WORKCOPY_CLEAN,
            dirty.as_ref().is_ok_and(Vec::is_empty),
            || format!("a clean checkout reported dirty paths {dirty:?}"),
        );
    } else {
        report.require(
            clause::DIRTY_UNDECLARED,
            dirty
                == Err(ScmError::CapabilityUnsupported {
                    capability: "dirty_tree",
                }),
            || format!("an undeclared dirty tree answered {dirty:?}"),
        );
    }

    let Some(driver) = &world.working_copy else {
        report.require(
            clause::WORKCOPY_UNEXERCISED,
            !(declared.staged_index || declared.dirty_tree),
            || "a working-copy reading is declared but the harness cannot edit a checkout".into(),
        );
        return report;
    };
    if declared.staged_index {
        driver.stage(EDITED_PATH, STAGED_BYTES);
        check_staged_after(&world, STAGED_BYTES, &mut report);
    }
    driver.edit(EDITED_PATH, EDITED_BYTES);
    if declared.staged_index {
        check_staged_after(&world, STAGED_BYTES, &mut report);
    }
    if declared.dirty_tree {
        let dirty = world.provider.dirty_paths(source);
        report.require(
            clause::DIRTY_PATHS,
            dirty
                .as_ref()
                .is_ok_and(|paths| paths.contains(&path(EDITED_PATH))),
            || format!("{EDITED_PATH} was edited on disk, dirty paths: {dirty:?}"),
        );
    }
    report
}

/// The staged reading must show the INDEX bytes, whatever is on disk.
fn check_staged_after(world: &ProvisionedSource, bytes: &[u8], report: &mut ConformanceReport) {
    let staged = world.provider.staged_changes(&world.source);
    let expected = seen(EDITED_PATH, "modified", Some(LIB_V2), Some(bytes));
    report.require(
        clause::STAGED_READS_INDEX,
        matches!(&staged, Ok(set) if observed_sorted(set) == [expected.clone()]),
        || format!("staged {EDITED_PATH}: expected {expected:?}, got {staged:?}"),
    );
}
