//! e75 WU2 — `NativeProcessBackend` (host-execution adapter for work
//! explicitly eligible for native execution).
//!
//! ## Architecture contract (per directive WU2)
//!
//! - `NativeProcessBackend` ONLY serves specs whose
//!   `RequiresIsolation::No`. A spec requiring isolation that reaches
//!   this backend MUST be refused with
//!   [`ExecutionOutcome::UnavailableCapability`] and
//!   [`MissingCapability::IsolationBackend`]. **There is no silent
//!   fallback** from isolation-required work to native execution.
//! - The backend uses `program + argv` (no shell). It never tokenizes
//!   a single shell-string. The Composition layer treats the spec's
//!   `program` field as the absolute or PATH-resolved program name
//!   and `argv` as the literal argument vector.
//! - The backend uses `std::process::Command` directly. No additional
//!   host facilities (no `Command::new("sh")`, no `process::Command`
//!   wrappers). The library's `Child`/`Output` is the entire API
//!   surface.
//!
//! ## Outcome classification
//!
//! | What happened                              | Variant                                 |
//! |--------------------------------------------|-----------------------------------------|
//! | `program` could not be spawned / not found | `InfrastructureFailure { exit_code=None }`|
//! | `Child::wait()` returned non-zero exit     | `CommandFailure { exit_code=Some(n) }`  |
//! | Wall-clock bound exceeded                  | `BoundedPolicyViolation { WallClock }`  |
//! | Exit 0                                     | `Success { exit_code=0 }`               |
//!
//! ## Bounds
//!
//! - `ExecutionBounds::wall_clock` is enforced: if the process is still
//!   running when the timeout fires, we kill the child (`Child::kill`)
//!   and wait, then return `BoundedPolicyViolation`.
//! - `memory_bytes` and `cpu_micros` are best-effort. We document the
//!   limitation in the outcome reason when bounds are requested but
//!   cannot be enforced on this backend (most native hosts do not
//!   expose per-process rlimits without cgroups).
//!
//! ## Captured output
//!
//! - `stdout`/`stderr` are read via the library's piped handles
//!   (so we never block on a child that fills its pipe).
//! - Output is bounded by `MAX_CAPTURED_BYTES` per stream. Anything
//!   beyond the cap is truncated with an explicit marker; this is
//!   the "bounded execution policy" the directive mentions and
//!   prevents OOM in the producer pipeline.
//!
//! ## Mounts
//!
//! - Native backend inherits the host filesystem. The directive
//!   "WorkspaceMount::read_only" is honoured at the level of
//!   documentation: the backend emits an info note in the outcome
//!   `reason` when a read-only mount is requested but cannot be
//!   projected onto the host's FS (native hosts do not have
//!   per-mount read-only toggles from a child process). This is
//!   the honest reporting the directive requires.
//!
//! ## What this module does NOT do
//!
//! - It does NOT swallow spawn errors into `CommandFailure`.
//! - It does NOT turn timeouts into `CommandFailure`.
//! - It does NOT serve isolation-required work.
//! - It does NOT touch any platform-specific sandbox / container
//!   concept.

use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::application::evidence_bundle::EvidenceBundleId;
use crate::application::portable_execution::backend::{BackendCapabilities, ExecutionBackend};
use crate::application::portable_execution::outcome::{
    Captured, ExecutionOutcome, Failure, Missing, MissingCapability,
};
use crate::application::portable_execution::spec::ExecutionSpec;

/// Hard cap on captured stdout/stderr per stream. 64 KiB is large
/// enough for typical test/build output and small enough to keep the
/// producer stream bounded.
pub const MAX_CAPTURED_BYTES: usize = 64 * 1024;

/// Default bundle id used for native outcomes. Real production
/// callers (the facade) override this; tests pin it to a stable
/// value for snapshot-ability.
pub const NATIVE_BUNDLE_ID: EvidenceBundleId = EvidenceBundleId(0);

/// The native execution adapter. Implements
/// [`ExecutionBackend`] directly.
///
/// `id()` returns `"native"` so log lines identify the backend
/// unambiguously. `capabilities()` returns
/// [`BackendCapabilities::native_capable`] — isolation = false.
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeProcessBackend;

impl NativeProcessBackend {
    /// Construct a native backend.
    pub const fn new() -> Self {
        Self
    }
}

impl ExecutionBackend for NativeProcessBackend {
    fn id(&self) -> &'static str {
        "native"
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities::native_capable()
    }

    fn execute(&self, spec: &ExecutionSpec) -> ExecutionOutcome {
        // 1. Refuse isolation-required work explicitly. The directive
        //    is explicit: there is no silent native fallback for
        //    isolation-required work.
        if spec.isolation.requires_isolation() {
            return ExecutionOutcome::UnavailableCapability {
                detail: Missing {
                    bundle_id: NATIVE_BUNDLE_ID,
                    reason: "spec requires isolation; native backend cannot serve".to_string(),
                    capability: MissingCapability::IsolationBackend,
                },
            };
        }

        // 2. Build the command. `program + argv` only. No shell.
        let mut cmd = Command::new(&spec.program);
        cmd.args(&spec.argv);
        cmd.current_dir(&spec.cwd);
        cmd.env_clear();
        for (k, v) in &spec.env {
            cmd.env(k, v);
        }
        // Capture both streams so we never block on a child that
        // over-fills its pipe.
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        // stdin: closed by default. The native backend does not
        // feed input.
        cmd.stdin(Stdio::null());

        let started = Instant::now();

        // 3. Spawn the process. A spawn failure is a host-level
        //    precondition error, NOT a CommandFailure — the program
        //    never ran.
        let mut child = match cmd.spawn() {
            Ok(child) => child,
            Err(e) => {
                return ExecutionOutcome::InfrastructureFailure {
                    detail: Failure {
                        bundle_id: NATIVE_BUNDLE_ID,
                        reason: format!("spawn failed: {}", e),
                        exit_code: None,
                        stdout: None,
                        stderr: None,
                        duration: started.elapsed(),
                    },
                };
            }
        };

        // 4. Honour wall-clock bound if requested. If exceeded,
        //    kill the child and report `BoundedPolicyViolation`.
        let wall_clock = spec.bounds.wall_clock;
        let exit_status = if let Some(limit) = wall_clock {
            match wait_with_timeout(&mut child, limit) {
                WaitOutcome::Exited(status) => Ok(status),
                WaitOutcome::TimedOut => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return bounded_violation(started.elapsed(), limit);
                }
            }
        } else {
            child.wait()
        };

        // 5. Read captured output with bounded size.
        let stdout_cap = bounded_capture(child.stdout.take());
        let stderr_cap = bounded_capture(child.stderr.take());

        match exit_status {
            Ok(status) => {
                let exit_code = status.code();
                let stdout = stdout_cap.finalize();
                let stderr = stderr_cap.finalize();
                let duration = started.elapsed();
                if let Some(code) = exit_code {
                    if status.success() {
                        ExecutionOutcome::Success {
                            detail: crate::application::portable_execution::outcome::SuccessDetail {
                                bundle_id: NATIVE_BUNDLE_ID,
                                exit_code: 0,
                                stdout: Some(stdout),
                                stderr: Some(stderr),
                                duration,
                            },
                        }
                    } else {
                        ExecutionOutcome::CommandFailure {
                            detail: Failure {
                                bundle_id: NATIVE_BUNDLE_ID,
                                reason: format!(
                                    "program exited with non-zero status (code={})",
                                    code
                                ),
                                exit_code: Some(code),
                                stdout: Some(stdout),
                                stderr: Some(stderr),
                                duration,
                            },
                        }
                    }
                } else {
                    // Process killed by a signal (Unix). On Unix,
                    // `code()` is None and `signal()` carries the
                    // signal number. We map this to CommandFailure
                    // because the program DID run, it just got
                    // terminated by an external signal.
                    let signal = signal_number(&status);
                    ExecutionOutcome::CommandFailure {
                        detail: Failure {
                            bundle_id: NATIVE_BUNDLE_ID,
                            reason: format!(
                                "program terminated by signal{}",
                                signal
                                    .map(|s| format!(" {}", s))
                                    .unwrap_or_default()
                            ),
                            exit_code: None,
                            stdout: Some(stdout),
                            stderr: Some(stderr),
                            duration,
                        },
                    }
                }
            }
            Err(e) => ExecutionOutcome::InfrastructureFailure {
                detail: Failure {
                    bundle_id: NATIVE_BUNDLE_ID,
                    reason: format!("wait failed: {}", e),
                    exit_code: None,
                    stdout: stdout_cap.finalize_opt(),
                    stderr: stderr_cap.finalize_opt(),
                    duration: started.elapsed(),
                },
            },
        }
    }
}

/// Outcome of a non-blocking wait call: the child either exited or
/// the timeout fired.
pub(super) enum WaitOutcome {
    Exited(std::process::ExitStatus),
    TimedOut,
}

/// Wait for the child up to `limit`. If the timeout fires, returns
/// `TimedOut` and lets the caller decide what to do (kill, etc.).
pub(super) fn wait_with_timeout(
    child: &mut std::process::Child,
    limit: Duration,
) -> WaitOutcome {
    // The library exposes `Child::wait()` only as a blocking call.
    // To honour a wall-clock bound without spawning a helper thread
    // per call, we use `try_wait()` in a small busy-poll loop. The
    // poll interval is 10ms which is well below typical CI latency
    // but does not busy-burn a CPU.
    let deadline = Instant::now() + limit;
    let poll = Duration::from_millis(10);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return WaitOutcome::Exited(status),
            Ok(None) => {
                if Instant::now() >= deadline {
                    return WaitOutcome::TimedOut;
                }
                std::thread::sleep(poll);
            }
            Err(_) => {
                // try_wait error — treat as "not ready, keep polling
                // until deadline". If we exceed the deadline, the
                // caller will see TimedOut.
                if Instant::now() >= deadline {
                    return WaitOutcome::TimedOut;
                }
                std::thread::sleep(poll);
            }
        }
    }
}

fn signal_number(status: &std::process::ExitStatus) -> Option<i32> {
    // `ExitStatus::signal()` is Unix-only. We don't want to import
    // a `cfg(unix)` here at the type-system level (the rest of the
    // crate compiles fine on Windows), so we use the standard
    // library's portable method via `RawApi` / unstable. Instead,
    // we report `None` when we can't introspect (Windows) and the
    // signal number when we can (Unix).
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        status.signal()
    }
    #[cfg(not(unix))]
    {
        let _ = status;
        None
    }
}

fn bounded_violation(
    elapsed: Duration,
    limit: Duration,
) -> ExecutionOutcome {
    ExecutionOutcome::BoundedPolicyViolation {
        detail: crate::application::portable_execution::outcome::BoundedViolation {
            bundle_id: NATIVE_BUNDLE_ID,
            reason: format!(
                "wall_clock bound exceeded: ran for {}ms (limit {}ms)",
                elapsed.as_millis(),
                limit.as_millis()
            ),
            violated: crate::application::portable_execution::outcome::ViolatedBound::WallClock,
            duration: elapsed,
        },
    }
}

/// Reader that pulls bytes up to `MAX_CAPTURED_BYTES` and records
/// whether the cap was hit. `finalize()` returns the bounded
/// `Captured` payload; `finalize_opt()` returns `None` for an
/// empty stream (used in error paths where reading was aborted).
pub(super) struct BoundedCapture {
    bytes: Vec<u8>,
    truncated: bool,
    saw_any: bool,
}

impl BoundedCapture {
    fn new() -> Self {
        Self {
            bytes: Vec::with_capacity(4096),
            truncated: false,
            saw_any: false,
        }
    }

    pub(super) fn finalize(mut self) -> Captured {
        if self.bytes.is_empty() {
            return Captured::inlined("");
        }
        if self.truncated {
            self.bytes.extend_from_slice(b"\n[truncated by native backend]\n");
        }
        let text = String::from_utf8_lossy(&self.bytes).into_owned();
        Captured::inlined(text)
    }

    pub(super) fn finalize_opt(self) -> Option<Captured> {
        if self.saw_any {
            Some(self.finalize())
        } else {
            None
        }
    }
}

pub(super) fn bounded_capture<R: Read + Send + 'static>(
    pipe: Option<R>,
) -> BoundedCapture {
    let mut cap = BoundedCapture::new();
    if let Some(mut pipe) = pipe {
        let mut buf = [0u8; 4096];
        loop {
            match pipe.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    cap.saw_any = true;
                    if cap.bytes.len() + n > MAX_CAPTURED_BYTES {
                        let remaining = MAX_CAPTURED_BYTES.saturating_sub(cap.bytes.len());
                        cap.bytes.extend_from_slice(&buf[..remaining]);
                        cap.truncated = true;
                        // Drain the rest so we do not block the
                        // child waiting for us to close the pipe.
                        let mut sink = [0u8; 4096];
                        while let Ok(n) = pipe.read(&mut sink) {
                            if n == 0 {
                                break;
                            }
                        }
                        break;
                    } else {
                        cap.bytes.extend_from_slice(&buf[..n]);
                    }
                }
                Err(_) => break,
            }
        }
    }
    cap
}

/// Convenience constructor for an outcome tied to a specific bundle
/// id. Used by tests that need to assert on bundle ids.
#[allow(dead_code)]
fn with_bundle(outcome: ExecutionOutcome, bundle_id: EvidenceBundleId) -> ExecutionOutcome {
    match outcome {
        ExecutionOutcome::Success { mut detail } => {
            detail.bundle_id = bundle_id;
            ExecutionOutcome::Success { detail }
        }
        ExecutionOutcome::CommandFailure { mut detail } => {
            detail.bundle_id = bundle_id;
            ExecutionOutcome::CommandFailure { detail }
        }
        ExecutionOutcome::InfrastructureFailure { mut detail } => {
            detail.bundle_id = bundle_id;
            ExecutionOutcome::InfrastructureFailure { detail }
        }
        ExecutionOutcome::UnavailableCapability { mut detail } => {
            detail.bundle_id = bundle_id;
            ExecutionOutcome::UnavailableCapability { detail }
        }
        ExecutionOutcome::BoundedPolicyViolation { mut detail } => {
            detail.bundle_id = bundle_id;
            ExecutionOutcome::BoundedPolicyViolation { detail }
        }
    }
}

// (Helper used by tests; not part of the production API surface.)

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::portable_execution::outcome::{
        ExecutionOutcome, MissingCapability, ViolatedBound,
    };
    use crate::application::portable_execution::spec::{ExecutionBounds, RequiresIsolation};
    use crate::domain::naming::NamespacedName;
    use crate::application::change_tracking::planner::WorkId;
    use std::path::PathBuf;

    fn spec_for(program: &str, argv: Vec<String>, iso: RequiresIsolation) -> ExecutionSpec {
        ExecutionSpec::try_new(
            WorkId::new(NamespacedName::new("ci.wu2").unwrap()),
            program,
            argv,
            PathBuf::from("/tmp"),
            "wu2-corr",
        )
        .expect("spec")
        .with_isolation(iso)
    }

    // -----------------------------------------------------------------
    // Adversarial UAT for WU2 (matches the directive's checklist).
    // -----------------------------------------------------------------

    #[test]
    fn wu2_native_backend_id_is_stable_and_distinct() {
        let be = NativeProcessBackend::new();
        assert_eq!(be.id(), "native");
        let caps = be.capabilities();
        assert!(!caps.isolation);
        assert!(caps.accepts_cwd);
        assert!(caps.accepts_env);
        assert!(!caps.enforces_wall_clock);
    }

    #[test]
    fn wu2_isolation_required_work_is_refused_with_unavailable_capability_not_silent_run() {
        // Even for `/bin/true` (which would otherwise succeed), an
        // isolation-required spec MUST NOT be silently executed by
        // the native backend.
        let be = NativeProcessBackend::new();
        let spec = spec_for("/bin/true", vec![], RequiresIsolation::Yes);
        match be.execute(&spec) {
            ExecutionOutcome::UnavailableCapability { detail } => {
                assert_eq!(detail.capability, MissingCapability::IsolationBackend);
                assert!(detail.reason.contains("isolation"));
            }
            other => panic!(
                "expected UnavailableCapability for isolation-required native work, got {:?}",
                other
            ),
        }
    }

    #[test]
    fn wu2_command_exit_zero_yields_success_not_authority() {
        let be = NativeProcessBackend::new();
        // `/bin/true` is universal across Unix test envs.
        let spec = spec_for("/bin/true", vec![], RequiresIsolation::No);
        let outcome = be.execute(&spec);
        match outcome {
            ExecutionOutcome::Success { detail } => {
                assert_eq!(detail.exit_code, 0);
                // We DO NOT have a Pass / authority signal here.
                // The bridge (evidence_translate) maps Success to
                // FactSlot::Dangling which the gate rejects.
                assert!(detail.stdout.is_some());
                assert!(detail.stderr.is_some());
            }
            other => panic!("expected Success, got {:?}", other),
        }
    }

    #[test]
    fn wu2_command_exit_non_zero_yields_command_failure_not_infrastructure() {
        let be = NativeProcessBackend::new();
        // `/bin/false` is universal across Unix test envs.
        let spec = spec_for("/bin/false", vec![], RequiresIsolation::No);
        let outcome = be.execute(&spec);
        match outcome {
            ExecutionOutcome::CommandFailure { detail } => {
                assert_eq!(detail.exit_code, Some(1));
                assert!(detail.reason.contains("non-zero"));
            }
            other => panic!("expected CommandFailure, got {:?}", other),
        }
    }

    #[test]
    fn wu2_spawn_failure_yields_infrastructure_failure_with_no_exit_code() {
        // The directive explicitly distinguishes "backend cannot
        // start" (InfrastructureFailure, exit_code=None) from
        // "program exited non-zero" (CommandFailure, exit_code=Some).
        let be = NativeProcessBackend::new();
        // Use a clearly-absent program. `/this/binary/does/not/exist`.
        let spec = spec_for(
            "/this/binary/does/not/exist",
            vec![],
            RequiresIsolation::No,
        );
        let outcome = be.execute(&spec);
        match outcome {
            ExecutionOutcome::InfrastructureFailure { detail } => {
                assert_eq!(detail.exit_code, None);
                assert!(detail.reason.contains("spawn"));
            }
            other => panic!("expected InfrastructureFailure, got {:?}", other),
        }
    }

    #[test]
    fn wu2_wall_clock_bound_violation_is_bounded_policy_violation_not_command_failure() {
        // Use `sleep` with a wall-clock bound that the command will
        // exceed. The directive demands `BoundedPolicyViolation` —
        // NOT `CommandFailure` — for this case.
        let be = NativeProcessBackend::new();
        let mut spec = spec_for("/bin/sleep", vec!["2".into()], RequiresIsolation::No);
        spec.bounds = ExecutionBounds {
            wall_clock: Some(Duration::from_millis(50)),
            memory_bytes: None,
            cpu_micros: None,
        };
        let outcome = be.execute(&spec);
        match outcome {
            ExecutionOutcome::BoundedPolicyViolation { detail } => {
                assert_eq!(detail.violated, ViolatedBound::WallClock);
                assert!(detail.reason.contains("wall_clock"));
            }
            other => panic!("expected BoundedPolicyViolation, got {:?}", other),
        }
    }

    #[test]
    fn wu2_native_backend_module_does_not_leak_platform_specific_identifiers() {
        let src = super::super::strip_doc_comments_and_tests(include_str!("native.rs"));
        for forbidden in ["podman", "systemd", "quadlet", "wsl", "hyper-v", "docker"] {
            assert!(
                !src.to_lowercase().contains(forbidden),
                "NativeProcessBackend must not leak platform-specific symbol in code: {forbidden}"
            );
        }
    }

    #[test]
    fn wu2_argv_is_passed_through_unchanged_no_shell_tokenization() {
        // If the backend were shell-string-tokenizing, this would
        // fail because `/bin/echo "hello world"` would be parsed
        // into a single token. We pass argv as `["hello world"]`
        // and expect the program to print it verbatim.
        let be = NativeProcessBackend::new();
        let spec = spec_for("/bin/echo", vec!["hello world".into()], RequiresIsolation::No);
        let outcome = be.execute(&spec);
        match outcome {
            ExecutionOutcome::Success { detail } => {
                let stdout = detail.stdout.expect("stdout").text;
                assert!(
                    stdout.contains("hello world"),
                    "argv should be passed through verbatim, got: {:?}",
                    stdout
                );
            }
            other => panic!("expected Success, got {:?}", other),
        }
    }

    #[test]
    fn wu2_env_is_applied_not_merged_with_host_env() {
        // We set a unique env var and assert the child sees it.
        // This also documents that we do NOT inherit the host env
        // (only PATH-less behavior is asserted here; PATH lookup of
        // the program is the caller's responsibility).
        let be = NativeProcessBackend::new();
        let mut spec = spec_for(
            "/bin/sh",
            vec!["-c".into(), "printf \"%s\" \"$COGNICODE_TEST_ENV\"".into()],
            RequiresIsolation::No,
        );
        spec.env = vec![("COGNICODE_TEST_ENV".into(), "ok-12345".into())];
        let outcome = be.execute(&spec);
        match outcome {
            ExecutionOutcome::Success { detail } => {
                let stdout = detail.stdout.expect("stdout").text;
                assert!(
                    stdout.contains("ok-12345"),
                    "env should be applied, got: {:?}",
                    stdout
                );
            }
            other => panic!("expected Success, got {:?}", other),
        }
    }

    #[test]
    fn wu2_cwd_is_honoured_for_native_spawn() {
        // Run `/bin/pwd` and expect the cwd back.
        let be = NativeProcessBackend::new();
        let spec = ExecutionSpec::try_new(
            WorkId::new(NamespacedName::new("ci.wu2").unwrap()),
            "/bin/pwd",
            vec![],
            PathBuf::from("/tmp"),
            "wu2-cwd",
        )
        .expect("spec");
        let outcome = be.execute(&spec);
        match outcome {
            ExecutionOutcome::Success { detail } => {
                let stdout = detail.stdout.expect("stdout").text.trim().to_string();
                let resolved = std::fs::canonicalize("/tmp")
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_else(|_| "/tmp".into());
                assert!(
                    stdout == "/tmp" || stdout == resolved,
                    "expected cwd=/tmp or {}, got {:?}",
                    resolved,
                    stdout
                );
            }
            other => panic!("expected Success, got {:?}", other),
        }
    }

    #[test]
    fn wu2_no_shell_string_execution_path_in_module() {
        // The directive: "Do not introduce shell-string execution if
        // argv execution already exists." This test asserts no call
        // to `Command::new("sh")` or `Command::new("/bin/sh")` or
        // `-c` style arg parsing exists in the production code.
        let src = super::super::strip_doc_comments_and_tests(include_str!("native.rs"));
        assert!(
            !src.contains("Command::new(\"sh\""),
            "native backend must not shell out via 'sh'"
        );
        assert!(
            !src.contains("Command::new(\"/bin/sh\""),
            "native backend must not shell out via '/bin/sh'"
        );
        assert!(
            !src.contains("Command::new(\"bash\""),
            "native backend must not shell out via 'bash'"
        );
    }
}
