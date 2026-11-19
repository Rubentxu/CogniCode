//! Complete logical coverage (CAA-08): partial physical analysis plus
//! valid reused evidence, unified into one snapshot whose verdict is
//! honest.
//!
//! ## What this owns
//!
//! This is the one place that decides whether a candidate snapshot has
//! COMPLETE logical coverage: every piece of work the planner named is
//! either freshly rerun or reused from a cache whose survival the
//! CAA-07 reuse decision proves. Nothing here runs detectors — the
//! caller supplies rerun records and the reuse index, and this module
//! only judges what they cover. It is pure, deterministic and does no
//! I/O, like the planner it composes.
//!
//! ## The honesty contract
//!
//! - Only `IncrementalComplete` and `FullRebuild` may ever claim a
//!   `Pass`. `Focused` and `Impacted` are scoped questions: answering
//!   them fully never proves the whole tree, so their verdict is
//!   always `Scoped`, even when every requested work item resolved.
//! - A branch with a parse error, a missing capability, a plugin with
//!   no coverage or a truncated read-set NEVER receives `Pass`: those
//!   arrive as [`CaptureGap`]s or as properties of the records
//!   themselves, and each one blocks.
//! - Evidence from two snapshots is never mixed: the plan's
//!   `to_snapshot` must BE the candidate, reused findings must be
//!   produced in the very base snapshot the planner diffed from, and
//!   fresh findings in the candidate itself. Anything else is
//!   rejected with `SnapshotMismatch` before its findings are merged.
//! - Reuse follows CAA-07 exactly: a cached record survives only when
//!   every identity layer survives; when the rule, capability, parser,
//!   sanitizer or semantic layers die, the work must have a rerun
//!   record or it is `Unresolved` — never silently skipped. Completeness
//!   is STRICTER than reuse here on one point, deliberately: a
//!   truncated read-set blocks even with no invalidating delta,
//!   because an incomplete lineage cannot prove non-impact — that is
//!   AC #2, a question the reuse owner does not answer.
//! - A completeness-claiming mode handed a plan that named no work
//!   gets `EmptyWorkPlan`, never a vacuous `Pass`.
//!
//! The reason codes are minted here and nowhere else; every surface
//! that renders this verdict (the CLI and MCP wiring is CAA-11) must
//! publish these exact strings, so two surfaces over identical inputs
//! cannot disagree.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::application::change_tracking::planner::{AffectedWorkPlan, WorkId};
use crate::domain::evidence_kernel::ids::SnapshotId;
use crate::domain::evidence_kernel::semantic_diff::FactDelta;
use crate::domain::findings::detector_reuse::{
    DetectorReuseIndex, DetectorRunIdentity, DetectorRunRecord, InvalidationLevel, ReboundFinding,
    rebind_candidate, reuse_decision,
};

/// The question the run was asked to answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisMode {
    /// Only the named scope was asked for; completeness is out of scope.
    Focused,
    /// Only work impacted by a change set was asked for.
    Impacted,
    /// Full logical coverage, physical reruns plus valid reuse.
    IncrementalComplete,
    /// Full logical coverage, everything rerun from scratch.
    FullRebuild,
}

impl AnalysisMode {
    /// Whether this mode may ever claim complete logical coverage.
    /// Scoped modes answer honestly even when they succeed.
    pub fn claims_complete_coverage(self) -> bool {
        matches!(self, Self::IncrementalComplete | Self::FullRebuild)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Focused => "focused",
            Self::Impacted => "impacted",
            Self::IncrementalComplete => "incremental_complete",
            Self::FullRebuild => "full_rebuild",
        }
    }
}

/// A stable reason code, shared by every surface that renders a
/// blocking issue. Adding a variant is a contract change; renaming one
/// never happens silently.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasonCode {
    /// A file failed to parse; whatever the detectors say is built on
    /// an incomplete tree.
    ParseError,
    /// A required capability was not offered by the environment.
    CapabilityMissing,
    /// A plugin was selected but covers nothing it was asked to cover.
    PluginUncovered,
    /// A read-set hit its bound: lineage cannot prove the world.
    ReadSetTruncated,
    /// The persisted reuse index could not be decoded intact.
    CacheCorrupt,
    /// Reuse was planned but no record exists for this work.
    CacheMissing,
    /// Evidence provenance names a snapshot other than the one the
    /// plan diffed from or the candidate itself.
    SnapshotMismatch,
    /// A cached record lost at least one identity layer and no rerun
    /// replaced it.
    ReuseInvalidated,
    /// The current run identity was not provided, so reuse cannot be
    /// proven for this work.
    IdentityMissing,
    /// The plan required rerunning this work and no fresh record was
    /// supplied.
    WorkNotRerun,
    /// A completeness-claiming mode was handed a plan that named no
    /// work at all: completeness over nothing is a false green, not
    /// a proof.
    EmptyWorkPlan,
}

impl ReasonCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ParseError => "parse_error",
            Self::CapabilityMissing => "capability_missing",
            Self::PluginUncovered => "plugin_uncovered",
            Self::ReadSetTruncated => "readset_truncated",
            Self::CacheCorrupt => "cache_corrupt",
            Self::CacheMissing => "cache_missing",
            Self::SnapshotMismatch => "snapshot_mismatch",
            Self::ReuseInvalidated => "reuse_invalidated",
            Self::IdentityMissing => "identity_missing",
            Self::WorkNotRerun => "work_not_rerun",
            Self::EmptyWorkPlan => "empty_work_plan",
        }
    }
}

impl std::fmt::Display for ReasonCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A health observation made while capturing evidence, judged here but
/// not produced here: parse failures, missing backends, uncovered
/// plugins, or a cache that would not decode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptureGap {
    pub code: ReasonCode,
    pub detail: String,
}

/// One blocking issue in a snapshot: the reason code and, when the
/// reason is about one piece of work, the work that carries it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockingIssue {
    pub work: Option<WorkId>,
    pub code: ReasonCode,
    pub detail: String,
}

/// How one piece of work ended up covered in the snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    /// The work was executed fresh against the candidate.
    Reran { findings: Vec<ReboundFinding> },
    /// The cached record survived every reuse layer.
    Reused { findings: Vec<ReboundFinding> },
    /// Nothing covers this work; the codes say why.
    Unresolved { codes: Vec<ReasonCode> },
}

impl Resolution {
    fn is_resolved(&self) -> bool {
        !matches!(self, Self::Unresolved { .. })
    }

    fn findings(&self) -> &[ReboundFinding] {
        match self {
            Self::Reran { findings } | Self::Reused { findings } => findings,
            Self::Unresolved { .. } => &[],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkOutcome {
    pub work: WorkId,
    pub resolution: Resolution,
}

/// The verdict of [`CompleteSnapshot::verdict`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// A completeness-claiming mode, every work resolved, no gaps.
    Pass,
    /// A completeness-claiming mode that failed to cover something.
    NotPass,
    /// A scoped mode: the answer is complete for its scope only and
    /// therefore can never claim the whole tree.
    Scoped,
}

/// The snapshot: what covered each work item, the merged findings, and
/// everything that blocks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteSnapshot {
    pub mode: AnalysisMode,
    pub candidate_snapshot: SnapshotId,
    pub outcomes: Vec<WorkOutcome>,
    /// Findings from resolved work only, merged and sorted by
    /// (fingerprint, produced_in, reported_in). A `NotPass` snapshot
    /// still lists what WAS proven; blocking gaps do not erase it.
    pub findings: Vec<ReboundFinding>,
    pub blocking: Vec<BlockingIssue>,
}

impl CompleteSnapshot {
    pub fn verdict(&self) -> Verdict {
        if !self.mode.claims_complete_coverage() {
            return Verdict::Scoped;
        }
        let all_resolved = self.outcomes.iter().all(|o| o.resolution.is_resolved());
        if self.blocking.is_empty() && all_resolved {
            Verdict::Pass
        } else {
            Verdict::NotPass
        }
    }

    /// The semantic fingerprint set of the merged findings: the
    /// comparison surface for AC #1 — two modes over the same frozen
    /// origin must agree here, modulo nothing.
    pub fn semantic_coverage(&self) -> BTreeSet<String> {
        self.findings
            .iter()
            .map(|f| f.semantic_fingerprint.as_str().to_string())
            .collect()
    }

    /// The stable reason codes this snapshot publishes, in order.
    pub fn reason_codes(&self) -> Vec<&'static str> {
        self.blocking.iter().map(|b| b.code.as_str()).collect()
    }
}

/// Everything the builder needs: the plan (CAA-06), the reuse index
/// and fresh records (CAA-07), the health observed during capture.
///
/// Convention: the string key of each work item IS the detector id
/// string — `WorkId` and `DetectorId` are both validated
/// `namespace.name`, and the reuse index keys its records by the
/// detector id of the run that produced them and the snapshot it ran over.
pub struct CompleteSnapshotRequest<'a> {
    pub mode: AnalysisMode,
    pub plan: &'a AffectedWorkPlan,
    pub candidate_snapshot: SnapshotId,
    /// The transition the plan was computed over. Completeness is
    /// always an assertion about a named transition, so the delta is
    /// required, never optional.
    pub delta: &'a FactDelta,
    pub cache: &'a DetectorReuseIndex,
    /// The identity a rerun of this work would execute under NOW,
    /// keyed by work id string. Required for every work a reuse
    /// decision must survive.
    pub current_identities: &'a BTreeMap<String, DetectorRunIdentity>,
    /// Fresh records for the work that was rerun, keyed by work id
    /// string.
    pub reruns: &'a BTreeMap<String, DetectorRunRecord>,
    pub gaps: &'a [CaptureGap],
}

/// Decode a persisted reuse index, mapping any decode failure to the
/// honest [`ReasonCode::CacheCorrupt`] gap instead of a half-read
/// cache.
pub fn load_reuse_index(text: &str) -> Result<DetectorReuseIndex, CaptureGap> {
    DetectorReuseIndex::from_json(text).map_err(|e| CaptureGap {
        code: ReasonCode::CacheCorrupt,
        detail: e.to_string(),
    })
}

/// Compose the snapshot. Deterministic: the outcome order follows the
/// plan (already sorted by `WorkId`), the merged findings sort by
/// semantic identity, and the blocking issues are the capture gaps
/// first, then per-work failures in plan order.
pub fn build_complete_snapshot(req: &CompleteSnapshotRequest<'_>) -> CompleteSnapshot {
    let mut blocking: Vec<BlockingIssue> = req
        .gaps
        .iter()
        .map(|g| BlockingIssue {
            work: None,
            code: g.code,
            detail: g.detail.clone(),
        })
        .collect();

    // A completeness claim is always about a NAMED transition ending
    // in the candidate itself. If the plan answers a different
    // question, no finding of it may be presented as coverage of
    // this candidate.
    let transition_mismatch =
        req.mode.claims_complete_coverage() && req.plan.to_snapshot != Some(req.candidate_snapshot);
    if transition_mismatch {
        blocking.push(BlockingIssue {
            work: None,
            code: ReasonCode::SnapshotMismatch,
            detail: format!(
                "the plan describes a transition into {:?}, but the candidate is {}",
                req.plan.to_snapshot.map(|s| s.0),
                req.candidate_snapshot.0
            ),
        });
    }
    if req.mode.claims_complete_coverage() && req.plan.decisions.is_empty() {
        blocking.push(BlockingIssue {
            work: None,
            code: ReasonCode::EmptyWorkPlan,
            detail: "the plan named no work; completeness over nothing is a false green"
                .to_string(),
        });
    }

    let mut outcomes = Vec::new();
    for decision in &req.plan.decisions {
        let key = decision.work.as_namespaced_name().as_str();
        let fresh_required =
            req.mode == AnalysisMode::FullRebuild || decision.disposition.must_rerun();
        let (resolution, mut issues) = if fresh_required {
            resolve_fresh(&decision.work, key, req)
        } else {
            resolve_reuse(&decision.work, key, req)
        };
        blocking.append(&mut issues);
        outcomes.push(WorkOutcome {
            work: decision.work.clone(),
            resolution,
        });
    }

    let mut findings: Vec<ReboundFinding> = if transition_mismatch {
        Vec::new()
    } else {
        outcomes
            .iter()
            .flat_map(|o| o.resolution.findings().to_vec())
            .collect()
    };
    findings.sort_by(|a, b| {
        a.semantic_fingerprint
            .as_str()
            .cmp(b.semantic_fingerprint.as_str())
            .then_with(|| a.produced_in.cmp(&b.produced_in))
            .then_with(|| a.reported_in.cmp(&b.reported_in))
    });

    CompleteSnapshot {
        mode: req.mode,
        candidate_snapshot: req.candidate_snapshot,
        outcomes,
        findings,
        blocking,
    }
}

fn issue(work: &WorkId, code: ReasonCode, detail: String) -> BlockingIssue {
    BlockingIssue {
        work: Some(work.clone()),
        code,
        detail,
    }
}

/// A fresh record covers its work only if it names the candidate as
/// its producing snapshot and its lineage is complete. Anything else
/// is an unresolved work item with the exact codes that say why.
fn resolve_fresh(
    work: &WorkId,
    key: &str,
    req: &CompleteSnapshotRequest<'_>,
) -> (Resolution, Vec<BlockingIssue>) {
    let unresolved = |codes: Vec<ReasonCode>| Resolution::Unresolved { codes };
    match req.reruns.get(key) {
        None => (
            unresolved(vec![ReasonCode::WorkNotRerun]),
            vec![issue(
                work,
                ReasonCode::WorkNotRerun,
                format!(
                    "the plan required {key} to run against the candidate; no fresh record was supplied"
                ),
            )],
        ),
        Some(record) => {
            let mut issues = Vec::new();
            if record.produced_snapshot != req.candidate_snapshot {
                issues.push(issue(
                    work,
                    ReasonCode::SnapshotMismatch,
                    format!(
                        "fresh record for {key} was produced in snapshot {}, not the candidate {}",
                        record.produced_snapshot.0, req.candidate_snapshot.0
                    ),
                ));
            }
            if record.observed_truncated {
                issues.push(issue(
                    work,
                    ReasonCode::ReadSetTruncated,
                    format!(
                        "fresh record for {key} has a truncated read-set; lineage proves nothing"
                    ),
                ));
            }
            if !issues.is_empty() {
                let codes = issues.iter().map(|i| i.code).collect();
                return (unresolved(codes), issues);
            }
            (
                Resolution::Reran {
                    findings: rebind_candidate(record, req.candidate_snapshot),
                },
                Vec::new(),
            )
        }
    }
}

/// A planner verdict of `Unaffected` licenses reuse ONLY while the
/// record provably backs the same world the plan diffed from: the
/// same base snapshot, a complete lineage, a provable survival of
/// every identity layer. If reuse dies, a rerun may replace it; if
/// neither exists, the work is unresolved — never skipped.
fn resolve_reuse(
    work: &WorkId,
    key: &str,
    req: &CompleteSnapshotRequest<'_>,
) -> (Resolution, Vec<BlockingIssue>) {
    // The record over the snapshot the plan diffed from. A detector kept
    // only for other bases falls back to its latest record, which the
    // snapshot check below refuses rather than reuses.
    let record = req
        .plan
        .from_snapshot
        .and_then(|from| req.cache.record_for(key, from))
        .or_else(|| req.cache.latest(key));
    let Some(record) = record else {
        return (
            Resolution::Unresolved {
                codes: vec![ReasonCode::CacheMissing],
            },
            vec![issue(
                work,
                ReasonCode::CacheMissing,
                format!(
                    "no cached record for {key}; the planner's non-impact cannot be reused from nothing"
                ),
            )],
        );
    };

    let mut issues = Vec::new();
    if record.observed_truncated {
        issues.push(issue(
            work,
            ReasonCode::ReadSetTruncated,
            format!("cached record for {key} has a truncated read-set; its non-impact is unproven"),
        ));
    }
    if Some(record.produced_snapshot) != req.plan.from_snapshot {
        issues.push(issue(
            work,
            ReasonCode::SnapshotMismatch,
            format!(
                "cached record for {key} was produced in snapshot {:?}, not the plan's base {:?}",
                record.produced_snapshot.0,
                req.plan.from_snapshot.map(|s| s.0)
            ),
        ));
    }
    let current = req.current_identities.get(key);
    if current.is_none() {
        issues.push(issue(
            work,
            ReasonCode::IdentityMissing,
            format!("the identity a rerun of {key} would use is unknown; reuse cannot be proven"),
        ));
    }
    if !issues.is_empty() {
        let codes = issues.iter().map(|i| i.code).collect();
        return (Resolution::Unresolved { codes }, issues);
    }
    let current = current.expect("checked by the IdentityMissing issue above");

    // The planner has already classified this transition as Unaffected using
    // the recorded read boundary. Reapplying the raw delta here would forget
    // boundary-excluded additions and invalidate otherwise safe reuse.
    match reuse_decision(record, current, None) {
        crate::domain::findings::detector_reuse::ReuseDecision::Reusable => (
            Resolution::Reused {
                findings: rebind_candidate(record, req.candidate_snapshot),
            },
            Vec::new(),
        ),
        crate::domain::findings::detector_reuse::ReuseDecision::Invalidated { levels } => {
            if req.reruns.contains_key(key) {
                resolve_fresh(work, key, req)
            } else {
                (
                    Resolution::Unresolved {
                        codes: vec![ReasonCode::ReuseInvalidated],
                    },
                    vec![issue(
                        work,
                        ReasonCode::ReuseInvalidated,
                        format!(
                            "reuse for {key} died at {:?}; no rerun replaced it",
                            levels
                                .iter()
                                .map(|l| match l {
                                    InvalidationLevel::Rule => "rule",
                                    InvalidationLevel::Capability => "capability",
                                    InvalidationLevel::Parser => "parser",
                                    InvalidationLevel::Sanitizer => "sanitizer",
                                    InvalidationLevel::SemanticDependency => "semantic",
                                })
                                .collect::<Vec<_>>()
                        ),
                    )],
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scoped_modes_never_claim_completeness() {
        assert!(!AnalysisMode::Focused.claims_complete_coverage());
        assert!(!AnalysisMode::Impacted.claims_complete_coverage());
        assert!(AnalysisMode::IncrementalComplete.claims_complete_coverage());
        assert!(AnalysisMode::FullRebuild.claims_complete_coverage());
    }

    #[test]
    fn reason_codes_are_stable_strings() {
        assert_eq!(ReasonCode::ParseError.as_str(), "parse_error");
        assert_eq!(ReasonCode::ReadSetTruncated.as_str(), "readset_truncated");
        assert_eq!(ReasonCode::ReuseInvalidated.as_str(), "reuse_invalidated");
        assert_eq!(ReasonCode::WorkNotRerun.as_str(), "work_not_rerun");
    }
}
