//! MCP status classification belongs to the MCP adapter crate.

use std::fs;
use std::path::Path;

#[test]
fn status_classification_is_owned_by_cognicode_mcp() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let core_mcp = manifest.join("src/interface/mcp");
    let declarations = fs::read_to_string(core_mcp.join("mod.rs"))
        .expect("read core MCP module declarations");

    assert!(
        !core_mcp.join("status.rs").exists(),
        "status.rs still lives in cognicode-core"
    );
    assert!(
        !declarations.contains("pub mod status;"),
        "cognicode-core still exposes MCP status classification"
    );
    assert!(
        manifest
            .join("../cognicode-mcp/src/status.rs")
            .is_file(),
        "status classification is not owned by cognicode-mcp"
    );
}
