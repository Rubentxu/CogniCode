//! MCP response utilities belong to the MCP adapter crate, not the core.

use std::fs;
use std::path::Path;

#[test]
fn response_utilities_are_owned_by_cognicode_mcp() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let core_mcp = manifest.join("src/interface/mcp");
    let declarations = fs::read_to_string(core_mcp.join("mod.rs"))
        .expect("read core MCP module declarations");

    for module in ["link_trace", "tool_response"] {
        assert!(
            !core_mcp.join(format!("{module}.rs")).exists(),
            "{module} still lives in cognicode-core"
        );
        assert!(
            !declarations.contains(&format!("pub mod {module};")),
            "cognicode-core still exposes {module}"
        );
        assert!(
            manifest
                .join(format!("../cognicode-mcp/src/{module}.rs"))
                .is_file(),
            "{module} is not owned by cognicode-mcp"
        );
    }
}
