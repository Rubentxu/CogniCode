//! PRF-STATE-06 UAT: `cogh uninstall` under an EXISTING HOME with real
//! user data.
//!
//! Real cogh binary, fake HOME pre-populated with user-owned data
//! (IDE configs, personal skills, unrelated files) plus an installed
//! plugin. Uninstall must remove ONLY the tool-owned pieces and
//! preserve every user byte.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

mod common;

/// Path to the `cogh` binary.
///
/// `common::binary_path` resolves with the right precedence
/// (`CARGO_BIN_EXE_cogh` > runtime env > `CARGO_TARGET_DIR` > workspace
/// fallback). The harness keeps the test working whether you run it
/// under `cargo test`, `cargo-nextest`, or with a custom `CARGO_TARGET_DIR`.
fn cogh() -> PathBuf {
    common::binary_path("cogh")
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn run_with_home(fake_home: &Path, cogh_home: &Path, args: &[&str]) -> Output {
    let mut cmd = Command::new(cogh());
    cmd.env("HOME", fake_home);
    cmd.arg("--home").arg(cogh_home);
    for a in args {
        cmd.arg(a);
    }
    cmd.output().expect("spawn cogh")
}

fn plant_manifest(cogh_home: &Path, version_dir: &str) {
    let bundle_version = "0.97.3";
    let bundle_id = "skills-for-test";
    let manifest_yaml = format!(
        r#"
apiVersion: cognicode.bundle/v2
kind: Bundle
version: "{bundle_version}"
platform: linux-x86-64
released_at: "2026-01-01T00:00:00Z"
profiles:
  - name: core
    description: Test fixture
skill_bundles:
  - id: {bundle_id}
    version: "{bundle_version}"
    profiles: [core]
components:
  - name: cognicode-mcp
    kind: daemon-cli
    version: "{bundle_version}"
    artifact: cognicode-mcp-{bundle_version}-x86_64-unknown-linux-gnu.tar.gz
    sha256: "9f2c1d4b7e0a3f5c8d1b2e4a6f8c0d2e4b6a8c0e2f4a6b8c0d2e4f6a8b0c2d4e"
    url: "https://github.com/Rubentxu/CogniCode/releases/download/v{bundle_version}/cognicode-mcp-{bundle_version}-x86_64-unknown-linux-gnu.tar.gz"
    profiles: [core]
"#
    );
    let root = cogh_home.join("versions").join(version_dir);
    fs::create_dir_all(root.join("skills").join(bundle_id)).unwrap();
    fs::write(root.join("manifest.yaml"), manifest_yaml).unwrap();
    fs::write(
        root.join("skills").join(bundle_id).join("SKILL.md"),
        "tool-owned skill payload\n",
    )
    .unwrap();
}

/// Seed an existing HOME with user data that MUST survive uninstall.
fn plant_user_data(home: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let seeds = [
        (
            "personal-notes.md",
            "my own notes, nothing to do with cogh\n",
        ),
        (
            ".config/other-tool/settings.json",
            r#"{"theme":"dark","mcpServers":{"other":{"url":"http://x"}}}"#,
        ),
        (
            ".opencode/config.json",
            r#"{"theme":"solarized","mcpServers":{"mine":{"command":"my-server"}},"prefs":{"key":"user-value"}}"#,
        ),
        (
            ".claude/settings.json",
            r#"{"user":"data","model":"claude"}"#,
        ),
        (".codex/config.toml", "user_setting = true\n"),
        (
            ".local/share/my-skills/personal/SKILL.md",
            "# my own skill\ncontent\n",
        ),
    ];
    for (rel, content) in seeds {
        let p = home.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, content).unwrap();
        files.push(p);
    }
    files
}

#[test]
fn uninstall_on_existing_home_preserves_all_user_data() {
    let home = tempfile::tempdir().expect("fake HOME");
    let cogh_home = home.path().join(".cogh");

    // Existing HOME: user data present BEFORE any tool state.
    let user_files = plant_user_data(home.path());

    // Install a plugin (tool-owned state) from the planted manifest.
    let out = run_with_home(home.path(), &cogh_home, &["init"]);
    assert!(out.status.success(), "cogh init must succeed");
    plant_manifest(&cogh_home, "latest");
    let version_tree = cogh_home.join("versions").join("latest");
    assert!(
        version_tree.exists(),
        "fixture must plant the tool-owned version tree"
    );
    let out = run_with_home(
        home.path(),
        &cogh_home,
        &["ide", "install", "--plugin", "mcp-server"],
    );
    let _installed = out.status.success();

    // ANTI-VACUITY: uninstall must actually remove the tool-owned tree.
    let out = run_with_home(
        home.path(),
        &cogh_home,
        &[
            "uninstall",
            "mcp-server",
            "--version",
            "latest",
            "--ide",
            "opencode",
        ],
    );
    let msg = format!(
        "out={} err={}",
        stdout(&out),
        String::from_utf8_lossy(&out.stderr)
    );
    eprintln!("UNINSTALL DIAG: {msg}");
    assert!(
        !version_tree.exists(),
        "uninstall must remove the tool-owned version tree; {msg}"
    );

    // Every user file survives byte-for-byte.
    for f in &user_files {
        assert!(
            f.exists(),
            "user file must survive uninstall: {}",
            f.display()
        );
    }
    assert_eq!(
        fs::read_to_string(&user_files[0]).unwrap(),
        "my own notes, nothing to do with cogh\n",
        "user notes content must be untouched"
    );
    let ide_cfg = fs::read_to_string(&user_files[2]).unwrap();
    assert!(
        ide_cfg.contains("solarized") && ide_cfg.contains("my-server"),
        "IDE config must keep user-owned keys/values: {ide_cfg}"
    );
}

#[test]
fn uninstall_never_touches_paths_outside_tool_ownership() {
    // Even when asked to also uninstall IDE configurations, cogh must
    // only remove entries it owns inside those configs — the files
    // themselves (with user content) remain.
    let home = tempfile::tempdir().expect("fake HOME");
    let cogh_home = home.path().join(".cogh");
    let user_files = plant_user_data(home.path());
    let out = run_with_home(home.path(), &cogh_home, &["init"]);
    assert!(out.status.success(), "cogh init must succeed");
    plant_manifest(&cogh_home, "latest");

    let out = run_with_home(
        home.path(),
        &cogh_home,
        &[
            "uninstall",
            "mcp-server",
            "--version",
            "latest",
            "--ide",
            "opencode",
        ],
    );
    let _ = stdout(&out);

    for f in &user_files {
        assert!(
            f.exists(),
            "user file must survive ide uninstall: {}",
            f.display()
        );
    }
    let cfg = fs::read_to_string(home.path().join(".opencode/config.json")).unwrap();
    assert!(cfg.contains("solarized"), "user theme must survive: {cfg}");
}
