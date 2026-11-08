//! The RMCP protocol adapter belongs to `cognicode-mcp`, not the core.
//!
//! This boundary is intentionally structural: it fails while the adapter is
//! still compiled by core and passes only when the owning transport crate
//! exposes it. It does not certify the complete MCP extraction.

use std::path::Path;

#[test]
fn core_does_not_expose_the_rmcp_adapter() {
    let core_manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let core_adapter = core_manifest.join("src/interface/mcp/rmcp_adapter.rs");
    let core_module = std::fs::read_to_string(core_manifest.join("src/interface/mcp/mod.rs"))
        .expect("read core MCP module declarations");

    assert!(
        !core_adapter.exists(),
        "RMCP adapter still lives in cognicode-core: {}",
        core_adapter.display()
    );
    assert!(
        !core_module.contains("pub mod rmcp_adapter;"),
        "cognicode-core still compiles the RMCP adapter"
    );

    let mcp_adapter = core_manifest
        .join("../cognicode-mcp/src/rmcp_adapter.rs")
        .canonicalize()
        .expect("the RMCP adapter must live in cognicode-mcp");
    assert!(mcp_adapter.is_file(), "RMCP adapter is not in cognicode-mcp");
}

#[test]
fn core_manifest_has_no_transport_only_dependencies() {
    let manifest = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"),
    )
    .expect("read core Cargo manifest");

    assert!(
        !manifest.lines().any(|line| line.trim() == "rmcp.workspace = true"),
        "cognicode-core still declares the RMCP transport dependency"
    );
    assert!(
        !manifest
            .lines()
            .any(|line| line.trim() == "process-wrap.workspace = true"),
        "cognicode-core still declares the process wrapper dependency"
    );
}
