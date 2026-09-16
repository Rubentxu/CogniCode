//! e75 WU1 — `ExecutionOutcome → Vec<ProducerOutput>` translation.
//!
//! This is the bridge from the portable seam ([`ExecutionOutcome`])
//! back into the e69 evidence vocabulary so that the gate (e69 WU3),
//! the bundle (e69 WU1), and the planner (e68 WU2) see a stream they
//! already understand.
//!
//! ## Mapping
//!
//! | Outcome variant           | Producer `Outcome`     | Notes |
//! |---------------------------|------------------------|-------|
//! | `Success`                 | `Evidence`             | One `ProducerOutput` with `FactSlot::Dangling` descriptor — "evidence that execution occurred", NOT a graded canonical fact. The gate rejects `Dangling` against required slots, preserving the no-silent-pass invariant. |
//! | `CommandFailure`          | `Failed`               | The program's non-zero exit. |
//! | `InfrastructureFailure`   | `Failed`               | Distinct origin (spawn never happened); origin preserved in `reason`. |
//! | `UnavailableCapability`   | `Missing`              | Distinct from `Failed`. The gate MUST treat `Missing` against a required rule as `InsufficientEvidence`. |
//! | `BoundedPolicyViolation`  | `Failed`               | The host enforced a bound. The `reason` preserves the violated bound. |
//!
//! ## Critical invariants preserved
//!
//! - Exit-code-zero NEVER carries authority: `Success` produces an
//!   `Outcome::Evidence` with `FactSlot::Dangling`, which the gate
//!   refuses against a slot rule (this is e69 WU3's existing
//!   fail-closed behaviour — unchanged).
//! - `UnavailableCapability` never produces `Evidence`. There is no
//!   path that turns "I cannot serve this" into "the trial produced
//!   a pass".
//!
//! ## What this module does NOT do
//!
//! - It does NOT consult a producer source registry. Slot resolution
//!   is the caller's job (`translate(...)` takes a `slot_for`
//!   resolver closure).
//! - It does NOT mint a `BundleEntry`. The output is `Vec<ProducerOutput>`
//!   because the aggregator is what produces the bundle.

use crate::application::evidence_bundle::{Outcome, ProducerOutput, ProducerSlot};
use crate::application::portable_execution::outcome::{
    BoundedViolation, Captured, ExecutionOutcome, Failure, Missing,
};
use crate::application::portable_execution::spec::ExecutionSpec;
use crate::domain::evidence_kernel::ids::{EvidenceId, FactId};
use crate::domain::findings::ports::{EvidenceDescriptor, FactSlot};
use crate::domain::kernel_ids::EvidenceGrade;

/// Translate an [`ExecutionOutcome`] into the e69 producer stream.
///
/// `slot_for` resolves the slot for a given spec. The composition uses
/// one slot per spec; tests can inject a per-spec resolver to assert
/// routing. The same slot is reused across all variants so the gate
/// can locate the rule.
pub fn translate(
    outcome: ExecutionOutcome,
    spec: &ExecutionSpec,
    slot_for: &dyn Fn(&ExecutionSpec) -> ProducerSlot,
) -> Vec<ProducerOutput> {
    let slot = slot_for(spec);
    vec![producer_for(slot, outcome, &spec.work_id.to_string())]
}

fn producer_for(slot: ProducerSlot, outcome: ExecutionOutcome, work_id: &str) -> ProducerOutput {
    match outcome {
        ExecutionOutcome::Success { detail } => {
            // The Success variant maps to Evidence with a Dangling
            // descriptor: "evidence that execution occurred". The
            // gate rejects Required rules against Dangling facts.
            // Captured stdout/stderr live on `detail` for the audit
            // log, not for the gate.
            let _ = detail;
            ProducerOutput {
                slot,
                outcome: Outcome::Evidence {
                    descriptor: EvidenceDescriptor {
                        id: EvidenceId(0),
                        grade: EvidenceGrade::Supports,
                        fact: FactSlot::Dangling { id: FactId(0) },
                    },
                },
            }
        }
        ExecutionOutcome::CommandFailure { detail } => ProducerOutput {
            slot,
            outcome: Outcome::Failed {
                reason: command_failure_reason(&detail, work_id),
                raw: Some(failure_raw(&detail)),
            },
        },
        ExecutionOutcome::InfrastructureFailure { detail } => ProducerOutput {
            slot,
            outcome: Outcome::Failed {
                reason: infrastructure_failure_reason(&detail, work_id),
                raw: Some(failure_raw(&detail)),
            },
        },
        ExecutionOutcome::UnavailableCapability { detail } => ProducerOutput {
            slot,
            outcome: Outcome::Missing {
                why_unreachable: missing_reason(&detail, work_id),
            },
        },
        ExecutionOutcome::BoundedPolicyViolation { detail } => ProducerOutput {
            slot,
            outcome: Outcome::Failed {
                reason: bounded_reason(&detail, work_id),
                raw: Some(bounded_raw(&detail, work_id)),
            },
        },
    }
}

fn command_failure_reason(detail: &Failure, work_id: &str) -> String {
    let code = detail.exit_code.unwrap_or(-1);
    format!(
        "command_failure: program exited {} (work={}): {}",
        code, work_id, detail.reason
    )
}

fn infrastructure_failure_reason(detail: &Failure, work_id: &str) -> String {
    format!(
        "infrastructure_failure: backend could not start (work={}): {}",
        work_id, detail.reason
    )
}

fn missing_reason(detail: &Missing, work_id: &str) -> String {
    format!(
        "unavailable_capability: {} (work={}): {}",
        detail.capability, work_id, detail.reason
    )
}

fn bounded_reason(detail: &BoundedViolation, work_id: &str) -> String {
    format!(
        "bounded_policy_violation: {} (work={}): {}",
        detail.violated, work_id, detail.reason
    )
}

fn failure_raw(detail: &Failure) -> String {
    let mut raw = String::new();
    if let Some(code) = detail.exit_code {
        raw.push_str(&format!("exit={}\n", code));
    }
    raw.push_str(&format!("duration={}ms\n", detail.duration.as_millis()));
    append_captured(&mut raw, "stdout", detail.stdout.as_ref());
    append_captured(&mut raw, "stderr", detail.stderr.as_ref());
    raw
}

fn bounded_raw(detail: &BoundedViolation, work_id: &str) -> String {
    let mut raw = String::new();
    raw.push_str(&format!("duration={}ms\n", detail.duration.as_millis()));
    raw.push_str(&bounded_reason(detail, work_id));
    raw.push('\n');
    raw
}

fn append_captured(buf: &mut String, label: &str, captured: Option<&Captured>) {
    if let Some(c) = captured {
        buf.push_str(&format!("--- {} ---\n", label));
        buf.push_str(&c.text);
        if !c.text.ends_with('\n') {
            buf.push('\n');
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::change_tracking::planner::WorkId;
    use crate::application::evidence_bundle::{EvidenceBundleId, ProducerSource};
    use crate::application::portable_execution::outcome::{
        BoundedViolation, Captured, ExecutionOutcome, Failure, Missing, MissingCapability,
        SuccessDetail, ViolatedBound,
    };
    use crate::application::portable_execution::spec::{ExecutionSpec, RequiresIsolation};
    use crate::domain::naming::NamespacedName;
    use std::path::PathBuf;
    use std::time::Duration;

    fn spec() -> ExecutionSpec {
        ExecutionSpec::try_new(
            WorkId::new(NamespacedName::new("ci.demo").unwrap()),
            "/bin/true",
            vec![],
            PathBuf::from("/tmp"),
            "corr-1",
        )
        .expect("spec")
    }

    fn slot() -> ProducerSlot {
        ProducerSlot {
            source: ProducerSource::CargoTest,
            slot_id: "e75::demo".into(),
        }
    }

    #[test]
    fn success_translates_to_evidence_with_dangling_fact_so_gate_remains_fail_closed() {
        let out = ExecutionOutcome::Success {
            detail: SuccessDetail {
                bundle_id: EvidenceBundleId(1),
                exit_code: 0,
                stdout: Some(Captured::inlined("hi")),
                stderr: Some(Captured::inlined("")),
                duration: Duration::from_millis(7),
            },
        };
        let slot_fn = |_: &ExecutionSpec| slot();
        let ps = translate(out, &spec(), &slot_fn);
        assert_eq!(ps.len(), 1);
        match &ps[0].outcome {
            Outcome::Evidence { descriptor } => {
                assert_eq!(descriptor.id, EvidenceId(0));
                assert!(matches!(descriptor.fact, FactSlot::Dangling { .. }));
            }
            other => panic!("expected Evidence, got {:?}", other),
        }
    }

    #[test]
    fn command_failure_is_a_failed_outcome_with_distinct_reason() {
        let out = ExecutionOutcome::CommandFailure {
            detail: Failure {
                bundle_id: EvidenceBundleId(2),
                reason: "non-zero".into(),
                exit_code: Some(2),
                stdout: Some(Captured::inlined("out")),
                stderr: Some(Captured::inlined("err")),
                duration: Duration::from_millis(3),
            },
        };
        let slot_fn = |_: &ExecutionSpec| slot();
        let ps = translate(out, &spec(), &slot_fn);
        match &ps[0].outcome {
            Outcome::Failed { reason, raw } => {
                assert!(reason.contains("command_failure"));
                assert!(reason.contains("program exited 2"));
                assert!(raw.is_some());
                assert!(raw.as_ref().unwrap().contains("exit=2"));
            }
            other => panic!("expected Failed, got {:?}", other),
        }
    }

    #[test]
    fn infrastructure_failure_label_is_distinguishable_in_reason() {
        let out = ExecutionOutcome::InfrastructureFailure {
            detail: Failure {
                bundle_id: EvidenceBundleId(3),
                reason: "spawn error".into(),
                exit_code: None,
                stdout: None,
                stderr: Some(Captured::inlined("oops")),
                duration: Duration::from_millis(1),
            },
        };
        let slot_fn = |_: &ExecutionSpec| slot();
        let ps = translate(out, &spec(), &slot_fn);
        match &ps[0].outcome {
            Outcome::Failed { reason, .. } => {
                assert!(reason.contains("infrastructure_failure"));
                assert!(reason.contains("backend could not start"));
            }
            other => panic!("expected Failed, got {:?}", other),
        }
    }

    #[test]
    fn unavailable_capability_translates_to_missing_not_evidence() {
        let out = ExecutionOutcome::UnavailableCapability {
            detail: Missing {
                bundle_id: EvidenceBundleId(4),
                reason: "no isolation backend".into(),
                capability: MissingCapability::IsolationBackend,
            },
        };
        let slot_fn = |_: &ExecutionSpec| slot();
        let ps = translate(out, &spec(), &slot_fn);
        match &ps[0].outcome {
            Outcome::Missing { why_unreachable } => {
                assert!(why_unreachable.contains("unavailable_capability"));
                assert!(why_unreachable.contains("isolation_backend"));
            }
            other => panic!(
                "expected Missing (NOT Evidence — the directive forbids it): got {:?}",
                other
            ),
        }
    }

    #[test]
    fn bounded_policy_violation_is_a_failed_outcome_with_violated_bound_in_reason() {
        let out = ExecutionOutcome::BoundedPolicyViolation {
            detail: BoundedViolation {
                bundle_id: EvidenceBundleId(5),
                reason: "exceeded 500ms".into(),
                violated: ViolatedBound::WallClock,
                duration: Duration::from_millis(500),
            },
        };
        let slot_fn = |_: &ExecutionSpec| slot();
        let ps = translate(out, &spec(), &slot_fn);
        match &ps[0].outcome {
            Outcome::Failed { reason, raw } => {
                assert!(reason.contains("bounded_policy_violation"));
                assert!(reason.contains("wall_clock"));
                assert!(raw.is_some());
            }
            other => panic!("expected Failed, got {:?}", other),
        }
    }

    #[test]
    fn unavailable_never_becomes_evidence_even_if_spec_is_isolation_required() {
        let isolated = spec().with_isolation(RequiresIsolation::Yes);
        let out = ExecutionOutcome::UnavailableCapability {
            detail: Missing {
                bundle_id: EvidenceBundleId(6),
                reason: "no isolation".into(),
                capability: MissingCapability::IsolationBackend,
            },
        };
        let slot_fn = |_: &ExecutionSpec| slot();
        let ps = translate(out, &isolated, &slot_fn);
        assert!(matches!(&ps[0].outcome, Outcome::Missing { .. }));
    }

    #[test]
    fn translater_does_not_leak_platform_specific_identifiers() {
        let src = super::super::strip_doc_comments_and_tests(include_str!("evidence_translate.rs"));
        for forbidden in ["podman", "systemd", "quadlet", "wsl", "hyper-v", "docker"] {
            assert!(
                !src.to_lowercase().contains(forbidden),
                "evidence_translate must not leak platform-specific symbol in code: {forbidden}"
            );
        }
    }

    #[test]
    fn failure_raw_includes_stdout_and_stderr_when_present() {
        let out = ExecutionOutcome::CommandFailure {
            detail: Failure {
                bundle_id: EvidenceBundleId(7),
                reason: "boom".into(),
                exit_code: Some(1),
                stdout: Some(Captured::inlined("hello")),
                stderr: Some(Captured::inlined("world")),
                duration: Duration::from_millis(4),
            },
        };
        let slot_fn = |_: &ExecutionSpec| slot();
        let ps = translate(out, &spec(), &slot_fn);
        match &ps[0].outcome {
            Outcome::Failed { raw: Some(raw), .. } => {
                assert!(raw.contains("exit=1"));
                assert!(raw.contains("--- stdout ---\nhello"));
                assert!(raw.contains("--- stderr ---\nworld"));
            }
            other => panic!("expected Failed w/ raw, got {:?}", other),
        }
    }
}
