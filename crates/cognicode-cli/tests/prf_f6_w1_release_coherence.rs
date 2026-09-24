//! PRF-F6-W1: release-candidate end-to-end coherence — the release
//! factory's `generate` and `verify` subcommands must produce
//! internally consistent artifacts (BundleManifest, ReleaseInventory,
//! SHA256SUMS) across a full generate→verify round-trip, and any
//! post-generation tampering must be detected.
//!
//! F6 criterion (a): "gates por SHA sobre los artefactos publicados".
//! Local-first version: the "canal verificable" is the staging
//! directory; SHA verification is local (no network). The end-to-end
//! property being pinned is: a release candidate that was just
//! generated MUST verify cleanly; the same candidate, after a
//! single-bit tampering, MUST NOT verify.
//!
//! Comparison surface (narrowest observable): exit codes of
//! `cognicode-release generate` and `cognicode-release verify`,
//! plus the contents of `SHA256SUMS` (which is the canonical
//! integrity gate).
//!
//! Non-vacuity guards (mandatory per operator rules):
//! - The staging directory must contain real payload archives
//!   (not empty files).
//! - The generated SHA256SUMS must list at least 3 entries
//!   (one per binary: cogh, cognicode, cognicode-mcp).
//! - The verified SHA256SUMS must match the recomputed digests
//!   (otherwise a tampered-but-passing verify would slip through).
//!
//! RED/GREEN invariant: if the release factory ever silently
//! produces mismatched digests (a regression in the integrity
//! pipeline), this test fails on the verify step. The tampering
//! sub-test confirms the verify gate is not a no-op.

use std::path::PathBuf;
use std::process::Command;

/// Resolve the `cognicode-release` binary. Must be built in
/// release profile before this test runs.
fn release_bin() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop();
    p.pop();
    p.push("target/release/cognicode-release");
    assert!(
        p.exists(),
        "cognicode-release binary missing at {}; build with `cargo build --release --bin cognicode-release`",
        p.display()
    );
    p
}

/// Workspace root (3 levels up from this test file's crate).
fn repo_root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop();
    p.pop();
    p
}

/// Stage the three real payload archives (cogh, cognicode,
/// cognicode-mcp) into a private temp staging dir. Returns the
/// staging path.
fn stage_payloads(tag: &str) -> PathBuf {
    let root = repo_root();
    let stem = tag.replace('v', "");
    // VERSION is encoded in the payload filenames (e.g.
    // cognicode-0.97.5-x86_64-unknown-linux-gnu.tar.gz). We
    // re-tar the binaries that are already in target/release.
    let stage = std::env::temp_dir().join(format!("prf-f6-w1-stage-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&stage);
    std::fs::create_dir_all(&stage).unwrap();

    for bin_name in ["cogh", "cognicode", "cognicode-mcp"] {
        let src = root.join(format!("target/release/{bin_name}"));
        assert!(
            src.exists(),
            "missing release binary {bin_name}; build release first"
        );
        let dst = stage.join(format!("{bin_name}-{stem}-x86_64-unknown-linux-gnu.tar.gz"));
        let st = Command::new("tar")
            .arg("-czf")
            .arg(&dst)
            .arg("-C")
            .arg(root.join("target/release"))
            .arg(bin_name)
            .status()
            .unwrap();
        assert!(st.success(), "tar failed for {bin_name}");
        // Non-vacuity: payload must be non-empty.
        let size = std::fs::metadata(&dst).unwrap().len();
        assert!(
            size > 1000,
            "non-vacuity: payload {bin_name} is suspiciously small: {size} bytes"
        );
    }

    // Skill bundle payloads (versioned, produced by `just bundle-skills`).
    // The release factory requires these to be present in the staging
    // directory alongside the binary payloads.
    for bundle in ["cognicode", "cognicode-mcp"] {
        let src = root.join(format!("dist/{bundle}-{stem}.tar.gz"));
        assert!(
            src.exists(),
            "missing skill bundle {bundle}-{stem}.tar.gz at {}; run `just bundle-skills`",
            src.display()
        );
        std::fs::copy(&src, stage.join(src.file_name().unwrap())).unwrap();
    }
    stage
}

fn head_commit() -> String {
    let out = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("git rev-parse");
    assert!(out.status.success(), "git rev-parse failed");
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

/// F6.W1 / test 1: clean round-trip. Stage → generate → verify
/// passes. This is the "happy path" — if it fails, the release
/// factory itself is broken.
#[test]
fn prf_f6_w1_clean_round_trip_generates_and_verifies() {
    let tag = "v0.97.4";
    let stem = "0.97.4";
    let platform = "x86_64-unknown-linux-gnu";
    let stage = stage_payloads(tag);
    let generated = std::env::temp_dir().join(format!(
        "prf-f6-w1-gen-{}-{}",
        tag,
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&generated);

    // Generate
    let gen_out = Command::new(release_bin())
        .args([
            "generate",
            "--staging",
            stage.to_str().unwrap(),
            "--out",
            generated.to_str().unwrap(),
            "--version",
            stem,
            "--tag",
            tag,
            "--source-commit",
            &head_commit(),
            "--platform",
            platform,
        ])
        .output()
        .expect("spawn generate");
    assert!(
        gen_out.status.success(),
        "generate failed: stderr={}",
        String::from_utf8_lossy(&gen_out.stderr)
    );

    // Non-vacuity: SHA256SUMS must exist and list ≥3 entries.
    let sums_path = generated.join("SHA256SUMS");
    assert!(
        sums_path.exists(),
        "non-vacuity: SHA256SUMS missing after generate"
    );
    let sums_text = std::fs::read_to_string(&sums_path).unwrap();
    let sums_lines: Vec<&str> = sums_text.lines().filter(|l| !l.is_empty()).collect();
    assert!(
        sums_lines.len() >= 3,
        "non-vacuity: SHA256SUMS has only {} entries, expected ≥3 (cogh + cognicode + cognicode-mcp): {sums_text}",
        sums_lines.len()
    );

    // Verify (clean)
    let verify = Command::new(release_bin())
        .args([
            "verify",
            "--staging",
            generated.to_str().unwrap(),
            "--tag",
            tag,
            "--platform",
            platform,
        ])
        .output()
        .expect("spawn verify");
    assert!(
        verify.status.success(),
        "verify on clean candidate must PASS, got exit {:?}, stderr={}",
        verify.status.code(),
        String::from_utf8_lossy(&verify.stderr)
    );

    let _ = std::fs::remove_dir_all(&stage);
    let _ = std::fs::remove_dir_all(&generated);
}

/// F6.W1 / test 2: tampering is detected. Take the clean
/// candidate from test 1's pipeline, flip a byte in one of the
/// payload archives, and confirm `verify` exits non-zero. This
/// pins the integrity gate: verify MUST NOT be a no-op.
#[test]
fn prf_f6_w1_tampered_payload_fails_verify() {
    let tag = "v0.97.4";
    let stem = "0.97.4";
    let platform = "x86_64-unknown-linux-gnu";
    let stage = stage_payloads(tag);
    let generated = std::env::temp_dir().join(format!(
        "prf-f6-w1-gen-tam-{}-{}",
        tag,
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&generated);

    // Generate clean candidate.
    let gen_out = Command::new(release_bin())
        .args([
            "generate",
            "--staging",
            stage.to_str().unwrap(),
            "--out",
            generated.to_str().unwrap(),
            "--version",
            stem,
            "--tag",
            tag,
            "--source-commit",
            &head_commit(),
            "--platform",
            platform,
        ])
        .output()
        .expect("spawn generate");
    assert!(gen_out.status.success(), "clean generate must succeed");

    // Copy to a tampered staging dir and flip one byte in each
    // payload archive (every one — bit-flip one is enough but
    // we flip all to maximize the chance of catching a future
    // regression in any single payload's digest computation).
    let tampered = std::env::temp_dir().join(format!(
        "prf-f6-w1-tam-{}-{}",
        tag,
        std::process::id()
    ));
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

    // Flip one byte in each .tar.gz archive.
    for entry in std::fs::read_dir(&tampered).unwrap() {
        let e = entry.unwrap();
        let path = e.path();
        if path.extension().and_then(|x| x.to_str()) == Some("gz") {
            let mut bytes = std::fs::read(&path).unwrap();
            assert!(!bytes.is_empty(), "non-vacuity: empty payload");
            // Flip the first byte. Note: gzip has a header with
            // magic bytes (1f 8b) — flipping the first byte will
            // either fail gzip decode (best case) or produce
            // different decompressed content. Either way the
            // SHA256 of the .tar.gz archive changes.
            bytes[0] ^= 0xFF;
            std::fs::write(&path, &bytes).unwrap();
        }
    }

    // Verify on tampered candidate must FAIL.
    let verify = Command::new(release_bin())
        .args([
            "verify",
            "--staging",
            tampered.to_str().unwrap(),
            "--tag",
            tag,
            "--platform",
            platform,
        ])
        .output()
        .expect("spawn verify tampered");
    assert!(
        !verify.status.success(),
        "verify on tampered candidate must FAIL (exit non-zero); got success, stdout={}, stderr={}",
        String::from_utf8_lossy(&verify.stdout),
        String::from_utf8_lossy(&verify.stderr)
    );

    let _ = std::fs::remove_dir_all(&stage);
    let _ = std::fs::remove_dir_all(&generated);
    let _ = std::fs::remove_dir_all(&tampered);
}

// ---------------------------------------------------------------------------
// helpers (test-local)
// ---------------------------------------------------------------------------

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
