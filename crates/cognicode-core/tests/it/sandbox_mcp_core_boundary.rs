//! Ownership boundary for the sandbox MCP process adapter.
//!
//! `McpServer` belongs to the sandbox orchestrator, not to the analysis core.
//! This structural contract prevents duplicate owners during the extraction.

use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn mcp_core_is_owned_by_cognicode_sandbox() {
    let root = workspace_root();
    assert!(
        !root
            .join("crates/cognicode-core/src/sandbox_core/mcp_core.rs")
            .exists(),
        "mcp_core.rs must not remain in cognicode-core"
    );
    assert!(
        root.join("crates/cognicode-sandbox/src/mcp_core.rs")
            .is_file(),
        "mcp_core.rs must be present in cognicode-sandbox"
    );
}
