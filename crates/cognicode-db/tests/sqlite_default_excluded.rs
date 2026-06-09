//! TDD RED-gate test: confirm that the workspace's `cognicode-db`
//! crate, when built with `--no-default-features`, does NOT pull in
//! `rusqlite` in its dependency graph.
//!
//! This is a meta-test that invokes `cargo tree` at test time. The
//! gate is the spec's RED-gate #1 for `sqlite-feature-gate`:
//! "cargo tree | grep -q rusqlite exits 1" when the feature is off.
//!
//! In PR 1 (sqlite still default), this test asserts the GATE is
//! installed correctly: building the crate with `--no-default-features`
//! must exclude rusqlite. PR 2 will then remove `sqlite` from the
//! default list and this test (without the cfg gate) will be the
//! permanent regression guard for the default build.

use std::process::Command;

#[test]
fn cognicode_db_no_default_features_excludes_rusqlite() {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .ancestors()
        .nth(2) // crates/.. — workspace root
        .expect("must be inside a workspace")
        .to_path_buf();

    let output = Command::new("cargo")
        .current_dir(&workspace_root)
        .args(["tree", "-p", "cognicode-db", "--no-default-features"])
        .output()
        .expect("failed to invoke `cargo tree` — is cargo on PATH?");

    assert!(
        output.status.success(),
        "`cargo tree` failed: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);

    // The dep tree has lines like:
    //     cognicode-db v0.5.0 (...)
    //     ├── anyhow v1.0.102
    //     ├── rusqlite v0.31.0      <-- we want to assert this is absent
    //
    // We check for the dep-line prefix `├── rusqlite` / `└── rusqlite`
    // to avoid false positives on the package's own name appearing
    // in the `cargo tree` header.
    let mut found_rusqlite = false;
    for line in stdout.lines() {
        let trimmed = line.trim_start();
        // dep-tree prefixes: "├── ", "└── ", "│   ├── ", "│   └── "
        if (trimmed.starts_with("├─ rusqlite")
            || trimmed.starts_with("└─ rusqlite")
            || trimmed.starts_with("├── rusqlite")
            || trimmed.starts_with("└── rusqlite"))
            && !line.contains("(")
        // (i.e. it's a dep name, not a "(proc-macro)" annotation)
        {
            found_rusqlite = true;
            break;
        }
    }

    assert!(
        !found_rusqlite,
        "FAIL: `cargo tree -p cognicode-db --no-default-features` still includes \
         `rusqlite` in the dep graph. The `sqlite` feature gate is not isolating the dep. \
         Tree:\n{}",
        stdout
    );
}
