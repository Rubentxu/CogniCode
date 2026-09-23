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

const VERSION: &str = "0.97.4";
const TAG: &str = "v0.97.4";
const PLATFORM: &str = "x86_64-unknown-linux-gnu";

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
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let stage = std::env::temp_dir().join(format!("prf-dist-uat-stage-{}", std::process::id()));
    let generated = std::env::temp_dir().join(format!("prf-dist-uat-gen-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&stage);
    let _ = std::fs::remove_dir_all(&generated);
    std::fs::create_dir_all(&stage).unwrap();

    // 1. Stage real payloads from target/release (real binaries, not fixtures).
    for stem in ["cogh", "cognicode", "cognicode-mcp"] {
        let src = root.join(format!("target/release/{stem}"));
        assert!(src.exists(), "missing release binary {stem}");
        let dst = stage.join(format!("{stem}-{VERSION}-{PLATFORM}.tar.gz"));
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
    // Skill bundle payloads (versioned, produced by `just bundle-skills`).
    for bundle in ["cognicode", "cognicode-mcp"] {
        let src = root.join(format!("dist/{bundle}-{VERSION}.tar.gz"));
        assert!(
            src.exists(),
            "missing skill bundle {}; run `COGNICODE_VERSION=0.97.4 just bundle-skills`",
            src.display()
        );
        std::fs::copy(&src, stage.join(src.file_name().unwrap())).unwrap();
    }

    // 2. Generate the release candidate from those bytes.
    let out = Command::new(release_bin())
        .args([
            "generate",
            "--staging",
            stage.to_str().unwrap(),
            "--out",
            generated.to_str().unwrap(),
            "--version",
            VERSION,
            "--tag",
            TAG,
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

    // 3. DIST-06: inventory provenance matches current HEAD.
    let inventory =
        std::fs::read_to_string(generated.join(format!("release-inventory-{VERSION}.json")))
            .unwrap();
    assert!(
        inventory.contains(&head_commit()),
        "inventory must record current HEAD as source_commit"
    );

    // 4. DIST-01/06: verification from the release candidate dir passes
    //    (hashes + manifest + composition agree).
    let ok = Command::new(release_bin())
        .args([
            "verify",
            "--staging",
            generated.to_str().unwrap(),
            "--tag",
            TAG,
            "--platform",
            PLATFORM,
        ])
        .status()
        .unwrap();
    assert!(ok.success(), "verify must PASS on the clean candidate");

    // 5. DIST-06 negative case: a tampered payload is rejected (exit != 0).
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
    let victim = tampered.join(format!("cognicode-{VERSION}-{PLATFORM}.tar.gz"));
    let mut bytes = std::fs::read(&victim).unwrap();
    bytes.extend_from_slice(b"tampered");
    std::fs::write(&victim, &bytes).unwrap();

    let bad = Command::new(release_bin())
        .args([
            "verify",
            "--staging",
            tampered.to_str().unwrap(),
            "--tag",
            TAG,
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
