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
//! each test writes a tiny shell wrapper at setup time:
//!
//! ```text
//! #!/bin/sh
//! export HOME="$1"; shift; exec "$@"
//! ```
//!
//! The test then invokes the wrapper with the tempdir as `$1` and
//! `cogh` plus its arguments as the rest. This redirect propagates
//! only to the subprocess tree, so the test process's own `$HOME`
//! is untouched. Each test owns its own wrapper + tempdir, so
//! parallel execution is safe.
//!
//! TDD contract: every test is RED before this commit, GREEN after.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

/// Path to the `cogh` binary injected by Cargo at build time.
fn cogh() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_cogh"))
}

/// Build a tiny shell wrapper that sets `HOME=$1` then `exec`s the
/// remaining args. Returns the wrapper's path (lives in a tempdir
/// that is cleaned up at the end of the test).
fn home_wrapper() -> (TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir for wrapper");
    let path = dir.path().join("with-home.sh");
    let mut f = fs::File::create(&path).expect("create wrapper");
    writeln!(
        f,
        "#!/bin/sh\nexport HOME=\"$1\"; shift; exec \"$@\"\n"
    )
    .expect("write wrapper");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&path).expect("stat wrapper").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).expect("chmod wrapper");
    }
    (dir, path)
}

/// Run `cogh --home <cogh_home> <args...>` under the wrapper with the
/// given `home` redirected to `fake_home`.
fn run_with_home(
    wrapper: &Path,
    fake_home: &Path,
    cogh_home: &Path,
    args: &[&str],
) -> Output {
    let mut cmd = Command::new(wrapper);
    cmd.arg(fake_home).arg(cogh());
    cmd.arg("--home").arg(cogh_home);
    for a in args {
        cmd.arg(a);
    }
    cmd.output().expect("spawn cogh via wrapper")
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// Initialise `<cogh_home>` via `cogh init` under the fake HOME.
fn init_home(wrapper: &Path, fake_home: &Path, cogh_home: &Path) -> Output {
    let out = run_with_home(wrapper, fake_home, cogh_home, &["init"]);
    assert!(
        out.status.success(),
        "cogh init must succeed; got {:?}\nstderr: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    out
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
    let (wrapper_dir, wrapper) = home_wrapper();
    let fake_home = wrapper_dir.path();
    let cogh_home = tempfile::tempdir().expect("cogh home");

    // Stub opencode config so detect finds it.
    let oc_dir = fake_home.join(".config/opencode");
    fs::create_dir_all(&oc_dir).expect("mkdir opencode config dir");
    fs::write(oc_dir.join("opencode.json"), "{}").expect("write opencode config");

    let out = run_with_home(&wrapper, fake_home, cogh_home.path(), &["ide", "detect"]);
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
    let (wrapper_dir, wrapper) = home_wrapper();
    let fake_home = wrapper_dir.path();
    let cogh_home = tempfile::tempdir().expect("cogh home");

    // Empty HOME — no IDE configs.
    let out = run_with_home(&wrapper, fake_home, cogh_home.path(), &["ide", "detect"]);
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
    let (wrapper_dir, wrapper) = home_wrapper();
    let fake_home = wrapper_dir.path();
    let cogh_home = tempfile::tempdir().expect("cogh home");

    let _ = init_home(&wrapper, fake_home, cogh_home.path());

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
    let (wrapper_dir, wrapper) = home_wrapper();
    let fake_home = wrapper_dir.path();
    let cogh_home = tempfile::tempdir().expect("cogh home");

    let _ = init_home(&wrapper, fake_home, cogh_home.path());

    let out = run_with_home(
        &wrapper,
        fake_home,
        cogh_home.path(),
        &["plugin", "list"],
    );
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
    let (wrapper_dir, wrapper) = home_wrapper();
    let fake_home = wrapper_dir.path();
    let cogh_home = tempfile::tempdir().expect("cogh home");

    // Stub opencode config with a pre-existing MCP entry.
    let oc_dir = fake_home.join(".config/opencode");
    fs::create_dir_all(&oc_dir).expect("mkdir opencode config dir");
    let oc_config = oc_dir.join("opencode.json");
    fs::write(&oc_config, OPENCODE_WITH_CHRONOS).expect("write opencode config");

    let _ = init_home(&wrapper, fake_home, cogh_home.path());

    let out = run_with_home(
        &wrapper,
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
    let (wrapper_dir, wrapper) = home_wrapper();
    let fake_home = wrapper_dir.path();
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

    let _ = init_home(&wrapper, fake_home, cogh_home.path());

    let out = run_with_home(
        &wrapper,
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
    let (wrapper_dir, wrapper) = home_wrapper();
    let fake_home = wrapper_dir.path();
    let cogh_home = tempfile::tempdir().expect("cogh home");

    // Stub zcode config.
    let zc_dir = fake_home.join(".zcode/v2");
    fs::create_dir_all(&zc_dir).expect("mkdir zcode config dir");
    let zc_config = zc_dir.join("config.json");
    fs::write(&zc_config, "{}").expect("write zcode config");

    let _ = init_home(&wrapper, fake_home, cogh_home.path());

    let out = run_with_home(
        &wrapper,
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
