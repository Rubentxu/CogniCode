//! La feature `rig` no se compila desde ningún crate del workspace: una
//! superficie que nadie puede construir ni probar no se publica.

use std::path::Path;

const CORE: &str = env!("CARGO_MANIFEST_DIR");

#[test]
fn the_core_declares_no_rig_feature_nor_dependency() {
    let manifest = std::fs::read_to_string(Path::new(CORE).join("Cargo.toml")).unwrap();
    assert!(!manifest.contains("rig-core"), "dependencia rig-core viva");
    assert!(
        !manifest.lines().any(|l| l.trim_start().starts_with("rig ")
            || l.trim_start().starts_with("rig=")),
        "feature `rig` declarada"
    );
}

#[test]
fn the_rig_interface_module_is_gone() {
    assert!(!Path::new(CORE).join("src/interface/rig").exists());
    let iface = std::fs::read_to_string(Path::new(CORE).join("src/interface/mod.rs")).unwrap();
    assert!(!iface.contains("pub mod rig"));
}
