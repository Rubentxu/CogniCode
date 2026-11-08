//! MCP output-contract derivation belongs to the MCP adapter crate.

use std::fs;
use std::path::Path;

#[test]
fn output_contracts_are_owned_by_cognicode_mcp() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let core_mcp = manifest.join("src/interface/mcp");
    let declarations = fs::read_to_string(core_mcp.join("mod.rs"))
        .expect("read core MCP module declarations");

    assert!(
        !core_mcp.join("output_contracts.rs").exists(),
        "output_contracts.rs still lives in cognicode-core"
    );
    assert!(
        !declarations.contains("pub mod output_contracts;"),
        "cognicode-core still exposes MCP output contracts"
    );
    assert!(
        manifest
            .join("../cognicode-mcp/src/output_contracts.rs")
            .is_file(),
        "output contracts are not owned by cognicode-mcp"
    );
}
