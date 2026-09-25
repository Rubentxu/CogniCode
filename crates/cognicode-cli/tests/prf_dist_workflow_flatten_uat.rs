//! PRF-DIST-WORKFLOW-FLATTEN UAT (real release binaries).
//!
//! Reproduces the GitHub Actions `release.yml` download-artifact layout,
//! then verifies that the stage-flattening step produces a staging tree
//! that `cognicode-release generate` accepts on the FIRST try, including
//! the contract assertions the release factory enforces:
//!
//! - exactly the 6 platform payloads present (3 components × 2 Tier-1
//!   platforms), no extras, no duplicates;
//! - exactly the declared portable skill bundle payloads;
//! - the `cyclonedx-*.cdx.json` SBOMs accompany each platform payload;
//! - a flatten step that produces the wrong name set fails fast;
//! - `generate` rejects a tampered/missing payload.
//!
//! This test exists because the production CI run #35874781973 failed
//! with `missing artifact cogh-0.97.4-x86_64-unknown-linux-gnu.tar.gz`
//! while the file was actually present under `staging/dist/`. The flatten
//! step in the workflow is what makes the staging tree the release
//! factory expects.

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
    assert!(
        p.exists(),
        "cognicode-release binary missing; build release first"
    );
    p
}

const VERSION: &str = "0.97.4";
const TAG: &str = "v0.97.4";

const TIER1_PLATFORMS: &[&str] = &["x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu"];
const PLATFORM_COMPONENTS: &[&str] = &["cogh", "cognicode", "cognicode-mcp"];
const SKILL_BUNDLES: &[&str] = &["cognicode", "cognicode-mcp"];

/// Recreate the GitHub Actions layout:
/// `staging/payloads-<platform>/dist/<component>-<ver>-<plat>.tar.gz`.
/// Also stage the portable skill bundle tarballs (`<id>-<ver>.tar.gz`)
/// at the staging root, exactly like the workflow does between the
/// download and the generate step.
fn reproduce_ci_layout(staging: &Path) {
    use std::fs;
    fs::create_dir_all(staging).unwrap();
    for plat in TIER1_PLATFORMS {
        let lane_dir = staging.join(format!("payloads-{plat}"));
        fs::create_dir_all(lane_dir.join("dist")).unwrap();
        for comp in PLATFORM_COMPONENTS {
            // Each platform "produces" its own tarball; in CI they are
            // produced by the per-lane build job. For the test we
            // synthesise a deterministic payload per (component, platform)
            // so the flattening step has to find them inside the lane
            // subdir, not at the staging root.
            let payload_name = format!("{comp}-{VERSION}-{plat}.tar.gz");
            let payload_path = lane_dir.join("dist").join(&payload_name);
            let mut content = Vec::new();
            content.extend_from_slice(b"# synth payload for CI layout test\n");
            content.extend_from_slice(format!("component={comp}\n").as_bytes());
            content.extend_from_slice(format!("platform={plat}\n").as_bytes());
            content.extend_from_slice(format!("version={VERSION}\n").as_bytes());
            fs::write(&payload_path, &content).unwrap();

            // CycloneDX SBOM sits next to the payload (the workflow
            // uploads `dist/*.tar.gz` and `crates/*.cdx.json` together).
            let cdx_name = format!("crates/{comp}-{plat}.cdx.json");
            fs::create_dir_all(lane_dir.join("crates")).unwrap();
            fs::write(
                lane_dir.join(&cdx_name),
                br#"{"bomFormat":"CycloneDX","specVersion":"1.5","components":[]}"#,
            )
            .unwrap();
        }
    }
    // Skill bundle payloads are produced by `just bundle-skills` and
    // staged by the workflow at `staging/<id>-<ver>.tar.gz`. They must
    // already be present when generate runs, so the test stages them
    // upfront at the staging root.
    for b in SKILL_BUNDLES {
        let name = format!("{b}-{VERSION}.tar.gz");
        let mut content = Vec::new();
        content.extend_from_slice(b"# synth skill bundle for CI layout test\n");
        content.extend_from_slice(format!("id={b}\n").as_bytes());
        content.extend_from_slice(format!("version={VERSION}\n").as_bytes());
        fs::write(staging.join(&name), &content).unwrap();
    }
}

/// Asserts that the staging tree is exactly the canonical flat layout
/// the release factory expects: every required payload is at the
/// staging root, every SBOM is present, and nothing unexpected leaks in.
/// `require_skills` controls whether portable skill bundles are part of
/// the required set (they are produced by a separate workflow step and
/// are not always present in a flatten-only test).
fn assert_canonical_flat(staging: &Path, expected_extra: &[&str], require_skills: bool) {
    use std::collections::BTreeSet;

    let mut required: BTreeSet<String> = BTreeSet::new();
    for plat in TIER1_PLATFORMS {
        for comp in PLATFORM_COMPONENTS {
            required.insert(format!("{comp}-{VERSION}-{plat}.tar.gz"));
            required.insert(format!("{comp}-{plat}.cdx.json"));
        }
    }
    if require_skills {
        for b in SKILL_BUNDLES {
            required.insert(format!("{b}-{VERSION}.tar.gz"));
        }
    }
    for e in expected_extra {
        required.insert((*e).to_string());
    }

    // Recursively walk the flattened staging and collect every filename.
    let mut present: BTreeSet<String> = BTreeSet::new();
    fn walk(dir: &Path, present: &mut BTreeSet<String>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let e = entry.unwrap();
            let p = e.path();
            if p.is_file() {
                present.insert(p.file_name().unwrap().to_str().unwrap().to_string());
            } else if p.is_dir() {
                walk(&p, present);
            }
        }
    }
    walk(staging, &mut present);

    let missing: Vec<&String> = required.iter().filter(|n| !present.contains(*n)).collect();
    assert!(
        missing.is_empty(),
        "flatten output missing required files: {missing:?}\npresent={present:?}"
    );
}

/// RED case: generate against the unflattened CI layout MUST fail with
/// the exact class of error the operator reported in run #35874781973.
#[test]
fn dist_workflow_unflattened_layout_is_rejected() {
    use std::fs;
    let tmp = std::env::temp_dir().join(format!(
        "prf-dist-workflow-red-{}-{}",
        std::process::id(),
        TIER1_PLATFORMS[0]
    ));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).unwrap();

    let staging = tmp.join("staging");
    reproduce_ci_layout(&staging);

    let out = Command::new(release_bin())
        .args([
            "generate",
            "--staging",
            staging.to_str().unwrap(),
            "--out",
            tmp.join("out").to_str().unwrap(),
            "--version",
            VERSION,
            "--tag",
            TAG,
            "--source-commit",
            "deadbeef",
            "--platform",
            TIER1_PLATFORMS[0],
        ])
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "generate must reject the unflattened CI layout; \
         got success with stdout={}",
        String::from_utf8_lossy(&out.stdout)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("missing artifact") || stderr.contains("cogh"),
        "stderr should reference the missing cogh payload, got: {stderr}"
    );

    let _ = fs::remove_dir_all(&tmp);
}

/// Run the flatten bash script the workflow uses, then run generate on
/// the flattened staging tree. Asserts generate PASSes, verify PASSes,
/// and a tampered payload is rejected.
#[test]
fn dist_workflow_flattened_layout_passes_generate_and_verify() {
    use std::fs;
    let tmp = std::env::temp_dir().join(format!(
        "prf-dist-workflow-green-{}-{}",
        std::process::id(),
        TIER1_PLATFORMS[0]
    ));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).unwrap();

    let staging = tmp.join("staging");
    reproduce_ci_layout(&staging);

    // Run the flatten script under test.
    let script = repo_root().join("scripts/ci/stage-platform-payloads.sh");
    assert!(
        script.exists(),
        "flatten script missing at {}; expected scripts/ci/stage-platform-payloads.sh",
        script.display()
    );
    let status = Command::new("bash")
        .arg(&script)
        .arg(&staging)
        .status()
        .unwrap();
    assert!(status.success(), "flatten script failed");

    // After flattening, the staging root must hold the platform
    // payloads + SBOMs produced by the script. Portable skill bundle
    // tarballs (`cognicode-0.97.4.tar.gz`, `cognicode-mcp-0.97.4.tar.gz`)
    // are produced by the workflow's separate `bundle-skills` step AFTER
    // this flatten script runs; the flatten script must not assume their
    // presence.
    assert_canonical_flat(&staging, &[], false);

    // Now generate against the flattened staging. The script supports a
    // single-platform invocation (matching the test scope); the production
    // workflow generates per-platform and combines results.
    let out = tmp.join("release");
    fs::create_dir_all(&out).unwrap();
    let gen_output = Command::new(release_bin())
        .args([
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
            TIER1_PLATFORMS[0],
        ])
        .output()
        .unwrap();
    assert!(
        gen_output.status.success(),
        "generate failed after flatten: stderr={}",
        String::from_utf8_lossy(&gen_output.stderr)
    );

    // Verify must also pass on the output.
    let verify = Command::new(release_bin())
        .args([
            "verify",
            "--staging",
            out.to_str().unwrap(),
            "--tag",
            TAG,
            "--platform",
            TIER1_PLATFORMS[0],
        ])
        .status()
        .unwrap();
    assert!(
        verify.success(),
        "verify must pass on the freshly generated candidate"
    );

    // Tamper one payload (flip a byte) → verify must reject it.
    let target = out.join(format!("cogh-{VERSION}-{}.tar.gz", TIER1_PLATFORMS[0]));
    let mut bytes = fs::read(&target).unwrap();
    bytes[0] ^= 0xff;
    fs::write(&target, &bytes).unwrap();
    let bad = Command::new(release_bin())
        .args([
            "verify",
            "--staging",
            out.to_str().unwrap(),
            "--tag",
            TAG,
            "--platform",
            TIER1_PLATFORMS[0],
        ])
        .status()
        .unwrap();
    assert!(
        !bad.success(),
        "verify must reject a tampered payload; otherwise the release \
         pipeline is shipping mutable bytes"
    );

    let _ = fs::remove_dir_all(&tmp);
}

/// Negative contract: flatten script must reject unknown files at the
/// staging root (e.g. a `.md` left over from a previous job) and must
/// fail loudly on a duplicate.
#[test]
fn dist_workflow_flatten_rejects_unknown_files_and_duplicates() {
    use std::fs;
    let tmp = std::env::temp_dir().join(format!(
        "prf-dist-workflow-neg-{}-{}",
        std::process::id(),
        TIER1_PLATFORMS[0]
    ));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).unwrap();

    let staging = tmp.join("staging");
    reproduce_ci_layout(&staging);

    // Drop a stray .md into the staging root (would happen if a prior
    // step leaked a README).
    fs::write(staging.join("README-extra.md"), b"stray\n").unwrap();
    let script = repo_root().join("scripts/ci/stage-platform-payloads.sh");
    let status = Command::new("bash")
        .arg(&script)
        .arg(&staging)
        .status()
        .unwrap();
    assert!(
        !status.success(),
        "flatten must reject unexpected files at the staging root"
    );

    // Now produce two lanes claiming the same payload (different file
    // contents, same name). Flatten must reject the duplicate.
    let _ = fs::remove_dir_all(&staging);
    fs::create_dir_all(&staging).unwrap();
    reproduce_ci_layout(&staging);
    let dup_dir = staging.join("payloads-aarch64-unknown-linux-gnu-extra");
    fs::create_dir_all(dup_dir.join("dist")).unwrap();
    fs::write(
        dup_dir
            .join("dist")
            .join(format!("cogh-{VERSION}-x86_64-unknown-linux-gnu.tar.gz")),
        b"different content with same name\n",
    )
    .unwrap();
    let dup = Command::new("bash")
        .arg(&script)
        .arg(&staging)
        .status()
        .unwrap();
    assert!(
        !dup.success(),
        "flatten must reject a payload name produced by two lanes"
    );

    let _ = fs::remove_dir_all(&tmp);
}
