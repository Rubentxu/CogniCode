//! Integration tests for `cogh plugin <subcommand>` and `cogh init` —
//! the plugin manager defined in `crates/cognicode-cli/src/cmd/layout.rs`
//! and the bundled-plugin installer reached via `cmd_init`.
//!
//! Locks down 2 of 4 contracts in `openspec/specs/cognicode-plugin/spec.md`
//! that are exercisable without network or `cogh install`:
//!
//! - REQ #1 "`plugin.yaml` is the canonical manifest format"
//!   - parses end-to-end (`plugin list` reads the description),
//!   - missing manifest falls back gracefully,
//!   - `cogh plugin add` rejects unknown non-bundled names.
//!
//! - REQ #4 "Bundled plugins ship with cogh"
//!   - `cogh init` installs the bundled plugins,
//!   - `cogh plugin list` enumerates them.
//!
//! The remaining 2 REQs (#2 versions addressable by ref, #3 sha256 mandatory)
//! are covered by the 4 unit tests in `crates/cognicode-cli/src/cmd/manifest.rs`
//! for the parser side; the end-to-end install + verify path requires
//! `cogh install` which is network-dependent.
//!
//! TDD contract: every test is RED before this commit, GREEN after.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Path to the `cogh` binary injected by Cargo at build time.
fn cogh() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_cogh"))
}

/// Run `cogh --home <home> <args...>` and capture output.
fn run_cogh<I, S>(home: &Path, args: I) -> Output
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    Command::new(cogh())
        .arg("--home")
        .arg(home)
        .args(args)
        .output()
        .expect("spawn cogh binary")
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

/// Drop a `plugin.yaml` into `<home>/plugins/<name>/`.
fn write_plugin(home: &Path, name: &str, yaml: &str) -> PathBuf {
    let dir = home.join("plugins").join(name);
    fs::create_dir_all(&dir).expect("mkdir plugin dir");
    fs::write(dir.join("plugin.yaml"), yaml).expect("write plugin.yaml");
    dir
}

/// A minimal valid plugin manifest for the custom-plugin fixture.
const SAMPLE_PLUGIN_YAML: &str = "\
apiVersion: cognicode/v1
kind: Plugin
name: my-test-plugin
description: A test plugin added manually for conformance
homepage: https://example.com/my-test-plugin
repository: https://github.com/example/my-test-plugin
versions:
  - ref: \"1.0.0\"
    artifact: my-test-plugin-1.0.0.tar.gz
    sha256: 0000000000000000000000000000000000000000000000000000000000000000
binaries:
  - name: my-test-binary
    path: bin/my-test-binary
    description: Test binary
";

// ============================================================================
// REQ #4 "Bundled plugins ship with cogh"
// ============================================================================

#[test]
fn cogh_init_installs_bundled_plugins() {
    let home = tempfile::tempdir().expect("temp home");
    let out = run_cogh(home.path(), ["init"]);

    assert!(
        out.status.success(),
        "cogh init must exit 0; got {:?}\nstderr: {}",
        out.status,
        stderr(&out)
    );
    let body = stdout(&out);
    assert!(
        body.contains("Installed "),
        "expected `Installed ` summary line; got: {body}"
    );
    assert!(
        body.contains("bundled plugin(s)"),
        "expected `bundled plugin(s)` summary; got: {body}"
    );

    // The plugins directory must be populated.
    let plugins = home.path().join("plugins");
    let entries: Vec<_> = fs::read_dir(&plugins)
        .expect("read plugins dir")
        .flatten()
        .collect();
    assert!(
        !entries.is_empty(),
        "expected at least one bundled plugin directory under {}; got 0 entries",
        plugins.display()
    );
}

#[test]
fn cogh_plugin_list_enumerates_bundled_plugins_after_init() {
    let home = tempfile::tempdir().expect("temp home");
    let init = run_cogh(home.path(), ["init"]);
    assert!(
        init.status.success(),
        "cogh init must succeed before listing; got {:?}\nstderr: {}",
        init.status,
        stderr(&init)
    );

    let out = run_cogh(home.path(), ["plugin", "list"]);
    assert!(
        out.status.success(),
        "cogh plugin list must exit 0; got {:?}\nstderr: {}",
        out.status,
        stderr(&out)
    );
    let body = stdout(&out);
    assert!(
        body.contains("Plugin"),
        "expected table header `Plugin`; got: {body}"
    );
    assert!(
        body.contains("mcp-server"),
        "expected canonical bundled plugin `mcp-server`; got: {body}"
    );
}

// ============================================================================
// REQ #1 "`plugin.yaml` is the canonical manifest format"
// ============================================================================

#[test]
fn cogh_plugin_list_shows_custom_plugin_with_parsed_description() {
    let home = tempfile::tempdir().expect("temp home");
    let init = run_cogh(home.path(), ["init"]);
    assert!(
        init.status.success(),
        "cogh init must succeed; got {:?}\nstderr: {}",
        init.status,
        stderr(&init)
    );
    write_plugin(home.path(), "my-test-plugin", SAMPLE_PLUGIN_YAML);

    let out = run_cogh(home.path(), ["plugin", "list"]);
    assert!(
        out.status.success(),
        "cogh plugin list must exit 0; got {:?}\nstderr: {}",
        out.status,
        stderr(&out)
    );
    let body = stdout(&out);
    assert!(
        body.contains("my-test-plugin"),
        "expected custom plugin name in listing; got: {body}"
    );
    assert!(
        body.contains("A test plugin added manually for conformance"),
        "expected parsed description in listing; got: {body}"
    );
}

#[test]
fn cogh_plugin_list_handles_plugin_without_yaml_gracefully() {
    let home = tempfile::tempdir().expect("temp home");
    let init = run_cogh(home.path(), ["init"]);
    assert!(
        init.status.success(),
        "cogh init must succeed; got {:?}\nstderr: {}",
        init.status,
        stderr(&init)
    );
    // Create a plugin directory with NO manifest — the listing should
    // not abort; it should print the name with an empty description.
    fs::create_dir_all(home.path().join("plugins").join("empty-plugin"))
        .expect("mkdir empty plugin dir");

    let out = run_cogh(home.path(), ["plugin", "list"]);
    assert!(
        out.status.success(),
        "cogh plugin list must NOT abort on missing manifest; got {:?}\nstderr: {}",
        out.status,
        stderr(&out)
    );
    let body = stdout(&out);
    assert!(
        body.contains("empty-plugin"),
        "expected `empty-plugin` in the listing despite missing manifest; got: {body}"
    );
}

#[test]
fn cogh_plugin_add_unknown_name_without_url_is_rejected() {
    let home = tempfile::tempdir().expect("temp home");
    let init = run_cogh(home.path(), ["init"]);
    assert!(
        init.status.success(),
        "cogh init must succeed; got {:?}\nstderr: {}",
        init.status,
        stderr(&init)
    );

    let out = run_cogh(home.path(), ["plugin", "add", "not-a-real-plugin"]);
    assert!(
        !out.status.success(),
        "cogh plugin add for unknown non-bundled name must exit non-zero; got {:?}",
        out.status
    );
    let combined = format!("{}{}", stdout(&out), stderr(&out));
    assert!(
        combined.contains("not bundled") || combined.contains("--from-url"),
        "expected stderr to mention `not bundled` or `--from-url`; got: {combined}"
    );
}
