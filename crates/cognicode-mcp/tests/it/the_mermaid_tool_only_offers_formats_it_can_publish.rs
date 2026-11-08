//! The mermaid tool must not offer a format the graph cannot publish.
//!
//! Measured: the vocabulary of the mermaid format lived in three layers that
//! did not agree with each other. The shape the tool published named the
//! formats in a sentence — «"code" (default, returns mermaid source) or
//! "svg" (renders to SVG)» — which a machine cannot read. The domain carried
//! a field documented as "code"|"svg" that nothing read. And the CLI passed
//! its own words ("txt", "png") into that field, so the domain received a
//! word its own contract said it could not receive.
//!
//! The consequence a consumer sees: a client that builds its call from the
//! published shape cannot tell which formats are real, and a format that
//! does not exist is accepted in silence — the tool answers with the source
//! and no error, as if the name had been understood.
//!
//! The law this file pins:
//!
//! - the published shape names the formats the server accepts, and
//! - a format the graph cannot publish is refused, with the ones it can.
//!
//! The refusal is the important half. A tool that answers an impossible
//! request with a plausible document teaches its client to believe answers
//! it cannot check; a tool that refuses leaves the client knowing that it
//! asked for something that does not exist.
//!
//! ## The vocabulary has one owner, and the shape names it
//!
//! The graph owns the vocabulary: `MermaidFormat` names the two forms the
//! renderer produces — the source, and the rendered diagram. The shape is
//! derived from that type, so the published list and the owner's list are
//! the same list, and a check ties them: when they separate, the check says
//! so instead of letting the published shape rot. The master says the same
//! list, because the master is the mirror of what the runtime publishes.
//!
//! The arguments in this file travel as JSON, the way they arrive: the
//! server deserializes them before the handler ever sees them, and that is
//! where a format that does not exist is refused.

use cognicode_core::domain::value_objects::MermaidFormat;
use cognicode_mcp::input_contracts::input_schema_for;
use cognicode_core::interface::mcp::schemas::ExportMermaidInput;
use serde_json::json;

/// The names an enumeration of a published shape names.
///
/// The absent value is not a name: a shape that lets the format be absent
/// publishes the names it accepts and nothing else.
fn names_of(enumeration: &serde_json::Value) -> Vec<String> {
    enumeration
        .as_array()
        .expect("the published shape names the formats it accepts")
        .iter()
        .filter_map(|name| name.as_str().map(|name| name.to_string()))
        .collect()
}

/// The formats the published shape names.
fn published_formats() -> Vec<String> {
    let published = input_schema_for("export_mermaid")
        .expect("the mermaid tool publishes the shape of its arguments");

    names_of(&published["properties"]["format"]["enum"])
}

/// The formats the owner names.
fn owner_formats() -> Vec<String> {
    MermaidFormat::ALL
        .iter()
        .map(|format| format.as_str().to_string())
        .collect()
}

/// The arguments the server reads, built the way the server reads them.
fn arguments(format: &str) -> Result<ExportMermaidInput, String> {
    serde_json::from_value(json!({ "format": format })).map_err(|error| error.to_string())
}

/// The master, read straight from disk.
fn master() -> serde_json::Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("product/tools.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    serde_json::from_str(&text).expect("the master is a JSON document")
}

/// The formats the master publishes for the mermaid tool.
fn master_formats() -> Vec<String> {
    let master = master();
    let tool = master["tools"]
        .as_array()
        .expect("the master publishes its tools")
        .iter()
        .find(|tool| tool["name"] == json!("export_mermaid"))
        .expect("the master publishes the mermaid tool");

    names_of(&tool["input_schema"]["properties"]["format"]["enum"])
}

/// The published shape names the formats, not a sentence about them.
///
/// A shape that names its values in prose is a shape a machine cannot read:
/// the values were in the sentence, and a sentence is not a contract.
#[test]
fn the_published_shape_names_the_formats_it_accepts() {
    let published = input_schema_for("export_mermaid")
        .expect("the mermaid tool publishes the shape of its arguments");

    let format = &published["properties"]["format"];
    assert!(
        format["enum"].is_array(),
        "the published shape names the formats it accepts: {format:?}"
    );
}

/// The published shape names what the owner names.
///
/// The shape is derived from the owner's type, so the two are the same
/// list. When they separate, the published shape has rotted: it says a
/// format the server refuses, or hides one it accepts.
#[test]
fn the_published_shape_names_what_the_owner_names() {
    let published = published_formats();
    let owner = owner_formats();

    assert_eq!(
        published, owner,
        "the published shape and the owner name different formats: the shape \
         says {published:?} and the owner says {owner:?}"
    );
}

/// A format the graph cannot publish is refused.
///
/// The server reads the arguments before the handler runs, so this is where
/// the refusal happens: the name never reaches the renderer.
#[test]
fn a_format_the_graph_cannot_publish_is_refused() {
    let answer = arguments("png");

    assert!(
        answer.is_err(),
        "the mermaid tool accepted a format the graph cannot publish: {answer:?}"
    );
}

/// The refusal names the formats the graph can publish.
///
/// A refusal that does not say what is possible leaves the client to guess.
#[test]
fn the_refusal_names_the_formats_the_graph_can_publish() {
    let refusal = arguments("png").expect_err("a format that cannot be published is refused");

    for name in ["code", "svg"] {
        assert!(
            refusal.contains(name),
            "the refusal does not name the formats the graph can publish: {refusal}"
        );
    }
}

/// The master says the same list the runtime publishes.
///
/// The master is the mirror of what the runtime advertises: when the two
/// separate, a client reads the master and calls something the server
/// refuses.
#[test]
fn the_master_says_the_same_list() {
    let master = master_formats();
    let published = published_formats();

    assert_eq!(
        master, published,
        "the master and the published shape name different formats: the master \
         says {master:?} and the shape says {published:?}"
    );
}
