//! ViewService and LensExecutor facade.

use std::collections::HashSet;
use std::sync::Arc;

use async_trait::async_trait;
use cognicode_core::domain::aggregates::SymbolId;
use cognicode_core::domain::traits::GraphQueryPort;

use crate::domain::lens::{LensContext, LensRegistry};
use crate::domain::object_identity::ObjectIdentity;
use crate::domain::views::scope_contains_file;
use crate::dto::{
    ChildrenSection, ContextualGraphResponse, GraphEdge, GraphNode, LensDescriptor, LensResult,
    ParentSection, SameLevelSection,
};
use crate::dto::{ContextualView, ViewDescriptorDto, ViewSpec};
use crate::dto::{InspectableObjectType, InspectionTarget, ViewContext};
use crate::error::{ExplorerError, ExplorerResult};
use crate::facades::{LensExecutor, ViewService};
use crate::ports::quality_repository::QualityRepository;
use crate::ports::source_reader::SourceReader;
use crate::ports::symbol_repository::{ResolvedSymbol, SymbolRepository};
use crate::registry::ViewRegistry;

pub struct ViewServiceImpl {
    repo: Arc<dyn SymbolRepository>,
    reader: Arc<dyn SourceReader>,
    quality: Option<Arc<dyn QualityRepository>>,
    lens_registry: LensRegistry,
    graph_query: Option<Arc<dyn GraphQueryPort>>,
    view_registry: Arc<ViewRegistry>,
}

impl ViewServiceImpl {
    pub fn new(
        repo: Arc<dyn SymbolRepository>,
        reader: Arc<dyn SourceReader>,
        quality: Option<Arc<dyn QualityRepository>>,
        lens_registry: LensRegistry,
        graph_query: Option<Arc<dyn GraphQueryPort>>,
        view_registry: Arc<ViewRegistry>,
    ) -> Self {
        Self {
            repo,
            reader,
            quality,
            lens_registry,
            graph_query,
            view_registry,
        }
    }

    fn available_views_sync(&self, object_id: &str) -> ExplorerResult<Vec<ViewDescriptorDto>> {
        let identity = ObjectIdentity::parse_mvp_id(object_id)?;
        let object_type = match &identity {
            ObjectIdentity::Symbol { .. } => InspectableObjectType::Symbol,
            ObjectIdentity::File { .. } => InspectableObjectType::File,
            ObjectIdentity::Scope { .. } => InspectableObjectType::Scope,
            ObjectIdentity::QualityIssue { .. } => InspectableObjectType::QualityIssue,
            ObjectIdentity::Rule { .. } => InspectableObjectType::Rule,
        };
        Ok(self.view_registry.list_for(object_type))
    }

    /// Resolve an `ObjectIdentity` to an `InspectionTarget`.
    ///
    /// For Symbol: resolves via `repo.resolve()`.
    /// For File: collects all symbols in the file via `repo.find_symbols_by_file()`.
    /// For Scope: collects all symbols in scope via `repo.all_symbols()` + `scope_contains_file` filter,
    ///            then derives the file list from those symbols.
    /// For Issue/Rule: quality repo must be wired (Phase 1 requires quality repo).
    fn resolve_inspection_target(
        &self,
        identity: &ObjectIdentity,
    ) -> ExplorerResult<InspectionTarget> {
        match identity {
            ObjectIdentity::Symbol { file, name, line } => {
                let symbol_id = cognicode_core::domain::aggregates::SymbolId::new(format!(
                    "{file}:{name}:{line}"
                ));
                let resolved = self.repo.resolve(&symbol_id)?.ok_or_else(|| {
                    ExplorerError::SymbolNotFound(format!("{file}:{name}:{line}"))
                })?;
                Ok(InspectionTarget::Symbol(resolved))
            }
            ObjectIdentity::File { path } => {
                let symbols = self.repo.find_symbols_by_file(path)?;
                Ok(InspectionTarget::File {
                    path: path.clone(),
                    symbols,
                })
            }
            ObjectIdentity::Scope { path } => {
                let all = self.repo.all_symbols().unwrap_or_default();
                let mut member_files: std::collections::BTreeSet<String> =
                    std::collections::BTreeSet::new();
                let mut member_symbols: Vec<ResolvedSymbol> = Vec::new();
                for sym in all {
                    if scope_contains_file(path, &sym.file) {
                        member_files.insert(sym.file.clone());
                        member_symbols.push(sym);
                    }
                }
                Ok(InspectionTarget::Scope {
                    path: path.clone(),
                    files: member_files.into_iter().collect(),
                    symbols: member_symbols,
                })
            }
            ObjectIdentity::QualityIssue { id } => {
                let quality = self.quality.as_ref().ok_or_else(|| {
                    ExplorerError::FeatureDisabled("quality repository not wired".into())
                })?;
                let issues = quality.issue_by_id(*id)?;
                let issue = issues.ok_or_else(|| {
                    ExplorerError::ResolutionFailed(format!("issue {} not found", id))
                })?;
                Ok(InspectionTarget::Issue(issue))
            }
            ObjectIdentity::Rule { rule_id } => Ok(InspectionTarget::Rule {
                rule_id: rule_id.clone(),
            }),
        }
    }

    fn available_lenses_sync(&self, object_id: &str) -> ExplorerResult<Vec<LensDescriptor>> {
        let identity = ObjectIdentity::parse_mvp_id(object_id)?;
        let object_type = match &identity {
            ObjectIdentity::Symbol { .. } => crate::dto::InspectableObjectType::Symbol,
            ObjectIdentity::File { .. } => crate::dto::InspectableObjectType::File,
            ObjectIdentity::Scope { .. } => crate::dto::InspectableObjectType::Scope,
            ObjectIdentity::QualityIssue { .. } => crate::dto::InspectableObjectType::QualityIssue,
            ObjectIdentity::Rule { .. } => crate::dto::InspectableObjectType::Rule,
        };
        Ok(self.lens_registry.applicable_to(&object_type))
    }

    fn apply_lens_sync(&self, object_id: &str, lens_id: &str) -> ExplorerResult<LensResult> {
        let identity = ObjectIdentity::parse_mvp_id(object_id)?;
        let lens = self.lens_registry.get(lens_id).ok_or_else(|| {
            ExplorerError::ResolutionFailed(format!("lens not found: {}", lens_id))
        })?;
        let ctx = LensContext::new(
            identity,
            self.repo.clone(),
            self.quality.clone(),
            self.reader.clone(),
            self.graph_query.clone(),
        );
        lens.apply(&ctx)
    }

    fn build_contextual_graph_sync(
        &self,
        focus_id: &str,
        level: &str,
        depth: u8,
        max_nodes: usize,
    ) -> ExplorerResult<ContextualGraphResponse> {
        // 1) Validate the request.
        if level != "file" {
            return Err(ExplorerError::InvalidQuery(format!(
                "level must be 'file' in Phase 1 (got: {level})"
            )));
        }
        if !(1..=2).contains(&depth) {
            return Err(ExplorerError::InvalidQuery(format!(
                "depth must be in 1..=2 (got: {depth})"
            )));
        }
        if !(50..=500).contains(&max_nodes) {
            return Err(ExplorerError::InvalidQuery(format!(
                "max_nodes must be in 50..=500 (got: {max_nodes})"
            )));
        }

        // 2) Resolve the focus symbol.
        let focus_symbol_id = SymbolId::new(focus_id);
        let focus_resolved = self
            .repo
            .resolve(&focus_symbol_id)?
            .ok_or_else(|| ExplorerError::SymbolNotFound(focus_id.to_string()))?;
        let focus_node = symbol_to_node(&focus_resolved);

        // 3) Build the parent + children section (file-level projection).
        let file_siblings = self.repo.find_symbols_by_file(&focus_resolved.file)?;
        let (parent, children, children_clipped) = if file_siblings.is_empty() {
            (None, None, false)
        } else {
            let parent_node = GraphNode {
                id: format!("file:{}", focus_resolved.file),
                label: focus_resolved.file.clone(),
                kind: "file".to_string(),
                file: Some(focus_resolved.file.clone()),
                line: None,
                style_class: "module".to_string(),
            };
            let parent_edge = GraphEdge {
                source: focus_resolved.id.to_string(),
                target: parent_node.id.clone(),
                relation: "lives_in".to_string(),
                style_class: "edge.calls".to_string(),
            };
            let parent_section = ParentSection {
                node: parent_node,
                edge: parent_edge,
            };

            let mut child_nodes: Vec<GraphNode> = Vec::new();
            let mut child_edges: Vec<GraphEdge> = Vec::new();
            for sib in file_siblings.iter().filter(|s| s.id != focus_resolved.id) {
                child_edges.push(GraphEdge {
                    source: sib.id.to_string(),
                    target: focus_resolved.id.to_string(),
                    relation: "lives_in".to_string(),
                    style_class: "edge.calls".to_string(),
                });
                child_nodes.push(symbol_to_node(sib));
            }

            let clipped = child_nodes.len() > max_nodes;
            if clipped {
                child_nodes.truncate(max_nodes);
                let kept: HashSet<String> = child_nodes.iter().map(|n| n.id.clone()).collect();
                child_edges.retain(|e| kept.contains(&e.source));
            }
            (
                Some(parent_section),
                Some(ChildrenSection {
                    nodes: child_nodes,
                    edges: child_edges,
                }),
                clipped,
            )
        };

        // 4) Build the same-level BFS up to `depth` hops, capped at
        //    the remaining budget (children take priority).
        let remaining_cap =
            max_nodes.saturating_sub(children.as_ref().map(|c| c.nodes.len()).unwrap_or(0));
        let (same_nodes, same_edges) = if remaining_cap == 0 {
            (Vec::new(), Vec::new())
        } else {
            bfs_same_level(
                self.repo.as_ref(),
                self.graph_query.as_ref().map(|gq| gq.as_ref()),
                &focus_symbol_id,
                depth,
                remaining_cap,
            )
        };

        // 5) Truncation flag.
        let fan_in = self
            .graph_query
            .as_ref()
            .map(|gq| gq.fan_in(&focus_symbol_id))
            .unwrap_or(0);
        let fan_out = self
            .graph_query
            .as_ref()
            .map(|gq| gq.fan_out(&focus_symbol_id))
            .unwrap_or(0);
        let bfs_clipped = !same_nodes.is_empty()
            && same_nodes.len() >= remaining_cap
            && (fan_in + fan_out) > remaining_cap as usize;
        let truncated = children_clipped || bfs_clipped;
        let truncated_reason = if truncated {
            Some("max_nodes_exceeded".to_string())
        } else {
            None
        };

        Ok(ContextualGraphResponse {
            focus_node,
            parent,
            children,
            same_level: SameLevelSection {
                nodes: same_nodes,
                edges: same_edges,
            },
            level: "file".to_string(),
            truncated,
            truncated_reason,
        })
    }
}

#[async_trait]
impl ViewService for ViewServiceImpl {
    async fn available_views(&self, object_id: &str) -> ExplorerResult<Vec<ViewDescriptorDto>> {
        let object_id = object_id.to_string();
        let result = self.available_views_sync(&object_id);
        tokio::task::spawn_blocking(move || result)
            .await
            .map_err(|e| ExplorerError::Anyhow(anyhow::anyhow!("join error: {}", e)))?
    }

    async fn contextual_view(
        &self,
        object_id: &str,
        view_id: &str,
    ) -> ExplorerResult<ContextualView> {
        let identity = ObjectIdentity::parse_mvp_id(object_id)?;
        let target = self.resolve_inspection_target(&identity)?;

        let executor = self.view_registry.get_executor(view_id).ok_or_else(|| {
            ExplorerError::ResolutionFailed(format!("view not found: {}", view_id))
        })?;

        let ctx = ViewContext {
            target: &target,
            repo: self.repo.as_ref(),
            reader: self.reader.as_ref(),
            quality: self.quality.as_ref().map(|q| q.as_ref()),
            graph_query: self.graph_query.as_ref().map(|g| g.as_ref()),
        };

        // AD-2: stamp descriptor metadata onto DTO at single seam
        let mut view = executor.build(&ctx).await?;
        view.view_kind = executor.view_kind();
        view.renderer_kind = executor.renderer_kind();
        Ok(view)
    }

    async fn build_contextual_graph(
        &self,
        focus_id: &str,
        level: &str,
        depth: u8,
        max_nodes: usize,
    ) -> ExplorerResult<ContextualGraphResponse> {
        let focus_id = focus_id.to_string();
        let level = level.to_string();
        let result = self.build_contextual_graph_sync(&focus_id, &level, depth, max_nodes);
        tokio::task::spawn_blocking(move || result)
            .await
            .map_err(|e| ExplorerError::Anyhow(anyhow::anyhow!("join error: {}", e)))?
    }

    async fn available_lenses(&self, object_id: &str) -> ExplorerResult<Vec<LensDescriptor>> {
        let object_id = object_id.to_string();
        let result = self.available_lenses_sync(&object_id);
        tokio::task::spawn_blocking(move || result)
            .await
            .map_err(|e| ExplorerError::Anyhow(anyhow::anyhow!("join error: {}", e)))?
    }

    async fn apply_lens(&self, object_id: &str, lens_id: &str) -> ExplorerResult<LensResult> {
        let object_id = object_id.to_string();
        let lens_id = lens_id.to_string();
        let result = self.apply_lens_sync(&object_id, &lens_id);
        tokio::task::spawn_blocking(move || result)
            .await
            .map_err(|e| ExplorerError::Anyhow(anyhow::anyhow!("join error: {}", e)))?
    }

    async fn execute_view_spec(
        &self,
        _spec: &ViewSpec,
        _object_id: &str,
    ) -> ExplorerResult<ContextualView> {
        Err(ExplorerError::FeatureDisabled(
            "execute_view_spec not implemented".into(),
        ))
    }
}

// ============================================================================
// Helper functions (mirrored from service.rs)
// ============================================================================

fn symbol_to_node(s: &ResolvedSymbol) -> GraphNode {
    let kind_label = format!("{:?}", s.kind).to_lowercase();
    GraphNode {
        id: s.id.to_string(),
        label: s.name.clone(),
        kind: kind_label.clone(),
        file: Some(s.file.clone()),
        line: Some(s.line),
        style_class: style_class_for_kind(&kind_label).to_string(),
    }
}

fn style_class_for_kind(kind: &str) -> &'static str {
    match kind {
        "function" | "method" | "fn" => "function",
        "module" | "crate" | "trait" => "module",
        "external" => "external",
        "file" => "module",
        _ => "function",
    }
}

/// BFS of same-level neighbours (callers + callees) of `start` up to
/// `depth` hops, capped at `cap` collected nodes.
fn bfs_same_level(
    repo: &dyn SymbolRepository,
    graph_query: Option<&dyn GraphQueryPort>,
    start: &SymbolId,
    depth: u8,
    cap: usize,
) -> (Vec<GraphNode>, Vec<GraphEdge>) {
    let mut visited: HashSet<String> = HashSet::new();
    visited.insert(start.to_string());
    let mut frontier: Vec<SymbolId> = vec![start.clone()];
    let mut nodes: Vec<GraphNode> = Vec::new();
    let mut edges: Vec<GraphEdge> = Vec::new();

    for _ in 0..depth {
        if nodes.len() >= cap {
            break;
        }
        let mut next: Vec<SymbolId> = Vec::new();
        for n in &frontier {
            let callers = graph_query.map(|gq| gq.callers(n)).unwrap_or_default();
            let callees = graph_query.map(|gq| gq.callees(n)).unwrap_or_default();
            for rel in callers.into_iter().chain(callees) {
                let nid = rel.id.to_string();
                if !visited.insert(nid.clone()) {
                    continue;
                }
                if nodes.len() >= cap {
                    break;
                }
                edges.push(GraphEdge {
                    source: n.to_string(),
                    target: nid.clone(),
                    relation: "calls".to_string(),
                    style_class: "edge.calls".to_string(),
                });
                if let Ok(Some(resolved)) = repo.resolve(&rel.id) {
                    nodes.push(symbol_to_node(&resolved));
                }
                next.push(rel.id);
            }
            if nodes.len() >= cap {
                break;
            }
        }
        if next.is_empty() {
            break;
        }
        frontier = next;
    }
    (nodes, edges)
}

#[async_trait]
impl LensExecutor for ViewServiceImpl {
    async fn apply_lens(&self, object_id: &str, lens_id: &str) -> ExplorerResult<LensResult> {
        ViewService::apply_lens(self, object_id, lens_id).await
    }
}

// ============================================================================
// Unit tests for ViewServiceImpl
// ============================================================================

#[cfg(test)]
mod view_service_tests {
    use super::*;

    /// Hand-rolled mock repo for ViewServiceImpl tests.
    struct MockRepo {
        symbols: std::collections::HashMap<String, ResolvedSymbol>,
    }

    impl MockRepo {
        fn new() -> Self {
            Self {
                symbols: std::collections::HashMap::new(),
            }
        }
        fn with_symbol(mut self, sym: ResolvedSymbol) -> Self {
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
            Vec::new()
        }
        fn all_symbols(&self) -> ExplorerResult<Vec<ResolvedSymbol>> {
            Ok(self.symbols.values().cloned().collect())
        }
        fn graph_stats(&self) -> crate::ports::symbol_repository::GraphStats {
            crate::ports::symbol_repository::GraphStats::default()
        }
    }

    fn make_resolved(file: &str, name: &str, line: u32) -> ResolvedSymbol {
        let kind = if name.starts_with("fn_") {
            cognicode_core::domain::value_objects::SymbolKind::Function
        } else {
            cognicode_core::domain::value_objects::SymbolKind::Struct
        };
        ResolvedSymbol {
            id: SymbolId::new(format!("{file}:{name}:{line}")),
            name: name.to_string(),
            kind,
            file: file.to_string(),
            line,
            signature: Some(format!("fn {name}() -> ()")),
        }
    }

    fn make_service(repo: MockRepo) -> ViewServiceImpl {
        let repo = Arc::new(repo) as Arc<dyn SymbolRepository>;
        let reader =
            Arc::new(MockReader::new(std::collections::HashMap::new())) as Arc<dyn SourceReader>;
        let view_registry = Arc::new(ViewRegistry::new(None));
        ViewServiceImpl::new(
            repo,
            reader,
            None,
            crate::domain::lens::default_registry(),
            None,
            view_registry,
        )
    }

    struct MockReader {
        content: std::sync::Mutex<std::collections::HashMap<String, String>>,
    }

    impl MockReader {
        fn new(content: std::collections::HashMap<String, String>) -> Self {
            Self {
                content: std::sync::Mutex::new(content),
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

    // --- available_views_sync delegates to view_registry.list_for ---

    #[test]
    fn available_views_sync_delegates_to_registry_for_symbol() {
        let service = make_service(MockRepo::new());
        let views = service
            .available_views_sync("symbol:src/main.rs:main:1")
            .unwrap();
        // Should return views from registry for Symbol type
        assert!(!views.is_empty(), "expected non-empty views for symbol");
        let ids: Vec<&str> = views.iter().map(|v| v.id.as_str()).collect();
        assert!(ids.contains(&"overview"), "overview should be in views");
        assert!(ids.contains(&"call-graph"), "call-graph should be in views");
    }

    #[test]
    fn available_views_sync_delegates_to_registry_for_file() {
        let service = make_service(MockRepo::new());
        let views = service.available_views_sync("file:src/main.rs").unwrap();
        assert!(!views.is_empty());
        let ids: Vec<&str> = views.iter().map(|v| v.id.as_str()).collect();
        assert!(
            ids.contains(&"overview"),
            "overview should be in views for file"
        );
    }

    #[test]
    fn available_views_sync_delegates_to_registry_for_scope() {
        let service = make_service(MockRepo::new());
        let views = service.available_views_sync("scope:src").unwrap();
        assert!(!views.is_empty());
    }

    #[test]
    fn available_views_sync_returns_empty_for_unknown_object_type() {
        // Unknown object types will cause parse errors - tested separately
        let service = make_service(MockRepo::new());
        let result = service.available_views_sync("unknown:foo");
        assert!(result.is_err(), "unknown prefix should error");
    }

    // --- resolve_inspection_target for Symbol ---

    #[test]
    fn resolve_inspection_target_resolves_symbol() {
        let sym = make_resolved("src/main.rs", "main", 1);
        let repo = MockRepo::new().with_symbol(sym.clone());
        let service = make_service(repo);

        let identity = ObjectIdentity::parse_mvp_id("symbol:src/main.rs:main:1").unwrap();
        let target = service.resolve_inspection_target(&identity).unwrap();

        match target {
            InspectionTarget::Symbol(resolved) => {
                assert_eq!(resolved.name, "main");
                assert_eq!(resolved.file, "src/main.rs");
                assert_eq!(resolved.line, 1);
            }
            _ => panic!("expected Symbol, got {:?}", target),
        }
    }

    #[test]
    fn resolve_inspection_target_symbol_not_found() {
        let service = make_service(MockRepo::new());
        let identity = ObjectIdentity::parse_mvp_id("symbol:src/missing.rs:missing:1").unwrap();
        let result = service.resolve_inspection_target(&identity);
        assert!(result.is_err());
    }

    // --- resolve_inspection_target for File ---

    #[test]
    fn resolve_inspection_target_resolves_file() {
        let sym1 = make_resolved("src/lib.rs", "foo", 10);
        let sym2 = make_resolved("src/lib.rs", "bar", 20);
        let repo = MockRepo::new().with_symbol(sym1).with_symbol(sym2);
        let service = make_service(repo);

        let identity = ObjectIdentity::parse_mvp_id("file:src/lib.rs").unwrap();
        let target = service.resolve_inspection_target(&identity).unwrap();

        match target {
            InspectionTarget::File { path, symbols } => {
                assert_eq!(path, "src/lib.rs");
                assert_eq!(symbols.len(), 2);
            }
            _ => panic!("expected File, got {:?}", target),
        }
    }

    // --- resolve_inspection_target for Scope ---

    #[test]
    fn resolve_inspection_target_resolves_scope() {
        let sym1 = make_resolved("src/foo/a.rs", "alpha", 1);
        let sym2 = make_resolved("src/foo/b.rs", "beta", 2);
        let sym3 = make_resolved("src/bar/c.rs", "gamma", 3); // outside scope
        let repo = MockRepo::new()
            .with_symbol(sym1)
            .with_symbol(sym2)
            .with_symbol(sym3);
        let service = make_service(repo);

        let identity = ObjectIdentity::parse_mvp_id("scope:src/foo").unwrap();
        let target = service.resolve_inspection_target(&identity).unwrap();

        match target {
            InspectionTarget::Scope {
                path,
                files,
                symbols,
            } => {
                assert_eq!(path, "src/foo");
                // 2 files in scope: src/foo/a.rs and src/foo/b.rs
                assert_eq!(files.len(), 2);
                assert!(files.contains(&"src/foo/a.rs".to_string()));
                assert!(files.contains(&"src/foo/b.rs".to_string()));
                // 2 symbols in scope
                assert_eq!(symbols.len(), 2);
            }
            _ => panic!("expected Scope, got {:?}", target),
        }
    }

    #[test]
    fn resolve_inspection_target_scope_empty_when_no_symbols() {
        let service = make_service(MockRepo::new());
        let identity = ObjectIdentity::parse_mvp_id("scope:src/empty").unwrap();
        let target = service.resolve_inspection_target(&identity).unwrap();

        match target {
            InspectionTarget::Scope { files, symbols, .. } => {
                assert!(files.is_empty());
                assert!(symbols.is_empty());
            }
            _ => panic!("expected Scope, got {:?}", target),
        }
    }

    // --- resolve_inspection_target for Rule ---

    #[test]
    fn resolve_inspection_target_resolves_rule() {
        let service = make_service(MockRepo::new());
        let identity = ObjectIdentity::parse_mvp_id("rule:rust:S100").unwrap();
        let target = service.resolve_inspection_target(&identity).unwrap();

        match target {
            InspectionTarget::Rule { rule_id } => {
                assert_eq!(rule_id, "rust:S100");
            }
            _ => panic!("expected Rule, got {:?}", target),
        }
    }

    // --- resolve_inspection_target for QualityIssue (no quality repo) ---

    #[test]
    fn resolve_inspection_target_issue_requires_quality_repo() {
        let service = make_service(MockRepo::new());
        let identity = ObjectIdentity::parse_mvp_id("issue:42").unwrap();
        let result = service.resolve_inspection_target(&identity);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ExplorerError::FeatureDisabled(_)));
    }

    // --- contextual_view dispatch ---

    #[tokio::test]
    async fn contextual_view_dispatches_to_executor() {
        let sym = make_resolved("src/main.rs", "main", 1);
        let repo = MockRepo::new().with_symbol(sym.clone());
        let service = make_service(repo);

        let view = service
            .contextual_view("symbol:src/main.rs:main:1", "overview")
            .await
            .unwrap();
        assert_eq!(view.view_id, "overview");
        assert_eq!(view.title, "Overview");
    }

    #[tokio::test]
    async fn contextual_view_returns_error_for_unknown_view() {
        let service = make_service(MockRepo::new());
        let result = service
            .contextual_view("symbol:src/main.rs:main:1", "nonexistent-view")
            .await;
        assert!(result.is_err());
    }
}
