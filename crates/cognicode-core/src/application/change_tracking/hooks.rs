//! CAA-10 — opt-in Git hooks as thin clients of the coordinator.
//!
//! Hook scripts are disposable shims: they exec the CogniCode binary,
//! which reads the exact staged index (CAA-03 adapter) and consults the
//! cost-aware coordinator (CAA-09) instead of re-scanning the tree.
//! Installation is opt-in and reversible; a managed script is identified
//! by [`MANAGED_MARKER`] so foreign hooks and `core.hooksPath` targets
//! are never overwritten, only honoured or left alone.
//!
//! Trust boundary: scripts embed the absolute path of the binary chosen
//! by the person who ran `hooks install` — never repository content —
//! so a malicious checkout cannot redirect the gate to its own code.

use crate::domain::scm::{ChangeKind, ChangeSet};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

/// Marker line identifying a CogniCode-managed hook script.
pub const MANAGED_MARKER: &str = "cognicode:managed-hook v1";

/// Git events the managed hooks cover.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookEvent {
    PreCommit,
    PrePush,
    PostCheckout,
    PostMerge,
    PostRewrite,
}

impl HookEvent {
    pub const ALL: [HookEvent; 5] = [
        HookEvent::PreCommit,
        HookEvent::PrePush,
        HookEvent::PostCheckout,
        HookEvent::PostMerge,
        HookEvent::PostRewrite,
    ];

    pub fn git_name(self) -> &'static str {
        match self {
            HookEvent::PreCommit => "pre-commit",
            HookEvent::PrePush => "pre-push",
            HookEvent::PostCheckout => "post-checkout",
            HookEvent::PostMerge => "post-merge",
            HookEvent::PostRewrite => "post-rewrite",
        }
    }

    pub fn from_git_name(name: &str) -> Option<HookEvent> {
        HookEvent::ALL.into_iter().find(|e| e.git_name() == name)
    }

    /// Gate events can stop the operation; observe events never do.
    pub fn is_gate(self) -> bool {
        matches!(self, HookEvent::PreCommit | HookEvent::PrePush)
    }
}

/// Errors raised by hook installation and resolution.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HookError {
    #[error("git could not resolve the hooks directory: {reason}")]
    GitResolution { reason: String },
    #[error("io error at {path}: {reason}")]
    Io { path: String, reason: String },
}

/// Renders the managed shim. `program` is the absolute path to the
/// CogniCode binary, chosen by the installer, never by the repository.
pub fn render_script(event: HookEvent, program: &Path) -> String {
    format!(
        "#!/bin/sh\n# {MANAGED_MARKER}\nexec '{}' hooks run {} \"$@\"\n",
        program.display(),
        event.git_name()
    )
}

/// Who currently owns a hook slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotState {
    Absent,
    Managed,
    Foreign,
}

fn slot_at(path: &Path) -> Result<SlotState, HookError> {
    match std::fs::read(path) {
        Ok(bytes) => {
            if String::from_utf8_lossy(&bytes).contains(MANAGED_MARKER) {
                Ok(SlotState::Managed)
            } else {
                Ok(SlotState::Foreign)
            }
        }
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(SlotState::Absent),
        Err(error) => Err(io_error(path, &error)),
    }
}

fn io_error(path: &Path, error: &std::io::Error) -> HookError {
    HookError::Io {
        path: path.display().to_string(),
        reason: error.to_string(),
    }
}

/// Outcome of [`install`]: what was written, refreshed or refused.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InstallReport {
    pub installed: Vec<HookEvent>,
    pub refreshed: Vec<HookEvent>,
    pub foreign_kept: Vec<HookEvent>,
}

/// Outcome of [`uninstall`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UninstallReport {
    pub removed: Vec<HookEvent>,
    pub foreign_kept: Vec<HookEvent>,
}

/// Installs managed scripts into `dir`, creating it when missing.
/// Managed slots are rewritten (idempotent); foreign slots are kept.
pub fn install(dir: &Path, program: &Path) -> Result<InstallReport, HookError> {
    std::fs::create_dir_all(dir).map_err(|error| io_error(dir, &error))?;
    let mut report = InstallReport::default();
    for event in HookEvent::ALL {
        let path = dir.join(event.git_name());
        match slot_at(&path)? {
            SlotState::Absent => {
                write_script(&path, event, program)?;
                report.installed.push(event);
            }
            SlotState::Managed => {
                write_script(&path, event, program)?;
                report.refreshed.push(event);
            }
            SlotState::Foreign => report.foreign_kept.push(event),
        }
    }
    Ok(report)
}

fn write_script(path: &Path, event: HookEvent, program: &Path) -> Result<(), HookError> {
    std::fs::write(path, render_script(event, program)).map_err(|error| io_error(path, &error))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
            .map_err(|error| io_error(path, &error))?;
    }
    Ok(())
}

/// Removes only managed scripts; foreign slots are kept.
pub fn uninstall(dir: &Path) -> Result<UninstallReport, HookError> {
    let mut report = UninstallReport::default();
    for event in HookEvent::ALL {
        let path = dir.join(event.git_name());
        match slot_at(&path)? {
            SlotState::Managed => {
                std::fs::remove_file(&path).map_err(|error| io_error(&path, &error))?;
                report.removed.push(event);
            }
            SlotState::Foreign => report.foreign_kept.push(event),
            SlotState::Absent => {}
        }
    }
    Ok(report)
}

/// Ownership of every managed slot in `dir`.
pub fn status(dir: &Path) -> Result<Vec<(HookEvent, SlotState)>, HookError> {
    HookEvent::ALL
        .into_iter()
        .map(|event| slot_at(&dir.join(event.git_name())).map(|state| (event, state)))
        .collect()
}

/// Abstraction over `git` so resolution is testable without a binary.
pub trait CommandRunner {
    fn run(&self, args: &[&str]) -> Result<(i32, String), HookError>;
}

/// Runs the real `git` binary.
pub struct SystemGit {
    program: PathBuf,
}

impl SystemGit {
    pub fn new() -> Self {
        Self {
            program: PathBuf::from("git"),
        }
    }

    pub fn with_program(program: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
        }
    }
}

impl Default for SystemGit {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandRunner for SystemGit {
    fn run(&self, args: &[&str]) -> Result<(i32, String), HookError> {
        let output = std::process::Command::new(&self.program)
            .args(args)
            .output()
            .map_err(|error| HookError::Io {
                path: self.program.display().to_string(),
                reason: error.to_string(),
            })?;
        Ok((
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stdout).trim().to_string(),
        ))
    }
}

/// Resolves the effective hooks directory for `repo`, honouring
/// `core.hooksPath` and worktrees (git maps both).
pub fn resolve_hooks_dir(runner: &dyn CommandRunner, repo: &Path) -> Result<PathBuf, HookError> {
    let repo_str = repo.display().to_string();
    let (code, stdout) = runner.run(&["-C", &repo_str, "rev-parse", "--git-path", "hooks"])?;
    if code != 0 || stdout.is_empty() {
        return Err(HookError::GitResolution {
            reason: format!("git rev-parse exited {code}: {stdout}"),
        });
    }
    let resolved = PathBuf::from(&stdout);
    Ok(if resolved.is_relative() {
        repo.join(resolved)
    } else {
        resolved
    })
}

/// What the injected gate analysis concluded about the pending change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Pass,
    NotPass(Vec<String>),
    /// The analysis could not run to completion (crash, timeout, absent
    /// infrastructure). Gate events must treat this as refusal.
    Inconclusive(String),
}

/// Final instruction to the hook shim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookDecision {
    Proceed,
    Block { reasons: Vec<String> },
}

impl HookDecision {
    pub fn exit_code(&self) -> i32 {
        match self {
            HookDecision::Proceed => 0,
            HookDecision::Block { .. } => 1,
        }
    }
}

/// Built-in staged-hygiene analysis: unmerged paths (`Unknown` changes
/// from the exact index) cannot be committed honestly.
pub fn verdict_from_staged(staged: &ChangeSet) -> Verdict {
    let unmerged: Vec<String> = staged
        .changes()
        .iter()
        .filter(|change| matches!(change.kind(), ChangeKind::Unknown))
        .map(|change| format!("unmerged path {}", change.path().as_str()))
        .collect();
    if unmerged.is_empty() {
        Verdict::Pass
    } else {
        Verdict::NotPass(unmerged)
    }
}

/// Maps event + verdict to a decision. Observe events never block and
/// never mint approvals; gate events fail closed on any non-Pass.
pub fn decide(event: HookEvent, verdict: &Verdict) -> HookDecision {
    if !event.is_gate() {
        return HookDecision::Proceed;
    }
    match verdict {
        Verdict::Pass => HookDecision::Proceed,
        Verdict::NotPass(reasons) => HookDecision::Block {
            reasons: reasons.clone(),
        },
        Verdict::Inconclusive(reason) => HookDecision::Block {
            reasons: vec![format!(
                "gate could not complete: {reason} (failing closed)"
            )],
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::scm::{Change, ContentDigest, PathKey, RevisionHandle};
    use std::cell::RefCell;

    fn path(value: &str) -> PathKey {
        PathKey::new(value).expect("valid path")
    }

    fn digest() -> ContentDigest {
        ContentDigest::from_bytes(b"payload")
    }

    fn handle() -> RevisionHandle {
        RevisionHandle::new("rev").expect("non-empty handle")
    }

    fn staged_with(changes: Vec<Change>) -> ChangeSet {
        ChangeSet::explicit(handle(), handle(), changes)
    }

    #[test]
    fn events_round_trip_names_and_gate_flags() {
        for event in HookEvent::ALL {
            assert_eq!(HookEvent::from_git_name(event.git_name()), Some(event));
        }
        assert_eq!(HookEvent::from_git_name("commit-msg"), None);
        assert!(HookEvent::PreCommit.is_gate() && HookEvent::PrePush.is_gate());
        assert!(!HookEvent::PostCheckout.is_gate());
        assert!(!HookEvent::PostMerge.is_gate());
        assert!(!HookEvent::PostRewrite.is_gate());
    }

    #[test]
    fn script_is_a_thin_exec_of_the_installer_chosen_binary() {
        let script = render_script(HookEvent::PreCommit, Path::new("/opt/cognicode/cognicode"));
        assert!(script.contains(MANAGED_MARKER));
        assert!(script.starts_with("#!/bin/sh\n"));
        assert!(script.contains("exec '/opt/cognicode/cognicode' hooks run pre-commit \"$@\""));
        // No repository-controlled interpolation beyond the event name.
        assert!(!script.contains("eval"));
    }

    #[test]
    fn install_is_opt_in_idempotent_and_never_touches_foreign_slots() {
        let dir = std::env::temp_dir().join(format!("cgh028-install-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let foreign = dir.join("pre-commit");
        // Directory does not exist yet: install creates it.
        let report = install(&dir, Path::new("/bin/cognicode")).expect("install");
        assert_eq!(report.installed.len(), 5);
        assert!(dir.join("post-rewrite").exists());

        // Second install refreshes instead of duplicating.
        let report = install(&dir, Path::new("/bin/cognicode")).expect("reinstall");
        assert!(report.installed.is_empty());
        assert_eq!(report.refreshed.len(), 5);

        // A third-party hook is preserved byte-for-byte.
        std::fs::write(&foreign, b"#!/bin/sh\nexit 0\n").expect("foreign");
        let report = install(&dir, Path::new("/bin/cognicode")).expect("with foreign");
        assert_eq!(report.foreign_kept, vec![HookEvent::PreCommit]);
        assert_eq!(
            std::fs::read(&foreign).expect("read"),
            b"#!/bin/sh\nexit 0\n"
        );

        // Uninstall removes only managed scripts and is reversible.
        let report = uninstall(&dir).expect("uninstall");
        assert_eq!(report.removed.len(), 4);
        assert_eq!(report.foreign_kept, vec![HookEvent::PreCommit]);
        assert!(foreign.exists());
        assert!(!dir.join("pre-push").exists());
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn status_reports_ownership_per_slot() {
        let dir = std::env::temp_dir().join(format!("cgh028-status-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        install(&dir, Path::new("/bin/cognicode")).expect("install");
        std::fs::write(dir.join("post-merge"), b"#!/bin/sh\n").expect("foreign");
        let entries = status(&dir).expect("status");
        let owned = |event: HookEvent| {
            entries
                .iter()
                .find(|(slot, _)| *slot == event)
                .map(|(_, state)| *state)
                .expect("slot")
        };
        assert_eq!(owned(HookEvent::PreCommit), SlotState::Managed);
        assert_eq!(owned(HookEvent::PostMerge), SlotState::Foreign);
        uninstall(&dir).expect("uninstall");
        let entries = status(&dir).expect("status after uninstall");
        assert!(
            entries
                .iter()
                .all(|(_, state)| matches!(state, SlotState::Absent | SlotState::Foreign))
        );
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    struct FakeRunner {
        stdout: String,
        code: i32,
        seen: RefCell<Vec<String>>,
    }

    impl CommandRunner for FakeRunner {
        fn run(&self, args: &[&str]) -> Result<(i32, String), HookError> {
            *self.seen.borrow_mut() = args.iter().map(|s| s.to_string()).collect();
            Ok((self.code, self.stdout.clone()))
        }
    }

    #[test]
    fn resolution_honours_relative_and_absolute_hook_paths() {
        let relative = FakeRunner {
            stdout: ".git/hooks".to_string(),
            code: 0,
            seen: RefCell::new(Vec::new()),
        };
        let dir = resolve_hooks_dir(&relative, Path::new("/repo")).expect("resolve");
        assert_eq!(dir, PathBuf::from("/repo/.git/hooks"));
        assert_eq!(
            relative.seen.borrow().as_slice(),
            ["-C", "/repo", "rev-parse", "--git-path", "hooks"].map(String::from)
        );

        let absolute = FakeRunner {
            stdout: "/shared/hooks".to_string(),
            code: 0,
            seen: RefCell::new(Vec::new()),
        };
        let dir = resolve_hooks_dir(&absolute, Path::new("/repo")).expect("resolve");
        assert_eq!(dir, PathBuf::from("/shared/hooks"));

        let failing = FakeRunner {
            stdout: String::new(),
            code: 128,
            seen: RefCell::new(Vec::new()),
        };
        let error = resolve_hooks_dir(&failing, Path::new("/repo")).expect_err("must fail");
        assert!(matches!(error, HookError::GitResolution { .. }));
    }

    #[test]
    fn unmerged_index_paths_fail_the_staged_verdict() {
        let clean = staged_with(vec![Change::modified(path("src/a.rs"), digest(), digest())]);
        assert_eq!(verdict_from_staged(&clean), Verdict::Pass);

        let empty = staged_with(vec![]);
        assert_eq!(verdict_from_staged(&empty), Verdict::Pass);

        let conflicted = staged_with(vec![Change::unknown(path("src/b.rs"))]);
        match verdict_from_staged(&conflicted) {
            Verdict::NotPass(reasons) => {
                assert_eq!(reasons, vec!["unmerged path src/b.rs".to_string()])
            }
            other => panic!("expected NotPass, got {other:?}"),
        }
    }

    #[test]
    fn gate_events_block_inconclusive_while_observe_events_never_do() {
        // AC#3: a gate that cannot complete fails explicitly.
        let verdict = Verdict::Inconclusive("index read refused".to_string());
        assert_eq!(
            decide(HookEvent::PreCommit, &verdict),
            HookDecision::Block {
                reasons: vec![
                    "gate could not complete: index read refused (failing closed)".to_string()
                ]
            }
        );
        // AC#3: non-blocking events never mint approvals nor block.
        assert_eq!(
            decide(HookEvent::PostMerge, &verdict),
            HookDecision::Proceed
        );
        assert_eq!(
            decide(
                HookEvent::PostCheckout,
                &Verdict::NotPass(vec!["finding".to_string()])
            ),
            HookDecision::Proceed
        );
        assert_eq!(
            decide(HookEvent::PrePush, &Verdict::Pass),
            HookDecision::Proceed
        );
        assert_eq!(HookDecision::Proceed.exit_code(), 0);
        assert_eq!(decide(HookEvent::PreCommit, &verdict).exit_code(), 1);
    }
}
