//! The sandbox resource DTO has a neutral core owner.

use std::fs;
use std::path::Path;

#[test]
fn resource_usage_is_owned_by_the_neutral_application_module() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let core_src = manifest.join("src");
    let application = core_src.join("application");
    let artifacts = core_src.join("sandbox_core/artifacts.rs");

    let application_mod = fs::read_to_string(application.join("mod.rs"))
        .expect("read application module declarations");
    let artifacts_source = fs::read_to_string(&artifacts).expect("read artifacts source");

    assert!(
        application.join("sandbox_resource_usage.rs").is_file(),
        "ResourceUsage neutral owner is missing"
    );
    assert!(
        application_mod.contains("pub mod sandbox_resource_usage;"),
        "application does not expose the neutral resource DTO"
    );
    assert!(
        !artifacts_source.contains("pub struct ResourceUsage"),
        "artifacts.rs still owns ResourceUsage"
    );
    assert!(
        artifacts_source.contains("application::sandbox_resource_usage::ResourceUsage"),
        "artifacts.rs does not use the neutral ResourceUsage owner"
    );
}
