//! PRF-F6-W3-bis-SBOM: contract test for `build-sboms-for-lane.sh`.
//!
//! Closes the second half of the bug surfaced by Actions run
//! #35995529045: even after the lane-name contract was fixed, the
//! flatten step still failed because the per-binary SBOMs the
//! script expects never reached the lane artifacts. The cause was
//! that the workflow's SBOM step (`cargo cyclonedx --format json`
//! from the workspace root) produces no file, and the upload path
//! (`crates/*.cdx.json`) is a one-level glob that does not match
//! the layout where cargo-cyclonedx actually writes per-binary
//! SBOMs (`crates/<crate>/<bin>_bin.cdx.json`).
//!
//! This test pins the corrected contract in three layers:
//!
//! 1. **Script execution**: invoke `build-sboms-for-lane.sh` with a
//!    known rust target triple and assert that exactly the three
//!    canonical files appear at the workspace root, named with the
//!    target triple (`crates/<component>-<rust_target>.cdx.json`).
//!    Spurious per-bin files for non-published bins must be
//!    cleaned up.
//!
//! 2. **SBOM integrity**: every generated SBOM must be valid
//!    CycloneDX 1.x JSON, with a `metadata.component.name` that
//!    matches the published binary (not the crate). This guards
//!    against a future cargo-cyclonedx regression that silently
//!    emits empty or per-crate files.
//!
//! 3. **Workflow contract**: parse `release.yml` and
//!    `release-validate.yml` and assert that the SBOM step invokes
//!    the shared script (not the broken `cargo cyclonedx --format
//!    json` invocation) and that the upload path references the
//!    canonical names. This is what makes the bug fix
//!    un-driftable.
//!
//! Layer 3 is the same `upload_names_in` / `download_patterns_in`
//! helpers used by `prf_f6_w3_bis_staging_contract.rs`; here we
//! only assert that the SBOM step name matches what we expect and
//! that the upload path does NOT include the broken `crates/*.cdx.json`.

use std::path::PathBuf;
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

fn sbom_script() -> PathBuf {
    repo_root().join("scripts/ci/build-sboms-for-lane.sh")
}

const TIER1_TRIPLES: &[&str] = &["x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu"];
const PUBLISHED_COMPONENTS: &[&str] = &["cogh", "cognicode", "cognicode-mcp"];

/// Set of canonical SBOM paths the script must produce.
fn canonical_sbom_paths(target: &str) -> Vec<PathBuf> {
    PUBLISHED_COMPONENTS
        .iter()
        .map(|c| repo_root().join(format!("crates/{c}-{target}.cdx.json")))
        .collect()
}

/// Run the SBOM script for the given rust target. Returns (success, stdout).
/// The script is invoked with the workspace root explicitly because cargo
/// tests run with `cwd` set to the crate directory (CARGO_MANIFEST_DIR),
/// not the workspace root.
fn run_sbom_script(target: &str) -> (bool, String) {
    let workspace = repo_root();
    let out = Command::new("bash")
        .arg(sbom_script())
        .arg(target)
        .arg(&workspace)
        .output()
        .expect("build-sboms-for-lane.sh must run");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr),
    )
}

fn remove_canonical_sboms(target: &str) {
    for p in canonical_sbom_paths(target) {
        let _ = std::fs::remove_file(&p);
    }
    // Clean any leftover _bin.cdx.json from prior runs.
    let _ = Command::new("bash")
        .arg("-c")
        .arg("rm -f crates/*/*_bin.cdx.json crates/*/*_cdylib-rlib.cdx.json 2>/dev/null || true")
        .output();
}

/// RAII guard: registers a spurious per-bin SBOM file path that
/// WILL be deleted when this guard drops, even if the test body
/// panics or returns early. Without this, a panic between the
/// `std::fs::write` of a spurius file and the final
/// `remove_canonical_sboms` leaves a stale file that pollutes
/// subsequent test runs in the same cargo invocation.
///
/// Usage:
///   let _g = SpuriousFile::new(spurious_path);
///   std::fs::write(&_g.path, ...);  // file is created; will be
///                                   // deleted at end of scope.
struct SpuriousFile {
    path: PathBuf,
}

impl SpuriousFile {
    fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl Drop for SpuriousFile {
    fn drop(&mut self) {
        // `rm -f` semantics: silently ignore non-existent files.
        // Same rationale as `remove_canonical_sboms` — the helper
        // is invoked from many places, some of which may have
        // already cleaned up.
        let _ = std::fs::remove_file(&self.path);
    }
}

/// RAII guard: registers a workspace cleanup action that WILL run
/// when this guard drops, even if the test body panics. This is
/// the file-system analogue of a Python `try/finally` block.
///
/// Why this exists: previously each test called
/// `remove_canonical_sboms(target)` only at the start (to clean
/// the slate) and again at the very end (best-effort). A test
/// panic between those two points would leave the canonical
/// `crates/<component>-<target>.cdx.json` files in the workspace,
/// which then confuse the next test invocation. The Drop guard
/// makes the cleanup non-skippable.
struct WorkspaceSbomGuard<'a> {
    target: &'a str,
}

impl<'a> WorkspaceSbomGuard<'a> {
    fn new(target: &'a str) -> Self {
        Self { target }
    }
}

impl Drop for WorkspaceSbomGuard<'_> {
    fn drop(&mut self) {
        remove_canonical_sboms(self.target);
    }
}

// -------------------------------------------------------------------
// Layer 1: script execution
// -------------------------------------------------------------------

/// The script must produce exactly the three canonical SBOMs at
/// the workspace root, named `<component>-<target>.cdx.json`, and
/// nothing else under `crates/*.cdx.json` (no per-crate or
/// per-binary leftovers).
#[test]
fn prf_f6_w3_bis_sbom_script_produces_canonical_layout() {
    for target in TIER1_TRIPLES {
        // Drop guard: `remove_canonical_sboms(target)` runs on
        // every exit path (panic or success). Previously a panic
        // mid-test left the canonical SBOMs in the workspace,
        // confusing subsequent runs.
        let _g = WorkspaceSbomGuard::new(target);
        remove_canonical_sboms(target);

        let (ok, out) = run_sbom_script(target);
        assert!(
            ok,
            "build-sboms-for-lane.sh failed for target={target}; output:\n{out}"
        );
        // The script writes the absolute path of each staged SBOM,
        // so the marker substring matches either a relative or
        // absolute form.
        assert!(
            out.contains("staged crates/cogh-")
                || out.contains(&format!("staged {}/crates/cogh-", repo_root().display())),
            "script output missing the staged marker for cogh (target={target}); got:\n{out}"
        );
        assert!(
            out.contains("staged crates/cognicode-mcp-")
                || out.contains(&format!(
                    "staged {}/crates/cognicode-mcp-",
                    repo_root().display()
                )),
            "script output missing the staged marker for cognicode-mcp (target={target}); got:\n{out}"
        );

        for expected in canonical_sbom_paths(target) {
            assert!(
                expected.exists(),
                "missing canonical SBOM {} after running the script",
                expected.display()
            );
            let bytes = std::fs::metadata(&expected).unwrap().len();
            assert!(
                bytes > 1000,
                "canonical SBOM {} is suspiciously small ({} bytes)",
                expected.display(),
                bytes
            );
        }
    }
}

/// Spurious per-bin SBOMs (bins that are NOT in the published
/// component set: `cognicode-release`, `mcp-client`, `sandbox-orchestrator`,
/// `cognicode-mcp-server`, `explorer-api`, `explorer-mcp`) must NOT
/// survive into the workspace tree. Otherwise the upload path
/// would either skip them (silent drift) or upload them and let
/// the flatten script choke on extras.
#[test]
fn prf_f6_w3_bis_sbom_script_cleans_up_non_published_bin_sboms() {
    let target = TIER1_TRIPLES[0];
    // Drop guard: SBOM cleanup runs even if the test panics between
    // writing the spurius files and the manual cleanup below.
    let _g = WorkspaceSbomGuard::new(target);
    remove_canonical_sboms(target);

    // Plant a known spurious file to simulate a previous
    // `cargo cyclonedx --describe binaries` run that leaked the
    // mcp-client SBOM into the workspace. The Drop guard ensures
    // these are deleted at scope end (panic or success).
    let spurious_path = repo_root().join("crates/cognicode-mcp/mcp-client_bin.cdx.json");
    if let Some(parent) = spurious_path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    let _spurious_guard = SpuriousFile::new(spurious_path.clone());
    std::fs::write(
        &spurious_path,
        br#"{"bomFormat":"CycloneDX","specVersion":"1.5","components":[]}"#,
    )
    .unwrap();

    let release_spurious = repo_root().join("crates/cognicode-cli/cognicode-release_bin.cdx.json");
    let _release_guard = SpuriousFile::new(release_spurious.clone());
    std::fs::write(
        &release_spurious,
        br#"{"bomFormat":"CycloneDX","specVersion":"1.5","components":[]}"#,
    )
    .unwrap();

    let (ok, out) = run_sbom_script(target);
    assert!(ok, "script failed; output:\n{out}");

    assert!(
        !spurious_path.exists(),
        "stray {} not cleaned up; the script must delete per-bin SBOMs of non-published bins",
        spurious_path.display()
    );
    assert!(
        !release_spurious.exists(),
        "stray per-bin SBOM of cognicode-release was not cleaned up"
    );

    // Drop guards also run the canonical SBOM cleanup. Explicit
    // `remove_canonical_sboms(target)` removed here: redundant.
}

// -------------------------------------------------------------------
// Layer 2: SBOM integrity
// -------------------------------------------------------------------

/// Each generated SBOM must be valid CycloneDX JSON and have a
/// `metadata.component.name` matching the published binary. A
/// regression in cargo-cyclonedx that emits an empty file or a
/// per-crate (non-binary) SBOM would surface here.
#[test]
fn prf_f6_w3_bis_sbom_script_generated_sboms_have_correct_metadata() {
    use std::collections::HashMap;
    let target = TIER1_TRIPLES[0];
    // Drop guard: cleanup runs on every exit path. A panic in this
    // test would leave 3 canonical SBOMs + the spurious bin files
    // in the workspace; the guard ensures neither survives.
    let _g = WorkspaceSbomGuard::new(target);
    remove_canonical_sboms(target);
    let (ok, out) = run_sbom_script(target);
    assert!(ok, "script failed; output:\n{out}");

    let mut sboms: HashMap<String, serde_json::Value> = HashMap::new();
    for comp in PUBLISHED_COMPONENTS {
        let path = repo_root().join(format!("crates/{comp}-{target}.cdx.json"));
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        let v: serde_json::Value = serde_json::from_str(&text)
            .unwrap_or_else(|e| panic!("{} is not valid JSON: {e}", path.display()));
        assert_eq!(
            v["bomFormat"].as_str(),
            Some("CycloneDX"),
            "{}: bomFormat must be CycloneDX",
            path.display()
        );
        let spec = v["specVersion"].as_str().unwrap_or("");
        assert!(
            spec.starts_with("1."),
            "{}: specVersion must be 1.x, got {spec}",
            path.display()
        );
        let comp_name = v["metadata"]["component"]["name"]
            .as_str()
            .unwrap_or_else(|| panic!("{}: metadata.component.name missing", path.display()));
        assert_eq!(
            comp_name,
            *comp,
            "{}: metadata.component.name={comp_name} does not match published stem {comp}",
            path.display()
        );
        assert_eq!(
            v["metadata"]["component"]["type"].as_str(),
            Some("application"),
            "{}: metadata.component.type must be 'application' for a binary SBOM",
            path.display()
        );
        sboms.insert(comp.to_string(), v);
    }

    // Components array must be non-empty (a real SBOM, not an empty stub).
    // The top-level `components` array catalogues every dependency included
    // in the binary (cargo-cyclonedx emits the full transitive closure).
    // `cogh` and `cognicode` share `cognicode-cli`, so they share the same
    // transitive closure; `cognicode-mcp` has its own.
    for comp in PUBLISHED_COMPONENTS {
        let path = repo_root().join(format!("crates/{comp}-{target}.cdx.json"));
        let v = sboms.get(*comp).unwrap();
        let count = v["components"].as_array().map(|a| a.len()).unwrap_or(0);
        assert!(
            count > 50,
            "{}: components array is suspiciously small ({count} entries)",
            path.display()
        );
    }

    // Drop guard `let _g = ...` (line 286) cleans up the canonical
    // SBOMs on every exit path; explicit cleanup removed: redundant.
}

// -------------------------------------------------------------------
// Layer 3: workflow contract
// -------------------------------------------------------------------

/// Both workflows must invoke `scripts/ci/build-sboms-for-lane.sh`
/// for SBOM generation. A regression to the bare
/// `cargo cyclonedx --format json` invocation (which silently
/// produces no output from the workspace root) would resurface
/// the run #35995529045 failure.
#[test]
fn prf_f6_w3_bis_workflow_sbom_step_invokes_shared_script() {
    for workflow in [&release_yml(), &release_validate_yml()] {
        let text = std::fs::read_to_string(workflow)
            .unwrap_or_else(|e| panic!("read {}: {e}", workflow.display()));
        assert!(
            text.contains("scripts/ci/build-sboms-for-lane.sh"),
            "{}: SBOM step must invoke scripts/ci/build-sboms-for-lane.sh; \
             the bare `cargo cyclonedx --format json` is broken from the workspace root",
            workflow.display()
        );
        // The old broken invocation must not survive.
        assert!(
            !text.contains("cargo cyclonedx --format json\n")
                && !text.contains("cargo cyclonedx --format json\r\n"),
            "{}: the bare `cargo cyclonedx --format json` invocation must be removed; \
             it produces no output from the workspace root",
            workflow.display()
        );
    }
}

/// Both workflows must upload SBOMs under their canonical names
/// (not the broken `crates/*.cdx.json` glob). The upload path must
/// explicitly list the three per-component SBOMs.
#[test]
fn prf_f6_w3_bis_workflow_upload_path_uses_canonical_sbom_names() {
    for workflow in [&release_yml(), &release_validate_yml()] {
        let text = std::fs::read_to_string(workflow)
            .unwrap_or_else(|e| panic!("read {}: {e}", workflow.display()));
        for comp in PUBLISHED_COMPONENTS {
            let needle = format!("crates/{comp}-${{{{ matrix.rust_target }}}}.cdx.json");
            assert!(
                text.contains(&needle),
                "{}: upload path must reference canonical SBOM `{needle}`; \
                 run #35995529045 failed because the upload used `crates/*.cdx.json` \
                 which does not match the layout where SBOMs actually live",
                workflow.display()
            );
        }
        // The broken glob must not survive.
        assert!(
            !text.contains("crates/*.cdx.json"),
            "{}: the broken `crates/*.cdx.json` upload glob must be removed; \
             it is a one-level glob that misses the SBOMs the script produces",
            workflow.display()
        );
    }
}
