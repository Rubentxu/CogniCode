//! The seam between what an SCM says changed and what the planner says
//! must run: the bridge of [`ChangeSet`] evidence into a [`FactDelta`],
//! consumed by the one existing planner — never a second one.
//!
//! ## What a fact diff alone cannot see
//!
//! [`compute_fact_delta`] compares triples. A provider that classified
//! a change the triples do not show — an unclassifiable `Unknown`, or a
//! manifest edit no analyzer fact depends on — is therefore invisible
//! to the diff, and the planner would call the world unchanged. The
//! bridge states exactly those changes as evidence facts in the delta
//! itself, so the planner's own conservative rule for additions (new
//! facts the previous execution cannot have read → `Unknown`) fires.
//! It adds that evidence and nothing else: `Renamed`/`Copied` changes
//! need no reparse by the CAA-02 contract and never escalate.
//!
//! ## What the bridge never does
//!
//! It does not schedule, does not rank, does not cache. Every verdict
//! comes from `planner::plan_affected_work` — the bridge only decides
//! what goes into the delta. Uncertainty must never collapse into a
//! skipped work item.

use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};

use crate::domain::evidence_kernel::fact::{Fact, FactValue};
use crate::domain::evidence_kernel::ids::{EntityId, FactId, SnapshotId};
use crate::domain::evidence_kernel::relation::RelationKind;
use crate::domain::evidence_kernel::semantic_diff::{
    FactChange, FactDelta, SemanticDiffError, compute_fact_delta,
};
use crate::domain::scm::{ChangeKind, ChangeSet, PathKey, SourceInventory, SourceRevision};

use super::planner::{AffectedWorkPlan, ExecutionDependency, plan_affected_work};

/// The predicate of a bridge evidence fact: an SCM-level assertion
/// about one path whose change the triple diff cannot express.
pub const SCM_EVIDENCE_PREDICATE: &str = "scm:evidence";

/// The facts derived for one revision, and the index saying which of
/// them live under which path. The index is what lets path evidence
/// speak to a triple diff.
#[derive(Debug, Clone)]
pub struct RevisionFacts {
    pub snapshot: SnapshotId,
    pub facts: Vec<Fact>,
    pub path_facts: BTreeMap<PathKey, Vec<FactId>>,
}

/// The delta the change set supports, plus the paths whose change it
/// cannot classify into triples at all.
#[derive(Debug, Clone)]
pub struct ScmDelta {
    pub delta: FactDelta,
    pub unclassified: Vec<PathKey>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ScmDeltaError {
    #[error(transparent)]
    Semantic(#[from] SemanticDiffError),
}

/// The snapshot identity of one revision: a fold of provider, project,
/// handle and content digest. The same revision always names the same
/// snapshot, and that is the reproducible half of the plan's identity.
pub fn snapshot_id_for(revision: &SourceRevision) -> SnapshotId {
    let mut hasher = std::hash::DefaultHasher::new();
    revision.source().provider().as_str().hash(&mut hasher);
    revision.source().project().hash(&mut hasher);
    revision.handle().as_str().hash(&mut hasher);
    revision.content().as_hex().hash(&mut hasher);
    SnapshotId(std::cmp::max(hasher.finish(), 1))
}

/// Evidenced rename pairs derivable from two inventories: a digest
/// deleted at exactly one path and added at exactly one other path.
/// A digest that appears or disappears at several paths is ambiguous
/// and is skipped — the bridge never invents a pairing the content
/// evidence does not support. Deterministically sorted by old path.
pub fn rename_pairs(base: &SourceInventory, head: &SourceInventory) -> Vec<(PathKey, PathKey)> {
    let changes = ChangeSet::between_raw(base, head);
    let mut deleted: BTreeMap<&str, Vec<&PathKey>> = BTreeMap::new();
    let mut added: BTreeMap<&str, Vec<&PathKey>> = BTreeMap::new();
    for change in changes.changes() {
        match change.kind() {
            ChangeKind::Deleted => {
                if let Some(d) = change.before() {
                    deleted.entry(d.as_hex()).or_default().push(change.path());
                }
            }
            ChangeKind::Added => {
                if let Some(d) = change.after() {
                    added.entry(d.as_hex()).or_default().push(change.path());
                }
            }
            _ => {}
        }
    }
    let mut pairs = Vec::new();
    for (digest, olds) in deleted {
        if olds.len() != 1 {
            continue;
        }
        if let Some(news) = added.get(digest)
            && news.len() == 1
        {
            pairs.push((olds[0].clone(), news[0].clone()));
        }
    }
    pairs.sort();
    pairs
}

/// The delta the change set supports over the two revisions' facts,
/// plus the paths whose change the change set itself could not
/// classify.
pub fn delta_from_change_set(
    base: &RevisionFacts,
    head: &RevisionFacts,
    changes: &ChangeSet,
) -> Result<ScmDelta, ScmDeltaError> {
    let delta = compute_fact_delta(
        base.snapshot,
        base.facts.iter().cloned(),
        head.snapshot,
        head.facts.iter().cloned(),
    )?;
    let mut unclassified: Vec<PathKey> = changes
        .changes()
        .iter()
        .filter(|c| matches!(c.kind(), ChangeKind::Unknown))
        .map(|c| c.path().clone())
        .collect();
    unclassified.sort();
    unclassified.dedup();
    Ok(ScmDelta {
        delta,
        unclassified,
    })
}

/// The plan for moving from `base` to `head`: the delta, the one
/// planner over it, and one evidence fact per path the change set
/// cannot speak about in triples — every `Unknown` change and every
/// change touching a path declared in `manifests`.
pub fn plan_between(
    base: &RevisionFacts,
    head: &RevisionFacts,
    changes: &ChangeSet,
    manifests: &BTreeSet<PathKey>,
    dependencies: &[ExecutionDependency],
) -> Result<AffectedWorkPlan, ScmDeltaError> {
    let mut scm = delta_from_change_set(base, head, changes)?;
    let escalated: BTreeSet<PathKey> = scm
        .unclassified
        .iter()
        .cloned()
        .chain(
            changes
                .changes()
                .iter()
                .filter(|c| manifests.contains(c.path()))
                .map(|c| c.path().clone()),
        )
        .collect();
    for path in escalated {
        scm.delta.added.push(evidence_change(&path));
    }
    sort_added(&mut scm.delta.added);
    Ok(plan_affected_work(&scm.delta, dependencies))
}

/// One evidence fact as a delta addition: a new assertion the previous
/// execution cannot have read, which is exactly the planner's trigger
/// for its conservative `Unknown`.
fn evidence_change(path: &PathKey) -> FactChange {
    let (subject, fact) = evidence_ids(path.as_str());
    FactChange {
        semantic_key: crate::domain::evidence_kernel::semantic_diff::FactSemanticKey {
            subject,
            predicate: RelationKind::try_new(SCM_EVIDENCE_PREDICATE)
                .expect("the evidence predicate is a valid ns:name by construction"),
            object: FactValue::Text(path.as_str().to_string()),
        },
        fact_id: fact,
    }
}

/// Stable ids for one evidence path: the same path always names the
/// same subject and fact id, so two runs of the bridge over the same
/// evidence produce the same delta.
fn evidence_ids(path: &str) -> (EntityId, FactId) {
    let mut hasher = std::hash::DefaultHasher::new();
    path.hash(&mut hasher);
    let folded = std::cmp::max(hasher.finish(), 1);
    (EntityId(folded), FactId(folded))
}

/// The deterministic order the kernel's own diff uses for additions:
/// subject, then predicate, then the rendered object.
fn sort_added(added: &mut [FactChange]) {
    added.sort_by(|a, b| {
        a.semantic_key
            .subject
            .0
            .cmp(&b.semantic_key.subject.0)
            .then_with(|| {
                a.semantic_key
                    .predicate
                    .to_string()
                    .cmp(&b.semantic_key.predicate.to_string())
            })
            .then_with(|| {
                format!("{:?}", a.semantic_key.object).cmp(&format!("{:?}", b.semantic_key.object))
            })
    });
}
