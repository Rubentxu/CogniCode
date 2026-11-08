//! The first 3d.2 transport slice must not remain exposed by core.
//!
//! This is deliberately a boundary test, not a presenter behavior test:
//! presentation belongs to the MCP adapter once the slice is moved. Keeping
//! the assertion here makes the dependency direction executable before the
//! move and prevents restoring the old core module by accident.

use std::fs;
use std::path::Path;

#[test]
fn core_does_not_expose_the_presenter_transport_module() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let core_module = manifest.join("src/interface/mcp/presenter_bridge.rs");
    let module_declaration = fs::read_to_string(manifest.join("src/interface/mcp/mod.rs"))
        .expect("read core MCP module declarations");

    assert!(
        !core_module.exists(),
        "presenter transport code still lives in cognicode-core: {}",
        core_module.display()
    );
    assert!(
        !module_declaration.contains("pub mod presenter_bridge;"),
        "cognicode-core still exposes the presenter transport module"
    );

    let adapter = manifest
        .join("../cognicode-mcp/src/presenter_bridge.rs")
        .canonicalize()
        .expect("the presenter module must live in cognicode-mcp");
    assert!(adapter.is_file(), "presenter module is not in the adapter");
}
