//! `sandbox_core` is owned by `cognicode-sandbox`, not by the analysis core.
//!
//! The scenario manifests, the artifact models, the scoring and the history of
//! the sandbox orchestrator are consumed by the orchestrator and by nothing
//! else, so they are compiled with it. When they lived in `cognicode-core`
//! every edit to the scorer recompiled the crate that the whole product
//! depends on. This structural contract keeps a second owner from growing back.

use std::fs;
use std::path::{Path, PathBuf};

fn crates_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

#[test]
fn the_analysis_core_does_not_compile_sandbox_core() {
    let core_src = crates_dir().join("cognicode-core/src");
    let core_lib = fs::read_to_string(core_src.join("lib.rs")).expect("read the core lib.rs");

    assert!(
        !core_lib
            .lines()
            .any(|line| line.trim().ends_with("mod sandbox_core;")),
        "cognicode-core still declares `mod sandbox_core;`"
    );
    assert!(
        !core_src.join("sandbox_core").exists(),
        "cognicode-core still carries the sandbox_core sources"
    );
}

#[test]
fn the_sandbox_crate_owns_every_sandbox_core_module() {
    let sandbox_core = crates_dir().join("cognicode-sandbox/src/sandbox_core");
    let declarations =
        fs::read_to_string(sandbox_core.join("mod.rs")).expect("read the sandbox_core mod.rs");

    for module in [
        "artifacts",
        "ground_truth",
        "history",
        "manifest",
        "read_source_reconstructor",
        "scoring",
    ] {
        assert!(
            sandbox_core.join(format!("{module}.rs")).is_file(),
            "{module}.rs must live in cognicode-sandbox"
        );
        assert!(
            declarations.contains(&format!("pub mod {module};")),
            "sandbox_core does not declare `{module}`"
        );
    }
}
