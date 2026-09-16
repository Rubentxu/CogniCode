//! e75 WU1 — `ExecutionBackend` (host-execution seam trait).
//!
//! Why a NEW trait (not adding to `WorkExecutor`):
//!
//! - `WorkExecutor` is the e70 narrow seam: `fn execute(&WorkId) ->
//!   Vec<ProducerOutput>`. It does not know about argv, env, isolation,
//!   or mounts — and per the e75 directive it must not be deformed
//!   into a Podman-aware entity.
//! - `ExecutionBackend` IS the host-execution seam. It cares about
//!   argv, cwd, env, isolation, bounds, and host capability. Its
//!   contract is "given an `ExecutionSpec`, produce an
//!   `ExecutionOutcome`". Adapters in WU2 / WU3 fill this trait.
//!
//! ## Capability declaration
//!
//! Each backend reports its capability set via
//! [`ExecutionBackend::capabilities`]. The composition
//! ([`super::comp`]) uses this to refuse work the backend cannot
//! service, returning `ExecutionOutcome::UnavailableCapability`. This
//! is the only mechanism by which the seam enforces the
//! "requires_isolation ⇒ isolation backend" invariant.

use super::outcome::ExecutionOutcome;
use super::spec::ExecutionSpec;

/// What an [`ExecutionBackend`] can and cannot do.
///
/// Capability is a *property of the backend*, queried once at
/// composition time. The composition layer uses it to refuse work
/// (`UnavailableCapability`) instead of letting the backend flounder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackendCapabilities {
    /// True iff the backend can host an isolated execution. This is
    /// the seam-recognised way to declare "I am the isolation
    /// backend". A backend with `isolation = false` MUST NOT be
    /// selected for `requires_isolation = Yes` specs.
    pub isolation: bool,
    /// True iff the backend accepts an explicit `cwd`. Always true in
    /// practice; the field exists for forward compatibility
    /// (e.g. remote backends may not honour the request).
    pub accepts_cwd: bool,
    /// True iff the backend honours env mutations. Always true in
    /// practice; reserved for future adapters.
    pub accepts_env: bool,
    /// True iff the backend enforces wall-clock bounds. `false` on
    /// most native backends (host cgroups aside).
    pub enforces_wall_clock: bool,
}

impl BackendCapabilities {
    /// Convenience: construct an isolation-capable backend descriptor
    /// (the e75 WU3 PodmanBackend in fact).
    pub fn isolation_capable() -> Self {
        Self {
            isolation: true,
            accepts_cwd: true,
            accepts_env: true,
            enforces_wall_clock: true,
        }
    }

    /// Convenience: construct a native-backend descriptor.
    pub fn native_capable() -> Self {
        Self {
            isolation: false,
            accepts_cwd: true,
            accepts_env: true,
            enforces_wall_clock: false,
        }
    }
}

/// The execution seam.
///
/// One method: take an `ExecutionSpec`, return an `ExecutionOutcome`.
/// Implementors (e75 WU2 `NativeProcessBackend`, e75 WU3
/// `PodmanBackend`) translate the spec into their host-native
/// representation, run the work, capture outputs, and classify the
/// result into one of the five outcome variants.
///
/// ## Required behaviour
///
/// - The implementation MUST classify cleanly into one of the five
///   `ExecutionOutcome` variants. It MUST NOT, for example, fold
///   spawn failure into `CommandFailure`. Use
///   [`ExecutionOutcome::InfrastructureFailure`] for spawn failures
///   and [`ExecutionOutcome::UnavailableCapability`] for "I cannot
///   serve this request at all".
/// - The implementation MUST honour `read_only` on every
///   [`WorkspaceMount`](super::spec::WorkspaceMount).
/// - The implementation MUST honour `requires_isolation`: a
///   non-isolation backend MUST refuse specs with
///   `RequiresIsolation::Yes` by returning
///   [`ExecutionOutcome::UnavailableCapability`] with
///   [`MissingCapability::IsolationBackend`](super::outcome::MissingCapability::IsolationBackend).
///   This is the no-silent-native-fallback invariant.
pub trait ExecutionBackend: Send + Sync {
    /// Stable identifier for diagnostics.
    fn id(&self) -> &'static str;

    /// The capabilities this backend declares. The composition layer
    /// queries this once; let it return a constant or near-constant.
    fn capabilities(&self) -> BackendCapabilities;

    /// Execute the spec, returning the outcome.
    fn execute(&self, spec: &ExecutionSpec) -> ExecutionOutcome;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::evidence_bundle::EvidenceBundleId;
    use crate::application::portable_execution::outcome::{ExecutionOutcome, SuccessDetail};
    use std::time::Duration;

    /// A backend that records what it received. We use this to assert
    /// composition-layer behaviour without invoking real spawn.
    struct CapturingBackend {
        caps: BackendCapabilities,
    }

    impl CapturingBackend {
        fn new(caps: BackendCapabilities) -> Self {
            Self { caps }
        }
    }

    impl ExecutionBackend for CapturingBackend {
        fn id(&self) -> &'static str {
            "capturing"
        }
        fn capabilities(&self) -> BackendCapabilities {
            self.caps
        }
        fn execute(&self, _spec: &ExecutionSpec) -> ExecutionOutcome {
            ExecutionOutcome::Success {
                detail: SuccessDetail {
                    bundle_id: EvidenceBundleId(0),
                    exit_code: 0,
                    stdout: None,
                    stderr: None,
                    duration: Duration::from_millis(0),
                },
            }
        }
    }

    #[test]
    fn capabilities_predicates_are_explicit() {
        let nat = BackendCapabilities::native_capable();
        let iso = BackendCapabilities::isolation_capable();
        assert!(!nat.isolation);
        assert!(iso.isolation);
        assert!(iso.enforces_wall_clock);
        assert!(!nat.enforces_wall_clock);
    }

    #[test]
    fn backend_module_does_not_leak_platform_specific_identifiers() {
        let src = super::super::strip_doc_comments_and_tests(include_str!("backend.rs"));
        for forbidden in ["podman", "systemd", "quadlet", "wsl", "hyper-v", "docker"] {
            assert!(
                !src.to_lowercase().contains(forbidden),
                "ExecutionBackend must not leak platform-specific symbol in code: {forbidden}"
            );
        }
    }

    #[test]
    fn capturing_backend_reports_configured_caps() {
        let be = CapturingBackend::new(BackendCapabilities::native_capable());
        assert!(!be.capabilities().isolation);
        let be2 = CapturingBackend::new(BackendCapabilities::isolation_capable());
        assert!(be2.capabilities().isolation);
    }
}
