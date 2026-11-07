//! The core manifest must not declare an exporter stack its code never names.
//!
//! `opentelemetry` (the API) is used by `infrastructure::telemetry`; the SDK and
//! the OTLP exporter are wired by the `cognicode-mcp` binary, which declares them
//! itself. A declared-but-unused dependency still compiles and still ties the
//! core to a transport it does not use.

use std::path::Path;

fn core_manifest() -> toml::Table {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    std::fs::read_to_string(&path)
        .expect("the core manifest is readable")
        .parse()
        .expect("the core manifest is valid TOML")
}

fn declared(manifest: &toml::Table, name: &str) -> bool {
    ["dependencies", "dev-dependencies", "build-dependencies"]
        .iter()
        .filter_map(|section| manifest.get(*section).and_then(|v| v.as_table()))
        .any(|table| table.contains_key(name))
}

#[test]
fn the_probe_sees_a_declared_dependency() {
    let manifest = core_manifest();
    assert!(
        declared(&manifest, "opentelemetry"),
        "the API is the one the core uses"
    );
}

#[test]
fn the_core_does_not_declare_the_otlp_exporter_or_the_sdk() {
    let manifest = core_manifest();
    for name in ["opentelemetry-otlp", "opentelemetry_sdk"] {
        assert!(
            !declared(&manifest, name),
            "{name} is declared but the core never names it"
        );
    }
}
