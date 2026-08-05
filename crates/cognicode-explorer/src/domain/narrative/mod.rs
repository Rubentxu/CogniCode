//! Narrative executor family — extracted from views.rs
//!
//! Contains the 4 narrative executors that shape domain objects into
//! navigable narrative views: ComposedNarrative, ProjectDiary, ExampleObject,
//! and EvidencePack.

use async_trait::async_trait;
use serde_json::json;

pub use crate::dto::{ContextualView, ExampleBlock, ExplorationSession, ViewBlock, ViewKind};
pub use crate::dto::{InspectableObjectType, RendererKind, ViewContext};
pub use crate::error::{ExplorerError, ExplorerResult};
pub use crate::facades::investigation::Investigation;

// Re-export ViewDescriptor, ViewExecutor, and InspectionTarget from views for implementors
pub use crate::domain::views::{InspectionTarget, ViewDescriptor, ViewExecutor};
pub use crate::registry::{ProviderWrapper, ViewDescriptorProvider};

// Re-export EvidenceBlock and related types used by narrative builders
pub use crate::dto::EvidenceBlock;
pub use crate::ports::source_reader::SourceReader;
pub use crate::ports::symbol_repository::{ResolvedSymbol, SymbolRepository};
pub use cognicode_core::domain::traits::graph_query_port::GraphQueryPort;

// Embed resolver — parses !view(...) markers in narrative markdown
pub mod embed;
pub use embed::EmbedResolver;

// ============================================================================
// ComposedNarrative — replay ExplorationSession as a navigable narrative
// ============================================================================

/// Pure shaper — no I/O, no async. Shapes an ExplorationSession into a
/// ContextualView with one ViewBlock per navigation event.
pub fn build_composed_narrative(session: &ExplorationSession) -> ContextualView {
    let blocks = if session.events.is_empty() {
        vec![ViewBlock {
            id: "empty".into(),
            title: "No events".into(),
            body: json!({ "message": "No events in this exploration" }),
        }]
    } else {
        session
            .events
            .iter()
            .enumerate()
            .map(|(i, e)| {
                let view_id_label = e.view_id.as_deref().unwrap_or("default view");
                // Resolve any !view(...) embeds in the query/narrative field
                let query_content = e.query.as_deref().unwrap_or("");
                let (cleaned_query, child_blocks) = EmbedResolver::resolve(query_content);
                let mut body = json!({
                    "object_id": e.object_id,
                    "view_id": view_id_label,
                    "query": cleaned_query,
                    "ts": e.ts,
                });
                if !child_blocks.is_empty() {
                    body["children"] = json!(child_blocks);
                }
                ViewBlock {
                    id: format!("{}:{}", session.id, i),
                    title: e.object_id.clone(),
                    body,
                }
            })
            .collect()
    };

    ContextualView {
        object_id: session.id.clone(),
        view_id: "composed-narrative".into(),
        title: "Composed Narrative".into(),
        view_kind: ViewKind::ComposedNarrative,
        renderer_kind: RendererKind::Composite,
        blocks,
        relations: vec![],
        evidence: vec![],
        findings: vec![],
    }
}

/// Pure shaper — no I/O, no async. Shapes an Investigation into a
/// ContextualView with narrative + evidence + artifacts.
pub fn build_investigation_narrative(investigation: &Investigation) -> ContextualView {
    let mut blocks = vec![ViewBlock {
        id: "header".into(),
        title: "Investigation Header".into(),
        body: json!({
            "id": investigation.id,
            "title": investigation.title,
            "goal": investigation.goal,
            "status": investigation.status.to_string(),
            "entry_point": investigation.entry_point,
            "created_at": investigation.created_at.to_string(),
            "updated_at": investigation.updated_at.to_string(),
        }),
    }];

    // Narrative block
    if !investigation.narrative.is_empty() {
        let (cleaned_narrative, child_blocks) = EmbedResolver::resolve(&investigation.narrative);
        let mut narrative_block = ViewBlock {
            id: "narrative".into(),
            title: "Narrative".into(),
            body: json!({
                "content": cleaned_narrative,
                "markdown": true,
            }),
        };
        // Attach resolved child blocks as nested embeds
        if !child_blocks.is_empty() {
            narrative_block.body["children"] = json!(child_blocks);
        }
        blocks.push(narrative_block);
    }

    // Evidence blocks
    if !investigation.evidence.is_empty() {
        let evidence_blocks: Vec<ViewBlock> = investigation
            .evidence
            .iter()
            .enumerate()
            .map(|(i, e)| ViewBlock {
                id: format!("evidence:{}", i),
                title: format!("Evidence: {}", e.object_id),
                body: json!({
                    "object_id": e.object_id,
                    "view_id": e.view_id,
                    "note": e.note,
                    "pinned_at": e.pinned_at.to_string(),
                }),
            })
            .collect();
        blocks.extend(evidence_blocks);
    }

    // Artifacts blocks
    if !investigation.artifacts.is_empty() {
        let artifact_blocks: Vec<ViewBlock> = investigation
            .artifacts
            .iter()
            .enumerate()
            .map(|(i, a)| ViewBlock {
                id: format!("artifact:{}", i),
                title: format!("Artifact: {}", a.title),
                body: json!({
                    "kind": a.kind,
                    "content": a.content,
                    "generated_from": a.generated_from,
                }),
            })
            .collect();
        blocks.extend(artifact_blocks);
    }

    ContextualView {
        object_id: investigation.id.clone(),
        view_id: "investigation-narrative".into(),
        title: format!("Investigation Narrative: {}", investigation.title),
        view_kind: ViewKind::ComposedNarrative,
        renderer_kind: RendererKind::Composite,
        blocks,
        relations: vec![],
        evidence: vec![],
        findings: vec![],
    }
}

/// Inventory provider for ComposedNarrative — used by list_for().
pub struct ComposedNarrativeProvider;
impl ViewDescriptorProvider for ComposedNarrativeProvider {
    fn id(&self) -> &'static str {
        "composed-narrative"
    }
    fn title(&self) -> &'static str {
        "Composed Narrative"
    }
    fn applies_to(&self) -> &'static [InspectableObjectType] {
        &[InspectableObjectType::SavedExploration]
    }
    fn view_kind(&self) -> ViewKind {
        ViewKind::ComposedNarrative
    }
    fn renderer_kind(&self) -> RendererKind {
        RendererKind::Composite
    }
}
pub static COMPOSED_NARRATIVE_PROVIDER: ComposedNarrativeProvider = ComposedNarrativeProvider;
inventory::submit!(ProviderWrapper {
    provider: &COMPOSED_NARRATIVE_PROVIDER as &dyn ViewDescriptorProvider
});

/// ViewExecutor for ComposedNarrative — receives the full ExplorationSession
/// or Investigation and delegates to the appropriate pure shaper.
pub struct ComposedNarrativeExecutor;
impl ViewDescriptor for ComposedNarrativeExecutor {
    fn id(&self) -> &'static str {
        "composed-narrative"
    }
    fn title(&self) -> &'static str {
        "Composed Narrative"
    }
    fn applies_to(&self) -> &'static [InspectableObjectType] {
        &[
            InspectableObjectType::SavedExploration,
            InspectableObjectType::Investigation,
        ]
    }
    fn view_kind(&self) -> ViewKind {
        ViewKind::ComposedNarrative
    }
    fn renderer_kind(&self) -> RendererKind {
        RendererKind::Composite
    }
}
#[async_trait]
impl ViewExecutor for ComposedNarrativeExecutor {
    async fn build(&self, ctx: &ViewContext<'_>) -> ExplorerResult<ContextualView> {
        match ctx.target {
            InspectionTarget::SavedExploration(session) => Ok(build_composed_narrative(session)),
            InspectionTarget::Investigation(investigation) => {
                Ok(build_investigation_narrative(investigation))
            }
            _ => Err(crate::error::ExplorerError::ViewNotAvailable {
                object_id: format!("{:?}", ctx.target),
                view_id: "composed-narrative".into(),
            }),
        }
    }
}

// ============================================================================
// EvidencePack — render collected evidence as structured document
// ============================================================================

/// Pure shaper — no I/O, no async. Shapes an Investigation's evidence into a
/// ContextualView with one ViewBlock per evidence item.
pub fn build_evidence_pack(investigation: &Investigation) -> ContextualView {
    let blocks = if investigation.evidence.is_empty() {
        vec![ViewBlock {
            id: "empty".into(),
            title: "No evidence".into(),
            body: json!({ "message": "No evidence pinned to this investigation" }),
        }]
    } else {
        investigation
            .evidence
            .iter()
            .enumerate()
            .map(|(i, e)| {
                let view_id_label = e.view_id.as_deref().unwrap_or("default view");
                ViewBlock {
                    id: format!("{}:{}", investigation.id, i),
                    title: format!("Evidence #{}: {}", i + 1, e.object_id),
                    body: json!({
                        "object_id": e.object_id,
                        "view_id": view_id_label,
                        "note": e.note,
                        "pinned_at": e.pinned_at.to_string(),
                    }),
                }
            })
            .collect()
    };

    ContextualView {
        object_id: investigation.id.clone(),
        view_id: "evidence-pack".into(),
        title: format!("Evidence Pack: {}", investigation.title),
        view_kind: ViewKind::EvidencePack,
        renderer_kind: RendererKind::Composite,
        blocks,
        relations: vec![],
        evidence: vec![],
        findings: vec![],
    }
}

/// Inventory provider for EvidencePack — used by list_for().
pub struct EvidencePackProvider;
impl ViewDescriptorProvider for EvidencePackProvider {
    fn id(&self) -> &'static str {
        "evidence-pack"
    }
    fn title(&self) -> &'static str {
        "Evidence Pack"
    }
    fn applies_to(&self) -> &'static [InspectableObjectType] {
        &[InspectableObjectType::Investigation]
    }
    fn view_kind(&self) -> ViewKind {
        ViewKind::EvidencePack
    }
    fn renderer_kind(&self) -> RendererKind {
        RendererKind::Composite
    }
}
pub static EVIDENCE_PACK_PROVIDER: EvidencePackProvider = EvidencePackProvider;
inventory::submit!(ProviderWrapper {
    provider: &EVIDENCE_PACK_PROVIDER as &dyn ViewDescriptorProvider
});

/// ViewExecutor for EvidencePack — receives the full Investigation
/// via InspectionTarget::Investigation and delegates to the pure shaper.
pub struct EvidencePackExecutor;
impl ViewDescriptor for EvidencePackExecutor {
    fn id(&self) -> &'static str {
        "evidence-pack"
    }
    fn title(&self) -> &'static str {
        "Evidence Pack"
    }
    fn applies_to(&self) -> &'static [InspectableObjectType] {
        &[InspectableObjectType::Investigation]
    }
    fn view_kind(&self) -> ViewKind {
        ViewKind::EvidencePack
    }
    fn renderer_kind(&self) -> RendererKind {
        RendererKind::Composite
    }
}
#[async_trait]
impl ViewExecutor for EvidencePackExecutor {
    async fn build(&self, ctx: &ViewContext<'_>) -> ExplorerResult<ContextualView> {
        match ctx.target {
            InspectionTarget::Investigation(investigation) => {
                Ok(build_evidence_pack(investigation))
            }
            _ => Err(crate::error::ExplorerError::ViewNotAvailable {
                object_id: format!("{:?}", ctx.target),
                view_id: "evidence-pack".into(),
            }),
        }
    }
}

// ============================================================================
// ProjectDiary — workspace exploration history as navigable narrative
// ============================================================================

/// Pure shaper for ProjectDiary — no I/O, no async.
/// Transforms workspace sessions into a ContextualView with one ViewBlock per session.
pub fn build_project_diary(target: &crate::dto::WorkspaceTarget) -> ContextualView {
    use crate::dto::ViewBlock;

    let blocks: Vec<ViewBlock> = if target.sessions.is_empty() {
        vec![ViewBlock {
            id: "empty".into(),
            title: "No sessions".into(),
            body: serde_json::json!({
                "block_type": "placeholder",
                "message": "No exploration sessions"
            }),
        }]
    } else {
        target
            .sessions
            .iter()
            .enumerate()
            .map(|(i, session)| {
                // Collect all event query content and resolve any !view(...) embeds
                let all_queries: String = session
                    .events
                    .iter()
                    .filter_map(|e| e.query.as_ref().map(|s| s.as_str()))
                    .collect::<Vec<&str>>()
                    .join("\n");
                let (_, child_blocks) = EmbedResolver::resolve(&all_queries);

                let mut body = serde_json::json!({
                    "block_type": "session",
                    "session_id": session.id,
                    "event_count": session.events.len(),
                    "created_at": session.created_at,
                    "investigation_id": session.investigation_id,
                });
                if !child_blocks.is_empty() {
                    body["children"] = json!(child_blocks);
                }
                ViewBlock {
                    id: format!("session:{}", i),
                    title: session.id.clone(),
                    body,
                }
            })
            .collect()
    };

    ContextualView {
        object_id: target.id.clone(),
        view_id: "project-diary".into(),
        title: "Project Diary".into(),
        view_kind: ViewKind::ProjectDiary,
        renderer_kind: RendererKind::Composite,
        blocks,
        relations: vec![],
        evidence: vec![],
        findings: vec![],
    }
}

/// ViewExecutor for ProjectDiary — receives WorkspaceTarget and delegates to the shaper.
pub struct ProjectDiaryExecutor;

impl ViewDescriptor for ProjectDiaryExecutor {
    fn id(&self) -> &'static str {
        "project-diary"
    }
    fn title(&self) -> &'static str {
        "Project Diary"
    }
    fn applies_to(&self) -> &'static [InspectableObjectType] {
        &[InspectableObjectType::Workspace]
    }
    fn view_kind(&self) -> ViewKind {
        ViewKind::ProjectDiary
    }
    fn renderer_kind(&self) -> RendererKind {
        RendererKind::Composite
    }
}

#[async_trait]
impl ViewExecutor for ProjectDiaryExecutor {
    async fn build(&self, ctx: &ViewContext<'_>) -> ExplorerResult<ContextualView> {
        match ctx.target {
            InspectionTarget::Workspace(target) => Ok(build_project_diary(target)),
            _ => Err(crate::error::ExplorerError::ViewNotAvailable {
                object_id: format!("{:?}", ctx.target),
                view_id: "project-diary".into(),
            }),
        }
    }
}

// ============================================================================
// ExampleObject — code usage examples as navigable narrative
// ============================================================================

/// Pure shaper for ExampleObject — no I/O, no async.
/// Transforms resolved symbol and example blocks into a ContextualView.
pub fn build_example_object(symbol: &ResolvedSymbol, examples: &[ExampleBlock]) -> ContextualView {
    use crate::dto::ViewBlock;

    let blocks: Vec<ViewBlock> = if examples.is_empty() {
        vec![ViewBlock {
            id: "empty".into(),
            title: "No examples".into(),
            body: serde_json::json!({
                "block_type": "placeholder",
                "message": "No usage examples found"
            }),
        }]
    } else {
        examples
            .iter()
            .map(|example| {
                // Resolve any !view(...) embeds in the example text
                let (cleaned_text, child_blocks) = EmbedResolver::resolve(&example.example_text);
                let mut body = serde_json::json!({
                    "block_type": example.kind,
                    "symbol_id": example.symbol_id,
                    "example_text": cleaned_text,
                    "source_location": example.source_location,
                });
                if !child_blocks.is_empty() {
                    body["children"] = json!(child_blocks);
                }
                ViewBlock {
                    id: example.symbol_id.clone(),
                    title: example.source_location.clone(),
                    body,
                }
            })
            .collect()
    };

    ContextualView {
        object_id: symbol.id.to_string(),
        view_id: "example-object".into(),
        title: format!("Examples: {}", symbol.name),
        view_kind: ViewKind::ExampleObject,
        renderer_kind: RendererKind::Composite,
        blocks,
        relations: vec![],
        evidence: vec![],
        findings: vec![],
    }
}

/// ViewExecutor for ExampleObject — resolves examples from graph repo then delegates to shaper.
pub struct ExampleObjectExecutor;

impl ViewDescriptor for ExampleObjectExecutor {
    fn id(&self) -> &'static str {
        "example-object"
    }
    fn title(&self) -> &'static str {
        "Example Object"
    }
    fn applies_to(&self) -> &'static [InspectableObjectType] {
        &[InspectableObjectType::Symbol]
    }
    fn view_kind(&self) -> ViewKind {
        ViewKind::ExampleObject
    }
    fn renderer_kind(&self) -> RendererKind {
        RendererKind::Composite
    }
}

#[async_trait]
impl ViewExecutor for ExampleObjectExecutor {
    async fn build(&self, ctx: &ViewContext<'_>) -> ExplorerResult<ContextualView> {
        match ctx.target {
            InspectionTarget::Symbol(symbol) => {
                // Fetch example blocks from the graph repository.
                // The graph_repo may be None when the graph layer is not wired;
                // in that case we fall back to an empty list (placeholder block
                // signals "no examples" to the caller).
                let examples: Vec<ExampleBlock> = match ctx.graph_repo {
                    Some(repo) => repo
                        .example_blocks_for_symbol(&symbol.id)
                        .await
                        .map_err(ExplorerError::from)?,
                    None => Vec::new(),
                };
                Ok(build_example_object(symbol, &examples))
            }
            _ => Err(crate::error::ExplorerError::ViewNotAvailable {
                object_id: format!("{:?}", ctx.target),
                view_id: "example-object".into(),
            }),
        }
    }
}

// ============================================================================
// Static executor instances — referenced by registry.rs
// ============================================================================

/// Composed narrative executor static instance.
pub static COMPOSED_NARRATIVE_EXECUTOR: ComposedNarrativeExecutor = ComposedNarrativeExecutor;

/// Evidence pack executor static instance.
pub static EVIDENCE_PACK_EXECUTOR: EvidencePackExecutor = EvidencePackExecutor;

/// Project diary executor static instance.
pub static PROJECT_DIARY_EXECUTOR: ProjectDiaryExecutor = ProjectDiaryExecutor;

/// Example object executor static instance.
pub static EXAMPLE_OBJECT_EXECUTOR: ExampleObjectExecutor = ExampleObjectExecutor;

// ============================================================================
// Test support — shared mocks for narrative executor tests
// ============================================================================

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use crate::error::ExplorerResult;
    use crate::ports::source_reader::SourceReader;
    use crate::ports::symbol_repository::{ResolvedSymbol, SymbolRepository};
    use async_trait::async_trait;
    use cognicode_core::domain::aggregates::generic_graph::{GraphEdge, GraphNode, NodeId};
    use cognicode_core::domain::aggregates::{CallEntry, SymbolId};
    use cognicode_core::domain::ports::GraphResult;
    use cognicode_core::domain::ports::graph_repository::GraphRepository;
    use cognicode_core::domain::traits::graph_query_port::{
        CalleeWithMetadata, CallerWithMetadata, GraphQueryPort, RelationTargetWithMetadata,
    };
    use cognicode_core::domain::value_objects::SymbolKind;
    use cognicode_core::domain::value_objects::edge_kind::EdgeKind;
    use cognicode_core::domain::value_objects::node_kind::NodeKind;
    use std::collections::HashMap;
    use std::sync::Mutex;

    /// Hand-rolled mock repository — no mockall to keep the crate's
    /// dev-dependencies slim. Returns pre-baked answers keyed by SymbolId.
    #[allow(unused)]
    pub(crate) struct MockRepo {
        symbols: HashMap<String, ResolvedSymbol>,
    }

    impl MockRepo {
        pub(crate) fn new() -> Self {
            Self {
                symbols: HashMap::new(),
            }
        }

        pub(crate) fn with(&mut self, sym: ResolvedSymbol) -> &mut Self {
            self.symbols.insert(sym.id.to_string(), sym);
            self
        }
    }

    impl SymbolRepository for MockRepo {
        fn resolve(&self, id: &SymbolId) -> ExplorerResult<Option<ResolvedSymbol>> {
            Ok(self.symbols.get(id.as_str()).cloned())
        }
        fn find_symbols_by_name(&self, _name: &str) -> ExplorerResult<Vec<ResolvedSymbol>> {
            Ok(Vec::new())
        }
        fn find_symbols_by_file(&self, file: &str) -> ExplorerResult<Vec<ResolvedSymbol>> {
            Ok(self
                .symbols
                .values()
                .filter(|s| s.file == file)
                .cloned()
                .collect())
        }
        fn module_list(&self) -> Vec<String> {
            let mut modules: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
            for s in self.symbols.values() {
                if let Some(parent) = std::path::Path::new(&s.file).parent() {
                    let p = parent.to_string_lossy().to_string();
                    if !p.is_empty() {
                        modules.insert(p);
                    }
                }
            }
            modules.into_iter().collect()
        }
        fn all_symbols(&self) -> ExplorerResult<Vec<ResolvedSymbol>> {
            Ok(self.symbols.values().cloned().collect())
        }
        fn graph_stats(&self) -> crate::ports::symbol_repository::GraphStats {
            crate::ports::symbol_repository::GraphStats::default()
        }
    }

    #[allow(unused)]
    pub(crate) struct MockReader {
        content: Mutex<HashMap<String, String>>,
    }

    impl MockReader {
        pub(crate) fn new(content: HashMap<String, String>) -> Self {
            Self {
                content: Mutex::new(content),
            }
        }
    }

    impl SourceReader for MockReader {
        fn read_source(&self, file: &str) -> ExplorerResult<String> {
            self.content
                .lock()
                .unwrap()
                .get(file)
                .cloned()
                .ok_or_else(|| crate::error::ExplorerError::SourceUnavailable {
                    file: file.to_string(),
                    object_id: file.to_string(),
                })
        }

        fn read_lines(
            &self,
            file: &str,
            start: u32,
            end: u32,
        ) -> ExplorerResult<Vec<(u32, String)>> {
            let content = self.read_source(file)?;
            Ok(content
                .lines()
                .enumerate()
                .map(|(i, l)| ((i + 1) as u32, l.to_string()))
                .filter(|(n, _)| *n >= start && *n <= end)
                .collect())
        }
    }

    /// Hand-rolled mock graph repository for rationale_subgraph tests.
    /// No mockall to keep dev-dependencies slim.
    #[allow(unused)]
    pub(crate) struct MockGraphRepo {
        nodes: HashMap<String, GraphNode>,
        edges: Vec<GraphEdge>,
    }

    impl MockGraphRepo {
        pub(crate) fn new() -> Self {
            Self {
                nodes: HashMap::new(),
                edges: Vec::new(),
            }
        }

        pub(crate) fn with_node(&mut self, node: GraphNode) -> &mut Self {
            self.nodes.insert(node.id.as_str().to_string(), node);
            self
        }

        pub(crate) fn with_edge(&mut self, edge: GraphEdge) -> &mut Self {
            self.edges.push(edge);
            self
        }
    }

    #[async_trait]
    impl GraphRepository for MockGraphRepo {
        async fn search(
            &self,
            _query: &str,
            _node_kinds: &[NodeKind],
            _limit: usize,
            _cursor: Option<&str>,
        ) -> GraphResult<cognicode_core::domain::ports::graph_repository::SearchPage> {
            Ok(
                cognicode_core::domain::ports::graph_repository::SearchPage {
                    items: Vec::new(),
                    raw_total: 0,
                    next_cursor: None,
                    raw_rank: 0.0,
                    item_ranks: Vec::new(),
                },
            )
        }

        async fn find_nodes_by_kind(&self, _kind: &NodeKind) -> GraphResult<Vec<GraphNode>> {
            Ok(Vec::new())
        }

        async fn get_node(&self, id: &NodeId) -> GraphResult<Option<GraphNode>> {
            Ok(self.nodes.get(id.as_str()).cloned())
        }

        async fn find_outgoing_edges(&self, _id: &NodeId) -> GraphResult<Vec<GraphEdge>> {
            Ok(Vec::new())
        }

        async fn edges_by_kind(
            &self,
            node: &NodeId,
            kinds: &[EdgeKind],
        ) -> GraphResult<Vec<GraphEdge>> {
            let kind_set: std::collections::HashSet<EdgeKind> = kinds.iter().cloned().collect();
            Ok(self
                .edges
                .iter()
                .filter(|e| e.source == *node && kind_set.contains(&e.kind))
                .cloned()
                .collect())
        }

        async fn rationale_subgraph(
            &self,
            focus: &NodeId,
            max_depth: u32,
            max_nodes: usize,
        ) -> GraphResult<(Vec<GraphNode>, Vec<GraphEdge>, bool)> {
            use std::collections::VecDeque;

            let focus_node = self.nodes.get(focus.as_str()).cloned();
            let Some(start_node) = focus_node else {
                return Ok((Vec::new(), Vec::new(), false));
            };

            // BFS over rationale edges (Cites, Justifies, Resolves, CorroboratedBy).
            let rationale_kinds: std::collections::HashSet<EdgeKind> = [
                EdgeKind::Cites,
                EdgeKind::Justifies,
                EdgeKind::Resolves,
                EdgeKind::CorroboratedBy,
            ]
            .into();

            let mut visited: std::collections::HashSet<String> = std::collections::HashSet::new();
            let mut queue: VecDeque<(String, u32)> = VecDeque::new();
            let mut result_nodes: Vec<GraphNode> = Vec::new();
            let mut result_edges: Vec<GraphEdge> = Vec::new();
            let mut truncated = false;

            queue.push_back((start_node.id.to_string(), 0));
            visited.insert(start_node.id.to_string());

            while let Some((node_id_str, depth)) = queue.pop_front() {
                if result_nodes.len() >= max_nodes {
                    truncated = true;
                    break;
                }
                if depth > max_depth {
                    continue;
                }

                let node_id = NodeId::new(node_id_str.clone());
                let node = self.nodes.get(&node_id_str);
                if let Some(n) = node {
                    if result_nodes.len() < max_nodes {
                        result_nodes.push(n.clone());
                    }
                }

                // Find all outgoing rationale edges from this node
                let outgoing: Vec<GraphEdge> = self
                    .edges
                    .iter()
                    .filter(|e| e.source == node_id && rationale_kinds.contains(&e.kind))
                    .cloned()
                    .collect();

                for edge in outgoing {
                    if result_edges.len() >= max_nodes {
                        truncated = true;
                    }
                    if result_edges.len() < max_nodes {
                        result_edges.push(edge.clone());
                    }
                    if !visited.contains(&edge.target.to_string()) {
                        visited.insert(edge.target.to_string());
                        queue.push_back((edge.target.to_string(), depth + 1));
                    }
                }
            }

            // Deduplicate nodes
            let mut seen = std::collections::HashSet::new();
            result_nodes.retain(|n| seen.insert(n.id.to_string()));

            Ok((result_nodes, result_edges, truncated))
        }

        async fn find_nodes_by_kind_paginated(
            &self,
            _kind: &NodeKind,
            _limit: usize,
            _cursor: Option<&str>,
        ) -> GraphResult<cognicode_core::domain::ports::graph_repository::SearchPage> {
            Ok(
                cognicode_core::domain::ports::graph_repository::SearchPage {
                    items: Vec::new(),
                    raw_total: 0,
                    next_cursor: None,
                    raw_rank: 0.0,
                    item_ranks: Vec::new(),
                },
            )
        }

        async fn search_paginated(
            &self,
            _query: &str,
            _kinds: &[NodeKind],
            _limit: usize,
            _cursor: Option<&str>,
        ) -> GraphResult<cognicode_core::domain::ports::graph_repository::SearchPage> {
            Ok(
                cognicode_core::domain::ports::graph_repository::SearchPage {
                    items: Vec::new(),
                    raw_total: 0,
                    next_cursor: None,
                    raw_rank: 0.0,
                    item_ranks: Vec::new(),
                },
            )
        }
    }

    /// Helper to create a ResolvedSymbol for tests.
    pub(crate) fn make_resolved(
        file: &str,
        name: &str,
        line: u32,
        kind: SymbolKind,
    ) -> ResolvedSymbol {
        ResolvedSymbol {
            id: SymbolId::new(format!("{file}:{name}:{line}")),
            name: name.to_string(),
            kind,
            file: file.to_string(),
            line,
            signature: Some(format!("fn {name}() -> ()")),
        }
    }
}

#[cfg(test)]
mod embed_integration_tests {
    use super::test_support::make_resolved;
    use super::*;
    use crate::dto::ExampleBlock;
    use crate::dto::ExplorationSession;
    use crate::facades::investigation::Investigation;
    use cognicode_core::domain::value_objects::SymbolKind;
    use time::OffsetDateTime;

    // -------------------------------------------------------------------------
    // build_investigation_narrative — embed resolution
    // -------------------------------------------------------------------------

    #[test]
    fn test_investigation_narrative_resolves_view_marker() {
        let investigation = Investigation {
            id: "inv-1".into(),
            workspace_id: "ws-1".into(),
            title: "Test Investigation".into(),
            goal: "Test goal".into(),
            status: crate::facades::investigation::InvestigationStatus::Active,
            entry_point: Some("test.rs".into()),
            panes: vec![],
            narrative: "!view(call-graph, symbol=main)".into(),
            evidence: vec![],
            artifacts: vec![],
            related_adrs: vec![],
            created_at: OffsetDateTime::now_utc(),
            updated_at: OffsetDateTime::now_utc(),
        };

        let view = build_investigation_narrative(&investigation);

        // Find the narrative block
        let narrative_block = view.blocks.iter().find(|b| b.id == "narrative").unwrap();

        // Should have children with the resolved embed
        let children = narrative_block
            .body
            .get("children")
            .expect("narrative should have children");
        let children_arr = children.as_array().expect("children should be array");
        assert_eq!(children_arr.len(), 1);

        let child = &children_arr[0];
        assert_eq!(child["id"], "embed:call-graph:0");
        assert_eq!(child["body"]["kind"], "call-graph");
        assert_eq!(child["body"]["params"]["symbol"], "main");
    }

    // -------------------------------------------------------------------------
    // build_project_diary — embed resolution
    // -------------------------------------------------------------------------

    #[test]
    fn test_project_diary_resolves_view_in_session_events() {
        let target = crate::dto::WorkspaceTarget {
            id: "ws-1".into(),
            root_path: "/test".into(),
            graph_status: crate::dto::GraphStatus::Missing,
            sessions: vec![ExplorationSession {
                id: "session-1".into(),
                workspace_id: "ws-1".into(),
                events: vec![crate::dto::ExplorationEvent {
                    object_id: "obj-1".into(),
                    view_id: Some("call-graph".into()),
                    query: Some("!view(moldql, query=MATCH)".into()),
                    ts: "2024-01-01T00:00:00Z".into(),
                }],
                navigation_mode: "pane-stack".into(),
                panes: vec![],
                created_at: "2024-01-01T00:00:00Z".into(),
                investigation_id: None,
            }],
        };

        let view = build_project_diary(&target);

        // Find the session block
        let session_block = view.blocks.iter().find(|b| b.id == "session:0").unwrap();

        // Should have children with the resolved embed
        let children = session_block
            .body
            .get("children")
            .expect("session should have children");
        let children_arr = children.as_array().expect("children should be array");
        assert_eq!(children_arr.len(), 1);

        let child = &children_arr[0];
        assert_eq!(child["id"], "embed:moldql:0");
        assert_eq!(child["body"]["kind"], "moldql");
        assert_eq!(child["body"]["params"]["query"], "MATCH");
    }

    // -------------------------------------------------------------------------
    // build_example_object — embed resolution
    // -------------------------------------------------------------------------

    #[test]
    fn test_example_object_resolves_view_in_example_text() {
        let symbol = make_resolved("test.rs", "foo", 10, SymbolKind::Function);
        let examples = vec![ExampleBlock {
            symbol_id: "test.rs:foo:10".into(),
            kind: crate::dto::ExampleKind::Usage,
            example_text: "!view(source, file=main.rs)".into(),
            source_location: "test.rs:10".into(),
        }];

        let view = build_example_object(&symbol, &examples);

        // Find the example block
        let example_block = view
            .blocks
            .iter()
            .find(|b| b.id == "test.rs:foo:10")
            .unwrap();

        // Should have children with the resolved embed
        let children = example_block
            .body
            .get("children")
            .expect("example should have children");
        let children_arr = children.as_array().expect("children should be array");
        assert_eq!(children_arr.len(), 1);

        let child = &children_arr[0];
        assert_eq!(child["id"], "embed:source:0");
        assert_eq!(child["body"]["kind"], "source");
        assert_eq!(child["body"]["params"]["file"], "main.rs");
    }

    #[test]
    fn test_example_object_no_markers_passes_through() {
        let symbol = make_resolved("test.rs", "bar", 20, SymbolKind::Function);
        let examples = vec![ExampleBlock {
            symbol_id: "test.rs:bar:20".into(),
            kind: crate::dto::ExampleKind::Usage,
            example_text: "Plain text without markers".into(),
            source_location: "test.rs:20".into(),
        }];

        let view = build_example_object(&symbol, &examples);
        let example_block = view
            .blocks
            .iter()
            .find(|b| b.id == "test.rs:bar:20")
            .unwrap();

        // Should NOT have children when no markers present
        assert!(example_block.body.get("children").is_none());
        assert_eq!(
            example_block.body["example_text"],
            "Plain text without markers"
        );
    }
}
