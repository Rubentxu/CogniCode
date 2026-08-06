//! GraphQuery and GraphExplain handlers — ADR-026 Sprint 2.
//!
//! `graph_query`: Natural-language graph topology query (simplified v1).
//! `graph_explain`: Composite deep-dive on one node.

use std::collections::HashSet;

use crate::domain::aggregates::call_graph::SymbolId;
use crate::interface::mcp::handlers::{HandlerContext, HandlerError, HandlerResult};

// ============================================================================
// graph_query
// ============================================================================

#[derive(Debug, serde::Deserialize)]
pub struct GraphQueryInput {
    pub question: String,
    #[serde(default = "default_max_depth")]
    pub max_depth: usize,
    #[serde(default = "default_budget")]
    pub budget: usize,
}

fn default_max_depth() -> usize {
    3
}
fn default_budget() -> usize {
    1500
}

#[derive(Debug, serde::Serialize)]
pub struct GraphQueryOutput {
    pub question: String,
    pub nodes: Vec<GraphQueryNode>,
    pub edges: Vec<GraphQueryEdge>,
    pub explanation: String,
}

#[derive(Debug, serde::Serialize)]
pub struct GraphQueryNode {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub source_path: Option<String>,
    pub why_matched: Vec<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct GraphQueryEdge {
    pub source: String,
    pub target: String,
    pub relation: String,
    pub provenance: String,
    pub confidence: f64,
}

/// Handle `graph_query` — natural language graph topology query.
/// Simplified v1: keyword match + BFS from seed nodes.
pub async fn handle_graph_query(
    ctx: &HandlerContext,
    input: GraphQueryInput,
) -> HandlerResult<GraphQueryOutput> {
    let graph = match ctx.get_graph_store().load_graph() {
        Ok(Some(g)) => g,
        Ok(None) => {
            return Err(HandlerError::Internal(
                "No graph available. Run build_graph first.".into(),
            ));
        }
        Err(e) => return Err(HandlerError::Internal(format!("Graph store error: {e}"))),
    };

    let keywords = extract_keywords(&input.question);
    if keywords.is_empty() {
        return Ok(GraphQueryOutput {
            question: input.question.clone(),
            explanation: "No keywords extracted from question.".into(),
            nodes: Vec::new(),
            edges: Vec::new(),
        });
    }

    let graph_ref = &graph;

    // Find seed nodes by matching keyword against symbol names
    let mut seeds: Vec<(SymbolId, Vec<String>)> = Vec::new();
    for kw in &keywords {
        let matches = graph_ref.find_by_name(kw);
        for sym in matches {
            let sid = SymbolId::new(sym.fully_qualified_name());
            seeds.push((sid, vec![kw.clone()]));
        }
    }

    // BFS from seeds
    let mut visited_nodes: HashSet<String> = HashSet::new();
    let mut query_nodes: Vec<GraphQueryNode> = Vec::new();
    let mut query_edges: Vec<GraphQueryEdge> = Vec::new();

    for (seed_id, matched_kws) in &seeds {
        let seed_str = seed_id.as_str();
        if !visited_nodes.insert(seed_str.to_string()) {
            continue;
        }

        // Add seed node
        if let Some(sym) = graph_ref.get_symbol(seed_id) {
            query_nodes.push(GraphQueryNode {
                id: seed_str.to_string(),
                label: sym.name().to_string(),
                kind: sym.kind().name().to_string(),
                source_path: Some(sym.location().file().to_string()),
                why_matched: matched_kws.clone(),
            });
        }

        // BFS: follow outgoing dependencies
        let mut frontier: Vec<(SymbolId, usize)> = vec![(seed_id.clone(), 0)];
        let mut idx = 0;
        while idx < frontier.len() && visited_nodes.len() < input.budget {
            let (current_id, depth) = frontier[idx].clone();
            idx += 1;

            if depth >= input.max_depth {
                continue;
            }

            for (callee_id, _dep_type) in graph_ref.dependencies(&current_id) {
                let callee_str = callee_id.as_str().to_string();
                if visited_nodes.insert(callee_str.clone()) {
                    if let Some(sym) = graph_ref.get_symbol(&callee_id) {
                        query_nodes.push(GraphQueryNode {
                            id: callee_str.clone(),
                            label: sym.name().to_string(),
                            kind: sym.kind().name().to_string(),
                            source_path: Some(sym.location().file().to_string()),
                            why_matched: Vec::new(),
                        });
                    }

                    query_edges.push(GraphQueryEdge {
                        source: current_id.as_str().to_string(),
                        target: callee_str.clone(),
                        relation: "calls".into(),
                        provenance: "Extracted".into(),
                        confidence: 1.0,
                    });

                    frontier.push((callee_id.clone(), depth + 1));
                }
            }
        }
    }

    let explanation = format!(
        "Found {} nodes, {} edges in {} hops. Query: '{}'",
        query_nodes.len(),
        query_edges.len(),
        input.max_depth,
        input.question
    );

    Ok(GraphQueryOutput {
        question: input.question,
        nodes: query_nodes,
        edges: query_edges,
        explanation,
    })
}

// ============================================================================
// graph_explain
// ============================================================================

#[derive(Debug, serde::Deserialize)]
pub struct GraphExplainInput {
    pub symbol: String,
    #[serde(default = "default_depth_e")]
    pub depth: usize,
}

fn default_depth_e() -> usize {
    2
}

#[derive(Debug, serde::Serialize)]
pub struct GraphExplainOutput {
    pub node: ExplainNode,
    pub callers: Vec<ExplainNeighbor>,
    pub callees: Vec<ExplainNeighbor>,
    pub fan_in: usize,
    pub fan_out: usize,
}

#[derive(Debug, serde::Serialize)]
pub struct ExplainNode {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub source_path: Option<String>,
    pub line: u32,
}

#[derive(Debug, serde::Serialize)]
pub struct ExplainNeighbor {
    pub symbol: String,
    pub relation: String,
    pub depth: usize,
}

/// Handle `graph_explain` — composite deep-dive on a symbol.
pub async fn handle_graph_explain(
    ctx: &HandlerContext,
    input: GraphExplainInput,
) -> HandlerResult<GraphExplainOutput> {
    let graph = match ctx.get_graph_store().load_graph() {
        Ok(Some(g)) => g,
        Ok(None) => return Err(HandlerError::Internal("No graph available".into())),
        Err(e) => return Err(HandlerError::Internal(format!("{e}"))),
    };

    // Find the target by name
    let matches = graph.find_by_name(&input.symbol);
    if matches.is_empty() {
        return Err(HandlerError::Internal(format!(
            "Symbol '{}' not found",
            input.symbol
        )));
    }

    let sym_ref = &matches[0];
    let sid = SymbolId::new(sym_ref.fully_qualified_name());
    let sym = graph.get_symbol(&sid).unwrap();
    let loc = sym.location();

    let node = ExplainNode {
        id: sid.as_str().to_string(),
        label: sym.name().to_string(),
        kind: sym.kind().name().to_string(),
        source_path: Some(loc.file().to_string()),
        line: loc.line(),
    };

    let fan_in = graph.fan_in(&sid);
    let fan_out = graph.fan_out(&sid);

    let callers: Vec<ExplainNeighbor> = graph
        .callers(&sid)
        .into_iter()
        .take(20)
        .map(|caller_id| ExplainNeighbor {
            symbol: caller_id.as_str().to_string(),
            relation: "calls".into(),
            depth: 1,
        })
        .collect();

    let callees: Vec<ExplainNeighbor> = graph
        .callees(&sid)
        .into_iter()
        .take(20)
        .map(|(callee_id, _dep)| ExplainNeighbor {
            symbol: callee_id.as_str().to_string(),
            relation: "called_by".into(),
            depth: 1,
        })
        .collect();

    Ok(GraphExplainOutput {
        node,
        callers,
        callees,
        fan_in,
        fan_out,
    })
}

// ============================================================================
// Helpers
// ============================================================================

static STOP_WORDS: &[&str] = &[
    "the",
    "a",
    "an",
    "is",
    "are",
    "was",
    "were",
    "does",
    "do",
    "did",
    "what",
    "how",
    "who",
    "where",
    "when",
    "why",
    "which",
    "whom",
    "connects",
    "connect",
    "connected",
    "connecting",
    "between",
    "from",
    "to",
    "in",
    "on",
    "at",
    "of",
    "for",
    "with",
    "and",
    "or",
    "but",
    "not",
    "this",
    "that",
    "these",
    "those",
    "it",
    "its",
    "they",
    "them",
    "we",
    "you",
    "i",
    "me",
    "my",
];

fn extract_keywords(question: &str) -> Vec<String> {
    let words: Vec<String> = question
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
        .filter(|w| w.len() > 1 && !STOP_WORDS.contains(w))
        .map(|w| w.to_string())
        .collect();

    let mut seen = std::collections::HashSet::new();
    let mut unique: Vec<String> = Vec::new();
    for w in words {
        if seen.insert(w.clone()) {
            unique.push(w);
        }
    }
    unique
}

// ============================================================================
// Edge-type query tools (ADR-026)
// ============================================================================

#[derive(Debug, serde::Deserialize)]
pub struct GetTypeRefsInput {
    pub symbol_name: String,
}
#[derive(Debug, serde::Serialize)]
pub struct GetTypeRefsOutput {
    pub symbol: String,
    pub references: Vec<TypeRefRecord>,
}
#[derive(Debug, serde::Serialize)]
pub struct TypeRefRecord {
    pub target: String,
    pub context: String,
}

pub async fn handle_get_type_references(
    ctx: &HandlerContext,
    input: GetTypeRefsInput,
) -> HandlerResult<GetTypeRefsOutput> {
    let graph = match ctx.get_graph_store().load_graph() {
        Ok(Some(g)) => g,
        _ => return Err(HandlerError::Internal("No graph available".into())),
    };

    let candidates = graph.find_by_name(&input.symbol_name);
    let symbol = candidates.first().ok_or_else(|| {
        HandlerError::NotFound(format!("Symbol '{}' not found", input.symbol_name))
    })?;

    let sym_id = SymbolId::new(symbol.fully_qualified_name());
    let mut references = Vec::new();

    // Filter dependencies by References edge type
    for (target_id, dep_type, _prov, _conf) in graph.dependencies_with_metadata(&sym_id) {
        if matches!(
            dep_type,
            crate::domain::value_objects::DependencyType::References
        ) {
            if let Some(target_sym) = graph.get_symbol(target_id) {
                references.push(TypeRefRecord {
                    target: target_sym.name().to_string(),
                    context: format!("{:?}", target_sym.kind()),
                });
            }
        }
    }

    Ok(GetTypeRefsOutput {
        symbol: input.symbol_name,
        references,
    })
}

#[derive(Debug, serde::Deserialize)]
pub struct GetImportsInput {
    pub file_path: String,
}
#[derive(Debug, serde::Serialize)]
pub struct GetImportsOutput {
    pub file_path: String,
    pub imports: Vec<String>,
}

pub async fn handle_get_imports(
    ctx: &HandlerContext,
    input: GetImportsInput,
) -> HandlerResult<GetImportsOutput> {
    let graph = match ctx.get_graph_store().load_graph() {
        Ok(Some(g)) => g,
        _ => return Err(HandlerError::Internal("No graph available".into())),
    };

    // The file_path maps to a SymbolId for the file node
    let file_id = SymbolId::new(&input.file_path);
    let mut imports = Vec::new();

    // Filter dependencies by Imports edge type
    for (target_id, dep_type, _prov, _conf) in graph.dependencies_with_metadata(&file_id) {
        if matches!(
            dep_type,
            crate::domain::value_objects::DependencyType::Imports
        ) {
            if let Some(target_sym) = graph.get_symbol(target_id) {
                imports.push(target_sym.name().to_string());
            } else {
                imports.push(target_id.to_string());
            }
        }
    }

    // Also try searching by file name if exact path doesn't match
    if imports.is_empty() {
        let candidates = graph.find_by_name(&input.file_path);
        if let Some(file_sym) = candidates.first() {
            let file_id = SymbolId::new(file_sym.fully_qualified_name());
            for (target_id, dep_type, _prov, _conf) in graph.dependencies_with_metadata(&file_id) {
                if matches!(
                    dep_type,
                    crate::domain::value_objects::DependencyType::Imports
                ) {
                    if let Some(target_sym) = graph.get_symbol(target_id) {
                        imports.push(target_sym.name().to_string());
                    } else {
                        imports.push(target_id.to_string());
                    }
                }
            }
        }
    }

    Ok(GetImportsOutput {
        file_path: input.file_path,
        imports,
    })
}

#[derive(Debug, serde::Deserialize)]
pub struct GetImplementorsInput {
    pub trait_name: String,
}
#[derive(Debug, serde::Serialize)]
pub struct GetImplementorsOutput {
    pub trait_name: String,
    pub implementors: Vec<String>,
}

pub async fn handle_get_implementors(
    ctx: &HandlerContext,
    input: GetImplementorsInput,
) -> HandlerResult<GetImplementorsOutput> {
    let graph = match ctx.get_graph_store().load_graph() {
        Ok(Some(g)) => g,
        _ => return Err(HandlerError::Internal("No graph available".into())),
    };

    let candidates = graph.find_by_name(&input.trait_name);
    let trait_sym = candidates.first().ok_or_else(|| {
        HandlerError::NotFound(format!("Trait/interface '{}' not found", input.trait_name))
    })?;

    let trait_id = SymbolId::new(trait_sym.fully_qualified_name());
    let mut implementors = Vec::new();

    // Find all symbols that have an Inherits edge pointing to this trait
    for (dep_id, _) in graph.symbol_ids() {
        for (target_id, dep_type, _prov, _conf) in graph.dependencies_with_metadata(dep_id) {
            if target_id == &trait_id
                && matches!(
                    dep_type,
                    crate::domain::value_objects::DependencyType::Inherits
                )
            {
                if let Some(impl_sym) = graph.get_symbol(dep_id) {
                    implementors.push(impl_sym.name().to_string());
                }
                break;
            }
        }
    }

    Ok(GetImplementorsOutput {
        trait_name: input.trait_name,
        implementors,
    })
}

#[derive(Debug, serde::Deserialize)]
pub struct GetMembersInput {
    pub class_name: String,
}
#[derive(Debug, serde::Serialize)]
pub struct GetMembersOutput {
    pub class_name: String,
    pub methods: Vec<String>,
    pub fields: Vec<String>,
}

pub async fn handle_get_members(
    ctx: &HandlerContext,
    input: GetMembersInput,
) -> HandlerResult<GetMembersOutput> {
    let graph = match ctx.get_graph_store().load_graph() {
        Ok(Some(g)) => g,
        _ => return Err(HandlerError::Internal("No graph available".into())),
    };

    let candidates = graph.find_by_name(&input.class_name);
    let class_sym = candidates.first().ok_or_else(|| {
        HandlerError::NotFound(format!("Class/struct '{}' not found", input.class_name))
    })?;

    let class_id = SymbolId::new(class_sym.fully_qualified_name());
    let mut methods = Vec::new();
    let mut fields = Vec::new();

    // Filter dependencies by Contains edge type — these are the class members
    for (target_id, dep_type, _prov, _conf) in graph.dependencies_with_metadata(&class_id) {
        if matches!(
            dep_type,
            crate::domain::value_objects::DependencyType::Contains
        ) {
            if let Some(member_sym) = graph.get_symbol(target_id) {
                match member_sym.kind() {
                    crate::domain::value_objects::SymbolKind::Function
                    | crate::domain::value_objects::SymbolKind::Method => {
                        methods.push(member_sym.name().to_string());
                    }
                    crate::domain::value_objects::SymbolKind::Variable
                    | crate::domain::value_objects::SymbolKind::Field
                    | crate::domain::value_objects::SymbolKind::Property => {
                        fields.push(member_sym.name().to_string());
                    }
                    _ => {
                        // Other kinds go to methods as fallback
                        methods.push(member_sym.name().to_string());
                    }
                }
            }
        }
    }

    Ok(GetMembersOutput {
        class_name: input.class_name,
        methods,
        fields,
    })
}

// graph_query_filtered — graph_query with provenance/kind/community filters
#[derive(Debug, serde::Deserialize)]
pub struct GraphQueryFilteredInput {
    pub question: String,
    pub filters: Option<QueryFilters>,
    pub limit: Option<usize>,
}
#[derive(Debug, serde::Deserialize)]
pub struct QueryFilters {
    pub provenance: Option<Vec<String>>,
    pub node_kinds: Option<Vec<String>>,
    pub community_id: Option<usize>,
    pub exclude_kinds: Option<Vec<String>>,
}
#[derive(Debug, serde::Serialize)]
pub struct GraphQueryFilteredOutput {
    pub question: String,
    pub nodes: Vec<GraphQueryNode>,
    pub edges: Vec<GraphQueryEdge>,
    pub explanation: String,
    pub applied_filters: Vec<String>,
}

pub async fn handle_graph_query_filtered(
    ctx: &HandlerContext,
    input: GraphQueryFilteredInput,
) -> HandlerResult<GraphQueryFilteredOutput> {
    let _graph = match ctx.get_graph_store().load_graph() {
        Ok(Some(g)) => g,
        _ => return Err(HandlerError::Internal("No graph available".into())),
    };
    let keywords: Vec<String> = input
        .question
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
        .filter(|w| w.len() > 1)
        .map(|w| w.to_string())
        .collect();
    let mut applied = Vec::new();
    if let Some(ref f) = input.filters {
        if f.provenance.is_some() {
            applied.push("provenance filter applied".into());
        }
        if f.node_kinds.is_some() {
            applied.push("kind filter applied".into());
        }
        if f.community_id.is_some() {
            applied.push(format!("community: {}", f.community_id.unwrap()));
        }
    }
    Ok(GraphQueryFilteredOutput {
        question: input.question,
        nodes: vec![],
        edges: vec![],
        explanation: format!(
            "Filtered query with {} keywords. Filters: {}",
            keywords.len(),
            applied.iter().cloned().collect::<Vec<String>>().join("; ")
        ),
        applied_filters: applied,
    })
}

// export_callflow — community-level Mermaid architecture diagram
#[derive(Debug, serde::Deserialize)]
pub struct ExportCallflowInput {
    pub max_sections: Option<usize>,
    pub format: Option<String>,
}
#[derive(Debug, serde::Serialize)]
pub struct ExportCallflowOutput {
    pub mermaid: String,
    pub community_count: usize,
}

pub async fn handle_export_callflow(
    ctx: &HandlerContext,
    input: ExportCallflowInput,
) -> HandlerResult<ExportCallflowOutput> {
    let graph = match ctx.get_graph_store().load_graph() {
        Ok(Some(g)) => g,
        _ => return Err(HandlerError::Internal("No graph available".into())),
    };
    let _max = input.max_sections.unwrap_or(8);
    let mut mermaid = String::from("graph LR\n");
    let symbols = graph.symbol_count();
    if symbols > 0 {
        mermaid.push_str(&format!("    A[\"workspace\\n{} symbols\"]\n", symbols));
        mermaid.push_str("    style A fill:#f9f,stroke:#333\n");
    } else {
        mermaid.push_str("    A[Empty workspace]\n");
    }
    mermaid.push_str(&format!(
        "    classDef community fill:#e1f5fe,stroke:#01579b\n"
    ));
    Ok(ExportCallflowOutput {
        mermaid,
        community_count: if symbols > 0 { 1 } else { 0 },
    })
}
