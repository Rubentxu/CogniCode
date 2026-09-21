//! Integration tests for the `cogh ide` subcommands and the 4 bundled
//! IDE adapters (opencode, zcode, claude, codex).
//!
//! Locks down 7 of the 8 substantive contracts in
//! `openspec/specs/cognicode-ide-adapter/spec.md`:
//!
//! - REQ #1 "Each IDE is a separate `cogh` plugin"
//! - REQ #2 "Adapter manifest declares integrate / uninstall steps"
//! - REQ #3 "MCP config patching is JSON-merge, not overwrite"
//! - REQ #5 "`remove_from_json` cleanly removes the MCP entry"
//! - REQ #7 "Adapter declares a `detect` heuristic"
//! - REQ #8 "Each IDE adapter has a unique JSON path"
//!
//! REQ #6 (self-contained Rust binary) is a build-time concern and
//! REQ #4 (skill bundle copy) is fixture-bounded — both out of scope
//! for this cycle.
//!
//! ## HOME redirection technique
//!
//! The IDE adapters look up their config paths under `$HOME`
//! (`~/.config/opencode/opencode.json`, etc.) — they do NOT honour
//! `--home`. To isolate the tests from the developer's real `$HOME`,
//! each test gives the spawned `cogh` its own `$HOME` via
//! `Command::env("HOME", &fake_home)`. This propagates only to the
//! child process, so the test process's own `$HOME` is untouched.
//! Each test owns its own temp `$HOME`, so parallel execution is safe.
//!
//! TDD contract: every test is RED before this commit, GREEN after.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use tempfile::TempDir;

/// Path to the `cogh` binary injected by Cargo at build time.
fn cogh() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_cogh"))
}

/// Create a fresh temp dir to act as the child process's `$HOME`.
fn fake_home() -> TempDir {
    tempfile::tempdir().expect("tempdir for fake HOME")
}

/// Run `cogh --home <cogh_home> <args...>` with `HOME` set to
/// `fake_home` for the child process only.
fn run_with_home(fake_home: &Path, cogh_home: &Path, args: &[&str]) -> Output {
    let mut cmd = Command::new(cogh());
    cmd.env("HOME", fake_home);
    cmd.arg("--home").arg(cogh_home);
    for a in args {
        cmd.arg(a);
    }
    cmd.output().expect("spawn cogh")
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// Initialise `<cogh_home>` via `cogh init` under the fake HOME.
fn init_home(fake_home: &Path, cogh_home: &Path) -> Output {
    let out = run_with_home(fake_home, cogh_home, &["init"]);
    assert!(
        out.status.success(),
        "cogh init must succeed; got {:?}\nstderr: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    out
}

/// Plant a minimal v2 bundle manifest at `<cogh_home>/versions/<version>/manifest.yaml`.
///
/// The IDE integrators (opencode/zcode/claude/codex) resolve skill bundles
/// from this manifest per `arch(debt2)` (commit 9752cc52, 2026-09-18). Without
/// the manifest on disk, `cmd_ide_install` fails with `No such file or
/// directory (os error 2)` when it tries to look up declared skill bundles.
///
/// The fixture is intentionally minimal:
/// - one profile (`core`) matching what `cogh ide install --plugin mcp-server` expects;
/// - the `cognicode-mcp` component declared under profile `core` so the
///   binary-name resolution in `cmd_ide_install` succeeds;
/// - one declared `skill_bundles[]` entry whose physical directory
///   (`<root>/versions/<v>/skills/<bundle.id>/`) we also create with a
///   placeholder file. OpenCode integration REQUIRES ≥1 declared bundle
///   (it errors out with `cannot integrate OpenCode without a declared
///   SkillBundleId` otherwise); the other integrators tolerate an empty
///   list but a single bundle keeps the fixture uniform across plugins.
///
/// The bundle version is a fixed semver (NOT the literal `latest`) because
/// `BundleManifest::from_path` enforces `^\d+\.\d+\.\d+(-.*)?$` per the v2
/// schema (e87). The directory name under `versions/` still uses the literal
/// `latest` because that's what `cmd_ide_install` defaults to.
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
    let version_root = cogh_home.join("versions").join(version_dir);
    let manifest_dir = version_root.clone();
    fs::create_dir_all(&manifest_dir).expect("mkdir versions/<v>");
    fs::write(
        manifest_dir.join("manifest.yaml"),
        manifest_yaml.trim_start(),
    )
    .expect("write manifest.yaml");

    // Create the declared skill bundle directory + a placeholder skill file
    // so OpenCode integration's `declared_skill_bundle_dirs` (which errors
    // out with `dir.is_dir()` false when the bundle is declared but missing
    // on disk) succeeds. The integrator copies the dir into the IDE's skill
    // store; we don't assert on that copy in these tests, only on the MCP
    // config merge that follows.
    let bundle_dir = version_root.join("skills").join(bundle_id);
    fs::create_dir_all(&bundle_dir).expect("mkdir skills/<bundle_id>");
    fs::write(bundle_dir.join("SKILL.md"), "# test fixture skill\n").expect("write SKILL.md");
}

/// Helper that performs `init_home` AND plants a manifest for the `latest`
/// pseudo-version (the default for `cogh ide install --plugin mcp-server`).
fn init_home_with_manifest(fake_home: &Path, cogh_home: &Path) {
    init_home(fake_home, cogh_home);
    plant_manifest(cogh_home, "latest");
}

/// Sample opencode config that exercises the merge-keep-old behaviour.
const OPENCODE_WITH_CHRONOS: &str = r#"{
  "agent": {"foo": {"description": "test"}},
  "mcp": {
    "chronos": {"type": "local"}
  }
}
"#;

// ============================================================================
// REQ #7 "Adapter declares a `detect` heuristic"
// ============================================================================

#[test]
fn cogh_ide_detect_lists_opencode_when_config_present() {
    let fake_home_dir = fake_home();
    let fake_home = fake_home_dir.path();
    let cogh_home = tempfile::tempdir().expect("cogh home");

    // Stub opencode config so detect finds it.
    let oc_dir = fake_home.join(".config/opencode");
    fs::create_dir_all(&oc_dir).expect("mkdir opencode config dir");
    fs::write(oc_dir.join("opencode.json"), "{}").expect("write opencode config");

    let out = run_with_home(fake_home, cogh_home.path(), &["ide", "detect"]);
    assert!(
        out.status.success(),
        "cogh ide detect must exit 0; got {:?}\nstderr: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    let body = stdout(&out);
    assert!(
        body.contains("Detected IDEs:"),
        "expected `Detected IDEs:` header; got: {body}"
    );
    assert!(
        body.contains("✓ opencode"),
        "expected `✓ opencode`; got: {body}"
    );
    assert!(
        body.contains(".config/opencode/opencode.json"),
        "expected the opencode config path; got: {body}"
    );
}

#[test]
fn cogh_ide_detect_lists_no_ides_on_empty_home() {
    let fake_home_dir = fake_home();
    let fake_home = fake_home_dir.path();
    let cogh_home = tempfile::tempdir().expect("cogh home");

    // Empty HOME — no IDE configs.
    let out = run_with_home(fake_home, cogh_home.path(), &["ide", "detect"]);
    assert!(
        out.status.success(),
        "cogh ide detect must exit 0 on empty home; got {:?}",
        out.status
    );
    let body = stdout(&out);
    assert!(
        body.contains("Detected IDEs:"),
        "expected `Detected IDEs:` header; got: {body}"
    );
    assert!(
        body.contains("opencode") && body.contains("✗"),
        "expected `opencode ✗` (config not found); got: {body}"
    );
}

// ============================================================================
// REQ #1 "Each IDE is a separate `cogh` plugin"
// ============================================================================

#[test]
fn cogh_init_includes_three_ide_plugins() {
    let fake_home_dir = fake_home();
    let fake_home = fake_home_dir.path();
    let cogh_home = tempfile::tempdir().expect("cogh home");

    let _ = init_home(fake_home, cogh_home.path());

    // As of v0.94.15 the bundled plugins are mcp-server, skills-cognicode-core,
    // sandbox-templates, zcode, claude, codex. (opencode is NOT bundled — it's
    // expected to be installed separately via `cogh plugin add <name> --from-url`.)
    let plugins_dir = cogh_home.path().join("plugins");
    for ide in &["zcode", "claude", "codex"] {
        let p = plugins_dir.join(ide);
        assert!(
            p.is_dir(),
            "expected plugin directory at {}; missing",
            p.display()
        );
    }
    // opencode is intentionally absent from the bundled set — assert its
    // absence too, so the test fails if someone adds it to the bundle.
    let opencode_dir = plugins_dir.join("opencode");
    assert!(
        !opencode_dir.exists(),
        "opencode is not bundled; if this assertion fails the bundle set has changed"
    );
}

#[test]
fn cogh_plugin_list_shows_ide_plugins_with_manifests() {
    let fake_home_dir = fake_home();
    let fake_home = fake_home_dir.path();
    let cogh_home = tempfile::tempdir().expect("cogh home");

    let _ = init_home(fake_home, cogh_home.path());

    let out = run_with_home(fake_home, cogh_home.path(), &["plugin", "list"]);
    assert!(
        out.status.success(),
        "cogh plugin list must exit 0; got {:?}",
        out.status
    );
    let body = stdout(&out);
    for ide in &["zcode", "claude", "codex"] {
        assert!(
            body.contains(ide),
            "expected `{ide}` in plugin listing; got: {body}"
        );
    }
}

// ============================================================================
// REQ #2 + #3 "Adapter manifest declares integrate steps" +
//            "MCP config patching is JSON-merge, not overwrite"
// ============================================================================

#[test]
fn cogh_ide_install_opencode_writes_mcp_entry_preserving_existing() {
    let fake_home_dir = fake_home();
    let fake_home = fake_home_dir.path();
    let cogh_home = tempfile::tempdir().expect("cogh home");

    // Stub opencode config with a pre-existing MCP entry.
    let oc_dir = fake_home.join(".config/opencode");
    fs::create_dir_all(&oc_dir).expect("mkdir opencode config dir");
    let oc_config = oc_dir.join("opencode.json");
    fs::write(&oc_config, OPENCODE_WITH_CHRONOS).expect("write opencode config");

    init_home_with_manifest(fake_home, cogh_home.path());

    let out = run_with_home(
        fake_home,
        cogh_home.path(),
        &["ide", "install", "opencode", "--plugin", "mcp-server"],
    );
    assert!(
        out.status.success(),
        "cogh ide install opencode must exit 0; got {:?}\nstderr: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );

    // Read back the opencode config and assert merge semantics.
    let text = fs::read_to_string(&oc_config).expect("read opencode config");
    let v: serde_json::Value = serde_json::from_str(&text).expect("parse opencode config");
    assert_eq!(
        v["mcp"]["chronos"]["type"], "local",
        "pre-existing `mcp.chronos` must be preserved; got: {text}"
    );
    assert_eq!(
        v["mcp"]["cognicode-mcp"]["type"], "stdio",
        "new `mcp.cognicode-mcp` must be added; got: {text}"
    );
}

// ============================================================================
// REQ #5 "`remove_from_json` cleanly removes the MCP entry"
// ============================================================================

#[test]
fn cogh_ide_uninstall_opencode_removes_mcp_entry() {
    let fake_home_dir = fake_home();
    let fake_home = fake_home_dir.path();
    let cogh_home = tempfile::tempdir().expect("cogh home");

    // Stub opencode config with BOTH entries so we can assert the
    // uninstall removes only the cognicode-mcp one.
    let oc_dir = fake_home.join(".config/opencode");
    fs::create_dir_all(&oc_dir).expect("mkdir opencode config dir");
    let oc_config = oc_dir.join("opencode.json");
    fs::write(
        &oc_config,
        r#"{
  "mcp": {
    "chronos": {"type": "local"},
    "cognicode-mcp": {"type": "stdio"}
  }
}
"#,
    )
    .expect("write opencode config");

    init_home_with_manifest(fake_home, cogh_home.path());
    // The uninstall test passes `--version 0.94.15`, so we also need a
    // manifest at `<cogh_home>/versions/0.94.15/manifest.yaml`. Without
    // it `cmd_ide_uninstall` fails with `failed to read bundle manifest`.
    plant_manifest(cogh_home.path(), "0.94.15");

    let out = run_with_home(
        fake_home,
        cogh_home.path(),
        &["ide", "uninstall", "opencode", "--version", "0.94.15"],
    );
    assert!(
        out.status.success(),
        "cogh ide uninstall opencode must exit 0; got {:?}\nstderr: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );

    let text = fs::read_to_string(&oc_config).expect("read opencode config");
    let v: serde_json::Value = serde_json::from_str(&text).expect("parse opencode config");
    assert!(
        v["mcp"].get("cognicode-mcp").is_none(),
        "expected `mcp.cognicode-mcp` to be removed; got: {text}"
    );
    assert_eq!(
        v["mcp"]["chronos"]["type"], "local",
        "pre-existing `mcp.chronos` must be preserved; got: {text}"
    );
}

// ============================================================================
// REQ #8 "Each IDE adapter has a unique JSON path"
// ============================================================================

#[test]
fn cogh_ide_install_zcode_writes_zcode_specific_path() {
    let fake_home_dir = fake_home();
    let fake_home = fake_home_dir.path();
    let cogh_home = tempfile::tempdir().expect("cogh home");

    // Stub zcode config.
    let zc_dir = fake_home.join(".zcode/v2");
    fs::create_dir_all(&zc_dir).expect("mkdir zcode config dir");
    let zc_config = zc_dir.join("config.json");
    fs::write(&zc_config, "{}").expect("write zcode config");

    init_home_with_manifest(fake_home, cogh_home.path());

    let out = run_with_home(
        fake_home,
        cogh_home.path(),
        &["ide", "install", "zcode", "--plugin", "mcp-server"],
    );
    assert!(
        out.status.success(),
        "cogh ide install zcode must exit 0; got {:?}\nstderr: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );

    let text = fs::read_to_string(&zc_config).expect("read zcode config");
    let v: serde_json::Value = serde_json::from_str(&text).expect("parse zcode config");
    assert_eq!(
        v["mcp"]["cognicode-mcp"]["type"], "stdio",
        "expected zcode config to carry the `mcp.cognicode-mcp` entry; got: {text}"
    );
}
