//! Ask-family tool handler.
//!
//! Implements 1 MCP tool for natural-language query routing:
//! - `cognicode_ask` — natural-language front-end that classifies a question
//!   and dispatches to the appropriate primitive tool chain
// e30.1 clippy baseline reset: pre-existing lint debt (see fix/e30.1-clippy-baseline-reset)
#![allow(unused_imports)]

use std::sync::Arc;

use async_trait::async_trait;
use rmcp::model::{CallToolResult, Content};
use serde::Deserialize;
use serde_json::Value;

use crate::ask::AskRouter;
use crate::mcp::context::McpContext;
use crate::mcp::envelope::{
    err_envelope, err_envelope_with_provenance, ok_envelope_with_provenance,
};
use crate::mcp::handler::ToolHandler;
use crate::mcp::{McpResultEnvelope, ProvenanceMetadata, TOOL_ASK};

// ============================================================================
// Arg struct
// ============================================================================

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct AskArgs {
    question: Option<String>,
    /// Reserved for future use (routing hints, conversation state).
    context: Option<serde_json::Value>,
}

// ============================================================================
// ToolHandler implementation
// ============================================================================

/// Handler for `cognicode_ask` — natural-language front-end that classifies
/// a question against 8 priority-ordered patterns and dispatches to the
/// right primitive chain.
struct AskHandler;

#[async_trait]
impl ToolHandler for AskHandler {
    fn name(&self) -> &'static str {
        TOOL_ASK
    }

    fn arg_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "question": {
                    "type": "string",
                    "description": "Natural-language question to classify and answer (required)."
                },
                "context": {
                    "type": "object",
                    "description": "Reserved for future use (routing hints, conversation state)."
                }
            },
            "required": ["question"]
        })
    }

    async fn handle(&self, ctx: &McpContext, params: Value) -> CallToolResult {
        let args: AskArgs = match serde_json::from_value(params) {
            Ok(a) => a,
            Err(e) => {
                return err_envelope(
                    TOOL_ASK,
                    "invalid_args",
                    &format!("{TOOL_ASK}: invalid args: {e}"),
                );
            }
        };

        let question = match args.question {
            Some(q) if !q.is_empty() => q,
            _ => {
                return err_envelope(
                    TOOL_ASK,
                    "missing_required_arg",
                    "cognicode_ask: missing required arg `question`",
                );
            }
        };

        // `context` is reserved for future use; the current router is a
        // pure function over `(question, _)`.
        let _ = args.context;

        // Classify the question and dispatch via the ask router.
        let classified = AskRouter::classify(&question);

        // Get the required services from context (PR 1 migration — fallbacks removed).
        let search = match ctx.search.as_ref() {
            Some(s) => s.as_ref(),
            None => {
                return err_envelope(TOOL_ASK, "facade_unavailable", "search service not wired");
            }
        };
        let workspace = match ctx.workspace.as_ref() {
            Some(w) => w.as_ref(),
            None => {
                return err_envelope(
                    TOOL_ASK,
                    "facade_unavailable",
                    "workspace service not wired",
                );
            }
        };
        let view = match ctx.view.as_ref() {
            Some(v) => v.as_ref(),
            None => {
                return err_envelope(TOOL_ASK, "facade_unavailable", "view service not wired");
            }
        };

        let outcome = crate::ask::dispatch::dispatch_ask(
            classified, search, workspace, view, &ctx.graph, None,
        )
        .await;

        // Serialize the envelope as the payload — the outer envelope from
        // dispatch_ask already has the right structure, but we need to
        // wrap it in our own envelope so the tool_name, version, timestamp
        // match this tool's identity.
        let provenance =
            ProvenanceMetadata::new(0.0, Some("ask-router".into())).unwrap_or_default();
        let payload = serde_json::to_value(outcome.envelope()).unwrap_or(serde_json::Value::Null);
        // A question that could not be answered travels as a failure. The
        // payload says what happened and what else could be asked; the flag
        // says the tool did not answer, and a client that reads only the
        // flag would otherwise read a failure as an answer.
        if outcome.is_failure() {
            return err_envelope_with_provenance(TOOL_ASK, &payload, provenance);
        }
        ok_envelope_with_provenance(TOOL_ASK, &payload, provenance)
    }
}

// ============================================================================
// Registry builder
// ============================================================================

/// Register the ask-family handler into the registry.
pub fn register_ask_handlers(registry: &mut crate::mcp::handler::ToolHandlerRegistry) {
    registry.register(AskHandler);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::facades::mocks::build_mocks;
    use serde_json::json;

    /// A question that could not be answered travels as a failure.
    ///
    /// The tool answers with the envelope of the router and with a flag, and
    /// the flag is what a client reads to know the tool did not answer. A
    /// failure that says it succeeded teaches the client to read a failure as
    /// an answer, and a client that learned that reads every later failure
    /// as an answer too.
    #[tokio::test]
    async fn a_question_that_cannot_be_answered_travels_as_a_failure() {
        let (search, view, workspace) = build_mocks();
        let context = McpContext::builder()
            .with_search(Arc::new(search))
            .with_view(Arc::new(view))
            .with_workspace(Arc::new(workspace))
            .build();

        // A question about the call graph, asked of a context that has no
        // call graph: the router cannot answer it.
        let result = AskHandler
            .handle(&context, json!({ "question": "path between `a` and `b`" }))
            .await;

        assert_eq!(
            result.is_error,
            Some(true),
            "a question that could not be answered travels as a failure: {result:?}"
        );
    }
}
