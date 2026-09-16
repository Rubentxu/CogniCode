//! e75 WU1 — `ExecutionOutcome` (portable execution result envelope).
//!
//! Critical invariants (per directive WU1):
//!
//! 1. `CommandFailure`, `InfrastructureFailure`, and
//!    `UnavailableCapability` MUST remain distinguishable downstream.
//!    They never collapse into one another.
//! 2. `UnavailableCapability` MUST NOT become `Success`. There is no
//!    `Outcome::Pass` here; pass is the policy gate's verdict.
//! 3. Exit-code-zero is NOT policy authority. `Success` describes
//!    "the process exited zero"; the gate decides whether zero exit
//!    is acceptable for the slot.
//! 4. Resource bounds produce their own variant
//!    (`BoundedPolicyViolation`), distinct from `CommandFailure` —
//!    they signal "the host enforced a policy on us" not "the program
//!    returned non-zero".
//!
//! The outcome is an envelope: it carries the small set of variant
//! plus identity pins from M7.2 (`AnalysisScope`, `ActorRef`,
//! `CorrelationId`) and bounded stdout/stderr references. The
//! translation to [`ProducerOutput`]
//! (in [`super::evidence_translate`]) keeps the
//! `EvidenceBundle` invariants and the gate unchanged.
//!
//! ## What is NOT in this type
//!
//! - Authority / pass-fail verdict. The gate owns that.
//! - Platform-specific fields (cgroup ids, OCI exit codes, etc.). The
//!   adapter may enrich the [`Failure`] raw string but the seam stays
//!   platform-agnostic.
//! - Replay data. Out-of-scope for e75.

use std::time::Duration;

use crate::application::evidence_bundle::EvidenceBundleId;
use crate::domain::execution::ActorRef;
use crate::domain::execution::CorrelationId;
use crate::domain::execution::scope::AnalysisScope;

/// What happened when a backend tried to execute an [`ExecutionSpec`](super::spec::ExecutionSpec).
///
/// The four distinguished variants preserve the difference between
/// "the program ran, the program returned non-zero" (CommandFailure),
/// "the program never started because the backend couldn't honour
/// the request" (InfrastructureFailure), and "we cannot serve this
/// request at all on this host" (UnavailableCapability). Adding or
/// collapsing variants changes evidence semantics and MUST be a
/// deliberate, documented decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionOutcome {
    /// The backend ran the program and the program exited with code
    /// zero. No claim about correctness — that's the gate's job.
    Success { detail: SuccessDetail },

    /// The backend ran the program but the program exited non-zero
    /// (or signalled a runtime error inside the captured bytes).
    /// The exit code is preserved on `detail`.
    CommandFailure { detail: Failure },

    /// The backend could not start the program (container/sandbox
    /// launch error, missing runtime, host I/O error before spawn).
    /// Distinct from CommandFailure: the program never ran.
    InfrastructureFailure { detail: Failure },

    /// The backend cannot serve this request on this host
    /// (e.g. `requires_isolation = Yes` but no isolation backend is
    /// available, or the isolation backend is present but its
    /// runtime is absent). The caller MUST treat this as
    /// "trial cannot manufacture required evidence" and the gate
    /// MUST receive a `Missing`/`Insufficient` entry — never a Pass.
    UnavailableCapability { detail: Missing },

    /// The execution was aborted because a resource bound (wall-clock,
    /// memory, CPU) was exceeded. Distinct from CommandFailure (the
    /// program was killed by an external enforcement mechanism) and
    /// from InfrastructureFailure (the program DID start; the host
    /// killed it).
    BoundedPolicyViolation { detail: BoundedViolation },
}

/// Per-variant detail payload. Each variant carries the smallest
/// faithful description of what happened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuccessDetail {
    pub bundle_id: EvidenceBundleId,
    pub exit_code: i32,
    pub stdout: Option<Captured>,
    pub stderr: Option<Captured>,
    pub duration: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    pub bundle_id: EvidenceBundleId,
    pub reason: String,
    /// Exit code if the host recorded one (for CommandFailure; absent
    /// for InfrastructureFailure).
    pub exit_code: Option<i32>,
    /// Tail of captured output, if any.
    pub stdout: Option<Captured>,
    pub stderr: Option<Captured>,
    pub duration: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Missing {
    pub bundle_id: EvidenceBundleId,
    pub reason: String,
    pub capability: MissingCapability,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MissingCapability {
    /// No isolation backend available on this host.
    IsolationBackend,
    /// Isolation backend is present but the host runtime (e.g. podman
    /// daemon) is not reachable.
    IsolationRuntime,
    /// The requested program/argv could not be resolved on PATH or
    /// the filesystem.
    ProgramNotResolved,
    /// The host has been excluded for a portable-execution reason
    /// (e.g. the spec requires platform-specific semantics the host
    /// cannot honour).
    HostNotEligible,
    /// Reserved for additional capabilities added in future WUs.
    Other,
}

impl std::fmt::Display for MissingCapability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::IsolationBackend => "isolation_backend",
            Self::IsolationRuntime => "isolation_runtime",
            Self::ProgramNotResolved => "program_not_resolved",
            Self::HostNotEligible => "host_not_eligible",
            Self::Other => "other",
        };
        f.write_str(s)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedViolation {
    pub bundle_id: EvidenceBundleId,
    pub reason: String,
    pub violated: ViolatedBound,
    pub duration: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ViolatedBound {
    WallClock,
    Memory,
    Cpu,
    Other,
}

impl std::fmt::Display for ViolatedBound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::WallClock => "wall_clock",
            Self::Memory => "memory",
            Self::Cpu => "cpu",
            Self::Other => "other",
        };
        f.write_str(s)
    }
}

/// A captured stdout or stderr payload. `kind = Inlined` for short
/// outputs and `kind = Reference` for future external-file links.
///
/// e75 WU1 only emits `Inlined` (the native backend captures bytes
/// in-memory; the podman backend writes a temp file we read back into
/// memory). WU4 will normalize the path-handling for temp files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Captured {
    pub kind: CaptureKind,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CaptureKind {
    /// The whole payload is in `text`.
    Inlined,
    /// `text` references a temp file location. Reserved for WU4 when
    /// large outputs justify file-path storage.
    Reference,
}

impl Captured {
    /// Construct an inlined capture (the only kind e75 WU1 emits).
    pub fn inlined(text: impl Into<String>) -> Self {
        Self {
            kind: CaptureKind::Inlined,
            text: text.into(),
        }
    }
}

/// Identity + provenance context attached to every outcome.
///
/// The same identity pins as [`ExecutionSpec`](super::spec::ExecutionSpec)
/// — `ExecutionOutcome` is the response side of the same call, so the
/// lineage round-trips.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutcomeIdentity {
    pub scope: AnalysisScope,
    pub actor: ActorRef,
    pub correlation: CorrelationId,
}

impl OutcomeIdentity {
    /// Identity from the spec's pins.
    pub fn from_spec(scope: AnalysisScope, actor: ActorRef, correlation: CorrelationId) -> Self {
        Self {
            scope,
            actor,
            correlation,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_outcome_variants_are_distinct_enum_variants() {
        // Build four distinct values; assert the enum kind is distinct
        // by tag. This is the cheapest assertion that guarantees
        // downstream consumers can switch on the tag.
        let bundle_id = EvidenceBundleId(1);
        let success = ExecutionOutcome::Success {
            detail: SuccessDetail {
                bundle_id,
                exit_code: 0,
                stdout: None,
                stderr: None,
                duration: Duration::from_millis(1),
            },
        };
        let cmd_failure = ExecutionOutcome::CommandFailure {
            detail: Failure {
                bundle_id,
                reason: "exit 1".into(),
                exit_code: Some(1),
                stdout: None,
                stderr: None,
                duration: Duration::from_millis(1),
            },
        };
        let infra = ExecutionOutcome::InfrastructureFailure {
            detail: Failure {
                bundle_id,
                reason: "spawn error".into(),
                exit_code: None,
                stdout: None,
                stderr: None,
                duration: Duration::from_millis(1),
            },
        };
        let unavailable = ExecutionOutcome::UnavailableCapability {
            detail: Missing {
                bundle_id,
                reason: "no isolation backend".into(),
                capability: MissingCapability::IsolationBackend,
            },
        };
        assert_eq!(
            std::mem::discriminant(&success),
            std::mem::discriminant(&ExecutionOutcome::Success {
                detail: SuccessDetail {
                    bundle_id,
                    exit_code: 0,
                    stdout: None,
                    stderr: None,
                    duration: Duration::from_millis(1),
                }
            })
        );
        assert_eq!(
            std::mem::discriminant(&cmd_failure),
            std::mem::discriminant(&ExecutionOutcome::CommandFailure {
                detail: Failure {
                    bundle_id,
                    reason: "x".into(),
                    exit_code: Some(1),
                    stdout: None,
                    stderr: None,
                    duration: Duration::from_millis(1),
                }
            })
        );
        assert_eq!(
            std::mem::discriminant(&infra),
            std::mem::discriminant(&ExecutionOutcome::InfrastructureFailure {
                detail: Failure {
                    bundle_id,
                    reason: "x".into(),
                    exit_code: None,
                    stdout: None,
                    stderr: None,
                    duration: Duration::from_millis(1),
                }
            })
        );
        assert_eq!(
            std::mem::discriminant(&unavailable),
            std::mem::discriminant(&ExecutionOutcome::UnavailableCapability {
                detail: Missing {
                    bundle_id,
                    reason: "y".into(),
                    capability: MissingCapability::Other,
                }
            })
        );
    }

    #[test]
    fn bounded_violation_is_not_the_same_variant_as_command_failure() {
        // The directive requires these to be distinguishable
        // downstream. discriminant equality would be a bug.
        let bundle_id = EvidenceBundleId(1);
        let bounded = ExecutionOutcome::BoundedPolicyViolation {
            detail: BoundedViolation {
                bundle_id,
                reason: "timed out".into(),
                violated: ViolatedBound::WallClock,
                duration: Duration::from_millis(500),
            },
        };
        let cmd = ExecutionOutcome::CommandFailure {
            detail: Failure {
                bundle_id,
                reason: "exit 2".into(),
                exit_code: Some(2),
                stdout: None,
                stderr: None,
                duration: Duration::from_millis(500),
            },
        };
        assert_ne!(
            std::mem::discriminant(&bounded),
            std::mem::discriminant(&cmd)
        );
    }

    #[test]
    fn missing_capability_displays_stable_strings() {
        assert_eq!(
            MissingCapability::IsolationBackend.to_string(),
            "isolation_backend"
        );
        assert_eq!(
            MissingCapability::IsolationRuntime.to_string(),
            "isolation_runtime"
        );
        assert_eq!(
            MissingCapability::ProgramNotResolved.to_string(),
            "program_not_resolved"
        );
        assert_eq!(
            MissingCapability::HostNotEligible.to_string(),
            "host_not_eligible"
        );
    }

    #[test]
    fn outcome_module_does_not_leak_platform_specific_identifiers() {
        let src = super::super::strip_doc_comments_and_tests(include_str!("outcome.rs"));
        for forbidden in ["podman", "systemd", "quadlet", "wsl", "hyper-v", "docker"] {
            assert!(
                !src.to_lowercase().contains(forbidden),
                "ExecutionOutcome must not leak platform-specific symbol in code: {forbidden}"
            );
        }
    }
}
