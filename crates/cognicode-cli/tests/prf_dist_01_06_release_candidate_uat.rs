//! PRF-DIST-01 / PRF-DIST-06 UAT (real binaries).
//!
//! DIST-01: the release factory produces a canonical manifest (artifact,
//! version, platform, SHA256, composition) from real payloads built from the
//! real release binaries, and Layer 0/1 separation holds (cogh vs runtime
//! components are distinct published components).
//!
//! DIST-06: hashes, inventory and build provenance agree (source_commit ==
//! current HEAD), and the artifacts verify from the release candidate
//! directory itself, not a checkout. Tampering is rejected.

use std::path::PathBuf;
use std::process::Command;

fn release_bin() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop();
    p.pop();
    p.push("target/release/cognicode-release");
    assert!(
        p.exists(),
        "cognicode-release binary missing; build release first"
    );
    p
}

const PLATFORM: &str = "x86_64-unknown-linux-gnu";

// Hermeticity (CR-00c): version and tag are derived from the workspace
// `Cargo.toml` instead of being hardcoded to a historical value. This makes
// the test invariant to version bumps and removes the dependency on a
// pre-existing `dist/` populated by a previous release run.
fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn workspace_version() -> String {
    let manifest = std::fs::read_to_string(workspace_root().join("Cargo.toml"))
        .expect("workspace Cargo.toml must be readable from the clone");
    // Find the [workspace.package] version line. The crate version may also
    // live under a bare [workspace] (older layouts), so we accept both.
    let mut section: Option<&str> = None;
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            section = Some(trimmed);
            continue;
        }
        let in_workspace = matches!(
            section,
            Some("[workspace]") | Some("[workspace.package]")
        );
        if in_workspace {
            if let Some(rest) = trimmed.strip_prefix("version") {
                let rest = rest.trim_start().strip_prefix('=').unwrap_or(rest);
                let rest = rest.trim();
                let stripped = rest.trim_matches('"');
                if !stripped.is_empty() {
                    return stripped.to_string();
                }
            }
        }
    }
    panic!("could not find workspace version in Cargo.toml");
}

fn workspace_tag() -> String {
    format!("v{}", workspace_version())
}

fn head_commit() -> String {
    let out = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("git rev-parse");
    assert!(out.status.success());
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

#[test]
fn dist_release_candidate_generates_verifies_and_detects_tampering() {
    // CR-00c: hermeticity. The test must not depend on any artefact that
    // lives outside the workspace root or that is not versioned in Git.
    // - VERSION/TAG are derived from the workspace Cargo.toml.
    // - Skill bundles are generated inside `stage/` directly from
    //   `skills/<name>/` (which IS versioned), so the test no longer
    //   reads from `dist/` (which is build output, not a Git artefact).
    // - All temporary state is created under std::env::temp_dir() and
    //   cleaned up at the end of the test.
    let root = workspace_root();
    let version = workspace_version();
    let tag = workspace_tag();

    let stage = std::env::temp_dir().join(format!("prf-dist-uat-stage-{}", std::process::id()));
    let generated = std::env::temp_dir().join(format!("prf-dist-uat-gen-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&stage);
    let _ = std::fs::remove_dir_all(&generated);
    std::fs::create_dir_all(&stage).unwrap();

    // 1. Stage real payloads from target/release (real binaries, not fixtures).
    for stem in ["cogh", "cognicode", "cognicode-mcp"] {
        let src = root.join(format!("target/release/{stem}"));
        assert!(src.exists(), "missing release binary {stem}");
        let dst = stage.join(format!("{stem}-{version}-{PLATFORM}.tar.gz"));
        let st = Command::new("tar")
            .arg("-czf")
            .arg(&dst)
            .arg("-C")
            .arg(root.join("target/release"))
            .arg(stem)
            .status()
            .unwrap();
        assert!(st.success(), "tar failed for {stem}");
    }
    // 2. Skill bundle payloads. Generated here directly from `skills/<name>/`
    //    into `stage/`, so the test is independent of any pre-existing
    //    `dist/` directory and of any release flow having been run before.
    //    The bundle contents must include `manifest.yaml` (mirrors
    //    `just bundle-skills`).
    for bundle in ["cognicode", "cognicode-mcp"] {
        let skill_dir = root.join("skills").join(bundle);
        assert!(
            skill_dir.join("manifest.yaml").exists(),
            "skill `{}` must have a versioned manifest.yaml at {}",
            bundle,
            skill_dir.display()
        );
        let dst = stage.join(format!("{bundle}-{version}.tar.gz"));
        let st = Command::new("tar")
            .arg("-czf")
            .arg(&dst)
            .arg("-C")
            .arg(&skill_dir)
            .arg(".")
            .status()
            .unwrap();
        assert!(st.success(), "tar failed for skill bundle {bundle}");
    }

    // 3. Generate the release candidate from those bytes.
    let out = Command::new(release_bin())
        .args([
            "generate",
            "--staging",
            stage.to_str().unwrap(),
            "--out",
            generated.to_str().unwrap(),
            "--version",
            &version,
            "--tag",
            &tag,
            "--source-commit",
            &head_commit(),
            "--platform",
            PLATFORM,
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "generate failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // 4. DIST-06: inventory provenance matches current HEAD.
    let inventory =
        std::fs::read_to_string(generated.join(format!("release-inventory-{version}.json")))
            .unwrap();
    assert!(
        inventory.contains(&head_commit()),
        "inventory must record current HEAD as source_commit"
    );

    // 5. DIST-01/06: verification from the release candidate dir passes
    //    (hashes + manifest + composition agree). We pass `--version`
    //    explicitly so the verify call is invariant to whatever default
    //    version the `cognicode-release` binary was compiled with; this
    //    keeps the test hermetic even if the local `target/release/`
    //    contains a stale binary from a prior workspace version.
    let ok = Command::new(release_bin())
        .args([
            "verify",
            "--staging",
            generated.to_str().unwrap(),
            "--version",
            &version,
            "--tag",
            &tag,
            "--platform",
            PLATFORM,
        ])
        .status()
        .unwrap();
    assert!(ok.success(), "verify must PASS on the clean candidate");

    // 6. DIST-06 negative case: a tampered payload is rejected (exit != 0).
    let tampered = std::env::temp_dir().join(format!("prf-dist-uat-tam-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tampered);
    std::fs::create_dir_all(&tampered).unwrap();
    for entry in std::fs::read_dir(&generated).unwrap() {
        let e = entry.unwrap();
        let dst = tampered.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &dst);
        } else {
            std::fs::copy(e.path(), &dst).unwrap();
        }
    }
    let victim = tampered.join(format!("cognicode-{version}-{PLATFORM}.tar.gz"));
    let mut bytes = std::fs::read(&victim).unwrap();
    bytes.extend_from_slice(b"tampered");
    std::fs::write(&victim, &bytes).unwrap();

    let bad = Command::new(release_bin())
        .args([
            "verify",
            "--staging",
            tampered.to_str().unwrap(),
            "--version",
            &version,
            "--tag",
            &tag,
            "--platform",
            PLATFORM,
        ])
        .status()
        .unwrap();
    assert!(
        !bad.success(),
        "verify must FAIL on a tampered payload (digest mismatch)"
    );

    // Cleanup.
    for d in [&stage, &generated, &tampered] {
        let _ = std::fs::remove_dir_all(d);
    }
}

fn copy_dir(src: &std::path::Path, dst: &std::path::Path) {
    std::fs::create_dir_all(dst).unwrap();
    for entry in std::fs::read_dir(src).unwrap() {
        let e = entry.unwrap();
        let d = dst.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &d);
        } else {
            std::fs::copy(e.path(), &d).unwrap();
        }
    }
}
