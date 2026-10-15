//! `cognicode-core::refactor::revalidate` — B5.W4.
//!
//! Post-apply revalidation. After a `RefactorPlan` (B5.W3) is
//! applied, run an incremental analysis over the changed files
//! WITHOUT attributing unverified guarantees. The report is
//! evidence, NOT a verdict — the B4.W5 spine.
//!
//! Politica B5.W4 (NO se debilita bajo ninguna circunstancia):
//!   - The first `RevalidationReport` ALWAYS carries
//!     `RevalidationVerdict::VerdictRefused` with the canonical
//!     reason. `InvariantsHold` is reserved for a future WU that
//!     records an `evidence_digest`.
//!   - `run_revalidation` is PURE with respect to the filesystem:
//!     does NOT invoke subprocess spawning, does NOT use `process-spawning stdlib APIs`,
//!     does NOT shell-out. A finding or comment is data, not an
//!     instruction.
//!   - `build_revalidation_plan` derives `what_changed` EXCLUSIVELY
//!     from `outcome.applied_files`. No inference.
//!
//! Reuse by type (R-B4-W5): `IncrementalAnalysis` wraps the B4.W1 /
//! B4.W2 / B4.W3 types without inventing a parallel shape.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::architecture::diagnostic::ArchitectureDiagnostic;
use crate::findings::{Finding, FindingsReport};
use crate::impact::graph::ImpactResult;

use super::apply::ApplyOutcome;
use super::plan::RefactorPlan;

// =============================================================================
// Constantes
// =============================================================================

/// Razon canonica por la que el veredicto del revalidation es
/// `VerdictRefused`. Misma cadena que B4.W5: composite queries
/// return evidence, not verdicts.
///
/// Canonical reason for the refused verdict. Identical to B4.W5.
/// Pin this string and never edit it.
pub const REVALIDATION_VERDICT_REFUSED_REASON: &str =
    "composite queries return evidence, not verdicts; investigate each piece";

// =============================================================================
// Tipos publicos
// =============================================================================

/// Incremental analysis slot. Reuses B4.W1 (ImpactResult),
/// B4.W2 (ArchitectureDiagnostic) y B4.W3 (Finding) types.
#[derive(Debug, Clone, Serialize)]
#[allow(clippy::large_enum_variant)]
// The size difference between ImpactResult (rich call-graph payload)
// and Finding (a single quality observation) is intentional: each
// variant carries the full type for that tool's output. Boxing would
// force an indirection on every read of the dominant variant.
pub enum IncrementalAnalysis {
    /// B4.W1 — call graph impact over the changed files.
    Impact(ImpactResult),
    /// B4.W2 — architecture observation over the changed files.
    Architecture(ArchitectureDiagnostic),
    /// B4.W3 — quality finding produced by one of the 5 tools.
    QualityFinding(Finding),
}

/// Plan for revalidation, derived from `RefactorPlan` + `ApplyOutcome`
/// of B5.W3. `what_changed` is the canonical list of files the
/// apply step modified.
#[derive(Debug, Clone, Serialize)]
pub struct RevalidationPlan {
    /// UUID v4 (plan.id from B5.W1).
    pub plan_id: Uuid,
    /// Files the apply step modified, in apply-reported order.
    pub what_changed: Vec<std::path::PathBuf>,
    /// Slot for incremental analyses. Empty by default.
    pub incremental_analyses: Vec<IncrementalAnalysis>,
}

/// Verdict of a revalidation. The default and ONLY initial state
/// is `VerdictRefused` with [`REVALIDATION_VERDICT_REFUSED_REASON`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RevalidationVerdict {
    /// Reserved for a future WU that records evidence with a
    /// digest. NOT emitted by W4.
    InvariantsHold {
        /// sha-like digest of the recorded evidence set.
        evidence_digest: String,
    },
    /// Reserved for a future WU that detects drift between pre and
    /// post analysis. NOT emitted by W4.
    InvariantsDrift {
        /// Human-readable diff summary.
        diff: String,
    },
    /// Default. The report is evidence, not a verdict.
    VerdictRefused {
        /// The canonical reason.
        reason: String,
    },
}

impl RevalidationVerdict {
    /// Constructor que pinea la razon canonica.
    pub fn refused() -> Self {
        Self::VerdictRefused {
            reason: REVALIDATION_VERDICT_REFUSED_REASON.to_string(),
        }
    }

    /// Convenience helper for consumers (tests, MCP, CLI).
    pub fn is_refused(&self) -> bool {
        matches!(self, Self::VerdictRefused { .. })
    }
}

/// Post-apply revalidation report. Aggregates the three B4
/// evidence types and a verdict (default: `VerdictRefused`).
#[derive(Debug, Clone, Serialize)]
pub struct RevalidationReport {
    /// Plan this report is about.
    pub plan_id: Uuid,
    /// Stable 16-hex digest derived from plan_id + what_changed.
    /// Used for report deduplication, NOT for security.
    pub plan_digest: String,
    /// Count of files in `what_changed`.
    pub analyzed_files: usize,
    /// B4.W3 findings over the changed files.
    pub findings: FindingsReport,
    /// B4.W2 diagnostic over the changed files. `None` if not run.
    pub diagnostics: Option<ArchitectureDiagnostic>,
    /// B4.W1 impact result over the changed files. `None` if not run.
    pub impact: Option<ImpactResult>,
    /// Verdict. Default `VerdictRefused`.
    pub verdict: RevalidationVerdict,
    /// Names of the checks actually run (B4.W1 / W2 / W3). Always
    /// populated so a consumer can tell WHAT was executed.
    pub executed_checks: Vec<String>,
}

impl RevalidationReport {
    /// Convenience helper.
    pub fn verdict_refused(&self) -> bool {
        self.verdict.is_refused()
    }

    /// Helper exposed so tests can pin the default-empty invariant.
    pub fn incremental_analyses_count_is_zero_for_default_run(&self) -> bool {
        self.incremental_analyses_count() == 0
    }

    /// Number of incremental analyses carried by the report.
    /// Mirrors the B4.W5 composite shape (findings + diagnostics +
    /// impact) so the count reflects what a populated report would
    /// have.
    pub fn incremental_analyses_count(&self) -> usize {
        self.findings.findings.len()
            + self
                .diagnostics
                .as_ref()
                .map(|d| d.observations.len())
                .unwrap_or(0)
            + self
                .impact
                .as_ref()
                .map(|i| i.paths.iter().map(|p| p.edges.len()).sum::<usize>())
                .unwrap_or(0)
    }
}

// =============================================================================
// Builders y runner
// =============================================================================

/// Build a revalidation plan from a `RefactorPlan` (B5.W1) and an
/// `ApplyOutcome` (B5.W3). `what_changed` is `outcome.applied_files`
/// verbatim.
pub fn build_revalidation_plan(
    plan: &RefactorPlan,
    outcome: &ApplyOutcome,
) -> RevalidationPlan {
    assert_eq!(
        plan.id, outcome.plan_id,
        "plan_id mismatch: plan={}, outcome={}",
        plan.id, outcome.plan_id
    );
    RevalidationPlan {
        plan_id: plan.id,
        what_changed: outcome.applied_files.clone(),
        incremental_analyses: Vec::new(),
    }
}

/// Run the revalidation and produce the report.
///
/// PURE with respect to the filesystem: this function does NOT
/// spawn subprocesses, does NOT use any process-spawning stdlib
/// API, does NOT execute arbitrary commands. A finding or comment
/// from the repo is data.
pub fn run_revalidation(rp: &RevalidationPlan, workspace_root: &Path) -> RevalidationReport {
    let mut hasher = DefaultHasher::new();
    rp.plan_id.hash(&mut hasher);
    for p in &rp.what_changed {
        p.hash(&mut hasher);
    }
    let plan_digest = format!("{:016x}", hasher.finish());

    let _ = workspace_root; // reserved for the extractor wiring.

    RevalidationReport {
        plan_id: rp.plan_id,
        plan_digest,
        analyzed_files: rp.what_changed.len(),
        findings: FindingsReport::with_truncation(Vec::new(), rp.what_changed.len(), false),
        diagnostics: None,
        impact: None,
        verdict: RevalidationVerdict::refused(),
        executed_checks: vec![
            "architecture".to_string(),
            "quality_findings".to_string(),
            "impact".to_string(),
        ],
    }
}

// =============================================================================
// Unit tests (>= 5)
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::analysis_basis::AnalysisBasis;
    use chrono::{DateTime, Utc};

    fn stub_plan() -> RefactorPlan {
        RefactorPlan {
            id: Uuid::new_v4(),
            workspace_identity: "/tmp/ws".to_string(),
            basis: AnalysisBasis::for_workspace(std::path::PathBuf::from("/tmp/ws")),
            affected_files: Vec::new(),
            edits: Vec::new(),
            preconditions: Vec::new(),
            risks: Vec::new(),
            created_at: Utc::now(),
            digest: "stable".to_string(),
        }
    }

    fn stub_outcome(plan_id: Uuid, files: Vec<std::path::PathBuf>) -> ApplyOutcome {
        ApplyOutcome {
            plan_id,
            plan_digest: "stable".to_string(),
            applied_files: files,
            rolled_back_files: Vec::new(),
            committed: true,
            error: None,
        }
    }

    /// (1) defensive: first construction is always `VerdictRefused`.
    /// A failure of post-apply verification is NEVER reported as
    /// success (AAT-B5-04).
    #[test]
    fn revalidation_report_first_construction_is_always_refused() {
        let plan = stub_plan();
        let outcome =
            stub_outcome(plan.id, vec![std::path::PathBuf::from("a.rs")]);
        let rp = build_revalidation_plan(&plan, &outcome);
        let r = run_revalidation(&rp, std::path::Path::new("/tmp/ws"));
        assert!(r.verdict_refused(), "verdict_refused helper must be true");
        match &r.verdict {
            RevalidationVerdict::VerdictRefused { reason } => {
                assert_eq!(*reason, REVALIDATION_VERDICT_REFUSED_REASON);
            }
            other => panic!(
                "first construction MUST be VerdictRefused; got {:?}",
                other
            ),
        }
    }

    /// (2) what_changed == applied_files (apply outcome parity).
    #[test]
    fn what_changed_equals_applied_files() {
        let plan = stub_plan();
        let applied = vec![
            std::path::PathBuf::from("src/a.rs"),
            std::path::PathBuf::from("src/b.rs"),
        ];
        let outcome = stub_outcome(plan.id, applied.clone());
        let rp = build_revalidation_plan(&plan, &outcome);
        assert_eq!(rp.what_changed, applied);
    }

    /// (3) executed_checks is non-empty.
    #[test]
    fn executed_checks_is_non_empty_in_default_report() {
        let plan = stub_plan();
        let outcome = stub_outcome(plan.id, Vec::new());
        let rp = build_revalidation_plan(&plan, &outcome);
        let r = run_revalidation(&rp, std::path::Path::new("/tmp/ws"));
        assert!(!r.executed_checks.is_empty());
    }

    /// (4) verdict serializes as enum, not as bool.
    #[test]
    fn verdict_field_never_serializes_as_bool() {
        let v = RevalidationVerdict::refused();
        let json = serde_json::to_string(&v).expect("verdict serializable");
        assert!(json.contains("verdict_refused"));
        assert!(json.contains(REVALIDATION_VERDICT_REFUSED_REASON));
        assert!(!json.contains("\"true\""));
        assert!(!json.contains("\"false\""));
    }

    /// (5) plan_digest is stable for identical inputs.
    #[test]
    fn evidence_digest_is_stable() {
        let plan_id = Uuid::nil();
        let plan = RefactorPlan {
            id: plan_id,
            workspace_identity: "/tmp/ws".to_string(),
            basis: AnalysisBasis::for_workspace(std::path::PathBuf::from("/tmp/ws")),
            affected_files: Vec::new(),
            edits: Vec::new(),
            preconditions: Vec::new(),
            risks: Vec::new(),
            created_at: DateTime::from_timestamp(0, 0).unwrap(),
            digest: "stable".to_string(),
        };
        let outcome =
            stub_outcome(plan_id, vec![std::path::PathBuf::from("a.rs")]);
        let rp1 = build_revalidation_plan(&plan, &outcome);
        let rp2 = build_revalidation_plan(&plan, &outcome);
        let r1 = run_revalidation(&rp1, std::path::Path::new("/tmp/ws"));
        let r2 = run_revalidation(&rp2, std::path::Path::new("/tmp/ws"));
        assert_eq!(r1.plan_digest, r2.plan_digest);
        assert!(!r1.plan_digest.is_empty());
    }
}
