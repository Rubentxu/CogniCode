//! Ownership boundary for resource measurement.
//!
//! Resource measurement belongs to the sandbox executable, not the analysis
//! core and not the shared `sandbox_core` library. This test is intentionally
//! structural: it prevents two modules from carrying duplicate owners for the
//! same resource contract.

use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn resource_module_is_owned_by_cognicode_sandbox() {
    let root = workspace_root();
    assert!(
        !root
            .join("crates/cognicode-sandbox/src/sandbox_core/resource.rs")
            .exists(),
        "resource.rs must not move into the sandbox_core library"
    );
    assert!(
        root.join("crates/cognicode-sandbox/src/resource.rs")
            .is_file(),
        "resource.rs must be present in cognicode-sandbox"
    );
}
