//! The sandbox failure taxonomy has a neutral core owner until sandbox artifacts move.

use std::fs;
use std::path::Path;

#[test]
fn failure_class_is_owned_by_the_neutral_application_module() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let core_src = manifest.join("src");
    let application = core_src.join("application");
    let sandbox_core = core_src.join("sandbox_core");
    let application_mod = fs::read_to_string(application.join("mod.rs"))
        .expect("read application module declarations");
    let sandbox_mod =
        fs::read_to_string(sandbox_core.join("mod.rs")).expect("read sandbox module declarations");

    assert!(
        application.join("sandbox_failure.rs").is_file(),
        "FailureClass neutral owner is missing"
    );
    assert!(
        application_mod.contains("pub mod sandbox_failure;"),
        "application does not expose the neutral failure taxonomy"
    );
    assert!(
        !sandbox_core.join("failure.rs").exists(),
        "failure.rs still lives under sandbox_core"
    );
    assert!(
        !sandbox_mod.contains("pub mod failure;"),
        "sandbox_core still exposes the old failure owner"
    );
}
