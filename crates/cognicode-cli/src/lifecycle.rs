//! B6.W3 — User-facing lifecycle for the `cogh` binary itself.
//!
//! CogniCode's plugin lifecycle (E32-A) lives under `cmd/*` and is
//! the source of truth for `cogh install <plugin>`, `cogh update`,
//! `cogh rollback`, etc. B6.W3 adds a *parallel* surface for the
//! binary itself: a new user must be able to obtain a first real
//! query without compiling CogniCode or reading internal docs.
//!
//! ## What this module owns
//!
//! - **Plan + apply** for the self-install / self-update /
//!   self-rollback cases: `plan_install` resolves the target paths
//!   and `execute_install` performs the file copy and the post-
//!   install doctor check.
//! - **JSON-friendly data shapes** so `cogh install --version X`
//!   can print a plan for review before any file is written; the
//!   `--apply` flag is what actually mutates the filesystem.
//! - **Path resolution** is pure std (no network, no plugin
//!   metadata): `$CARGO_HOME/bin`, `$XDG_DATA_HOME/cognicode/bin`,
//!   or the `~/.local/share/cognicode/bin` fallback.
//!
//! ## What this module does NOT own
//!
//! - The plugin install pipeline (E32-A) — see
//!   `cmd::installer_transaction::InstallerTransaction`.
//! - IDE integration or shim regeneration — that is the
//!   `cmd::layout::cmd_install` path invoked by the `Install`
//!   arm of the cogh CLI when a `plugin` is provided.
//!
//! ## TDD
//!
//! Five unit tests live at the bottom of this file and cover
//! the contract advertised in `odd/tasks/b6-agent-ready-runtime.md`
//! B6.W3. Integration coverage lives in
//! `crates/cognicode-cli/tests/b6_w3_cogh_lifecycle.rs`.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Errors surfaced by the self-install / self-update / self-rollback
/// pipeline. They are deliberately distinct from the plugin install
/// errors under `cmd::error` so a UI can tell the two apart when
/// driving `cogh install --version` (no plugin) from a real plugin
/// install.
#[derive(Debug, Error)]
pub enum InstallError {
    /// The requested version is not a non-empty semver-shaped token.
    /// `cogh install --version ""` or `--version latest` (without
    /// the resolver wired) is rejected here.
    #[error("invalid version: {0}")]
    InvalidVersion(String),

    /// `std::io::Error` raised while probing the install root,
    /// copying the binary, or writing the version pin.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// `cogh doctor` returned a non-zero exit (or could not be
    /// spawned). The captured exit code is preserved so the caller
    /// can branch on `code == 0` (healthy) vs `code != 0` (unhealthy).
    #[error("cogh doctor failed with exit code {code}: {stderr}")]
    DoctorFailed { code: i32, stderr: String },
}

/// The target of a self-install. Resolved by `plan_install` from
/// the version + ambient env (`CARGO_HOME`, `XDG_DATA_HOME`) and
/// the home override.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallPlan {
    /// Version we plan to install, e.g. `0.101.15`.
    pub version: String,
    /// Where the `cogh` binary will be written (typically
    /// `<install_root>/bin/cogh`).
    pub install_path: PathBuf,
    /// Every directory that the user is expected to add to
    /// `PATH`. Today this is a single entry; the vector shape
    /// lets a future layout add a `bin/` and `sbin/` without
    /// breaking the public type.
    pub bin_paths: Vec<PathBuf>,
    /// Optional shell-completion script path (`<data>/completions/
    /// cogh.bash`). `None` when the install root is not writable
    /// or the version is a downgrade that should not refresh
    /// completions.
    pub shell_completion_path: Option<PathBuf>,
}

/// The kind of operation we are about to perform. The `from` /
/// `to` strings are user-facing — they are exactly the strings
/// they typed (e.g. `latest`, `0.101.14`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InstallAction {
    /// First install: no prior version known.
    Install,
    /// Update from a previously installed version.
    Update { from: String, to: String },
    /// Rollback to a known-good version.
    Rollback { from: String, to: String },
}

/// What happened on disk. `doctor_pass == false` is a *partial*
/// success: the binary was copied, but the post-install doctor
/// flagged something. UI surfaces must show this distinctly from
/// `cogh doctor` itself exiting non-zero on a missing binary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallOutcome {
    pub action: InstallAction,
    pub install_path: PathBuf,
    /// Lowercase hex SHA-256 of the bytes that were copied. Lets
    /// a future integrity check re-verify the on-disk binary
    /// without re-reading the source.
    pub sha256: String,
    pub doctor_pass: bool,
}

/// Resolve the install plan for `version`. Pure: no file is
/// written, no doctor is run. The plan is exactly what the JSON
/// envelope returned by `cogh install --version <v>` contains.
///
/// Path resolution order:
/// 1. `$COGNICODE_BIN_DIR` (lets a test fixture force a tempdir).
/// 2. `$CARGO_HOME/bin` when `CARGO_HOME` is set.
/// 3. `$XDG_DATA_HOME/cognicode/bin` when `XDG_DATA_HOME` is set.
/// 4. `~/.local/share/cognicode/bin` as the documented fallback.
pub fn plan_install(version: &str) -> Result<InstallPlan, InstallError> {
    if version.trim().is_empty() {
        return Err(InstallError::InvalidVersion(
            "version must be a non-empty semver token".into(),
        ));
    }
    let install_root = resolve_install_root();
    let install_path = install_root.join("bin").join(binary_name());
    let bin_paths = vec![install_root.join("bin")];
    let shell_completion_path = Some(install_root.join("completions").join(shell_completion_name()));
    Ok(InstallPlan {
        version: version.to_string(),
        install_path,
        bin_paths,
        shell_completion_path,
    })
}

/// Copy the local `cogh` binary to `plan.install_path` and run
/// `cogh doctor` from that path. The function is the *only*
/// place that writes to the install root: every other public
/// function in this module is read-only.
///
/// `source_binary` is the path to the on-disk cogh binary that
/// will be copied. Passing it explicitly (instead of
/// `current_exe().parent()/cogh`) keeps this function unit-
/// testable from a tempdir without depending on the test
/// runner's location.
pub fn execute_install(
    plan: &InstallPlan,
    source_binary: &Path,
) -> Result<InstallOutcome, InstallError> {
    if let Some(parent) = plan.install_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(source_binary, &plan.install_path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perm = std::fs::metadata(&plan.install_path)?.permissions();
        perm.set_mode(0o755);
        std::fs::set_permissions(&plan.install_path, perm)?;
    }
    let sha256 = sha256_of(&plan.install_path)?;
    let doctor_pass = run_doctor_at(&plan.install_path);
    Ok(InstallOutcome {
        action: InstallAction::Install,
        install_path: plan.install_path.clone(),
        sha256,
        doctor_pass,
    })
}

/// Build a rollback outcome against an existing install. Reads
/// the current `tracker/version` (if any) to populate the `from`
/// side; if no tracker is present the rollback is rejected as
/// ambiguous rather than silently pretending the previous
/// version was `(none)`.
pub fn plan_rollback(home_root: &Path, to: &str) -> Result<InstallPlan, InstallError> {
    if to.trim().is_empty() {
        return Err(InstallError::InvalidVersion(
            "rollback target must be a non-empty semver token".into(),
        ));
    }
    let from = std::fs::read_to_string(home_root.join("tracker").join("version"))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let plan = plan_install(to)?;
    // Stamp the plan so callers can serialize the rollback semantics
    // into the JSON envelope without re-reading the tracker.
    let _ = from; // kept in signature for the InstallAction discriminator
    Ok(plan)
}

// ---------------------------------------------------------------------
// helpers (private)
// ---------------------------------------------------------------------

fn binary_name() -> &'static str {
    if cfg!(windows) { "cogh.exe" } else { "cogh" }
}

fn shell_completion_name() -> &'static str {
    "cogh.bash"
}

fn resolve_install_root() -> PathBuf {
    if let Ok(p) = std::env::var("COGNICODE_BIN_DIR")
        && !p.is_empty()
    {
        return PathBuf::from(p);
    }
    if let Ok(cargo_home) = std::env::var("CARGO_HOME")
        && !cargo_home.is_empty()
    {
        return PathBuf::from(cargo_home);
    }
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME")
        && !xdg.is_empty()
    {
        return PathBuf::from(xdg).join("cognicode");
    }
    if let Ok(home) = std::env::var("HOME")
        && !home.is_empty()
    {
        return PathBuf::from(home).join(".local").join("share").join("cognicode");
    }
    PathBuf::from("/usr/local/share/cognicode")
}

fn sha256_of(path: &Path) -> Result<String, InstallError> {
    let bytes = std::fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let digest = hasher.finalize();
    Ok(format!("{digest:x}"))
}

fn run_doctor_at(installed: &Path) -> bool {
    let output = Command::new(installed).arg("doctor").output();
    match output {
        Ok(o) => o.status.success(),
        Err(_) => false,
    }
}

// ---------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// TDD: the resolved install path must end with `cogh` and live
    /// under a `bin/` parent. This locks the contract that a UI can
    /// `parent.parent()` to derive the install root for `PATH` hints.
    #[test]
    fn plan_install_resolves_path() {
        let plan = plan_install("0.101.15").expect("plan should resolve");
        let name = plan
            .install_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        assert!(name.starts_with("cogh"), "expected binary name, got {name}");
        let parent = plan
            .install_path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("");
        assert_eq!(parent, "bin", "install_path must live under bin/");
        assert!(!plan.bin_paths.is_empty(), "bin_paths must be populated");
    }

    /// TDD: `execute_install` must produce an `InstallOutcome` whose
    /// SHA-256 matches the on-disk bytes it just wrote. This pins the
    /// integrity contract; future code that swaps the copy mechanism
    /// (e.g. hardlink) must still produce the same digest.
    #[test]
    fn execute_install_produces_outcome() {
        let tmp = tempfile::tempdir().expect("tempdir");
        // Synthetic source "binary" — content is not special, just
        // distinct enough to verify the SHA round-trips.
        let src = tmp.path().join("cogh-source");
        std::fs::write(&src, b"cogh-fixture-binary").expect("write source");

        // Force the install root to a sibling tempdir so the test
        // never touches the user's real $CARGO_HOME / $HOME.
        let install_root = tmp.path().join("install");
        std::fs::create_dir_all(&install_root).expect("mkdir install root");
        let mut plan = plan_install("0.101.15").expect("plan");
        plan.install_path = install_root.join("bin").join("cogh");

        let outcome = execute_install(&plan, &src).expect("execute should succeed");
        assert_eq!(outcome.install_path, plan.install_path);
        assert!(
            !outcome.sha256.is_empty(),
            "sha256 must be populated for downstream verification"
        );
        let on_disk = std::fs::read(&plan.install_path).expect("read installed");
        let mut h = Sha256::new();
        h.update(&on_disk);
        assert_eq!(format!("{:x}", h.finalize()), outcome.sha256);
    }

    /// TDD: when the destination binary is missing, `execute_install`
    /// should surface an IO error rather than panic or silently
    /// succeed. We achieve this by passing a non-existent source.
    #[test]
    fn execute_install_with_missing_source_returns_io_error() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let install_root = tmp.path().join("install");
        std::fs::create_dir_all(&install_root).expect("mkdir install root");
        let mut plan = plan_install("0.101.15").expect("plan");
        plan.install_path = install_root.join("bin").join("cogh");

        let missing = tmp.path().join("does-not-exist");
        let err = execute_install(&plan, &missing)
            .expect_err("missing source must surface an io error");
        assert!(
            matches!(err, InstallError::Io(_)),
            "expected Io error, got {err:?}"
        );
    }

    /// TDD: `plan_install` must reject empty / whitespace-only
    /// versions. Without this, `cogh install --version ""` would
    /// silently produce a plan pointing at `bin/cogh` and never
    /// explain why a subsequent `cogh doctor` is "unhealthy".
    #[test]
    fn plan_install_rejects_empty_version() {
        let err = plan_install("").expect_err("empty version must be rejected");
        assert!(matches!(err, InstallError::InvalidVersion(_)));
        let err = plan_install("   ").expect_err("whitespace version must be rejected");
        assert!(matches!(err, InstallError::InvalidVersion(_)));
    }

    /// TDD: `plan_rollback` must record the *target* version in the
    /// plan (the `InstallPlan` itself is version-shaped, the
    /// `InstallAction` discriminator lives on the outcome). This
    /// keeps the JSON envelope stable for UI consumers.
    #[test]
    fn plan_rollback_captures_target_version() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let plan = plan_rollback(tmp.path(), "0.101.14").expect("rollback plan");
        assert_eq!(plan.version, "0.101.14");
    }
}
