//! e75 WU1 — `ExecutionSpec` (portable execution request).
//!
//! Why this is a NEW type and not a method on `WorkExecutor`:
//!
//! - `WorkExecutor::execute(&self, work: &WorkId) -> Vec<ProducerOutput>`
//!   is the e70 narrow application seam: "given a unit of work, return
//!   what it produced". It is deliberately total and side-effect free
//!   for in-memory testing.
//! - e75 introduces a host-execution seam (`ExecutionBackend`) that
//!   cares about argv, cwd, env, isolation, bounds, network policy.
//!   Adding those concerns to `WorkExecutor` would deform the
//!   application abstraction.
//! - The composition `TrialExecutor -> WorkExecutor (facade) ->
//!   ExecutionBackend` lives in [`super::comp`] and is the only place
//!   that knows about both seams.
//!
//! `ExecutionSpec` therefore carries what `ExecutionBackend` needs,
//! not what `WorkExecutor` needs. The `work_id` linkage is *also*
//! carried so the facade can route `WorkId -> ExecutionSpec`.
//!
//! ## Cognitive contract (per directive WU1)
//!
//! - `requires_isolation = false`: any backend may handle this. If the
//!   selected backend cannot, it returns `UnavailableCapability`.
//! - `requires_isolation = true`: the seam REQUIRES an isolation-capable
//!   backend. If none is available, `UnavailableCapability` is the
//!   answer; `NativeProcessBackend` MUST NOT silently serve.
//!
//! ## What is NOT in this type
//!
//! - Podman, systemd, Quadlet, WSL, Hyper-V names. Those live in the
//!   adapter impls. The seam is platform-agnostic.
//! - Process env-merging policy (system env + spec env, PATH lookup,
//!   shell expansion). Each adapter normalizes for its host. This
//!   module only declares the input set.
//! - Authority. The spec does not declare pass / fail; the *outcome*
//!   declares success and the gate declares pass.

use std::path::PathBuf;
use std::time::Duration;

use crate::domain::execution::ActorRef;
use crate::domain::execution::CorrelationId;
use crate::domain::execution::scope::AnalysisScope;
use crate::domain::kernel_ids::SnapshotId;
use crate::domain::value_objects::WorkspaceId;

/// Reason an [`ExecutionSpec`] cannot be constructed.
///
/// A spec is total and well-formed by construction; any rejection
/// upstream of the seam is a contract violation we surface explicitly
/// instead of papering-over with `Default::default`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecError {
    /// A correlation id was empty.
    EmptyCorrelation,
    /// The actor id was empty.
    EmptyActor,
}

impl std::fmt::Display for SpecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyCorrelation => f.write_str("spec correlation id must not be empty"),
            Self::EmptyActor => f.write_str("spec actor id must not be empty"),
        }
    }
}

impl std::error::Error for SpecError {}

/// Identity of the work to execute.
///
/// This is the value the facade reads when reconciling
/// `WorkId -> ExecutionSpec`. The producer / gate code references
/// `WorkId`; the seam references `work_id` for routing only.
pub type WorkId = crate::application::change_tracking::planner::WorkId;

/// Request to execute a single portable command.
///
/// `ExecutionSpec` is the **input** side of the execution seam.
/// Constructed by the caller (planner/facade), interpreted by the
/// selected [`ExecutionBackend`](super::backend::ExecutionBackend).
///
/// All fields are explicit on purpose: the seam does not lean on host
/// defaults. Adapter impls translate this into a backend-native
/// representation (a `Command::new(program)` chain for native, a
/// `podman run --rm` invocation for podman, etc.).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionSpec {
    /// Stable identifier of the work this spec executes. Used by the
    /// facade to route from `WorkId` to backend outputs and by the
    /// gate to link to the producer slot.
    pub work_id: WorkId,

    /// Program to invoke (absolute path, or PATH-resolved by the
    /// adapter — never a shell string).
    ///
    /// Adapters prefer `program` + `argv` over a single shell-string
    /// command. Shell expansion is opt-in via `shell: bool` and is
    /// reserved for cases where an existing Work definition explicitly
    /// requires it (rare, and gated by the adapter).
    pub program: String,

    /// Argv passed to `program`. Each entry is one literal argv entry;
    /// no shell tokenization is performed unless `shell = true`.
    pub argv: Vec<String>,

    /// Working directory for the process. Adapters interpret
    /// platform-relativized paths via the workspace/mount layer (WU4);
    /// the spec stores the host-canonical path.
    pub cwd: PathBuf,

    /// Environment variables to set (full set). Adapters typically
    /// merge this into a minimal env, *not* the host's full env. PATH
    /// is not implicitly set unless listed here.
    pub env: Vec<(String, String)>,

    /// Isolation requirement.
    ///
    /// - `RequiresIsolation::Yes` — only an isolation-capable backend
    ///   may serve; otherwise `UnavailableCapability`.
    /// - `RequiresIsolation::No` — any backend may serve.
    ///
    /// This flag is the **only** mechanism by which the seam enforces
    /// the no-silent-native-fallback invariant.
    pub isolation: RequiresIsolation,

    /// Bounded execution policy. Adapters honour these where the
    /// platform allows; some caps are best-effort on portable hosts
    /// (e.g. macOS process memory caps). The outcome distinguishes
    /// a per-command exit failure from a timeout/bound violation.
    pub bounds: ExecutionBounds,

    /// Identity layer pin: the workspace + snapshot this spec is
    /// pinned to (from M7.2 / e64).
    pub scope: AnalysisScope,

    /// Identity layer pin: who is asking for this execution.
    pub actor: ActorRef,

    /// Identity layer pin: which logical operation this belongs to.
    pub correlation: CorrelationId,

    /// Workspace mount plan. The map is `host_path ->
    /// ContainerMount`. Empty `mounts` means `cwd` and no other paths
    /// are exposed to the process (native backends naturally inherit
    /// the host filesystem; isolation backends start empty unless
    /// mounts are listed).
    ///
    /// WU4 formalizes the path-normalization contract. e75 WU1 keeps
    /// the type minimal: a host path, a container path, read-only
    /// flag, and a propagation flag.
    pub mounts: Vec<WorkspaceMount>,
}

/// Whether the request requires isolation-backed execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RequiresIsolation {
    /// Work is safe / test-only / no side-effect tolerance.
    /// Any backend may serve.
    No,
    /// Work touches a workspace, runs side effects, or is otherwise
    /// required to be isolated. Only isolation-capable backends may
    /// serve; otherwise the seam MUST return `UnavailableCapability`.
    Yes,
}

impl RequiresIsolation {
    /// True iff the request requires isolation-backed execution.
    pub fn requires_isolation(self) -> bool {
        matches!(self, RequiresIsolation::Yes)
    }
}

/// Bounds on the execution. Mirrored by adapters where supported;
/// ignored (logged) where not.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExecutionBounds {
    /// Wall-clock timeout. `None` means "no enforced timeout" — the
    /// adapter may still apply its own default.
    pub wall_clock: Option<Duration>,
    /// Approximate memory cap, in bytes. Best-effort on portable
    /// hosts; isolation backends may enforce more precisely (cgroups).
    pub memory_bytes: Option<u64>,
    /// Approximate CPU quota. Best-effort; isolation backends may map
    /// to cgroup `cpu.shares`.
    pub cpu_micros: Option<u64>,
}

/// One workspace mount the process should see.
///
/// `host_path` is the host-canonical path. `container_path` is the
/// path the process will see. `read_only = true` MUST be honoured by
/// every adapter (native or isolation).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceMount {
    pub host_path: PathBuf,
    pub container_path: PathBuf,
    pub read_only: bool,
    /// Propagation semantics. Native backend ignores this. Isolation
    /// backend passes through to the platform's propagation flag
    /// (e.g. `private`, `shared`, `slave` on systemd-nspawn / podman).
    pub propagation: MountPropagation,
}

/// POSIX-style propagation flag. Backend-defined semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MountPropagation {
    /// No propagation to peer mounts.
    Private,
    /// Propagation is shared; children see peer changes.
    Shared,
    /// Slave propagation: receives but does not transmit.
    Slave,
}

impl Default for MountPropagation {
    fn default() -> Self {
        Self::Private
    }
}

impl ExecutionSpec {
    /// Toggle the isolation requirement (builder helper used by tests
    /// and composition code).
    pub fn with_isolation(mut self, iso: RequiresIsolation) -> Self {
        self.isolation = iso;
        self
    }

    /// Replace the workspace mounts (builder helper used by tests
    /// and composition code).
    pub fn with_mounts(mut self, mounts: Vec<WorkspaceMount>) -> Self {
        self.mounts = mounts;
        self
    }

    /// Construct a minimal spec for the common case (test-only
    /// execution, no bounds, no mounts).
    ///
    /// All identity fields are validated. The spec carries:
    ///
    /// - `scope`: default workspace + `SnapshotId(0)` (kernel sentinel).
    /// - `actor`: `System("test")`.
    /// - `correlation`: caller-supplied (must be non-empty).
    ///
    /// Intended for **tests and obvious in-memory use**. Production
    /// callers should populate identity via [`Self::try_new`].
    pub fn try_new(
        work_id: WorkId,
        program: impl Into<String>,
        argv: Vec<String>,
        cwd: PathBuf,
        correlation: &str,
    ) -> Result<Self, SpecError> {
        let correlation =
            CorrelationId::new(correlation).map_err(|_| SpecError::EmptyCorrelation)?;
        let actor = ActorRef::system("test");
        let scope = AnalysisScope::new(WorkspaceId::default(), SnapshotId::new(0));
        Ok(Self {
            work_id,
            program: program.into(),
            argv,
            cwd,
            env: vec![],
            isolation: RequiresIsolation::No,
            bounds: ExecutionBounds::default(),
            scope,
            actor,
            correlation,
            mounts: vec![],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::naming::NamespacedName;

    fn work_id(s: &str) -> WorkId {
        WorkId::new(NamespacedName::new(s).unwrap())
    }

    #[test]
    fn requires_isolation_predicate_matches_only_yes() {
        assert!(!RequiresIsolation::No.requires_isolation());
        assert!(RequiresIsolation::Yes.requires_isolation());
    }

    #[test]
    fn try_new_builds_a_well_formed_spec() {
        let spec = ExecutionSpec::try_new(
            work_id("ci.demo"),
            "/bin/true",
            vec!["--version".into()],
            PathBuf::from("/tmp/demo"),
            "trial-1",
        )
        .expect("spec");
        assert_eq!(spec.program, "/bin/true");
        assert_eq!(spec.argv.len(), 1);
        assert!(spec.env.is_empty());
        assert!(!spec.isolation.requires_isolation());
        assert!(spec.mounts.is_empty());
        assert!(spec.bounds.wall_clock.is_none());
        assert_eq!(spec.correlation.as_str(), "trial-1");
        assert_eq!(spec.actor.to_string(), "system:test");
    }

    #[test]
    fn try_new_rejects_empty_correlation() {
        let err = ExecutionSpec::try_new(
            work_id("ci.demo"),
            "/bin/true",
            vec![],
            PathBuf::from("/tmp/demo"),
            "   ",
        )
        .unwrap_err();
        assert_eq!(err, SpecError::EmptyCorrelation);
    }

    #[test]
    fn execution_spec_carries_identity_pins_for_correlation() {
        // The spec is supposed to thread M7.2 identity through to the
        // outcome. A round-trip equal-asserting proves the field set
        // is stable.
        let spec = ExecutionSpec::try_new(
            work_id("ci.demo"),
            "/bin/echo",
            vec!["hello".into()],
            PathBuf::from("/tmp"),
            "trial-mnt",
        )
        .expect("spec");
        let spec = spec_with_bounds_and_mount(spec);
        let cloned = spec.clone();
        assert_eq!(cloned, spec);
        assert!(cloned.isolation.requires_isolation());
        assert!(cloned.mounts.first().expect("mount").read_only);
    }

    /// Decorate a try_new-built spec with bounds + isolation + mounts.
    fn spec_with_bounds_and_mount(mut s: ExecutionSpec) -> ExecutionSpec {
        s.isolation = RequiresIsolation::Yes;
        s.bounds = ExecutionBounds {
            wall_clock: Some(Duration::from_millis(500)),
            memory_bytes: Some(64 * 1024 * 1024),
            cpu_micros: Some(500_000),
        };
        s.mounts = vec![WorkspaceMount {
            host_path: PathBuf::from("/tmp"),
            container_path: PathBuf::from("/work"),
            read_only: true,
            propagation: MountPropagation::Private,
        }];
        s
    }

    #[test]
    fn execution_spec_no_platform_specific_names_are_leaked() {
        let src = super::super::strip_doc_comments_and_tests(include_str!("spec.rs"));
        for forbidden in ["podman", "systemd", "quadlet", "wsl", "hyper-v", "docker"] {
            assert!(
                !src.to_lowercase().contains(forbidden),
                "ExecutionSpec must not leak platform-specific symbol in code: {forbidden}"
            );
        }
    }
}
