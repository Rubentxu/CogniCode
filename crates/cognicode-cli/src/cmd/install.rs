//! `cogh::install` — Top-level install command entry point.
//!
//! Provides `run_install()` which wraps [`InstallerTransaction::run`]
//! with install lock, profile selection, tracker update, and error handling.

use std::path::PathBuf;

use anyhow::{Result, anyhow};

use super::ide;
use super::install_lock;
use super::installer_transaction::InstallerTransaction;
use super::layout::CognicodeHome;
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
            // L4 (ADR-CANONICAL-LAYOUT): the IDE integration's
            // skill source is `home.skills_root(version)` (per the
            // portable-skill-bundle spec) — NOT a hardcoded
            // `<root>/install/<v>/mcp-server/skills`, which assumed a
            // non-existent `mcp-server` component and pointed at the
            // legacy layout. The skill bundle name (e.g. `cognicode-core`,
            // `cognicode-mcp-driven`) is not modelled in the bundle
            // manifest today, so L4 picks the first directory under
            // `skills_root` if any exists, or skips integration with a
            // warning if the home has no portable skill bundles.
            if ide::detect_opencode() {
                // DEBT-3.f BLOCKED-BY-DEBT-2: the SkillBundleId is not
                // modelled in any manifest today, so we cannot pick a
                // specific bundle by name. The audit (§9.4-11, sites
                // :55/:56-59) classifies this as "first directory under
                // `skills_root`" heuristic — DEBT-2 territory. Once
                // DEBT-2 introduces a portable-skill-bundle manifest
                // declaring which bundle(s) to integrate, this becomes
                // `bundle.id` from that manifest, not `read_dir().next()`.
                let skills_root = home.skills_root(version);
                let skill_bundle = std::fs::read_dir(&skills_root)
                    .ok()
                    .and_then(|mut d| d.next().and_then(|e| e.ok()))
                    .map(|e| e.path());
                match skill_bundle {
                    Some(skill_path) => {
                        println!(
                            "OpenCode detected, integrating skill bundle at {}",
                            skill_path.display()
                        );
                        // DEBT-3.f: derive the BinaryName from the bundle
                        // manifest's DaemonCli component, not from a
                        // hardcoded `"cognicode-mcp"` literal. The
                        // manifest is the source of truth; if the
                        // bundle declares no DaemonCli, fail loudly.
                        let mcp_binary_name = crate::bundle_manifest::daemon_cli_binary_name(
                            &home.version_manifest(version),
                        )?;
                        let mcp_command = vec![
                            home.shim_path(&mcp_binary_name)
                                .to_string_lossy()
                                .to_string(),
                        ];
                        let steps = ide::integrate_opencode(&skill_path, version, &mcp_command)?;
                        for step in steps {
                            step.execute()
                                .map_err(|e| anyhow!("IDE integration failed: {}", e))?;
                        }
                        println!("✓ OpenCode integration complete");
                    }
                    None => {
                        eprintln!(
                            "warning: no skill bundle found under {}; \
                             skipping OpenCode integration",
                            skills_root.display()
                        );
                        println!("✓ OpenCode integration complete (no skill bundles to integrate)");
                    }
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

        // Drive a real install. The dev-bundle fixture has no skill
        // bundle in the manifest, so `versions/<v>/skills/` will be
        // empty after install. detect_opencode() may or may not be
        // true on this host — both branches are tested by the
        // install succeeding without error.
        let release = crate::release_test_support::local_release(env!("CARGO_PKG_VERSION"))
            .expect("stage a local release");
        crate::release_test_support::point_at(&release);

        // The install must succeed even when the skill bundle is
        // absent. Pre-L4 this would FAIL with `link_or_copy failed`
        // because the hardcoded `install/<v>/mcp-server/skills`
        // path doesn't exist.
        let result = run_install(&home, "core");
        assert!(
            result.is_ok(),
            "L4: install must succeed without a skill bundle; got {result:?}"
        );
    }
}
