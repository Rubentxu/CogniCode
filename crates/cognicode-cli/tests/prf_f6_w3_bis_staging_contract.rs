//! PRF-F6-W3-bis: shared staging contract between release.yml and
//! release-validate.yml.
//!
//! Closes the bug surfaced by Actions run #35991553492: the upload
//! step in `release-validate.yml` named its artifacts
//! `validate-payloads-${{ matrix.platform }}`, so after
//! `download-artifact` the staging tree contained
//! `staging/validate-payloads-linux-x86-64/...` directories that
//! the flatten script did not know how to consume. The flatten
//! script expected `payloads-<target-triple>/` directories (the
//! contract that `release.yml` already produced); the validate
//! workflow had silently diverged from the production contract.
//!
//! This test pins the corrected contract in three layers:
//!
//! 1. **Workflow contract**: parse `release.yml` and
//!    `release-validate.yml` and assert that every
//!    `actions/upload-artifact` uses the same `payloads-*` name
//!    pattern and every `actions/download-artifact` uses
//!    `pattern: payloads-*`. This prevents future divergences
//!    between the two workflows.
//!
//! 2. **Flatten script contract**: execute the actual
//!    `stage-platform-payloads.sh` against a synthetic staging
//!    tree that matches what the workflow now produces
//!    (`staging/payloads-linux-x86-64/dist/cogh-*-x86_64-unknown-linux-gnu.tar.gz`).
//!    The script must map the short platform identifier to the
//!    target triple internally without a substitution hack and
//!    produce a valid flattened staging tree.
//!
//! 3. **Negative cases**: unknown platform identifier, duplicate
//!    lane claiming the same triple, missing Tier-1 platform,
//!    short-form + triple-alias collision, and wrong-triple
//!    tarball in a lane. All must be rejected loudly.
//!
//! The test uses the **same short platform identifier** that the
//! workflow injects (`matrix.platform == linux-x86-64` and
//! `linux-aarch64`). Older tests built the layout by hand with the
//! target triple as the lane name; that hand-built layout must
//! continue to work via an explicit alias accepted by the script,
//! so this test also exercises the alias path for backwards
//! compatibility with existing fixtures.

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop();
    p.pop();
    p
}

fn release_validate_yml() -> PathBuf {
    repo_root().join(".github/workflows/release-validate.yml")
}

fn release_yml() -> PathBuf {
    repo_root().join(".github/workflows/release.yml")
}

fn flatten_script() -> PathBuf {
    repo_root().join("scripts/ci/stage-platform-payloads.sh")
}

/// Tier-1 platform short identifiers, matching `matrix.platform` in
/// the workflows' strategy matrix.
const TIER1_SHORT: &[&str] = &["linux-x86-64", "linux-aarch64"];
/// Tier-1 platform target triples, matching the tarball + SBOM
/// filename suffix produced by `cognicode-release name`.
const TIER1_TRIPLES: &[&str] = &["x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu"];
const PLATFORM_COMPONENTS: &[&str] = &["cogh", "cognicode", "cognicode-mcp"];
const SKILL_BUNDLES: &[&str] = &["cognicode", "cognicode-mcp"];

/// Read the workflow file as text and return all `actions/upload-artifact@v4`
/// `name:` lines (the ones under the `with:` block), scoped to a job
/// name (matches the `  <job>:` key). `None` returns every upload name.
fn upload_names_in(workflow: &Path, job: Option<&str>) -> Vec<String> {
    let text = std::fs::read_to_string(workflow)
        .unwrap_or_else(|e| panic!("read {}: {e}", workflow.display()));
    let mut out = Vec::new();
    let mut current_job: Option<String> = None;
    let target_job = job.map(|s| s.to_string());
    let mut in_target_upload = false;
    for line in text.lines() {
        let leading = line.len() - line.trim_start().len();
        // Job declarations are two-space-indented identifiers ending with ':'.
        if leading == 2 {
            let bare = line.trim();
            if let Some(name) = bare.strip_suffix(':') {
                if !name.contains(' ') && !name.starts_with('#') {
                    current_job = Some(name.to_string());
                }
            }
        }
        let trimmed = line.trim_start();
        let job_match = target_job
            .as_ref()
            .map_or(true, |t| current_job.as_deref() == Some(t.as_str()));

        if trimmed.starts_with("uses: actions/upload-artifact") && job_match {
            in_target_upload = true;
            continue;
        }
        if in_target_upload {
            // Only accept `name:` lines that live inside the `with:` block.
            // `name:` at job or step level (e.g. `    name: assemble-and-verify-local`)
            // is ignored because the leading indent is < 8.
            if trimmed.starts_with("name:") && leading >= 8 {
                let value = trimmed.trim_start_matches("name:").trim();
                out.push(value.to_string());
            }
            // Exit when we leave the upload step (a step boundary at indent 6).
            if trimmed.starts_with("- ") && leading <= 6 {
                in_target_upload = false;
            }
            // Also exit when we hit a non-`with:` line at the same or lower indent.
            if !trimmed.is_empty()
                && !trimmed.starts_with("name:")
                && !trimmed.starts_with("path:")
                && !trimmed.starts_with("if-")
                && !trimmed.starts_with("#")
                && !trimmed.starts_with("with:")
                && !trimmed.starts_with("uses:")
                && leading <= 8
            {
                in_target_upload = false;
            }
        }
        let _ = &mut in_target_upload; // suppress unused warning
    }
    out
}

/// Read the workflow file and return all `actions/download-artifact@v4`
/// `pattern:` lines (inside the `with:` block), scoped to a job name.
fn download_patterns_in(workflow: &Path, job: Option<&str>) -> Vec<String> {
    let text = std::fs::read_to_string(workflow)
        .unwrap_or_else(|e| panic!("read {}: {e}", workflow.display()));
    let mut out = Vec::new();
    let mut current_job: Option<String> = None;
    let target_job = job.map(|s| s.to_string());
    let mut in_target_download = false;
    for line in text.lines() {
        let leading = line.len() - line.trim_start().len();
        if leading == 2 {
            let bare = line.trim();
            if let Some(name) = bare.strip_suffix(':') {
                if !name.contains(' ') && !name.starts_with('#') {
                    current_job = Some(name.to_string());
                }
            }
        }
        let trimmed = line.trim_start();
        let job_match = target_job
            .as_ref()
            .map_or(true, |t| current_job.as_deref() == Some(t.as_str()));

        if trimmed.starts_with("uses: actions/download-artifact") && job_match {
            in_target_download = true;
            continue;
        }
        if in_target_download {
            if trimmed.starts_with("pattern:") && leading >= 8 {
                let value = trimmed.trim_start_matches("pattern:").trim();
                out.push(value.to_string());
            }
            if trimmed.starts_with("- ") && leading <= 6 {
                in_target_download = false;
            }
            if !trimmed.is_empty()
                && !trimmed.starts_with("pattern:")
                && !trimmed.starts_with("path:")
                && !trimmed.starts_with("merge-")
                && !trimmed.starts_with("if-")
                && !trimmed.starts_with("#")
                && !trimmed.starts_with("with:")
                && !trimmed.starts_with("uses:")
                && leading <= 8
            {
                in_target_download = false;
            }
        }
    }
    out
}

/// Reproduce the layout that `actions/download-artifact@v4` produces
/// after the corrected contract: each lane directory is named
/// `payloads-<short-platform-id>/` (e.g. `payloads-linux-x86-64/`),
/// and inside it the build job has uploaded tarballs with the
/// canonical target triple and matching SBOMs.
fn reproduce_workflow_layout(staging: &Path, use_short: bool) {
    use std::fs;
    fs::create_dir_all(staging).unwrap();
    for (short, triple) in TIER1_SHORT.iter().zip(TIER1_TRIPLES.iter()) {
        let lane_dir = if use_short {
            staging.join(format!("payloads-{short}"))
        } else {
            staging.join(format!("payloads-{triple}"))
        };
        fs::create_dir_all(lane_dir.join("dist")).unwrap();
        for comp in PLATFORM_COMPONENTS {
            let payload_name = format!("{comp}-{}-{triple}.tar.gz", env!("CARGO_PKG_VERSION"));
            let payload_path = lane_dir.join("dist").join(&payload_name);
            let mut content = Vec::new();
            content.extend_from_slice(b"# F6.W3-bis synth payload\n");
            content.extend_from_slice(format!("component={comp}\n").as_bytes());
            content.extend_from_slice(format!("platform={triple}\n").as_bytes());
            content.extend_from_slice(format!("short={short}\n").as_bytes());
            fs::write(&payload_path, &content).unwrap();

            let cdx_name = format!("crates/{comp}-{triple}.cdx.json");
            fs::create_dir_all(lane_dir.join("crates")).unwrap();
            fs::write(
                lane_dir.join(&cdx_name),
                br#"{"bomFormat":"CycloneDX","specVersion":"1.5","components":[]}"#,
            )
            .unwrap();
        }
    }
    // Pre-staged skill bundles (workflow stages these before the
    // flatten step).
    for b in SKILL_BUNDLES {
        let name = format!("{b}-{}.tar.gz", env!("CARGO_PKG_VERSION"));
        fs::write(staging.join(&name), b"# skill bundle synth\n").unwrap();
    }
}

fn run_flatten(staging: &Path) -> std::process::ExitStatus {
    Command::new("bash")
        .arg(flatten_script())
        .arg(staging)
        .status()
        .unwrap()
}

// -------------------------------------------------------------------
// Layer 1: workflow contract
// -------------------------------------------------------------------

/// Build jobs in both workflows must upload lane artifacts under the
/// SAME name pattern (`payloads-${{ matrix.platform }}`) so the
/// `stage-platform-payloads.sh` script can consume them without
/// branching on the workflow. Run #35991553492 failed because
/// `release-validate.yml`'s `build` job used `validate-payloads-*`
/// while `release.yml`'s `build` job used `payloads-*`.
///
/// Other jobs (e.g. the `validate` job that uploads the assembled
/// release output under a different name) are out of scope here; the
/// lane ↔ flatten contract is solely about what `build` uploads and
/// what `validate` downloads.
#[test]
fn prf_f6_w3_bis_workflow_upload_names_are_shared() {
    let rv = upload_names_in(&release_validate_yml(), Some("build"));
    let rel = upload_names_in(&release_yml(), Some("build"));
    assert!(
        !rv.is_empty(),
        "could not find any upload-artifact name in release-validate.yml#build"
    );
    assert!(
        !rel.is_empty(),
        "could not find any upload-artifact name in release.yml#build"
    );
    for name in &rv {
        // The lane artifact name must be exactly
        // `payloads-${{ matrix.platform }}` — no extra prefix like
        // `validate-payloads-` is allowed, even though both strings
        // share the suffix `payloads-${{ matrix.platform }}`.
        // Run #35991553492 failed precisely because the name was
        // `validate-payloads-${{ matrix.platform }}`.
        assert!(
            name == "payloads-${{ matrix.platform }}",
            "release-validate.yml#build upload name `{name}` must be exactly `payloads-${{ matrix.platform }}`; \
             run #35991553492 failed because this workflow diverged from release.yml"
        );
    }
    for name in &rel {
        assert_eq!(
            name, "payloads-${{ matrix.platform }}",
            "release.yml#build upload name `{name}` changed; the shared contract must be preserved"
        );
    }
}

/// The `validate` job in both workflows must download the lane
/// artifacts with the same pattern (`payloads-*`). Negative-test jobs
/// download a different artifact (the assembled release output) and
/// are out of scope for this contract.
#[test]
fn prf_f6_w3_bis_workflow_download_patterns_are_shared() {
    let rv = download_patterns_in(&release_validate_yml(), Some("validate"));
    let rel = download_patterns_in(&release_yml(), Some("release"));
    assert!(
        !rv.is_empty(),
        "could not find any download-artifact pattern in release-validate.yml#validate"
    );
    assert!(
        !rel.is_empty(),
        "could not find any download-artifact pattern in release.yml#release"
    );
    for p in &rv {
        assert_eq!(
            p, "payloads-*",
            "release-validate.yml#validate download pattern `{p}` must be exactly `payloads-*`"
        );
    }
    for p in &rel {
        assert_eq!(
            p, "payloads-*",
            "release.yml#release download pattern `{p}` changed; the shared contract must be preserved"
        );
    }
}

// -------------------------------------------------------------------
// Layer 2: flatten script happy paths
// -------------------------------------------------------------------

/// Reproduce the contract that `actions/download-artifact@v4`
/// produces after the fix (`payloads-<short-platform-id>/` lanes).
/// The script must accept the short identifier, map it to the
/// target triple internally, and flatten the staging tree
/// successfully.
#[test]
fn prf_f6_w3_bis_flatten_accepts_short_platform_ids_from_workflow() {
    use std::fs;
    let tmp = std::env::temp_dir().join(format!("prf-f6-w3bis-short-{}", std::process::id()));
    let _ = fs::remove_dir_all(&tmp);
    let staging = tmp.join("staging");
    fs::create_dir_all(&staging).unwrap();

    reproduce_workflow_layout(&staging, true);

    let status = run_flatten(&staging);
    let out = Command::new("bash")
        .arg(flatten_script())
        .arg(&staging)
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        status.success(),
        "flatten failed for short-id layout; stderr={stderr}"
    );

    // After flatten, the staging root must contain the canonical
    // 6 tarballs (3 components x 2 Tier-1 platforms) named with the
    // target triple, plus the 2 skill bundles.
    for triple in TIER1_TRIPLES {
        for comp in PLATFORM_COMPONENTS {
            let payload = staging.join(format!(
                "{comp}-{}-{triple}.tar.gz",
                env!("CARGO_PKG_VERSION")
            ));
            assert!(
                payload.exists(),
                "flatten output missing canonical payload {}",
                payload.display()
            );
            let sbom = staging.join(format!("{comp}-{triple}.cdx.json"));
            assert!(
                sbom.exists(),
                "flatten output missing SBOM {}",
                sbom.display()
            );
        }
    }
    for b in SKILL_BUNDLES {
        let bundle = staging.join(format!("{b}-{}.tar.gz", env!("CARGO_PKG_VERSION")));
        assert!(
            bundle.exists(),
            "flatten output missing skill bundle {}",
            bundle.display()
        );
    }

    let _ = fs::remove_dir_all(&tmp);
}

/// Backwards compatibility: the script must still accept the
/// triple-as-lane-name layout that pre-existing tests and fixtures
/// use, because not every reproducer is rewritten at once.
#[test]
fn prf_f6_w3_bis_flatten_accepts_triple_alias_lanes_for_existing_fixtures() {
    use std::fs;
    let tmp = std::env::temp_dir().join(format!("prf-f6-w3bis-triple-{}", std::process::id()));
    let _ = fs::remove_dir_all(&tmp);
    let staging = tmp.join("staging");
    fs::create_dir_all(&staging).unwrap();

    reproduce_workflow_layout(&staging, false);

    let status = run_flatten(&staging);
    let out = Command::new("bash")
        .arg(flatten_script())
        .arg(&staging)
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        status.success(),
        "flatten failed for triple-alias layout; stderr={stderr}"
    );

    let _ = fs::remove_dir_all(&tmp);
}

// -------------------------------------------------------------------
// Layer 3: negative cases
// -------------------------------------------------------------------

/// Unknown short platform identifier (e.g. `linux-riscv64`) must be
/// rejected loudly so a typo in `matrix.platform` cannot silently
/// reach the release factory.
#[test]
fn prf_f6_w3_bis_flatten_rejects_unknown_platform() {
    use std::fs;
    let tmp = std::env::temp_dir().join(format!("prf-f6-w3bis-unknown-{}", std::process::id()));
    let _ = fs::remove_dir_all(&tmp);
    let staging = tmp.join("staging");
    fs::create_dir_all(&staging).unwrap();

    // Single lane with an unknown platform id, but ship the
    // tarballs under that exact name anyway so the script cannot
    // mask the rejection with a missing-file error.
    let lane = staging.join("payloads-linux-riscv64");
    fs::create_dir_all(lane.join("dist")).unwrap();
    for comp in PLATFORM_COMPONENTS {
        fs::write(
            lane.join("dist")
                .join(format!("{comp}-0.0.0-riscv64gc-unknown-linux-gnu.tar.gz")),
            b"unrelated\n",
        )
        .unwrap();
        fs::create_dir_all(lane.join("crates")).unwrap();
        fs::write(
            lane.join("crates")
                .join(format!("{comp}-riscv64gc-unknown-linux-gnu.cdx.json")),
            br#"{"bomFormat":"CycloneDX","specVersion":"1.5","components":[]}"#,
        )
        .unwrap();
    }

    let out = Command::new("bash")
        .arg(flatten_script())
        .arg(&staging)
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "flatten must reject unknown platform suffix; got success, stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("unknown platform suffix") || stderr.contains("linux-riscv64"),
        "error must name the offending platform; got: {stderr}"
    );

    let _ = fs::remove_dir_all(&tmp);
}

/// Missing a Tier-1 platform must be rejected. If only the x86_64
/// lane is uploaded (e.g. a CI failure on aarch64) the flatten
/// script must not silently produce a release missing one platform.
#[test]
fn prf_f6_w3_bis_flatten_rejects_missing_tier1_platform() {
    use std::fs;
    let tmp = std::env::temp_dir().join(format!("prf-f6-w3bis-missingplat-{}", std::process::id()));
    let _ = fs::remove_dir_all(&tmp);
    let staging = tmp.join("staging");
    fs::create_dir_all(&staging).unwrap();

    // Only ship the x86_64 lane using short ids.
    let lane = staging.join("payloads-linux-x86-64");
    fs::create_dir_all(lane.join("dist")).unwrap();
    for comp in PLATFORM_COMPONENTS {
        fs::write(
            lane.join("dist").join(format!(
                "{comp}-{}-x86_64-unknown-linux-gnu.tar.gz",
                env!("CARGO_PKG_VERSION")
            )),
            b"x86_64\n",
        )
        .unwrap();
        fs::create_dir_all(lane.join("crates")).unwrap();
        fs::write(
            lane.join("crates")
                .join(format!("{comp}-x86_64-unknown-linux-gnu.cdx.json")),
            br#"{"bomFormat":"CycloneDX","specVersion":"1.5","components":[]}"#,
        )
        .unwrap();
    }

    let out = Command::new("bash")
        .arg(flatten_script())
        .arg(&staging)
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "flatten must reject a layout that omits a Tier-1 platform"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("aarch64-unknown-linux-gnu"),
        "error must reference the missing Tier-1 platform; got: {stderr}"
    );

    let _ = fs::remove_dir_all(&tmp);
}

/// A short-form lane and its target-triple alias lane together
/// must be rejected as a duplicate platform claim. Otherwise the
/// script would silently pick one and lose the other.
#[test]
fn prf_f6_w3_bis_flatten_rejects_short_and_triple_alias_collision() {
    use std::fs;
    let tmp = std::env::temp_dir().join(format!("prf-f6-w3bis-collision-{}", std::process::id()));
    let _ = fs::remove_dir_all(&tmp);
    let staging = tmp.join("staging");
    fs::create_dir_all(&staging).unwrap();

    // Short id + triple alias for x86_64; aarch64 lane only via short id.
    let short_x86 = staging.join("payloads-linux-x86-64");
    let triple_x86 = staging.join("payloads-x86_64-unknown-linux-gnu");
    let short_aarch = staging.join("payloads-linux-aarch64");
    for lane in [&short_x86, &triple_x86, &short_aarch] {
        fs::create_dir_all(lane.join("dist")).unwrap();
        fs::create_dir_all(lane.join("crates")).unwrap();
    }
    // Both x86 lanes ship the same component payload name; the
    // script must refuse to pick one over the other.
    for lane in [&short_x86, &triple_x86] {
        for comp in PLATFORM_COMPONENTS {
            fs::write(
                lane.join("dist").join(format!(
                    "{comp}-{}-x86_64-unknown-linux-gnu.tar.gz",
                    env!("CARGO_PKG_VERSION")
                )),
                b"x86_64 payload\n",
            )
            .unwrap();
            fs::write(
                lane.join("crates")
                    .join(format!("{comp}-x86_64-unknown-linux-gnu.cdx.json")),
                br#"{"bomFormat":"CycloneDX","specVersion":"1.5","components":[]}"#,
            )
            .unwrap();
        }
    }
    for comp in PLATFORM_COMPONENTS {
        fs::write(
            short_aarch.join("dist").join(format!(
                "{comp}-{}-aarch64-unknown-linux-gnu.tar.gz",
                env!("CARGO_PKG_VERSION")
            )),
            b"aarch64 payload\n",
        )
        .unwrap();
        fs::write(
            short_aarch
                .join("crates")
                .join(format!("{comp}-aarch64-unknown-linux-gnu.cdx.json")),
            br#"{"bomFormat":"CycloneDX","specVersion":"1.5","components":[]}"#,
        )
        .unwrap();
    }

    let out = Command::new("bash")
        .arg(flatten_script())
        .arg(&staging)
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "flatten must reject two lanes claiming the same platform"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("two lane directories claim the same platform"),
        "error must reference the duplicate platform claim; got: {stderr}"
    );
    assert!(
        stderr.contains("x86_64-unknown-linux-gnu"),
        "error must name the colliding platform; got: {stderr}"
    );

    let _ = fs::remove_dir_all(&tmp);
}

/// A lane whose tarball triple disagrees with the resolved lane
/// platform must be rejected. This is the cross-lane swap bug: a
/// lane named `payloads-linux-x86-64` shipping an
/// `aarch64-unknown-linux-gnu` tarball.
#[test]
fn prf_f6_w3_bis_flatten_rejects_wrong_triple_inside_lane() {
    use std::fs;
    let tmp = std::env::temp_dir().join(format!("prf-f6-w3bis-wrongtriple-{}", std::process::id()));
    let _ = fs::remove_dir_all(&tmp);
    let staging = tmp.join("staging");
    fs::create_dir_all(&staging).unwrap();

    // Lane claims x86_64 short id but ships an aarch64 tarball.
    // The find glob `${platform}.tar.gz` already filters by triple,
    // so to exercise the defensive regex we must bypass the glob
    // by stuffing the lane with a payload that does NOT match the
    // expected triple — the script then has nothing to copy and
    // reports missing payload for x86_64. The second arm of this
    // test asserts the missing-payload path is taken.
    let lane = staging.join("payloads-linux-x86-64");
    fs::create_dir_all(lane.join("dist")).unwrap();
    for comp in PLATFORM_COMPONENTS {
        fs::write(
            lane.join("dist").join(format!(
                "{comp}-{}-aarch64-unknown-linux-gnu.tar.gz",
                env!("CARGO_PKG_VERSION")
            )),
            b"swapped\n",
        )
        .unwrap();
        fs::create_dir_all(lane.join("crates")).unwrap();
        fs::write(
            lane.join("crates")
                .join(format!("{comp}-x86_64-unknown-linux-gnu.cdx.json")),
            br#"{"bomFormat":"CycloneDX","specVersion":"1.5","components":[]}"#,
        )
        .unwrap();
    }
    // Add a valid aarch64 lane so the missing-platform check does
    // not trip first.
    let aarch = staging.join("payloads-linux-aarch64");
    fs::create_dir_all(aarch.join("dist")).unwrap();
    fs::create_dir_all(aarch.join("crates")).unwrap();
    for comp in PLATFORM_COMPONENTS {
        fs::write(
            aarch.join("dist").join(format!(
                "{comp}-{}-aarch64-unknown-linux-gnu.tar.gz",
                env!("CARGO_PKG_VERSION")
            )),
            b"real aarch64\n",
        )
        .unwrap();
        fs::write(
            aarch
                .join("crates")
                .join(format!("{comp}-aarch64-unknown-linux-gnu.cdx.json")),
            br#"{"bomFormat":"CycloneDX","specVersion":"1.5","components":[]}"#,
        )
        .unwrap();
    }

    let out = Command::new("bash")
        .arg(flatten_script())
        .arg(&staging)
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "flatten must reject a lane whose tarballs do not match its declared platform"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("missing payload") || stderr.contains("x86_64-unknown-linux-gnu"),
        "error must name the offending lane + platform; got: {stderr}"
    );

    let _ = fs::remove_dir_all(&tmp);
}

/// Component-name prefix over-match bug (Actions run #35998814863):
/// when the find pattern is `${comp}-*-${platform}.tar.gz`, the glob
/// `cognicode-*-x86_64-unknown-linux-gnu.tar.gz` matches BOTH
/// `cognicode-0.97.5-...tar.gz` AND `cognicode-mcp-0.97.5-...tar.gz`
/// because `cognicode-mcp` starts with the `cognicode` prefix. With
/// `-print -quit` find then returns the alphabetically (or
/// filesystem-order) first match; whichever it is, the script
/// mis-attributes it to whichever component it is currently
/// iterating. The next iteration for the OTHER component (e.g.
/// `cognicode-mcp`) then either hits the same file again (duplicate
/// error in CI run #3) or silently copies the wrong file as the
/// right component's payload (a silent corruption that the
/// duplicate path happened to surface in run #3).
///
/// The contract fix is to anchor the find pattern with `[0-9]`
/// (semver always starts with a digit) so that `cognicode-mcp`
/// cannot satisfy `cognicode-[0-9]*-`. This test pins the
/// disambiguation by removing the conflicting sibling: a lane that
/// ships `cognicode-mcp-...tar.gz` but NOT `cognicode-...tar.gz`
/// must produce a "missing payload for cognicode" error from the
/// flatten script. Before the fix the script silently accepted
/// `cognicode-mcp-...tar.gz` as `cognicode`'s payload, which is a
/// silent data corruption: the resulting `cognicode-0.97.5-...tar.gz`
/// at the staging root is a copy of the MCP server binary.
#[test]
fn prf_f6_w3_bis_flatten_rejects_cognicode_overmatch_into_mcp() {
    use std::fs;
    let tmp = std::env::temp_dir().join(format!("prf-f6-w3bis-overmatch-{}", std::process::id()));
    let _ = fs::remove_dir_all(&tmp);
    let staging = tmp.join("staging");
    fs::create_dir_all(&staging).unwrap();

    let version = env!("CARGO_PKG_VERSION");
    // Build a single-lane staging tree that ships cogh and
    // cognicode-mcp tarballs but NOT a plain cognicode tarball.
    // After the fix the flatten script MUST report that
    // cognicode's payload is missing; before the fix it silently
    // consumed cognicode-mcp's tarball as cognicode's.
    let lane = staging.join("payloads-linux-x86-64");
    fs::create_dir_all(lane.join("dist")).unwrap();
    fs::create_dir_all(lane.join("crates")).unwrap();
    for comp in ["cogh", "cognicode-mcp"] {
        fs::write(
            lane.join("dist")
                .join(format!("{comp}-{version}-x86_64-unknown-linux-gnu.tar.gz")),
            format!("payload {comp}\n").as_bytes(),
        )
        .unwrap();
        fs::write(
            lane.join("crates")
                .join(format!("{comp}-x86_64-unknown-linux-gnu.cdx.json")),
            br#"{"bomFormat":"CycloneDX","specVersion":"1.5","components":[]}"#,
        )
        .unwrap();
    }
    // Add a valid aarch64 lane so the missing-platform check does
    // not trip first.
    let aarch = staging.join("payloads-linux-aarch64");
    fs::create_dir_all(aarch.join("dist")).unwrap();
    fs::create_dir_all(aarch.join("crates")).unwrap();
    for comp in ["cogh", "cognicode", "cognicode-mcp"] {
        fs::write(
            aarch
                .join("dist")
                .join(format!("{comp}-{version}-aarch64-unknown-linux-gnu.tar.gz")),
            format!("payload {comp} aarch64\n").as_bytes(),
        )
        .unwrap();
        fs::write(
            aarch
                .join("crates")
                .join(format!("{comp}-aarch64-unknown-linux-gnu.cdx.json")),
            br#"{"bomFormat":"CycloneDX","specVersion":"1.5","components":[]}"#,
        )
        .unwrap();
    }

    let out = Command::new("bash")
        .arg(flatten_script())
        .arg(&staging)
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "flatten must reject a lane whose `cognicode` payload is missing \
         (it must NOT silently consume `cognicode-mcp-*.tar.gz` as the \
         `cognicode` payload)"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("missing payload for component cognicode"),
        "error must report that cognicode's payload is missing; got: {stderr}"
    );

    let _ = fs::remove_dir_all(&tmp);
}

/// Companion test that pins the symmetric case: a lane that ships
/// `cognicode-...tar.gz` but NOT `cognicode-mcp-...tar.gz` must also
/// be rejected with a "missing payload for cognicode-mcp" error
/// (the same over-match in the other direction). This guards
/// against future regressions where a developer "fixes" the bug
/// only for the cognicode side.
#[test]
fn prf_f6_w3_bis_flatten_rejects_cognicode_mcp_overmatch_into_cognicode() {
    use std::fs;
    let tmp =
        std::env::temp_dir().join(format!("prf-f6-w3bis-overmatch-rev-{}", std::process::id()));
    let _ = fs::remove_dir_all(&tmp);
    let staging = tmp.join("staging");
    fs::create_dir_all(&staging).unwrap();

    let version = env!("CARGO_PKG_VERSION");
    let lane = staging.join("payloads-linux-x86-64");
    fs::create_dir_all(lane.join("dist")).unwrap();
    fs::create_dir_all(lane.join("crates")).unwrap();
    for comp in ["cogh", "cognicode"] {
        fs::write(
            lane.join("dist")
                .join(format!("{comp}-{version}-x86_64-unknown-linux-gnu.tar.gz")),
            format!("payload {comp}\n").as_bytes(),
        )
        .unwrap();
        fs::write(
            lane.join("crates")
                .join(format!("{comp}-x86_64-unknown-linux-gnu.cdx.json")),
            br#"{"bomFormat":"CycloneDX","specVersion":"1.5","components":[]}"#,
        )
        .unwrap();
    }
    let aarch = staging.join("payloads-linux-aarch64");
    fs::create_dir_all(aarch.join("dist")).unwrap();
    fs::create_dir_all(aarch.join("crates")).unwrap();
    for comp in ["cogh", "cognicode", "cognicode-mcp"] {
        fs::write(
            aarch
                .join("dist")
                .join(format!("{comp}-{version}-aarch64-unknown-linux-gnu.tar.gz")),
            format!("payload {comp} aarch64\n").as_bytes(),
        )
        .unwrap();
        fs::write(
            aarch
                .join("crates")
                .join(format!("{comp}-aarch64-unknown-linux-gnu.cdx.json")),
            br#"{"bomFormat":"CycloneDX","specVersion":"1.5","components":[]}"#,
        )
        .unwrap();
    }

    let out = Command::new("bash")
        .arg(flatten_script())
        .arg(&staging)
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "flatten must reject a lane whose `cognicode-mcp` payload is missing"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("missing payload for component cognicode-mcp"),
        "error must report that cognicode-mcp's payload is missing; got: {stderr}"
    );

    let _ = fs::remove_dir_all(&tmp);
}

/// Static guard: the flatten script's tarball discovery must NOT
/// use the bare `${comp}-*-${platform}.tar.gz` glob in its `find`
/// invocation, because that glob is greedy in the wrong direction
/// and causes the cognicode-vs-cognicode-mcp over-match described
/// above. The semver version always starts with a digit, so the
/// find pattern must anchor with `[0-9]` right after the component
/// stem.
#[test]
fn prf_f6_w3_bis_flatten_find_pattern_uses_digit_anchor() {
    let text = std::fs::read_to_string(flatten_script()).expect("read flatten script");
    let needle = r#"-name "${comp}-[0-9]*-${platform}.tar.gz""#;
    assert!(
        text.contains(needle),
        "flatten script must use a digit-anchored find pattern ({needle:?}) \
         to prevent the cognicode-vs-cognicode-mcp prefix over-match \
         that broke Actions run #35998814863"
    );
    // And it must NOT still carry the old buggy glob in any
    // actual `find` invocation. Bare `${comp}-*-${platform}` is
    // still acceptable in error messages (so users see what was
    // looked for), but the find line itself must be digit-anchored.
    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') {
            continue;
        }
        if !line.contains("find ") {
            continue;
        }
        assert!(
            !line.contains(r#"${comp}-*-"#),
            "flatten script must not invoke `find` with the bare \
             `${{comp}}-*-${{platform}}.tar.gz` glob (line: {line:?}); \
             use `${{comp}}-[0-9]*-${{platform}}.tar.gz` instead"
        );
    }
}

/// Round-5 hardening: the **sanity check** at the end of the flatten
/// script (line ~250, `compgen -G "${STAGING}/${comp}-*-${platform}.tar.gz"`)
/// has the same over-match bug as the find pattern had: `cognicode-*-X`
/// matches `cognicode-mcp-X` too. In the normal flow the bug is masked
/// by `copy_unique` having already copied both tarballs, but the sanity
/// check is the **second line of defense** — if `copy_unique` ever
/// regresses or a future refactor changes the population order, the
/// sanity check must NOT silently green-light a staging set that is
/// missing the `cognicode` payload. This test pins the compgen pattern
/// to the digit anchor so any future regression is caught statically.
#[test]
fn prf_f6_w3_bis_flatten_compgen_sanity_check_uses_digit_anchor() {
    let text = std::fs::read_to_string(flatten_script()).expect("read flatten script");

    // The fix: the compgen sanity check must use `[0-9]` after the
    // component stem, just like the find pattern above.
    let anchored_compgen = r#"compgen -G "${STAGING}/${comp}-[0-9]*-${platform}.tar.gz""#;
    assert!(
        text.contains(anchored_compgen),
        "flatten script must use a digit-anchored compgen pattern in its \
         final sanity check ({anchored_compgen:?}); the bare \
         `${{comp}}-*-${{platform}}.tar.gz` glob over-matches `cognicode-mcp` \
         for `cognicode` and silently returns success when the cognicode \
         payload is missing"
    );

    // And the bare glob must NOT appear inside any compgen invocation.
    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') {
            continue;
        }
        if !line.contains("compgen ") {
            continue;
        }
        assert!(
            !line.contains(r#"${comp}-*-"#),
            "flatten script must not invoke `compgen` with the bare \
             `${{comp}}-*-${{platform}}.tar.gz` glob (line: {line:?}); \
             use `${{comp}}-[0-9]*-${{platform}}.tar.gz` instead"
        );
    }
}

/// Round-5 dynamic regression test: pin the bash `compgen -G` glob
/// semantics so the assumption underlying the digit-anchor fix stays
/// true.
///
/// The flatten script's final sanity check uses
/// `compgen -G "${STAGING}/${comp}-*-${platform}.tar.gz"`. If bash's
/// `compgen -G` semantics ever changed so that `cognicode-*-X.tar.gz`
/// did NOT match `cognicode-mcp-0.97.5-X.tar.gz`, the digit-anchor fix
/// would no longer be necessary. Conversely, if the semantics changed
/// in a way that made even `cognicode-[0-9]*-X.tar.gz` over-match
/// `cognicode-mcp-0.97.5-X.tar.gz`, the digit-anchor fix would stop
/// working silently.
///
/// This test pins the **specific semantic property we depend on**:
/// in this bash version, `cognicode-*-X.tar.gz` over-matches
/// `cognicode-mcp-0.97.5-X.tar.gz` (false positive), AND
/// `cognicode-[0-9]*-X.tar.gz` does NOT (true negative). If bash
/// behavior ever diverges, the test fails and forces an explicit
/// review of the fix.
///
/// We synthesize the post-copy-unique state directly: a staging root
/// where only `cognicode-mcp-X-X.tar.gz` exists, plus both SBOMs. We
/// cannot reach this state through the `stage-platform-payloads.sh`
/// entry point because `copy_unique` would refuse it earlier, so we
/// evaluate the compgen pattern against that root in isolation.
#[test]
fn prf_f6_w3_bis_flatten_compgen_pattern_rejects_cognicode_missing_with_mcp_present() {
    use std::process::Command;

    let staging = tempdir_in_target("compgen-false-positive");
    // Stage the post-copy-unique state with cognicode TAR missing
    // but cognicode-mcp TAR present. SBOMs are irrelevant for the
    // compgen check (which targets `.tar.gz`), but include them so
    // the layout looks like a real post-flatten staging root.
    std::fs::write(
        staging.join("cognicode-mcp-0.97.5-x86_64-unknown-linux-gnu.tar.gz"),
        b"",
    )
    .unwrap();
    std::fs::write(
        staging.join("cogh-0.97.5-x86_64-unknown-linux-gnu.tar.gz"),
        b"",
    )
    .unwrap();
    std::fs::write(
        staging.join("cognicode-x86_64-unknown-linux-gnu.cdx.json"),
        b"{}",
    )
    .unwrap();
    std::fs::write(
        staging.join("cognicode-mcp-x86_64-unknown-linux-gnu.cdx.json"),
        b"{}",
    )
    .unwrap();
    std::fs::write(
        staging.join("cogh-x86_64-unknown-linux-gnu.cdx.json"),
        b"{}",
    )
    .unwrap();

    let staging_str = staging.display().to_string();
    // Bare glob (the bug): `cognicode-*-X.tar.gz` matches
    // `cognicode-mcp-0.97.5-X.tar.gz`, so compgen exits 0 falsely.
    let buggy_status = Command::new("bash")
        .arg("-c")
        .arg(format!(
            "compgen -G '{staging_str}/cognicode-*-x86_64-unknown-linux-gnu.tar.gz' \
             '{staging_str}' > /dev/null"
        ))
        .status()
        .expect("spawn bash for buggy compgen check");
    assert!(
        buggy_status.success(),
        "this test is meaningful only if the BARE glob produces a false \
         positive (i.e. matches cognicode-mcp). If bash's compgen semantics \
         changed and it now returns failure, the digit-anchor fix in the \
         flatten script may no longer be necessary — review the fix and \
         decide whether to remove it."
    );

    // Anchored glob (the fix): `cognicode-[0-9]*-X.tar.gz` does NOT
    // match `cognicode-mcp-0.97.5-X.tar.gz` (the digit anchor requires
    // the version token to be right after the component stem, so a
    // sibling component whose name shares a prefix cannot satisfy
    // the pattern). compgen exits 1.
    let fixed_status = Command::new("bash")
        .arg("-c")
        .arg(format!(
            "compgen -G '{staging_str}/cognicode-[0-9]*-x86_64-unknown-linux-gnu.tar.gz' \
             '{staging_str}' > /dev/null"
        ))
        .status()
        .expect("spawn bash for fixed compgen check");
    assert!(
        !fixed_status.success(),
        "the digit-anchored compgen glob MUST return failure when only \
         `cognicode-mcp-X-X.tar.gz` is present; if it returns success the \
         over-match is back, which means the fix no longer protects the \
         sanity check"
    );

    std::fs::remove_dir_all(&staging).ok();
}

/// Helper: create a unique tempdir under `target/` (the workspace's
/// cargo build dir, gitignored). Returns the absolute path.
fn tempdir_in_target(label: &str) -> std::path::PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = repo_root()
        .join("target")
        .join(format!("prf-f6-w3-bis-compgen-{label}-{stamp}"));
    std::fs::create_dir_all(&dir).expect("create tempdir");
    dir
}
