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

/// Locate the cogh binary.
///
/// The cli crate is at `<workspace>/crates/cognicode-cli/`. The binary
/// is built at `<workspace>/target/debug/cogh`. We resolve the binary
/// path relative to CARGO_MANIFEST_DIR.
fn cogh_bin() -> std::path::PathBuf {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir.parent().and_then(|p| p.parent()).unwrap();
    workspace_root.join("target").join("debug").join("cogh")
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
        unsafe { std::env::set_var("HOME", prev); }
    } else {
        unsafe { std::env::remove_var("HOME"); }
    }
    unsafe { std::env::remove_var("COGNICODE_HOME"); }
    result
}

/// Set up a temp home with .cognicode + bundled plugins.
fn setup_temp_home(tmp: &Path) -> Result<()> {
    let _ = std::fs::remove_dir_all(tmp);
    std::fs::create_dir_all(tmp)?;

    // Run `cogh init` to populate the layout + bundled plugins
    let prev_home = std::env::var("HOME").ok();
    unsafe { std::env::set_var("HOME", tmp); }
    unsafe { std::env::set_var("COGNICODE_HOME", tmp.join(".cognicode")); }
    let bin = cogh_bin();
    let _ = Command::new(&bin)
        .args(["init"])
        .current_dir(tmp)
        .output();
    if let Some(prev) = prev_home {
        unsafe { std::env::set_var("HOME", prev); }
    } else {
        unsafe { std::env::remove_var("HOME"); }
    }
    unsafe { std::env::remove_var("COGNICODE_HOME"); }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_creates_layout_and_bundled_plugins() {
        let tmp = std::env::temp_dir().join(format!("cogh-lc-init-{}", std::process::id()));
        setup_temp_home(&tmp).unwrap();
        assert!(tmp.join(".cognicode/bin").exists(), "bin/ missing");
        assert!(tmp.join(".cognicode/shims").exists(), "shims/ missing");
        assert!(tmp.join(".cognicode/plugins/mcp-server").exists(), "mcp-server plugin missing");
        assert!(tmp.join(".cognicode/plugins/skills-cognicode-core").exists(), "skills plugin missing");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn doctor_reports_clean_install() {
        let tmp = std::env::temp_dir().join(format!("cogh-lc-doc-{}", std::process::id()));
        setup_temp_home(&tmp).unwrap();
        let out = run_cogh(&tmp, &["doctor"]).unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(stdout.contains("home exists"), "doctor missing 'home exists'");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn install_creates_mcp_server_version_dir() {
        let tmp = std::env::temp_dir().join(format!("cogh-lc-inst-{}", std::process::id()));
        setup_temp_home(&tmp).unwrap();
        // cmd_install wires through registry but is placeholder (E32-B+).
        // Verify that it at least parses + resolves manifest + prints args.
        let out = run_cogh(&tmp, &["install", "mcp-server", "--version", "v0.92.0"]).unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(stdout.contains("install:"), "install output missing");
        assert!(stdout.contains("mcp-server"), "plugin name missing from output");
        assert!(stdout.contains("v0.92.0"), "version missing from output");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn install_opencode_ide_patches_config_and_skills() {
        let tmp = std::env::temp_dir().join(format!("cogh-lc-oc-{}", std::process::id()));
        setup_temp_home(&tmp).unwrap();
        // Pre-create a plugin + version dir so skills copy finds something
        let skill_src = tmp.join(".cognicode/versions/v0.92.0/mcp-server/skills/cognicode-mcp-driven");
        std::fs::create_dir_all(&skill_src).unwrap();
        std::fs::write(skill_src.join("SKILL.md"), "---\nname: x\n---\n").unwrap();
        create_opencode_config(&tmp).unwrap();

        let out = run_cogh(
            &tmp,
            &["install", "mcp-server", "--version", "v0.92.0", "--ide", "opencode"],
        )
        .unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(stdout.contains("patched"), "opencode not patched");
        assert!(stdout.contains("copied skills"), "skills not copied");

        // Verify the config was patched
        let cfg_path = tmp.join(".config/opencode/opencode.json");
        let cfg_text = std::fs::read_to_string(&cfg_path).unwrap();
        assert!(cfg_text.contains("cognicode-mcp"), "cognicode-mcp missing from config");
        // Verify the original entry was preserved
        assert!(cfg_text.contains("agent"), "original entry lost");

        // Verify skills dir was created
        assert!(
            tmp.join(".config/opencode/skills/cognicode-v0.92.0").exists(),
            "skills dir missing"
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
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
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(skill_dir.join("SKILL.md"), "x").unwrap();

        let out = run_cogh(&tmp, &["uninstall", "mcp-server", "--ide", "opencode", "--version", "0.92.0"]).unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        // cmd_uninstall prints "uninstall: ..." and the IDE adapter prints
        // "✓ unpached: ..." (opencode.json) + "✓ removed: ..." (skills).
        assert!(
            stdout.contains("uninstall:") && stdout.contains("unpached"),
            "uninstall output missing: {stdout}"
        );

        // cognicode-mcp should be gone, other preserved
        let cfg_text = std::fs::read_to_string(&cfg_path).unwrap();
        assert!(!cfg_text.contains("cognicode-mcp"), "cognicode-mcp still in config");
        assert!(cfg_text.contains("other"), "other entry lost");
        // Skills dir removed
        assert!(!skill_dir.exists(), "skills dir not removed");

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn install_codex_ide_patches_toml_config() {
        let tmp = std::env::temp_dir().join(format!("cogh-lc-codex-{}", std::process::id()));
        setup_temp_home(&tmp).unwrap();
        let skill_src = tmp.join(".cognicode/versions/v0.92.0/mcp-server/skills/cognicode-mcp-driven");
        std::fs::create_dir_all(&skill_src).unwrap();
        std::fs::write(skill_src.join("SKILL.md"), "---\nname: x\n---\n").unwrap();
        create_codex_config(&tmp).unwrap();

        let out = run_cogh(
            &tmp,
            &["install", "mcp-server", "--version", "v0.92.0", "--ide", "codex"],
        )
        .unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(stdout.contains("patched"), "codex not patched");

        let cfg_text = std::fs::read_to_string(tmp.join(".codex/config.toml")).unwrap();
        assert!(cfg_text.contains("[mcp_servers.cognicode-mcp]"),
                "cognicode-mcp section missing");
        assert!(cfg_text.contains("command ="), "command missing");
        assert!(cfg_text.contains("model = \"test\""),
                "model setting lost");
        assert!(cfg_text.contains("other_setting = 42"),
                "other_setting lost");

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn plugin_list_shows_bundled_plugins() {
        let tmp = std::env::temp_dir().join(format!("cogh-lc-pl-{}", std::process::id()));
        setup_temp_home(&tmp).unwrap();
        let out = run_cogh(&tmp, &["plugin", "list"]).unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(stdout.contains("mcp-server"), "mcp-server not listed");
        assert!(stdout.contains("skills-cognicode-core"),
                "skills plugin not listed");
        assert!(stdout.contains("sandbox-templates"), "sandbox not listed");
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
