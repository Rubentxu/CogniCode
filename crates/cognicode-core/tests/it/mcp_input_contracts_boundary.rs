//! MCP input-contract derivation belongs to the MCP adapter crate.

use std::fs;
use std::path::Path;

#[test]
fn input_contracts_are_owned_by_cognicode_mcp() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let core_mcp = manifest.join("src/interface/mcp");
    let declarations = fs::read_to_string(core_mcp.join("mod.rs"))
        .expect("read core MCP module declarations");

    assert!(
        !core_mcp.join("input_contracts.rs").exists(),
        "input_contracts.rs still lives in cognicode-core"
    );
    assert!(
        !declarations.contains("pub mod input_contracts;"),
        "cognicode-core still exposes MCP input contracts"
    );
    assert!(
        manifest
            .join("../cognicode-mcp/src/input_contracts.rs")
            .is_file(),
        "input contracts are not owned by cognicode-mcp"
    );
}
