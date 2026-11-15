//! The dispatch boundary: where a published tool becomes the answer a
//! client reads.
//!
//! The transport wires the calls; this module decides what happens to
//! them: whether the server may answer them (read-only), what to do with
//! the arguments (the BC layer of renamed parameters), how they are
//! counted (the instrumentation), whether the rate limiter lets them
//! through, how long they may take (the window of their category), and
//! which tool answers them.
//!
//! It lives apart from the transport wiring because it is a different
//! concern and because it is the part that grows: the routing table
//! names every published tool, and the transport does not.

use cognicode_core::infrastructure::telemetry::get_global_metrics;
use cognicode_core::interface::mcp::error::{InterfaceError, InterfaceResult};
use cognicode_core::interface::mcp::handlers::HandlerContext;
use opentelemetry::KeyValue;
use rmcp::model::CallToolRequestParams;
use std::time::Instant;

use crate::rmcp_adapter::{is_strict_rate_limit_category, lookup_category, tool_is_mutating};

/// Handles the tool call by dispatching to the appropriate handler
///
/// M1.1: All tool calls flow through this single instrumentation boundary.
/// Timing, call count, and error classification are recorded here — no
/// per-handler timing or record_tool_usage calls needed.
///
/// The boundary is reachable from outside the crate on purpose: it is where a
/// published tool becomes the answer a client reads, and a caller that wants
/// that answer without a transport — an in-process client, a test that has to
/// see what the client sees — has to be able to come through here. Reaching
/// the handler directly instead would leave the caller reading a copy of the
/// dispatch rather than the dispatch itself.
pub async fn call_tool_handler(
    ctx: &HandlerContext,
    request: CallToolRequestParams,
) -> InterfaceResult<String> {
    call_tool_handler_with_window(ctx, request, None).await
}

/// The same dispatch, with the window the caller names.
///
/// `None` —what [`call_tool_handler`] passes— means the window of the
/// tool's category: the window the product publishes, derived from the
/// work's own budget and from what the catalogue declares.
///
/// A window named here replaces it, and that is how the case in which the
/// deadline cuts a call is reachable: no published tool has a work longer
/// than its window (there are contracts that watch it), so the case cannot
/// be reached with the production windows.
pub async fn call_tool_handler_with_window(
    ctx: &HandlerContext,
    request: CallToolRequestParams,
    window: Option<std::time::Duration>,
) -> InterfaceResult<String> {
    let tool_name = request.name.as_ref();
    let mut arguments = request.arguments.unwrap_or_default();

    // PRF-SEC-02 + PRF-MCP-05: read-only mode rejects mutating tools before any handler
    // runs. Error is typed/honest (isError text), not a silent success.
    // Authority comes from the declared `cognicode.authority` field (primary oracle),
    // with no second list to fall back to.
    if tool_is_mutating(tool_name) && ctx.is_read_only() {
        return Err(InterfaceError::ReadOnly {
            tool: tool_name.to_string(),
        });
    }

    // UAT 2026-08-10 DEFECT-1: BC layer for parameter naming.
    // Renames legacy parameter names to their canonical equivalents so
    // callers that pre-date the v1.0.0 convention continue to work.
    // Each substitution emits a structured `warn` so dashboards can
    // drive migration without this layer silently propping them up.
    let substitutions = crate::parameter_aliases::apply_aliases(tool_name, &mut arguments);
    if !substitutions.is_empty() {
        let renames = substitutions
            .iter()
            .map(|(old, new)| format!("{old}\u{2192}{new}"))
            .collect::<Vec<_>>()
            .join(", ");
        tracing::warn!(
            tool = %tool_name,
            renames = %renames,
            "deprecated MCP parameter name(s) used; this BC shim ships with v1.0.0 and is scheduled for removal in v1.1"
        );
    }

    // M1.1: Centralized instrumentation boundary
    let start = Instant::now();
    let metrics = get_global_metrics();

    // M3.2 / M3.3: Resolve the tool's category once. Used both for
    // timeout selection (M3.2) and rate-limit key prefixing (M3.3).
    let category = lookup_category(tool_name);

    // M3.3: Pre-match rate-limit check. The key is per-tool so each
    // tool has its own 100/60s budget. Expensive categories (graph,
    // navigation, aix) use a stricter key prefix to keep their
    // budgets separate from the rest of the dispatcher.
    let rate_key = if is_strict_rate_limit_category(&category) {
        format!("strict:{}", tool_name)
    } else {
        format!("tool:{}", tool_name)
    };
    if !ctx.rate_limiter().check_with_key(&rate_key) {
        let err: InterfaceResult<String> = Err(InterfaceError::RateLimited);
        let status = crate::status::classify_status(tool_name, &err);
        let duration_ms = start.elapsed().as_millis() as f64;
        // M3.4: Emit the structured per-call log line for parity
        // with the success path so dashboards see the rate-limit hit.
        tracing::info!(
            tool = %tool_name,
            duration_ms = %duration_ms as u64,
            status = %status,
            "tool_call"
        );
        if let Some(m) = &metrics {
            m.calls.add(
                1,
                &[
                    KeyValue::new("tool", tool_name.to_string()),
                    KeyValue::new("status", status),
                ],
            );
            m.duration.record(
                duration_ms,
                &[
                    KeyValue::new("tool", tool_name.to_string()),
                    KeyValue::new("status", status),
                ],
            );
            // Record the error with error_type="rate_limit_exceeded"
            // so it shows up in the M1.6 error metric.
            m.errors.add(
                1,
                &[
                    KeyValue::new("tool", tool_name.to_string()),
                    KeyValue::new("error_type", "rate_limit_exceeded"),
                ],
            );
        }
        return err;
    }

    // M3.2: Wrap the per-tool match dispatch with a category-based
    // timeout. The match block itself is unchanged; only the wrapper
    // differs. The window is the one the tool's category declares,
    // raised to the window of the graph when the catalogue says the
    // tool needs the graph. On timeout, the dispatch future is dropped
    // and the call is answered with the timeout refusal.
    //
    // A window named by the caller replaces that one: the window of a
    // tool is the one its work's budget allows, so no published tool
    // outlives it —and the case in which the deadline cuts a call is
    // then only reachable with a window the caller names.
    let timeout =
        window.unwrap_or_else(|| crate::robustness::timeout_for_tool(tool_name, &category));
    let dispatch = async {
        // TASK-82 — los goals. Un goal no es un tool con su brazo: es una
        // declaración del registro que dice cómo se implementa, y la
        // implementación es la misma que llama el CLI. El goal se resuelve
        // antes del `match` porque su nombre no es un caso del `match`: lo
        // construye el registro.
        if let Some(goal) = cognicode_core::interface::goals::find(tool_name) {
            let output = cognicode_core::interface::mcp::handlers::goal::handle_goal(
                ctx,
                goal,
                arguments.into(),
            )
            .await?;
            return Ok(serde_json::to_string(&output)?);
        }
        match tool_name {
            "get_file_symbols" => {
                let input: cognicode_core::interface::mcp::schemas::GetFileSymbolsInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    cognicode_core::interface::mcp::handlers::handle_get_file_symbols(ctx, input)
                        .await?;
                Ok(serde_json::to_string(&output)?)
            }
            "build_graph" => {
                let input: cognicode_core::interface::mcp::handlers::BuildGraphInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    cognicode_core::interface::mcp::handlers::handle_build_graph(ctx, input)
                        .await?;

                // M2.1: Record graph statistics after successful build
                if output.success {
                    let graph = ctx.analysis_service().get_project_graph();
                    let symbols = graph.symbol_count() as u64;
                    let edges = graph.edge_count() as u64;
                    let projection =
                        cognicode_core::infrastructure::graph::project_call_graph(&graph);
                    let health_score =
                    cognicode_core::application::services::graph_insights::GraphInsightsService::analyze(
                        &graph,
                        projection.as_ref(),
                    )
                        .health_score;
                    if let Some(m) = &metrics {
                        m.record_graph_stats(symbols, edges, health_score);
                    }
                    // M3.1: Flip the readiness flag so /ready returns 200
                    ctx.mark_graph_loaded();
                }

                Ok(serde_json::to_string(&output)?)
            }
            "get_call_hierarchy" => {
                let input: cognicode_core::interface::mcp::schemas::GetCallHierarchyInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    cognicode_core::interface::mcp::handlers::handle_get_call_hierarchy(ctx, input)
                        .await?;
                Ok(serde_json::to_string(&output)?)
            }
            "analyze_impact" => {
                let input: cognicode_core::interface::mcp::schemas::AnalyzeImpactInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    cognicode_core::interface::mcp::handlers::handle_analyze_impact(ctx, input)
                        .await?;
                Ok(serde_json::to_string(&output)?)
            }

            "composite_impact_query" => {
                let input: cognicode_core::interface::mcp::schemas::CompositeImpactQueryInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    cognicode_core::interface::mcp::handlers::retired::handle_composite_impact_query(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string(&output)?)
            }

            "safe_refactor" => {
                let input: cognicode_core::interface::mcp::schemas::SafeRefactorInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    cognicode_core::interface::mcp::handlers::refactor_handlers::handle_safe_refactor(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string(&output)?)
            }
            "safe_refactor_preview" => {
                let input: cognicode_core::interface::mcp::schemas::SafeRefactorPreviewInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    cognicode_core::interface::mcp::handlers::refactor_handlers::handle_safe_refactor_preview(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string(&output)?)
            }
            "find_usages" => {
                let input: cognicode_core::interface::mcp::schemas::FindUsagesInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    cognicode_core::interface::mcp::handlers::handle_find_usages(ctx, input)
                        .await?;
                Ok(serde_json::to_string(&output)?)
            }
            // E1.W3 — EvidenceStore MCP tools. Mirror `cognicode evidence
            // list|search` so the equivalence test pins the same JSON
            // schema on both surfaces. See `cognicode-cli/tests/
            // evidence_cli_mcp_equivalence.rs`.
            #[cfg(feature = "evidence-cli-ladybug")]
            "list_evidence" => {
                use cognicode_core::application::evidence_backend;
                use cognicode_core::domain::ports::evidence_store::EvidenceKind;

                let workspace = arguments
                    .get("workspace")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let kind = match arguments.get("kind").and_then(|v| v.as_str()) {
                    None | Some("") => None,
                    Some("log") => Some(EvidenceKind::Log),
                    Some("trace") => Some(EvidenceKind::Trace),
                    Some("measurement") => Some(EvidenceKind::Measurement),
                    Some("external") => Some(EvidenceKind::External),
                    Some(other) => {
                        return Err(InterfaceError::InvalidInput(format!(
                            "unknown evidence kind: {other} (expected one of \
                             log | trace | measurement | external)"
                        )));
                    }
                };

                let factory = evidence_backend::evidence_backend_factory().ok_or_else(|| {
                    InterfaceError::Internal(
                        "evidence backend not registered; the MCP server build does not \
                         have the `evidence-cli-ladybug` feature enabled, or the runtime \
                         adapter did not register a factory at startup."
                            .to_string(),
                    )
                })?;
                let backend = factory(None).map_err(InterfaceError::Internal)?;
                let rows = backend
                    .list(workspace, kind)
                    .map_err(InterfaceError::Internal)?;
                Ok(cognicode_core::application::evidence_rows::render_evidence_rows_json(rows))
            }
            #[cfg(feature = "evidence-cli-ladybug")]
            "search_evidence" => {
                use cognicode_core::application::evidence_backend;

                let workspace = arguments
                    .get("workspace")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let query = arguments
                    .get("query")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        InterfaceError::InvalidInput(
                            "missing required parameter `query`".to_string(),
                        )
                    })?;
                let limit = arguments
                    .get("limit")
                    .and_then(|v| v.as_u64())
                    .map(|n| n as usize)
                    .unwrap_or(25);

                let factory = evidence_backend::evidence_backend_factory().ok_or_else(|| {
                    InterfaceError::Internal(
                        "evidence backend not registered; the MCP server build does not \
                         have the `evidence-cli-ladybug` feature enabled, or the runtime \
                         adapter did not register a factory at startup."
                            .to_string(),
                    )
                })?;
                let backend = factory(None).map_err(InterfaceError::Internal)?;
                let rows = backend
                    .search(workspace, query, limit)
                    .map_err(InterfaceError::Internal)?;
                Ok(cognicode_core::application::evidence_rows::render_evidence_rows_json(rows))
            }
            "get_complexity" => {
                let input: cognicode_core::interface::mcp::schemas::GetComplexityInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    cognicode_core::interface::mcp::handlers::handle_get_complexity(ctx, input)
                        .await?;
                Ok(serde_json::to_string(&output)?)
            }
            "get_entry_points" => {
                let input: cognicode_core::interface::mcp::schemas::GetEntryPointsInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    cognicode_core::interface::mcp::handlers::handle_get_entry_points(ctx, input)
                        .await?;
                Ok(serde_json::to_string(&output)?)
            }
            "get_leaf_functions" => {
                let input: cognicode_core::interface::mcp::schemas::GetLeafFunctionsInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    cognicode_core::interface::mcp::handlers::handle_get_leaf_functions(ctx, input)
                        .await?;
                Ok(serde_json::to_string(&output)?)
            }
            "trace_path" => {
                let input: cognicode_core::interface::mcp::schemas::TracePathInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    cognicode_core::interface::mcp::handlers::handle_trace_path(ctx, input).await?;
                Ok(serde_json::to_string(&output)?)
            }
            "export_mermaid" => {
                let input: cognicode_core::interface::mcp::schemas::ExportMermaidInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    cognicode_core::interface::mcp::handlers::handle_export_mermaid(ctx, input)
                        .await?;
                Ok(serde_json::to_string(&output)?)
            }
            "get_hot_paths" => {
                let input: cognicode_core::interface::mcp::schemas::GetHotPathsInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    cognicode_core::interface::mcp::handlers::handle_get_hot_paths(ctx, input)
                        .await?;
                Ok(serde_json::to_string(&output)?)
            }
            "get_all_symbols" => {
                let input: cognicode_core::interface::mcp::schemas::GetAllSymbolsInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    cognicode_core::interface::mcp::handlers::handle_get_all_symbols(ctx, input)
                        .await?;
                Ok(serde_json::to_string(&output)?)
            }
            "find_dead_code" => {
                let input: cognicode_core::interface::mcp::schemas::FindDeadCodeInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    cognicode_core::interface::mcp::handlers::handle_find_dead_code(ctx, input)
                        .await?;
                Ok(serde_json::to_string(&output)?)
            }
            "get_module_dependencies" => {
                let input: cognicode_core::interface::mcp::schemas::GetModuleDependenciesInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    cognicode_core::interface::mcp::handlers::handle_get_module_dependencies(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string(&output)?)
            }

            "query_symbol_index" => {
                let input: cognicode_core::interface::mcp::schemas::QuerySymbolInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    cognicode_core::interface::mcp::handlers::handle_query_symbol_index(ctx, input)
                        .await?;
                Ok(serde_json::to_string(&output)?)
            }
            "build_call_subgraph" => {
                let input: cognicode_core::interface::mcp::schemas::BuildSubgraphInput =
                    serde_json::from_value(arguments.into())?;
                let output = cognicode_core::interface::mcp::handlers::handle_build_call_subgraph(
                    ctx, input,
                )
                .await?;
                Ok(serde_json::to_string(&output)?)
            }
            "get_per_file_graph" => {
                let input: cognicode_core::interface::mcp::schemas::GetPerFileGraphInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    cognicode_core::interface::mcp::handlers::handle_get_per_file_graph(ctx, input)
                        .await?;
                Ok(serde_json::to_string(&output)?)
            }

            "get_symbol_code" => {
                let input: cognicode_core::interface::mcp::schemas::SymbolCodeInput =
                    serde_json::from_value(arguments.into())?;
                let output = cognicode_core::interface::mcp::handlers::handle_get_symbol_code(
                    ctx.symbol_code().clone(),
                    ctx.validator().clone(),
                    ctx.working_dir().to_path_buf(),
                    input,
                )
                .await?;
                Ok(serde_json::to_string(&output)?)
            }

            "go_to_definition" => {
                let input: cognicode_core::interface::mcp::schemas::GoToDefinitionInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    cognicode_core::interface::mcp::handlers::lsp_handlers::handle_go_to_definition(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string(&output)?)
            }
            "hover" => {
                let input: cognicode_core::interface::mcp::schemas::HoverInput =
                    serde_json::from_value(arguments.into())?;
                let output = cognicode_core::interface::mcp::handlers::lsp_handlers::handle_hover(
                    ctx, input,
                )
                .await?;
                Ok(serde_json::to_string(&output)?)
            }
            "find_references" => {
                let input: cognicode_core::interface::mcp::schemas::FindReferencesInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    cognicode_core::interface::mcp::handlers::lsp_handlers::handle_find_references(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string(&output)?)
            }
            "rename_preview" => {
                let input: cognicode_core::interface::mcp::schemas::RenamePreviewInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    cognicode_core::interface::mcp::handlers::lsp_handlers::handle_rename_preview(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string(&output)?)
            }
            "read_file" => {
                let input: cognicode_core::interface::mcp::schemas::ReadFileInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    cognicode_core::interface::mcp::handlers::handle_read_file(ctx, input).await?;
                Ok(serde_json::to_string(&output)?)
            }
            "write_file" => {
                let input: cognicode_core::interface::mcp::schemas::WriteFileInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    cognicode_core::interface::mcp::handlers::handle_write_file(ctx, input).await?;
                Ok(serde_json::to_string(&output)?)
            }
            "edit_file" => {
                let input: cognicode_core::interface::mcp::schemas::EditFileInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    cognicode_core::interface::mcp::handlers::handle_edit_file(ctx, input).await?;
                Ok(serde_json::to_string(&output)?)
            }
            "search_content" => {
                let input: cognicode_core::interface::mcp::schemas::SearchContentInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    cognicode_core::interface::mcp::handlers::handle_search_content(ctx, input)
                        .await?;
                Ok(serde_json::to_string(&output)?)
            }
            "list_files" => {
                let input: cognicode_core::interface::mcp::schemas::ListFilesInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    cognicode_core::interface::mcp::handlers::handle_list_files(ctx, input).await?;
                Ok(serde_json::to_string(&output)?)
            }
            "retrieve_and_verify" => {
                let input: cognicode_core::interface::mcp::schemas::RetrieveAndVerifyInput =
                    serde_json::from_value(arguments.into())?;
                let output = cognicode_core::interface::mcp::handlers::handle_retrieve_and_verify(
                    ctx, input,
                )
                .await?;
                Ok(serde_json::to_string(&output)?)
            }
            // AIX-1: Smart Overview & Ranked Symbols

            // AIX-2: Onboarding Plan & Auto Diagnose & Refactor Plan

            // AIX-3: NL to Symbol & Ask About Code & Find Pattern
            "nl_to_symbol" => {
                let input: cognicode_core::interface::mcp::schemas::NlToSymbolInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    cognicode_core::interface::mcp::handlers::aix_handlers::handle_nl_to_symbol(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string(&output)?)
            }
            "ask_about_code" => {
                let input: cognicode_core::interface::mcp::schemas::AskAboutCodeInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    cognicode_core::interface::mcp::handlers::aix_handlers::handle_ask_about_code(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string(&output)?)
            }
            "find_pattern_by_intent" => {
                let input: cognicode_core::interface::mcp::schemas::FindPatternByIntentInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    cognicode_core::interface::mcp::handlers::aix_handlers::handle_find_pattern_by_intent(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string(&output)?)
            }
            // AIX-4: Compare Call Graphs & Detect API Breaks

            // AIX-5: System Prompt Context & God Functions & Long Params
            "detect_god_functions" => {
                let input: cognicode_core::interface::mcp::schemas::DetectGodFunctionsInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    cognicode_core::interface::mcp::handlers::aix_handlers::handle_detect_god_functions(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string(&output)?)
            }
            "detect_long_parameter_lists" => {
                let input: cognicode_core::interface::mcp::schemas::DetectLongParamsInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                cognicode_core::interface::mcp::handlers::aix_handlers::handle_detect_long_parameter_lists(
                    ctx, input,
                )
                .await?;
                Ok(serde_json::to_string(&output)?)
            }
            // PL3: Symbol Hotness Tracking

            // AVC: Agent-Verifiable Context tools
            #[cfg(feature = "contract-store")]
            "generate_contract" => {
                let input: cognicode_core::interface::mcp::schemas::GenerateContractInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    cognicode_core::interface::mcp::handlers::aix_handlers::handle_generate_contract(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string(&output)?)
            }
            #[cfg(feature = "contract-store")]
            "validate_contract" => {
                let input: cognicode_core::interface::mcp::schemas::ValidateContractInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    cognicode_core::interface::mcp::handlers::aix_handlers::handle_validate_contract(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string(&output)?)
            }
            // Phase 3A: Proactive Tools
            #[cfg(feature = "persistence")]
            // Detect Drift tool (S7000-S7003)
            "detect_drift" => {
                let input: cognicode_core::interface::mcp::schemas::DetectDriftInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    cognicode_core::interface::mcp::handlers::aix_handlers::handle_detect_drift(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string(&output)?)
            }
            // Batch D: Agent Task Tools (bidirectional interaction)

            // Phase 4b: Graph analytics tools (extracted to graph_handlers.rs)
            "graph_pagerank" => {
                let input: cognicode_core::interface::mcp::schemas::GraphPageRankInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    cognicode_core::interface::mcp::handlers::graph_handlers::handle_graph_pagerank(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string(&output)?)
            }
            "graph_all_paths" => {
                let input: cognicode_core::interface::mcp::schemas::GraphAllPathsInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    cognicode_core::interface::mcp::handlers::graph_handlers::handle_graph_all_paths(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string(&output)?)
            }
            "graph_condensed" => {
                let input: cognicode_core::interface::mcp::schemas::GraphCondensedInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    cognicode_core::interface::mcp::handlers::graph_handlers::handle_graph_condensed(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string(&output)?)
            }
            "graph_god_nodes" => {
                let input: cognicode_core::interface::mcp::schemas::GraphGodNodesInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    cognicode_core::interface::mcp::handlers::graph_handlers::handle_graph_god_nodes(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string(&output)?)
            }
            "graph_reduced" => {
                let input: cognicode_core::interface::mcp::schemas::GraphReducedInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    cognicode_core::interface::mcp::handlers::graph_handlers::handle_graph_reduced(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string(&output)?)
            }
            "graph_feedback_arcs" => {
                let input: cognicode_core::interface::mcp::schemas::GraphFeedbackArcsInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    cognicode_core::interface::mcp::handlers::graph_handlers::handle_graph_feedback_arcs(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string(&output)?)
            }

            // Phase 5: Community Detection handlers (extracted to graph_handlers.rs)
            "graph_communities" => {
                let input: cognicode_core::interface::mcp::schemas::GraphCommunitiesInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    cognicode_core::interface::mcp::handlers::graph_handlers::handle_graph_communities(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string(&output)?)
            }
            "graph_community_detail" => {
                let input: cognicode_core::interface::mcp::schemas::GraphCommunityDetailInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    cognicode_core::interface::mcp::handlers::graph_handlers::handle_graph_community_detail(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string(&output)?)
            }
            "graph_surprising_connections" => {
                let input: cognicode_core::interface::mcp::schemas::GraphSurprisingConnectionsInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                cognicode_core::interface::mcp::handlers::graph_handlers::handle_graph_surprising_connections(
                    ctx, input,
                )
                .await?;
                Ok(serde_json::to_string(&output)?)
            }

            // Phase 6: IDF-weighted Search & Unified Insights (extracted to graph_handlers.rs)
            "graph_search_idf" => {
                let input: cognicode_core::interface::mcp::schemas::GraphSearchIdfInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    cognicode_core::interface::mcp::handlers::graph_handlers::handle_graph_search_idf(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string(&output)?)
            }
            "graph_insights" => {
                let input: cognicode_core::interface::mcp::schemas::GraphInsightsInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    cognicode_core::interface::mcp::handlers::graph_handlers::handle_graph_insights(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string(&output)?)
            }
            "graph_suggest_questions" => {
                let input: cognicode_core::interface::mcp::schemas::GraphSuggestQuestionsInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                cognicode_core::interface::mcp::handlers::graph_handlers::handle_graph_suggest_questions(
                    ctx, input,
                )
                .await?;
                Ok(serde_json::to_string(&output)?)
            }
            // Sprint 2: Graphify-style tools (ADR-026)
            "graph_query" => {
                let input: cognicode_core::interface::mcp::handlers::graph_query_handlers::GraphQueryInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    cognicode_core::interface::mcp::handlers::graph_query_handlers::handle_graph_query(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string(&output)?)
            }
            "graph_explain" => {
                let input: cognicode_core::interface::mcp::handlers::graph_query_handlers::GraphExplainInput =
                serde_json::from_value(arguments.into())?;
                let output =
                    cognicode_core::interface::mcp::handlers::graph_query_handlers::handle_graph_explain(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string(&output)?)
            }
            // Edge-type query tools (ADR-026)
            "get_type_references" => {
                let input: cognicode_core::interface::mcp::handlers::graph_query_handlers::GetTypeRefsInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                cognicode_core::interface::mcp::handlers::graph_query_handlers::handle_get_type_references(
                    ctx, input,
                )
                .await?;
                Ok(serde_json::to_string(&output)?)
            }
            "get_imports" => {
                let input: cognicode_core::interface::mcp::handlers::graph_query_handlers::GetImportsInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    cognicode_core::interface::mcp::handlers::graph_query_handlers::handle_get_imports(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string(&output)?)
            }
            "get_implementors" => {
                let input: cognicode_core::interface::mcp::handlers::graph_query_handlers::GetImplementorsInput =
                serde_json::from_value(arguments.into())?;
                let output =
                    cognicode_core::interface::mcp::handlers::graph_query_handlers::handle_get_implementors(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string(&output)?)
            }
            "get_members" => {
                let input: cognicode_core::interface::mcp::handlers::graph_query_handlers::GetMembersInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    cognicode_core::interface::mcp::handlers::graph_query_handlers::handle_get_members(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string(&output)?)
            }
            "graph_query_filtered" => {
                let input: cognicode_core::interface::mcp::handlers::graph_query_handlers::GraphQueryFilteredInput = serde_json::from_value(arguments.into())?;
                let output =
                cognicode_core::interface::mcp::handlers::graph_query_handlers::handle_graph_query_filtered(
                    ctx, input,
                )
                .await?;
                Ok(serde_json::to_string(&output)?)
            }
            "export_callflow" => {
                let input: cognicode_core::interface::mcp::handlers::graph_query_handlers::ExportCallflowInput =
                serde_json::from_value(arguments.into())?;
                let output =
                    cognicode_core::interface::mcp::handlers::graph_query_handlers::handle_export_callflow(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string(&output)?)
            }
            // Sprint 5: Consolidated + High-value tools (ADR-027 + ADR-028)
            "smart_search" => {
                let input: cognicode_core::interface::mcp::schemas::SmartSearchInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    cognicode_core::interface::mcp::handlers::consolidated_handlers::handle_smart_search(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string(&output)?)
            }
            "graph_analyze" => {
                let input: cognicode_core::interface::mcp::handlers::retired::GraphAnalyzeInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    cognicode_core::interface::mcp::handlers::retired::handle_graph_analyze(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string(&output)?)
            }
            "project_overview" => {
                let input: cognicode_core::interface::mcp::handlers::consolidated_handlers::ProjectOverviewInput = serde_json::from_value(arguments.into())?;
                let output =
                cognicode_core::interface::mcp::handlers::consolidated_handlers::handle_project_overview(
                    ctx, input,
                )
                .await?;
                Ok(serde_json::to_string(&output)?)
            }
            "codebase_map" => {
                let input: cognicode_core::interface::mcp::handlers::consolidated_handlers::CodebaseMapInput =
                serde_json::from_value(arguments.into())?;
                let output =
                    cognicode_core::interface::mcp::handlers::consolidated_handlers::handle_codebase_map(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string(&output)?)
            }
            "project_insights" => {
                let input: cognicode_core::interface::mcp::handlers::consolidated_handlers::ProjectInsightsInput = serde_json::from_value(arguments.into())?;
                let output =
                cognicode_core::interface::mcp::handlers::consolidated_handlers::handle_project_insights(
                    ctx, input,
                )
                .await?;
                Ok(serde_json::to_string(&output)?)
            }
            "review_pr" => {
                let input: cognicode_core::interface::mcp::handlers::consolidated_handlers::ReviewPrInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    cognicode_core::interface::mcp::handlers::consolidated_handlers::handle_review_pr(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string(&output)?)
            }
            "iac_query" => {
                let input: cognicode_core::interface::mcp::handlers::consolidated_handlers::IacQueryInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    cognicode_core::interface::mcp::handlers::consolidated_handlers::handle_iac_query(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string(&output)?)
            }
            // SOLID Audit tool
            "solid_audit" => {
                let output =
                    cognicode_core::interface::mcp::handlers::handle_solid_audit(ctx).await?;
                Ok(serde_json::to_string(&output)?)
            }
            // Architecture check
            "check_architecture" => {
                let input: cognicode_core::interface::mcp::schemas::CheckArchitectureInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    cognicode_core::interface::mcp::handlers::handle_check_architecture(ctx, input)
                        .await?;
                Ok(serde_json::to_string(&output)?)
            }
            // Graph checkpoint management
            "graph_checkpoint" => {
                let input: cognicode_core::interface::mcp::handlers::consolidated_handlers::GraphCheckpointInput =
                serde_json::from_value(arguments.into())?;
                let output = cognicode_core::interface::mcp::handlers::consolidated_handlers::handle_graph_checkpoint(
                ctx, input,
            )
            .await?;
                Ok(serde_json::to_string(&output)?)
            }
            // Merge per-file graphs into consolidated project graph
            "merge_graphs" => {
                let input: cognicode_core::interface::mcp::schemas::MergeGraphsInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    cognicode_core::interface::mcp::handlers::handle_merge_graphs(ctx, input)
                        .await?;
                Ok(serde_json::to_string(&output)?)
            }
            // Build lightweight symbol index
            "build_lightweight_index" => {
                let input: cognicode_core::interface::mcp::schemas::BuildIndexInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    cognicode_core::interface::mcp::handlers::handle_build_lightweight_index(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string(&output)?)
            }
            // Incremental reindex after edits (requires persistence feature)
            #[cfg(feature = "persistence")]
            "reparse_on_edit" => {
                let input: cognicode_core::interface::mcp::schemas::ReparseOnEditInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    cognicode_core::interface::mcp::handlers::aix_handlers::handle_reparse_on_edit(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string(&output)?)
            }
            #[cfg(not(feature = "persistence"))]
            "reparse_on_edit" => {
                let _ = ctx;
                Err(InterfaceError::Internal(
                    "reparse_on_edit requires the `persistence` feature".to_string(),
                ))
            }
            // Sprint 5.3: graph_diff and graph_timeline tools
            // ViewSpec tools (ADR-008)
            "list_view_specs" => {
                let input: cognicode_core::interface::mcp::schemas::ListViewSpecsInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    cognicode_core::interface::mcp::handlers::consolidated_handlers::handle_list_view_specs(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string(&output)?)
            }
            "read_view_spec" => {
                let input: cognicode_core::interface::mcp::schemas::ReadViewSpecInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    cognicode_core::interface::mcp::handlers::consolidated_handlers::handle_read_view_spec(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string(&output)?)
            }

            // M5.2 WU2: program-analysis dispatch arms.
            "cfg_per_function" => {
                let input: cognicode_core::interface::mcp::handlers::program_analysis_handlers::ProgramAnalysisToolInput =
                    serde_json::from_value(arguments.into())?;
                cognicode_core::interface::mcp::handlers::program_analysis_handlers::handle_cfg(
                    input,
                )
                .map_err(InterfaceError::Internal)
            }
            "dominators_cfg" => {
                let input: cognicode_core::interface::mcp::handlers::program_analysis_handlers::ProgramAnalysisToolInput =
                    serde_json::from_value(arguments.into())?;
                cognicode_core::interface::mcp::handlers::program_analysis_handlers::handle_dominators_cfg(
                    input,
                )
                .map_err(InterfaceError::Internal)
            }
            "slice_forward" => {
                let input: cognicode_core::interface::mcp::handlers::program_analysis_handlers::ProgramAnalysisToolInput =
                    serde_json::from_value(arguments.into())?;
                cognicode_core::interface::mcp::handlers::program_analysis_handlers::handle_slice_forward(
                    input,
                )
                .map_err(InterfaceError::Internal)
            }
            "slice_backward" => {
                let input: cognicode_core::interface::mcp::handlers::program_analysis_handlers::ProgramAnalysisToolInput =
                    serde_json::from_value(arguments.into())?;
                cognicode_core::interface::mcp::handlers::program_analysis_handlers::handle_slice_backward(
                    input,
                )
                .map_err(InterfaceError::Internal)
            }
            "taint_flow" => {
                let input: cognicode_core::interface::mcp::handlers::program_analysis_handlers::ProgramAnalysisToolInput =
                    serde_json::from_value(arguments.into())?;
                cognicode_core::interface::mcp::handlers::program_analysis_handlers::handle_taint_flow(input)
                    .map_err(InterfaceError::Internal)
            }
            #[cfg(feature = "program-analysis-server")]
            "interproc_summary" => {
                let input: cognicode_core::interface::mcp::handlers::program_analysis_handlers::ProgramAnalysisToolInput =
                    serde_json::from_value(arguments.into())?;
                cognicode_core::interface::mcp::handlers::program_analysis_handlers::handle_interproc_summary(input)
                    .map_err(InterfaceError::Internal)
            }
            // The findings pipeline grounds its findings in the canonical
            // evidence kernel, so the tool is dispatched only in the builds
            // that carry it. The catalogue declares the same gate, so a
            // build without the kernel does not advertise it either.
            #[cfg(feature = "evidence-kernel")]
            "explain_finding" => {
                let input: cognicode_core::interface::mcp::handlers::findings_handlers::ExplainFindingInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    cognicode_core::interface::mcp::handlers::findings_handlers::handle_explain_finding(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string(&output)?)
            }

            _ => Err(InterfaceError::ToolNotFound(tool_name.to_string())),
        }
    };
    let result = match tokio::time::timeout(timeout, dispatch).await {
        Ok(inner) => inner,
        Err(_elapsed) => Err(InterfaceError::Timeout),
    };

    // M1.2: Classify status for metrics
    let status = crate::status::classify_status(tool_name, &result);

    // M1.1: Record duration + classify status for error recording
    let duration_ms = start.elapsed().as_millis() as f64;

    // M3.4: Structured per-call log line — one entry per tool call
    // (success, error, gated, missing, skipped). Emitted at the
    // universal instrumentation boundary so every tool flow is captured
    // uniformly. Field names match M3-Sprint-spec.md §M3.4.
    tracing::info!(
        tool = %tool_name,
        duration_ms = %duration_ms as u64,
        status = %status,
        "tool_call"
    );
    if let Some(m) = &metrics {
        // M1.6: Record calls with tool + status labels
        m.calls.add(
            1,
            &[
                KeyValue::new("tool", tool_name.to_string()),
                KeyValue::new("status", status),
            ],
        );
        // M1.6: Record duration with tool + status labels
        m.duration.record(
            duration_ms,
            &[
                KeyValue::new("tool", tool_name.to_string()),
                KeyValue::new("status", status),
            ],
        );
    }

    // M1.1: Record error metrics (error_type is separate from status)
    if let Err(e) = &result
        && let Some(m) = &metrics
    {
        // M3.2 / M3.3: Distinguish timeout and rate_limit_exceeded
        // from generic errors. The kind of the failure is its type; the
        // taxonomy is machine-readable without reading the message.
        let error_type = crate::status::error_type(e);
        m.errors.add(
            1,
            &[
                KeyValue::new("tool", tool_name.to_string()),
                KeyValue::new("error_type", error_type),
            ],
        );
    }

    result
}
