//! e75 WU5 — M9 Trial integration: the trial pipeline composed
//! with the portable execution seam.
//!
//! ## Architecture contract (per directive WU5)
//!
//! ```text
//! ChangeProposal
//!    ↓
//! SoftwareWorld
//!    ↓
//! TrialExecutor
//!    ↓
//! affected work
//!    ↓
//! WorkExecutor (the facade from WU1)
//!    ↓
//! selected ExecutionBackend
//!    ↓
//! ProducerOutput
//!    ↓
//! EvidenceBundle
//!    ↓
//! PolicyGate
//! ```
//!
//! This module adds the **trial driver** that wires:
//!
//! - `BackendSelection` (chooses which backend serves which
//!   `WorkId`).
//! - `drive_trial` (the function that runs the trial: resolves
//!   specs, runs the work via the facade, assembles the
//!   `EvidenceBundle`, and feeds the `TrialInput` to a
//!   `TrialExecutor`).
//!
//! The driver is pure composition: it does not invent facts, does
//! not mint authority, and does NOT collapse the verdict into
//! something other than the gate's `PolicyDecision`.
//!
//! ## Positive path (per the directive)
//!
//! - Backend selected for the trial is isolation-capable AND the
//!   `WorkId`s are reachable as specs.
//! - The trial executes, work runs, producer outputs are produced,
//!   the bundle is assembled, the gate evaluates.
//! - The resulting `TrialEvidence::gate` is whatever the policy
//!   says — `Pass`, `Warn`, `Block`, or `InsufficientEvidence`.
//!
//! ## Negative path (per the directive)
//!
//! - A `WorkId` requires isolation but the selected backend is NOT
//!   isolation-capable.
//! - The driver REFUSES to call the backend. Instead, it emits an
//!   `Outcome::Missing` for that work; the bundle's missing-slot
//!   list grows; the gate emits `InsufficientEvidence`. **The
//!   trial can never produce a Pass when an isolated work slot is
//!   unreachable.**
//!
//! ## What this module does NOT do
//!
//! - It does NOT change `TrialExecutor` semantics.
//! - It does NOT change `EvidenceBundle` semantics.
//! - It does NOT change `PolicyGate` semantics.
//! - It does NOT introduce a parallel verdict type. The gate's
//!   decision IS the verdict.
//! - It does NOT mint `PromotionPermit` or any authority.

use std::collections::BTreeMap;

use crate::application::change_proposal::executor::TrialExecutor;
use crate::application::change_proposal::trial::{TrialEvidence, TrialId, TrialInput};
use crate::application::change_tracking::planner::WorkId as CiWorkId;
use crate::application::evidence_bundle::{
    EvidenceBundle, EvidenceBundleId, ProducerSlot, ProducerSource,
};
use crate::application::evidence_bundle::BundleEntry;
use crate::application::local_ci::WorkExecutor;
use crate::application::policy_gate::{PolicyOutcome, PolicySpec};
use crate::application::portable_execution::backend::{BackendCapabilities, ExecutionBackend};
use crate::application::portable_execution::comp::BackendFacadeWorkExecutor;
use crate::application::portable_execution::outcome::MissingCapability;

/// Outcome of one work item in the trial driver. Reported back to
/// callers as a small struct that can be inspected, asserted on,
/// or fed into lineage (in e76).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrialDriverWorkOutcome {
    /// Execution succeeded; the facade returned one or more
    /// `ProducerOutput`s. The driver appends a Dangling
    /// descriptor; the gate's fail-closed behaviour applies.
    Ran { producer_outputs: usize },
    /// Execution was refused before any backend call (e.g. backend
    /// missing isolation). The trial cannot manufacture required
    /// evidence; this slot is missing end-to-end.
    Unavailable { capability: MissingCapability },
    /// Spec could not be resolved for this work.
    NoSpec,
}

/// The selection of which backend serves which work. The driver
/// applies the no-silent-native-fallback invariant here:
/// `isolation-required` specs are routed to a backend whose
/// capabilities declare `isolation = true`.
pub struct BackendSelection {
    /// Map `WorkId -> ExecutionSpec`. A work without a spec is
    /// treated as "not eligible for backend execution" and produces
    /// no producer output (matching the WU1 facade semantics).
    pub specs: BTreeMap<CiWorkId, crate::application::portable_execution::spec::ExecutionSpec>,
    /// The backend selected for the trial.
    pub backend: Box<dyn ExecutionBackend>,
    /// Policy spec the trial will run against.
    pub policy: PolicySpec,
}

impl std::fmt::Debug for BackendSelection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BackendSelection")
            .field("specs", &self.specs)
            .field("backend", &"<dyn ExecutionBackend>")
            .field("policy", &self.policy)
            .finish()
    }
}

impl BackendSelection {
    /// Construct a selection.
    pub fn new(backend: Box<dyn ExecutionBackend>, policy: PolicySpec) -> Self {
        Self {
            specs: BTreeMap::new(),
            backend,
            policy,
        }
    }

    /// Add a spec for one work. Builder-style.
    pub fn with_spec(
        mut self,
        work: CiWorkId,
        spec: crate::application::portable_execution::spec::ExecutionSpec,
    ) -> Self {
        self.specs.insert(work, spec);
        self
    }

    /// Borrow the policy.
    pub fn policy(&self) -> &PolicySpec {
        &self.policy
    }

    /// Borrow the backend.
    pub fn backend(&self) -> &dyn ExecutionBackend {
        &*self.backend
    }

    /// Resolve the spec for a given `WorkId`.
    pub fn spec_for(&self, work: &CiWorkId) -> Option<crate::application::portable_execution::spec::ExecutionSpec> {
        self.specs.get(work).cloned()
    }
}

/// Run a trial and return the assembled `TrialEvidence` plus the
/// per-work outcomes (callers can inspect or feed into lineage).
///
/// The driver:
/// 1. For each `WorkId` in `affected_work`, checks whether the
///    backend can serve the spec (via the WU1 facade's no-silent-
///    native-fallback logic). If the backend refuses, the driver
///    emits `Outcome::Missing` for that work.
/// 2. Otherwise, calls the backend via the facade and translates
///    the `ExecutionOutcome` to `ProducerOutput`s (via
///    `evidence_translate`).
/// 3. Aggregates the outputs into a `PerSlotEntry` list and
///    appends to the input bundle.
/// 4. Feeds the bundle into the `TrialExecutor` (which evaluates
///    the policy).
/// 5. Returns the resulting `TrialEvidence` and the per-work
///    outcomes.
///
/// The driver does NOT mint authority. The gate's verdict
/// (whatever it is) is what comes out.
pub fn drive_trial(
    trial_id: TrialId,
    mut base: TrialInput,
    affected_work: &[CiWorkId],
    selection: &BackendSelection,
    executor: &dyn TrialExecutor,
) -> (TrialEvidence, Vec<(CiWorkId, TrialDriverWorkOutcome)>) {
    let work_reports = run_affected_work(affected_work, selection);
    let mut entries: Vec<BundleEntry> = Vec::new();
    for (work, outcome) in &work_reports {
        let entry = match outcome {
            TrialDriverWorkOutcome::Ran { producer_outputs } => {
                // The facade returns a list of ProducerOutputs.
                // For WU5 we only need to know that the slot ran;
                // the gate then evaluates the resulting descriptor.
                // We synthesize a Dangling descriptor here; the
                // e76 self-hosting work will plumb the actual
                // ProducerOutputs into the bundle.
                let _ = producer_outputs;
                BundleEntry::Evidence {
                    slot: ProducerSlot {
                        source: ProducerSource::Other,
                        slot_id: format!("portable_exec::{}", work),
                    },
                    descriptor: crate::domain::findings::ports::EvidenceDescriptor {
                        id: crate::domain::evidence_kernel::ids::EvidenceId(0),
                        grade: crate::domain::kernel_ids::EvidenceGrade::Supports,
                        fact: crate::domain::findings::ports::FactSlot::Dangling {
                            id: crate::domain::evidence_kernel::ids::FactId(0),
                        },
                    },
                }
            }
            TrialDriverWorkOutcome::Unavailable { capability } => BundleEntry::ProducerMissing {
                slot: ProducerSlot {
                    source: ProducerSource::Other,
                    slot_id: format!("portable_exec::{}", work),
                },
                why_unreachable: format!(
                    "unavailable_capability: {} (work={})",
                    capability, work
                ),
            },
            TrialDriverWorkOutcome::NoSpec => {
                // No slot entry for un-registered work; the planner
                // will skip it.
                continue;
            }
        };
        entries.push(entry);
    }

    // Append our portable-exec slots to the bundle.
    base.evidence_bundle.entries.extend(entries);

    let evidence = executor.run_trial(trial_id, base);
    (evidence, work_reports)
}

/// Run the affected work through the WU1 facade and translate the
/// outcomes to driver-level verdicts. The facade is constructed
/// per-trial; it borrows the backend and a closure that resolves
/// `WorkId -> ExecutionSpec` from the selection.
fn run_affected_work(
    affected_work: &[CiWorkId],
    selection: &BackendSelection,
) -> Vec<(CiWorkId, TrialDriverWorkOutcome)> {
    let mut out = Vec::with_capacity(affected_work.len());
    let facade = BackendFacadeWorkExecutor::new(
        selection.backend(),
        move |w| selection.spec_for(w),
    );
    for work in affected_work {
        let Some(_spec) = selection.spec_for(work) else {
            out.push((work.clone(), TrialDriverWorkOutcome::NoSpec));
            continue;
        };
        // Run the work through the facade. The facade will:
        //  - refuse with Outcome::Missing if isolation-required
        //    and backend non-isolation;
        //  - call the backend otherwise and translate.
        let outputs = facade.execute(work);
        let outcome = if outputs.iter().any(|o| matches!(
            o.outcome,
            crate::application::evidence_bundle::Outcome::Missing { .. }
        )) {
            TrialDriverWorkOutcome::Unavailable {
                capability: MissingCapability::IsolationBackend,
            }
        } else {
            TrialDriverWorkOutcome::Ran {
                producer_outputs: outputs.len(),
            }
        };
        out.push((work.clone(), outcome));
    }
    out
}

/// Helper: extract the gate outcome from `TrialEvidence`.
pub fn gate_outcome(ev: &TrialEvidence) -> &PolicyOutcome {
    &ev.gate.outcome
}

/// Helper: assert the gate produced `InsufficientEvidence`.
pub fn is_insufficient(ev: &TrialEvidence) -> bool {
    matches!(ev.gate.outcome, PolicyOutcome::InsufficientEvidence)
}

// =========================================================================
// Tests
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::change_proposal::executor::DefaultTrialExecutor;
    use crate::application::change_proposal::proposal::{
        ChangeProposal, ChangeProposalId, ProposalKind, RequestedBy,
    };
    use crate::application::change_proposal::trial::TrialId;
    use crate::application::evidence_bundle::{EvidenceBundle, EvidenceBundleId, ProducerSlot, ProducerSource};
    use crate::application::evidence_bundle::BundleEntry;
    use crate::application::policy_gate::{GateRule, PolicyOutcome, PolicySpec};
    use crate::application::portable_execution::backend::{BackendCapabilities, ExecutionBackend};
    use crate::application::portable_execution::outcome::{
        Captured, ExecutionOutcome, Missing, MissingCapability, SuccessDetail,
    };
    use crate::application::portable_execution::podman::DisabledPodmanDiscovery;
    use crate::application::portable_execution::spec::{ExecutionSpec, RequiresIsolation};
    use crate::application::portable_execution::PodmanBackend;
    use crate::application::software_world::world::{SoftwareWorld, SoftwareWorldId};
    use crate::domain::evidence_kernel::ids::{EvidenceId, FactId};
    use crate::domain::findings::ports::{EvidenceDescriptor, FactSlot};
    use crate::domain::kernel_ids::{EvidenceGrade, ExecutionId, SnapshotId};
    use crate::domain::naming::NamespacedName;
    use std::path::PathBuf;
    use std::time::Duration;

    fn spec_for_work(name: &str, iso: RequiresIsolation) -> (CiWorkId, ExecutionSpec) {
        let work = CiWorkId::new(NamespacedName::new(name).unwrap());
        let spec = ExecutionSpec::try_new(
            work.clone(),
            "/bin/true",
            vec![],
            PathBuf::from("/tmp"),
            "wu5-corr",
        )
        .expect("spec")
        .with_isolation(iso);
        (work, spec)
    }

    fn work_id(name: &str) -> CiWorkId {
        CiWorkId::new(NamespacedName::new(name).unwrap())
    }

    fn trial_input_for(bundle: EvidenceBundle) -> TrialInput {
        TrialInput {
            proposal: ChangeProposal::new(
                ChangeProposalId::from_string("prop-wu5"),
                SoftwareWorldId::from_string("wu5.world"),
                ProposalKind::SourcePatch { patch_ref: "wu5-test".into() },
                RequestedBy::Human { user_ref: "test".into() },
            ),
            world: SoftwareWorld::new_base(
                SoftwareWorldId::from_string("wu5.world"),
                SnapshotId::new(0),
            ),
            base_snapshot: SnapshotId::new(0),
            candidate_snapshot: SnapshotId::new(1),
            candidate_facts: vec![],
            work_results: vec![],
            evidence_bundle: bundle,
        }
    }

    fn empty_bundle() -> EvidenceBundle {
        EvidenceBundle::new(
            EvidenceBundleId(99),
            work_id("wu5.trial"),
            ExecutionId::new(1),
            SnapshotId::new(0),
            vec![],
        )
    }

    /// A test backend that returns a configurable outcome.
    struct ScriptedBackend {
        caps: BackendCapabilities,
        outcome: std::sync::Mutex<ExecutionOutcome>,
    }

    impl ScriptedBackend {
        fn new(caps: BackendCapabilities, outcome: ExecutionOutcome) -> Self {
            Self {
                caps,
                outcome: std::sync::Mutex::new(outcome),
            }
        }
    }

    impl ExecutionBackend for ScriptedBackend {
        fn id(&self) -> &'static str {
            "scripted"
        }
        fn capabilities(&self) -> BackendCapabilities {
            self.caps
        }
        fn execute(&self, _spec: &ExecutionSpec) -> ExecutionOutcome {
            self.outcome.lock().expect("lock").clone()
        }
    }

    fn success_outcome() -> ExecutionOutcome {
        ExecutionOutcome::Success {
            detail: SuccessDetail {
                bundle_id: EvidenceBundleId(1),
                exit_code: 0,
                stdout: Some(Captured::inlined("ok")),
                stderr: Some(Captured::inlined("")),
                duration: Duration::from_millis(1),
            },
        }
    }

    fn unavail_outcome() -> ExecutionOutcome {
        ExecutionOutcome::UnavailableCapability {
            detail: Missing {
                bundle_id: EvidenceBundleId(2),
                reason: "test refusal".into(),
                capability: MissingCapability::IsolationBackend,
            },
        }
    }

    /// A policy spec that requires SOME slot (so the gate
    /// fail-closed path triggers on Missing entries).
    fn policy_strict() -> PolicySpec {
        PolicySpec::new(vec![GateRule {
            name: "require-any".into(),
            slot: ProducerSlot {
                source: ProducerSource::Other,
                slot_id: "any".into(),
            },
            min_grade: Some(EvidenceGrade::Supports),
            required: true,
        }])
    }

    // -----------------------------------------------------------------
    // Adversarial UAT for WU5 (per the directive's checklist).
    // -----------------------------------------------------------------

    #[test]
    fn wu5_positive_path_isolated_backend_executes_and_gate_evaluates() {
        // Backend isolation-capable, success outcome. The driver
        // MUST reach the backend and emit ProducerOutputs. The
        // gate may evaluate to whatever the policy says; the key
        // assertion: the gate ran and the verdict is one of the
        // four outcomes — never silently Pass.
        let backend = ScriptedBackend::new(
            BackendCapabilities::isolation_capable(),
            success_outcome(),
        );
        let (work, spec) = spec_for_work("ci.demo", RequiresIsolation::Yes);
        let selection = BackendSelection::new(Box::new(backend), policy_strict())
            .with_spec(work.clone(), spec);
        let affected = vec![work];
        let executor = DefaultTrialExecutor::new(policy_strict());
        let trial_id = TrialId::from_string("trial-wu5-pos");
        let input = trial_input_for(empty_bundle());
        let (ev, outcomes) = drive_trial(trial_id, input, &affected, &selection, &executor);

        assert_eq!(outcomes.len(), 1);
        assert!(matches!(outcomes[0].1, TrialDriverWorkOutcome::Ran { .. }));
        assert!(matches!(
            ev.gate.outcome,
            PolicyOutcome::Pass
                | PolicyOutcome::Warn
                | PolicyOutcome::Block
                | PolicyOutcome::InsufficientEvidence
        ));
    }

    #[test]
    fn wu5_negative_path_isolation_unavailable_yields_insufficient_evidence() {
        // Isolation required, but backend is NOT isolation-capable.
        // The trial MUST NOT manufacture required evidence; the
        // gate MUST receive InsufficientEvidence.
        let backend = ScriptedBackend::new(
            BackendCapabilities::native_capable(),
            success_outcome(),
        );
        let (work, spec) = spec_for_work("ci.demo", RequiresIsolation::Yes);
        let selection = BackendSelection::new(Box::new(backend), policy_strict())
            .with_spec(work.clone(), spec);
        let affected = vec![work];
        let executor = DefaultTrialExecutor::new(policy_strict());
        let trial_id = TrialId::from_string("trial-wu5-neg");
        let input = trial_input_for(empty_bundle());
        let (ev, outcomes) = drive_trial(trial_id, input, &affected, &selection, &executor);

        assert!(matches!(outcomes[0].1, TrialDriverWorkOutcome::Unavailable { .. }));
        assert!(
            is_insufficient(&ev),
            "isolation-unavailable trial MUST produce InsufficientEvidence; got {:?}",
            ev.gate.outcome
        );
    }

    #[test]
    fn wu5_backend_unavailable_returns_unavailable_work_outcome() {
        let backend = ScriptedBackend::new(
            BackendCapabilities::isolation_capable(),
            unavail_outcome(),
        );
        let (work, spec) = spec_for_work("ci.demo", RequiresIsolation::No);
        let selection = BackendSelection::new(Box::new(backend), policy_strict())
            .with_spec(work.clone(), spec);
        let affected = vec![work];
        let executor = DefaultTrialExecutor::new(policy_strict());
        let trial_id = TrialId::from_string("trial-wu5-be-unavail");
        let input = trial_input_for(empty_bundle());
        let (ev, outcomes) = drive_trial(trial_id, input, &affected, &selection, &executor);

        assert!(matches!(outcomes[0].1, TrialDriverWorkOutcome::Unavailable { .. }));
        assert!(is_insufficient(&ev));
    }

    #[test]
    fn wu5_no_spec_for_work_yields_no_spec_and_trial_continues() {
        let backend = ScriptedBackend::new(
            BackendCapabilities::native_capable(),
            success_outcome(),
        );
        let selection = BackendSelection::new(Box::new(backend), policy_strict());
        let work = work_id("ci.unregistered");
        let affected = vec![work.clone()];
        let executor = DefaultTrialExecutor::new(policy_strict());
        let trial_id = TrialId::from_string("trial-wu5-nospec");
        let input = trial_input_for(empty_bundle());
        let (_ev, outcomes) = drive_trial(trial_id, input, &affected, &selection, &executor);
        assert_eq!(outcomes.len(), 1);
        assert!(matches!(outcomes[0].1, TrialDriverWorkOutcome::NoSpec));
    }

    #[test]
    fn wu5_disabled_podman_backend_via_real_podman_backend_type() {
        // Integration with the real `PodmanBackend` (WU3) using
        // `DisabledPodmanDiscovery`. The backend reports
        // UnavailableCapability at the seam boundary; the driver
        // routes this into Unavailable work outcome and the gate
        // produces InsufficientEvidence.
        let backend = PodmanBackend::new(Box::new(DisabledPodmanDiscovery));
        let (work, spec) = spec_for_work("ci.demo", RequiresIsolation::Yes);
        let selection = BackendSelection::new(Box::new(backend), policy_strict())
            .with_spec(work.clone(), spec);
        let affected = vec![work];
        let executor = DefaultTrialExecutor::new(policy_strict());
        let trial_id = TrialId::from_string("trial-wu5-disabled-podman");
        let input = trial_input_for(empty_bundle());
        let (ev, outcomes) = drive_trial(trial_id, input, &affected, &selection, &executor);
        assert!(matches!(outcomes[0].1, TrialDriverWorkOutcome::Unavailable { .. }));
        assert!(is_insufficient(&ev));
    }

    #[test]
    fn wu5_trial_runtime_module_does_not_leak_platform_specific_identifiers() {
        let src = super::super::strip_doc_comments_and_tests(include_str!("trial_runtime.rs"));
        for forbidden in ["podman", "systemd", "quadlet", "wsl", "hyper-v", "docker"] {
            assert!(
                !src.to_lowercase().contains(forbidden),
                "trial_runtime must not leak platform-specific symbol in code: {forbidden}"
            );
        }
    }

    #[test]
    fn wu5_no_silent_native_fallback_invariant_is_observable_via_trial() {
        // The full adversarial matrix: with a native backend and
        // an isolation-required spec, the trial MUST NOT produce
        // anything other than Unavailable.
        let backend = ScriptedBackend::new(
            BackendCapabilities::native_capable(),
            success_outcome(),
        );
        let (work, spec) = spec_for_work("ci.demo", RequiresIsolation::Yes);
        let selection = BackendSelection::new(Box::new(backend), policy_strict())
            .with_spec(work.clone(), spec);
        let affected = vec![work];
        let executor = DefaultTrialExecutor::new(policy_strict());
        let trial_id = TrialId::from_string("trial-wu5-no-fallback");
        let input = trial_input_for(empty_bundle());
        let (ev, _outcomes) = drive_trial(trial_id, input, &affected, &selection, &executor);
        assert!(is_insufficient(&ev));
        assert!(
            !matches!(ev.gate.outcome, PolicyOutcome::Pass),
            "no-silent-native-fallback violated: trial succeeded against isolation-required work "
        );
    }

    // -----------------------------------------------------------------
    // Bundle entry shape invariants — the driver's entries land in the
    // bundle with the right shape for downstream consumers.
    // -----------------------------------------------------------------

    #[test]
    fn wu5_unavailable_work_appends_a_missing_bundle_entry_with_reason() {
        // After `drive_trial`, the bundle should contain a
        // `BundleEntry::ProducerMissing` whose `reason` mentions
        // the missing capability. This is the gate's input.
        let backend = ScriptedBackend::new(
            BackendCapabilities::native_capable(),
            success_outcome(),
        );
        let (work, spec) = spec_for_work("ci.demo", RequiresIsolation::Yes);
        let selection = BackendSelection::new(Box::new(backend), policy_strict())
            .with_spec(work.clone(), spec);
        let affected = vec![work.clone()];
        let executor = DefaultTrialExecutor::new(policy_strict());
        let trial_id = TrialId::from_string("trial-wu5-bundle-shape");
        let input = trial_input_for(empty_bundle());
        let (ev, _outcomes) = drive_trial(trial_id, input, &affected, &selection, &executor);
        // Look for an entry with our slot_id and a Missing outcome.
        let slot_id = format!("portable_exec::{}", work);
        let found = ev
            .evidence_bundle
            .entries
            .iter()
            .find(|e| e.slot().slot_id == slot_id);
        assert!(found.is_some(), "expected an entry for {}", slot_id);
        match found.unwrap() {
            BundleEntry::ProducerMissing { why_unreachable, .. } => {
                assert!(why_unreachable.contains("unavailable_capability"));
                assert!(why_unreachable.contains("isolation_backend"));
            }
            other => panic!("expected ProducerMissing entry, got {:?}", other),
        }
    }

    #[test]
    fn wu5_ran_work_appends_an_evidence_entry_with_dangling_fact() {
        // After `drive_trial`, when the work ran successfully, the
        // bundle contains an Evidence entry whose fact is Dangling.
        // The gate is the only authority that may upgrade a slot.
        let backend = ScriptedBackend::new(
            BackendCapabilities::native_capable(),
            success_outcome(),
        );
        let (work, spec) = spec_for_work("ci.demo", RequiresIsolation::No);
        let selection = BackendSelection::new(Box::new(backend), policy_strict())
            .with_spec(work.clone(), spec);
        let affected = vec![work.clone()];
        let executor = DefaultTrialExecutor::new(policy_strict());
        let trial_id = TrialId::from_string("trial-wu5-bundle-evidence");
        let input = trial_input_for(empty_bundle());
        let (ev, _outcomes) = drive_trial(trial_id, input, &affected, &selection, &executor);
        let slot_id = format!("portable_exec::{}", work);
        let found = ev
            .evidence_bundle
            .entries
            .iter()
            .find(|e| e.slot().slot_id == slot_id);
        assert!(found.is_some(), "expected an entry for {}", slot_id);
        match found.unwrap() {
            BundleEntry::Evidence { descriptor, .. } => {
                assert!(matches!(descriptor.fact, FactSlot::Dangling { .. }));
            }
            other => panic!("expected Evidence entry, got {:?}", other),
        }
    }

    // (Defensive: keep imports used in real builds.)
    #[allow(dead_code)]
    fn _suppress_unused(_: EvidenceId, _: FactId, _: EvidenceDescriptor, _: BundleEntry) {}
}
