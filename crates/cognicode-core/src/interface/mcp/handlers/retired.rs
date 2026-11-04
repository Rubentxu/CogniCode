//! Names that are no longer advertised, still served by the tool that computes.
//!
//! Two MCP tools were published in `product/tools.json` as `stable`
//! while the code behind them computed nothing:
//!
//! - `graph_analyze` answered every mode with the same canned
//!   `{"type": ..., "nodes": <symbol count>}`. Its payload even
//!   claimed that the SCC had been computed with
//!   `petgraph::algo::tarjan_scc`; no algorithm ran. The three
//!   algorithms it names are computed by `graph_condensed`
//!   (SCC condensation), `graph_reduced` (transitive reduction) and
//!   `graph_feedback_arcs` (feedback arc set).
//! - `composite_impact_query` promised an evidence bundle — `impact`,
//!   `architecture`, `findings` — that it could never deliver: the
//!   extractors it waits for were never connected, and the follow-ups
//!   it suggests were looked up by comparing a *symbol* name against a
//!   *tool* name, so the lookup could not match a symbol. The impact
//!   of a symbol is computed by `analyze_impact`.
//!
//! Both names leave `tools/list`: a tool that does not compute does
//! not belong in the catalogue of what the product offers. Both names
//! stay callable, because a published name that disappears without a
//! word breaks the callers that still use it. The call is served by
//! the tool that actually computes, and the answer says which one
//! served it and what replaced it.
//!
//! That is the whole deprecation: the name still works, the
//! advertisement is gone, and the payload is the real computation
//! instead of a promise.

use serde::Serialize;
use serde_json::Value;

use super::graph_handlers::{
    handle_graph_condensed, handle_graph_feedback_arcs, handle_graph_reduced,
};
use super::{HandlerContext, HandlerError, HandlerResult, handle_analyze_impact};
use crate::interface::mcp::schemas::{
    AnalyzeImpactInput, CompositeImpactQueryInput, GraphCondensedInput, GraphFeedbackArcsInput,
    GraphReducedInput,
};

/// The answer a retired name returns: the payload of the tool that
/// computed it, plus the provenance of that tool.
///
/// The payload is flattened next to the provenance, so the answer reads
/// like the tool that produced it: a caller that moves from the retired
/// name to the tool that replaced it finds the same fields, and the
/// three provenance fields say what happened.
#[derive(Debug, Serialize)]
pub struct RetiredAliasResult {
    /// The retired name the caller used.
    pub alias: &'static str,
    /// The tool that computed the answer.
    pub served_by: &'static str,
    /// What replaced the alias, and why the alias exists at all.
    pub deprecation: &'static str,
    /// The payload the computing tool produced.
    #[serde(flatten)]
    pub result: serde_json::Map<String, Value>,
}

/// The mode `graph_analyze` was called with.
///
/// The name is kept because the wire shape is published: a caller
/// that sends `{"mode": "scc"}` still gets an answer, and the answer
/// is now the condensation that `scc` names.
#[derive(Debug, serde::Deserialize)]
pub struct GraphAnalyzeInput {
    #[serde(default = "default_analyze_mode")]
    pub mode: String,
}

fn default_analyze_mode() -> String {
    "scc".into()
}

const GRAPH_ANALYZE_DEPRECATION: &str = "graph_analyze is no longer advertised: it never computed the algorithms it named. \
The same mode is served by the tool that does compute it.";

const COMPOSITE_IMPACT_DEPRECATION: &str = "composite_impact_query is no longer advertised: the evidence bundle it promised \
(impact, architecture, findings) was never wired, and the follow-ups it suggested were looked up by comparing a symbol \
name against a tool name. The impact of a symbol is computed by analyze_impact.";

/// Serve the retired `graph_analyze` name with the algorithm it named.
///
/// The mode picks the tool that computes it, and the payload is that
/// tool's own result — the condensation of the strongly connected
/// components, the transitive reduction, or the feedback arc set.
/// Nothing is faked: a mode that names no algorithm is an invalid
/// input, and a missing graph is an error, exactly as it is for the
/// tool that computes it.
pub async fn handle_graph_analyze(
    ctx: &HandlerContext,
    input: GraphAnalyzeInput,
) -> HandlerResult<RetiredAliasResult> {
    let mode = input.mode.as_str();
    // One arm per algorithm: the mode names the algorithm, the arm names
    // the tool that computes it, and the answer says which one served it.
    let (served_by, result) = match mode {
        "scc" => (
            "graph_condensed",
            handle_graph_condensed(ctx, GraphCondensedInput {}).await?,
        ),
        "reduced" => (
            "graph_reduced",
            handle_graph_reduced(ctx, GraphReducedInput {}).await?,
        ),
        "feedback_arcs" => (
            "graph_feedback_arcs",
            handle_graph_feedback_arcs(ctx, GraphFeedbackArcsInput {}).await?,
        ),
        other => {
            return Err(HandlerError::InvalidInput(format!(
                "Unknown mode '{other}'. Valid modes: scc, reduced, feedback_arcs",
            )));
        }
    };

    Ok(RetiredAliasResult {
        alias: "graph_analyze",
        served_by,
        deprecation: GRAPH_ANALYZE_DEPRECATION,
        result: into_payload(result)?,
    })
}

/// Serve the retired `composite_impact_query` name with the tool that
/// computes the impact of a symbol.
///
/// The composite was queried with a *qualified* name; the tool that
/// computes impact takes a *symbol* name, so the alias uses the last
/// segment of the qualified name and lets that tool resolve it with
/// its own rules.
pub async fn handle_composite_impact_query(
    ctx: &HandlerContext,
    input: CompositeImpactQueryInput,
) -> HandlerResult<RetiredAliasResult> {
    let symbol_name = last_segment(&input.qualified_name);
    let output = handle_analyze_impact(
        ctx,
        AnalyzeImpactInput {
            symbol_name,
            compressed: false,
        },
    )
    .await?;

    let result = serde_json::to_value(output)
        .map_err(|e| HandlerError::Internal(format!("impact result is not serialisable: {e}")))?;

    Ok(RetiredAliasResult {
        alias: "composite_impact_query",
        served_by: "analyze_impact",
        deprecation: COMPOSITE_IMPACT_DEPRECATION,
        result: into_payload(result)?,
    })
}

/// The payload of a computing tool, as the fields the answer carries.
///
/// A computing tool answers with an object; a payload that is not an
/// object has no fields to flatten, and flattening it would produce an
/// answer with no payload at all. That is a server fault, not a
/// caller's mistake.
fn into_payload(value: Value) -> HandlerResult<serde_json::Map<String, Value>> {
    match value {
        Value::Object(map) => Ok(map),
        other => Err(HandlerError::Internal(format!(
            "the tool that served this alias answered with a {other}, not an object"
        ))),
    }
}

/// The last segment of a qualified name.
///
/// `crate::module::function` and `module.function` both name
/// `function`. A name with no separator is already the symbol's name.
fn last_segment(qualified_name: &str) -> String {
    qualified_name
        .rsplit("::")
        .next()
        .unwrap_or(qualified_name)
        .rsplit('.')
        .next()
        .unwrap_or(qualified_name)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_last_segment_is_the_symbol_name() {
        assert_eq!(last_segment("crate::module::function"), "function");
        assert_eq!(last_segment("module.function"), "function");
        assert_eq!(last_segment("function"), "function");
        assert_eq!(last_segment(""), "");
    }
}
