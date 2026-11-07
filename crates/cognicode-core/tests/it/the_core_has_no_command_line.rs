//! The command line lives in `cognicode-cli`, not in the core.
//!
//! `cognicode-cli` depends on `cognicode-core`, so the core cannot name the CLI
//! at all: no `interface::cli` module, no `clap` in the manifest, and no
//! re-export of `Cli`/`CommandExecutor` at the crate root.

use std::path::Path;

fn core(path: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(path)
}

#[test]
fn the_core_has_no_interface_cli_module() {
    assert!(
        !core("src/interface/cli").exists(),
        "src/interface/cli sigue en el core"
    );
}

#[test]
fn the_core_manifest_does_not_declare_clap() {
    let manifest = std::fs::read_to_string(core("Cargo.toml")).expect("manifest");
    let declared = manifest
        .lines()
        .any(|l| l.trim_start().starts_with("clap") && l.contains('='));
    assert!(!declared, "el core declara clap");
}

#[test]
fn the_core_root_does_not_reexport_the_cli() {
    let lib = std::fs::read_to_string(core("src/lib.rs")).expect("lib.rs");
    assert!(!lib.contains("interface::cli"), "lib.rs reexporta el CLI");
}
