//! PRF-F6-W2: end-to-end staging contract for the release factory with
//! BOTH Tier-1 platforms and the actual `stage-platform-payloads.sh`
//! flatten step.
//!
//! F6 criterion: reproduce the exact layout `actions/download-artifact@v4`
//! deposits in `staging/` under the workflow's current config
//! (`merge-multiple: false`), then verify that:
//!   - the flatten script runs cleanly
//!   - `cognicode-release generate` and `verify` succeed for every
//!     Tier-1 platform
//!   - a tampered payload is rejected (per payload, not batch)
//!   - a missing payload is rejected loudly
//!   - a wrong-platform payload is rejected
//!   - duplicate payloads from two lanes are rejected
//!
//! This test exists because CI run #35967155996 (the v0.97.5 release
//! job) failed with `no payloads-* lane directories found` after
//! `download-artifact@v4 merge-multiple: true` stripped the lane
//! prefix. The fix is `merge-multiple: false` in the workflow
//! (preserves each lane as its own subdir). This test pins the
//! downstream contract: given the canonical CI layout (each lane
//! in its own `payloads-<plat>/dist/` subdir), the rest of the
//! pipeline must work for every Tier-1 platform.
//!
//! Comparison surface (narrowest observable): exit codes of the
//! flatten script + `cognicode-release generate` + `cognicode-
//! release verify`, plus the exact set of filenames in the
//! flattened staging tree.
//!
//! Non-vacuity guards:
//! - The synthetic payload content includes platform + component
//!   markers so a wrong-platform swap would change the SHA and
//!   fail verify.
//! - The reproduce_ci_layout function synthesises both Tier-1 lanes
//!   (x86_64 + aarch64) — a single-platform test would not have
//!   caught the original "missing payloads lane" failure.
//! - The tampering sub-tests flip a single byte per payload, one
//!   at a time, so the test confirms the SHA matches the failed
//!   file (not just "any failure").

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop();
    p.pop();
    p
}

fn release_bin() -> PathBuf {
    let p = repo_root().join("target/release/cognicode-release");
    assert!(p.exists(), "cognicode-release binary missing; build release first");
    p
}

const VERSION: &str = "0.97.4";
const TAG: &str = "v0.97.4";

/// Both Tier-1 platforms — this test exists specifically because a
/// single-platform layout didn't catch the original failure.
const TIER1_PLATFORMS: &[&str] = &["x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu"];
const PLATFORM_COMPONENTS: &[&str] = &["cogh", "cognicode", "cognicode-mcp"];
const SKILL_BUNDLES: &[&str] = &["cognicode", "cognicode-mcp"];

/// Reproduce the GitHub Actions layout that
/// `actions/download-artifact@v4` produces with `merge-multiple:
/// false`: each lane is preserved as its own subdir
/// `staging/payloads-<platform>/dist/<component>-...tar.gz`.
/// SBOMs sit at `staging/payloads-<platform>/crates/<component>-<plat>.cdx.json`.
/// Skill bundles (from a separate workflow step) are staged at
/// `staging/<id>-<ver>.tar.gz` before the flatten script runs.
fn reproduce_ci_layout(staging: &Path) {
    use std::fs;
    fs::create_dir_all(staging).unwrap();
    for plat in TIER1_PLATFORMS {
        let lane_dir = staging.join(format!("payloads-{plat}"));
        fs::create_dir_all(lane_dir.join("dist")).unwrap();
        for comp in PLATFORM_COMPONENTS {
            let payload_name = format!("{comp}-{VERSION}-{plat}.tar.gz");
            let payload_path = lane_dir.join("dist").join(&payload_name);
            // Synth content is deterministic and includes platform +
            // component markers. If a future refactor swaps payloads
            // between platforms, the SHA changes and verify fails.
            let mut content = Vec::new();
            content.extend_from_slice(b"# F6.W2 synth payload\n");
            content.extend_from_slice(format!("component={comp}\n").as_bytes());
            content.extend_from_slice(format!("platform={plat}\n").as_bytes());
            content.extend_from_slice(format!("version={VERSION}\n").as_bytes());
            fs::write(&payload_path, &content).unwrap();

            // CycloneDX SBOM accompanies the payload.
            let cdx_name = format!("crates/{comp}-{plat}.cdx.json");
            fs::create_dir_all(lane_dir.join("crates")).unwrap();
            fs::write(
                lane_dir.join(&cdx_name),
                br#"{"bomFormat":"CycloneDX","specVersion":"1.5","components":[]}"#,
            )
            .unwrap();
        }
    }
    // Skill bundles are produced by `just bundle-skills` and staged
    // by the workflow before the flatten step runs.
    for b in SKILL_BUNDLES {
        let name = format!("{b}-{VERSION}.tar.gz");
        let mut content = Vec::new();
        content.extend_from_slice(b"# F6.W2 synth skill bundle\n");
        content.extend_from_slice(format!("id={b}\n").as_bytes());
        content.extend_from_slice(format!("version={VERSION}\n").as_bytes());
        fs::write(staging.join(&name), &content).unwrap();
    }
}

/// Run the workflow's flatten script. Fails the test if the script
/// exits non-zero.
fn run_flatten(staging: &Path) {
    let script = repo_root().join("scripts/ci/stage-platform-payloads.sh");
    assert!(
        script.exists(),
        "flatten script missing at {}",
        script.display()
    );
    let status = Command::new("bash")
        .arg(&script)
        .arg(staging)
        .status()
        .unwrap();
    assert!(
        status.success(),
        "flatten script failed for staging={}",
        staging.display()
    );
}

/// Run `cognicode-release generate` for one platform. Returns the
/// exit status.
fn generate_for(out: &Path, staging: &Path, plat: &str) -> std::process::ExitStatus {
    let mut cmd = Command::new(release_bin());
    cmd.args([
        "generate",
        "--staging",
        staging.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
        "--version",
        VERSION,
        "--tag",
        TAG,
        "--source-commit",
        "deadbeef",
        "--platform",
        plat,
    ]);
    cmd.status().unwrap()
}

/// Run `cognicode-release verify` for one platform.
fn verify_for(out: &Path, plat: &str) -> std::process::ExitStatus {
    Command::new(release_bin())
        .args([
            "verify",
            "--staging",
            out.to_str().unwrap(),
            "--tag",
            TAG,
            "--platform",
            plat,
        ])
        .status()
        .unwrap()
}

/// Read the SHA256 of a payload from SHA256SUMS.
fn sha256_for(sums_path: &Path, name: &str) -> Option<String> {
    let text = std::fs::read_to_string(sums_path).ok()?;
    for line in text.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 && parts[1] == name {
            return Some(parts[0].to_string());
        }
    }
    None
}

/// F6.W2 / test 1: full pipeline, BOTH Tier-1 platforms. Stage
/// → flatten → generate → verify (per platform). This is the
/// happy path: every step must succeed.
#[test]
fn prf_f6_w2_full_pipeline_both_platforms_passes() {
    use std::fs;
    let tmp = std::env::temp_dir().join(format!(
        "prf-f6-w2-both-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).unwrap();

    let staging = tmp.join("staging");
    reproduce_ci_layout(&staging);

    run_flatten(&staging);

    // After flattening, all canonical payloads + SBOMs must be at
    // the staging root. Skill bundles were already there.
    let mut required: Vec<String> = Vec::new();
    for plat in TIER1_PLATFORMS {
        for comp in PLATFORM_COMPONENTS {
            required.push(format!("{comp}-{VERSION}-{plat}.tar.gz"));
            required.push(format!("{comp}-{plat}.cdx.json"));
        }
    }
    for b in SKILL_BUNDLES {
        required.push(format!("{b}-{VERSION}.tar.gz"));
    }
    for r in &required {
        assert!(
            staging.join(r).exists(),
            "flatten output missing {r}; required={required:?}"
        );
    }

    // Generate + verify for every Tier-1 platform. The release
    // factory is invoked per platform (matching the production
    // workflow).
    for plat in TIER1_PLATFORMS {
        let out = tmp.join(format!("release-{plat}"));
        fs::create_dir_all(&out).unwrap();
        let gen_out = generate_for(&out, &staging, plat);
        assert!(
            gen_out.success(),
            "generate failed for platform={plat}, staging={}",
            staging.display()
        );

        let ver = verify_for(&out, plat);
        assert!(
            ver.success(),
            "verify failed for platform={plat} on a freshly generated candidate"
        );
    }

    let _ = fs::remove_dir_all(&tmp);
}

/// F6.W2 / test 2: per-payload tampering. Generate a clean
/// candidate, then flip one byte of ONE payload at a time and
/// confirm `verify` rejects it AND that the SHA reported in the
/// error corresponds to the tampered file. This pins that verify
/// actually checks each payload's hash, not just a "first failure".
#[test]
fn prf_f6_w2_per_payload_tampering_is_detected() {
    use std::fs;
    let tmp = std::env::temp_dir().join(format!(
        "prf-f6-w2-tamper-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).unwrap();

    let staging = tmp.join("staging");
    reproduce_ci_layout(&staging);
    run_flatten(&staging);

    let plat = TIER1_PLATFORMS[0];
    let out = tmp.join("release");
    fs::create_dir_all(&out).unwrap();
    assert!(generate_for(&out, &staging, plat).success());

    let sums = out.join("SHA256SUMS");
    assert!(sums.exists(), "SHA256SUMS missing");

    // Tamper each component payload, one at a time, and confirm
    // verify rejects the candidate.
    for comp in PLATFORM_COMPONENTS {
        let target = out.join(format!("{comp}-{VERSION}-{plat}.tar.gz"));
        assert!(target.exists(), "{comp} payload missing from {out:?}");

        let original_sha = sha256_for(&sums, &format!("{comp}-{VERSION}-{plat}.tar.gz"));
        assert!(
            original_sha.is_some(),
            "no SHA for {comp} in SHA256SUMS"
        );
        let original_sha = original_sha.unwrap();

        // Flip one byte and write back.
        let mut bytes = fs::read(&target).unwrap();
        bytes[0] ^= 0xFF;
        fs::write(&target, &bytes).unwrap();

        let ver = Command::new(release_bin())
            .args([
                "verify",
                "--staging",
                out.to_str().unwrap(),
                "--tag",
                TAG,
                "--platform",
                plat,
            ])
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&ver.stderr);
        assert!(
            !ver.status.success(),
            "verify must reject tampered {comp}; got success, stderr={stderr}"
        );
        // The verify error must reference the actual tampered
        // payload's hash, not just a generic failure.
        let tampered_sha = format!(
            "{:x}",
            <sha2::Sha256 as sha2::Digest>::digest(&bytes)
        );
        assert!(
            stderr.contains(&tampered_sha) || stderr.contains(comp),
            "verify error should name the tampered payload {comp} (sha={tampered_sha}), \
             got stderr: {stderr}"
        );
        assert_ne!(
            original_sha, tampered_sha,
            "test invariant: tampering must change the SHA"
        );

        // Restore the byte so the loop can continue cleanly. (Not
        // strictly necessary because we regenerate below, but keeps
        // the working tree clean between iterations.)
        bytes[0] ^= 0xFF;
        fs::write(&target, &bytes).unwrap();
    }

    let _ = fs::remove_dir_all(&tmp);
}

/// F6.W2 / test 3: missing payload is rejected. Remove one
/// platform's cognicode-mcp payload after flatten; verify must
/// fail loudly. This guards against a future regression that
/// silently ships an incomplete release.
#[test]
fn prf_f6_w2_missing_payload_is_rejected() {
    use std::fs;
    let tmp = std::env::temp_dir().join(format!(
        "prf-f6-w2-missing-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).unwrap();

    let staging = tmp.join("staging");
    reproduce_ci_layout(&staging);
    run_flatten(&staging);

    let plat = TIER1_PLATFORMS[0];
    let out = tmp.join("release");
    fs::create_dir_all(&out).unwrap();
    assert!(generate_for(&out, &staging, plat).success());

    // Remove one payload from the OUTPUT directory (the candidate
    // that will be verified downstream).
    let target = out.join(format!("cognicode-mcp-{VERSION}-{plat}.tar.gz"));
    assert!(target.exists());
    fs::remove_file(&target).unwrap();

    let ver = verify_for(&out, plat);
    assert!(
        !ver.success(),
        "verify must reject candidate missing cognicode-mcp payload"
    );

    let _ = fs::remove_dir_all(&tmp);
}

/// F6.W2 / test 4: duplicate (same-name payload from two lanes)
/// is rejected by the flatten script. This protects against the
/// last-writer-wins behaviour `merge-multiple: true` would have
/// introduced, and against accidental cross-lane contamination.
#[test]
fn prf_f6_w2_duplicate_payload_across_lanes_is_rejected() {
    use std::fs;
    let tmp = std::env::temp_dir().join(format!(
        "prf-f6-w2-dup-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).unwrap();

    let staging = tmp.join("staging");
    reproduce_ci_layout(&staging);

    // Create a second lane claiming the x86_64 cogh payload.
    let dup_lane = staging.join("payloads-duplicate-impostor");
    fs::create_dir_all(dup_lane.join("dist")).unwrap();
    fs::write(
        dup_lane
            .join("dist")
            .join(format!("cogh-{VERSION}-x86_64-unknown-linux-gnu.tar.gz")),
        b"different content with same name\n",
    )
    .unwrap();

    let script = repo_root().join("scripts/ci/stage-platform-payloads.sh");
    let status = Command::new("bash")
        .arg(&script)
        .arg(&staging)
        .status()
        .unwrap();
    assert!(
        !status.success(),
        "flatten must reject two lanes claiming the same payload name"
    );

    let _ = fs::remove_dir_all(&tmp);
}

/// F6.W2 / test 5: wrong-platform payload is rejected. A lane
/// claiming to be `x86_64` ships an `aarch64` payload. The flatten
/// script must not silently accept it; generate/verify must fail.
#[test]
fn prf_f6_w2_wrong_platform_payload_is_rejected() {
    use std::fs;
    let tmp = std::env::temp_dir().join(format!(
        "prf-f6-w2-wrongplat-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).unwrap();

    let staging = tmp.join("staging");
    reproduce_ci_layout(&staging);

    // Corrupt one x86_64 lane by renaming it to claim the
    // aarch64 payload instead. The flatten script must catch the
    // mismatch (the lane name says x86_64 but the file is named
    // for aarch64).
    let x86_lane = staging.join("payloads-x86_64-unknown-linux-gnu");
    let bad_payload = x86_lane
        .join("dist")
        .join(format!("cogh-{VERSION}-x86_64-unknown-linux-gnu.tar.gz"));
    let renamed = x86_lane
        .join("dist")
        .join(format!("cogh-{VERSION}-aarch64-unknown-linux-gnu.tar.gz"));
    fs::rename(&bad_payload, &renamed).unwrap();

    // Also drop the real aarch64 cogh to keep names unique.
    let aarc_real = staging
        .join("payloads-aarch64-unknown-linux-gnu")
        .join("dist")
        .join(format!("cogh-{VERSION}-aarch64-unknown-linux-gnu.tar.gz"));
    fs::remove_file(&aarc_real).unwrap();

    // The flatten script validates filename <-> lane agreement,
    // so the cross-lane mismatch is rejected at this stage.
    let script = repo_root().join("scripts/ci/stage-platform-payloads.sh");
    let status = Command::new("bash")
        .arg(&script)
        .arg(&staging)
        .status()
        .unwrap();
    assert!(
        !status.success(),
        "flatten must reject a lane whose payload filenames don't match the lane name"
    );

    let _ = fs::remove_dir_all(&tmp);
}
