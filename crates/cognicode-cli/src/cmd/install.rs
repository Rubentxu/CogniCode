//! `cogh::install` — Top-level install command entry point.
//!
//! Provides `run_install()` which wraps [`InstallerTransaction::run`]
//! with install lock, profile selection, tracker update, and error handling.

use std::path::PathBuf;

use anyhow::{anyhow, Result};

use super::ide;
use super::install_lock;
use super::installer_transaction::InstallerTransaction;
use super::tracker;

/// Run the atomic install transaction with lock and tracker.
//
// Loads the bundle manifest, validates the version, and executes the
/// install pipeline (download → verify → extract → shim → manifest).
/// Writes the installed version to the tracker on success.
///
/// Returns the path to the written install manifest on success.
pub fn run_install(profile: &str) -> Result<PathBuf> {
    // 1. Acquire install lock
    let lock = install_lock::acquire_lock()
        .map_err(|e| anyhow!("failed to acquire install lock: {}", e))?;

    // 2. Run installer transaction
    let result = InstallerTransaction::run(profile);

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

            // 4. Integrate with IDE adapters if OpenCode is detected
            if ide::detect_opencode() {
                println!("OpenCode detected, integrating...");
                // skill_path is the mcp-server plugin's skills directory
                let skill_path = manifest_path
                    .parent()
                    .map(|p| p.join("mcp-server/skills"))
                    .unwrap_or_else(|| PathBuf::from("~/.cognicode/skills"));
                // Construct mcp_command using shim path (same pattern as cmd_ide_install)
                let mcp_command = vec![
                    dirs::home_dir()
                        .unwrap_or_else(|| PathBuf::from("~"))
                        .join(".cognicode/shims/cognicode-mcp")
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
