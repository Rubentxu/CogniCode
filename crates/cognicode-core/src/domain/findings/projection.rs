//! CU-2.2 — Read-side projection of verified findings.
//!
//! This module is the **seam** between CogniCode (technical authority:
//! what the kernel verified) and CogniCode Guard (admission authority:
//! who signed the rule). It does not transfer authority — Guard still
//! owns `PolicyDecision`, `RuleAdmission`, signatures and receipts.
//!
//! ## Contract (SPEC-CU-003.2 / SPEC-CU-003.3)
//!
//! The projection is **total** and **read-only**:
//!
//! - `Result<FindingProjection, ProjectionError>` is the only return
//!   type. A finding that cannot be projected returns a discriminated
//!   `Unsupported | Inconclusive | Failed` ADT — never a default `Verified`.
//! - Every typed id (`SnapshotId`, `EvidenceId`, `FactId`) is preserved.
//! - `EvidenceId`s are bound to the snapshot they were observed under,
//!   so a consumer cannot promote evidence from a stale snapshot.
//! - `EvidenceClass::A|B` projects to `Verified`. `C` projects to
//!   `Advisory`. `D` (hypothesis / heuristic) projects to `Inconclusive`
//!   unless the caller has overridden the threshold.
//!
//! ## Non-goals
//!
//! - The projection does **not** sign anything. It carries typed ids
//!   that Guard can verify against its own `ApprovalVerifier`.
//! - The projection does **not** introduce a new wire format. The
//!   `assurance-evidence/v1` envelope (consumer: Kotlin DTO) is a
//!   separate, externally-constrained concern. This projection is an
//!   *internal* ADT used by CogniCode's own read-side code paths.

use crate::domain::findings::{CausalStep, EvidenceClass, Finding};
use crate::domain::kernel_ids::{EntityId, EvidenceId, FactId, SnapshotId};

/// The total read-side projection of one verified finding.
///
/// `Verified` is the only variant that is admissible as a `Block` basis
/// in Guard. `Advisory` and `Inconclusive` cannot block. `Unsupported`
/// and `Failed` are honest discriminants: never converted to `Verified`
/// or `[]` by any caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FindingProjection {
    /// Evidence verified, scoped to a specific snapshot, with grounded
    /// causal chain. This is the only variant a consumer (Guard) may
    /// promote to a `Block` decision.
    Verified {
        /// Id under which the evidence was observed. Every `evidence`
        /// handle is bound to this snapshot.
        snapshot_id: SnapshotId,
        /// Detector that produced the finding, with the authority it
        /// held at execution time.
        detector: crate::domain::findings::DetectorExecutionRef,
        /// Severity as the kernel reported it (Critical/Warning/Info).
        severity: crate::domain::findings::FindingSeverity,
        /// Evidence handles, each bound to `snapshot_id`.
        evidence: Vec<TypedEvidenceHandle>,
        /// Grounded causal chain, in order.
        causes: Vec<CauseProjection>,
        /// Human-readable message (must be non-empty per kernel).
        message: String,
    },
    /// Observation is informative; it has no verifiable evidence
    /// (`EvidenceClass::C` or `D` with at least one handle). A consumer
    /// may surface it but never promote it to a `Block`.
    Advisory {
        snapshot_id: SnapshotId,
        detector: crate::domain::findings::DetectorExecutionRef,
        /// Evidence handles, each bound to `snapshot_id`. May be empty
        /// for purely heuristic findings.
        evidence: Vec<TypedEvidenceHandle>,
        message: String,
    },
    /// The finding lacks enough information to decide. The reasons
    /// (gaps) are returned so the consumer can render them.
    Inconclusive {
        snapshot_id: SnapshotId,
        detector: crate::domain::findings::DetectorExecutionRef,
        /// Non-empty: a single gap collapses to itself.
        gaps: Vec<Gap>,
    },
}

/// Discriminated error: the consumer must not infer `Verified` from
/// any of these. `ProjectionError::Unsupported` is for findings that
/// do not belong in the requested projection; `Inconclusive` is
/// transported as `Err` because it carries a `Vec<Gap>` payload
/// and the projection succeeds only when the result is `Verified`
/// or `Advisory`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectionError {
    /// The finding was made on a different snapshot than the one
    /// the projection was asked about. Re-binding to a new snapshot
    /// would silently leak evidence across workspaces; this is the
    /// only honest answer.
    Unsupported(UnknownReason),
    /// The finding is real but lacks enough information to project.
    /// Carries the gap list so the consumer can render why.
    Inconclusive(Vec<Gap>),
    /// A structural failure (e.g. invalid detector reference). The
    /// finding must not be admitted in any way.
    Failed(String),
}

/// Reasons a finding cannot be projected to a usable form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnknownReason {
    /// The finding's `evidence_class` is `Unknown` (e.g. feature flag
    /// `evidence-kernel` was off when the finding was produced).
    EvidenceClassUnknown,
    /// The finding carries no evidence handles; it has no provenance
    /// to project.
    NoEvidence,
    /// The finding's snapshot does not match the one the projection
    /// is about. Carries the observed snapshot id.
    SnapshotMismatch {
        observed: SnapshotId,
        requested: SnapshotId,
    },
}

/// Non-empty list of reasons a finding is `Inconclusive`. The kernel
/// does not collapse them into a single discriminant because each
/// has a different remediation path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Gap {
    /// The evidence class is below the threshold required for a
    /// `Verified` projection (e.g. `D` when the projection requires
    /// `A` or `B`).
    EvidenceClassBelowThreshold { class: EvidenceClass },
    /// A causal step lacks a fact id or evidence id; the chain is
    /// not fully grounded.
    UngroundedCausalStep,
    /// The finding has no evidence handles at all.
    NoEvidenceHandles,
}

/// An `EvidenceId` bound to the snapshot it was observed under.
///
/// The binding is enforced by construction: there is no way to build
/// a `TypedEvidenceHandle` without a `SnapshotId`. Consumers that
/// accept these handles never need to ask "which snapshot is this
/// from?" — the answer is in the value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedEvidenceHandle {
    pub id: EvidenceId,
    pub snapshot: SnapshotId,
}

/// A causal step projected for read-side consumption.
///
/// `Grounded` carries the fact and evidence ids that back the step.
/// `Ungrounded` is a placeholder: it is the caller's responsibility
/// to fail closed if the chain contains any `Ungrounded` step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CauseProjection {
    Grounded {
        kind: crate::domain::findings::CausalStepKind,
        subject: Option<EntityId>,
        fact: Option<FactId>,
        evidence: Option<EvidenceId>,
        detail: String,
    },
    Ungrounded {
        kind: crate::domain::findings::CausalStepKind,
        subject: Option<EntityId>,
        detail: String,
    },
}

/// Project one finding into the read-side ADT.
///
/// This is the **only** function that decides whether a finding is
/// admissible. The decision is based on the kernel's
/// `EvidenceClass` and the snapshot match — no heuristics.
///
/// # Errors
///
/// - `ProjectionError::Unsupported` when the finding is for a
///   different snapshot, has unknown evidence class, or has no
///   evidence. A consumer (Guard) receiving this cannot promote
///   the finding; it must surface the reason.
/// - `ProjectionError::Inconclusive(Vec<Gap>)` is reserved for
///   cases where the projection cannot recover snapshot or detector
///   context (a malformed or synthetic finding). D-class findings
///   with full context are projected as `Ok(Inconclusive {...})`,
///   not as the error variant, so the absence of provenance is a
///   structural discriminant the consumer can rely on.
/// - `ProjectionError::Failed` on structural problems (e.g. an
///   invalid detector reference). This is the only way the
///   projection reports an internal error; the consumer must not
///   infer any policy from it.
pub fn project_finding(
    finding: &Finding,
    requested_snapshot: &SnapshotId,
) -> Result<FindingProjection, ProjectionError> {
    // The finding carries an observed snapshot indirectly through its
    // detector's `context.execution` (the runtime that produced it).
    // A finding without an execution context has no provenance: it
    // could not have been produced by a real run, and the only
    // honest projection is to reject it.
    let observed = match finding.detector.context.as_ref() {
        Some(c) => c.scope.snapshot,
        None => {
            return Err(ProjectionError::Unsupported(
                UnknownReason::EvidenceClassUnknown,
            ));
        }
    };
    if observed != *requested_snapshot {
        return Err(ProjectionError::Unsupported(
            UnknownReason::SnapshotMismatch {
                observed,
                requested: *requested_snapshot,
            },
        ));
    }

    let snapshot_id = *requested_snapshot;

    if finding.evidence.is_empty() {
        return Err(ProjectionError::Unsupported(UnknownReason::NoEvidence));
    }

    // The evidence handles, each bound to the snapshot. The binding
    // is structural: `TypedEvidenceHandle` cannot exist without its
    // snapshot.
    let evidence: Vec<TypedEvidenceHandle> = finding
        .evidence
        .iter()
        .map(|id| TypedEvidenceHandle {
            id: *id,
            snapshot: snapshot_id,
        })
        .collect();

    let causes: Vec<CauseProjection> = finding.causal_chain.iter().map(project_cause).collect();

    let detector = finding.detector.clone();

    match finding.evidence_class {
        EvidenceClass::A | EvidenceClass::B => Ok(FindingProjection::Verified {
            snapshot_id,
            detector,
            severity: finding.severity,
            evidence,
            causes,
            message: finding.message.clone(),
        }),
        EvidenceClass::C => Ok(FindingProjection::Advisory {
            snapshot_id,
            detector,
            evidence,
            message: finding.message.clone(),
        }),
        EvidenceClass::D => Ok(FindingProjection::Inconclusive {
            snapshot_id,
            detector,
            // D-class is below the A|B threshold for Verifiable, but
            // the finding has full provenance: snapshot, detector
            // and evidence are all known. The gap explains the
            // reason; the consumer can render it. H0.4: the Err arm
            // is reserved for "no provenance" cases, not "below
            // threshold with full context".
            gaps: vec![Gap::EvidenceClassBelowThreshold {
                class: finding.evidence_class,
            }],
        }),
    }
}

fn project_cause(step: &CausalStep) -> CauseProjection {
    if step.fact.is_some() || step.evidence.is_some() {
        CauseProjection::Grounded {
            kind: step.kind,
            subject: step.subject,
            fact: step.fact,
            evidence: step.evidence,
            detail: step.detail.clone(),
        }
    } else {
        CauseProjection::Ungrounded {
            kind: step.kind,
            subject: step.subject,
            detail: step.detail.clone(),
        }
    }
}
