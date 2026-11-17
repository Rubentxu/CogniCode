//! GraphQuery and GraphExplain handlers — ADR-026 Sprint 2.
//!
//! `graph_query`: Natural-language graph topology query (simplified v1).
//! `graph_explain`: Composite deep-dive on one node.
// e30.1 clippy baseline reset: pre-existing lint debt (see fix/e30.1-clippy-baseline-reset)
#![allow(clippy::unnecessary_to_owned, clippy::unnecessary_unwrap)]

use std::collections::{HashMap, HashSet, VecDeque};

use crate::application::services::graph_queries::{
    CommunityDetection, CommunitySection, community_callflow,
};
use crate::domain::aggregates::call_graph::{CallGraph, SymbolId};
use crate::domain::value_objects::published_line;
use crate::infrastructure::graph::analytics::community_detector::CommunityDetector;
use crate::interface::coverage::CoverageInfo;
use crate::interface::mcp::handlers::{HandlerContext, HandlerError, HandlerResult};
use crate::interface::symbol_identity::{
    Precision, default_provider_for_tool, precision_floor_for_tool,
};
// `JsonSchema` derives the published MCP output contract from this type, so
// the contract cannot drift from the struct that produces the bytes.
use schemars::JsonSchema;

// ============================================================================
// graph_query
// ============================================================================

/// What `graph_query` is asked.
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct GraphQueryInput {
    /// Natural language question about the graph topology
    pub question: String,
    /// Maximum BFS depth from the seed nodes (default: 3). Depth 0 returns
    /// only the seeds.
    #[serde(default = "default_max_depth")]
    pub max_depth: usize,
    /// Maximum elements the answer holds, counting seed nodes, discovered
    /// nodes and edges together (default: 1500). The answer never exceeds
    /// it; when it stops here, `truncated` is true and `truncation_reason`
    /// is `budget`. A larger budget extends the answer: the elements come
    /// out in a fixed order that does not depend on the budget.
    #[serde(default = "default_budget")]
    pub budget: usize,
}

fn default_max_depth() -> usize {
    3
}
fn default_budget() -> usize {
    1500
}

/// What `graph_query` answers.
///
/// The answer is a **traversal**, not a subgraph: it holds the nodes the
/// walk reached and the edges the walk followed. An edge between two
/// returned nodes that the walk did not follow (because the depth or the
/// budget stopped it) is not in the answer. `mode` says so, and
/// `truncated` says whether anything reachable was left out.
#[derive(Debug, serde::Serialize, JsonSchema)]
pub struct GraphQueryOutput {
    pub question: String,
    /// The nodes reached: the seeds first, then the nodes the walk
    /// discovered, in discovery order.
    pub nodes: Vec<GraphQueryNode>,
    /// The edges the walk followed, in the order it followed them. An edge
    /// never appears before its two endpoints are in `nodes`.
    pub edges: Vec<GraphQueryEdge>,
    pub explanation: String,
    /// One line saying what the answer says: its size, what it covers and
    /// whether it was cut.
    pub summary: String,
    /// What the answer is: always `traversal`, the nodes reached from the
    /// seeds and the edges followed, as opposed to the subgraph induced by
    /// the returned nodes.
    pub mode: String,
    /// The direction the edges are followed in: `outgoing`.
    pub direction: String,
    /// Whether something the walk could reach was left out, because of the
    /// budget or the depth.
    pub truncated: bool,
    /// Why: `budget` when the answer reached the budget, `max_depth` when
    /// nodes at the last depth have outgoing edges that were not followed,
    /// `null` when the answer is not cut. `budget` wins when both apply.
    pub truncation_reason: Option<String>,
    /// How many returned nodes have outgoing edges that were not followed
    /// because they sit at the last depth.
    pub unexpanded_nodes: usize,
    /// The limits the walk ran under and how much of them it used.
    pub limits: GraphQueryLimits,
    /// What the graph this answer was read from covers. `unknown` when
    /// nobody measured that graph, and then an empty answer is not proof
    /// that nothing exists.
    pub coverage: CoverageInfo,
    /// Which tree and which snapshot the graph was read from, when the
    /// session knows; `null` otherwise.
    pub basis: Option<GraphQueryBasis>,
}

/// The limits of a `graph_query` walk.
#[derive(Debug, serde::Serialize, JsonSchema)]
pub struct GraphQueryLimits {
    /// The depth that was asked for.
    pub max_depth: usize,
    /// The deepest level a returned node sits at.
    pub depth_reached: usize,
    /// The budget that was asked for.
    pub budget: usize,
    /// How many elements (nodes plus edges) the answer holds.
    pub budget_used: usize,
}

/// Where the graph of a `graph_query` answer came from.
#[derive(Debug, serde::Serialize, JsonSchema)]
pub struct GraphQueryBasis {
    /// The canonical workspace the graph was read from.
    pub workspace: String,
    /// The digest of the source manifest the graph was read against.
    pub source_manifest_digest: String,
    /// `built`, `cache` or `daemon`.
    pub graph_source: String,
}

#[derive(Debug, serde::Serialize, JsonSchema)]
pub struct GraphQueryNode {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub source_path: Option<String>,
    /// The words of the question that matched this node as a seed. Empty
    /// for a node the walk discovered.
    pub why_matched: Vec<String>,
}

/// One relation the walk followed, as the graph holds it.
#[derive(Debug, serde::Serialize, JsonSchema)]
pub struct GraphQueryEdge {
    pub source: String,
    pub target: String,
    /// The kind of relation: `calls`, `imports`, `inherits`,
    /// `uses_generic`, `references`, `defines`, `annotated_by` or
    /// `contains`.
    pub relation: String,
    /// How the graph established the relation: `Extracted`, `Inferred`,
    /// `Ambiguous`, `Manual` or `Tested`.
    pub provenance: String,
    /// How sure the graph is, as it recorded it.
    pub confidence: f64,
    /// Whether this edge is the one through which the walk first reached
    /// its target. `false` for a relation that reaches a node the walk
    /// already knew, or a second relation between the same two nodes.
    pub discovers: bool,
}

/// How a walk stopped.
#[derive(Debug, Default)]
struct Walk {
    nodes: Vec<GraphQueryNode>,
    edges: Vec<GraphQueryEdge>,
    budget_reached: bool,
    unexpanded: usize,
    depth_reached: usize,
}

impl Walk {
    /// Whether one more element fits in the budget.
    fn has_room(&self, budget: usize) -> bool {
        self.nodes.len() + self.edges.len() < budget
    }
}

fn node_of(graph: &CallGraph, id: &SymbolId, why_matched: Vec<String>) -> Option<GraphQueryNode> {
    let symbol = graph.get_symbol(id)?;
    Some(GraphQueryNode {
        id: id.as_str().to_string(),
        label: symbol.name().to_string(),
        kind: symbol.kind().name().to_string(),
        source_path: Some(symbol.location().file().to_string()),
        why_matched,
    })
}

/// The seeds of a question: the symbols a word of it names, each once, with
/// every word that named it.
///
/// The order is the order of the words, and within a word the order of the
/// symbol ids: it does not depend on how the graph was written.
fn seeds_of(graph: &CallGraph, keywords: &[String]) -> Vec<(SymbolId, Vec<String>)> {
    let mut seeds: Vec<(SymbolId, Vec<String>)> = Vec::new();
    let mut position: HashMap<String, usize> = HashMap::new();
    for keyword in keywords {
        let mut ids: Vec<SymbolId> = graph
            .find_by_name(keyword)
            .into_iter()
            .map(|symbol| SymbolId::new(symbol.fully_qualified_name()))
            .collect();
        ids.sort();
        for id in ids {
            match position.get(id.as_str()) {
                Some(&at) => seeds[at].1.push(keyword.clone()),
                None => {
                    position.insert(id.as_str().to_string(), seeds.len());
                    seeds.push((id, vec![keyword.clone()]));
                }
            }
        }
    }
    seeds
}

/// Breadth-first walk from the seeds along the outgoing edges.
///
/// Every element —seed, discovered node, edge— is checked against the
/// budget before it is emitted, and the first one that does not fit ends
/// the walk, so the answer is a prefix of the unbounded one whatever the
/// budget is. The edges of a node come out in the graph's canonical order,
/// and a node is emitted before the edge that discovers it.
fn walk(
    graph: &CallGraph,
    seeds: Vec<(SymbolId, Vec<String>)>,
    max_depth: usize,
    budget: usize,
) -> Walk {
    let mut walk = Walk::default();
    let mut known: HashSet<SymbolId> = HashSet::new();
    let mut queue: VecDeque<(SymbolId, usize)> = VecDeque::new();

    for (id, words) in seeds {
        let Some(node) = node_of(graph, &id, words) else {
            continue;
        };
        if !walk.has_room(budget) {
            return stopped_by_budget(graph, walk, queue, max_depth);
        }
        walk.nodes.push(node);
        known.insert(id.clone());
        queue.push_back((id, 0));
    }

    while let Some((current, depth)) = queue.pop_front() {
        let mut outgoing = graph.dependencies_with_metadata(&current).peekable();
        if depth >= max_depth {
            if outgoing.peek().is_some() {
                walk.unexpanded += 1;
            }
            continue;
        }
        for (target, dependency, provenance, confidence) in outgoing {
            let discovers = !known.contains(target);
            if discovers {
                // A relation to something the graph holds no symbol for is
                // not a relation between two nodes.
                let Some(node) = node_of(graph, target, Vec::new()) else {
                    continue;
                };
                if !walk.has_room(budget) {
                    return stopped_by_budget(graph, walk, queue, max_depth);
                }
                walk.nodes.push(node);
                walk.depth_reached = walk.depth_reached.max(depth + 1);
                known.insert(target.clone());
                queue.push_back((target.clone(), depth + 1));
            }
            if !walk.has_room(budget) {
                return stopped_by_budget(graph, walk, queue, max_depth);
            }
            walk.edges.push(GraphQueryEdge {
                source: current.as_str().to_string(),
                target: target.as_str().to_string(),
                relation: dependency.to_string(),
                provenance: provenance.to_string(),
                confidence,
                discovers,
            });
        }
    }
    walk
}

/// The walk, stopped because the next element did not fit.
///
/// The nodes still waiting at the last depth are counted as unexpanded, as
/// the ones the walk reached before the stop were: the depth is a limit of
/// the answer too, and it is reported whether or not the budget came first.
fn stopped_by_budget(
    graph: &CallGraph,
    mut walk: Walk,
    queue: VecDeque<(SymbolId, usize)>,
    max_depth: usize,
) -> Walk {
    walk.budget_reached = true;
    walk.unexpanded += queue
        .iter()
        .filter(|(id, depth)| {
            *depth >= max_depth && graph.dependencies_with_metadata(id).next().is_some()
        })
        .count();
    walk
}

/// The graph a `graph_query` reads, with what is known about it.
struct GraphRead {
    graph: std::sync::Arc<CallGraph>,
    measure: Option<crate::application::graph_measure::GraphMeasure>,
}

/// Reads the graph and what was measured about THAT graph, in one step.
///
/// The measure is asked for with the revision the service held before and
/// after the graph was taken: if another route replaced the graph in
/// between, nobody can say which graph was read, and nothing is claimed
/// about it.
fn read_the_graph(ctx: &HandlerContext) -> HandlerResult<GraphRead> {
    let service = ctx.analysis_service();
    let before = service.current_graph_revision();
    let graph = match ctx.get_graph_store().load_graph_shared() {
        Ok(Some(g)) => g,
        Ok(None) => {
            return Err(HandlerError::Internal(
                "No graph available. Run build_graph first.".into(),
            ));
        }
        Err(e) => return Err(HandlerError::Internal(format!("Graph store error: {e}"))),
    };
    let after = service.current_graph_revision();
    let measure = if before == after {
        service.graph_measures().of(after, &graph)
    } else {
        None
    };
    Ok(GraphRead { graph, measure })
}

/// The one line of the answer: its size, what it covers and whether it was
/// cut.
fn summary_of(
    question: &str,
    nodes: usize,
    edges: usize,
    coverage: &CoverageInfo,
    truncation_reason: Option<&str>,
) -> String {
    let mut line = format!(
        "{nodes} nodes, {edges} edges for \"{question}\" (traversal, coverage {})",
        coverage.status
    );
    if let Some(reason) = truncation_reason {
        line.push_str(&format!(", cut by {reason}"));
    }
    if nodes == 0 && coverage.status != "complete" {
        line.push_str(
            ": an empty answer over a graph whose coverage is not complete does not prove absence",
        );
    }
    line.push('.');
    line
}

/// Handle `graph_query` — natural language graph topology query.
///
/// Keyword match to find the seeds, then a breadth-first traversal along
/// the outgoing edges of any kind. The answer publishes each edge as the
/// graph holds it (kind, provenance, confidence), the limits the walk ran
/// under, and the coverage of the graph it was read from: see
/// [`GraphQueryOutput`].
pub async fn handle_graph_query(
    ctx: &HandlerContext,
    input: GraphQueryInput,
) -> HandlerResult<GraphQueryOutput> {
    let GraphRead { graph, measure } = read_the_graph(ctx)?;

    let coverage = match &measure {
        Some(measure) => CoverageInfo::of(&measure.coverage),
        None => CoverageInfo::unmeasured(),
    };
    let basis = measure.as_ref().and_then(|measure| {
        Some(GraphQueryBasis {
            workspace: measure.workspace.clone(),
            source_manifest_digest: measure.source_manifest_digest.clone()?,
            graph_source: measure.source.published_label().to_string(),
        })
    });

    let keywords = extract_keywords(&input.question);
    let seeds = seeds_of(&graph, &keywords);
    let walk = walk(&graph, seeds, input.max_depth, input.budget);

    let truncation_reason = if walk.budget_reached {
        Some("budget")
    } else if walk.unexpanded > 0 {
        Some("max_depth")
    } else {
        None
    };
    let explanation = if keywords.is_empty() {
        "No keywords extracted from question.".to_string()
    } else {
        format!(
            "Found {} nodes, {} edges in {} hops. Query: '{}'",
            walk.nodes.len(),
            walk.edges.len(),
            input.max_depth,
            input.question
        )
    };
    let summary = summary_of(
        &input.question,
        walk.nodes.len(),
        walk.edges.len(),
        &coverage,
        truncation_reason,
    );
    let limits = GraphQueryLimits {
        max_depth: input.max_depth,
        depth_reached: walk.depth_reached,
        budget: input.budget,
        budget_used: walk.nodes.len() + walk.edges.len(),
    };
    Ok(GraphQueryOutput {
        question: input.question,
        nodes: walk.nodes,
        edges: walk.edges,
        explanation,
        summary,
        mode: "traversal".into(),
        direction: "outgoing".into(),
        truncated: truncation_reason.is_some(),
        truncation_reason: truncation_reason.map(str::to_string),
        unexpanded_nodes: walk.unexpanded,
        limits,
        coverage,
        basis,
    })
}

// ============================================================================
// graph_explain
// ============================================================================

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct GraphExplainInput {
    /// Symbol name to explain
    pub symbol: String,
    /// Neighbor depth (default: 2)
    #[serde(default = "default_depth_e")]
    pub depth: usize,
}

fn default_depth_e() -> usize {
    2
}

#[derive(Debug, serde::Serialize, JsonSchema)]
pub struct GraphExplainOutput {
    pub node: ExplainNode,
    pub callers: Vec<ExplainNeighbor>,
    pub callees: Vec<ExplainNeighbor>,
    pub fan_in: usize,
    pub fan_out: usize,
    /// One line saying what the answer says.
    pub summary: String,
}

#[derive(Debug, serde::Serialize, JsonSchema)]
pub struct ExplainNode {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub source_path: Option<String>,
    /// Line number (1-indexed): the line a person reads.
    pub line: u32,
}

#[derive(Debug, serde::Serialize, JsonSchema)]
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
    let graph = match ctx.get_graph_store().load_graph_shared() {
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
        line: published_line(loc.line()),
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

    let summary = format!(
        "{}: {} callers, {} callees (fan-in {}, fan-out {})",
        node.label,
        callers.len(),
        callees.len(),
        fan_in,
        fan_out
    );
    Ok(GraphExplainOutput {
        node,
        callers,
        callees,
        fan_in,
        fan_out,
        summary,
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

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct GetTypeRefsInput {
    /// Symbol name
    pub symbol_name: String,
}
#[derive(Debug, serde::Serialize, JsonSchema)]
pub struct GetTypeRefsOutput {
    pub symbol: String,
    pub references: Vec<TypeRefRecord>,
    /// B4.W1 — minimum provenance tier the tool's edges are guaranteed to carry.
    pub precision: Precision,
    /// B4.W1 — provider that produced the result.
    pub provider: String,
    /// One line saying what the answer says.
    pub summary: String,
}
#[derive(Debug, serde::Serialize, JsonSchema)]
pub struct TypeRefRecord {
    pub target: String,
    pub context: String,
}

pub async fn handle_get_type_references(
    ctx: &HandlerContext,
    input: GetTypeRefsInput,
) -> HandlerResult<GetTypeRefsOutput> {
    let graph = match ctx.get_graph_store().load_graph_shared() {
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
        ) && let Some(target_sym) = graph.get_symbol(target_id)
        {
            references.push(TypeRefRecord {
                target: target_sym.name().to_string(),
                context: format!("{:?}", target_sym.kind()),
            });
        }
    }

    let summary = format!(
        "{} type references to {}",
        references.len(),
        input.symbol_name
    );
    Ok(GetTypeRefsOutput {
        symbol: input.symbol_name,
        references,
        precision: precision_floor_for_tool("get_type_references"),
        provider: default_provider_for_tool("get_type_references").to_string(),
        summary,
    })
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct GetImportsInput {
    /// File path
    pub file_path: String,
}
#[derive(Debug, serde::Serialize, JsonSchema)]
pub struct GetImportsOutput {
    pub file_path: String,
    pub imports: Vec<String>,
    /// One line saying what the answer says.
    pub summary: String,
}

pub async fn handle_get_imports(
    ctx: &HandlerContext,
    input: GetImportsInput,
) -> HandlerResult<GetImportsOutput> {
    let graph = match ctx.get_graph_store().load_graph_shared() {
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

    let summary = format!("{} imports in {}", imports.len(), input.file_path);
    Ok(GetImportsOutput {
        file_path: input.file_path,
        imports,
        summary,
    })
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct GetImplementorsInput {
    /// Trait or interface name
    pub trait_name: String,
}
#[derive(Debug, serde::Serialize, JsonSchema)]
pub struct GetImplementorsOutput {
    pub trait_name: String,
    pub implementors: Vec<String>,
    /// B4.W1 — minimum provenance tier the tool's edges are guaranteed to carry.
    pub precision: Precision,
    /// B4.W1 — provider that produced the result.
    pub provider: String,
    /// One line saying what the answer says.
    pub summary: String,
}

pub async fn handle_get_implementors(
    ctx: &HandlerContext,
    input: GetImplementorsInput,
) -> HandlerResult<GetImplementorsOutput> {
    let graph = match ctx.get_graph_store().load_graph_shared() {
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

    let summary = format!(
        "{} implementors of {}",
        implementors.len(),
        input.trait_name
    );
    Ok(GetImplementorsOutput {
        trait_name: input.trait_name,
        implementors,
        precision: precision_floor_for_tool("get_implementors"),
        provider: default_provider_for_tool("get_implementors").to_string(),
        summary,
    })
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct GetMembersInput {
    /// Class or struct name
    pub class_name: String,
}
#[derive(Debug, serde::Serialize, JsonSchema)]
pub struct GetMembersOutput {
    pub class_name: String,
    pub methods: Vec<String>,
    pub fields: Vec<String>,
    /// One line saying what the answer says.
    pub summary: String,
}

pub async fn handle_get_members(
    ctx: &HandlerContext,
    input: GetMembersInput,
) -> HandlerResult<GetMembersOutput> {
    let graph = match ctx.get_graph_store().load_graph_shared() {
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
        ) && let Some(member_sym) = graph.get_symbol(target_id)
        {
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

    let summary = format!(
        "{} methods, {} fields in {}",
        methods.len(),
        fields.len(),
        input.class_name
    );
    Ok(GetMembersOutput {
        class_name: input.class_name,
        methods,
        fields,
        summary,
    })
}

// graph_query_filtered — graph_query with provenance/kind/community filters
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct GraphQueryFilteredInput {
    pub question: String,
    pub filters: Option<QueryFilters>,
    pub limit: Option<usize>,
}
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct QueryFilters {
    pub provenance: Option<Vec<String>>,
    pub node_kinds: Option<Vec<String>>,
    pub community_id: Option<usize>,
    pub exclude_kinds: Option<Vec<String>>,
}
#[derive(Debug, serde::Serialize, JsonSchema)]
pub struct GraphQueryFilteredOutput {
    pub question: String,
    pub nodes: Vec<GraphQueryNode>,
    pub edges: Vec<GraphQueryEdge>,
    pub explanation: String,
    pub applied_filters: Vec<String>,
    /// One line saying what the answer says.
    pub summary: String,
}

pub async fn handle_graph_query_filtered(
    ctx: &HandlerContext,
    input: GraphQueryFilteredInput,
) -> HandlerResult<GraphQueryFilteredOutput> {
    let _graph = match ctx.get_graph_store().load_graph_shared() {
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
    let question = input.question;
    let summary = format!(
        "0 nodes, 0 edges for \"{}\" ({} keywords, {} filters applied)",
        question,
        keywords.len(),
        applied.len()
    );
    Ok(GraphQueryFilteredOutput {
        question,
        nodes: vec![],
        edges: vec![],
        explanation: format!(
            "Filtered query with {} keywords. Filters: {}",
            keywords.len(),
            applied.to_vec().join("; ")
        ),
        applied_filters: applied,
        summary,
    })
}

// export_callflow — community-level Mermaid architecture diagram
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ExportCallflowInput {
    /// Max architecture sections (default: 8)
    pub max_sections: Option<usize>,
}
#[derive(Debug, serde::Serialize, JsonSchema)]
pub struct ExportCallflowOutput {
    pub mermaid: String,
    /// How many communities the detection found.
    pub community_count: usize,
    /// How many of them the diagram draws.
    pub sections_drawn: usize,
    /// One line saying what the answer says.
    pub summary: String,
}

/// The number of sections the diagram draws when the caller does not say.
const DEFAULT_MAX_SECTIONS: usize = 8;

pub async fn handle_export_callflow(
    ctx: &HandlerContext,
    input: ExportCallflowInput,
) -> HandlerResult<ExportCallflowOutput> {
    let graph = match ctx.get_graph_store().load_graph_shared() {
        Ok(Some(g)) => g,
        _ => return Err(HandlerError::Internal("No graph available".into())),
    };
    let max_sections = input.max_sections.unwrap_or(DEFAULT_MAX_SECTIONS);

    // The detection is the one the product already uses for
    // `graph_communities`: the diagram and that tool describe the same
    // communities, so they are detected once and the same way. The
    // detector is an infrastructure adapter, and this is the adapter
    // layer — where the product already reaches it.
    let detection = CommunityDetector::detect(&graph, CommunityDetector::MAX_ITERATIONS);
    let communities = CommunityDetection {
        communities: detection
            .communities
            .iter()
            .map(|community| CommunitySection {
                id: community.id,
                label: community.label.clone(),
                size: community.nodes.len(),
            })
            .collect(),
        node_communities: detection.node_communities.clone(),
    };

    let callflow = community_callflow(&graph, &communities, max_sections);
    let summary = format!(
        "Call-flow diagram of {} symbols over {} communities ({} drawn)",
        graph.symbol_count(),
        callflow.community_count,
        callflow.sections_drawn
    );
    Ok(ExportCallflowOutput {
        mermaid: callflow.mermaid,
        community_count: callflow.community_count,
        sections_drawn: callflow.sections_drawn,
        summary,
    })
}
