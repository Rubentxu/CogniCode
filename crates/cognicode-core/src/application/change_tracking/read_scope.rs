//! Published read scopes: what a facts query read, by file and symbol, and
//! which reads a set of changed facts invalidates.
//!
//! The execution read-set ([`ReadSet`]) names [`FactId`]s, and a fact id is
//! scoped to one snapshot: the same assertion can carry a different id in the
//! next revision. A consumer that holds a read-set from the base revision
//! therefore cannot ask "did anything I read change?" on its own. A
//! [`ReadScope`] projects the read-set onto the `(path, symbol)` pairs the
//! facts live under, which are comparable across revisions, and carries the
//! completeness marks a consumer needs to judge it.
//!
//! ## Resolution
//!
//! [`resolve_reads`] answers one question for one scope against the facts
//! that changed ([`ChangedFacts`]):
//!
//! - [`ReadVerdict::Invalidated`]: a changed fact lives where the query read.
//! - [`ReadVerdict::Valid`]: nothing the query read changed, and nothing
//!   unknown could have.
//! - [`ReadVerdict::Revalidate`]: the scope cannot prove either. An absent,
//!   truncated or unreadable scope, an unattributed read or change, an
//!   escalated path the facts cannot express, missing facts, or a new fact a
//!   wide read could have seen all land here. Uncertainty never collapses
//!   into `Valid`, and a scope is never extrapolated past what it recorded.
//!
//! The rules mirror the affected-work planner (`planner`): removed or changed
//! facts that were read invalidate, new facts an aggregate read could have
//! seen and escalated paths make the answer unknown. They are not a second
//! planner; [`ChangedFacts::between`] takes its delta and its escalations
//! from the same bridge ([`delta_from_change_set`]) the planner consumes.
//!
//! ## Wire contract
//!
//! A [`ReadScope`] serializes as JSON with the schema
//! [`READ_SCOPE_SCHEMA`] (`cognicode.read-scope/v1`):
//!
//! | field | type | meaning |
//! | --- | --- | --- |
//! | `schema` | string | schema version; required |
//! | `reads` | array of `{path, symbol?}` | sorted, deduplicated; no `symbol` means the whole file |
//! | `truncated` | bool | the recorder dropped reads past its bound |
//! | `unresolved_reads` | integer | reads that could not be attributed to a path; default 0 |
//! | `breadth` | `"pointwise"` or `"aggregate"` | whether the query also read a whole collection; default `"aggregate"` |
//!
//! Compatibility is additive: later versions of the same schema may add
//! fields, readers ignore fields they do not know, and a missing optional
//! field takes its conservative default (`breadth` defaults to `aggregate`,
//! the wider assumption). A change that removes or re-types a field, or
//! changes what a field means, publishes a new schema string. A reader that
//! meets a schema it does not know treats the scope as absent, and
//! [`resolve_reads`] answers [`ReadVerdict::Revalidate`] for it.
//!
//! The symbol is the declared name without its line: a line number shifts
//! when unrelated code above it is edited, and it is not part of what a
//! query reads. Two symbols with the same name in one file share a scope,
//! which can only widen an invalidation, never narrow it.
//!
//! This module is pure: no I/O, no clock, no persistence.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde::{Deserialize, Serialize};

use crate::domain::evidence_kernel::fact::{Fact, FactValue};
use crate::domain::evidence_kernel::identity::Identity;
use crate::domain::evidence_kernel::ids::{EntityId, FactId};
use crate::domain::evidence_kernel::semantic_diff::FactDelta;
use crate::domain::findings::detector_reuse::DetectorRunRecord;
use crate::domain::readset::ReadSet;
use crate::domain::scm::{ChangeKind, ChangeSet};

use super::revision_facts::ProducedRevision;
use super::scm_delta::{RevisionFacts, ScmDeltaError, delta_from_change_set};

/// The schema string of a published [`ReadScope`].
pub const READ_SCOPE_SCHEMA: &str = "cognicode.read-scope/v1";

/// The predicate whose facts name a symbol: `(symbol, defines, identity)`.
const DEFINES_PREDICATE: &str = "core:defines";

/// One place a query read: a file, and optionally a symbol inside it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ScopedRead {
    /// The repository-relative path of the file.
    pub path: String,
    /// The declared name of the symbol, without its line. `None` means the
    /// query read the file as a whole (or a fact no symbol owns).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
}

impl ScopedRead {
    /// Whether a read here and a change there touch the same place: the same
    /// path, and the same symbol unless either side speaks for the whole file.
    fn overlaps(&self, path: &str, symbol: Option<&str>) -> bool {
        self.path == path
            && match (self.symbol.as_deref(), symbol) {
                (Some(read), Some(changed)) => read == changed,
                _ => true,
            }
    }
}

/// How wide a query read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadBreadth {
    /// The query read exactly the facts it names: a new fact elsewhere
    /// cannot change its answer.
    Pointwise,
    /// The query also read a whole collection ("every function"): a new fact
    /// anywhere could change its answer.
    Aggregate,
}

fn conservative_breadth() -> ReadBreadth {
    ReadBreadth::Aggregate
}

/// The published read-set of a facts query. See the module documentation for
/// the wire contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadScope {
    /// The schema this value was written under.
    pub schema: String,
    /// What the query read, sorted and deduplicated.
    pub reads: Vec<ScopedRead>,
    /// The recorder dropped reads past its bound: the list is a lower bound.
    pub truncated: bool,
    /// Reads whose fact could not be attributed to a path in the facts the
    /// scope was built from. They are reads the scope cannot name.
    #[serde(default)]
    pub unresolved_reads: usize,
    /// How wide the query read.
    #[serde(default = "conservative_breadth")]
    pub breadth: ReadBreadth,
}

impl ReadScope {
    /// The scope of a read-set, resolved against the facts of the revision
    /// the read-set was recorded on.
    pub fn from_read_set(read_set: &ReadSet, facts: &RevisionFacts, breadth: ReadBreadth) -> Self {
        Self::from_reads(
            read_set.iter().copied(),
            read_set.is_truncated(),
            facts,
            breadth,
        )
    }

    /// The scope of one stored detector run, or `None` when the run was
    /// recorded against a different snapshot than `facts`: its fact ids mean
    /// nothing there, so no scope can honestly be published.
    pub fn from_run(
        record: &DetectorRunRecord,
        facts: &RevisionFacts,
        breadth: ReadBreadth,
    ) -> Option<Self> {
        (record.produced_snapshot == facts.snapshot).then(|| {
            Self::from_reads(
                record.observed_facts.iter().copied(),
                record.observed_truncated,
                facts,
                breadth,
            )
        })
    }

    /// The scope of an explicit list of read facts. A read whose fact the
    /// revision does not hold, or holds under no path, is counted in
    /// `unresolved_reads` instead of being dropped.
    pub fn from_reads(
        reads: impl IntoIterator<Item = FactId>,
        truncated: bool,
        facts: &RevisionFacts,
        breadth: ReadBreadth,
    ) -> Self {
        let attribution = Attribution::of(facts);
        let mut scoped = BTreeSet::new();
        let mut unresolved_reads = 0;
        for fact in reads {
            match attribution.of_fact(fact) {
                Some(read) => {
                    scoped.insert(read);
                }
                None => unresolved_reads += 1,
            }
        }
        Self {
            schema: READ_SCOPE_SCHEMA.to_string(),
            reads: scoped.into_iter().collect(),
            truncated,
            unresolved_reads,
            breadth,
        }
    }

    /// The exact scope of a query that read the named places directly, with
    /// no facts to attribute them through: nothing was dropped and nothing is
    /// left unattributed.
    pub fn from_scoped_reads(
        reads: impl IntoIterator<Item = ScopedRead>,
        breadth: ReadBreadth,
    ) -> Self {
        let reads: BTreeSet<ScopedRead> = reads.into_iter().collect();
        Self {
            schema: READ_SCOPE_SCHEMA.to_string(),
            reads: reads.into_iter().collect(),
            truncated: false,
            unresolved_reads: 0,
            breadth,
        }
    }

    /// Whether the list names every read: nothing was dropped and nothing
    /// was left unattributed.
    pub fn is_exact(&self) -> bool {
        !self.truncated && self.unresolved_reads == 0
    }

    /// Whether this reader knows the schema the scope was written under.
    pub fn is_supported(&self) -> bool {
        self.schema == READ_SCOPE_SCHEMA
    }
}

/// Where each fact of one revision lives: its path, and its symbol when a
/// `defines` fact names its subject.
struct Attribution<'a> {
    facts: HashMap<FactId, &'a Fact>,
    paths: HashMap<FactId, &'a str>,
    symbols: HashMap<EntityId, String>,
}

impl<'a> Attribution<'a> {
    fn of(revision: &'a RevisionFacts) -> Self {
        let facts = revision.facts.iter().map(|fact| (fact.id, fact)).collect();
        let paths = revision
            .path_facts
            .iter()
            .flat_map(|(path, ids)| ids.iter().map(move |id| (*id, path.as_str())))
            .collect();
        let symbols = revision
            .facts
            .iter()
            .filter(|fact| fact.predicate.as_str() == DEFINES_PREDICATE)
            .filter_map(|fact| match &fact.object {
                FactValue::Text(identity) => Identity::parse(identity)
                    .map(|identity| (fact.subject, identity.name().to_string())),
                _ => None,
            })
            .collect();
        Self {
            facts,
            paths,
            symbols,
        }
    }

    fn of_fact(&self, id: FactId) -> Option<ScopedRead> {
        let fact = self.facts.get(&id)?;
        let path = self.paths.get(&id)?;
        Some(ScopedRead {
            path: (*path).to_string(),
            symbol: self.symbols.get(&fact.subject).cloned(),
        })
    }
}

/// The facts that changed between two revisions, in the same `(path,
/// symbol)` terms a [`ReadScope`] speaks, plus what the facts cannot say.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChangedFacts {
    by_path: BTreeMap<String, BTreeSet<Option<String>>>,
    unattributed: usize,
    additions: usize,
    escalated: BTreeSet<String>,
    incomplete: bool,
}

impl ChangedFacts {
    /// The changed facts of a delta: removed facts are attributed through the
    /// base revision, added facts through the head. A change whose fact the
    /// revision does not place under a path is counted as unattributed, and
    /// no scope can then be proven valid.
    pub fn from_delta(delta: &FactDelta, base: &RevisionFacts, head: &RevisionFacts) -> Self {
        let mut changed = Self::default();
        let removed = Attribution::of(base);
        for change in &delta.removed {
            changed.attribute(removed.of_fact(change.fact_id));
        }
        let added = Attribution::of(head);
        for change in &delta.added {
            changed.additions += 1;
            changed.attribute(added.of_fact(change.fact_id));
        }
        changed
    }

    /// The changed facts of a transition, with the same escalations the
    /// reuse assessment applies: a path the SCM could not classify, or a
    /// manifest the change touched, is a change facts cannot express and
    /// leaves every scope unprovable; facts missing on either revision leave
    /// unchanged scopes unprovable too.
    ///
    /// # Errors
    /// The two revisions share a snapshot identity, so no delta exists.
    pub fn between(
        base: &ProducedRevision,
        head: &ProducedRevision,
        changes: &ChangeSet,
    ) -> Result<Self, ScmDeltaError> {
        let scm = delta_from_change_set(&base.facts, &head.facts, changes)?;
        let mut changed = Self::from_delta(&scm.delta, &base.facts, &head.facts);
        let manifests: BTreeSet<_> = base.manifests.union(&head.manifests).collect();
        let touched_manifests = changes
            .changes()
            .iter()
            .filter(|change| manifests.contains(change.path()))
            .map(|change| change.path());
        let unclassified = changes
            .changes()
            .iter()
            .filter(|change| matches!(change.kind(), ChangeKind::Unknown))
            .map(|change| change.path());
        for path in touched_manifests.chain(unclassified) {
            changed.escalate(path.as_str());
        }
        changed.incomplete = !base.coverage.is_complete() || !head.coverage.is_complete();
        Ok(changed)
    }

    fn attribute(&mut self, read: Option<ScopedRead>) {
        match read {
            Some(read) => {
                self.by_path
                    .entry(read.path)
                    .or_default()
                    .insert(read.symbol);
            }
            None => self.unattributed += 1,
        }
    }

    fn escalate(&mut self, path: &str) {
        self.escalated.insert(path.to_string());
        self.by_path
            .entry(path.to_string())
            .or_default()
            .insert(None);
    }

    /// Whether no fact changed and nothing is unknown about the change.
    pub fn is_empty(&self) -> bool {
        self.by_path.is_empty()
            && self.unattributed == 0
            && self.escalated.is_empty()
            && !self.incomplete
    }

    /// Whether something changed about the symbol `symbol` in `path`.
    ///
    /// A change is attributed to the symbol it is about, so this answers
    /// whether a name that was missing can now be found there.
    pub fn touches_symbol(&self, path: &str, symbol: &str) -> bool {
        self.by_path
            .get(path)
            .is_some_and(|symbols| symbols.iter().any(|name| name.as_deref() == Some(symbol)))
    }

    fn touches(&self, read: &ScopedRead) -> bool {
        self.by_path.get(&read.path).is_some_and(|symbols| {
            symbols
                .iter()
                .any(|symbol| read.overlaps(&read.path, symbol.as_deref()))
        })
    }
}

/// What a set of changed facts means for one published read scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadVerdict {
    /// Nothing the query read changed, and nothing unknown could have.
    Valid,
    /// A changed fact lives where the query read.
    Invalidated,
    /// The scope cannot prove either: the query must be answered again.
    Revalidate,
}

/// Why a scope was not `Valid`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum ReadReason {
    /// No scope was published for the query.
    ScopeAbsent,
    /// The scope was written under a schema this reader does not know.
    UnsupportedSchema {
        /// The schema the scope carries.
        found: String,
    },
    /// The recorder dropped reads: the scope is a lower bound.
    ScopeTruncated,
    /// Some reads could not be attributed to a path.
    UnresolvedReads {
        /// How many.
        count: usize,
    },
    /// Some changed facts could not be attributed to a path.
    UnattributedChanges {
        /// How many.
        count: usize,
    },
    /// A change facts cannot express touched this path (an unclassified
    /// change or a manifest).
    EscalatedPath {
        /// The path.
        path: String,
    },
    /// Facts are missing for some analyzable paths of either revision.
    IncompleteFacts,
    /// New facts appeared and the query read a whole collection.
    AggregateAddition,
    /// A changed fact lives where the query read.
    ReadChanged {
        /// The read that overlaps the change.
        read: ScopedRead,
    },
}

/// The answer for one scope: the verdict and every reason behind it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadResolution {
    /// The verdict.
    pub verdict: ReadVerdict,
    /// Every reason found, empty exactly when the verdict is `Valid`.
    pub reasons: Vec<ReadReason>,
}

impl ReadResolution {
    /// Whether the query must be answered again: anything but `Valid`.
    pub fn must_rerun(&self) -> bool {
        self.verdict != ReadVerdict::Valid
    }
}

/// Resolves which reads a set of changed facts invalidates.
///
/// An absent scope, one under an unknown schema, a truncated one and one with
/// unattributed reads are never `Valid`: they are the cases where the scope
/// does not name everything the query read, so nothing can be extrapolated
/// from it.
pub fn resolve_reads(scope: Option<&ReadScope>, changed: &ChangedFacts) -> ReadResolution {
    let Some(scope) = scope else {
        return revalidate(vec![ReadReason::ScopeAbsent]);
    };
    if !scope.is_supported() {
        return revalidate(vec![ReadReason::UnsupportedSchema {
            found: scope.schema.clone(),
        }]);
    }
    let hits = scope
        .reads
        .iter()
        .filter(|read| changed.touches(read))
        .map(|read| ReadReason::ReadChanged { read: read.clone() });
    let mut reasons: Vec<ReadReason> = hits.collect();
    let invalidated = !reasons.is_empty();
    reasons.extend(uncertainty(scope, changed));
    let verdict = match (invalidated, reasons.is_empty()) {
        (true, _) => ReadVerdict::Invalidated,
        (false, false) => ReadVerdict::Revalidate,
        (false, true) => ReadVerdict::Valid,
    };
    ReadResolution { verdict, reasons }
}

fn revalidate(reasons: Vec<ReadReason>) -> ReadResolution {
    ReadResolution {
        verdict: ReadVerdict::Revalidate,
        reasons,
    }
}

/// Everything that keeps an unhit scope from being proven valid.
fn uncertainty(scope: &ReadScope, changed: &ChangedFacts) -> Vec<ReadReason> {
    let mut reasons = Vec::new();
    if scope.truncated {
        reasons.push(ReadReason::ScopeTruncated);
    }
    if scope.unresolved_reads > 0 {
        reasons.push(ReadReason::UnresolvedReads {
            count: scope.unresolved_reads,
        });
    }
    if changed.unattributed > 0 {
        reasons.push(ReadReason::UnattributedChanges {
            count: changed.unattributed,
        });
    }
    reasons.extend(
        changed
            .escalated
            .iter()
            .map(|path| ReadReason::EscalatedPath { path: path.clone() }),
    );
    if changed.incomplete {
        reasons.push(ReadReason::IncompleteFacts);
    }
    if scope.breadth == ReadBreadth::Aggregate && changed.additions > 0 {
        reasons.push(ReadReason::AggregateAddition);
    }
    reasons
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::evidence_kernel::fact::{ProducerKind, ProvenanceRecord};
    use crate::domain::evidence_kernel::ids::SnapshotId;
    use crate::domain::evidence_kernel::relation::RelationKind;
    use crate::domain::evidence_kernel::semantic_diff::compute_fact_delta;
    use crate::domain::readset::{InMemoryReadSetRecorder, ReadSetConfig};
    use crate::domain::scm::PathKey;
    use crate::domain::value_objects::Provenance;

    fn path(text: &str) -> PathKey {
        PathKey::new(text).expect("path")
    }

    fn fact(id: u64, subject: u64, predicate: &str, object: FactValue, snap: u64) -> Fact {
        Fact::new(
            FactId(id),
            EntityId(subject),
            RelationKind::try_new(predicate).expect("predicate"),
            object,
            SnapshotId(snap),
            ProvenanceRecord::new(
                Provenance::Extracted,
                ProducerKind::DeterministicAnalyzer,
                None,
            ),
        )
        .expect("fact")
    }

    fn text(value: &str) -> FactValue {
        FactValue::Text(value.to_string())
    }

    /// One file `src/a.rs` with symbols `alpha` (subject 10) and `beta`
    /// (subject 20), each defined and each calling something; one file
    /// `src/b.rs` with `gamma` (subject 30). `alpha_callee` lets a test
    /// change what alpha calls without touching anything else.
    fn revision(snap: u64, alpha_callee: &str, with_delta: bool) -> RevisionFacts {
        let mut facts = vec![
            fact(1, 10, "core:defines", text("src/a.rs:alpha:1"), snap),
            fact(2, 10, "core:calls", text(alpha_callee), snap),
            fact(3, 20, "core:defines", text("src/a.rs:beta:5"), snap),
            fact(4, 20, "core:calls", text("other"), snap),
            fact(5, 30, "core:defines", text("src/b.rs:gamma:1"), snap),
            fact(6, 30, "core:calls", text("other"), snap),
        ];
        let mut path_facts: BTreeMap<PathKey, Vec<FactId>> = BTreeMap::new();
        path_facts.insert(path("src/a.rs"), (1..=4).map(FactId).collect());
        path_facts.insert(path("src/b.rs"), (5..=6).map(FactId).collect());
        if with_delta {
            facts.push(fact(7, 40, "core:defines", text("src/b.rs:delta:9"), snap));
            path_facts
                .get_mut(&path("src/b.rs"))
                .expect("b.rs")
                .push(FactId(7));
        }
        RevisionFacts {
            snapshot: SnapshotId(snap),
            facts,
            path_facts,
        }
    }

    fn changed(base: &RevisionFacts, head: &RevisionFacts) -> ChangedFacts {
        let delta = compute_fact_delta(
            base.snapshot,
            base.facts.iter().cloned(),
            head.snapshot,
            head.facts.iter().cloned(),
        )
        .expect("delta");
        ChangedFacts::from_delta(&delta, base, head)
    }

    fn scope_of(facts: &RevisionFacts, ids: &[u64], breadth: ReadBreadth) -> ReadScope {
        ReadScope::from_reads(ids.iter().copied().map(FactId), false, facts, breadth)
    }

    fn read(path: &str, symbol: Option<&str>) -> ScopedRead {
        ScopedRead {
            path: path.to_string(),
            symbol: symbol.map(str::to_string),
        }
    }

    #[test]
    fn a_read_set_publishes_the_files_and_symbols_it_read() {
        let base = revision(1, "helper", false);
        let mut recorder = InMemoryReadSetRecorder::new(ReadSetConfig { max_records: None });
        for id in [2, 6, 2, 4] {
            recorder.record(FactId(id)).expect("record");
        }
        let scope = ReadScope::from_read_set(&recorder.finalize(), &base, ReadBreadth::Pointwise);

        assert_eq!(scope.schema, READ_SCOPE_SCHEMA);
        assert_eq!(
            scope.reads,
            vec![
                read("src/a.rs", Some("alpha")),
                read("src/a.rs", Some("beta")),
                read("src/b.rs", Some("gamma")),
            ]
        );
        assert!(scope.is_exact());
    }

    #[test]
    fn a_fact_no_symbol_owns_is_a_whole_file_read() {
        let mut base = revision(1, "helper", false);
        base.facts
            .push(fact(8, 99, "core:imports", text("std::fmt"), 1));
        base.path_facts
            .get_mut(&path("src/a.rs"))
            .expect("a.rs")
            .push(FactId(8));

        let scope = scope_of(&base, &[8], ReadBreadth::Pointwise);

        assert_eq!(scope.reads, vec![read("src/a.rs", None)]);
    }

    #[test]
    fn the_truncation_mark_of_the_read_set_is_published() {
        let base = revision(1, "helper", false);
        let mut recorder = InMemoryReadSetRecorder::new(ReadSetConfig {
            max_records: std::num::NonZeroUsize::new(1),
        });
        for id in [2, 4, 6] {
            recorder.record(FactId(id)).expect("record");
        }
        let scope = ReadScope::from_read_set(&recorder.finalize(), &base, ReadBreadth::Pointwise);

        assert!(scope.truncated);
        assert!(!scope.is_exact());
    }

    #[test]
    fn a_read_the_revision_does_not_hold_is_counted_not_dropped() {
        let base = revision(1, "helper", false);

        let scope = scope_of(&base, &[2, 4040], ReadBreadth::Pointwise);

        assert_eq!(scope.unresolved_reads, 1);
        assert!(!scope.is_exact());
    }

    #[test]
    fn a_run_recorded_on_another_snapshot_publishes_no_scope() {
        use crate::domain::findings::detector_reuse::{DetectorRunIdentity, DetectorRunRecord};
        use crate::domain::findings::digest::DetectorDigest;
        use crate::domain::findings::{
            AnalysisCapability, DetectorAuthority, DetectorDigests, DetectorId,
        };
        let digest = |s: &str| DetectorDigest::from_content(s);
        let record = |snapshot: u64| DetectorRunRecord {
            identity: DetectorRunIdentity {
                detector_id: DetectorId::new("security.demo").expect("id"),
                version: "1".to_string(),
                authority: DetectorAuthority::Gated,
                digests: DetectorDigests {
                    logic: digest("l"),
                    policy: digest("p"),
                    semantic: digest("s"),
                    instance: digest("i"),
                },
                effective_capabilities: [AnalysisCapability::AstPattern].into_iter().collect(),
                parser: digest("parser"),
                sanitizer: digest("sanitizer"),
            },
            observed_facts: [FactId(2)].into_iter().collect(),
            observed_truncated: true,
            produced_snapshot: SnapshotId(snapshot),
            findings: Vec::new(),
        };
        let base = revision(1, "helper", false);

        assert!(ReadScope::from_run(&record(2), &base, ReadBreadth::Pointwise).is_none());
        let scope = ReadScope::from_run(&record(1), &base, ReadBreadth::Pointwise).expect("scope");
        assert!(scope.truncated, "the run's truncation mark carries over");
        assert_eq!(scope.reads, vec![read("src/a.rs", Some("alpha"))]);
    }

    #[test]
    fn a_change_to_what_was_read_invalidates_the_read() {
        let base = revision(1, "helper", false);
        let head = revision(2, "renamed", false);
        let scope = scope_of(&base, &[2], ReadBreadth::Pointwise);

        let resolution = resolve_reads(Some(&scope), &changed(&base, &head));

        assert_eq!(resolution.verdict, ReadVerdict::Invalidated);
        assert!(resolution.must_rerun());
        assert_eq!(
            resolution.reasons,
            vec![ReadReason::ReadChanged {
                read: read("src/a.rs", Some("alpha"))
            }]
        );
    }

    #[test]
    fn a_change_elsewhere_in_the_same_file_does_not_invalidate_a_symbol_read() {
        let base = revision(1, "helper", false);
        let head = revision(2, "renamed", false);
        let scope = scope_of(&base, &[4], ReadBreadth::Pointwise);

        let resolution = resolve_reads(Some(&scope), &changed(&base, &head));

        assert_eq!(resolution.verdict, ReadVerdict::Valid);
        assert!(resolution.reasons.is_empty());
        assert!(!resolution.must_rerun());
    }

    #[test]
    fn a_whole_file_read_is_invalidated_by_any_change_in_the_file() {
        let mut base = revision(1, "helper", false);
        base.facts
            .push(fact(8, 99, "core:imports", text("std::fmt"), 1));
        base.path_facts
            .get_mut(&path("src/a.rs"))
            .expect("a.rs")
            .push(FactId(8));
        let head = revision(2, "renamed", false);
        let scope = scope_of(&base, &[8], ReadBreadth::Pointwise);

        let resolution = resolve_reads(Some(&scope), &changed(&base, &head));

        assert_eq!(resolution.verdict, ReadVerdict::Invalidated);
    }

    #[test]
    fn a_change_in_another_file_does_not_invalidate_a_pointwise_read() {
        let base = revision(1, "helper", false);
        let head = revision(2, "helper", true);
        let scope = scope_of(&base, &[2], ReadBreadth::Pointwise);

        let resolution = resolve_reads(Some(&scope), &changed(&base, &head));

        assert_eq!(resolution.verdict, ReadVerdict::Valid);
    }

    #[test]
    fn a_new_fact_makes_an_aggregate_read_revalidate() {
        let base = revision(1, "helper", false);
        let head = revision(2, "helper", true);
        let scope = scope_of(&base, &[2], ReadBreadth::Aggregate);

        let resolution = resolve_reads(Some(&scope), &changed(&base, &head));

        assert_eq!(resolution.verdict, ReadVerdict::Revalidate);
        assert_eq!(resolution.reasons, vec![ReadReason::AggregateAddition]);
    }

    #[test]
    fn an_absent_scope_forces_revalidation() {
        let resolution = resolve_reads(None, &ChangedFacts::default());

        assert_eq!(resolution.verdict, ReadVerdict::Revalidate);
        assert_eq!(resolution.reasons, vec![ReadReason::ScopeAbsent]);
    }

    #[test]
    fn a_truncated_scope_is_never_valid_even_when_nothing_changed() {
        let base = revision(1, "helper", false);
        let mut scope = scope_of(&base, &[2], ReadBreadth::Pointwise);
        scope.truncated = true;

        let resolution = resolve_reads(Some(&scope), &ChangedFacts::default());

        assert_eq!(resolution.verdict, ReadVerdict::Revalidate);
        assert_eq!(resolution.reasons, vec![ReadReason::ScopeTruncated]);
    }

    #[test]
    fn a_scope_with_unattributed_reads_is_never_valid() {
        let base = revision(1, "helper", false);
        let scope = scope_of(&base, &[2, 4040], ReadBreadth::Pointwise);

        let resolution = resolve_reads(Some(&scope), &ChangedFacts::default());

        assert_eq!(resolution.verdict, ReadVerdict::Revalidate);
        assert_eq!(
            resolution.reasons,
            vec![ReadReason::UnresolvedReads { count: 1 }]
        );
    }

    #[test]
    fn a_scope_under_an_unknown_schema_is_not_trusted() {
        let base = revision(1, "helper", false);
        let mut scope = scope_of(&base, &[2], ReadBreadth::Pointwise);
        scope.schema = "cognicode.read-scope/v9".to_string();

        let resolution = resolve_reads(Some(&scope), &ChangedFacts::default());

        assert_eq!(resolution.verdict, ReadVerdict::Revalidate);
        assert_eq!(
            resolution.reasons,
            vec![ReadReason::UnsupportedSchema {
                found: "cognicode.read-scope/v9".to_string()
            }]
        );
    }

    #[test]
    fn a_change_that_no_path_owns_leaves_every_scope_unprovable() {
        let base = revision(1, "helper", false);
        let mut head = revision(2, "helper", true);
        head.path_facts
            .get_mut(&path("src/b.rs"))
            .expect("b.rs")
            .retain(|id| *id != FactId(7));
        let scope = scope_of(&base, &[2], ReadBreadth::Pointwise);

        let resolution = resolve_reads(Some(&scope), &changed(&base, &head));

        assert_eq!(resolution.verdict, ReadVerdict::Revalidate);
        assert_eq!(
            resolution.reasons,
            vec![ReadReason::UnattributedChanges { count: 1 }]
        );
    }

    #[test]
    fn an_escalated_path_invalidates_what_it_read_and_unsettles_the_rest() {
        let base = revision(1, "helper", false);
        let mut changes = ChangedFacts::default();
        changes.escalate("src/b.rs");
        let in_b = scope_of(&base, &[6], ReadBreadth::Pointwise);
        let in_a = scope_of(&base, &[2], ReadBreadth::Pointwise);

        assert_eq!(
            resolve_reads(Some(&in_b), &changes).verdict,
            ReadVerdict::Invalidated
        );
        let elsewhere = resolve_reads(Some(&in_a), &changes);
        assert_eq!(elsewhere.verdict, ReadVerdict::Revalidate);
        assert_eq!(
            elsewhere.reasons,
            vec![ReadReason::EscalatedPath {
                path: "src/b.rs".to_string()
            }]
        );
    }

    #[test]
    fn missing_facts_make_an_unchanged_scope_unprovable() {
        let base = revision(1, "helper", false);
        let changes = ChangedFacts {
            incomplete: true,
            ..ChangedFacts::default()
        };
        let scope = scope_of(&base, &[2], ReadBreadth::Pointwise);

        let resolution = resolve_reads(Some(&scope), &changes);

        assert_eq!(resolution.verdict, ReadVerdict::Revalidate);
        assert_eq!(resolution.reasons, vec![ReadReason::IncompleteFacts]);
    }

    #[test]
    fn a_hit_is_reported_even_when_the_scope_is_also_truncated() {
        let base = revision(1, "helper", false);
        let head = revision(2, "renamed", false);
        let mut scope = scope_of(&base, &[2], ReadBreadth::Pointwise);
        scope.truncated = true;

        let resolution = resolve_reads(Some(&scope), &changed(&base, &head));

        assert_eq!(resolution.verdict, ReadVerdict::Invalidated);
        assert!(resolution.reasons.contains(&ReadReason::ScopeTruncated));
    }

    #[test]
    fn the_wire_form_round_trips_and_pins_the_field_names() {
        let base = revision(1, "helper", false);
        let scope = scope_of(&base, &[2, 5], ReadBreadth::Pointwise);

        let json = serde_json::to_value(&scope).expect("serialize");

        assert_eq!(
            json,
            serde_json::json!({
                "schema": "cognicode.read-scope/v1",
                "reads": [
                    {"path": "src/a.rs", "symbol": "alpha"},
                    {"path": "src/b.rs", "symbol": "gamma"},
                ],
                "truncated": false,
                "unresolved_reads": 0,
                "breadth": "pointwise",
            })
        );
        let back: ReadScope = serde_json::from_value(json).expect("deserialize");
        assert_eq!(back, scope);
    }

    #[test]
    fn a_reader_ignores_unknown_fields_and_defaults_missing_ones_to_the_wider_case() {
        let wire = serde_json::json!({
            "schema": "cognicode.read-scope/v1",
            "reads": [{"path": "src/a.rs"}],
            "truncated": false,
            "added_by_a_later_minor": 7,
        });

        let scope: ReadScope = serde_json::from_value(wire).expect("tolerant reader");

        assert_eq!(scope.breadth, ReadBreadth::Aggregate);
        assert_eq!(scope.unresolved_reads, 0);
        assert_eq!(scope.reads, vec![read("src/a.rs", None)]);
    }

    #[test]
    fn a_scope_without_a_schema_does_not_parse() {
        let wire = serde_json::json!({"reads": [], "truncated": false});

        assert!(serde_json::from_value::<ReadScope>(wire).is_err());
    }

    // ----- over the real revision producer ------------------------------

    use super::super::revision_facts::{FactsLimits, produce_revision_facts};
    use crate::domain::scm::fake::FakeSource;
    use crate::domain::scm::ports::RevisionReader;
    use crate::domain::scm::{ChangeReader, ProviderCapabilities, SourceRef, SourceRevision};

    type Files<'a> = &'a [(&'a str, &'a [u8])];

    const ALPHA_V1: &[u8] = b"pub fn alpha() { first(); }\n";
    const ALPHA_V2: &[u8] = b"pub fn alpha() { second(); }\n";
    const BETA: &[u8] = b"pub fn beta() { helper(); }\n";

    fn transition(
        base_files: Files,
        head_files: Files,
    ) -> (ProducedRevision, ProducedRevision, ChangeSet) {
        let fake = FakeSource::new(
            "fake",
            ProviderCapabilities {
                ancestor_queries: true,
                ..ProviderCapabilities::NONE
            },
        )
        .with_revision("main", &[], base_files)
        .with_revision("feature", &["main"], head_files);
        let source = SourceRef::new("fake", "demo").expect("source");
        let produce = |name: &str| {
            let revision: SourceRevision = fake.resolve(&source, name).expect("revision");
            produce_revision_facts(&fake, &revision, None, &FactsLimits::default()).expect("facts")
        };
        let (base, head) = (produce("main"), produce("feature"));
        let changes = fake
            .changes_between(&base.revision, &head.revision)
            .expect("changes");
        (base, head, changes)
    }

    fn reading_file(base: &ProducedRevision, file: &str) -> ReadScope {
        let ids = base.facts.path_facts[&path(file)].iter().copied();
        ReadScope::from_reads(ids, false, &base.facts, ReadBreadth::Pointwise)
    }

    #[test]
    fn a_scope_over_real_facts_names_the_symbols_of_the_file() {
        let (base, _, _) = transition(&[("src/a.rs", ALPHA_V1)], &[("src/a.rs", ALPHA_V2)]);

        let scope = reading_file(&base, "src/a.rs");

        assert!(scope.reads.contains(&read("src/a.rs", Some("alpha"))));
        assert!(scope.reads.iter().all(|r| r.path == "src/a.rs"));
        assert!(scope.is_exact());
    }

    #[test]
    fn a_one_file_edit_invalidates_that_file_and_proves_the_other_valid() {
        let files_base: Files = &[("src/a.rs", ALPHA_V1), ("src/b.rs", BETA)];
        let files_head: Files = &[("src/a.rs", ALPHA_V2), ("src/b.rs", BETA)];
        let (base, head, changes) = transition(files_base, files_head);
        let changed = ChangedFacts::between(&base, &head, &changes).expect("changed");

        let in_a = resolve_reads(Some(&reading_file(&base, "src/a.rs")), &changed);
        let in_b = resolve_reads(Some(&reading_file(&base, "src/b.rs")), &changed);

        assert_eq!(in_a.verdict, ReadVerdict::Invalidated);
        assert_eq!(in_b.verdict, ReadVerdict::Valid);
    }

    #[test]
    fn a_manifest_change_leaves_every_scope_unproven() {
        let files_base: Files = &[("src/b.rs", BETA), ("Cargo.toml", b"[package]\n")];
        let files_head: Files = &[
            ("src/b.rs", BETA),
            ("Cargo.toml", b"[package]\nname=\"x\"\n"),
        ];
        let (base, head, changes) = transition(files_base, files_head);
        let changed = ChangedFacts::between(&base, &head, &changes).expect("changed");

        let resolution = resolve_reads(Some(&reading_file(&base, "src/b.rs")), &changed);

        assert_eq!(resolution.verdict, ReadVerdict::Revalidate);
        assert!(resolution.reasons.contains(&ReadReason::EscalatedPath {
            path: "Cargo.toml".to_string()
        }));
    }

    #[test]
    fn missing_facts_on_either_revision_leave_an_untouched_scope_unproven() {
        let files_base: Files = &[("src/b.rs", BETA), ("src/bad.rs", &[0xff, 0x00])];
        let files_head: Files = &[("src/b.rs", BETA), ("src/bad.rs", &[0xfe, 0x00])];
        let (base, head, changes) = transition(files_base, files_head);
        let changed = ChangedFacts::between(&base, &head, &changes).expect("changed");

        let resolution = resolve_reads(Some(&reading_file(&base, "src/b.rs")), &changed);

        assert_eq!(resolution.verdict, ReadVerdict::Revalidate);
        assert!(resolution.reasons.contains(&ReadReason::IncompleteFacts));
    }
}
