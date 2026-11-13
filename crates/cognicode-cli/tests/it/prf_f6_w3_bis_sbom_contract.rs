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
//! This test pins the corrected contract in two layers:
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
//! The release lane's use of the shared scripts is pinned by the staging
//! contract, not here.
//!
//! **Where it runs.** The real `cargo cyclonedx` writes an SBOM next to the
//! manifest of every workspace member, not only the crate it is started in.
//! Run against the repository itself, the contract left files in `crates/`
//! (the leftover `cognicode_graph_wasm_cdylib-rlib.cdx.json` is the one that
//! survived) and the three tests deleted each other's files whenever they ran
//! at the same time. Each test now runs the script in a private copy of the
//! workspace, so the tool still sees a real workspace and the repository sees
//! nothing.

use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::TempDir;

fn repo_root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop();
    p.pop();
    p
}

fn sbom_script() -> PathBuf {
    repo_root().join("scripts/ci/build-sboms-for-lane.sh")
}

// TIER1_PLATFORMS is x86_64-only until TF-7 provisions aarch64
// (release_contract.rs). Restore the second element when the lane
// exists. The SBOM contract is per-lane, so a single target is enough
// to pin it; a second would be a coverage gain, not a correctness
// requirement.
const TIER1_TRIPLES: &[&str] = &["x86_64-unknown-linux-gnu"];
const PUBLISHED_COMPONENTS: &[&str] = &["cogh", "cognicode", "cognicode-mcp"];

/// Directories that are build output or tooling state, never workspace
/// source: copying them would cost time and carry nothing the tool reads.
const NOT_WORKSPACE_SOURCE: &[&str] = &["target", "node_modules", ".git", "pkg"];

/// A private copy of the workspace: the manifests, the lockfile, the
/// toolchain pin and every crate. It lives in a temp dir that is removed
/// when the value drops, on every exit path, so nothing needs a cleanup
/// guard and nothing is shared with the other tests.
struct PrivateWorkspace {
    dir: TempDir,
}

impl PrivateWorkspace {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("a private workspace needs a temp dir");
        let source = repo_root();
        for file in [
            "Cargo.toml",
            "Cargo.lock",
            "rust-toolchain.toml",
            ".cargo/config.toml",
        ] {
            copy_file(&source.join(file), &dir.path().join(file));
        }
        copy_tree(&source.join("crates"), &dir.path().join("crates"));
        Self { dir }
    }

    fn root(&self) -> &Path {
        self.dir.path()
    }

    /// Set of canonical SBOM paths the script must produce.
    fn canonical_sbom_paths(&self, target: &str) -> Vec<PathBuf> {
        PUBLISHED_COMPONENTS
            .iter()
            .map(|c| self.root().join(format!("crates/{c}-{target}.cdx.json")))
            .collect()
    }

    /// Run the SBOM script for the given rust target against this copy.
    /// Returns (success, stdout + stderr).
    fn run_sbom_script(&self, target: &str) -> (bool, String) {
        let out = Command::new("bash")
            .arg(sbom_script())
            .arg(target)
            .arg(self.root())
            .output()
            .expect("build-sboms-for-lane.sh must run");
        (
            out.status.success(),
            String::from_utf8_lossy(&out.stdout).to_string()
                + &String::from_utf8_lossy(&out.stderr),
        )
    }
}

fn copy_file(from: &Path, to: &Path) {
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent)
            .unwrap_or_else(|e| panic!("create {}: {e}", parent.display()));
    }
    // `fs::copy` follows symlinks: the crates' LICENSE files are links to
    // the root licenses, and the copy gets their content.
    std::fs::copy(from, to)
        .unwrap_or_else(|e| panic!("copy {} -> {}: {e}", from.display(), to.display()));
}

/// Copy a directory tree, leaving out build output and any SBOM a previous
/// run of the tool left behind (the copy must start without them).
fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap_or_else(|e| panic!("create {}: {e}", to.display()));
    for entry in std::fs::read_dir(from).unwrap_or_else(|e| panic!("read {}: {e}", from.display()))
    {
        let entry = entry.unwrap_or_else(|e| panic!("entry of {}: {e}", from.display()));
        let name = entry.file_name();
        let name_text = name.to_string_lossy();
        let path = entry.path();
        // `metadata` (not `file_type`) so a symlink is judged by what it
        // points at.
        let meta =
            std::fs::metadata(&path).unwrap_or_else(|e| panic!("stat {}: {e}", path.display()));
        if meta.is_dir() {
            if !NOT_WORKSPACE_SOURCE.contains(&name_text.as_ref()) {
                copy_tree(&path, &to.join(&name));
            }
        } else if !name_text.ends_with(".cdx.json") && !name_text.ends_with(".cdx.xml") {
            copy_file(&path, &to.join(&name));
        }
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
        let workspace = PrivateWorkspace::new();

        let (ok, out) = workspace.run_sbom_script(target);
        assert!(
            ok,
            "build-sboms-for-lane.sh failed for target={target}; output:\n{out}"
        );

        for expected in workspace.canonical_sbom_paths(target) {
            // The script writes the absolute path of each staged SBOM.
            assert!(
                out.contains(&format!("staged {}", expected.display())),
                "script output missing the staged marker for {} (target={target}); got:\n{out}",
                expected.display()
            );
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
    let workspace = PrivateWorkspace::new();

    // Plant a known spurious file to simulate a previous
    // `cargo cyclonedx --describe binaries` run that leaked the
    // mcp-client SBOM into the workspace.
    let stub = br#"{"bomFormat":"CycloneDX","specVersion":"1.5","components":[]}"#;
    let spurious_path = workspace
        .root()
        .join("crates/cognicode-mcp/mcp-client_bin.cdx.json");
    std::fs::write(&spurious_path, stub).unwrap();
    let release_spurious = workspace
        .root()
        .join("crates/cognicode-cli/cognicode-release_bin.cdx.json");
    std::fs::write(&release_spurious, stub).unwrap();

    let (ok, out) = workspace.run_sbom_script(target);
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
    let workspace = PrivateWorkspace::new();
    let (ok, out) = workspace.run_sbom_script(target);
    assert!(ok, "script failed; output:\n{out}");

    let mut sboms: HashMap<String, serde_json::Value> = HashMap::new();
    for comp in PUBLISHED_COMPONENTS {
        let path = workspace
            .root()
            .join(format!("crates/{comp}-{target}.cdx.json"));
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
        let path = workspace
            .root()
            .join(format!("crates/{comp}-{target}.cdx.json"));
        let v = sboms.get(*comp).unwrap();
        let count = v["components"].as_array().map(|a| a.len()).unwrap_or(0);
        assert!(
            count > 50,
            "{}: components array is suspiciously small ({count} entries)",
            path.display()
        );
    }
}
