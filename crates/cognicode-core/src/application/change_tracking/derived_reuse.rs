//! Derived query results: what they say, and when what they say did not
//! change.
//!
//! ## The two things a derived answer carries
//!
//! A deterministic query over a revision answers with facts. The answer
//! depends on two different things, and the product used to conflate them:
//!
//! - **what the answer says** — the relations it exposes and the places it
//!   names them;
//! - **where the answer was computed** — the snapshot, the producer and the
//!   files it read.
//!
//! A line that moves changes the second without changing the first: the
//! `core:defines` fact of a symbol carries its line, so editing the code
//! above a function changes the fact that names it. A fact-level diff
//! cannot tell that apart from a change in what the function does, so the
//! work that read the symbol is scheduled again for an answer that would
//! come out the same. This module is where the two are told apart: a
//! derived answer carries an **identity** — a digest over what it says,
//! with no line in it — and the places it names are **locations**, which
//! are re-anchored to the new snapshot when the answer is reused.
//!
//! ## What the identity is made of
//!
//! The identity of a thing an answer names is the product's own notion of
//! a symbol's identity across snapshots: `(file, name, kind)`. The line is
//! not part of it — the line is where the thing was found, not what it is
//! — and the kind is: a symbol that stops being a function and starts
//! being a struct is a different thing, even when its name and its place
//! did not move.
//!
//! ## What an answer depends on
//!
//! The [`ReadScope`] of the reads it made, plus the definitions of the
//! names it mentions: a call that names a symbol depends on where that
//! symbol is defined, wherever it is. A name the query looked for and did
//! not find is an **absence**, and an absence is a dependency like any
//! other: the thing appearing later changes the answer.
//!
//! ## When an answer may be reused
//!
//! Only when nothing it depends on changed. A changed path, a name that
//! appeared where the answer looked and found nothing, a dependency set
//! that moved, a different producer or a different coverage class forbid
//! the reuse, and each of them is named. Uncertainty never collapses into
//! a reuse: a scope that cannot prove what it read proves nothing, and an
//! answer built on it is computed again.
//!
//! ## Nothing here is a runtime
//!
//! The module is pure: no I/O, no clock, no cache, no daemon. The patterns
//! it follows — a memo that keeps its value when the recomputed one is
//! equal, an index maintained by the difference — are patterns, and the
//! product keeps its own storage.

use std::collections::{BTreeMap, BTreeSet};

use crate::domain::evidence_kernel::continuity::view::SnapshotEntityView;
use crate::domain::evidence_kernel::fact::{Fact, FactValue};
use crate::domain::evidence_kernel::identity::Identity;
use crate::domain::evidence_kernel::ids::{EntityId, FactId, SnapshotId};
use crate::domain::findings::digest::DigestStream;
use crate::domain::readset::{InMemoryReadSetRecorder, ReadSetConfig};
use crate::domain::scm::PathKey;

use super::planner::{WorkDisposition, WorkId};
use super::read_scope::{ReadBreadth, ReadScope};
use super::scm_delta::RevisionFacts;

/// The predicate whose fact names a symbol: `(symbol, defines, identity)`.
const DEFINES_PREDICATE: &str = "core:defines";

/// The identity of a thing a derived answer names.
///
/// The file it lives in, its declared name and its kind. The line is not
/// part of it: the line is where the thing was found, not what it is, and
/// it moves when unrelated code above it is edited. The kind is part of
/// it: a symbol that stops being a function and starts being a struct is
/// a different thing, even when its name and its place did not move.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DerivedSubject {
    /// The repository-relative path of the file the thing lives in.
    pub file: String,
    /// The declared name of the thing, without its line.
    pub name: String,
    /// The kind of the thing, as the producer named it.
    pub kind: String,
}

/// One relation a derived answer exposes: what a thing does to something.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct DerivedRelation {
    /// The subject: the thing the relation is about.
    pub subject: DerivedSubject,
    /// The relation itself: the predicate that holds.
    pub relation: String,
    /// The object: what the relation says about the subject.
    pub object: String,
}

/// Where a subject was when the answer was computed.
///
/// The location is not the identity: it moves when the code above it
/// changes, and an answer that kept a stale line would lie about where
/// the thing is.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct DerivedLocation {
    /// The repository-relative path of the file.
    pub file: String,
    /// The declared name of the thing.
    pub name: String,
    /// The line it was on, when the producer named one. The base is the
    /// producer's own: the facts side is one-based.
    pub line: Option<u32>,
}

/// A name the query looked for and did not find.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct DerivedAbsence {
    /// The path the name was looked for in.
    pub path: PathKey,
    /// The name that was not there.
    pub name: String,
}

/// The digest of what a derived answer says.
///
/// The identity of an answer is what its dependents rely on: two answers
/// with the same identity are the same answer, even when they were
/// computed on different snapshots.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DerivedIdentity(String);

impl DerivedIdentity {
    /// The identity of the relations an answer exposes.
    ///
    /// The relations are digested in canonical order, so two answers that
    /// say the same thing have the same identity whatever order they were
    /// produced in. The locations are not part of it: they say where the
    /// answer was computed, not what it says.
    pub fn of(relations: &[DerivedRelation]) -> Self {
        let mut canonical: Vec<&DerivedRelation> = relations.iter().collect();
        canonical.sort();
        let mut stream = DigestStream::new();
        for relation in canonical {
            stream
                .update(relation.subject.file.as_bytes())
                .update(b"\0")
                .update(relation.subject.name.as_bytes())
                .update(b"\0")
                .update(relation.subject.kind.as_bytes())
                .update(b"\0")
                .update(relation.relation.as_bytes())
                .update(b"\0")
                .update(relation.object.as_bytes())
                .update(b"\0");
        }
        Self(stream.finish_hex())
    }

    /// The digest, as it is written down.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A derived answer: what it says, and what it depends on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivedAnswer {
    /// The identity its dependents rely on.
    pub identity: DerivedIdentity,
    /// The relations the answer exposes, in canonical order.
    pub relations: Vec<DerivedRelation>,
    /// Where the subjects were when the answer was computed.
    pub locations: Vec<DerivedLocation>,
    /// What the answer read.
    pub scope: ReadScope,
    /// The names the query looked for and did not find.
    pub absences: Vec<DerivedAbsence>,
    /// The facts the answer read whose subject the revision does not
    /// define: the answer cannot say what they are about, so it is a lower
    /// bound of what the query found.
    pub unattributed: usize,
    /// The snapshot the answer was computed on.
    pub produced_in: SnapshotId,
    /// The producer that produced the facts the answer read.
    pub producer: String,
    /// Whether the coverage of the run that produced it was complete.
    pub coverage_complete: bool,
}

impl DerivedAnswer {
    /// Whether two answers say the same thing.
    ///
    /// The identity is the content; the producer and the coverage class are
    /// the conditions under which that content means the same thing. An
    /// answer computed by another producer, or over an incomplete coverage,
    /// is not the same answer even when it says the same words.
    pub fn observably_equivalent(&self, other: &DerivedAnswer) -> bool {
        self.identity == other.identity
            && self.producer == other.producer
            && self.coverage_complete == other.coverage_complete
    }

    /// The answer as it stands on the new snapshot.
    ///
    /// The identity is kept — that is the point: a recomputed answer that
    /// says the same thing is the same answer, and the dependents that hold
    /// it do not have to be answered again. The locations are the ones the
    /// new snapshot gives, and the scope is the recomputed one: the
    /// recomputation may have read more than the stored answer did, and a
    /// stored answer that kept the narrower scope would miss the next
    /// change under the new dependency.
    pub fn reanchored(&self, recomputed: &DerivedAnswer) -> DerivedAnswer {
        let mut absences = self.absences.clone();
        for absence in &recomputed.absences {
            if !absences.contains(absence) {
                absences.push(absence.clone());
            }
        }
        absences.sort();
        DerivedAnswer {
            identity: self.identity.clone(),
            relations: self.relations.clone(),
            locations: recomputed.locations.clone(),
            scope: recomputed.scope.clone(),
            absences,
            unattributed: recomputed.unattributed,
            produced_in: recomputed.produced_in,
            producer: recomputed.producer.clone(),
            coverage_complete: recomputed.coverage_complete,
        }
    }

    /// The names the answer mentions but does not define.
    ///
    /// The definition of a subject is not a mention: it is the subject
    /// itself. Everything else the answer says about — a callee, a type —
    /// is a name that resolves somewhere, and the answer depends on where.
    pub fn mentioned_names(&self) -> BTreeSet<String> {
        self.relations
            .iter()
            .filter(|relation| relation.relation != DEFINES_PREDICATE)
            .map(|relation| relation.object.clone())
            .collect()
    }

    /// The paths the answer read.
    pub fn read_paths(&self) -> BTreeSet<PathKey> {
        self.scope
            .reads
            .iter()
            .filter_map(|read| PathKey::new(&read.path).ok())
            .collect()
    }

    /// The places the answer's scope says a name is defined.
    pub fn definitions_of(&self, name: &str) -> BTreeSet<PathKey> {
        self.scope
            .reads
            .iter()
            .filter(|read| read.symbol.as_deref() == Some(name))
            .filter_map(|read| PathKey::new(&read.path).ok())
            .collect()
    }
}

/// What changed since the stored answer was produced.
///
/// The paths that changed and the paths that were added are two different
/// things: a path that was added cannot be one the answer read, but a
/// collection read can still see it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DerivedChange {
    /// The paths whose content changed.
    pub changed: BTreeSet<PathKey>,
    /// The paths that were added.
    pub added: BTreeSet<PathKey>,
}

impl DerivedChange {
    /// A change of the paths that changed, and of none that were added.
    pub fn of(paths: &[PathKey]) -> Self {
        Self {
            changed: paths.iter().cloned().collect(),
            added: BTreeSet::new(),
        }
    }
}

/// Why a derived answer was not reused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DerivedReuseReason {
    /// A path the answer read changed.
    ReadPathChanged {
        /// The path.
        path: PathKey,
    },
    /// A name the answer looked for and did not find is now there.
    AbsenceViolated {
        /// The path the name was looked for in.
        path: PathKey,
        /// The name.
        name: String,
    },
    /// The set of places a mentioned name is defined changed: the answer
    /// depends on where the name resolves, and that moved.
    DependencyMoved {
        /// The name.
        name: String,
    },
    /// The producer that produced the facts the answer read changed.
    ProducerChanged {
        /// The producer of the stored answer.
        before: String,
        /// The producer of the new run.
        after: String,
    },
    /// The coverage of the run changed: the two answers do not cover the
    /// same ground, so they are not the same answer.
    CoverageChanged,
    /// The scope of the answer does not name everything it read: it is a
    /// lower bound, so nothing can be proven from it.
    ScopeIncomplete,
    /// A path was added and the answer read a whole collection: a new
    /// fact anywhere could change what a collection says.
    CollectionAddition {
        /// The path that was added.
        path: PathKey,
    },
    /// Some facts the answer read name a subject the revision does not
    /// define: the answer cannot say what they are about.
    UnattributedReads {
        /// How many.
        count: usize,
    },
}

/// The verdict on reusing a stored derived answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DerivedReuseDecision {
    /// The answer still holds: it can be reused as it is.
    Reusable,
    /// It does not: it has to be computed again, and this is why.
    Recompute {
        /// Every reason found.
        reasons: Vec<DerivedReuseReason>,
    },
}

impl DerivedReuseDecision {
    /// Whether the answer can be reused.
    pub fn is_reusable(&self) -> bool {
        matches!(self, DerivedReuseDecision::Reusable)
    }

    /// The reasons the answer was not reused, empty when it can be.
    pub fn reasons(&self) -> &[DerivedReuseReason] {
        match self {
            DerivedReuseDecision::Reusable => &[],
            DerivedReuseDecision::Recompute { reasons } => reasons,
        }
    }
}

/// A deterministic query over the facts of one revision.
///
/// The query reads the facts of one path and looks for a set of names
/// there. A name the revision does not define in that path is an absence
/// the query records: the thing appearing later changes the answer.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct FactsQuery {
    /// The path the query reads.
    pub path: PathKey,
    /// The names the query looks for in that path.
    pub looks_for: Vec<String>,
    /// How wide the read was: a query that read a whole collection sees
    /// what appears anywhere, and one that read exactly the facts it
    /// names does not.
    pub breadth: ReadBreadth,
}

/// The answer of one query over one revision.
///
/// The facts of the queried path are projected onto what a consumer can
/// observe: the relations between the things the path defines, with the
/// line of each one kept apart from its identity. The scope of the answer
/// covers the facts of the path and the definitions of every name the
/// answer mentions, wherever those definitions live.
pub fn answer_of(
    revision: &RevisionFacts,
    query: &FactsQuery,
    producer: &str,
    coverage_complete: bool,
) -> DerivedAnswer {
    let view = SnapshotEntityView::from_facts(&revision.facts, revision.snapshot);
    let defines = DefinesIndex::of(&revision.facts);

    // The facts of the queried path are what the query reads.
    let mut read = InMemoryReadSetRecorder::new(ReadSetConfig { max_records: None });
    let facts_of_path = revision
        .path_facts
        .get(&query.path)
        .cloned()
        .unwrap_or_default();
    for fact in &facts_of_path {
        let _ = read.record(*fact);
    }

    let mut relations: Vec<DerivedRelation> = Vec::new();
    let mut subjects: BTreeMap<EntityId, DerivedSubject> = BTreeMap::new();
    let mut unresolved = 0usize;
    for fact_id in &facts_of_path {
        let Some(fact) = revision.facts.iter().find(|fact| fact.id == *fact_id) else {
            unresolved += 1;
            continue;
        };
        let Some(subject) = subject_of(&view, fact.subject) else {
            unresolved += 1;
            continue;
        };
        let object = match fact.predicate.as_str() {
            DEFINES_PREDICATE => subject.name.clone(),
            _ => match &fact.object {
                FactValue::Text(text) => text.clone(),
                other => format!("{other:?}"),
            },
        };
        subjects.insert(fact.subject, subject.clone());
        relations.push(DerivedRelation {
            subject,
            relation: fact.predicate.as_str().to_string(),
            object,
        });
    }
    relations.sort();
    relations.dedup();

    // The locations of the things the answer names.
    let mut locations: Vec<DerivedLocation> = subjects
        .iter()
        .filter_map(|(entity, subject)| {
            let entity = view.entities.get(entity)?;
            let identity = Identity::parse(&entity.fqn)?;
            Some(DerivedLocation {
                file: subject.file.clone(),
                name: subject.name.clone(),
                line: identity.line(),
            })
        })
        .collect();
    locations.sort();
    locations.dedup();

    // The indirect dependency: a call that names a symbol depends on where
    // that symbol is defined, and the definition may live in another file.
    let mentioned: BTreeSet<String> = relations
        .iter()
        .filter(|relation| relation.relation != DEFINES_PREDICATE)
        .map(|relation| relation.object.clone())
        .collect();
    for name in &mentioned {
        for entity in view.entities.values().filter(|entity| &entity.name == name) {
            if let Some(fact_id) = defines.fact_of(entity.entity) {
                let _ = read.record(fact_id);
            }
        }
    }

    let scope = ReadScope::from_read_set(&read.finalize(), revision, query.breadth);

    // The names the query looked for and did not find.
    let mut absences: Vec<DerivedAbsence> = Vec::new();
    for name in &query.looks_for {
        let defined = view.entities.values().any(|entity| {
            entity.name == *name && identity_file(&entity.fqn) == query.path.as_str()
        });
        if !defined {
            absences.push(DerivedAbsence {
                path: query.path.clone(),
                name: name.clone(),
            });
        }
    }
    absences.sort();

    DerivedAnswer {
        identity: DerivedIdentity::of(&relations),
        relations,
        locations,
        scope,
        absences,
        unattributed: unresolved,
        produced_in: revision.snapshot,
        producer: producer.to_string(),
        coverage_complete,
    }
}

/// The thing a fact is about, or `None` when the fact names an entity the
/// revision does not define (an unattributed read).
fn subject_of(view: &SnapshotEntityView, subject: EntityId) -> Option<DerivedSubject> {
    let entity = view.entities.get(&subject)?;
    let identity = Identity::parse(&entity.fqn)?;
    Some(DerivedSubject {
        file: identity.file().to_string(),
        name: identity.name().to_string(),
        kind: entity.kind.clone(),
    })
}

/// The file an identity string lives in.
fn identity_file(fqn: &str) -> String {
    Identity::parse(fqn)
        .map(|identity| identity.file().to_string())
        .unwrap_or_default()
}

/// The `core:defines` facts of a revision, indexed by the entity they name.
struct DefinesIndex<'a> {
    facts: BTreeMap<EntityId, (FactId, &'a str)>,
}

impl<'a> DefinesIndex<'a> {
    fn of(facts: &'a [Fact]) -> Self {
        let mut index = BTreeMap::new();
        for fact in facts {
            if fact.predicate.as_str() != DEFINES_PREDICATE {
                continue;
            }
            let FactValue::Text(identity) = &fact.object else {
                continue;
            };
            index
                .entry(fact.subject)
                .or_insert((fact.id, identity.as_str()));
        }
        Self { facts: index }
    }

    fn fact_of(&self, entity: EntityId) -> Option<FactId> {
        self.facts.get(&entity).map(|(id, _)| *id)
    }
}

/// Decides whether a stored derived answer can be reused on a new snapshot.
///
/// `change` says what changed since the answer was produced; `head` is the
/// entity view of the new snapshot, which says where the names the answer
/// mentions are defined now. The answer is reusable only when nothing it
/// depends on moved: a changed path it read, a name that appeared where it
/// looked and found nothing, a dependency that moved, a new path a
/// collection read can see, another producer or another coverage class.
pub fn reuse_decision(
    stored: &DerivedAnswer,
    change: &DerivedChange,
    head: &SnapshotEntityView,
    head_producer: &str,
    head_coverage_complete: bool,
) -> DerivedReuseDecision {
    let mut reasons: Vec<DerivedReuseReason> = Vec::new();

    if stored.producer != head_producer {
        reasons.push(DerivedReuseReason::ProducerChanged {
            before: stored.producer.clone(),
            after: head_producer.to_string(),
        });
    }
    if stored.coverage_complete != head_coverage_complete {
        reasons.push(DerivedReuseReason::CoverageChanged);
    }
    if !stored.scope.is_exact() {
        reasons.push(DerivedReuseReason::ScopeIncomplete);
    }
    if stored.unattributed > 0 {
        reasons.push(DerivedReuseReason::UnattributedReads {
            count: stored.unattributed,
        });
    }

    let read_paths = stored.read_paths();
    for path in &change.changed {
        if read_paths.contains(path) {
            reasons.push(DerivedReuseReason::ReadPathChanged { path: path.clone() });
        }
    }

    // A path that was added cannot be one the answer read, but a
    // collection read sees what appears anywhere.
    if stored.scope.breadth == ReadBreadth::Aggregate {
        for path in &change.added {
            reasons.push(DerivedReuseReason::CollectionAddition { path: path.clone() });
        }
    }

    // The names the answer looked for and did not find: a name that is now
    // defined in the path where it was missing changes the answer.
    for absence in &stored.absences {
        if change.changed.contains(&absence.path)
            && head.entities.values().any(|entity| {
                entity.name == absence.name && identity_file(&entity.fqn) == absence.path.as_str()
            })
        {
            reasons.push(DerivedReuseReason::AbsenceViolated {
                path: absence.path.clone(),
                name: absence.name.clone(),
            });
        }
    }

    // The indirect dependency: a mentioned name is defined in some places;
    // when that set moves, the answer depends on something else now.
    for name in stored.mentioned_names() {
        let before = stored.definitions_of(&name);
        let after = head_definitions_of(head, &name);
        if before != after {
            reasons.push(DerivedReuseReason::DependencyMoved { name });
        }
    }

    if reasons.is_empty() {
        DerivedReuseDecision::Reusable
    } else {
        DerivedReuseDecision::Recompute { reasons }
    }
}

/// The places a name is defined, as the head view holds them.
fn head_definitions_of(head: &SnapshotEntityView, name: &str) -> BTreeSet<PathKey> {
    head.entities
        .values()
        .filter(|entity| entity.name == name)
        .filter_map(|entity| PathKey::new(&identity_file(&entity.fqn)).ok())
        .collect()
}

/// The comparison between revalidating a derived answer and recomputing it.
///
/// The units are the ones the scheduler already counts: the revalidation
/// spends what the decision cost to examine, and the recomputation spends
/// what the query cost the last time it ran. An adversarial transition —
/// one that changes everything the answer depends on — spends the
/// revalidation and gets nothing back, and the measurement says so
/// instead of hiding it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivedReuseMeasurement {
    /// What the revalidation spent.
    pub revalidation_units: u64,
    /// What the recomputation would have spent.
    pub recompute_units: u64,
    /// Whether the answer was reused.
    pub reused: bool,
    /// What the revalidation bought: the units it saved, zero when the
    /// answer had to be computed again anyway.
    pub saved_units: u64,
    /// The reasons the answer was not reused.
    pub reasons: Vec<DerivedReuseReason>,
}

impl DerivedReuseMeasurement {
    /// Measures a decision against the cost of recomputing the answer.
    pub fn of(
        decision: &DerivedReuseDecision,
        revalidation_units: u64,
        recompute_units: u64,
    ) -> Self {
        let reused = decision.is_reusable();
        Self {
            revalidation_units,
            recompute_units,
            reused,
            saved_units: if reused {
                recompute_units.saturating_sub(revalidation_units)
            } else {
                0
            },
            reasons: decision.reasons().to_vec(),
        }
    }
}

/// The decision for the work that consumed a derived answer.
///
/// A work that consumed an answer depends on what the answer SAYS, not on
/// the facts it was computed from: a line that moved changes the facts and
/// not the answer, and a work that is answered again for that is a work
/// that did the same thing twice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivedWorkDecision {
    /// The work the decision is about.
    pub work: WorkId,
    /// What the answer means for the work.
    pub disposition: WorkDisposition,
    /// Why.
    pub reasons: Vec<DerivedWorkReason>,
}

/// Why a work was placed in its disposition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DerivedWorkReason {
    /// The answer says the same thing on the new snapshot: the work keeps
    /// the identity it holds and does not have to be answered again.
    IdentityPreserved,
    /// The answer says something else: the work has to be answered again.
    IdentityChanged,
    /// Nothing could be proven: the answer does not name everything it
    /// read, so the work has to be answered again.
    Unprovable,
}

/// The decision for the work that consumed a derived answer.
///
/// The outcome is what answering the query produced on the new snapshot.
/// When the answer was reused, or when it was computed again and says the
/// same thing, the work is `Unaffected`: it holds an identity that is still
/// the identity. When the answer says something else, the work is
/// `Affected`. When the answer cannot prove what it read, the work is
/// `Unknown`: uncertainty never collapses into an unaffected work.
pub fn plan_derived_work(work: &WorkId, outcome: &DerivedOutcome) -> DerivedWorkDecision {
    let (disposition, reasons) = match outcome {
        DerivedOutcome::Reused { .. } => (
            WorkDisposition::Unaffected,
            vec![DerivedWorkReason::IdentityPreserved],
        ),
        DerivedOutcome::Recomputed {
            answer,
            identity_preserved,
        } => {
            if *identity_preserved {
                (
                    WorkDisposition::Unaffected,
                    vec![DerivedWorkReason::IdentityPreserved],
                )
            } else if !answer.scope.is_exact() || answer.unattributed > 0 {
                (
                    WorkDisposition::Unknown,
                    vec![DerivedWorkReason::Unprovable],
                )
            } else {
                (
                    WorkDisposition::Affected,
                    vec![DerivedWorkReason::IdentityChanged],
                )
            }
        }
    };
    DerivedWorkDecision {
        work: work.clone(),
        disposition,
        reasons,
    }
}

/// The store of derived answers.
///
/// One answer per query and base: comparing against alternating bases
/// must not make each comparison discard the previous base's answer, so a
/// query keeps one answer per base it ran on, up to a bound. The store is
/// bounded, and forgetting is safe: a forgotten answer is simply not
/// there, and the query runs again.
#[derive(Debug, Clone)]
pub struct DerivedAnswerStore {
    answers: BTreeMap<FactsQuery, Vec<(SnapshotId, DerivedAnswer)>>,
    capacity: usize,
    forgotten: usize,
}

impl DerivedAnswerStore {
    /// A store that keeps at most `capacity` answers per query.
    pub fn new(capacity: usize) -> Self {
        Self {
            answers: BTreeMap::new(),
            capacity: capacity.max(1),
            forgotten: 0,
        }
    }

    /// Remembers the answer of a query on a snapshot.
    pub fn store(&mut self, query: FactsQuery, answer: DerivedAnswer) {
        let snapshot = answer.produced_in;
        let entries = self.answers.entry(query).or_default();
        entries.retain(|(kept, _)| *kept != snapshot);
        entries.push((snapshot, answer));
        while entries.len() > self.capacity {
            entries.remove(0);
            self.forgotten += 1;
        }
    }

    /// The answer of a query on a snapshot, when the store still holds it.
    pub fn get(&self, query: &FactsQuery, snapshot: SnapshotId) -> Option<&DerivedAnswer> {
        self.answers
            .get(query)?
            .iter()
            .find(|(kept, _)| *kept == snapshot)
            .map(|(_, answer)| answer)
    }

    /// How many answers the store holds.
    pub fn len(&self) -> usize {
        self.answers
            .values()
            .map(|entries| entries.len())
            .sum::<usize>()
    }

    /// Whether the store holds nothing.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// How many answers the bound made the store forget.
    pub fn forgotten(&self) -> usize {
        self.forgotten
    }
}

/// What answering a query on a new snapshot produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DerivedOutcome {
    /// The stored answer still held: it was reused as it is.
    Reused {
        /// The answer, as it was stored.
        answer: DerivedAnswer,
    },
    /// The answer had to be computed again.
    Recomputed {
        /// The answer, re-anchored to the new snapshot when it says the
        /// same thing it said before.
        answer: DerivedAnswer,
        /// Whether the answer says the same thing it said before. When it
        /// does, the dependents that hold it keep the identity they hold.
        identity_preserved: bool,
    },
}

/// Answers a query on a new snapshot, reusing the stored answer when it
/// still holds.
///
/// `base` is the snapshot the stored answer was computed on. When nothing
/// the stored answer depends on changed, it is reused as it is: no query
/// runs, and the answer keeps the places it had. When it has to be
/// computed again and the new answer says the same thing, the stored
/// answer keeps its identity and takes the new places — the dependents
/// that hold it do not have to be answered again.
pub fn answer_on(
    store: &mut DerivedAnswerStore,
    query: &FactsQuery,
    base: SnapshotId,
    head: &RevisionFacts,
    change: &DerivedChange,
    producer: &str,
    coverage_complete: bool,
) -> DerivedOutcome {
    let head_view = SnapshotEntityView::from_facts(&head.facts, head.snapshot);
    let stored = store.get(query, base).cloned();

    if let Some(stored) = stored {
        let decision = reuse_decision(&stored, change, &head_view, producer, coverage_complete);
        if decision.is_reusable() {
            return DerivedOutcome::Reused { answer: stored };
        }
        let recomputed = answer_of(head, query, producer, coverage_complete);
        let preserved = stored.observably_equivalent(&recomputed);
        let answer = if preserved {
            stored.reanchored(&recomputed)
        } else {
            recomputed
        };
        store.store(query.clone(), answer.clone());
        return DerivedOutcome::Recomputed {
            answer,
            identity_preserved: preserved,
        };
    }

    let answer = answer_of(head, query, producer, coverage_complete);
    store.store(query.clone(), answer.clone());
    DerivedOutcome::Recomputed {
        answer,
        identity_preserved: false,
    }
}

#[cfg(test)]
/// The fixtures the tests of this module share.
mod tests_support {
    use super::*;

    use crate::domain::evidence_kernel::fact::{ProducerKind, ProvenanceRecord};
    use crate::domain::evidence_kernel::ids::{EntityId, FactId, SnapshotId};
    use crate::domain::evidence_kernel::relation::RelationKind;
    use crate::domain::value_objects::Provenance;

    /// The producer the facts of a revision carry.
    pub const PRODUCER: &str = "cognicode.revision-facts/v4";

    pub fn path(text: &str) -> PathKey {
        PathKey::new(text).expect("a path")
    }

    pub fn fact(
        id: u64,
        subject: u64,
        predicate: &str,
        object: &str,
        detail: Option<&str>,
        snap: u64,
    ) -> Fact {
        Fact::new(
            FactId::new(id),
            EntityId::new(subject),
            RelationKind::try_new(predicate).expect("a predicate"),
            FactValue::Text(object.to_string()),
            SnapshotId::new(snap),
            ProvenanceRecord::new(
                Provenance::Extracted,
                ProducerKind::DeterministicAnalyzer,
                detail.map(str::to_string),
            ),
        )
        .expect("a fact")
    }

    /// One file `src/a.rs` with `alpha` (subject 10), which calls
    /// `helper`, and one file `src/b.rs` with `helper` (subject 30).
    ///
    /// `alpha_line` is where `alpha` sits, `alpha_kind` what it is, and
    /// `helper_path` where `helper` is defined: the three knobs a test
    /// needs to move a line, to change a kind and to move a definition.
    pub fn revision(
        snap: u64,
        alpha_line: u32,
        alpha_kind: &str,
        helper_path: &str,
        helper_line: u32,
    ) -> RevisionFacts {
        let alpha_detail = format!("kind={alpha_kind}");
        let helper_detail = "kind=Function".to_string();
        let alpha_fqn = format!("src/a.rs:alpha:{alpha_line}");
        let helper_fqn = format!("{helper_path}:helper:{helper_line}");
        let facts = vec![
            fact(1, 10, "core:defines", &alpha_fqn, Some(&alpha_detail), snap),
            fact(2, 10, "core:calls", "helper", None, snap),
            fact(
                3,
                30,
                "core:defines",
                &helper_fqn,
                Some(&helper_detail),
                snap,
            ),
        ];
        let mut path_facts: BTreeMap<PathKey, Vec<FactId>> = BTreeMap::new();
        path_facts.insert(path("src/a.rs"), vec![FactId::new(1), FactId::new(2)]);
        path_facts.insert(path(helper_path), vec![FactId::new(3)]);
        RevisionFacts {
            snapshot: SnapshotId::new(snap),
            facts,
            path_facts,
        }
    }

    pub fn query() -> FactsQuery {
        FactsQuery {
            path: path("src/a.rs"),
            looks_for: Vec::new(),
            breadth: ReadBreadth::Pointwise,
        }
    }

    pub fn answer(revision: &RevisionFacts) -> DerivedAnswer {
        answer_of(revision, &query(), PRODUCER, true)
    }

    pub fn head_view(revision: &RevisionFacts) -> SnapshotEntityView {
        SnapshotEntityView::from_facts(&revision.facts, revision.snapshot)
    }

    pub fn change(paths: &[&str]) -> DerivedChange {
        DerivedChange::of(&paths.iter().map(|text| path(text)).collect::<Vec<_>>())
    }
}

#[cfg(test)]
mod tests {
    use super::tests_support::*;

    use super::*;
    use crate::application::change_tracking::read_scope::READ_SCOPE_SCHEMA;

    /// A line that moves changes the fact that names the symbol, and
    /// nothing about what the answer says.
    #[test]
    fn a_line_that_moved_does_not_change_what_the_answer_says() {
        let base = revision(1, 1, "Function", "src/b.rs", 1);
        let head = revision(2, 9, "Function", "src/b.rs", 1);

        let before = answer(&base);
        let after = answer(&head);

        assert_eq!(
            before.identity, after.identity,
            "the line is where the symbol was found, not what it is"
        );
        assert!(before.observably_equivalent(&after));
        assert_ne!(
            before.locations, after.locations,
            "the place moved, so the answer has to say where the symbol is now"
        );
    }

    /// The kind is part of what a thing is: a symbol that stops being a
    /// function and starts being a struct is another thing.
    #[test]
    fn a_kind_that_changed_is_another_answer() {
        let base = revision(1, 1, "Function", "src/b.rs", 1);
        let head = revision(2, 1, "Struct", "src/b.rs", 1);

        assert_ne!(answer(&base).identity, answer(&head).identity);
        assert!(!answer(&base).observably_equivalent(&answer(&head)));
    }

    /// Two worktrees of the same tree say the same thing: the identity
    /// does not depend on the snapshot it was computed on.
    #[test]
    fn two_worktrees_of_the_same_tree_say_the_same_thing() {
        let one = revision(1, 1, "Function", "src/b.rs", 1);
        let other = revision(2, 1, "Function", "src/b.rs", 1);

        assert_eq!(answer(&one).identity, answer(&other).identity);
        assert!(answer(&one).observably_equivalent(&answer(&other)));
    }

    /// The re-anchored answer keeps the identity and takes the new places.
    #[test]
    fn the_re_anchored_answer_keeps_its_identity_and_moves_its_places() {
        let base = revision(1, 1, "Function", "src/b.rs", 1);
        let head = revision(2, 9, "Function", "src/b.rs", 1);

        let before = answer(&base);
        let after = answer(&head);
        let reanchored = before.reanchored(&after);

        assert_eq!(reanchored.identity, before.identity);
        assert_eq!(reanchored.locations, after.locations);
        assert_eq!(reanchored.produced_in, after.produced_in);
    }

    /// The answer depends on where the names it mentions are defined, even
    /// when that is another file.
    #[test]
    fn the_answer_depends_on_where_the_names_it_mentions_are_defined() {
        let base = revision(1, 1, "Function", "src/b.rs", 1);
        let answer = answer(&base);

        assert!(
            answer
                .scope
                .reads
                .iter()
                .any(|read| read.path == "src/b.rs" && read.symbol.as_deref() == Some("helper")),
            "a call that names a symbol depends on where that symbol is defined: {:?}",
            answer.scope.reads
        );
    }

    /// An unrelated change leaves the answer reusable.
    #[test]
    fn an_unrelated_change_leaves_the_answer_reusable() {
        let base = revision(1, 1, "Function", "src/b.rs", 1);
        let head = revision(2, 1, "Function", "src/b.rs", 1);

        let decision = reuse_decision(
            &answer(&base),
            &change(&["src/c.rs"]),
            &head_view(&head),
            PRODUCER,
            true,
        );

        assert!(
            decision.is_reusable(),
            "nothing the answer depends on changed: {:?}",
            decision.reasons()
        );
    }

    /// A change in the path the answer read forbids the reuse.
    #[test]
    fn a_change_in_the_path_the_answer_read_forbids_the_reuse() {
        let base = revision(1, 1, "Function", "src/b.rs", 1);
        let head = revision(2, 1, "Function", "src/b.rs", 1);

        let decision = reuse_decision(
            &answer(&base),
            &change(&["src/a.rs"]),
            &head_view(&head),
            PRODUCER,
            true,
        );

        assert_eq!(
            decision.reasons(),
            &[DerivedReuseReason::ReadPathChanged {
                path: path("src/a.rs")
            }]
        );
    }

    /// A name the answer looked for and did not find, that is now there,
    /// forbids the reuse.
    #[test]
    fn an_absence_that_is_now_there_forbids_the_reuse() {
        let base = revision(1, 1, "Function", "src/b.rs", 1);
        let mut head = revision(2, 1, "Function", "src/b.rs", 1);
        head.facts.push(fact(
            4,
            40,
            "core:defines",
            "src/a.rs:ghost:3",
            Some("kind=Function"),
            2,
        ));
        head.path_facts
            .get_mut(&path("src/a.rs"))
            .expect("the path")
            .push(FactId::new(4));

        let query = FactsQuery {
            path: path("src/a.rs"),
            looks_for: vec!["ghost".to_string()],
            breadth: ReadBreadth::Pointwise,
        };
        let stored = answer_of(&base, &query, PRODUCER, true);
        assert_eq!(stored.absences.len(), 1, "the missing name is recorded");

        let decision = reuse_decision(
            &stored,
            &change(&["src/a.rs"]),
            &head_view(&head),
            PRODUCER,
            true,
        );

        assert!(
            decision
                .reasons()
                .contains(&DerivedReuseReason::AbsenceViolated {
                    path: path("src/a.rs"),
                    name: "ghost".to_string(),
                }),
            "the thing the answer looked for and did not find is now there: {:?}",
            decision.reasons()
        );
    }

    /// A definition that moved forbids the reuse: the answer depends on
    /// where the name it mentions resolves.
    #[test]
    fn a_definition_that_moved_forbids_the_reuse() {
        let base = revision(1, 1, "Function", "src/b.rs", 1);
        let head = revision(2, 1, "Function", "src/c.rs", 1);

        let decision = reuse_decision(
            &answer(&base),
            &change(&["src/c.rs"]),
            &head_view(&head),
            PRODUCER,
            true,
        );

        assert!(
            decision
                .reasons()
                .contains(&DerivedReuseReason::DependencyMoved {
                    name: "helper".to_string()
                }),
            "the name the answer mentions resolves somewhere else now: {:?}",
            decision.reasons()
        );
    }

    /// Another producer forbids the reuse: the same words can mean another
    /// thing when another producer wrote them.
    #[test]
    fn another_producer_forbids_the_reuse() {
        let base = revision(1, 1, "Function", "src/b.rs", 1);
        let head = revision(2, 1, "Function", "src/b.rs", 1);

        let decision = reuse_decision(
            &answer(&base),
            &change(&[]),
            &head_view(&head),
            "cognicode.revision-facts/v5",
            true,
        );

        assert_eq!(
            decision.reasons(),
            &[DerivedReuseReason::ProducerChanged {
                before: PRODUCER.to_string(),
                after: "cognicode.revision-facts/v5".to_string()
            }]
        );
    }

    /// Another coverage class forbids the reuse: the two answers do not
    /// cover the same ground.
    #[test]
    fn another_coverage_class_forbids_the_reuse() {
        let base = revision(1, 1, "Function", "src/b.rs", 1);
        let head = revision(2, 1, "Function", "src/b.rs", 1);

        let decision = reuse_decision(
            &answer(&base),
            &change(&[]),
            &head_view(&head),
            PRODUCER,
            false,
        );

        assert_eq!(decision.reasons(), &[DerivedReuseReason::CoverageChanged]);
    }

    /// A scope that does not name everything it read proves nothing: the
    /// answer is computed again.
    #[test]
    fn a_scope_that_proves_nothing_forbids_the_reuse() {
        let base = revision(1, 1, "Function", "src/b.rs", 1);
        let head = revision(2, 1, "Function", "src/b.rs", 1);

        let mut stored = answer(&base);
        stored.scope = ReadScope {
            schema: READ_SCOPE_SCHEMA.to_string(),
            reads: stored.scope.reads.clone(),
            truncated: true,
            unresolved_reads: 0,
            breadth: ReadBreadth::Pointwise,
        };

        let decision = reuse_decision(&stored, &change(&[]), &head_view(&head), PRODUCER, true);

        assert_eq!(decision.reasons(), &[DerivedReuseReason::ScopeIncomplete]);
    }

    /// The measurement says what the revalidation bought, and an
    /// adversarial transition gets nothing back.
    #[test]
    fn the_measurement_says_what_the_revalidation_bought() {
        let base = revision(1, 1, "Function", "src/b.rs", 1);
        let head = revision(2, 1, "Function", "src/b.rs", 1);
        let stored = answer(&base);

        let reusable = DerivedReuseMeasurement::of(
            &reuse_decision(
                &stored,
                &change(&["src/c.rs"]),
                &head_view(&head),
                PRODUCER,
                true,
            ),
            10,
            100,
        );
        assert!(reusable.reused);
        assert_eq!(reusable.saved_units, 90);

        let adversarial = DerivedReuseMeasurement::of(
            &reuse_decision(
                &stored,
                &change(&["src/a.rs"]),
                &head_view(&head),
                PRODUCER,
                true,
            ),
            10,
            100,
        );
        assert!(!adversarial.reused);
        assert_eq!(
            adversarial.saved_units, 0,
            "the revalidation was spent and the answer had to be computed again"
        );
        assert_eq!(adversarial.reasons.len(), 1);
    }

    /// A collection read sees what appears anywhere: a path that was
    /// added forbids the reuse.
    #[test]
    fn a_new_path_forbids_the_reuse_of_a_collection_read() {
        let base = revision(1, 1, "Function", "src/b.rs", 1);
        let head = revision(2, 1, "Function", "src/b.rs", 1);
        let mut stored = answer(&base);
        stored.scope.breadth = ReadBreadth::Aggregate;

        let change = DerivedChange {
            changed: BTreeSet::new(),
            added: BTreeSet::from([path("src/c.rs")]),
        };
        let decision = reuse_decision(&stored, &change, &head_view(&head), PRODUCER, true);

        assert_eq!(
            decision.reasons(),
            &[DerivedReuseReason::CollectionAddition {
                path: path("src/c.rs")
            }]
        );
    }

    /// A read that named exactly the facts it read does not see what
    /// appears elsewhere: a path that was added leaves it reusable.
    #[test]
    fn a_new_path_does_not_forbid_the_reuse_of_a_pointwise_read() {
        let base = revision(1, 1, "Function", "src/b.rs", 1);
        let head = revision(2, 1, "Function", "src/b.rs", 1);

        let change = DerivedChange {
            changed: BTreeSet::new(),
            added: BTreeSet::from([path("src/c.rs")]),
        };
        let decision = reuse_decision(&answer(&base), &change, &head_view(&head), PRODUCER, true);

        assert!(decision.is_reusable(), "{:?}", decision.reasons());
    }

    /// A rename in the queried path is another answer: the name the answer
    /// says is not the name it said before.
    #[test]
    fn a_rename_in_the_queried_path_is_not_the_same_answer() {
        let base = revision(1, 1, "Function", "src/b.rs", 1);
        let mut head = revision(2, 1, "Function", "src/b.rs", 1);
        head.facts.retain(|fact| fact.id != FactId::new(1));
        head.facts.push(fact(
            5,
            10,
            "core:defines",
            "src/a.rs:renamed:1",
            Some("kind=Function"),
            2,
        ));
        head.path_facts
            .get_mut(&path("src/a.rs"))
            .expect("the path")
            .push(FactId::new(5));

        assert_ne!(answer(&base).identity, answer(&head).identity);
    }

    /// A rename elsewhere forbids the reuse: the answer names a symbol
    /// that is no longer there, and the definition it depended on moved.
    #[test]
    fn a_rename_elsewhere_forbids_the_reuse() {
        let base = revision(1, 1, "Function", "src/b.rs", 1);
        let mut head = revision(2, 1, "Function", "src/b.rs", 1);
        head.facts.retain(|fact| fact.id != FactId::new(3));
        head.facts.push(fact(
            5,
            30,
            "core:defines",
            "src/b.rs:renamed:1",
            Some("kind=Function"),
            2,
        ));
        head.path_facts
            .get_mut(&path("src/b.rs"))
            .expect("the path")
            .push(FactId::new(5));

        let decision = reuse_decision(
            &answer(&base),
            &change(&["src/b.rs"]),
            &head_view(&head),
            PRODUCER,
            true,
        );

        assert!(!decision.is_reusable());
        assert!(
            decision
                .reasons()
                .contains(&DerivedReuseReason::DependencyMoved {
                    name: "helper".to_string()
                }),
            "the name the answer mentions no longer resolves where it did: {:?}",
            decision.reasons()
        );
    }
}

#[cfg(test)]
mod store_tests {
    use super::tests_support::*;

    use super::*;

    /// An unrelated change reuses the stored answer: the query does not
    /// run again.
    #[test]
    fn an_unrelated_change_reuses_the_stored_answer() {
        let base = revision(1, 1, "Function", "src/b.rs", 1);
        let head = revision(2, 1, "Function", "src/b.rs", 1);
        let mut store = DerivedAnswerStore::new(4);

        let query = query();
        let first = answer_on(
            &mut store,
            &query,
            SnapshotId::new(1),
            &base,
            &change(&[]),
            PRODUCER,
            true,
        );
        assert!(matches!(first, DerivedOutcome::Recomputed { .. }));

        let second = answer_on(
            &mut store,
            &query,
            SnapshotId::new(1),
            &head,
            &change(&["src/c.rs"]),
            PRODUCER,
            true,
        );
        match second {
            DerivedOutcome::Reused { answer } => {
                assert_eq!(answer.produced_in, SnapshotId::new(1));
            }
            other => panic!("the stored answer still held: {other:?}"),
        }
        assert_eq!(store.len(), 1, "the answer was not stored twice");
    }

    /// A line that moved forces the query to run again, and the answer
    /// keeps the identity its dependents hold.
    #[test]
    fn a_line_that_moved_keeps_the_identity_the_dependents_hold() {
        let base = revision(1, 1, "Function", "src/b.rs", 1);
        let head = revision(2, 9, "Function", "src/b.rs", 1);
        let mut store = DerivedAnswerStore::new(4);
        let query = query();

        let first = answer_on(
            &mut store,
            &query,
            SnapshotId::new(1),
            &base,
            &change(&[]),
            PRODUCER,
            true,
        );
        let identity = match first {
            DerivedOutcome::Recomputed { answer, .. } => answer.identity,
            other => panic!("the first run computes: {other:?}"),
        };

        let second = answer_on(
            &mut store,
            &query,
            SnapshotId::new(1),
            &head,
            &change(&["src/a.rs"]),
            PRODUCER,
            true,
        );
        match second {
            DerivedOutcome::Recomputed {
                answer,
                identity_preserved,
            } => {
                assert!(
                    identity_preserved,
                    "the answer says the same thing: the dependents keep the identity they hold"
                );
                assert_eq!(answer.identity, identity);
                assert_eq!(answer.locations[0].line, Some(9), "the place moved");
            }
            other => panic!("the change forces the query to run again: {other:?}"),
        }
    }

    /// A change in what the answer says does not preserve the identity.
    #[test]
    fn a_change_in_what_the_answer_says_does_not_preserve_the_identity() {
        let base = revision(1, 1, "Function", "src/b.rs", 1);
        let head = revision(2, 1, "Struct", "src/b.rs", 1);
        let mut store = DerivedAnswerStore::new(4);
        let query = query();

        let _ = answer_on(
            &mut store,
            &query,
            SnapshotId::new(1),
            &base,
            &change(&[]),
            PRODUCER,
            true,
        );
        let second = answer_on(
            &mut store,
            &query,
            SnapshotId::new(1),
            &head,
            &change(&["src/a.rs"]),
            PRODUCER,
            true,
        );
        match second {
            DerivedOutcome::Recomputed {
                identity_preserved, ..
            } => assert!(!identity_preserved),
            other => panic!("the change forces the query to run again: {other:?}"),
        }
    }

    /// The store is bounded: the oldest base is forgotten, and forgetting
    /// is safe — the answer is simply not there.
    #[test]
    fn the_store_forgets_the_oldest_base_and_stays_correct() {
        let base = revision(1, 1, "Function", "src/b.rs", 1);
        let mut store = DerivedAnswerStore::new(1);
        let query = query();

        let _ = answer_on(
            &mut store,
            &query,
            SnapshotId::new(1),
            &base,
            &change(&[]),
            PRODUCER,
            true,
        );
        assert_eq!(store.len(), 1);

        let other = revision(2, 1, "Function", "src/b.rs", 1);
        let _ = answer_on(
            &mut store,
            &query,
            SnapshotId::new(1),
            &other,
            &change(&["src/a.rs"]),
            PRODUCER,
            true,
        );

        assert_eq!(store.len(), 1, "the bound holds one answer per query");
        assert_eq!(store.forgotten(), 1, "the oldest base was forgotten");
        assert!(
            store.get(&query, SnapshotId::new(1)).is_none(),
            "the forgotten answer is not there: the query runs again"
        );
    }
}

#[cfg(test)]
mod work_tests {
    use super::tests_support::*;

    use super::*;

    use crate::domain::naming::NamespacedName;

    fn work() -> WorkId {
        WorkId::new(NamespacedName::new("tests.pack").expect("a namespaced name"))
    }

    /// A work that consumed an answer that was reused keeps the identity
    /// it holds: it does not have to be answered again.
    #[test]
    fn a_reused_answer_leaves_the_work_unaffected() {
        let base = revision(1, 1, "Function", "src/b.rs", 1);
        let head = revision(2, 1, "Function", "src/b.rs", 1);
        let mut store = DerivedAnswerStore::new(4);
        let query = query();

        let _ = answer_on(
            &mut store,
            &query,
            SnapshotId::new(1),
            &base,
            &change(&[]),
            PRODUCER,
            true,
        );
        let outcome = answer_on(
            &mut store,
            &query,
            SnapshotId::new(1),
            &head,
            &change(&["src/c.rs"]),
            PRODUCER,
            true,
        );

        let decision = plan_derived_work(&work(), &outcome);
        assert_eq!(decision.disposition, WorkDisposition::Unaffected);
        assert_eq!(decision.reasons, vec![DerivedWorkReason::IdentityPreserved]);
    }

    /// A work that consumed an answer that was computed again and says the
    /// same thing keeps the identity it holds.
    #[test]
    fn an_answer_that_says_the_same_thing_leaves_the_work_unaffected() {
        let base = revision(1, 1, "Function", "src/b.rs", 1);
        let head = revision(2, 9, "Function", "src/b.rs", 1);
        let mut store = DerivedAnswerStore::new(4);
        let query = query();

        let _ = answer_on(
            &mut store,
            &query,
            SnapshotId::new(1),
            &base,
            &change(&[]),
            PRODUCER,
            true,
        );
        let outcome = answer_on(
            &mut store,
            &query,
            SnapshotId::new(1),
            &head,
            &change(&["src/a.rs"]),
            PRODUCER,
            true,
        );

        let decision = plan_derived_work(&work(), &outcome);
        assert_eq!(decision.disposition, WorkDisposition::Unaffected);
        assert_eq!(decision.reasons, vec![DerivedWorkReason::IdentityPreserved]);
    }

    /// A work that consumed an answer that says something else is affected.
    #[test]
    fn an_answer_that_says_something_else_leaves_the_work_affected() {
        let base = revision(1, 1, "Function", "src/b.rs", 1);
        let head = revision(2, 1, "Struct", "src/b.rs", 1);
        let mut store = DerivedAnswerStore::new(4);
        let query = query();

        let _ = answer_on(
            &mut store,
            &query,
            SnapshotId::new(1),
            &base,
            &change(&[]),
            PRODUCER,
            true,
        );
        let outcome = answer_on(
            &mut store,
            &query,
            SnapshotId::new(1),
            &head,
            &change(&["src/a.rs"]),
            PRODUCER,
            true,
        );

        let decision = plan_derived_work(&work(), &outcome);
        assert_eq!(decision.disposition, WorkDisposition::Affected);
        assert_eq!(decision.reasons, vec![DerivedWorkReason::IdentityChanged]);
    }
}
