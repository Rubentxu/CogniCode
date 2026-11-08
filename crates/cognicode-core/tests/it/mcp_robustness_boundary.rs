//! The MCP robustness surface belongs to `cognicode-mcp`, not the core.

use std::fs;
use std::path::Path;

#[test]
fn robustness_is_owned_by_cognicode_mcp() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let core_mcp = manifest.join("src/interface/mcp");
    let declarations =
        fs::read_to_string(core_mcp.join("mod.rs")).expect("read core MCP module declarations");

    assert!(
        !core_mcp.join("robustness.rs").exists(),
        "robustness.rs still lives in cognicode-core"
    );
    assert!(
        !declarations.contains("pub mod robustness;"),
        "cognicode-core still exposes MCP robustness"
    );
    assert!(
        manifest
            .join("../cognicode-mcp/src/robustness.rs")
            .is_file(),
        "robustness is not owned by cognicode-mcp"
    );
}
