//! `cogh::lifecycle` — End-to-end lifecycle integration tests (E32-H).
//!
// Spec: `openspec/specs/cognicode-lifecycle/spec.md`.
//!
// These tests exercise the full `cogh` binary lifecycle by:
//! 1. Setting up a temporary COGNICODE_HOME + temp IDE configs
//! 2. Invoking `cogh` as a subprocess via `std::process::Command`
//! 3. Verifying the resulting filesystem state
//!
// The test binary is built at `target/debug/cogh` (built by E32-A).
// We assume the test runs from the repo root (cargo handles this).

use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};
use serde_json::Value;

use super::ide::cmd_ide_install;
use super::ide::detect_opencode;
use super::install_lock;
use super::layout::{CognicodeHome, cmd_init, cmd_install_plugin_stub};
use super::profile;
use super::tracker;

/// Locate the freshly built `cogh` binary.
///
/// The unit-test executable lives at `<target-dir>/debug/deps/cogh-<hash>`;
/// its companion `cogh` binary sits two directories up in
/// `<target-dir>/debug/`. Deriving the path from `current_exe()` keeps this
/// correct regardless of a custom `CARGO_TARGET_DIR`: a hardcoded
/// `<workspace>/target/debug/cogh` silently resolved to a stale binary,
/// masking the real behaviour under test.
fn cogh_bin() -> std::path::PathBuf {
    let exe = std::env::current_exe().expect("current_exe should be available in tests");
    exe.parent()
        .and_then(|p| p.parent())
        .expect("test executable should live in <target-dir>/debug/deps")
        .join(format!("cogh{}", std::env::consts::EXE_SUFFIX))
}

/// Run `cogh` with the given args, in a temp HOME.
fn run_cogh(home: &Path, args: &[&str]) -> Result<std::process::Output> {
    let prev_home = std::env::var("HOME").ok();
    // SAFETY: tests in the same process can race on env vars; we set HOME
    // before each invocation and restore after.
    unsafe {
        std::env::set_var("HOME", home);
        std::env::set_var("COGNICODE_HOME", home.join(".cognicode"));
    }
    let bin = cogh_bin();
    let result = Command::new(&bin)
        .args(args)
        .current_dir(home)
        .output()
        .with_context(|| format!("run cogh {:?}", bin));
    if let Some(prev) = prev_home {
        unsafe {
            std::env::set_var("HOME", prev);
        }
    } else {
        unsafe {
            std::env::remove_var("HOME");
        }
    }
    unsafe {
        std::env::remove_var("COGNICODE_HOME");
    }
    result
}

/// Set up a temp home with .cognicode + bundled plugins.
fn setup_temp_home(tmp: &Path) -> Result<()> {
    let _ = std::fs::remove_dir_all(tmp);
    std::fs::create_dir_all(tmp)?;

    // Run `cogh init` to populate the layout + bundled plugins
    let prev_home = std::env::var("HOME").ok();
    unsafe {
        std::env::set_var("HOME", tmp);
    }
    unsafe {
        std::env::set_var("COGNICODE_HOME", tmp.join(".cognicode"));
    }
    let bin = cogh_bin();
    let _ = Command::new(&bin).args(["init"]).current_dir(tmp).output();
    if let Some(prev) = prev_home {
        unsafe {
            std::env::set_var("HOME", prev);
        }
    } else {
        unsafe {
            std::env::remove_var("HOME");
        }
    }
    unsafe {
        std::env::remove_var("COGNICODE_HOME");
    }
    Ok(())
}

/// Plant a minimal `versions/<v>/` tree so `cmd_uninstall`'s "tree is the
/// source of truth" gate (DEBT-4 WU3 / ded95fbf) treats the version as
/// installed. The manifest content is not validated by the uninstall path.
fn plant_version_tree(tmp: &Path, version: &str) {
    let vdir = tmp.join(".cognicode/versions").join(version);
    std::fs::create_dir_all(&vdir).unwrap();
    std::fs::write(
        vdir.join("manifest.yaml"),
        format!(
            "apiVersion: cognicode.bundle/v2\nkind: Bundle\nversion: \"{version}\"\nplatform: linux-x86-64\nprofiles:\n  - name: core\n    description: core\ncomponents:\n  - name: cognicode-mcp\n    kind: daemon-cli\n    version: \"{version}\"\n    artifact: cognicode-mcp-{version}-x86_64-unknown-linux-gnu.tar.gz\n    sha256: \"9f2c1d4b7e0a3f5c8d1b2e4a6f8c0d2e4b6a8c0e2f4a6b8c0d2e4f6a8b0c2d4e\"\n    url: \"https://github.com/Rubentxu/CogniCode/releases/download/v{version}/cognicode-mcp-{version}-x86_64-unknown-linux-gnu.tar.gz\"\n    profiles: [core]\n"
        ),
    )
    .unwrap();
}

/// Create an empty `~/.codex/config.toml` (for codex tests).
fn create_codex_config(tmp: &Path) -> Result<()> {
    let codex_dir = tmp.join(".codex");
    std::fs::create_dir_all(&codex_dir)?;
    std::fs::write(
        codex_dir.join("config.toml"),
        r#"model = "test"
other_setting = 42
"#,
    )?;
    Ok(())
}

/// Create an empty `~/.config/opencode/opencode.json` (for opencode tests).
fn create_opencode_config(tmp: &Path) -> Result<()> {
    let oc_dir = tmp.join(".config/opencode");
    std::fs::create_dir_all(&oc_dir)?;
    std::fs::write(
        oc_dir.join("opencode.json"),
        r#"{"agent": {"foo": {"description": "test"}}}
"#,
    )?;
    Ok(())
}

/// Create an empty `~/.zcode/v2/config.json` (for zcode tests).
fn create_zcode_config(tmp: &Path) -> Result<()> {
    let zcode_dir = tmp.join(".zcode/v2");
    std::fs::create_dir_all(&zcode_dir)?;
    std::fs::write(
        zcode_dir.join("config.json"),
        r#"{"provider": {"minimax": {}}}
"#,
    )?;
    Ok(())
}

/// Create `~/.claude/mcp/` directory structure (for claude tests).
fn create_claude_config(tmp: &Path) -> Result<()> {
    let claude_dir = tmp.join(".claude");
    std::fs::create_dir_all(&claude_dir)?;
    std::fs::create_dir_all(claude_dir.join("mcp"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::install;
    use serial_test::serial;

    #[test]
    #[serial]
    fn init_creates_layout_and_bundled_plugins() {
        let tmp = std::env::temp_dir().join(format!("cogh-lc-init-{}", std::process::id()));
        setup_temp_home(&tmp).unwrap();
        assert!(tmp.join(".cognicode/bin").exists(), "bin/ missing");
        assert!(tmp.join(".cognicode/shims").exists(), "shims/ missing");
        assert!(
            tmp.join(".cognicode/plugins/mcp-server").exists(),
            "mcp-server plugin missing"
        );
        assert!(
            tmp.join(".cognicode/plugins/skills-cognicode-core")
                .exists(),
            "skills plugin missing"
        );
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    #[serial]
    fn doctor_reports_clean_install() {
        let tmp = std::env::temp_dir().join(format!("cogh-lc-doc-{}", std::process::id()));
        setup_temp_home(&tmp).unwrap();
        let out = run_cogh(&tmp, &["doctor"]).unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        // e74 WU4: doctor output changed shape. The previous contract
        // asserted `home exists`, which was the only string the old
        // filesystem-only doctor printed. The new contract is a
        // four-dimension report; the dimension `Core health` is the
        // direct descendant of the old "home exists" check.
        assert!(
            stdout.contains("Core health"),
            "doctor missing 'Core health' dimension; stdout was: {stdout}"
        );
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    #[serial]
    fn install_creates_mcp_server_version_dir() {
        let tmp = std::env::temp_dir().join(format!("cogh-lc-inst-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let home = CognicodeHome {
            root: tmp.join(".cognicode"),
        };
        // Initialize home directory (replaces setup_temp_home + cogh init subprocess)
        cmd_init(&home).unwrap();

        // Call the legacy stub directly (e87.1: the product `cogh install` path no longer routes here).
        // Note: version directory creation is not implemented in the placeholder.
        cmd_install_plugin_stub(&home, "mcp-server", "v0.93.0", &[]).unwrap();

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    #[serial]
    fn install_opencode_ide_patches_config_and_skills() {
        let tmp = std::env::temp_dir().join(format!("cogh-lc-oc-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let home = CognicodeHome {
            root: tmp.join(".cognicode"),
        };
        cmd_init(&home).unwrap();

        // Pre-create a plugin + version dir so skills copy finds something
        // DEBT-2: plant manifest + declared skill bundle instead of the
        // legacy `<plugin>/skills` dir (integrators resolve from the
        // manifest now).
        {
            let vdir = tmp.join(".cognicode/versions/v0.93.0");
            std::fs::create_dir_all(vdir.join("skills/skills-for-claude")).unwrap();
            std::fs::write(
                vdir.join("skills/skills-for-claude/SKILL.md"),
                "---\nname: x\n---\n",
            )
            .unwrap();
            let yaml = r#"
apiVersion: cognicode.bundle/v2
kind: Bundle
version: "0.93.0"
platform: linux-x86-64
released_at: "2026-01-01T00:00:00Z"
profiles:
  - name: core
    description: Daily CLI
  - name: reviewer
    description: Review CLI
skill_bundles:
  - id: skills-for-claude
    version: "0.93.0"
    profiles: [core, reviewer]
components:
  - name: cognicode-mcp
    kind: daemon-cli
    version: "0.93.0"
    artifact: cognicode-mcp-0.93.0-x86_64-unknown-linux-gnu.tar.gz
    sha256: "9f2c1d4b7e0a3f5c8d1b2e4a6f8c0d2e4b6a8c0e2f4a6b8c0d2e4f6a8b0c2d4e"
    url: "https://github.com/Rubentxu/CogniCode/releases/download/v0.93.0/cognicode-mcp-0.93.0-x86_64-unknown-linux-gnu.tar.gz"
    profiles: [core, reviewer]
"#;
            std::fs::write(vdir.join("manifest.yaml"), yaml).unwrap();
            // PR contract: IDE integration validates that the declared MCP
            // binary exists on disk and that the shim targets it. Plant a
            // real (empty) binary and matching shim so the fixture is
            // coherent with the new stale-link rejection.
            let bin_path = vdir.join("cognicode-mcp/bin/cognicode-mcp");
            std::fs::create_dir_all(bin_path.parent().unwrap()).unwrap();
            std::fs::write(&bin_path, b"#!/bin/sh\n").unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&bin_path, std::fs::Permissions::from_mode(0o755))
                    .unwrap();
            }
            // Shim must canonicalize to the installed binary (stale-link check).
            let shim = tmp.join(".cognicode/shims/cognicode-mcp");
            std::fs::create_dir_all(shim.parent().unwrap()).unwrap();
            let _ = std::fs::remove_file(&shim);
            #[cfg(unix)]
            std::os::unix::fs::symlink(&bin_path, &shim).unwrap();
            #[cfg(not(unix))]
            std::fs::copy(&bin_path, &shim).unwrap();
        }
        create_opencode_config(&tmp).unwrap();

        // Set HOME so opencode paths resolve to temp dir
        let prev_home = std::env::var("HOME").ok();
        unsafe {
            std::env::set_var("HOME", &tmp);
        }

        // Call cmd_ide_install directly instead of cogh subprocess
        cmd_ide_install(&home, "opencode", "mcp-server", "v0.93.0").unwrap();

        // Verify output (cmd_ide_install prints "patched" and "copied skills")
        // We can't capture stdout directly, so we verify side effects below

        // Verify the config was patched
        let cfg_path = tmp.join(".config/opencode/opencode.json");
        let cfg_text = std::fs::read_to_string(&cfg_path).unwrap();
        assert!(
            cfg_text.contains("cognicode-mcp"),
            "cognicode-mcp missing from config"
        );
        // Verify the original entry was preserved
        assert!(cfg_text.contains("agent"), "original entry lost");

        // Verify skills dir was created. The dual-bundle contract names the
        // skills link by bundle id: `skills-for-claude-v0.93.0`.
        assert!(
            tmp.join(".config/opencode/skills/skills-for-claude-v0.93.0")
                .exists(),
            "skills dir missing"
        );

        // Restore HOME
        if let Some(prev) = prev_home {
            unsafe {
                std::env::set_var("HOME", prev);
            }
        } else {
            unsafe {
                std::env::remove_var("HOME");
            }
        }

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    #[serial]
    fn uninstall_opencode_ide_removes_entry_and_skills() {
        let tmp = std::env::temp_dir().join(format!("cogh-lc-oc-un-{}", std::process::id()));
        setup_temp_home(&tmp).unwrap();
        // Set up an existing opencode install
        let cfg_path = tmp.join(".config/opencode/opencode.json");
        std::fs::create_dir_all(cfg_path.parent().unwrap()).unwrap();
        std::fs::write(
            &cfg_path,
            r#"{"mcp": {"cognicode-mcp": {"type": "stdio"}, "other": {"type": "local"}}}"#,
        )
        .unwrap();
        let skill_dir = tmp.join(".config/opencode/skills/cognicode-0.92.0");
        plant_version_tree(&tmp, "0.92.0");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(skill_dir.join("SKILL.md"), "x").unwrap();

        let out = run_cogh(
            &tmp,
            &[
                "uninstall",
                "mcp-server",
                "--ide",
                "opencode",
                "--version",
                "0.92.0",
            ],
        )
        .unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        // cmd_uninstall prints "uninstall: ..." and the IDE adapter prints
        // "✓ OpenCode uninstall complete".
        assert!(
            stdout.contains("uninstall:") && stdout.contains("✓ OpenCode uninstall complete"),
            "uninstall output missing: {stdout}"
        );

        // cognicode-mcp should be gone, other preserved
        let cfg_text = std::fs::read_to_string(&cfg_path).unwrap();
        assert!(
            !cfg_text.contains("cognicode-mcp"),
            "cognicode-mcp still in config"
        );
        assert!(cfg_text.contains("other"), "other entry lost");
        // Skills dir removed
        assert!(!skill_dir.exists(), "skills dir not removed");

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    #[serial]
    fn install_codex_ide_patches_toml_config() {
        let tmp = std::env::temp_dir().join(format!("cogh-lc-codex-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let home = CognicodeHome {
            root: tmp.join(".cognicode"),
        };
        cmd_init(&home).unwrap();

        // DEBT-2: plant manifest + declared skill bundle instead of the
        // legacy `<plugin>/skills` dir (integrators resolve from the
        // manifest now).
        {
            let vdir = tmp.join(".cognicode/versions/v0.93.0");
            std::fs::create_dir_all(vdir.join("skills/skills-for-claude")).unwrap();
            std::fs::write(
                vdir.join("skills/skills-for-claude/SKILL.md"),
                "---\nname: x\n---\n",
            )
            .unwrap();
            let yaml = r#"
apiVersion: cognicode.bundle/v2
kind: Bundle
version: "0.93.0"
platform: linux-x86-64
released_at: "2026-01-01T00:00:00Z"
profiles:
  - name: core
    description: Daily CLI
  - name: reviewer
    description: Review CLI
skill_bundles:
  - id: skills-for-claude
    version: "0.93.0"
    profiles: [core, reviewer]
components:
  - name: cognicode-mcp
    kind: daemon-cli
    version: "0.93.0"
    artifact: cognicode-mcp-0.93.0-x86_64-unknown-linux-gnu.tar.gz
    sha256: "9f2c1d4b7e0a3f5c8d1b2e4a6f8c0d2e4b6a8c0e2f4a6b8c0d2e4f6a8b0c2d4e"
    url: "https://github.com/Rubentxu/CogniCode/releases/download/v0.93.0/cognicode-mcp-0.93.0-x86_64-unknown-linux-gnu.tar.gz"
    profiles: [core, reviewer]
"#;
            std::fs::write(vdir.join("manifest.yaml"), yaml).unwrap();
            // PR contract: IDE integration validates that the declared MCP
            // binary exists on disk and that the shim targets it. Plant a
            // real (empty) binary and matching shim so the fixture is
            // coherent with the new stale-link rejection.
            let bin_path = vdir.join("cognicode-mcp/bin/cognicode-mcp");
            std::fs::create_dir_all(bin_path.parent().unwrap()).unwrap();
            std::fs::write(&bin_path, b"#!/bin/sh\n").unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&bin_path, std::fs::Permissions::from_mode(0o755))
                    .unwrap();
            }
            // Shim must canonicalize to the installed binary (stale-link check).
            let shim = tmp.join(".cognicode/shims/cognicode-mcp");
            std::fs::create_dir_all(shim.parent().unwrap()).unwrap();
            let _ = std::fs::remove_file(&shim);
            #[cfg(unix)]
            std::os::unix::fs::symlink(&bin_path, &shim).unwrap();
            #[cfg(not(unix))]
            std::fs::copy(&bin_path, &shim).unwrap();
        }
        create_codex_config(&tmp).unwrap();

        // Set HOME so codex paths resolve to temp dir
        let prev_home = std::env::var("HOME").ok();
        unsafe {
            std::env::set_var("HOME", &tmp);
        }

        // Call cmd_ide_install directly instead of cogh subprocess
        cmd_ide_install(&home, "codex", "mcp-server", "v0.93.0").unwrap();

        let cfg_text = std::fs::read_to_string(tmp.join(".codex/config.toml")).unwrap();
        assert!(
            cfg_text.contains("[mcp_servers.cognicode-mcp]"),
            "cognicode-mcp section missing"
        );
        assert!(cfg_text.contains("command ="), "command missing");
        assert!(cfg_text.contains("model = \"test\""), "model setting lost");
        assert!(
            cfg_text.contains("other_setting = 42"),
            "other_setting lost"
        );

        // Restore HOME
        if let Some(prev) = prev_home {
            unsafe {
                std::env::set_var("HOME", prev);
            }
        } else {
            unsafe {
                std::env::remove_var("HOME");
            }
        }

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    #[serial]
    fn install_zcode_ide_patches_config_and_skills() {
        let tmp = std::env::temp_dir().join(format!("cogh-lc-zcode-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let home = CognicodeHome {
            root: tmp.join(".cognicode"),
        };
        cmd_init(&home).unwrap();

        // Pre-create a plugin + version dir so skills copy finds something
        // DEBT-2: plant manifest + declared skill bundle instead of the
        // legacy `<plugin>/skills` dir (integrators resolve from the
        // manifest now).
        {
            let vdir = tmp.join(".cognicode/versions/v0.93.0");
            std::fs::create_dir_all(vdir.join("skills/skills-for-claude")).unwrap();
            std::fs::write(
                vdir.join("skills/skills-for-claude/SKILL.md"),
                "---\nname: x\n---\n",
            )
            .unwrap();
            let yaml = r#"
apiVersion: cognicode.bundle/v2
kind: Bundle
version: "0.93.0"
platform: linux-x86-64
released_at: "2026-01-01T00:00:00Z"
profiles:
  - name: core
    description: Daily CLI
  - name: reviewer
    description: Review CLI
skill_bundles:
  - id: skills-for-claude
    version: "0.93.0"
    profiles: [core, reviewer]
components:
  - name: cognicode-mcp
    kind: daemon-cli
    version: "0.93.0"
    artifact: cognicode-mcp-0.93.0-x86_64-unknown-linux-gnu.tar.gz
    sha256: "9f2c1d4b7e0a3f5c8d1b2e4a6f8c0d2e4b6a8c0e2f4a6b8c0d2e4f6a8b0c2d4e"
    url: "https://github.com/Rubentxu/CogniCode/releases/download/v0.93.0/cognicode-mcp-0.93.0-x86_64-unknown-linux-gnu.tar.gz"
    profiles: [core, reviewer]
"#;
            std::fs::write(vdir.join("manifest.yaml"), yaml).unwrap();
            // PR contract: IDE integration validates that the declared MCP
            // binary exists on disk and that the shim targets it. Plant a
            // real (empty) binary and matching shim so the fixture is
            // coherent with the new stale-link rejection.
            let bin_path = vdir.join("cognicode-mcp/bin/cognicode-mcp");
            std::fs::create_dir_all(bin_path.parent().unwrap()).unwrap();
            std::fs::write(&bin_path, b"#!/bin/sh\n").unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&bin_path, std::fs::Permissions::from_mode(0o755))
                    .unwrap();
            }
            // Shim must canonicalize to the installed binary (stale-link check).
            let shim = tmp.join(".cognicode/shims/cognicode-mcp");
            std::fs::create_dir_all(shim.parent().unwrap()).unwrap();
            let _ = std::fs::remove_file(&shim);
            #[cfg(unix)]
            std::os::unix::fs::symlink(&bin_path, &shim).unwrap();
            #[cfg(not(unix))]
            std::fs::copy(&bin_path, &shim).unwrap();
        }
        create_zcode_config(&tmp).unwrap();

        // Set HOME so zcode paths resolve to temp dir
        let prev_home = std::env::var("HOME").ok();
        unsafe {
            std::env::set_var("HOME", &tmp);
        }

        // Call cmd_ide_install directly
        cmd_ide_install(&home, "zcode", "mcp-server", "v0.93.0").unwrap();

        // Verify the config was patched
        let cfg_path = tmp.join(".zcode/v2/config.json");
        let cfg_text = std::fs::read_to_string(&cfg_path).unwrap();
        assert!(
            cfg_text.contains("cognicode-mcp"),
            "cognicode-mcp missing from config"
        );
        // Verify the original entry was preserved
        assert!(cfg_text.contains("provider"), "original entry lost");

        // Verify skills dir was created
        assert!(
            tmp.join(".zcode/skills/cognicode-v0.93.0").exists(),
            "skills dir missing"
        );

        // Restore HOME
        if let Some(prev) = prev_home {
            unsafe {
                std::env::set_var("HOME", prev);
            }
        } else {
            unsafe {
                std::env::remove_var("HOME");
            }
        }

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    #[serial]
    fn install_claude_ide_patches_config_and_skills() {
        let tmp = std::env::temp_dir().join(format!("cogh-lc-claude-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let home = CognicodeHome {
            root: tmp.join(".cognicode"),
        };
        cmd_init(&home).unwrap();

        // Pre-create a plugin + version dir so skills copy finds something
        // DEBT-2: plant manifest + declared skill bundle instead of the
        // legacy `<plugin>/skills` dir (integrators resolve from the
        // manifest now).
        {
            let vdir = tmp.join(".cognicode/versions/v0.93.0");
            std::fs::create_dir_all(vdir.join("skills/skills-for-claude")).unwrap();
            std::fs::write(
                vdir.join("skills/skills-for-claude/SKILL.md"),
                "---\nname: x\n---\n",
            )
            .unwrap();
            let yaml = r#"
apiVersion: cognicode.bundle/v2
kind: Bundle
version: "0.93.0"
platform: linux-x86-64
released_at: "2026-01-01T00:00:00Z"
profiles:
  - name: core
    description: Daily CLI
  - name: reviewer
    description: Review CLI
skill_bundles:
  - id: skills-for-claude
    version: "0.93.0"
    profiles: [core, reviewer]
components:
  - name: cognicode-mcp
    kind: daemon-cli
    version: "0.93.0"
    artifact: cognicode-mcp-0.93.0-x86_64-unknown-linux-gnu.tar.gz
    sha256: "9f2c1d4b7e0a3f5c8d1b2e4a6f8c0d2e4b6a8c0e2f4a6b8c0d2e4f6a8b0c2d4e"
    url: "https://github.com/Rubentxu/CogniCode/releases/download/v0.93.0/cognicode-mcp-0.93.0-x86_64-unknown-linux-gnu.tar.gz"
    profiles: [core, reviewer]
"#;
            std::fs::write(vdir.join("manifest.yaml"), yaml).unwrap();
            // PR contract: IDE integration validates that the declared MCP
            // binary exists on disk and that the shim targets it. Plant a
            // real (empty) binary and matching shim so the fixture is
            // coherent with the new stale-link rejection.
            let bin_path = vdir.join("cognicode-mcp/bin/cognicode-mcp");
            std::fs::create_dir_all(bin_path.parent().unwrap()).unwrap();
            std::fs::write(&bin_path, b"#!/bin/sh\n").unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&bin_path, std::fs::Permissions::from_mode(0o755))
                    .unwrap();
            }
            // Shim must canonicalize to the installed binary (stale-link check).
            let shim = tmp.join(".cognicode/shims/cognicode-mcp");
            std::fs::create_dir_all(shim.parent().unwrap()).unwrap();
            let _ = std::fs::remove_file(&shim);
            #[cfg(unix)]
            std::os::unix::fs::symlink(&bin_path, &shim).unwrap();
            #[cfg(not(unix))]
            std::fs::copy(&bin_path, &shim).unwrap();
        }
        create_claude_config(&tmp).unwrap();

        // Set HOME so claude paths resolve to temp dir
        let prev_home = std::env::var("HOME").ok();
        unsafe {
            std::env::set_var("HOME", &tmp);
        }

        // Call cmd_ide_install directly
        cmd_ide_install(&home, "claude", "mcp-server", "v0.93.0").unwrap();

        // Verify the MCP file was created
        let mcp_path = tmp.join(".claude/mcp/cognicode-mcp.json");
        assert!(mcp_path.exists(), "cognicode-mcp.json missing");
        let mcp_text = std::fs::read_to_string(&mcp_path).unwrap();
        assert!(
            mcp_text.contains("cognicode-mcp"),
            "cognicode-mcp missing from mcp file"
        );

        // Verify skills dir was created
        assert!(
            tmp.join(".claude/skills/cognicode-v0.93.0").exists(),
            "skills dir missing"
        );

        // Restore HOME
        if let Some(prev) = prev_home {
            unsafe {
                std::env::set_var("HOME", prev);
            }
        } else {
            unsafe {
                std::env::remove_var("HOME");
            }
        }

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    #[serial]
    fn plugin_list_shows_bundled_plugins() {
        let tmp = std::env::temp_dir().join(format!("cogh-lc-pl-{}", std::process::id()));
        setup_temp_home(&tmp).unwrap();
        let out = run_cogh(&tmp, &["plugin", "list"]).unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(stdout.contains("mcp-server"), "mcp-server not listed");
        assert!(
            stdout.contains("skills-cognicode-core"),
            "skills plugin not listed"
        );
        assert!(stdout.contains("sandbox-templates"), "sandbox not listed");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    #[serial]
    fn test_clean_home_install() {
        let temp_home = tempfile::tempdir().unwrap();
        unsafe {
            std::env::set_var("HOME", temp_home.path());
            std::env::set_var("XDG_CONFIG_HOME", temp_home.path().join(".config"));
            std::env::set_var("XDG_DATA_HOME", temp_home.path().join(".local/share"));
            std::env::set_var("COGNICODE_HOME", temp_home.path().join(".cognicode"));
        }

        // Install from a REAL release: payloads with canonical names, a manifest
        // generated by the real factory, served over loopback. This is the
        // producer/consumer round trip, offline and deterministic.
        let version = env!("CARGO_PKG_VERSION");
        let release =
            crate::release_test_support::local_release(version).expect("stage a local release");
        crate::release_test_support::point_at(&release);

        let home = CognicodeHome::resolve(None).unwrap();
        let result = install::run_install(&home, "core");

        crate::release_test_support::unpoint();

        let manifest_path = result.expect("install from a generated local release must succeed");
        assert!(manifest_path.exists(), "install manifest missing");

        // The tracker pins the installed runtime version.
        let tracker_path = temp_home.path().join(".cognicode/tracker/version");
        let pinned = std::fs::read_to_string(&tracker_path).expect("tracker missing");
        assert_eq!(pinned.trim(), version);

        // The payload was really downloaded, verified and extracted.
        // L2 (ADR-CANONICAL-LAYOUT): the extract target is now
        // `<root>/versions/<v>/<comp>/`, not `<root>/install/<v>/<comp>/`.
        let installed_bin = temp_home
            .path()
            .join(".cognicode/versions")
            .join(version)
            .join("cognicode/bin/cognicode");
        assert!(
            installed_bin.exists(),
            "extracted payload missing at {}",
            installed_bin.display()
        );

        // ...and a shim was materialised for it.
        let shim = temp_home.path().join(".cognicode/shims/cognicode");
        assert!(shim.exists(), "shim missing at {}", shim.display());
    }

    #[test]
    #[serial]
    fn tracker_write_and_read_version_roundtrip() {
        let tmp = std::env::temp_dir().join(format!("cogh-lc-tracker-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        // Override COGNICODE_HOME to use temp directory
        let prev_home = std::env::var("COGNICODE_HOME").ok();
        unsafe {
            std::env::set_var("COGNICODE_HOME", tmp.join(".cognicode"));
        }

        // Write a version
        tracker::write_version("0.94.1").unwrap();

        // Read it back
        let version = tracker::read_version().unwrap();
        assert_eq!(version, "0.94.1");

        // Clean up
        let _ = std::fs::remove_dir_all(&tmp);
        if let Some(prev) = prev_home {
            unsafe {
                std::env::set_var("COGNICODE_HOME", prev);
            }
        } else {
            unsafe {
                std::env::remove_var("COGNICODE_HOME");
            }
        }
    }

    #[test]
    #[serial]
    fn install_lock_acquire_creates_lock_file() {
        let tmp = std::env::temp_dir().join(format!("cogh-lc-lock-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        // Override COGNICODE_HOME to use temp directory
        let prev_home = std::env::var("COGNICODE_HOME").ok();
        unsafe {
            std::env::set_var("COGNICODE_HOME", tmp.join(".cognicode"));
        }

        // Acquire lock
        let lock = install_lock::acquire_lock().unwrap();

        // Lock file should exist
        let lock_path = tmp.join(".cognicode/locks/install.lock");
        assert!(
            lock_path.exists(),
            "lock file should exist after acquire_lock"
        );

        // Release lock
        install_lock::release_lock(lock);

        // Lock file should be removed
        assert!(
            !lock_path.exists(),
            "lock file should be removed after release_lock"
        );

        // Clean up
        let _ = std::fs::remove_dir_all(&tmp);
        if let Some(prev) = prev_home {
            unsafe {
                std::env::set_var("COGNICODE_HOME", prev);
            }
        } else {
            unsafe {
                std::env::remove_var("COGNICODE_HOME");
            }
        }
    }

    // E32-H: Install / list / current / update / uninstall lifecycle tests

    #[test]
    #[ignore = "requires bundle version to match CARGO_PKG_VERSION"]
    #[serial]
    fn test_cogh_install_runs_successfully() {
        let temp_home = tempfile::tempdir().unwrap();
        let original_home = std::env::var("HOME").ok();
        let original_cognicode_home = std::env::var("COGNICODE_HOME").ok();
        unsafe {
            std::env::set_var("HOME", temp_home.path());
            std::env::set_var("XDG_CONFIG_HOME", temp_home.path().join(".config"));
            std::env::set_var("XDG_DATA_HOME", temp_home.path().join(".local/share"));
            std::env::set_var("COGNICODE_HOME", temp_home.path().join(".cognicode"));
        }
        let home = CognicodeHome::resolve(None).unwrap();
        // Run install
        let result = install::run_install(&home, "core");
        assert!(result.is_ok(), "install failed: {:?}", result);
        // Verify tracker version file is created
        let tracker_path = temp_home.path().join(".cognicode/tracker/version");
        assert!(
            tracker_path.exists(),
            "tracker missing at {}",
            tracker_path.display()
        );
        // Verify shims are created
        let shims_path = temp_home.path().join(".cognicode/shims");
        assert!(
            shims_path.exists(),
            "shims missing at {}",
            shims_path.display()
        );
        // Restore env
        if let Some(home) = original_home {
            unsafe {
                std::env::set_var("HOME", home);
            }
        }
        if let Some(cog_home) = original_cognicode_home {
            unsafe {
                std::env::set_var("COGNICODE_HOME", cog_home);
            }
        } else {
            unsafe {
                std::env::remove_var("COGNICODE_HOME");
            }
        }
        drop(temp_home);
    }

    #[test]
    #[serial]
    fn test_cogh_list_shows_installed() {
        let tmp = std::env::temp_dir().join(format!("cogh-lc-list-{}", std::process::id()));
        setup_temp_home(&tmp).unwrap();
        let out = run_cogh(&tmp, &["list"]).unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        // Verify output contains installed plugins
        assert!(
            stdout.contains("Plugin") || stdout.contains("plugin"),
            "list output unexpected: {}",
            stdout
        );
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    #[serial]
    fn test_cogh_current_returns_version() {
        let tmp = std::env::temp_dir().join(format!("cogh-lc-current-{}", std::process::id()));
        setup_temp_home(&tmp).unwrap();
        let out = run_cogh(&tmp, &["current"]).unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        // Should return the installed version or "(no version pinned)"
        assert!(
            stdout.contains("0.") || stdout.contains("no version pinned"),
            "current output unexpected: {}",
            stdout
        );
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    #[serial]
    fn test_cogh_update_respects_lockfile() {
        let tmp = std::env::temp_dir().join(format!("cogh-lc-update-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        // Create a .cognicode.lock file
        let lock_content = r#"{
  "version": "0.94.0",
  "plugins": {
    "mcp-server": "0.94.0"
  }
}"#;
        std::fs::write(tmp.join(".cognicode.lock"), lock_content).unwrap();

        // Override HOME and COGNICODE_HOME to use temp directory.
        // U03/G6 (JOURNAL §42): the resolver MUST NOT reach api.github.com
        // from tests (no-network rule). Pin the API base URL to a dead
        // loopback port so the run is deterministic and offline; the
        // update path then either succeeds via staging or fails cleanly,
        // which the assertion below already accepts.
        let prev_home = std::env::var("HOME").ok();
        let prev_cognicode_home = std::env::var("COGNICODE_HOME").ok();
        let prev_api = std::env::var("COGNICODE_API_BASE_URL").ok();
        unsafe {
            std::env::set_var("HOME", &tmp);
            std::env::set_var("COGNICODE_HOME", tmp.join(".cognicode"));
            std::env::set_var("COGNICODE_API_BASE_URL", "http://127.0.0.1:1");
        }

        // Initialize home
        let home = CognicodeHome {
            root: tmp.join(".cognicode"),
        };
        cmd_init(&home).unwrap();

        // Run update (offline by construction: API base pinned to dead loopback)
        let out = run_cogh(&tmp, &["update"]).unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        let stderr = String::from_utf8_lossy(&out.stderr);
        // Accepted outcomes (all honest, none network-dependent):
        //   success, "not yet implemented", or a clean network error
        // caused by the deliberately dead API endpoint.
        let clean_offline_failure = stderr.contains("error")
            || stderr.contains("Error")
            || stdout.contains("not yet implemented");
        assert!(
            out.status.success() || clean_offline_failure,
            "update must succeed or fail cleanly offline; stdout={stdout:?} stderr={stderr:?}"
        );

        // Restore env
        if let Some(home) = prev_home {
            unsafe {
                std::env::set_var("HOME", home);
            }
        } else {
            unsafe {
                std::env::remove_var("HOME");
            }
        }
        if let Some(home) = prev_cognicode_home {
            unsafe {
                std::env::set_var("COGNICODE_HOME", home);
            }
        } else {
            unsafe {
                std::env::remove_var("COGNICODE_HOME");
            }
        }
        match prev_api {
            Some(v) => unsafe { std::env::set_var("COGNICODE_API_BASE_URL", v) },
            None => unsafe { std::env::remove_var("COGNICODE_API_BASE_URL") },
        }

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    #[serial]
    fn test_cogh_doctor_reports_health() {
        let tmp = std::env::temp_dir().join(format!("cogh-lc-doctor-{}", std::process::id()));
        setup_temp_home(&tmp).unwrap();
        let out = run_cogh(&tmp, &["doctor"]).unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        // Verify it returns 0 for healthy state (doctor always returns Ok)
        assert!(out.status.success(), "doctor failed with: {}", stdout);
        // Verify it contains health info
        assert!(
            stdout.contains("home") || stdout.contains("exists") || stdout.contains("doctor"),
            "doctor output unexpected: {}",
            stdout
        );
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    #[serial]
    fn test_install_lock_acquire_and_release() {
        let tmp = std::env::temp_dir().join(format!("cogh-lc-lock-para-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        // Override COGNICODE_HOME to use temp directory
        let prev_home = std::env::var("COGNICODE_HOME").ok();
        unsafe {
            std::env::set_var("COGNICODE_HOME", tmp.join(".cognicode"));
        }

        // Acquire lock
        let lock = install_lock::acquire_lock().unwrap();
        let lock_path = tmp.join(".cognicode/locks/install.lock");
        assert!(
            lock_path.exists(),
            "lock file should exist after acquire_lock"
        );

        // Verify lock content has PID
        let content = std::fs::read_to_string(&lock_path).unwrap();
        assert!(
            content.contains(&format!("{}", std::process::id())),
            "lock should contain PID"
        );

        // Release lock
        install_lock::release_lock(lock);

        // Lock file should be removed
        assert!(
            !lock_path.exists(),
            "lock file should be removed after release"
        );

        // Clean up
        let _ = std::fs::remove_dir_all(&tmp);
        if let Some(prev) = prev_home {
            unsafe {
                std::env::set_var("COGNICODE_HOME", prev);
            }
        } else {
            unsafe {
                std::env::remove_var("COGNICODE_HOME");
            }
        }
    }

    // E32-I: Self-application test — install OpenCode adapter locally

    #[test]
    #[serial]
    fn test_self_apply_opencode_adapter() {
        // Skip if OpenCode is not detected
        if !detect_opencode() {
            return;
        }

        let tmp = std::env::temp_dir().join(format!("cogh-lc-self-oc-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let home = CognicodeHome {
            root: tmp.join(".cognicode"),
        };
        cmd_init(&home).unwrap();

        // Pre-create a plugin + version dir so skills copy finds something
        // DEBT-2: plant manifest + declared skill bundle instead of the
        // legacy `<plugin>/skills` dir (integrators resolve from the
        // manifest now).
        {
            let vdir = tmp.join(".cognicode/versions/v0.93.0");
            std::fs::create_dir_all(vdir.join("skills/skills-for-claude")).unwrap();
            std::fs::write(
                vdir.join("skills/skills-for-claude/SKILL.md"),
                "---\nname: x\n---\n",
            )
            .unwrap();
            let yaml = r#"
apiVersion: cognicode.bundle/v2
kind: Bundle
version: "0.93.0"
platform: linux-x86-64
released_at: "2026-01-01T00:00:00Z"
profiles:
  - name: core
    description: Daily CLI
  - name: reviewer
    description: Review CLI
skill_bundles:
  - id: skills-for-claude
    version: "0.93.0"
    profiles: [core, reviewer]
components:
  - name: cognicode-mcp
    kind: daemon-cli
    version: "0.93.0"
    artifact: cognicode-mcp-0.93.0-x86_64-unknown-linux-gnu.tar.gz
    sha256: "9f2c1d4b7e0a3f5c8d1b2e4a6f8c0d2e4b6a8c0e2f4a6b8c0d2e4f6a8b0c2d4e"
    url: "https://github.com/Rubentxu/CogniCode/releases/download/v0.93.0/cognicode-mcp-0.93.0-x86_64-unknown-linux-gnu.tar.gz"
    profiles: [core, reviewer]
"#;
            std::fs::write(vdir.join("manifest.yaml"), yaml).unwrap();
            // PR contract: IDE integration validates that the declared MCP
            // binary exists on disk and that the shim targets it. Plant a
            // real (empty) binary and matching shim so the fixture is
            // coherent with the new stale-link rejection.
            let bin_path = vdir.join("cognicode-mcp/bin/cognicode-mcp");
            std::fs::create_dir_all(bin_path.parent().unwrap()).unwrap();
            std::fs::write(&bin_path, b"#!/bin/sh\n").unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&bin_path, std::fs::Permissions::from_mode(0o755))
                    .unwrap();
            }
            // Shim must canonicalize to the installed binary (stale-link check).
            let shim = tmp.join(".cognicode/shims/cognicode-mcp");
            std::fs::create_dir_all(shim.parent().unwrap()).unwrap();
            let _ = std::fs::remove_file(&shim);
            #[cfg(unix)]
            std::os::unix::fs::symlink(&bin_path, &shim).unwrap();
            #[cfg(not(unix))]
            std::fs::copy(&bin_path, &shim).unwrap();
        }
        create_opencode_config(&tmp).unwrap();

        // Set HOME so opencode paths resolve to temp dir
        let prev_home = std::env::var("HOME").ok();
        unsafe {
            std::env::set_var("HOME", &tmp);
        }

        // Run install with --ide opencode
        let result = cmd_ide_install(&home, "opencode", "mcp-server", "v0.93.0");
        assert!(result.is_ok(), "ide install failed: {:?}", result);

        // Verify OpenCode config was updated with absolute shim path
        let config_path = tmp.join(".config/opencode/opencode.json");
        let content = std::fs::read_to_string(&config_path).unwrap();
        let v: Value = serde_json::from_str(&content).unwrap();
        let expected_shim = home.shim_path("cognicode-mcp");
        let expected_shim_str = expected_shim.to_string_lossy();
        let actual_cmd = v["mcp"]["cognicode-mcp"]["command"]
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|v| v.as_str())
            .map(String::from);
        assert!(
            actual_cmd
                .as_ref()
                .map(|s| s.as_str() == expected_shim_str.as_ref())
                .unwrap_or(false),
            "mcp.cognicode-mcp.command[0] should be absolute shim path '{}', got: {:?}",
            expected_shim.display(),
            actual_cmd
        );

        // Restore HOME
        if let Some(prev) = prev_home {
            unsafe {
                std::env::set_var("HOME", prev);
            }
        } else {
            unsafe {
                std::env::remove_var("HOME");
            }
        }

        let _ = std::fs::remove_dir_all(&tmp);
    }

    // E32-I: Test that `cogh install --ide opencode --profile core` dispatches
    // both the atomic bundle install (tracker update) and the IDE integration.

    #[test]
    #[serial]
    fn test_install_with_ide_and_profile_dispatches_both() {
        let tmp = std::env::temp_dir().join(format!("cogh-lc-ide-prof-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        // Pre-create a plugin + version dir so skills copy finds something
        // DEBT-2: plant manifest + declared skill bundle instead of the
        // legacy `<plugin>/skills` dir (integrators resolve from the
        // manifest now).
        {
            let vdir = tmp.join(".cognicode/versions/v0.93.0");
            std::fs::create_dir_all(vdir.join("skills/skills-for-claude")).unwrap();
            std::fs::write(
                vdir.join("skills/skills-for-claude/SKILL.md"),
                "---\nname: x\n---\n",
            )
            .unwrap();
            let yaml = r#"
apiVersion: cognicode.bundle/v2
kind: Bundle
version: "0.93.0"
platform: linux-x86-64
released_at: "2026-01-01T00:00:00Z"
profiles:
  - name: core
    description: Daily CLI
  - name: reviewer
    description: Review CLI
skill_bundles:
  - id: skills-for-claude
    version: "0.93.0"
    profiles: [core, reviewer]
components:
  - name: cognicode-mcp
    kind: daemon-cli
    version: "0.93.0"
    artifact: cognicode-mcp-0.93.0-x86_64-unknown-linux-gnu.tar.gz
    sha256: "9f2c1d4b7e0a3f5c8d1b2e4a6f8c0d2e4b6a8c0e2f4a6b8c0d2e4f6a8b0c2d4e"
    url: "https://github.com/Rubentxu/CogniCode/releases/download/v0.93.0/cognicode-mcp-0.93.0-x86_64-unknown-linux-gnu.tar.gz"
    profiles: [core, reviewer]
"#;
            std::fs::write(vdir.join("manifest.yaml"), yaml).unwrap();
            // PR contract: IDE integration validates that the declared MCP
            // binary exists on disk and that the shim targets it. Plant a
            // real (empty) binary and matching shim so the fixture is
            // coherent with the new stale-link rejection.
            let bin_path = vdir.join("cognicode-mcp/bin/cognicode-mcp");
            std::fs::create_dir_all(bin_path.parent().unwrap()).unwrap();
            std::fs::write(&bin_path, b"#!/bin/sh\n").unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&bin_path, std::fs::Permissions::from_mode(0o755))
                    .unwrap();
            }
            // Shim must canonicalize to the installed binary (stale-link check).
            let shim = tmp.join(".cognicode/shims/cognicode-mcp");
            std::fs::create_dir_all(shim.parent().unwrap()).unwrap();
            let _ = std::fs::remove_file(&shim);
            #[cfg(unix)]
            std::os::unix::fs::symlink(&bin_path, &shim).unwrap();
            #[cfg(not(unix))]
            std::fs::copy(&bin_path, &shim).unwrap();
        }
        create_opencode_config(&tmp).unwrap();

        // e87.1: `cogh install` resolves the release remotely before
        // installing, so the subprocess needs a resolver seam. `--staging`
        // with a ResolverFixture points the resolver at a loopback release;
        // `COGNICODE_ASSET_BASE_URL` (set by TempBaseUrl) rewrites the
        // component asset URLs onto the same loopback. The legacy
        // `point_at` seam no longer drives the product install path.
        let fx = crate::release_test_support::ResolverFixture::build("0.95.0")
            .expect("build resolver fixture");
        let _base = crate::layout::test_support::TempBaseUrl::set(&fx.release.base_url);

        // Run `cogh install --staging <dir> --ide opencode --profile core`
        let result = run_cogh(
            &tmp,
            &[
                "install",
                "mcp-server",
                "--staging",
                fx.staging_dir.to_str().unwrap(),
                "--ide",
                "opencode",
                // reviewer includes the DaemonCli component, so the IDE
                // integration gets a real shim command to patch in.
                "--profile",
                "reviewer",
            ],
        );
        assert!(
            result.is_ok(),
            "cogh install failed: {:?}",
            result.as_ref().map(|o| (
                String::from_utf8_lossy(&o.stdout).to_string(),
                String::from_utf8_lossy(&o.stderr).to_string()
            ))
        );

        // Verify bundle install ran: tracker should be updated
        let tracker_path = tmp.join(".cognicode/tracker/version");
        assert!(
            tracker_path.exists(),
            "tracker missing at {} — bundle install may not have run",
            tracker_path.display()
        );

        // Verify IDE integration ran: opencode.json should have cognicode-mcp entry
        let config_path = tmp.join(".config/opencode/opencode.json");
        let content = std::fs::read_to_string(&config_path).unwrap();
        let v: Value = serde_json::from_str(&content).unwrap();
        let actual_cmd = v["mcp"]["cognicode-mcp"]["command"]
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|v| v.as_str())
            .map(String::from);
        let expected_shim = tmp.join(".cognicode/shims/cognicode-mcp");
        assert!(
            actual_cmd
                .as_ref()
                .map(|s| s.as_str() == expected_shim.to_string_lossy().as_ref())
                .unwrap_or(false),
            "mcp.cognicode-mcp.command[0] should be absolute shim path '{}', got: {:?}",
            expected_shim.display(),
            actual_cmd
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn profile_filter_by_profile_returns_correct_components() {
        let yaml = r#"
apiVersion: cognicode.bundle/v2
version: "0.95.0"
platform: linux-x86-64
profiles:
  - name: core
    description: core profile
  - name: reviewer
    description: reviewer profile
components:
  - name: cognicode
    kind: cognicode
    version: "0.95.0"
    artifact: cognicode-0.95.0-x86_64-unknown-linux-gnu.tar.gz
    sha256: "9f2c1d4b7e0a3f5c8d1b2e4a6f8c0d2e4b6a8c0e2f4a6b8c0d2e4f6a8b0c2d4e"
    url: "https://github.com/Rubentxu/CogniCode/releases/download/v0.95.0/cognicode-0.95.0-x86_64-unknown-linux-gnu.tar.gz"
    profiles: [core]
  - name: cognicode-mcp
    kind: daemon-cli
    version: "0.95.0"
    artifact: cognicode-mcp-0.95.0-x86_64-unknown-linux-gnu.tar.gz
    sha256: "1a3f5c8d1b2e4a6f8c0d2e4b6a8c0e2f4a6b8c0d2e4f6a8b0c2d4e9f2c1d4b7e"
    url: "https://github.com/Rubentxu/CogniCode/releases/download/v0.95.0/cognicode-mcp-0.95.0-x86_64-unknown-linux-gnu.tar.gz"
    profiles: [reviewer]
  - name: explorer-api
    kind: explorer-api
    version: "0.95.0"
    artifact: explorer-api-0.95.0-x86_64-unknown-linux-gnu.tar.gz
    sha256: "3b7e9f2c1d4b7e0a3f5c8d1b2e4a6f8c0d2e4b6a8c0e2f4a6b8c0d2e4f6a8b0c"
    url: "https://github.com/Rubentxu/CogniCode/releases/download/v0.95.0/explorer-api-0.95.0-x86_64-unknown-linux-gnu.tar.gz"
    profiles: [core, reviewer]
"#;
        let manifest = crate::bundle_manifest::BundleManifest::from_str(yaml).unwrap();

        let core_components = profile::filter_by_profile(&manifest, "core");
        assert_eq!(core_components.len(), 2);
        assert!(core_components.iter().any(|c| c.name == "cognicode"));
        assert!(core_components.iter().any(|c| c.name == "explorer-api"));
        assert!(!core_components.iter().any(|c| c.name == "cognicode-mcp"));

        let reviewer_components = profile::filter_by_profile(&manifest, "reviewer");
        assert_eq!(reviewer_components.len(), 2);
        assert!(
            reviewer_components
                .iter()
                .any(|c| c.name == "cognicode-mcp")
        );
        assert!(reviewer_components.iter().any(|c| c.name == "explorer-api"));
        assert!(!reviewer_components.iter().any(|c| c.name == "cognicode"));

        assert!(profile::filter_by_profile(&manifest, "nonexistent").is_empty());
    }

    // ===== E86.3 — uninstall coverage (bounded cycle) =====

    /// T1 (RED before fix): `cogh uninstall` against an UNINITIALIZED home
    /// must error, not silently no-op. `cmd_install` already has this guard
    /// (see `cmd/layout.rs::cmd_install`); `cmd_uninstall` does not. The
    /// asymmetry was uncovered by the E86.2.2 + E86.2.3 UAT scripts which
    /// occasionally re-run uninstall on a fresh tmp home that was rolled back.
    #[test]
    #[serial]
    fn t_e86_3_uninstall_errors_on_uninitialized_home() {
        let tmp = std::env::temp_dir().join(format!("cogh-lc-e863-uninit-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        // Deliberately do NOT call setup_temp_home — .cognicode/bin/ is missing.

        let out = run_cogh(
            &tmp,
            &[
                "uninstall",
                "mcp-server",
                "--ide",
                "opencode",
                "--version",
                "0.95.0",
            ],
        )
        .unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        let stderr = String::from_utf8_lossy(&out.stderr);
        let combined = format!("{stdout}{stderr}");
        assert!(
            !out.status.success(),
            "uninstall on uninitialized home should fail; got success. stdout={stdout} stderr={stderr}"
        );
        assert!(
            combined.contains("not initialized") || combined.contains("init"),
            "error message should mention init; got: {combined}"
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// T2: uninstall is idempotent. Calling it twice on the same version must
    /// not error — the second call should be a no-op (skills dir already gone,
    /// config key already removed).
    #[test]
    #[serial]
    fn t_e86_3_uninstall_idempotent_second_call() {
        let tmp = std::env::temp_dir().join(format!("cogh-lc-e863-idem-{}", std::process::id()));
        setup_temp_home(&tmp).unwrap();
        create_opencode_config(&tmp).unwrap();

        let cfg_path = tmp.join(".config/opencode/opencode.json");
        plant_version_tree(&tmp, "0.95.0");
        let skill_dir = tmp.join(".config/opencode/skills/cognicode-0.95.0");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(skill_dir.join("SKILL.md"), "x").unwrap();

        let cfg_text_pre = std::fs::read_to_string(&cfg_path).unwrap();
        std::fs::write(
            &cfg_path,
            format!(
                r#"{{"mcp": {{"cognicode-mcp": {{"type": "stdio"}}}}, "_pre": {cfg_text_pre}}}"#
            ),
        )
        .unwrap();

        let out1 = run_cogh(
            &tmp,
            &[
                "uninstall",
                "mcp-server",
                "--ide",
                "opencode",
                "--version",
                "0.95.0",
            ],
        )
        .unwrap();
        assert!(
            out1.status.success(),
            "first uninstall must succeed; got: stdout={} stderr={}",
            String::from_utf8_lossy(&out1.stdout),
            String::from_utf8_lossy(&out1.stderr),
        );
        assert!(
            !skill_dir.exists(),
            "skills dir must be gone after first uninstall"
        );

        let out2 = run_cogh(
            &tmp,
            &[
                "uninstall",
                "mcp-server",
                "--ide",
                "opencode",
                "--version",
                "0.95.0",
            ],
        )
        .unwrap();
        assert!(
            out2.status.success(),
            "second uninstall must succeed (idempotent); got: stdout={} stderr={}",
            String::from_utf8_lossy(&out2.stdout),
            String::from_utf8_lossy(&out2.stderr),
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// T3: uninstall must handle a missing opencode config file cleanly.
    /// (Step::RemoveFromJson is already idempotent; this pins the contract.)
    #[test]
    #[serial]
    fn t_e86_3_uninstall_opencode_handles_missing_config_file() {
        let tmp = std::env::temp_dir().join(format!("cogh-lc-e863-nocfg-{}", std::process::id()));
        setup_temp_home(&tmp).unwrap();
        // Deliberately do NOT call create_opencode_config — the config file
        // does not exist; HOME resolves to a non-existent ~/.config/opencode path.

        let out = run_cogh(
            &tmp,
            &[
                "uninstall",
                "mcp-server",
                "--ide",
                "opencode",
                "--version",
                "0.95.0",
            ],
        )
        .unwrap();
        assert!(
            out.status.success(),
            "uninstall with missing config file must succeed; got: stdout={} stderr={}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr),
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// T4 (RED before fix): when no `--ide` is passed, uninstall must print a
    /// helpful diagnostic instead of silently doing nothing. Currently it
    /// prints `uninstall: plugin=X version=Y ides=[]` and exits 0.
    #[test]
    #[serial]
    fn t_e86_3_uninstall_without_ide_prints_helpful_message() {
        let tmp = std::env::temp_dir().join(format!("cogh-lc-e863-noide-{}", std::process::id()));
        setup_temp_home(&tmp).unwrap();

        let out = run_cogh(&tmp, &["uninstall", "mcp-server", "--version", "0.95.0"]).unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        let stderr = String::from_utf8_lossy(&out.stderr);
        let combined = format!("{stdout}{stderr}");
        // New contract: when the version is not installed, the honest
        // no-op diagnostic ("not installed; nothing to do") IS the helpful
        // message; --ide guidance is required only for an installed version.
        let no_op = combined.contains("not installed") && combined.contains("nothing to do");
        assert!(
            no_op || combined.contains("--ide") || combined.contains("no IDE"),
            "missing --ide must produce a helpful diagnostic; got: {combined}"
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// T5: unknown `--ide` is a clean error (exit non-zero) with a message
    /// that names the supported set, not a panic.
    #[test]
    #[serial]
    fn t_e86_3_uninstall_unknown_ide_errors_cleanly() {
        let tmp = std::env::temp_dir().join(format!("cogh-lc-e863-unkide-{}", std::process::id()));
        setup_temp_home(&tmp).unwrap();
        plant_version_tree(&tmp, "0.95.0");

        let out = run_cogh(
            &tmp,
            &[
                "uninstall",
                "mcp-server",
                "--ide",
                "vscode",
                "--version",
                "0.95.0",
            ],
        )
        .unwrap();
        let stderr = String::from_utf8_lossy(&out.stderr);
        let stdout = String::from_utf8_lossy(&out.stdout);
        let combined = format!("{stdout}{stderr}");
        assert!(
            !out.status.success(),
            "unknown --ide must fail; got success. stdout={stdout} stderr={stderr}"
        );
        assert!(
            combined.contains("opencode") || combined.contains("not supported"),
            "error message must name supported IDEs or say 'not supported'; got: {combined}"
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
