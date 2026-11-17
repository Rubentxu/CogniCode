//! The `preflight` goal: what a set of proposed edits would do to the facts
//! of a Git revision, and what producing them costs, before anything is
//! written.
//!
//! The caller names a base revision and the new text of the files it intends
//! to change. The goal lays those edits over the base as an in-memory
//! candidate revision ([`OverlayReader`]), assembles the facts of both
//! sides with the same producer a real revision gets, and answers with the
//! facts delta and the cost of the candidate's facts.
//!
//! ## What it never does
//!
//! - It never writes the workspace: the edits exist only in the call.
//! - It never writes the cache: the fact object store is opened read-only, no
//!   revision snapshot is saved and no read-set is recorded, so the real
//!   revision handle stays exactly as it was and the candidate leaves no trace
//!   under any handle.
//! - It never shares state between calls: the candidate handle folds the
//!   session, the base and the edits, and lives only in the answer.
//!
//! ## Honesty
//!
//! The delta is the one `compare` reports, with the same `impact_status`: a
//! file no language reader claims, or one that could not be parsed, makes it
//! `partial`, a lower bound. Edits are whole-file texts: binary content cannot
//! be proposed.
//!
//! One function, [`run`], serves the command line and the MCP tool.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::application::change_tracking::revision_facts::{
    FactObjectStore, FactsLimits, ProducedRevision, assess_impact, produce_revision_facts,
};
use crate::application::change_tracking::scm_delta::delta_from_change_set;
use crate::application::revision_snapshots::RevisionSnapshotStore;
use crate::domain::evidence_kernel::fact::{Fact, FactValue};
use crate::domain::evidence_kernel::ids::FactId;
use crate::domain::scm::{
    ChangeKind, ChangeSet, Edit, OverlayReader, PathKey, RevisionReader, SourceRef,
};
use crate::infrastructure::git::GitScmAdapter;
use crate::interface::goals::GoalEnvironment;

use super::compare::{RevisionFactsCoverage, side_coverage};

/// The session label of a call that names none.
const DEFAULT_SESSION: &str = "default";

/// The most facts listed per side of the delta; the counts are never capped.
const MAX_LISTED_FACTS: usize = 200;

fn default_base_ref() -> String {
    "HEAD".to_string()
}

/// Arguments of the `preflight` goal.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct PreflightInput {
    /// The Git revision the edits apply to. Defaults to `HEAD`.
    #[serde(default = "default_base_ref")]
    pub base_ref: String,
    /// The proposed edits, as an object from repository-relative path to the
    /// complete new text of the file, or to `null` to delete it.
    pub edits: BTreeMap<String, Option<String>>,
    /// A label scoping the candidate: the same edits proposed from two
    /// sessions name two distinct candidates. Defaults to `default`.
    #[serde(default)]
    pub session: Option<String>,
}

/// The base revision the edits were laid over.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PreflightBase {
    /// The reference the caller supplied.
    pub requested_ref: String,
    /// The resolved immutable revision handle.
    pub revision: String,
    /// The digest of all tracked content at the base.
    pub content_digest: String,
    /// Tracked files at the base.
    pub tracked_files: usize,
}

/// The in-memory revision: the base plus the edits.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PreflightCandidate {
    /// The candidate handle. It is never a real revision handle and exists
    /// only in this answer.
    pub handle: String,
    /// The session that scopes the handle.
    pub session: String,
    /// The digest of all tracked content of the candidate.
    pub content_digest: String,
    /// Tracked files of the candidate.
    pub tracked_files: usize,
}

/// One fact on one side of the delta.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PreflightFact {
    /// The file the fact was extracted from.
    pub path: String,
    /// The relation the fact asserts, as `namespace:name`.
    pub predicate: String,
    /// What the relation points at, as text.
    pub object: String,
}

/// The change in facts the edits cause.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PreflightDelta {
    /// Facts the candidate asserts and the base does not.
    pub facts_added: usize,
    /// Facts the base asserts and the candidate does not.
    pub facts_removed: usize,
    /// The added facts, in path order; capped, see `listing_truncated`.
    pub added: Vec<PreflightFact>,
    /// The removed facts, in path order; capped, see `listing_truncated`.
    pub removed: Vec<PreflightFact>,
    /// Whether a list was cut at the cap. The counts are always complete.
    pub listing_truncated: bool,
    /// Changed paths no language reader claims: their effect is invisible.
    pub unanalyzed_changed_paths: Vec<String>,
    /// Paths with a missing side of facts.
    pub gap_paths: Vec<String>,
}

/// What producing the candidate's facts costs.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PreflightCost {
    /// Paths whose content differs from the base. An edit that restores the
    /// base bytes is not counted.
    pub changed_paths: usize,
    /// Bytes of text the edits carry.
    pub bytes_edited: usize,
    /// Analyzable files added or modified: the facts that must be produced
    /// anew. This is the least the candidate parses; `candidate.parsed_files`
    /// is what this run measured, higher when the fact store is cold.
    pub files_to_reparse: usize,
    /// What the base's facts cover and how many were parsed or reused.
    pub base: RevisionFactsCoverage,
    /// What the candidate's facts cover and how many were parsed or reused.
    pub candidate: RevisionFactsCoverage,
}

/// The answer of the `preflight` goal.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GoalPreflight {
    /// One line that summarises the answer.
    pub summary: String,
    /// The version of this answer's schema.
    pub schema_version: String,
    /// The goal that answers.
    pub goal: String,
    /// The revision the edits apply to.
    pub base: PreflightBase,
    /// The in-memory revision the edits make.
    pub candidate: PreflightCandidate,
    /// Always `false`: neither the workspace nor the fact store nor the
    /// revision snapshots were written.
    pub persisted: bool,
    /// How far the delta reaches: `evaluated`, `partial` or `not_evaluated`.
    pub impact_status: String,
    /// Why the delta has that status.
    pub impact_reason: String,
    /// The facts the edits add and remove.
    pub delta: PreflightDelta,
    /// What the candidate's facts cost.
    pub cost: PreflightCost,
}

/// Runs the preflight of `input` over `workspace`.
///
/// `cache_root`, when named, is only read: stored fact objects of unchanged
/// bytes are reused, nothing is added.
///
/// # Errors
/// The input is not valid (no edits, an unsafe path, a delete of an untracked
/// path), the base does not resolve, or the facts of a side cannot be
/// assembled.
pub fn preflight(
    workspace: &Path,
    cache_root: Option<&Path>,
    input: &PreflightInput,
) -> Result<GoalPreflight, String> {
    let edits = parse_edits(&input.edits)?;
    let bytes_edited = edits
        .values()
        .map(|edit| match edit {
            Edit::Write(bytes) => bytes.len(),
            Edit::Delete => 0,
        })
        .sum();
    let session = input.session.as_deref().unwrap_or(DEFAULT_SESSION);

    let root = workspace
        .canonicalize()
        .map_err(|error| format!("cannot resolve repository root: {error}"))?;
    let source =
        SourceRef::new("git", &root.to_string_lossy()).map_err(|error| error.to_string())?;
    let scm = GitScmAdapter::new(root.clone());
    let base = scm
        .resolve(&source, &input.base_ref)
        .map_err(|error| format!("cannot resolve revision `{}`: {error}", input.base_ref))?;
    let overlay = OverlayReader::new(&scm, &base, session, edits)
        .map_err(|error| format!("the edits cannot be laid over the base: {error}"))?;

    let objects = cache_root.map(|cache| {
        FactObjectStore::beside(&RevisionSnapshotStore::for_workspace(cache, &root)).read_only()
    });
    let limits = FactsLimits::default();
    let base_facts = produce_revision_facts(&overlay, &base, objects.as_ref(), &limits)
        .map_err(|error| format!("cannot assemble the facts of the base: {error}"))?;
    let candidate_facts =
        produce_revision_facts(&overlay, overlay.candidate(), objects.as_ref(), &limits)
            .map_err(|error| format!("cannot assemble the facts of the candidate: {error}"))?;

    let base_inventory = overlay
        .inventory(&base)
        .map_err(|error| format!("cannot inventory the base: {error}"))?;
    let candidate_inventory = overlay
        .inventory(overlay.candidate())
        .map_err(|error| format!("cannot inventory the candidate: {error}"))?;
    let changes = ChangeSet::between(
        &base,
        overlay.candidate(),
        &base_inventory,
        &candidate_inventory,
    );

    let impact = assess_impact(&base_facts, &candidate_facts, &changes)
        .map_err(|error| format!("the fact delta could not be computed: {error}"))?;
    let delta = listed_delta(&base_facts, &candidate_facts, &changes)?;
    let files_to_reparse = changes
        .changes()
        .iter()
        .filter(|change| matches!(change.kind(), ChangeKind::Added | ChangeKind::Modified))
        .filter(|change| !candidate_facts.unanalyzed.contains(change.path()))
        .count();

    let summary = format!(
        "{} edit(s) over {}: {} path(s) change, {} fact(s) added, {} removed; the delta is {}.",
        input.edits.len(),
        base.handle().as_str(),
        changes.changes().len(),
        impact.facts_added,
        impact.facts_removed,
        impact.status.as_str()
    );
    Ok(GoalPreflight {
        summary,
        schema_version: "cognicode.goal.preflight/v1".to_string(),
        goal: "preflight".to_string(),
        base: PreflightBase {
            requested_ref: input.base_ref.clone(),
            revision: base.handle().as_str().to_string(),
            content_digest: base.content().as_hex().to_string(),
            tracked_files: base_inventory.entries().len(),
        },
        candidate: PreflightCandidate {
            handle: overlay.candidate().handle().as_str().to_string(),
            session: session.to_string(),
            content_digest: overlay.candidate().content().as_hex().to_string(),
            tracked_files: candidate_inventory.entries().len(),
        },
        persisted: false,
        impact_status: impact.status.as_str().to_string(),
        impact_reason: impact.reason,
        delta: PreflightDelta {
            facts_added: impact.facts_added,
            facts_removed: impact.facts_removed,
            added: delta.added,
            removed: delta.removed,
            listing_truncated: delta.truncated,
            unanalyzed_changed_paths: paths_text(&impact.unanalyzed_changed),
            gap_paths: paths_text(&impact.gap_paths),
        },
        cost: PreflightCost {
            changed_paths: changes.changes().len(),
            bytes_edited,
            files_to_reparse,
            base: side_coverage(&base_facts),
            candidate: side_coverage(&candidate_facts),
        },
    })
}

fn paths_text(paths: &[PathKey]) -> Vec<String> {
    paths.iter().map(|path| path.as_str().to_string()).collect()
}

/// The wire edits as the overlay takes them: every path validated, `null`
/// meaning delete.
fn parse_edits(
    edits: &BTreeMap<String, Option<String>>,
) -> Result<BTreeMap<PathKey, Edit>, String> {
    if edits.is_empty() {
        return Err("the goal `preflight` needs at least one edit".to_string());
    }
    edits
        .iter()
        .map(|(path, text)| {
            let key = PathKey::new(path).map_err(|error| error.to_string())?;
            let edit = match text {
                Some(text) => Edit::Write(text.clone().into_bytes()),
                None => Edit::Delete,
            };
            Ok((key, edit))
        })
        .collect()
}

struct ListedDelta {
    added: Vec<PreflightFact>,
    removed: Vec<PreflightFact>,
    truncated: bool,
}

/// The facts the delta adds and removes, each with its file, in a stable order
/// and capped at [`MAX_LISTED_FACTS`] per side.
fn listed_delta(
    base: &ProducedRevision,
    candidate: &ProducedRevision,
    changes: &ChangeSet,
) -> Result<ListedDelta, String> {
    let scm = delta_from_change_set(&base.facts, &candidate.facts, changes)
        .map_err(|error| format!("the fact delta could not be computed: {error}"))?;
    let (added, added_cut) = listing(candidate, scm.delta.added.iter().map(|c| c.fact_id));
    let (removed, removed_cut) = listing(base, scm.delta.removed.iter().map(|c| c.fact_id));
    Ok(ListedDelta {
        added,
        removed,
        truncated: added_cut || removed_cut,
    })
}

fn listing(
    side: &ProducedRevision,
    ids: impl Iterator<Item = FactId>,
) -> (Vec<PreflightFact>, bool) {
    let facts: HashMap<FactId, &Fact> = side.facts.facts.iter().map(|f| (f.id, f)).collect();
    let paths: HashMap<FactId, &PathKey> = side
        .facts
        .path_facts
        .iter()
        .flat_map(|(path, ids)| ids.iter().map(move |id| (*id, path)))
        .collect();
    let mut listed: Vec<PreflightFact> = ids
        .filter_map(|id| {
            let fact = facts.get(&id)?;
            Some(PreflightFact {
                path: paths
                    .get(&id)
                    .map(|p| p.as_str().to_string())
                    .unwrap_or_default(),
                predicate: fact.predicate.as_str().to_string(),
                object: object_text(&fact.object),
            })
        })
        .map(|fact| {
            (
                (
                    fact.path.clone(),
                    fact.predicate.clone(),
                    fact.object.clone(),
                ),
                fact,
            )
        })
        .collect::<BTreeMap<_, _>>()
        .into_values()
        .collect();
    let truncated = listed.len() > MAX_LISTED_FACTS;
    listed.truncate(MAX_LISTED_FACTS);
    (listed, truncated)
}

fn object_text(value: &FactValue) -> String {
    match value {
        FactValue::Text(text) => text.clone(),
        FactValue::Int(number) => number.to_string(),
        FactValue::Float(number) => number.to_string(),
        FactValue::Bool(flag) => flag.to_string(),
        FactValue::Ref(entity) => format!("entity:{}", entity.0),
    }
}

/// The implementation of the goal: reads its arguments and returns its answer.
pub fn run(ctx: &GoalEnvironment, arguments: &Value) -> Result<Value, String> {
    let input: PreflightInput = serde_json::from_value(arguments.clone())
        .map_err(|error| format!("the arguments of the goal `preflight` are not valid: {error}"))?;
    let answer = preflight(ctx.workspace, ctx.cache_root, &input)?;
    serde_json::to_value(answer)
        .map_err(|error| format!("the answer of the goal `preflight` is not serialisable: {error}"))
}

/// The answer of the goal `preflight` as text.
pub fn render(answer: &Value) -> String {
    let preflight: GoalPreflight = serde_json::from_value(answer.clone())
        .expect("the answer of the goal `preflight` is the answer its own type describes");
    let mut text = format!("{}\n", preflight.summary);
    text.push_str(&format!(
        "Candidate {} (session {}), nothing written.\n",
        preflight.candidate.handle, preflight.candidate.session
    ));
    text.push_str(&format!(
        "Cost: {} changed path(s), {} byte(s) edited, {} file(s) to reparse; the candidate parsed {} and reused {}.\n",
        preflight.cost.changed_paths,
        preflight.cost.bytes_edited,
        preflight.cost.files_to_reparse,
        preflight.cost.candidate.parsed_files,
        preflight.cost.candidate.reused_files
    ));
    for (sign, facts) in [
        ("+", &preflight.delta.added),
        ("-", &preflight.delta.removed),
    ] {
        for fact in facts {
            text.push_str(&format!(
                "  {sign} {} {} {}\n",
                fact.path, fact.predicate, fact.object
            ));
        }
    }
    text.push_str(&format!("Impact: {}\n", preflight.impact_reason));
    text
}

#[cfg(test)]
mod tests;
