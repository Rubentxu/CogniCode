//! e75 WU1 — Composition root for the portable execution seam.
//!
//! This module is the **only place** that knows about both seams:
//!
//! ```text
//! TrialExecutor
//!   └── run_trial(trial_id, input)        # M9 envelope
//!        └─ (via e70 run_local_vertical)
//!             └─ WorkExecutor::execute(work)
//!                  └─ BackendFacadeWorkExecutor  # this module
//!                       ├─ resolve spec for work_id
//!                       ├─ if isolation required and backend not isolation-capable:
//!                       │     return Vec<ProducerOutput> with Outcome::Missing
//!                       └─ else: backend.execute(spec) → ExecutionOutcome
//!                              └─ translate to Vec<ProducerOutput>
//! ```
//!
//! `WorkExecutor` is unchanged. `TrialExecutor` is unchanged. The
//! facade (`BackendFacadeWorkExecutor`) is the only NEW type and
//! already lives here, not in `local_ci/`. Anyone wiring e75 into
//! the M9 trial uses `BackendFacadeWorkExecutor::new(backend,
//! resolve_work)` and gets a `WorkExecutor` impl.
//!
//! ## The no-silent-native-fallback invariant
//!
//! If the spec requires isolation and the backend does NOT declare
//! `capabilities().isolation = true`, we DO NOT call the backend.
//! We synthesise an `Outcome::Missing` with a stable reason so the
//! gate receives `InsufficientEvidence` rather than a forged pass.
//!
//! ## Pure / total / no I/O
//!
//! This composition is pure & total. It does not spawn any
//! processes; the actual spawn lives in the
//! [`ExecutionBackend`] impls (WU2, WU3). The facade is the orchestrator
//! only.

use std::collections::BTreeMap;

use crate::application::change_tracking::planner::WorkId as CiWorkId;
use crate::application::evidence_bundle::{Outcome, ProducerOutput, ProducerSource};
use crate::application::local_ci::WorkExecutor;
use crate::application::portable_execution::backend::ExecutionBackend;
use crate::application::portable_execution::evidence_translate;
use crate::application::portable_execution::outcome::{ExecutionOutcome, MissingCapability};
use crate::application::portable_execution::spec::{ExecutionSpec, RequiresIsolation};

/// Stable id used for the synthetic slot returned when the seam
/// refuses to call the backend. Real producers carry slots that are
/// meaningful to the gate; this is the "fallback" slot used only for
/// isolation refusal.
const FALLBACK_PRODUCER_SOURCE: ProducerSource = ProducerSource::Other;

/// A `WorkExecutor` that delegates to an `ExecutionBackend` plus a
/// `WorkId -> ExecutionSpec` resolver.
///
/// This is the **only** type that satisfies `WorkExecutor` while
/// knowing about the host-execution seam. All real call sites in M9
/// wire this in place of `InMemoryWorkExecutor`.
///
/// Construction:
///
/// ```ignore
/// let fac = BackendFacadeWorkExecutor::new(&backend, |work| {
///     // ... resolve spec for the work, or None if not eligible
/// });
/// let report = run_local_vertical(&delta, &deps, &fac, &spec);
/// ```
pub struct BackendFacadeWorkExecutor<'a, F>
where
    F: Fn(&CiWorkId) -> Option<ExecutionSpec> + Send + Sync,
{
    backend: &'a dyn ExecutionBackend,
    resolve_work: F,
    /// Slot resolver. The pipeline determines the producer slot for
    /// each spec (one slot per spec); the default uses a synthetic
    /// `(ProducerSource::Other, work_id)` slot. Real callers can
    /// override.
    slot_for: Box<
        dyn Fn(&ExecutionSpec) -> crate::application::evidence_bundle::ProducerSlot + Send + Sync,
    >,
    /// Per-process bundle id source. We seed it from
    /// [`crate::application::evidence_bundle::next_bundle_id`].
    bundle_seed: std::sync::atomic::AtomicU64,
}

impl<'a, F> BackendFacadeWorkExecutor<'a, F>
where
    F: Fn(&CiWorkId) -> Option<ExecutionSpec> + Send + Sync,
{
    /// Construct a facade with the default slot-for resolver
    /// (`(Other, work_id)`) and the default bundle id source.
    pub fn new(backend: &'a dyn ExecutionBackend, resolve_work: F) -> Self {
        Self {
            backend,
            resolve_work,
            slot_for: Box::new(default_slot_for),
            bundle_seed: std::sync::atomic::AtomicU64::new(1),
        }
    }

    /// Override the slot resolver (e.g. when wiring into a real
    /// ProducerSource registry).
    pub fn with_slot_for(
        mut self,
        slot_for: impl Fn(&ExecutionSpec) -> crate::application::evidence_bundle::ProducerSlot
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.slot_for = Box::new(slot_for);
        self
    }

    /// Borrow the configured backend.
    pub fn backend(&self) -> &dyn ExecutionBackend {
        self.backend
    }
}

fn default_slot_for(spec: &ExecutionSpec) -> crate::application::evidence_bundle::ProducerSlot {
    crate::application::evidence_bundle::ProducerSlot {
        source: FALLBACK_PRODUCER_SOURCE,
        slot_id: format!("portable_exec::{}", spec.work_id),
    }
}

fn fallback_bundle_id(
    seq: &std::sync::atomic::AtomicU64,
) -> crate::application::evidence_bundle::EvidenceBundleId {
    // The facade mints a bundle id only when the seam synthesises a
    // refusal outcome. The id does not have to be globally unique
    // across bundles (this is diagnostic-only). We use a local
    // counter to keep test runs deterministic relative to the global
    // one in `next_bundle_id`.
    crate::application::evidence_bundle::EvidenceBundleId(
        seq.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
    )
}

fn refusal_missing(
    work_id: &CiWorkId,
    _bundle_id: crate::application::evidence_bundle::EvidenceBundleId,
    capability: MissingCapability,
    reason: impl Into<String>,
) -> Vec<ProducerOutput> {
    vec![ProducerOutput {
        slot: crate::application::evidence_bundle::ProducerSlot {
            source: FALLBACK_PRODUCER_SOURCE,
            slot_id: format!("portable_exec::{}", work_id),
        },
        outcome: Outcome::Missing {
            why_unreachable: format!(
                "unavailable_capability: {} (work={}): {}",
                capability,
                work_id,
                reason.into()
            ),
        },
    }]
}

impl<'a, F> WorkExecutor for BackendFacadeWorkExecutor<'a, F>
where
    F: Fn(&CiWorkId) -> Option<ExecutionSpec> + Send + Sync,
{
    fn execute(&self, work: &CiWorkId) -> Vec<ProducerOutput> {
        // 1. Resolve the spec for this work. If the planner says
        //    "this work is not eligible for backend execution",
        //    return empty (matches InMemoryWorkExecutor semantics:
        //    a non-eligible WorkId produces no producer outputs).
        let Some(spec) = (self.resolve_work)(work) else {
            return Vec::new();
        };

        // 2. Enforce the no-silent-native-fallback invariant.
        //
        //    If the spec REQUIRES isolation and the configured backend
        //    is NOT isolation-capable, refuse — do NOT call the
        //    backend. The gate must receive Missing, not Evidence.
        if spec.isolation.requires_isolation() && !self.backend.capabilities().isolation {
            let bundle_id = fallback_bundle_id(&self.bundle_seed);
            return refusal_missing(
                work,
                bundle_id,
                MissingCapability::IsolationBackend,
                "spec requires isolation but no isolation-capable backend is wired in",
            );
        }

        // 3. Delegate to the backend and translate the outcome.
        let outcome: ExecutionOutcome = self.backend.execute(&spec);
        // The orchestrator uses one slot per spec; the slot_for
        // closure is the single source of slot routing.
        let slot_for = &*self.slot_for;
        evidence_translate::translate(outcome, &spec, slot_for)
    }
}

/// Convenience builder: a ready-to-use facade with a `BTreeMap`-based
/// resolver. Useful for tests.
pub struct StaticBackendFacade<'a> {
    backend: &'a dyn ExecutionBackend,
    specs: BTreeMap<CiWorkId, ExecutionSpec>,
}

impl<'a> StaticBackendFacade<'a> {
    pub fn new(backend: &'a dyn ExecutionBackend) -> Self {
        Self {
            backend,
            specs: BTreeMap::new(),
        }
    }
    pub fn with_spec(mut self, work: CiWorkId, spec: ExecutionSpec) -> Self {
        self.specs.insert(work, spec);
        self
    }
    pub fn into_facade(
        self,
    ) -> BackendFacadeWorkExecutor<'a, impl Fn(&CiWorkId) -> Option<ExecutionSpec> + Send + Sync>
    {
        let Self { backend, specs } = self;
        BackendFacadeWorkExecutor::new(backend, move |w| specs.get(w).cloned())
    }
}

// (The trait impl returns Vec<ProducerOutput>; any unused-warning
// silencer below.)

// =====================================================================
// Adversarial UAT
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::change_tracking::planner::WorkId;
    use crate::application::evidence_bundle::{EvidenceBundleId, Outcome, ProducerSlot};
    use crate::application::local_ci::WorkExecutor;
    use crate::application::portable_execution::backend::{BackendCapabilities, ExecutionBackend};
    use crate::application::portable_execution::outcome::{
        ExecutionOutcome, Missing, MissingCapability, SuccessDetail,
    };
    use crate::application::portable_execution::spec::{ExecutionSpec, RequiresIsolation};
    use crate::domain::naming::NamespacedName;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    fn spec(iso: RequiresIsolation) -> ExecutionSpec {
        ExecutionSpec::try_new(
            WorkId::new(NamespacedName::new("ci.demo").unwrap()),
            "/bin/true",
            vec![],
            PathBuf::from("/tmp"),
            "corr-1",
        )
        .expect("spec")
        .with_isolation(iso)
    }

    struct StaticBackend {
        caps: BackendCapabilities,
        /// Number of times `execute` was called. Tests assert this.
        calls: AtomicUsize,
        /// Outcome to return on every call.
        outcome: std::sync::Mutex<ExecutionOutcome>,
    }

    impl StaticBackend {
        fn new(caps: BackendCapabilities, outcome: ExecutionOutcome) -> Self {
            Self {
                caps,
                calls: AtomicUsize::new(0),
                outcome: std::sync::Mutex::new(outcome),
            }
        }
        fn calls(&self) -> usize {
            self.calls.load(Ordering::Relaxed)
        }
    }

    impl ExecutionBackend for StaticBackend {
        fn id(&self) -> &'static str {
            "static-test"
        }
        fn capabilities(&self) -> BackendCapabilities {
            self.caps
        }
        fn execute(&self, _spec: &ExecutionSpec) -> ExecutionOutcome {
            self.calls.fetch_add(1, Ordering::Relaxed);
            self.outcome.lock().expect("lock").clone()
        }
    }

    fn success_outcome() -> ExecutionOutcome {
        ExecutionOutcome::Success {
            detail: SuccessDetail {
                bundle_id: EvidenceBundleId(1),
                exit_code: 0,
                stdout: None,
                stderr: None,
                duration: Duration::from_millis(1),
            },
        }
    }

    fn missing_outcome() -> ExecutionOutcome {
        ExecutionOutcome::UnavailableCapability {
            detail: Missing {
                bundle_id: EvidenceBundleId(2),
                reason: "test refusal".into(),
                capability: MissingCapability::IsolationBackend,
            },
        }
    }

    // --- Composition UAT ---

    #[test]
    fn facade_routes_a_resolved_spec_to_the_backend() {
        let backend = StaticBackend::new(BackendCapabilities::native_capable(), success_outcome());
        let work = WorkId::new(NamespacedName::new("ci.demo").unwrap());
        let facade =
            BackendFacadeWorkExecutor::new(&backend, move |_| Some(spec(RequiresIsolation::No)));
        let outs = facade.execute(&work);
        assert_eq!(
            backend.calls(),
            1,
            "backend MUST be called when spec resolves"
        );
        assert_eq!(outs.len(), 1);
        assert!(matches!(outs[0].outcome, Outcome::Evidence { .. }));
    }

    #[test]
    fn facade_returns_empty_when_resolver_returns_none() {
        let backend = StaticBackend::new(BackendCapabilities::native_capable(), success_outcome());
        let work = WorkId::new(NamespacedName::new("ci.demo").unwrap());
        let facade = BackendFacadeWorkExecutor::new(&backend, |_| None);
        let outs = facade.execute(&work);
        assert!(outs.is_empty());
        assert_eq!(
            backend.calls(),
            0,
            "backend MUST NOT be called when no spec"
        );
    }

    #[test]
    fn isolation_required_with_native_backend_yields_missing_not_evidence() {
        // The directive: "requires_isolation = true + Podman unavailable
        // → UnavailableCapability. NOT NativeProcessBackend."
        let backend = StaticBackend::new(BackendCapabilities::native_capable(), success_outcome());
        let work = WorkId::new(NamespacedName::new("ci.demo").unwrap());
        let facade =
            BackendFacadeWorkExecutor::new(&backend, move |_| Some(spec(RequiresIsolation::Yes)));
        let outs = facade.execute(&work);
        assert_eq!(
            backend.calls(),
            0,
            "isolation-required spec MUST NOT be served by a non-isolation backend"
        );
        assert_eq!(outs.len(), 1);
        match &outs[0].outcome {
            Outcome::Missing { why_unreachable } => {
                assert!(why_unreachable.contains("unavailable_capability"));
                assert!(why_unreachable.contains("isolation_backend"));
            }
            other => panic!("expected Missing, got {:?}", other),
        }
    }

    #[test]
    fn isolation_required_with_isolation_backend_invokes_backend() {
        let backend =
            StaticBackend::new(BackendCapabilities::isolation_capable(), success_outcome());
        let work = WorkId::new(NamespacedName::new("ci.demo").unwrap());
        let facade =
            BackendFacadeWorkExecutor::new(&backend, move |_| Some(spec(RequiresIsolation::Yes)));
        let outs = facade.execute(&work);
        assert_eq!(backend.calls(), 1);
        assert!(matches!(outs[0].outcome, Outcome::Evidence { .. }));
    }

    #[test]
    fn unavailability_is_propagated_as_missing_through_the_facade() {
        // The backend itself returns UnavailableCapability → we
        // expect Missing (NOT Evidence) to come out of the facade.
        let backend =
            StaticBackend::new(BackendCapabilities::isolation_capable(), missing_outcome());
        let work = WorkId::new(NamespacedName::new("ci.demo").unwrap());
        let facade =
            BackendFacadeWorkExecutor::new(&backend, move |_| Some(spec(RequiresIsolation::No)));
        let outs = facade.execute(&work);
        assert_eq!(backend.calls(), 1);
        assert!(matches!(&outs[0].outcome, Outcome::Missing { .. }));
    }

    #[test]
    fn success_zero_exit_does_not_produce_a_pass_authority_signal() {
        // This is the heart of the e75 directive: a zero-exit outcome
        // MUST NOT mint authority. The facade forwards the backend's
        // outcome into the producer stream exactly once, and the
        // gate (already fail-closed against Dangling) does the rest.
        let backend = StaticBackend::new(BackendCapabilities::native_capable(), success_outcome());
        let work = WorkId::new(NamespacedName::new("ci.demo").unwrap());
        let facade =
            BackendFacadeWorkExecutor::new(&backend, move |_| Some(spec(RequiresIsolation::No)));
        let outs = facade.execute(&work);
        // We get exactly ONE ProducerOutput. The gate then evaluates.
        assert_eq!(outs.len(), 1);
        // The shape is Evidence-with-Dangling; the gate is the only
        // authority that can elevate a slot to Pass.
        match &outs[0].outcome {
            Outcome::Evidence { descriptor } => {
                let crate::domain::findings::ports::FactSlot::Dangling { .. } = &descriptor.fact
                else {
                    panic!("Success must produce a Dangling fact slot, not a Resolved one");
                };
            }
            other => panic!("expected Evidence, got {:?}", other),
        }
    }

    #[test]
    fn facade_supports_a_custom_slot_for_resolver() {
        let backend = StaticBackend::new(BackendCapabilities::native_capable(), success_outcome());
        let work = WorkId::new(NamespacedName::new("ci.demo").unwrap());
        let custom_slot = ProducerSlot {
            source: ProducerSource::CargoTest,
            slot_id: "ci.demo:e75".into(),
        };
        let expected = custom_slot.clone();
        let facade =
            BackendFacadeWorkExecutor::new(&backend, move |_| Some(spec(RequiresIsolation::No)))
                .with_slot_for(move |_| expected.clone());
        let outs = facade.execute(&work);
        assert_eq!(outs[0].slot, custom_slot);
    }

    #[test]
    fn facade_module_does_not_leak_platform_specific_identifiers() {
        let src = super::super::strip_doc_comments_and_tests(include_str!("comp.rs"));
        for forbidden in ["podman", "systemd", "quadlet", "wsl", "hyper-v", "docker"] {
            assert!(
                !src.to_lowercase().contains(forbidden),
                "comp must not leak platform-specific symbol in code: {forbidden}"
            );
        }
    }

    #[test]
    fn static_facade_builder_routes_specs_through_btreemap() {
        let backend = StaticBackend::new(BackendCapabilities::native_capable(), success_outcome());
        let work_a = WorkId::new(NamespacedName::new("ci.a").unwrap());
        let work_b = WorkId::new(NamespacedName::new("ci.b").unwrap());
        let facade = StaticBackendFacade::new(&backend)
            .with_spec(work_a.clone(), spec(RequiresIsolation::No))
            .with_spec(work_b.clone(), spec(RequiresIsolation::No))
            .into_facade();
        let outs_a = facade.execute(&work_a);
        let outs_b = facade.execute(&work_b);
        assert_eq!(outs_a.len(), 1);
        assert_eq!(outs_b.len(), 1);
        // unused work produces empty outputs (mirrors InMemoryWorkExecutor).
        let work_c = WorkId::new(NamespacedName::new("ci.c").unwrap());
        let outs_c = facade.execute(&work_c);
        assert!(outs_c.is_empty());
    }
}
