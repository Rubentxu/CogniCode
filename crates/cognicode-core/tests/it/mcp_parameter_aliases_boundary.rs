//! Parameter alias resolution belongs to the MCP adapter crate.

use std::fs;
use std::path::Path;

#[test]
fn parameter_aliases_are_owned_by_cognicode_mcp() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let core_mcp = manifest.join("src/interface/mcp");
    let declarations =
        fs::read_to_string(core_mcp.join("mod.rs")).expect("read core MCP module declarations");

    assert!(
        !core_mcp.join("parameter_aliases.rs").exists(),
        "parameter_aliases still lives in cognicode-core"
    );
    assert!(
        !declarations.contains("pub mod parameter_aliases;"),
        "cognicode-core still exposes parameter_aliases"
    );
    assert!(
        manifest
            .join("../cognicode-mcp/src/parameter_aliases.rs")
            .is_file(),
        "parameter_aliases is not owned by cognicode-mcp"
    );
}
