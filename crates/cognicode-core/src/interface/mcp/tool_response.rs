//! How a tool's answer reaches the wire.
//!
//! A tool call answers two readers, and they want two different things.
//!
//! - **A program** wants the object. A tool that publishes an output
//!   contract has told the client the shape of its answer, so the answer
//!   has to arrive where the contract says it does: in
//!   `structuredContent`. A client that had to dig the object out of a
//!   text block had to parse prose to get what it was promised.
//! - **An agent** wants one line. The same answer read as text is a
//!   sentence about the answer, not the answer itself to be parsed. That
//!   line is the *summary*.
//!
//! The summary is not a second copy of the payload. It is the payload's own
//! one-line field — `summary`, or `message` in the payloads that carried
//! their line before this rule existed: the line that the type which
//! produced the answer already wrote. The wire reads it; it never writes one
//! of its own, because a summary invented at the boundary is a summary
//! nobody can hold to.
//!
//! The text block that carries the JSON stays exactly where it was. It is
//! the compatibility clause of the specification — a tool that returns
//! structured content also returns the serialised JSON in a text block —
//! and it is the contract the existing consumers of this product read: the
//! first text block is what `scripts/mcp/mcp_smoke_all.py` returns, and
//! what the integration suites parse. Moving it would be a breaking change
//! with no deprecation behind it.
//!
//! So an answer with a contract is two blocks: the JSON first, where it has
//! always been, and the summary beside it.
//!
//! ## El sobre
//!
//! Every answer also carries an **envelope**
//! ([`crate::interface::hateoas`]): the result, what it rests on, what it
//! left out and what can be done next. The envelope is what travels in
//! `structuredContent`, and the lines that say what comes next travel as
//! text blocks beside the summary — an agent reading the answer as text
//! sees the next step without having to parse anything.
//!
//! The envelope does not replace the payload: the payload stays in the
//! first text block, exactly where the consumers of this product read it.

use rmcp::model::{CallToolResult, Content, ToolAnnotations};
use serde_json::{Map, Value};

use crate::interface::hateoas::{HandleContext, envelope_for, follow_up_lines};

/// The behaviour hints a tool publishes, derived from its declared authority.
///
/// The authority is the catalogue's own word about what a tool does, and
/// these hints are how that word reaches a client that never read the
/// catalogue. They are hints, not guarantees — the specification says so —
/// but a client that cannot tell a read-only tool from a mutating one has
/// to guess, and guessing is what the authority exists to prevent.
///
/// Two authorities are read-only in every sense: they change nothing, and
/// calling them twice changes nothing either. The rest change something, so
/// they are marked destructive, and they are not idempotent because this
/// product does not claim they are.
///
/// An authority nobody can read is treated as the most dangerous one
/// (`destructive`, and open to the world): the hint is the only thing a
/// client has to go on, and an unknown tool is exactly the case where
/// assuming the best is the mistake that costs.
pub fn annotations_for(authority: &str) -> ToolAnnotations {
    match authority {
        "read" => ToolAnnotations::from_raw(None, Some(true), Some(false), Some(true), Some(false)),
        "network" => {
            ToolAnnotations::from_raw(None, Some(false), Some(true), Some(false), Some(true))
        }
        "mutating" | "execute" => {
            ToolAnnotations::from_raw(None, Some(false), Some(true), Some(false), Some(false))
        }
        _ => ToolAnnotations::from_raw(None, Some(false), Some(true), Some(false), Some(true)),
    }
}

/// The answer to a tool call, shaped for the wire.
///
/// `body` is the JSON the handler produced. The answer carries the
/// **envelope** as `structuredContent` — the result, its basis, its budget
/// and its links — and the body's own `summary` line as a second text
/// block, with the "what comes next" lines beside it.
///
/// The first text block is still the body, byte for byte: that is the
/// compatibility clause the existing consumers read.
pub fn success_result(
    tool_name: &str,
    body: String,
    arguments: &Map<String, Value>,
    context: &HandleContext,
) -> CallToolResult {
    let payload: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
    let envelope = envelope_for(tool_name, arguments, &payload, context);

    let mut content = vec![Content::text(body)];
    if let Some(line) = summary_line(&payload) {
        content.push(Content::text(line));
    }
    for line in follow_up_lines(&envelope) {
        content.push(Content::text(line));
    }

    let mut result = CallToolResult::success(content);
    result.structured_content = serde_json::to_value(&envelope).ok();
    result
}

/// The payload's own one-line summary, when it carries one.
///
/// The field is named `summary`, and the payloads that carried their
/// one-liner before the rule existed call it `message`. Both names are read
/// here, in this order, and nowhere else: a payload that grew a second
/// prose field saying the same thing as the one it already had would be a
/// payload with two sources of truth.
fn summary_line(payload: &Value) -> Option<String> {
    let line = payload
        .get("summary")
        .or_else(|| payload.get("message"))?
        .as_str()?;
    if line.trim().is_empty() {
        return None;
    }
    Some(line.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// The text of a content block, when it is a text block.
    fn text_of(block: &Content) -> Option<&str> {
        block.raw.as_text().map(|t| t.text.as_str())
    }

    #[test]
    fn a_read_authority_publishes_read_only_hints() {
        let hints = annotations_for("read");
        assert_eq!(hints.read_only_hint, Some(true));
        assert_eq!(hints.destructive_hint, Some(false));
        assert_eq!(hints.idempotent_hint, Some(true));
        assert_eq!(hints.open_world_hint, Some(false));
    }

    #[test]
    fn a_mutating_authority_publishes_destructive_hints() {
        let hints = annotations_for("mutating");
        assert_eq!(hints.read_only_hint, Some(false));
        assert_eq!(hints.destructive_hint, Some(true));
        assert_eq!(hints.idempotent_hint, Some(false));
    }

    #[test]
    fn a_network_authority_is_open_to_the_world() {
        let hints = annotations_for("network");
        assert_eq!(hints.open_world_hint, Some(true));
        assert_eq!(hints.destructive_hint, Some(true));
    }

    #[test]
    fn an_authority_nobody_can_read_is_treated_as_the_worst() {
        let hints = annotations_for("who-knows");
        assert_eq!(hints.read_only_hint, Some(false));
        assert_eq!(hints.destructive_hint, Some(true));
        assert_eq!(hints.open_world_hint, Some(true));
    }

    /// El contexto al que se anclan los handles de la llamada.
    fn context() -> HandleContext {
        HandleContext {
            workspace: "/repo".to_string(),
            snapshot: "snap-1".to_string(),
        }
    }

    #[test]
    fn an_answer_carries_the_envelope_with_the_result_inside() {
        let body = json!({"summary": "42 symbols", "symbols": 42}).to_string();
        let result = success_result("build_graph", body.clone(), &Map::new(), &context());

        assert_eq!(result.content.len(), 2, "the JSON and the summary");
        assert_eq!(text_of(&result.content[0]), Some(body.as_str()));
        assert_eq!(text_of(&result.content[1]), Some("42 symbols"));

        let envelope = result.structured_content.expect("the envelope");
        assert_eq!(envelope["schema_version"], json!(1));
        assert_eq!(envelope["state"], json!("complete"));
        assert_eq!(
            envelope["result"],
            json!({"summary": "42 symbols", "symbols": 42}),
            "the result travels whole inside the envelope"
        );
        assert!(
            envelope["_links"].is_array(),
            "the envelope names the links"
        );
        assert_eq!(result.is_error, Some(false));
    }

    #[test]
    fn a_payload_that_calls_its_line_message_still_publishes_it() {
        let body = json!({"message": "Graph loaded from built: 42 symbols"}).to_string();
        let result = success_result("build_graph", body, &Map::new(), &context());

        assert_eq!(result.content.len(), 2);
        assert_eq!(
            text_of(&result.content[1]),
            Some("Graph loaded from built: 42 symbols")
        );
    }

    #[test]
    fn a_tool_without_a_contract_answers_with_the_envelope_too() {
        let body = json!({"summary": "not a contract"}).to_string();
        let result = success_result("write_file", body, &Map::new(), &context());

        let envelope = result.structured_content.expect("the envelope");
        assert_eq!(envelope["result"]["summary"], json!("not a contract"));
    }

    #[test]
    fn a_body_that_is_not_an_object_keeps_the_text_alone() {
        let result = success_result(
            "build_graph",
            "[1, 2, 3]".to_string(),
            &Map::new(),
            &context(),
        );

        assert_eq!(result.content.len(), 1);
        let envelope = result.structured_content.expect("the envelope");
        assert_eq!(
            envelope["result"],
            json!([1, 2, 3]),
            "the envelope carries the payload as it is"
        );
    }

    #[test]
    fn an_empty_summary_publishes_no_block() {
        let body = json!({"summary": "   "}).to_string();
        let result = success_result("build_graph", body, &Map::new(), &context());

        assert_eq!(result.content.len(), 1);
        assert!(result.structured_content.is_some());
    }
}
