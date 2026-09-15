//! Integration tests for `cogh skill validate` — the portable skill
//! bundle validator defined in `crates/cognicode-cli/src/cmd/skill.rs`.
//!
//! Locks down the 4 of 7 contracts in
//! `openspec/specs/portable-skill-bundle/spec.md` that are exercisable
//! without network or `cogh install` (the remaining 2 — version coupling
//! to MCP server, requires-plugin cascading — are network-dependent and
//! belong to a future cycle that mocks the registry):
//!
//! - REQ #1 "Skill bundle is a portable directory tree" (loads + rejects
//!   non-directory).
//! - REQ #2 "`SKILL.md` has portable YAML frontmatter" (manifest required
//!   and parsed; maturity validated).
//! - REQ #3 "Portable bundle has NO IDE-specific fields" (rejects
//!   `compatibility: opencode`).
//! - REQ #4 "`manifest.yaml` declares cogh metadata" (parsed and printed).
//! - REQ #7 "`references/` and `assets/` are copied recursively"
//!   (referenced scripts must exist on disk).
//!
//! TDD contract: every test is RED before this commit, GREEN after.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Path to the `cogh` binary injected by Cargo at build time.
fn cogh() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_cogh"))
}

/// Run `cogh --home <home> skill validate <path>` and capture output.
fn run_validate(home: &Path, bundle: &Path) -> Output {
    Command::new(cogh())
        .arg("--home")
        .arg(home)
        .arg("skill")
        .arg("validate")
        .arg(bundle)
        .output()
        .expect("spawn cogh skill validate")
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

/// Write a portable skill bundle fixture into `dir`.
///
/// `manifest_yaml` is the body of `manifest.yaml` (no leading newline).
/// `skill_md` is the body of `SKILL.md` (the function takes care of the
/// `---` frontmatter delimiters if `skill_md` already includes them).
fn write_bundle(dir: &Path, manifest_yaml: &str, skill_md: &str) {
    fs::write(dir.join("manifest.yaml"), manifest_yaml).expect("write manifest.yaml");
    fs::write(dir.join("SKILL.md"), skill_md).expect("write SKILL.md");
}

/// Create a script file at `dir/<rel>` so that the validator's
/// `referenced script exists` check passes.
fn write_script(dir: &Path, rel: &str) -> PathBuf {
    let path = dir.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("mkdir script parent");
    }
    fs::write(&path, "#!/bin/sh\necho helper\n").expect("write script");
    path
}

/// A minimal valid manifest for the test fixtures.
const VALID_MANIFEST: &str = "\
apiVersion: cognicode/v1
kind: SkillBundle
name: test-skill
description: Test skill bundle for conformance coverage
version: 1.0.0
maturity: stable
homepage: https://example.com/test
authors:
  - CogniCode Team
requires: []
scripts:
  - references/helper.sh
assets: []
";

/// A minimal valid `SKILL.md` with portable frontmatter.
const VALID_SKILL_MD: &str = "\
---
name: test-skill
description: Test skill
license: MIT
metadata:
  version: \"1.0.0\"
  maturity: stable
---

# Test skill body
";

// ============================================================================
// REQ #1 "Skill bundle is a portable directory tree"
// ============================================================================

#[test]
fn portable_skill_bundle_valid_directory_passes_validation() {
    let home = tempfile::tempdir().expect("temp home");
    let bundle = tempfile::tempdir().expect("temp bundle");
    write_bundle(bundle.path(), VALID_MANIFEST, VALID_SKILL_MD);
    write_script(bundle.path(), "references/helper.sh");

    let out = run_validate(home.path(), bundle.path());
    assert!(
        out.status.success(),
        "valid bundle must exit 0; got {:?}\nstderr: {}",
        out.status,
        stderr(&out)
    );
    let body = stdout(&out);
    assert!(
        body.contains("✓ skill bundle valid"),
        "expected `✓ skill bundle valid` on stdout; got: {body}"
    );
    assert!(
        body.contains("test-skill"),
        "expected bundle name on stdout; got: {body}"
    );
    assert!(
        body.contains("v1.0.0") || body.contains("1.0.0"),
        "expected bundle version on stdout; got: {body}"
    );
}

#[test]
fn portable_skill_bundle_non_directory_path_is_rejected() {
    let home = tempfile::tempdir().expect("temp home");
    let bogus = home.path().join("does-not-exist");
    let out = run_validate(home.path(), &bogus);

    assert!(
        !out.status.success(),
        "non-directory path must exit non-zero; got {:?}",
        out.status
    );
    assert!(
        stderr(&out).contains("not a directory"),
        "expected stderr to flag `not a directory`; got: {}",
        stderr(&out)
    );
}

// ============================================================================
// REQ #4 "`manifest.yaml` declares cogh metadata"
// ============================================================================

#[test]
fn portable_skill_bundle_manifest_yaml_required_and_parsed() {
    let home = tempfile::tempdir().expect("temp home");
    let bundle = tempfile::tempdir().expect("temp bundle");
    write_bundle(bundle.path(), VALID_MANIFEST, VALID_SKILL_MD);
    write_script(bundle.path(), "references/helper.sh");

    let out = run_validate(home.path(), bundle.path());
    assert!(
        out.status.success(),
        "manifest validation must exit 0; got {:?}\nstderr: {}",
        out.status,
        stderr(&out)
    );
    let body = stdout(&out);
    assert!(
        body.contains("Test skill bundle for conformance coverage"),
        "expected description in stdout; got: {body}"
    );
    assert!(
        body.contains("stable"),
        "expected maturity `stable` in stdout; got: {body}"
    );
}

#[test]
fn portable_skill_bundle_missing_manifest_is_rejected() {
    let home = tempfile::tempdir().expect("temp home");
    let bundle = tempfile::tempdir().expect("temp bundle");
    // Only SKILL.md, no manifest.yaml.
    fs::write(bundle.path().join("SKILL.md"), VALID_SKILL_MD).expect("write SKILL.md");

    let out = run_validate(home.path(), bundle.path());
    assert!(
        !out.status.success(),
        "missing manifest.yaml must exit non-zero; got {:?}",
        out.status
    );
    assert!(
        stderr(&out).contains("missing manifest.yaml"),
        "expected stderr to flag `missing manifest.yaml`; got: {}",
        stderr(&out)
    );
}

// ============================================================================
// REQ #2 "`SKILL.md` has portable YAML frontmatter"
// ============================================================================

#[test]
fn portable_skill_bundle_missing_skill_md_is_rejected() {
    let home = tempfile::tempdir().expect("temp home");
    let bundle = tempfile::tempdir().expect("temp bundle");
    // Only manifest.yaml.
    fs::write(bundle.path().join("manifest.yaml"), VALID_MANIFEST).expect("write manifest.yaml");

    let out = run_validate(home.path(), bundle.path());
    assert!(
        !out.status.success(),
        "missing SKILL.md must exit non-zero; got {:?}",
        out.status
    );
    assert!(
        stderr(&out).contains("missing SKILL.md"),
        "expected stderr to flag `missing SKILL.md`; got: {}",
        stderr(&out)
    );
}

#[test]
fn portable_skill_bundle_invalid_maturity_is_rejected() {
    let home = tempfile::tempdir().expect("temp home");
    let bundle = tempfile::tempdir().expect("temp bundle");
    let manifest = "\
apiVersion: cognicode/v1
kind: SkillBundle
name: bad-maturity
description: Should be rejected
version: 1.0.0
maturity: bogus
";
    write_bundle(bundle.path(), manifest, VALID_SKILL_MD);

    let out = run_validate(home.path(), bundle.path());
    assert!(
        !out.status.success(),
        "invalid maturity must exit non-zero; got {:?}",
        out.status
    );
    assert!(
        stderr(&out).contains("maturity must be one of"),
        "expected stderr to flag maturity rule; got: {}",
        stderr(&out)
    );
}

// ============================================================================
// REQ #3 "Portable bundle has NO IDE-specific fields"
// ============================================================================

#[test]
fn portable_skill_bundle_ide_specific_compatibility_field_rejected() {
    let home = tempfile::tempdir().expect("temp home");
    let bundle = tempfile::tempdir().expect("temp bundle");
    let ide_locked_skill = "\
---
name: locked
description: IDE-locked skill (forbidden)
compatibility: opencode
---

# body
";
    write_bundle(bundle.path(), VALID_MANIFEST, ide_locked_skill);

    let out = run_validate(home.path(), bundle.path());
    assert!(
        !out.status.success(),
        "IDE-specific compatibility field must exit non-zero; got {:?}",
        out.status
    );
    assert!(
        stderr(&out).contains("IDE-specific 'compatibility: opencode'"),
        "expected stderr to flag IDE-specific compatibility; got: {}",
        stderr(&out)
    );
}

// ============================================================================
// REQ #7 "`references/` and `assets/` are copied recursively"
// ============================================================================

#[test]
fn portable_skill_bundle_referenced_script_must_exist() {
    let home = tempfile::tempdir().expect("temp home");
    let bundle = tempfile::tempdir().expect("temp bundle");
    let manifest_no_scripts = "\
apiVersion: cognicode/v1
kind: SkillBundle
name: ghost-script
description: Manifest references a script that does not exist on disk
version: 1.0.0
maturity: stable
scripts:
  - references/ghost.sh
assets: []
";
    write_bundle(bundle.path(), manifest_no_scripts, VALID_SKILL_MD);
    // Note: write_script is intentionally NOT called — the test asserts
    // that the validator catches the dangling reference.

    let out = run_validate(home.path(), bundle.path());
    assert!(
        !out.status.success(),
        "missing referenced script must exit non-zero; got {:?}",
        out.status
    );
    assert!(
        stderr(&out).contains("referenced script missing"),
        "expected stderr to flag missing referenced script; got: {}",
        stderr(&out)
    );
}
