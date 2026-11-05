//! The published arguments are the declared ones.
//!
//! The catalogue publishes an `input_schema` for every tool, and the
//! arguments the server actually reads live in a Rust struct per tool.
//! Until now those were two descriptions of the same thing: the catalogue
//! carried a hand-written copy of the shape, and the copy was the one that
//! rotted.
//!
//! The copy rotted in a way a name check cannot see. Four tools accepted an
//! argument the published schema never mentioned — `include_external`,
//! `module_filter`, `source_file` and `edit_ranges` — so a client that
//! builds its call from the schema could not pass them, and a client that
//! read the code could. The catalogue was not wrong about the tools it
//! listed; it was silent about the arguments of four of them.
//!
//! This suite pins four contracts:
//!
//! 1. **The published shape is the derived one.** The schema the catalogue
//!    publishes for a tool is the schema derived from the struct that reads
//!    its arguments. A hand-written copy is a second description of a type
//!    that already exists, and it can drift without anyone noticing.
//!
//! 2. **Every tool that publishes arguments publishes them.** A tool that
//!    reads its arguments one key at a time has no derived contract, and
//!    saying so is the honest answer; but the set of those tools is small,
//!    explicit, and every one of them is named here. A new tool without a
//!    contract is a hole, not a detail.
//!
//! 3. **The runtime advertises the derived shape.** The schema a client sees
//!    in `tools/list` is the one the server honours, so the two cannot
//!    disagree about the same tool.
//!
//! 4. **The value the published shape excludes is the value the server
//!    refuses.** A shape that enumerates the values an argument accepts is
//!    a promise, and the server has to keep it: a value outside the
//!    enumeration is a rejection, not a silent fallback.
//!
//! The catalogue carries a copy of the derived schema, and the copy is
//! regenerated from the derivation. That regeneration is a merge, not a
//! replacement: the fields the runtime does not publish stay curated in the
//! catalogue.

use cognicode_core::interface::mcp::error::InterfaceError;
use cognicode_core::interface::mcp::file_ops_handlers::handle_read_file;
use cognicode_core::interface::mcp::handlers::HandlerContext;
use cognicode_core::interface::mcp::input_contracts::{
    input_schema_for, published_input_schemas, tools_with_input_schema,
};
use cognicode_core::interface::mcp::schemas::ReadFileInput;
use serde_json::Value;
use std::collections::BTreeMap;

/// The tools that publish no derived contract, and why.
///
/// Each of them reads its arguments from the raw JSON object instead of a
/// struct, so there is no type to derive a schema from. The list is
/// explicit so that a tool added without a contract fails the suite
/// instead of joining these three silently.
const WITHOUT_A_DERIVED_CONTRACT: &[(&str, &str)] = &[
    (
        "list_evidence",
        "reads `workspace` and `kind` from the raw arguments object",
    ),
    (
        "search_evidence",
        "reads `workspace`, `query` and `limit` from the raw arguments object",
    ),
    ("solid_audit", "takes no arguments at all"),
];

fn catalog_path() -> std::path::PathBuf {
    let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for ancestor in manifest.ancestors() {
        let candidate = ancestor.join("product").join("tools.json");
        if candidate.is_file() {
            return candidate;
        }
    }
    panic!("could not locate product/tools.json above {manifest:?}");
}

fn catalog() -> BTreeMap<String, Value> {
    let path = catalog_path();
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read catalogue file {path:?}: {e}"));
    let value: Value = serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("catalogue file {path:?} is not valid JSON: {e}"));
    value["tools"]
        .as_array()
        .unwrap_or_else(|| panic!("catalogue file {path:?} has no `tools` array"))
        .iter()
        .map(|tool| {
            let name = tool["name"]
                .as_str()
                .expect("every catalogue entry is named")
                .to_string();
            (name, tool.clone())
        })
        .collect()
}

#[test]
fn the_published_shape_is_the_derived_one() {
    let catalogue = catalog();
    let derived = published_input_schemas();
    assert!(
        !derived.is_empty(),
        "no input contract was derived; the check below would pass without \
         checking anything"
    );
    for (tool, schema) in derived {
        let published = catalogue
            .get(*tool)
            .unwrap_or_else(|| panic!("{tool} publishes a contract but is not in the catalogue"));
        assert_eq!(
            published["input_schema"],
            Value::Object(schema.clone()),
            "{tool}: the catalogue publishes a shape the server does not read. \
             The published shape is the derived one; regenerate the catalogue \
             from the derivation"
        );
    }
}

#[test]
fn every_tool_that_publishes_arguments_publishes_them() {
    let catalogue = catalog();
    let derived: std::collections::BTreeSet<&str> = tools_with_input_schema().into_iter().collect();
    let known: std::collections::BTreeSet<&str> = WITHOUT_A_DERIVED_CONTRACT
        .iter()
        .map(|(tool, _)| *tool)
        .collect();
    for (name, tool) in &catalogue {
        // A tool this build does not advertise has no contract to derive:
        // the contract of a tool nobody can call is a promise to nobody.
        let feature = tool
            .get("feature")
            .and_then(|f| f.as_str())
            .map(str::to_owned);
        if !cognicode_core::interface::catalog::tool_feature_enabled(&feature) {
            continue;
        }
        if derived.contains(name.as_str()) || known.contains(name.as_str()) {
            continue;
        }
        panic!(
            "{name} publishes a schema but no contract is derived from it, and \
             it is not one of the tools that read their arguments from the raw \
             object: {known:?}. Either declare the arguments in a struct or \
             name the tool here, with the reason"
        );
    }
    // The other direction: a contract for a tool the catalogue does not
    // publish is a contract nobody can read.
    for tool in derived {
        assert!(
            catalogue.contains_key(tool),
            "{tool} publishes a derived contract but is not in the catalogue"
        );
    }
}

#[test]
fn the_tools_without_a_contract_are_the_ones_named_here() {
    let catalogue = catalog();
    for (tool, reason) in WITHOUT_A_DERIVED_CONTRACT {
        assert!(
            catalogue.contains_key(*tool),
            "{tool} is named as a tool without a derived contract but is not \
             in the catalogue: the reason recorded for it ({reason}) no longer \
             describes anything"
        );
        assert!(
            input_schema_for(tool).is_none(),
            "{tool} is named as a tool without a derived contract but one is \
             derived for it: the note is stale"
        );
    }
}

#[test]
fn the_runtime_advertises_the_derived_shape() {
    // The schema a client reads in `tools/list` is the one the server
    // honours. A client that builds its call from what the server
    // advertised must be able to pass every argument the server reads.
    let advertised = cognicode_core::interface::mcp::rmcp_adapter::build_all_tools();
    let derived = published_input_schemas();
    for tool in advertised {
        let name = tool.name.as_ref();
        let Some(schema) = derived.get(name) else {
            continue;
        };
        let advertised_schema = Value::Object((*tool.input_schema).clone());
        assert_eq!(
            advertised_schema,
            Value::Object(schema.clone()),
            "{name}: the runtime advertises a shape that is not the derived one"
        );
    }
}

/// The value the published shape excludes is the value the server refuses.
///
/// The published shape names the modes `read_file` accepts. A mode that is
/// not among them is a rejection: the server used to read it as `raw` in
/// silence, so a client that asked for a mode the server does not have
/// received a different read, and nobody told it.
#[tokio::test]
async fn a_read_mode_the_published_shape_excludes_is_refused() {
    let published = input_schema_for("read_file").expect("read_file publishes a contract");
    let modes = published["properties"]["mode"]["enum"]
        .as_array()
        .expect("the published shape names the modes it accepts");
    assert!(
        modes.iter().any(|mode| mode == "raw"),
        "the published shape names the modes it accepts: {modes:?}"
    );
    assert!(
        !modes.iter().any(|mode| mode == "contorno"),
        "a mode that does not exist cannot be in the published shape"
    );

    let workspace = tempfile::tempdir().expect("workspace");
    let file = workspace.path().join("notas.txt");
    std::fs::write(&file, "contenido\n").expect("the file exists");
    let ctx = HandlerContext::builder()
        .with_working_dir(workspace.path().to_path_buf())
        .build();
    let input = ReadFileInput {
        path: file.to_str().expect("the path is text").to_string(),
        start_line: None,
        end_line: None,
        mode: Some("contorno".to_string()),
        chunk_size: None,
        continuation_token: None,
    };
    let error = handle_read_file(&ctx, input)
        .await
        .expect_err("a mode that does not exist is a rejection");
    let text = InterfaceError::from(error).to_string();
    assert!(
        text.contains("Unknown read mode"),
        "the rejection says which mode it refused: {text}"
    );
    assert!(
        !text.contains("Internal error"),
        "a rejection is a decision of the product, not a crash: {text}"
    );
}
