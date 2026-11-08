//! The MCP cancellation registry belongs to the MCP transport crate.

use std::fs;
use std::path::Path;

#[test]
fn cancellation_registry_is_owned_by_cognicode_mcp() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let core_mcp = manifest.join("src/interface/mcp");
    let declarations =
        fs::read_to_string(core_mcp.join("mod.rs")).expect("read core MCP module declarations");

    assert!(
        !core_mcp.join("cancellation.rs").exists(),
        "cancellation.rs still lives in cognicode-core"
    );
    assert!(
        !declarations.contains("pub mod cancellation;"),
        "cognicode-core still exposes the MCP cancellation registry"
    );
    assert!(
        manifest
            .join("../cognicode-mcp/src/cancellation.rs")
            .is_file(),
        "cancellation registry is not owned by cognicode-mcp"
    );
}
