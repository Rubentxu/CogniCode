//! `cogh::install` — Top-level install command entry point.
//!
//! Provides `run_install()` which wraps [`InstallerTransaction::run`]
//! with install lock, profile selection, tracker update, and error handling.

use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};

use super::bundle_manifest::declared_skill_bundle_dirs;
use super::ide;
use super::install_lock;
use super::installer_transaction::InstallerTransaction;
use super::layout::CognicodeHome;
use super::release_contract::ArtifactKind;
use super::tracker;

/// Run the atomic install transaction with lock and tracker.
//
// Loads the bundle manifest, validates the version, and executes the
/// install pipeline (download → verify → extract → shim → manifest).
/// Writes the installed version to the tracker on success.
///
/// Returns the path to the written install manifest on success.
pub fn run_install(home: &CognicodeHome, profile: &str) -> Result<PathBuf> {
    // 1. Acquire install lock
    let lock = install_lock::acquire_lock()
        .map_err(|e| anyhow!("failed to acquire install lock: {}", e))?;

    // 2. Run installer transaction
    let result = InstallerTransaction::run(home, profile);

    match result {
        Ok(manifest_path) => {
            // 3. Extract version from manifest path and write tracker
            let version = manifest_path
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|v| v.to_str())
                .unwrap_or("unknown");
            tracker::write_version(version)
                .map_err(|e| anyhow!("failed to write tracker: {}", e))?;

            // 4. Integrate with IDE adapters if OpenCode is detected.
            //
            // L4 (ADR-CANONICAL-LAYOUT): the skill source lives under
            // `home.skills_root(version)` (per the portable-skill-bundle
            // spec).
            //
            // DEBT-2: the SkillBundleId now comes from the bundle
            // manifest's `skill_bundles[]` declaration — never from a
            // `read_dir().next()` scan (that heuristic is retired; see
            // ADR-IDENTITY-MAP-distribution §9.4-11, site :55-59).
            // A declared bundle missing on disk is a hard error: no
            // silent fallback to another directory.
            if ide::detect_opencode() {
                let declared = declared_skill_bundle_dirs(
                    &home.skills_root(version),
                    &home.version_manifest(version),
                    profile,
                )?;
                if declared.is_empty() {
                    eprintln!(
                        "warning: bundle manifest declares no skill bundles for profile \
                         `{profile}`; skipping OpenCode skill integration"
                    );
                    println!("✓ OpenCode integration complete (no skill bundles to integrate)");
                } else {
                    // DEBT-3.f: derive the BinaryName from the bundle
                    // manifest's DaemonCli component, not from a
                    // hardcoded `"cognicode-mcp"` literal. The
                    // manifest is the source of truth; if the
                    // bundle declares no DaemonCli, fail loudly.
                    //
                    // DEBT-2c: a profile may legitimately declare skill
                    // bundles without a DaemonCli component (e.g. `core`
                    // ships the `cognicode` skills but no MCP daemon). In
                    // that case the shim resolution is simply skipped —
                    // the skills are plain files, they do not invoke the
                    // daemon.
                    let manifest = home.version_manifest(version);
                    // DEBT-2c: a profile may declare skill bundles without a
                    // DaemonCli component (e.g. `core` ships the `cognicode`
                    // skills but no MCP daemon). The skills are plain files
                    // that do not invoke the daemon, so shim resolution is
                    // simply skipped. `daemon_cli_binary_name` keeps its
                    // strict fail-loud contract; the *caller* decides whether
                    // a missing DaemonCli is legitimate for this profile.
                    let has_daemon_cli =
                        !crate::bundle_manifest::BundleManifest::from_path(&manifest)?
                            .components_by_kind(ArtifactKind::DaemonCli)
                            .is_empty();
                    let mcp_command = if has_daemon_cli {
                        let mcp_binary_name =
                            crate::bundle_manifest::daemon_cli_binary_name(&manifest)?;
                        vec![
                            home.shim_path(&mcp_binary_name)
                                .to_string_lossy()
                                .to_string(),
                        ]
                    } else {
                        Vec::new()
                    };
                    for skill_path in declared {
                        println!(
                            "OpenCode detected, integrating skill bundle at {}",
                            skill_path.display()
                        );
                        let steps = ide::integrate_opencode(&skill_path, version, &mcp_command)?;
                        for step in steps {
                            step.execute()
                                .map_err(|e| anyhow!("IDE integration failed: {}", e))?;
                        }
                    }
                    println!("✓ OpenCode integration complete");
                }
            }

            // 5. Release lock
            install_lock::release_lock(lock);
            println!(
                "Installed version {} to {}",
                version,
                manifest_path.display()
            );
            Ok(manifest_path)
        }
        Err(e) => {
            // Rollback happened inside InstallerTransaction
            install_lock::release_lock(lock);
            Err(anyhow!("install failed: {}", e))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::test_support::TempCognicodeHome;

    /// L4 T1: `run_install` must not crash when the canonical
    /// `versions/<v>/skills/` directory is empty or missing. Pre-L4
    /// the function hardcoded `<root>/install/<v>/mcp-server/skills`
    /// and would surface a `link_or_copy failed` error (visible in
    /// UAT Phase D reinstall). L4 derives the skill source from
    /// `home.skills_root(version)` and skips integration with a
    /// warning if no skill bundle is present.
    #[test]
    #[serial_test::serial]
    fn t_l4_install_emits_warning_when_no_skill_bundle_present() {
        let _temphome = TempCognicodeHome::new();
        let home = CognicodeHome::resolve(None).expect("resolve home");
        home.init().expect("init home");

        // Drive a real install. Since DEBT-2c the release contract
        // publishes the `cognicode` skill bundle for the `core`
        // profile, so the manifest declares it and the installer
        // extracts it into `versions/<v>/skills/cognicode/`. The
        // install must succeed end-to-end with the declared bundle
        // present (the original L4 regression — a hardcoded
        // `install/<v>/mcp-server/skills` path — stays covered:
        // the path is now derived from the manifest declaration).
        let release = crate::release_test_support::local_release(env!("CARGO_PKG_VERSION"))
            .expect("stage a local release");
        crate::release_test_support::point_at(&release);

        let result = run_install(&home, "core");
        assert!(result.is_ok(), "L4: install must succeed; got {result:?}");
        let skills_dir = home
            .skills_root(env!("CARGO_PKG_VERSION"))
            .join("cognicode");
        assert!(
            skills_dir.is_dir(),
            "DEBT-2c: the declared `cognicode` skill bundle must be extracted at {}",
            skills_dir.display()
        );
    }

    /// DEBT-2 strict T: `declared_skill_bundle_dirs` must resolve the
    /// SkillBundleId from the manifest's `skill_bundles[]` declaration
    /// and reject a declared-but-missing bundle. Identities planted
    /// pairwise-distinct:
    ///     ComponentId   = "cognicode-mcp"
    ///     SkillBundleId = "skills-for-claude"  (≠ ComponentId)
    #[test]
    fn t_debt2_declared_skill_bundle_dirs_use_manifest_ids() {
        use crate::bundle_manifest::BundleManifest;
        let _temphome = TempCognicodeHome::new();
        let home = CognicodeHome::resolve(None).expect("resolve home");
        home.init().expect("init home");

        let version = env!("CARGO_PKG_VERSION");
        let manifest_yaml = format!(
            r#"
apiVersion: cognicode.bundle/v2
kind: Bundle
version: "{version}"
platform: linux-x86-64
released_at: "2026-01-01T00:00:00Z"
profiles:
  - name: core
    description: Daily CLI
skill_bundles:
  - id: skills-for-claude
    version: "{version}"
    profiles: [core]
components:
  - name: cognicode-mcp
    kind: daemon-cli
    version: "{version}"
    artifact: cognicode-mcp-{version}-x86_64-unknown-linux-gnu.tar.gz
    sha256: "9f2c1d4b7e0a3f5c8d1b2e4a6f8c0d2e4b6a8c0e2f4a6b8c0d2e4f6a8b0c2d4e"
    url: "https://github.com/Rubentxu/CogniCode/releases/download/v{version}/cognicode-mcp-{version}-x86_64-unknown-linux-gnu.tar.gz"
    profiles: [core]
"#
        );
        std::fs::create_dir_all(home.version_root(version)).expect("create version dir");
        std::fs::write(home.version_manifest(version), &manifest_yaml).expect("write manifest");
        let manifest = BundleManifest::from_str(&manifest_yaml).expect("manifest valid");
        assert_ne!(
            manifest.skill_bundles[0].id, "cognicode-mcp",
            "gate validity: SkillBundleId must be pairwise-distinct from ComponentId"
        );

        // Declared but NOT on disk: hard error, no first-dir fallback.
        let err = declared_skill_bundle_dirs(
            &home.skills_root(version),
            &home.version_manifest(version),
            "core",
        )
        .expect_err("declared-but-missing bundle must fail loudly");
        let msg = format!("{err:#}");
        assert!(
            msg.contains("skills-for-claude") && msg.contains("refusing to guess"),
            "error must name the declared SkillBundleId and refuse fallback; got: {msg}"
        );

        // Present on disk: resolved by manifest id, not by directory scan.
        let bundle_dir = home.skill_bundle(version, "skills-for-claude");
        std::fs::create_dir_all(&bundle_dir).expect("create skill bundle dir");
        let dirs = declared_skill_bundle_dirs(
            &home.skills_root(version),
            &home.version_manifest(version),
            "core",
        )
        .expect("declared-and-present must resolve");
        assert_eq!(dirs, vec![bundle_dir]);

        // A different profile that declares nothing: empty, no fallback
        // to whichever directory exists.
        assert!(
            declared_skill_bundle_dirs(
                &home.skills_root(version),
                &home.version_manifest(version),
                "reviewer"
            )
            .expect("no declarations for profile")
            .is_empty(),
            "profiles with no declarations must get no skill bundles"
        );
    }
}
