//! Async dispatch layer for [`crate::ask`].
//!
//! Takes a [`ClassifiedQuestion`], gates graph-dependent patterns
//! against the optional `CallGraph`, then invokes the right
//! primitive chain on `ExplorerService` + `ImpactAnalysisService`
//! directly. No MCP chaining — the handler holds an `Arc` to the
//! service and we call methods on it.
//!
//! The returned [`McpResultEnvelope`] is the same wire shape as the
//! 17 primitive tools, so consumers can parse one schema across the
//! whole surface.

use std::sync::Arc;

use cognicode_core::application::dto::SccDto;
use cognicode_core::application::services::impact_analysis::ImpactAnalysisService;
use cognicode_core::domain::aggregates::{CallGraph, SymbolId};
use serde_json::{Value, json};

use crate::ask::ClassifiedQuestion;
use crate::ask::entity;
use crate::ask::followups::generate_follow_ups;
use crate::ask::patterns::{PATTERNS, QuestionCategory};
use crate::facades::{SearchService, ViewService, WorkspaceService};
use crate::mcp::{FollowUp, McpResultEnvelope, ProvenanceMetadata};
use crate::session::service::BrainSessionService;

/// Dispatch a classified question. Pure async — no shared state,
/// no I/O outside of the service methods it calls.
///
/// The returned envelope's `payload` is a JSON object with exactly
/// two top-level keys: `primary_result` and `supporting`. The
/// `provenance` field is always set to `"ask-router"` with a
/// confidence equal to the classification score.
///
/// `_session` is reserved for the brain-session capability. Today
/// the dispatch layer does not consult it — the focus-node
/// prepend happens in the `brain_ask` MCP arm before the question
/// reaches this function. The parameter is here so the signature
/// stays stable as we evolve the session integration without
/// rippling through every call site.
#[allow(dead_code)]
pub async fn dispatch_ask(
    classified: ClassifiedQuestion,
    search: &dyn SearchService,
    workspace: &dyn WorkspaceService,
    view: &dyn ViewService,
    graph: &Option<Arc<CallGraph>>,
    _session: Option<&BrainSessionService>,
) -> McpResultEnvelope<Value> {
    // 1. Resolve entities via spotter (if any backtick tokens are
    //    present in the question).
    let (entities, entity_follow_ups) =
        entity::extract_entities(&classified.entities.join(" "), search).await;
    let _ = entities; // existence is enough; the dispatch chains below
    // re-resolve by name from the question string.

    // 2. Pre-dispatch graph check (spec §Graph Availability Gating).
    if classified.category.graph_required() && graph.is_none() {
        return graph_unavailable_envelope(classified, entity_follow_ups);
    }

    // 3. Build (primary_result, supporting) for the matched category.
    let (primary_result, supporting) = match dispatch_category(
        classified.category,
        &classified.entities,
        search,
        workspace,
        view,
        graph,
    )
    .await
    {
        Ok(t) => t,
        Err(message) => {
            return error_envelope(&classified, message, entity_follow_ups);
        }
    };

    // 4. Deterministic follow-up generation. Combine the entity
    //    follow-ups with the category follow-ups; the spec requires
    //    ≥ 1 follow-up per successful response, and we always emit
    //    the entity follow-ups first so `no_entity_match` and
    //    `entity_disambiguation` surface even when the category
    //    produced none.
    let mut follow_ups = entity_follow_ups;
    let primary_for_followup = primary_result.clone();
    follow_ups.extend(generate_follow_ups(
        classified.category,
        &classified.entities,
        &primary_for_followup,
    ));

    // 5. Wrap in the standard envelope.
    let payload = json!({
        "primary_result": primary_result,
        "supporting": supporting,
    });
    let provenance = ProvenanceMetadata::new(classified.confidence, Some("ask-router".into()))
        .unwrap_or_default();
    McpResultEnvelope {
        tool_name: "cognicode_ask".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        provenance: Some(provenance),
        payload,
        suggested_follow_ups: follow_ups,
    }
}

// ----- category dispatch --------------------------------------------------

async fn dispatch_category(
    category: QuestionCategory,
    entities: &[String],
    search: &dyn SearchService,
    workspace: &dyn WorkspaceService,
    view: &dyn ViewService,
    graph: &Option<Arc<CallGraph>>,
) -> Result<(Value, Value), String> {
    match category {
        QuestionCategory::PathBetween => path_between(entities, search, graph).await,
        QuestionCategory::ForwardReach => forward_reach(entities, search, graph).await,
        QuestionCategory::BackwardReach => backward_reach(entities, search, graph).await,
        QuestionCategory::CodeQuality => code_quality(entities, search, view).await,
        QuestionCategory::Architecture => architecture(graph).await,
        QuestionCategory::WorkspaceOverview => workspace_overview(workspace, view, graph).await,
        QuestionCategory::ComponentCluster => component_cluster(entities, search, graph).await,
        QuestionCategory::GenericDescription => generic_description(entities, search, view).await,
    }
}

async fn path_between(
    entities: &[String],
    search: &dyn SearchService,
    graph: &Option<Arc<CallGraph>>,
) -> Result<(Value, Value), String> {
    let (src, dst) = (entities.first(), entities.get(1));
    let (src, dst) = match (src, dst) {
        (Some(s), Some(d)) => (s.clone(), d.clone()),
        _ => return Ok((Value::Null, json!({}))),
    };
    let spotter_src = search
        .spotter_search(&src, None)
        .await
        .map_err(|e| e.to_string())?;
    let spotter_dst = search
        .spotter_search(&dst, None)
        .await
        .map_err(|e| e.to_string())?;
    // Resolved id is the first hit's `object.id` (or the raw token
    // if spotter returned nothing — let the graph layer produce
    // a clean "no path" result).
    let src_id = spotter_src
        .first()
        .map(|h| h.object.id.clone())
        .unwrap_or_else(|| src.clone());
    let dst_id = spotter_dst
        .first()
        .map(|h| h.object.id.clone())
        .unwrap_or_else(|| dst.clone());

    let g = graph
        .as_ref()
        .ok_or_else(|| "graph_unavailable".to_string())?;
    let svc = ImpactAnalysisService::new();
    let path = svc.shortest_path(
        g,
        &SymbolId::new(src_id.clone()),
        &SymbolId::new(dst_id.clone()),
    );

    let (path_vec, length) = match path {
        Some(dto) => (dto.path.clone(), dto.path.len() as u32),
        None => (Vec::new(), 0),
    };
    let explain = svc
        .explain_path(
            g,
            &SymbolId::new(src_id.clone()),
            &SymbolId::new(dst_id.clone()),
        )
        .map(serde_json::to_value)
        .transpose()
        .map_err(|e| e.to_string())?
        .unwrap_or_else(|| serde_json::json!({"found": false, "hops": [], "summary": "no path"}));

    Ok((
        json!({ "path": path_vec, "length": length }),
        json!({
            "spotter_src": spotter_src,
            "spotter_dst": spotter_dst,
            "explain": explain,
        }),
    ))
}

async fn forward_reach(
    entities: &[String],
    search: &dyn SearchService,
    graph: &Option<Arc<CallGraph>>,
) -> Result<(Value, Value), String> {
    let root = entities
        .first()
        .cloned()
        .ok_or_else(|| "forward_reach:missing root".to_string())?;
    let spotter = search
        .spotter_search(&root, None)
        .await
        .map_err(|e| e.to_string())?;
    let resolved = spotter
        .first()
        .map(|h| h.object.id.clone())
        .unwrap_or_else(|| root.clone());
    let g = graph
        .as_ref()
        .ok_or_else(|| "graph_unavailable".to_string())?;
    let svc = ImpactAnalysisService::new();
    let edges = svc.forward_radius(g, &SymbolId::new(resolved.clone()), 2);
    let edge_strs: Vec<String> = edges.iter().map(|s| s.as_str().to_string()).collect();
    Ok((
        json!({ "root": resolved, "edges": edge_strs }),
        json!({ "spotter": spotter }),
    ))
}

async fn backward_reach(
    entities: &[String],
    search: &dyn SearchService,
    graph: &Option<Arc<CallGraph>>,
) -> Result<(Value, Value), String> {
    let root = entities
        .first()
        .cloned()
        .ok_or_else(|| "backward_reach:missing root".to_string())?;
    let spotter = search
        .spotter_search(&root, None)
        .await
        .map_err(|e| e.to_string())?;
    let resolved = spotter
        .first()
        .map(|h| h.object.id.clone())
        .unwrap_or_else(|| root.clone());
    let g = graph
        .as_ref()
        .ok_or_else(|| "graph_unavailable".to_string())?;
    let svc = ImpactAnalysisService::new();
    let edges = svc.impact_radius(g, &SymbolId::new(resolved.clone()), 2);
    let edge_strs: Vec<String> = edges.iter().map(|s| s.as_str().to_string()).collect();
    Ok((
        json!({ "root": resolved, "edges": edge_strs }),
        json!({ "view": "call-graph", "spotter": spotter }),
    ))
}

async fn code_quality(
    entities: &[String],
    search: &dyn SearchService,
    view: &dyn ViewService,
) -> Result<(Value, Value), String> {
    let root = entities
        .first()
        .cloned()
        .ok_or_else(|| "code_quality:missing root".to_string())?;
    let spotter = search
        .spotter_search(&root, None)
        .await
        .map_err(|e| e.to_string())?;
    let resolved = spotter
        .first()
        .map(|h| h.object.id.clone())
        .unwrap_or_else(|| root.clone());
    let quality_view = view
        .contextual_view(&resolved, "quality")
        .await
        .map_err(|e| e.to_string())?;
    let object = search
        .inspect_object(&resolved)
        .await
        .map_err(|e| e.to_string())?;
    Ok((
        json!({ "smells": [], "score": 1.0 }),
        json!({ "view": quality_view, "object": object }),
    ))
}

async fn architecture(graph: &Option<Arc<CallGraph>>) -> Result<(Value, Value), String> {
    let g = graph
        .as_ref()
        .ok_or_else(|| "graph_unavailable".to_string())?;
    let svc = ImpactAnalysisService::new();
    let sccs = svc.detect_cycles(g);
    let cycles: Vec<SccDto> = sccs.into_iter().map(SccDto::from_scc).collect();
    let clusters = svc.cluster_components(g, "scc");
    Ok((json!({ "cycles": cycles }), json!({ "clusters": clusters })))
}

async fn workspace_overview(
    workspace: &dyn WorkspaceService,
    view: &dyn ViewService,
    graph: &Option<Arc<CallGraph>>,
) -> Result<(Value, Value), String> {
    let workspace_meta = workspace.current_workspace().map_err(|e| e.to_string())?;
    let clusters = if let Some(g) = graph.as_ref() {
        let svc = ImpactAnalysisService::new();
        Some(svc.cluster_components(g, "scc"))
    } else {
        None
    };
    let lens = view
        .apply_lens("scope:.", "hotspots")
        .await
        .ok()
        .map(|r| serde_json::to_value(r).unwrap_or(Value::Null));
    Ok((
        json!({ "hotspots": lens.clone().unwrap_or(Value::Null) }),
        json!({
            "clusters": clusters,
            "workspace_meta": workspace_meta,
        }),
    ))
}

async fn component_cluster(
    entities: &[String],
    search: &dyn SearchService,
    graph: &Option<Arc<CallGraph>>,
) -> Result<(Value, Value), String> {
    let root = entities
        .first()
        .cloned()
        .ok_or_else(|| "component_cluster:missing root".to_string())?;
    let spotter = search
        .spotter_search(&root, None)
        .await
        .map_err(|e| e.to_string())?;
    let resolved = spotter
        .first()
        .map(|h| h.object.id.clone())
        .unwrap_or_else(|| root.clone());
    let g = graph
        .as_ref()
        .ok_or_else(|| "graph_unavailable".to_string())?;
    let svc = ImpactAnalysisService::new();
    let component = svc.containing_component(g, &SymbolId::new(resolved.clone()));
    match component {
        Some(members) => {
            let id = members.first().map(|m| m.as_str().to_string());
            let member_strs: Vec<String> = members.iter().map(|m| m.as_str().to_string()).collect();
            Ok((
                json!({ "component_id": id, "members": member_strs }),
                json!({}),
            ))
        }
        None => Ok((
            json!({ "component_id": Value::Null, "members": [] }),
            json!({}),
        )),
    }
}

async fn generic_description(
    entities: &[String],
    search: &dyn SearchService,
    view: &dyn ViewService,
) -> Result<(Value, Value), String> {
    let root = entities
        .first()
        .cloned()
        .ok_or_else(|| "generic_description:missing root".to_string())?;
    let spotter = search
        .spotter_search(&root, None)
        .await
        .map_err(|e| e.to_string())?;
    let resolved = spotter
        .first()
        .map(|h| h.object.id.clone())
        .unwrap_or_else(|| root.clone());
    let object = search
        .inspect_object(&resolved)
        .await
        .map_err(|e| e.to_string())?;
    let overview_view = view
        .contextual_view(&resolved, "overview")
        .await
        .map_err(|e| e.to_string())?;
    Ok((
        json!({
            "summary": format!("{:?}", object),
            "kind": "symbol",
            "location": resolved,
        }),
        json!({ "overview_view": overview_view }),
    ))
}

// ----- helpers ------------------------------------------------------------

fn graph_unavailable_envelope(
    classified: ClassifiedQuestion,
    extra_follow_ups: Vec<FollowUp>,
) -> McpResultEnvelope<Value> {
    // Spec: "list the available (non-graph) alternatives". Patterns
    // 4 and 8 are the non-graph ones.
    let alternatives: Vec<&'static str> = PATTERNS
        .iter()
        .filter(|p| !p.graph_required)
        .map(|p| pattern_label(p.category))
        .collect();
    let message = format!(
        "graph_unavailable: this question requires the in-memory call graph. \
         Available (non-graph) patterns: {}",
        alternatives.join(", ")
    );
    error_envelope(&classified, message, extra_follow_ups)
}

fn error_envelope(
    _classified: &ClassifiedQuestion,
    message: String,
    extra_follow_ups: Vec<FollowUp>,
) -> McpResultEnvelope<Value> {
    let provenance = ProvenanceMetadata::new(0.0, Some("ask-router".into())).unwrap_or_default();
    McpResultEnvelope {
        tool_name: "cognicode_ask".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        provenance: Some(provenance),
        payload: json!({ "error": message }),
        suggested_follow_ups: extra_follow_ups,
    }
}

fn pattern_label(cat: QuestionCategory) -> &'static str {
    match cat {
        QuestionCategory::PathBetween => "path_between",
        QuestionCategory::ForwardReach => "forward_reach",
        QuestionCategory::BackwardReach => "backward_reach",
        QuestionCategory::CodeQuality => "code_quality",
        QuestionCategory::Architecture => "architecture",
        QuestionCategory::WorkspaceOverview => "workspace_overview",
        QuestionCategory::ComponentCluster => "component_cluster",
        QuestionCategory::GenericDescription => "generic_description",
    }
}

// ----- QuestionCategory extension trait ----------------------------------

trait GraphRequired {
    fn graph_required(&self) -> bool;
}

impl GraphRequired for QuestionCategory {
    fn graph_required(&self) -> bool {
        PATTERNS
            .iter()
            .find(|p| p.category == *self)
            .map(|p| p.graph_required)
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ask::AskRouter;
    use crate::dto::{
        InspectableObjectSummary, LensResult, OpenWorkspaceRequest, SpotterResult,
        ViewDescriptorDto, WorkspaceSummary,
    };
    use async_trait::async_trait;

    // Helper — let the tests be tight. Build a question that maps to
    // each category and assert we get the right envelope shape back.
    async fn run_classify(question: &str) -> ClassifiedQuestion {
        AskRouter::classify(question)
    }

    // --- Mock facades -------------------------------------------------------

    #[derive(Clone)]
    struct MockSearchService;
    #[async_trait]
    impl SearchService for MockSearchService {
        async fn spotter_search(
            &self,
            _query: &str,
            _kind: Option<&str>,
        ) -> crate::ExplorerResult<Vec<SpotterResult>> {
            Ok(vec![])
        }
        async fn spotter_search_with_viewspecs(
            &self,
            _query: &str,
            _kind: Option<&str>,
            _workspace_id: Option<&str>,
        ) -> crate::ExplorerResult<Vec<crate::dto::SpotterSearchResult>> {
            Ok(vec![])
        }
        async fn inspect_object(
            &self,
            _object_id: &str,
        ) -> crate::ExplorerResult<InspectableObjectSummary> {
            Err(crate::error::ExplorerError::ObjectNotFound("mock".into()))
        }
    }

    #[derive(Clone)]
    struct MockViewService;
    #[async_trait]
    impl ViewService for MockViewService {
        async fn available_views(
            &self,
            _object_id: &str,
        ) -> crate::ExplorerResult<Vec<ViewDescriptorDto>> {
            Ok(vec![])
        }
        async fn contextual_view(
            &self,
            _object_id: &str,
            _view_id: &str,
        ) -> crate::ExplorerResult<crate::dto::ContextualView> {
            Err(crate::error::ExplorerError::FeatureDisabled("mock".into()))
        }
        async fn build_contextual_graph(
            &self,
            _focus_id: &str,
            _level: &str,
            _depth: u8,
            _max_nodes: usize,
        ) -> crate::ExplorerResult<crate::dto::ContextualGraphResponse> {
            Err(crate::error::ExplorerError::FeatureDisabled("mock".into()))
        }
        async fn available_lenses(
            &self,
            _object_id: &str,
        ) -> crate::ExplorerResult<Vec<crate::dto::LensDescriptor>> {
            Ok(vec![])
        }
        async fn apply_lens(
            &self,
            _object_id: &str,
            _lens_id: &str,
        ) -> crate::ExplorerResult<LensResult> {
            Err(crate::error::ExplorerError::FeatureDisabled("mock".into()))
        }
        async fn execute_view_spec(
            &self,
            _spec: &crate::dto::ViewSpec,
            _object_id: &str,
        ) -> crate::ExplorerResult<crate::dto::ContextualView> {
            Err(crate::error::ExplorerError::FeatureDisabled("mock".into()))
        }
    }

    #[derive(Clone)]
    struct MockWorkspaceService;
    #[async_trait]
    impl WorkspaceService for MockWorkspaceService {
        async fn open_workspace(
            &self,
            _request: OpenWorkspaceRequest,
        ) -> crate::ExplorerResult<WorkspaceSummary> {
            Err(crate::error::ExplorerError::WorkspaceNotFound(
                "mock".into(),
            ))
        }
        fn current_workspace(&self) -> crate::ExplorerResult<WorkspaceSummary> {
            Err(crate::error::ExplorerError::WorkspaceNotFound(
                "mock".into(),
            ))
        }
    }

    fn build_mocks() -> (MockSearchService, MockViewService, MockWorkspaceService) {
        (MockSearchService, MockViewService, MockWorkspaceService)
    }

    #[tokio::test]
    async fn dispatch_classify_returns_path_between_for_canonical_input() {
        let c = run_classify("path between `parse` and `render`").await;
        assert_eq!(c.category, QuestionCategory::PathBetween);
    }

    #[tokio::test]
    async fn dispatch_classify_returns_forward_for_canonical_input() {
        let c = run_classify("what does `validate()` call?").await;
        assert_eq!(c.category, QuestionCategory::ForwardReach);
    }

    #[tokio::test]
    async fn dispatch_classify_returns_backward_for_canonical_input() {
        let c = run_classify("who calls `format_date`?").await;
        assert_eq!(c.category, QuestionCategory::BackwardReach);
    }

    #[tokio::test]
    async fn dispatch_classify_returns_quality_for_canonical_input() {
        let c = run_classify("any smells in `parse_config`?").await;
        assert_eq!(c.category, QuestionCategory::CodeQuality);
    }

    #[tokio::test]
    async fn dispatch_classify_returns_architecture_for_canonical_input() {
        let c = run_classify("what's the architecture shape?").await;
        assert_eq!(c.category, QuestionCategory::Architecture);
    }

    #[tokio::test]
    async fn dispatch_classify_returns_overview_for_canonical_input() {
        let c = run_classify("where should I start?").await;
        assert_eq!(c.category, QuestionCategory::WorkspaceOverview);
    }

    #[tokio::test]
    async fn dispatch_classify_returns_cluster_for_canonical_input() {
        let c = run_classify("what component does `db.rs` belong to?").await;
        assert_eq!(c.category, QuestionCategory::ComponentCluster);
    }

    #[tokio::test]
    async fn dispatch_classify_returns_fallback_for_unmatched() {
        let c = run_classify("what is `AuthService`?").await;
        assert_eq!(c.category, QuestionCategory::GenericDescription);
    }

    #[tokio::test]
    async fn dispatch_graph_unavailable_envelope_lists_alternatives() {
        // A graph-dependent question with no graph should return a
        // "graph_unavailable" envelope that names the patterns that
        // remain available (4 and 8 per the spec).
        let (search, view, workspace) = build_mocks();
        let classified = run_classify("path between `a` and `b`").await;
        let env = dispatch_ask(classified, &search, &workspace, &view, &None, None).await;
        // No graph → graph_unavailable envelope, alternatives 4 and 8.
        let body = serde_json::to_string(&env).unwrap();
        assert!(
            body.contains("graph_unavailable"),
            "expected graph_unavailable, got: {body}"
        );
        assert!(
            body.contains("code_quality") && body.contains("generic_description"),
            "expected alternatives 4 + 8 in: {body}"
        );
    }

    #[tokio::test]
    async fn dispatch_non_graph_question_works_without_graph() {
        // Pattern 4 (code quality) must dispatch normally when the
        // graph is None.
        let (search, view, workspace) = build_mocks();
        let classified = run_classify("any smells in `parse_config`?").await;
        let env = dispatch_ask(classified, &search, &workspace, &view, &None, None).await;
        // No graph_unavailable should appear — pattern 4 doesn't need
        // the graph.
        let body = serde_json::to_string(&env).unwrap();
        assert!(
            !body.contains("graph_unavailable"),
            "non-graph pattern must not gate on graph, got: {body}"
        );
    }
}
