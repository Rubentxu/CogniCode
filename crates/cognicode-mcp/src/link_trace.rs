//! A local, opt-in record of the links each answer offered.
//!
//! The envelope's `_links` travel in `structuredContent`, and a client
//! transcript keeps only the text blocks, where the link lines carry no
//! `rel`. A measurement of which offers an agent follows therefore cannot be
//! rebuilt from a transcript; the server is the only place that knows both
//! the offer and the call that follows it.
//!
//! The record is off unless `COGNICODE_LINK_TRACE` names a file. It is a
//! local write the operator asked for by name: nothing leaves the machine and
//! a failure to write never fails the tool call. One line per call, in the
//! shape `scripts/measure/link_follow.py` reads: `tool`, `arguments`,
//! `links`, `session`.

use std::io::Write;
use std::path::Path;

use rmcp::model::CallToolResult;
use serde_json::{Map, Value, json};

/// The variable that turns the record on, with the file it appends to.
pub const LINK_TRACE_VARIABLE: &str = "COGNICODE_LINK_TRACE";

/// The line for one answered call: what was asked and what was offered.
pub fn link_trace_line(
    tool_name: &str,
    arguments: &Map<String, Value>,
    result: &CallToolResult,
    session: &str,
) -> Value {
    let links = result
        .structured_content
        .as_ref()
        .and_then(|envelope| envelope.get("_links"))
        .cloned()
        .unwrap_or_else(|| json!([]));
    json!({
        "tool": tool_name,
        "arguments": arguments,
        "links": links,
        "session": session,
    })
}

/// Appends one line to the trace file. Returns the failure instead of
/// raising it: the caller decides that a lost line is only a warning.
pub fn append_line(path: &Path, line: &Value) -> std::io::Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(file, "{line}")
}

/// One name per server process: the calls of one agent session share it.
fn session_id() -> &'static str {
    static SESSION: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    SESSION.get_or_init(|| {
        let started = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_millis())
            .unwrap_or(0);
        format!("{}-{started}", std::process::id())
    })
}

/// Records the call when the operator asked for it; does nothing otherwise.
pub fn record(tool_name: &str, arguments: &Map<String, Value>, result: &CallToolResult) {
    let Some(path) = std::env::var_os(LINK_TRACE_VARIABLE).filter(|value| !value.is_empty()) else {
        return;
    };
    let line = link_trace_line(tool_name, arguments, result, session_id());
    if let Err(error) = append_line(Path::new(&path), &line) {
        tracing::warn!("link trace not written: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cognicode_core::interface::hateoas::HandleContext;
    use crate::tool_response::success_result;

    fn context() -> HandleContext {
        HandleContext {
            workspace: "/repo".to_string(),
            snapshot: "snap-1".to_string(),
        }
    }

    fn arguments() -> Map<String, Value> {
        let mut arguments = Map::new();
        arguments.insert("symbol_name".to_string(), json!("hub"));
        arguments
    }

    #[test]
    fn a_line_carries_what_link_follow_reads() {
        let body = json!({"summary": "ok"}).to_string();
        let result = success_result("analyze_impact", body, &arguments(), &context());
        let envelope_links = result.structured_content.as_ref().unwrap()["_links"].clone();

        let line = link_trace_line("analyze_impact", &arguments(), &result, "s1");

        assert_eq!(line["tool"], json!("analyze_impact"));
        assert_eq!(line["arguments"], json!({"symbol_name": "hub"}));
        assert_eq!(line["session"], json!("s1"));
        assert_eq!(
            line["links"], envelope_links,
            "the offers, whole, with their rel"
        );
    }

    #[test]
    fn an_answer_without_an_envelope_offers_nothing() {
        let result = CallToolResult::success(vec![]);
        let line = link_trace_line("status", &Map::new(), &result, "s1");
        assert_eq!(line["links"], json!([]));
    }

    #[test]
    fn lines_are_appended_one_per_row() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("trace.jsonl");
        append_line(&path, &json!({"tool": "a"})).unwrap();
        append_line(&path, &json!({"tool": "b"})).unwrap();

        let written = std::fs::read_to_string(&path).unwrap();
        let rows: Vec<Value> = written
            .lines()
            .map(|row| serde_json::from_str(row).unwrap())
            .collect();
        assert_eq!(rows, vec![json!({"tool": "a"}), json!({"tool": "b"})]);
    }

    #[test]
    fn an_unwritable_path_is_an_error_not_a_panic() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("no-such-dir").join("trace.jsonl");
        assert!(append_line(&missing, &json!({})).is_err());
    }
}
