//! Ownership boundary for the first `sandbox_core` extraction slice.
//!
//! Resource measurement belongs to the sandbox executable, not the analysis
//! core. This test is intentionally structural: it prevents the two crates
//! from carrying duplicate owners for the same resource contract.

use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn resource_module_is_owned_by_cognicode_sandbox() {
    let root = workspace_root();
    assert!(
        !root
            .join("crates/cognicode-core/src/sandbox_core/resource.rs")
            .exists(),
        "resource.rs must not remain in cognicode-core"
    );
    assert!(
        root.join("crates/cognicode-sandbox/src/resource.rs")
            .is_file(),
        "resource.rs must be present in cognicode-sandbox"
    );
}
