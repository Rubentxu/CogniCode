//! Ownership boundary for the sandbox MCP process adapter.
//!
//! `McpServer` belongs to the sandbox orchestrator binary, not to the analysis core
//! and not to the shared `sandbox_core` library. This structural contract
//! prevents duplicate owners.

use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn mcp_core_is_owned_by_cognicode_sandbox() {
    let root = workspace_root();
    assert!(
        !root
            .join("crates/cognicode-sandbox/src/sandbox_core/mcp_core.rs")
            .exists(),
        "mcp_core.rs must not move into the sandbox_core library"
    );
    assert!(
        root.join("crates/cognicode-sandbox/src/mcp_core.rs")
            .is_file(),
        "mcp_core.rs must be present in cognicode-sandbox"
    );
}
