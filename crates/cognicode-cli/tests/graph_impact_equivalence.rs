//! `graph impact` and the MCP `analyze_impact` are adapters over one
//! question, so for the same project and the same name they have to give the
//! same answer.
//!
//! The project holds two symbols whose names share a prefix (`foo` and
//! `foo_bar`), each with its own caller. A name means the symbol that carries
//! it, not every symbol that contains it as a substring: the dependents of
//! `foo_bar` are not dependents of `foo`.

mod common;

use cognicode_core::interface::mcp::handlers::{HandlerContext, handle_analyze_impact};
use cognicode_core::interface::mcp::schemas::AnalyzeImpactInput;
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

fn project() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "cognicode-cli-impact-parity-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default()
    ));
    std::fs::create_dir_all(dir.join("src")).expect("create the project");
    std::fs::write(
        dir.join("src/lib.rs"),
        "pub fn foo() -> u32 { 1 }\n\
         pub fn foo_bar() -> u32 { 2 }\n\
         pub fn uses_foo() -> u32 { foo() }\n\
         pub fn uses_bar() -> u32 { foo_bar() }\n",
    )
    .expect("write the source");
    dir
}

/// The real binary, as a sub-process. The command prints a progress line
/// before the JSON document, so the document is read from its first brace.
fn cli_impact(dir: &Path, symbol: &str) -> Value {
    let output = Command::new(common::binary_path("cognicode"))
        .args(["--format", "json", "graph", "impact", symbol])
        .arg(dir)
        .output()
        .expect("run cognicode");
    assert!(
        output.status.success(),
        "the CLI failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let start = stdout
        .find('{')
        .expect("the CLI answers with a JSON object");
    serde_json::from_str(&stdout[start..]).expect("the CLI answer parses")
}

fn names(answer: &Value, key: &str) -> BTreeSet<String> {
    answer[key]
        .as_array()
        .unwrap_or_else(|| panic!("`{key}` is a list: {answer}"))
        .iter()
        .map(|v| v.as_str().expect("a string").to_string())
        .collect()
}

async fn mcp_impact(dir: &Path, symbol: &str) -> Value {
    let ctx = HandlerContext::builder().with_working_dir(dir).build();
    let out = handle_analyze_impact(
        &ctx,
        AnalyzeImpactInput {
            symbol_name: symbol.to_string(),
            compressed: false,
        },
    )
    .await
    .expect("the MCP answers");
    serde_json::to_value(&out).expect("the MCP output serializes")
}

#[tokio::test]
async fn a_name_does_not_match_the_symbols_that_merely_contain_it() {
    let dir = project();
    let cli = cli_impact(&dir, "foo");

    assert_eq!(
        names(&cli, "impacted_symbols"),
        BTreeSet::from(["uses_foo".to_string()]),
        "the dependents of `foo_bar` are not dependents of `foo`: {cli}"
    );
    assert_eq!(
        cli["direct_dependents"], 1,
        "only `foo` itself carries the name: {cli}"
    );
}

#[tokio::test]
async fn the_cli_and_the_mcp_agree_on_the_same_name() {
    let dir = project();
    for symbol in ["foo", "foo_bar"] {
        let cli = cli_impact(&dir, symbol);
        let mcp = mcp_impact(&dir, symbol).await;

        assert_eq!(
            names(&cli, "impacted_symbols"),
            names(&mcp, "impacted_symbols"),
            "impacted symbols of `{symbol}` differ.\ncli: {cli}\nmcp: {mcp}"
        );
        assert_eq!(
            names(&cli, "impacted_files"),
            names(&mcp, "impacted_files"),
            "impacted files of `{symbol}` differ.\ncli: {cli}\nmcp: {mcp}"
        );
        assert_eq!(
            cli["risk_level"].as_str().map(str::to_lowercase),
            mcp["risk_level"].as_str().map(str::to_string),
            "risk of `{symbol}` differs.\ncli: {cli}\nmcp: {mcp}"
        );
    }
}
