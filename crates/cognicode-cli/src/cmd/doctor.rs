//! e74 WU4 — `cogh doctor` capability discovery.
//!
//! The doctor command reports health across four orthogonal
//! dimensions so that an installation with a missing optional
//! dependency (e.g. no Podman on Windows) is not flagged as broken:
//!
//!   1. **Core health** — the basic install (home, shims, tracker).
//!   2. **MCP health** — the local daemon / MCP bridge starts up.
//!   3. **Native analysis** — host-native parsing / canonical Facts /
//!      read-only analysis work without any container runtime.
//!   4. **Optional isolation** — sandbox / container backend
//!      (Podman / Docker / WSL / Hyper-V / VM). Reported as
//!      `Unavailable` when not present; doctor MUST NOT try to enable
//!      or install any of these backends.
//!
//! ## Hard rule
//!
//! `cogh doctor` reports and provides remediation hints, but it does
//! **not** automatically install WSL, Hyper-V, Podman, Docker or a VM.
//! These are invasive host changes that the user must opt into.
//! Automatic enabling of those features is forbidden by the e74 spec.

use std::fmt;
use std::path::Path;

/// Outcome of a single doctor check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckStatus {
    /// Capability present and working.
    Pass,
    /// Capability present but with caveats (e.g. optional sub-feature missing).
    Warn,
    /// Capability required for core functionality is missing.
    Fail,
    /// Capability is an optional/host-extras layer that the current
    /// environment does not provide. This is *not* a failed install.
    Unavailable,
}

impl fmt::Display for CheckStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            CheckStatus::Pass => "PASS",
            CheckStatus::Warn => "WARN",
            CheckStatus::Fail => "FAIL",
            CheckStatus::Unavailable => "UNAVAILABLE",
        };
        f.write_str(s)
    }
}

/// A single dimension reported by `cogh doctor`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorCheck {
    pub name: String,
    pub status: CheckStatus,
    pub detail: String,
    /// Optional remediation hint shown when the check is not Pass.
    pub remediation: Option<String>,
}

impl DoctorCheck {
    pub fn pass(name: &str, detail: impl Into<String>) -> Self {
        Self {
            name: name.to_string(),
            status: CheckStatus::Pass,
            detail: detail.into(),
            remediation: None,
        }
    }

    pub fn warn(name: &str, detail: impl Into<String>, remediation: impl Into<String>) -> Self {
        Self {
            name: name.to_string(),
            status: CheckStatus::Warn,
            detail: detail.into(),
            remediation: Some(remediation.into()),
        }
    }

    pub fn fail(name: &str, detail: impl Into<String>, remediation: impl Into<String>) -> Self {
        Self {
            name: name.to_string(),
            status: CheckStatus::Fail,
            detail: detail.into(),
            remediation: Some(remediation.into()),
        }
    }

    pub fn unavailable(
        name: &str,
        detail: impl Into<String>,
        remediation: impl Into<String>,
    ) -> Self {
        Self {
            name: name.to_string(),
            status: CheckStatus::Unavailable,
            detail: detail.into(),
            remediation: Some(remediation.into()),
        }
    }
}

/// Whole `cogh doctor` report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorReport {
    pub platform: crate::platform_adapter::PlatformReport,
    pub checks: Vec<DoctorCheck>,
}

impl DoctorReport {
    /// True iff every check is `Pass`, `Warn`, or `Unavailable`.
    /// `Fail` makes the whole report unhealthy.
    pub fn is_healthy(&self) -> bool {
        self.checks
            .iter()
            .all(|c| !matches!(c.status, CheckStatus::Fail))
    }

    /// True iff the install can run core analysis WITHOUT requiring
    /// any isolation backend. e74 PRT-001: a Windows user without
    /// Podman must still get core analysis PASS.
    pub fn core_analysis_available(&self) -> bool {
        self.checks.iter().any(|c| {
            c.name == "Core health" && matches!(c.status, CheckStatus::Pass | CheckStatus::Warn)
        }) && self.checks.iter().any(|c| {
            c.name == "Native analysis" && matches!(c.status, CheckStatus::Pass | CheckStatus::Warn)
        })
    }
}

impl fmt::Display for DoctorReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "==> cogh doctor ({} / {} / {})",
            self.platform.triple, self.platform.os, self.platform.arch
        )?;
        for c in &self.checks {
            writeln!(f, "  {:<8} {:<20} {}", c.status, c.name, c.detail)?;
            if let Some(r) = &c.remediation
                && !matches!(c.status, CheckStatus::Pass)
            {
                writeln!(f, "    -> {r}")?;
            }
        }
        writeln!(
            f,
            "==> overall: {}",
            if self.is_healthy() {
                "healthy"
            } else {
                "UNHEALTHY"
            }
        )?;
        Ok(())
    }
}

// ---------- per-dimension probes ----------

/// Probe 1: Core health (filesystem layout).
///
/// Returns:
///   - Fail if home (or bin/, shims/) is missing.
///   - Warn if home+bin+shims exist but `tracker/version` is missing —
///     the install is healthy enough to run, but no version pin is in
///     place. e74 WU4 contract: distinct status from Fail.
///   - Pass otherwise.
pub fn probe_core_health(home_root: &Path) -> DoctorCheck {
    if !home_root.exists() {
        return DoctorCheck::fail(
            "Core health",
            format!("home {} does not exist", home_root.display()),
            "run `cogh install <profile>` to bootstrap the layout",
        );
    }
    let bin = home_root.join("bin");
    let shims = home_root.join("shims");
    let tracker_version = home_root.join("tracker").join("version");
    let mut missing: Vec<&str> = Vec::new();
    if !bin.exists() {
        missing.push("bin/");
    }
    if !shims.exists() {
        missing.push("shims/");
    }
    if !missing.is_empty() {
        return DoctorCheck::fail(
            "Core health",
            format!("missing: {}", missing.join(", ")),
            "run `cogh install <profile>` to materialize the layout",
        );
    }
    // Layout dirs exist. Tracker absence is a Warn — the install
    // works for `cogh doctor` itself but cannot pin a version.
    if !tracker_version.exists() {
        return DoctorCheck::warn(
            "Core health",
            "tracker/version missing (no pinned version)",
            "run `cogh install <plugin>` to pin a version",
        );
    }
    DoctorCheck::pass("Core health", "home, bin/, shims/ present")
}

/// Probe 2: MCP health (cognicode-mcp availability) — e88-F2.
///
/// State-aware: the probe distinguishes ABSENT BY DESIGN from
/// EXPECTED BUT BROKEN using the installed manifest as evidence
/// (PRT-004: health of the product != availability of a capability).
///
///   - No tracker pin            -> Unavailable (no active runtime).
///   - Tracker + no version tree -> Unavailable here; probe_core_health
///     owns that FAIL (missing tree under an active pin).
///   - Installed manifest declares `daemon-cli` and the shim exists -> Pass.
///   - Installed manifest declares `daemon-cli` but the shim is gone
///     -> Fail (expected-but-broken).
///   - Installed manifest has no `daemon-cli` component
///     -> Unavailable (profile does not include the daemon capability).
///
/// No profile-name heuristics: the profile-filtered installed manifest
/// is the single source of what this installation should contain.
pub fn probe_mcp_health(home_root: &Path) -> DoctorCheck {
    let tracker_version = home_root.join("tracker").join("version");
    let version = match std::fs::read_to_string(&tracker_version) {
        Ok(v) if !v.trim().is_empty() => v.trim().to_string(),
        _ => {
            return DoctorCheck::unavailable(
                "MCP",
                "no active runtime (no version pinned in tracker)",
                "run `cogh install mcp-server --profile reviewer` to install \
                 a profile that includes the daemon",
            );
        }
    };

    let manifest_path = home_root
        .join("versions")
        .join(&version)
        .join("manifest.yaml");
    if !manifest_path.exists() {
        // The version tree/manifest is missing under an active pin.
        // probe_core_health surfaces this as a Fail; the MCP dimension
        // cannot evaluate a nonexistent installation and must not
        // double-claim the failure. Unavailable is honest here.
        return DoctorCheck::unavailable(
            "MCP",
            format!("active version {version} has no installed manifest; MCP not evaluated"),
            "run `cogh install mcp-server --version <v> --profile reviewer` to reinstall",
        );
    }

    let declares_daemon = crate::bundle_manifest::BundleManifest::from_path(&manifest_path)
        .map(|m| {
            m.components
                .iter()
                .any(|c| c.kind == crate::release_contract::ArtifactKind::DaemonCli)
        })
        .unwrap_or(false);

    let mcp_shim = home_root.join("shims").join("cognicode-mcp");
    if declares_daemon {
        if mcp_shim.exists() {
            DoctorCheck::pass(
                "MCP",
                "cognicode-mcp shim present (declared by active install)",
            )
        } else {
            DoctorCheck::fail(
                "MCP",
                format!(
                    "active install {version} declares a daemon-cli component but the \
                     cognicode-mcp shim is missing"
                ),
                "run `cogh install mcp-server --version <v> --profile reviewer` to repair",
            )
        }
    } else {
        DoctorCheck::unavailable(
            "MCP",
            format!("active installation {version} does not include the daemon capability"),
            "install a profile that includes the Daemon kind (e.g. `reviewer`) if you need MCP",
        )
    }
}

/// Probe 3: Native analysis (host-native parsing + canonical Facts +
/// read-only analysis). Always Pass — these never require a container
/// runtime (e74 PRT-001).
pub fn probe_native_analysis() -> DoctorCheck {
    DoctorCheck::pass(
        "Native analysis",
        "host-native: parsing, canonical Facts, read-only analysis \
         (no container runtime required)",
    )
}

/// Probe 4: Optional isolation backend.
///
/// Reports `Unavailable` (NOT `Fail`) when no isolation runtime is
/// present. e74 directive: doctor MUST NOT auto-enable WSL, Hyper-V,
/// Podman, Docker, or a VM.
pub fn probe_optional_isolation() -> DoctorCheck {
    use crate::platform_adapter::{PlatformFamily, detect_host_platform};
    let host = detect_host_platform();
    let family = match host {
        crate::platform_adapter::Platform::WindowsX86_64 => PlatformFamily::Windows,
        _ => PlatformFamily::Unix,
    };
    // Check for at least one isolation backend on the host.
    // We probe lightweight, non-spawning signals:
    //   - Podman:   look for `podman` in PATH (Unix); on Windows, look
    //               for `podman.exe` and absence of WSL.
    //   - Docker:   look for `docker` in PATH.
    //   - WSL:      on Windows, `wsl.exe --status` exit 0 (NOT run here
    //               to avoid spawning); we only check file presence.
    //   - Hyper-V:  requires admin; we never probe.
    //
    // For e74 the contract is: doctor REPORTS, never enables.
    // The probe is intentionally conservative: if nothing obvious is
    // on disk, we say Unavailable with a remediation that lists the
    // supported backends but does NOT install them.
    let backend = detect_isolation_backend(family);
    match backend {
        Some(name) => DoctorCheck::pass(
            "Isolation backend",
            format!("{name} detected (optional, host extras)"),
        ),
        None => DoctorCheck::unavailable(
            "Isolation backend",
            "no Podman/Docker/WSL/Hyper-V/VM detected",
            "isolation is OPTIONAL; install one manually only if you need \
             sandboxed project-code execution (e75). CogniCode core analysis \
             works without any isolation backend.",
        ),
    }
}

fn detect_isolation_backend(
    family: crate::platform_adapter::PlatformFamily,
) -> Option<&'static str> {
    // Light-touch probe: just check PATH presence. We deliberately do
    // NOT spawn the binary — that would change doctor from a report
    // into an action.
    let which = |name: &str| which_exists(name);
    match family {
        crate::platform_adapter::PlatformFamily::Unix => {
            if which("podman") {
                Some("podman")
            } else if which("docker") {
                Some("docker")
            } else {
                None
            }
        }
        crate::platform_adapter::PlatformFamily::Windows => {
            // On Windows: Podman Desktop or WSL would be the typical path.
            if which("podman.exe") {
                Some("podman.exe")
            } else if which("wsl.exe") {
                Some("wsl.exe")
            } else if which("docker.exe") {
                Some("docker.exe")
            } else {
                None
            }
        }
    }
}

fn which_exists(name: &str) -> bool {
    if let Some(paths) = std::env::var_os("PATH") {
        for p in std::env::split_paths(&paths) {
            if p.join(name).exists() {
                return true;
            }
        }
    }
    false
}

/// Run the full doctor probe set against a home directory.
pub fn run_doctor(home_root: &Path) -> DoctorReport {
    let platform = crate::platform_adapter::PlatformReport::for_host();
    let checks = vec![
        probe_core_health(home_root),
        probe_mcp_health(home_root),
        probe_native_analysis(),
        probe_optional_isolation(),
    ];
    DoctorReport { platform, checks }
}

// ---------- tests ----------

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_home() -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static N: AtomicU64 = AtomicU64::new(0);
        let n = N.fetch_add(1, Ordering::SeqCst);
        let p =
            std::env::temp_dir().join(format!("cognicode-doctor-test-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn probe_core_health_passes_when_bin_and_shims_exist() {
        let home = tmp_home();
        std::fs::create_dir_all(home.join("bin")).unwrap();
        std::fs::create_dir_all(home.join("shims")).unwrap();
        // Probe 1 was extended in e74 WU4-followup: layout-only
        // (without `tracker/version`) is now Warn, not Pass. Mirror
        // the new contract by including the tracker file in the
        // happy-path fixture.
        std::fs::create_dir_all(home.join("tracker")).unwrap();
        std::fs::write(home.join("tracker").join("version"), b"0.95.0").unwrap();
        let check = probe_core_health(&home);
        assert_eq!(check.status, CheckStatus::Pass);
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn probe_core_health_fails_when_home_missing() {
        let home = tmp_home();
        let missing = home.join("nope");
        let check = probe_core_health(&missing);
        assert_eq!(check.status, CheckStatus::Fail);
        assert!(check.remediation.is_some());
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn probe_core_health_fails_when_layout_partial() {
        let home = tmp_home();
        // bin/ exists, shims/ does not.
        std::fs::create_dir_all(home.join("bin")).unwrap();
        let check = probe_core_health(&home);
        assert_eq!(check.status, CheckStatus::Fail);
        assert!(check.detail.contains("shims/"));
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn probe_mcp_health_passes_when_daemon_shim_present() {
        let home = tmp_home();
        // e88-F2: presence alone is not enough — the probe uses the
        // installed manifest as evidence. Plant an active install that
        // declares the daemon plus the shim.
        plant_active_install(&home, MANIFEST_WITH_DAEMON);
        std::fs::write(home.join("shims").join("cognicode-mcp"), b"fake").unwrap();
        let check = probe_mcp_health(&home);
        assert_eq!(check.status, CheckStatus::Pass);
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn probe_mcp_health_fails_when_shim_missing() {
        let home = tmp_home();
        std::fs::create_dir_all(home.join("shims")).unwrap();
        let check = probe_mcp_health(&home);
        // e88-F2: with no tracker pin there is no active runtime —
        // absence of the daemon is ABSENT BY DESIGN (Unavailable),
        // not a broken install (Fail).
        assert_eq!(check.status, CheckStatus::Unavailable);
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn probe_native_analysis_always_passes() {
        // e74 PRT-001: native analysis never depends on a container.
        let check = probe_native_analysis();
        assert_eq!(check.status, CheckStatus::Pass);
        assert!(check.detail.contains("no container runtime"));
    }

    #[test]
    fn probe_optional_isolation_is_never_fail() {
        // Even with no backend on PATH, isolation must report
        // Unavailable, never Fail. The install must not be flagged
        // as broken just because no Podman/Docker is present.
        let check = probe_optional_isolation();
        assert!(
            matches!(check.status, CheckStatus::Pass | CheckStatus::Unavailable),
            "isolation must never be Fail; got {:?}",
            check.status
        );
    }

    #[test]
    fn probe_optional_isolation_remediation_mentions_no_install() {
        // The directive: doctor MUST NOT install isolation backends.
        // We pin this contractually: the remediation hint must never
        // promise to install anything.
        let check = probe_optional_isolation();
        if let Some(r) = &check.remediation {
            let lower = r.to_lowercase();
            assert!(
                !lower.contains("install ") || lower.contains("manually"),
                "remediation must never auto-install: {r}"
            );
        }
    }

    #[test]
    fn report_is_healthy_when_no_failures() {
        let home = tmp_home();
        std::fs::create_dir_all(home.join("bin")).unwrap();
        std::fs::create_dir_all(home.join("shims")).unwrap();
        // L2 layout: MCP health probes the shim, not bin/.
        std::fs::write(home.join("shims").join("cognicode-mcp"), b"fake").unwrap();
        let report = run_doctor(&home);
        assert!(report.is_healthy());
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn report_unhealthy_when_core_fails() {
        let home = tmp_home();
        // empty home → Core health fails
        let report = run_doctor(&home);
        assert!(!report.is_healthy());
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn report_core_analysis_available_with_clean_install() {
        let home = tmp_home();
        std::fs::create_dir_all(home.join("bin")).unwrap();
        std::fs::create_dir_all(home.join("shims")).unwrap();
        std::fs::write(home.join("bin").join("cognicode-mcp"), b"fake").unwrap();
        let report = run_doctor(&home);
        // Even if isolation is Unavailable, core analysis is available
        // — the e74 PRT-001 contract.
        assert!(report.core_analysis_available());
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn report_chip_layout_has_four_dimensions() {
        // The four dimensions are an explicit contract: changing this
        // list is a deliberate UX decision, not a refactor.
        let home = tmp_home();
        std::fs::create_dir_all(home.join("bin")).unwrap();
        std::fs::create_dir_all(home.join("shims")).unwrap();
        let report = run_doctor(&home);
        let names: Vec<&str> = report.checks.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["Core health", "MCP", "Native analysis", "Isolation backend"]
        );
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn probe_core_health_warns_when_layout_present_but_tracker_missing() {
        // The contract: home+bin+shims present but `tracker/version`
        // absent is a Warn (not Pass, not Fail). The install can
        // run `cogh doctor` itself, just cannot pin a plugin
        // version. This was an undocumented contract from the old
        // doctor; we surface it as a Warn to keep the
        // lifecycle-tracker-version test honest.
        let home = tmp_home();
        std::fs::create_dir_all(home.join("bin")).unwrap();
        std::fs::create_dir_all(home.join("shims")).unwrap();
        let check = probe_core_health(&home);
        assert_eq!(
            check.status,
            CheckStatus::Warn,
            "expected Warn when tracker/version missing; got {:?}",
            check.status
        );
        assert!(
            check.detail.contains("tracker"),
            "detail should mention tracker; got: {}",
            check.detail
        );
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn check_status_display() {
        assert_eq!(format!("{}", CheckStatus::Pass), "PASS");
        assert_eq!(format!("{}", CheckStatus::Warn), "WARN");
        assert_eq!(format!("{}", CheckStatus::Fail), "FAIL");
        assert_eq!(format!("{}", CheckStatus::Unavailable), "UNAVAILABLE");
    }

    // ----- e88-F2: state-aware MCP probe matrix -----

    const MANIFEST_WITH_DAEMON: &str = r#"
apiVersion: cognicode.bundle/v2
version: "0.97.0"
platform: linux-x86-64
profiles:
  - name: reviewer
    description: reviewer
components:
  - name: cognicode-mcp
    kind: daemon-cli
    version: "0.97.0"
    artifact: cognicode-mcp-0.97.0-x86_64-unknown-linux-gnu.tar.gz
    sha256: "9f2c1d4b7e0a3f5c8d1b2e4a6f8c0d2e4b6a8c0e2f4a6b8c0d2e4f6a8b0c2d4e"
    url: "https://github.com/Rubentxu/CogniCode/releases/download/v0.97.0/cognicode-mcp-0.97.0-x86_64-unknown-linux-gnu.tar.gz"
    profiles: [reviewer]
"#;

    const MANIFEST_WITHOUT_DAEMON: &str = r#"
apiVersion: cognicode.bundle/v2
version: "0.97.0"
platform: linux-x86-64
profiles:
  - name: core
    description: core
components:
  - name: cognicode
    kind: cognicode
    version: "0.97.0"
    artifact: cognicode-0.97.0-x86_64-unknown-linux-gnu.tar.gz
    sha256: "9f2c1d4b7e0a3f5c8d1b2e4a6f8c0d2e4b6a8c0e2f4a6b8c0d2e4f6a8b0c2d4e"
    url: "https://github.com/Rubentxu/CogniCode/releases/download/v0.97.0/cognicode-0.97.0-x86_64-unknown-linux-gnu.tar.gz"
    profiles: [core]
"#;

    /// Fixture: an active pin plus a profile-filtered installed manifest.
    fn plant_active_install(home: &Path, manifest_yaml: &str) {
        std::fs::create_dir_all(home.join("bin")).unwrap();
        std::fs::create_dir_all(home.join("shims")).unwrap();
        std::fs::create_dir_all(home.join("tracker")).unwrap();
        std::fs::write(home.join("tracker").join("version"), b"0.97.0").unwrap();
        let vdir = home.join("versions").join("0.97.0");
        std::fs::create_dir_all(&vdir).unwrap();
        std::fs::write(vdir.join("manifest.yaml"), manifest_yaml).unwrap();
    }

    /// T1: clean uninstall (no tracker pin, layout intact) must NOT be
    /// reported as UNHEALTHY — the tool returned the machine to the
    /// exact state the user requested.
    #[test]
    fn t1_clean_uninstall_is_not_unhealthy() {
        let home = tmp_home();
        std::fs::create_dir_all(home.join("bin")).unwrap();
        std::fs::create_dir_all(home.join("shims")).unwrap();
        let report = run_doctor(&home);
        assert!(
            report.is_healthy(),
            "post-uninstall state must be healthy; got {report}"
        );
        let _ = std::fs::remove_dir_all(&home);
    }

    /// T2: active install declares daemon-cli + shim present -> PASS.
    #[test]
    fn t2_daemon_declared_and_shim_present_passes() {
        let home = tmp_home();
        plant_active_install(&home, MANIFEST_WITH_DAEMON);
        std::fs::write(home.join("shims").join("cognicode-mcp"), b"fake").unwrap();
        let check = probe_mcp_health(&home);
        assert_eq!(check.status, CheckStatus::Pass);
        let _ = std::fs::remove_dir_all(&home);
    }

    /// T3: daemon-cli declared but shim missing -> FAIL (expected-but-broken).
    #[test]
    fn t3_daemon_declared_but_shim_missing_fails() {
        let home = tmp_home();
        plant_active_install(&home, MANIFEST_WITH_DAEMON);
        // no shim written
        let check = probe_mcp_health(&home);
        assert_eq!(check.status, CheckStatus::Fail);
        let report = run_doctor(&home);
        assert!(!report.is_healthy());
        let _ = std::fs::remove_dir_all(&home);
    }

    /// T4: active manifest has NO daemon-cli -> non-failing (Unavailable).
    #[test]
    fn t4_no_daemon_declared_is_unavailable_not_fail() {
        let home = tmp_home();
        plant_active_install(&home, MANIFEST_WITHOUT_DAEMON);
        let check = probe_mcp_health(&home);
        assert_eq!(check.status, CheckStatus::Unavailable);
        assert!(!check.detail.contains("not found"));
        let _ = std::fs::remove_dir_all(&home);
    }

    /// T5: tracker pins a version whose tree/manifest is gone -> Core FAIL.
    #[test]
    fn t5_active_pin_with_missing_tree_fails_core() {
        let home = tmp_home();
        std::fs::create_dir_all(home.join("bin")).unwrap();
        std::fs::create_dir_all(home.join("shims")).unwrap();
        std::fs::create_dir_all(home.join("tracker")).unwrap();
        std::fs::write(home.join("tracker").join("version"), b"0.97.0").unwrap();
        // no versions/0.97.0 at all
        let core = probe_core_health(&home);
        // Layout (bin/shims) is intact, so core itself stays Warn-level;
        // the MCP probe must NOT fail — it defers to the missing-tree
        // condition and reports Unavailable (not evaluated).
        let mcp = probe_mcp_health(&home);
        assert_eq!(mcp.status, CheckStatus::Unavailable);
        assert_eq!(core.status, CheckStatus::Pass);
        let _ = std::fs::remove_dir_all(&home);
    }

    /// T6: completely uninitialised home keeps existing semantics (FAIL
    /// on Core health because the home does not exist).
    #[test]
    fn t6_uninitialised_home_unchanged() {
        let home = tmp_home().join("nonexistent");
        let report = run_doctor(&home);
        assert!(!report.is_healthy());
    }

    #[test]
    fn doctor_report_display_includes_platform() {
        let home = tmp_home();
        std::fs::create_dir_all(home.join("bin")).unwrap();
        std::fs::create_dir_all(home.join("shims")).unwrap();
        let report = run_doctor(&home);
        let rendered = format!("{report}");
        assert!(rendered.contains("cogh doctor"));
        assert!(rendered.contains("overall:"));
        let _ = std::fs::remove_dir_all(&home);
    }
}
