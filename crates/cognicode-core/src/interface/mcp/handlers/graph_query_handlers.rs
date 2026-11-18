//! GraphQuery and GraphExplain handlers — ADR-026 Sprint 2.
//!
//! `graph_query`: Natural-language graph topology query (simplified v1).
//! `graph_explain`: Composite deep-dive on one node.
// e30.1 clippy baseline reset: pre-existing lint debt (see fix/e30.1-clippy-baseline-reset)
#![allow(clippy::unnecessary_to_owned, clippy::unnecessary_unwrap)]

use std::collections::{HashMap, HashSet, VecDeque};

use crate::application::graph_measure::GraphMeasure;
use crate::application::services::graph_operators::{
    QueryError, Relation, Target, TypedQuery, run, symbols_named,
};
use crate::application::services::graph_queries::{
    CommunityDetection, CommunitySection, GraphQueryError, community_callflow,
};
use crate::domain::aggregates::call_graph::{CallGraph, SymbolId};
use crate::domain::value_objects::{Provenance, SymbolKind, published_line};
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

/// Where the graph an answer was read from came from: the basis `graph_query`
/// and `codebase_map` publish.
#[derive(Debug, serde::Serialize, JsonSchema)]
pub struct GraphQueryBasis {
    /// The canonical workspace the graph was read from.
    pub workspace: String,
    /// The digest of the source manifest the graph was read against.
    pub source_manifest_digest: String,
    /// `built`, `cache` or `daemon`.
    pub graph_source: String,
}

impl GraphQueryBasis {
    /// The basis of the graph `measure` was taken for. `None` when the
    /// measure does not know the digest of the manifest the graph was read
    /// against: a basis without it would not say which snapshot it is.
    pub(crate) fn of(measure: &GraphMeasure) -> Option<Self> {
        Some(Self {
            workspace: measure.workspace.clone(),
            source_manifest_digest: measure.source_manifest_digest.clone()?,
            graph_source: measure.source.published_label().to_string(),
        })
    }
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

/// What a filtered query left out of the walk because a filter excluded it,
/// as opposed to the budget or the depth.
///
/// The counts are over the part of the walk that ran: when the answer is
/// cut, what lies beyond the cut is counted by nobody.
#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Serialize, JsonSchema)]
pub struct FilteredOut {
    /// Distinct symbols the walk reached and a node filter excluded: seeds
    /// and discovered nodes alike.
    pub nodes: usize,
    /// Relations the walk met and a filter excluded: those whose provenance
    /// is not admitted, and those that lead to an excluded node.
    pub edges: usize,
}

/// How a walk stopped.
#[derive(Debug, Default)]
struct Walk {
    nodes: Vec<GraphQueryNode>,
    edges: Vec<GraphQueryEdge>,
    budget_reached: bool,
    unexpanded: usize,
    depth_reached: usize,
    filtered_out: FilteredOut,
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
        for id in symbols_named(graph, keyword) {
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

/// What a walk is allowed to follow and to return.
///
/// The default admits everything: it is the traversal `graph_query` runs.
/// `graph_query_filtered` narrows that same traversal, so a filter restricts
/// what the walk follows (and so what it can reach through it), never what
/// the answer claims to have looked at.
#[derive(Debug, Default)]
struct QueryFilter {
    /// Provenances the walk follows. `None` follows every provenance.
    provenance: Option<HashSet<Provenance>>,
    /// Kinds a node must have. `None` admits every kind.
    kinds: Option<HashSet<SymbolKind>>,
    /// Kinds no node may have. Wins over `kinds`.
    excluded_kinds: HashSet<SymbolKind>,
    /// The community every node must belong to.
    community: Option<CommunityScope>,
}

/// A community, as the detection that `graph_communities` runs numbers it.
#[derive(Debug)]
struct CommunityScope {
    id: u32,
    /// Community of every symbol, by symbol id.
    members: HashMap<String, u32>,
}

impl QueryFilter {
    fn admits_edge(&self, provenance: Provenance) -> bool {
        self.provenance
            .as_ref()
            .is_none_or(|admitted| admitted.contains(&provenance))
    }

    /// Whether the walk may reach `id`. A relation to something the graph
    /// holds no symbol for is not the filter's to judge: the walk drops it
    /// on its own, as it does without filters.
    fn admits_node(&self, graph: &CallGraph, id: &SymbolId) -> bool {
        let Some(symbol) = graph.get_symbol(id) else {
            return true;
        };
        let kind = symbol.kind();
        if self.excluded_kinds.contains(kind) {
            return false;
        }
        if self
            .kinds
            .as_ref()
            .is_some_and(|kinds| !kinds.contains(kind))
        {
            return false;
        }
        self.community
            .as_ref()
            .is_none_or(|scope| scope.members.get(id.as_str()) == Some(&scope.id))
    }

    /// Whether `id` has an outgoing relation this filter lets the walk
    /// follow.
    fn has_followable_outgoing(&self, graph: &CallGraph, id: &SymbolId) -> bool {
        graph
            .dependencies_with_metadata(id)
            .any(|(target, _, provenance, _)| {
                self.admits_edge(provenance) && self.admits_node(graph, target)
            })
    }
}

/// Breadth-first walk from the seeds along the outgoing edges.
///
/// Every element —seed, discovered node, edge— is checked against the
/// budget before it is emitted, and the first one that does not fit ends
/// the walk, so the answer is a prefix of the unbounded one whatever the
/// budget is. The edges of a node come out in the graph's canonical order,
/// and a node is emitted before the edge that discovers it. The filter
/// decides what is followed before the budget sees it: an excluded element
/// is counted in `filtered_out` and costs no budget.
fn walk(
    graph: &CallGraph,
    filter: &QueryFilter,
    seeds: Vec<(SymbolId, Vec<String>)>,
    max_depth: usize,
    budget: usize,
) -> Walk {
    let mut walk = Walk::default();
    let mut known: HashSet<SymbolId> = HashSet::new();
    let mut excluded: HashSet<SymbolId> = HashSet::new();
    let mut queue: VecDeque<(SymbolId, usize)> = VecDeque::new();

    for (id, words) in seeds {
        let Some(node) = node_of(graph, &id, words) else {
            continue;
        };
        if !filter.admits_node(graph, &id) {
            excluded.insert(id);
            continue;
        }
        if !walk.has_room(budget) {
            walk.filtered_out.nodes = excluded.len();
            return stopped_by_budget(graph, filter, walk, queue, max_depth);
        }
        walk.nodes.push(node);
        known.insert(id.clone());
        queue.push_back((id, 0));
    }

    while let Some((current, depth)) = queue.pop_front() {
        if depth >= max_depth {
            if filter.has_followable_outgoing(graph, &current) {
                walk.unexpanded += 1;
            }
            continue;
        }
        for (target, dependency, provenance, confidence) in
            graph.dependencies_with_metadata(&current)
        {
            if !filter.admits_edge(provenance) {
                walk.filtered_out.edges += 1;
                continue;
            }
            let discovers = !known.contains(target);
            if discovers {
                if !filter.admits_node(graph, target) {
                    excluded.insert(target.clone());
                    walk.filtered_out.edges += 1;
                    continue;
                }
                // A relation to something the graph holds no symbol for is
                // not a relation between two nodes.
                let Some(node) = node_of(graph, target, Vec::new()) else {
                    continue;
                };
                if !walk.has_room(budget) {
                    walk.filtered_out.nodes = excluded.len();
                    return stopped_by_budget(graph, filter, walk, queue, max_depth);
                }
                walk.nodes.push(node);
                walk.depth_reached = walk.depth_reached.max(depth + 1);
                known.insert(target.clone());
                queue.push_back((target.clone(), depth + 1));
            }
            if !walk.has_room(budget) {
                walk.filtered_out.nodes = excluded.len();
                return stopped_by_budget(graph, filter, walk, queue, max_depth);
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
    walk.filtered_out.nodes = excluded.len();
    walk
}

/// The walk, stopped because the next element did not fit.
///
/// The nodes still waiting at the last depth are counted as unexpanded, as
/// the ones the walk reached before the stop were: the depth is a limit of
/// the answer too, and it is reported whether or not the budget came first.
fn stopped_by_budget(
    graph: &CallGraph,
    filter: &QueryFilter,
    mut walk: Walk,
    queue: VecDeque<(SymbolId, usize)>,
    max_depth: usize,
) -> Walk {
    walk.budget_reached = true;
    walk.unexpanded += queue
        .iter()
        .filter(|(id, depth)| *depth >= max_depth && filter.has_followable_outgoing(graph, id))
        .count();
    walk
}

/// The graph a `graph_query` reads, with what is known about it.
pub(crate) struct GraphRead {
    pub(crate) graph: std::sync::Arc<CallGraph>,
    pub(crate) measure: Option<GraphMeasure>,
}

/// Reads the graph and what was measured about THAT graph, in one step.
///
/// The measure is asked for with the revision the service held before and
/// after the graph was taken: if another route replaced the graph in
/// between, nobody can say which graph was read, and nothing is claimed
/// about it.
pub(crate) fn read_the_graph(ctx: &HandlerContext) -> HandlerResult<GraphRead> {
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
/// cut, and what the filters left out when there were any.
fn summary_of(
    question: &str,
    nodes: usize,
    edges: usize,
    coverage: &CoverageInfo,
    truncation_reason: Option<&str>,
    filtered_out: &FilteredOut,
) -> String {
    let mut line = format!(
        "{nodes} nodes, {edges} edges for \"{question}\" (traversal, coverage {})",
        coverage.status
    );
    if let Some(reason) = truncation_reason {
        line.push_str(&format!(", cut by {reason}"));
    }
    if *filtered_out != FilteredOut::default() {
        line.push_str(&format!(
            ", {} nodes and {} edges left out by the filters",
            filtered_out.nodes, filtered_out.edges
        ));
    }
    if nodes == 0 && coverage.status != "complete" {
        line.push_str(
            ": an empty answer over a graph whose coverage is not complete does not prove absence",
        );
    }
    line.push('.');
    line
}

/// The traversal both graph query tools run: the seeds the question names,
/// the walk under `filter`, and the limits and coverage of what was read.
///
/// The answer is a function of the graph, the question, the limits and the
/// filter; nothing else decides what comes out.
fn answer_for(
    read: GraphRead,
    question: String,
    max_depth: usize,
    budget: usize,
    filter: &QueryFilter,
) -> (GraphQueryOutput, FilteredOut) {
    let GraphRead { graph, measure } = read;
    let coverage = match &measure {
        Some(measure) => CoverageInfo::of(&measure.coverage),
        None => CoverageInfo::unmeasured(),
    };
    let basis = measure.as_ref().and_then(GraphQueryBasis::of);

    let keywords = extract_keywords(&question);
    let seeds = seeds_of(&graph, &keywords);
    let walk = walk(&graph, filter, seeds, max_depth, budget);

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
            max_depth,
            question
        )
    };
    let summary = summary_of(
        &question,
        walk.nodes.len(),
        walk.edges.len(),
        &coverage,
        truncation_reason,
        &walk.filtered_out,
    );
    let limits = GraphQueryLimits {
        max_depth,
        depth_reached: walk.depth_reached,
        budget,
        budget_used: walk.nodes.len() + walk.edges.len(),
    };
    let answer = GraphQueryOutput {
        question,
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
    };
    (answer, walk.filtered_out)
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
    let read = read_the_graph(ctx)?;
    let (answer, _) = answer_for(
        read,
        input.question,
        input.max_depth,
        input.budget,
        &QueryFilter::default(),
    );
    Ok(answer)
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

    // The question has one owner. A name that several symbols carry is not
    // a question about whichever came first: the answer lists nothing and
    // its summary says which declarations share the name.
    let query = TypedQuery::new(
        Target::Name(input.trait_name.clone()),
        vec![Relation::Implementors],
    );
    let (implementors, summary) = match run(&graph, &query, None) {
        Ok(answer) => {
            let mut rows = answer.rows;
            rows.sort_by(|a, b| a.id.cmp(&b.id));
            let implementors: Vec<String> = rows
                .iter()
                .filter_map(|row| graph.get_symbol(&row.id))
                .map(|symbol| symbol.name().to_string())
                .collect();
            let summary = format!(
                "{} implementors of {}",
                implementors.len(),
                input.trait_name
            );
            (implementors, summary)
        }
        Err(QueryError::Graph(GraphQueryError::Ambiguous { name, candidates })) => (
            Vec::new(),
            format!(
                "0 implementors listed: `{name}` is declared {} times ({}) and none was picked",
                candidates.len(),
                candidates.join(", ")
            ),
        ),
        Err(QueryError::Graph(GraphQueryError::NotFound { .. })) => {
            return Err(HandlerError::NotFound(format!(
                "Trait/interface '{}' not found",
                input.trait_name
            )));
        }
        Err(QueryError::Invalid(invalid)) => {
            return Err(HandlerError::InvalidInput(invalid.to_string()));
        }
    };

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

// ============================================================================
// graph_query_filtered
// ============================================================================

/// What `graph_query_filtered` is asked: a `graph_query` question, narrowed.
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct GraphQueryFilteredInput {
    /// Natural language question about the graph topology; its words seed
    /// the walk exactly as they do for `graph_query`.
    pub question: String,
    /// What the walk may follow and return. Omitted or empty, the answer is
    /// the one `graph_query` gives.
    pub filters: Option<QueryFilters>,
    /// Maximum elements the answer holds, counting seed nodes, discovered
    /// nodes and edges together (default: 1500): the budget of
    /// `graph_query`. The answer never exceeds it; what the filters exclude
    /// costs none of it.
    pub limit: Option<usize>,
    /// Maximum BFS depth from the seed nodes (default: 3). Depth 0 returns
    /// only the seeds.
    #[serde(default = "default_max_depth")]
    pub max_depth: usize,
}

/// The filters of `graph_query_filtered`. Every one that is given must hold;
/// a name that is not one of the accepted values is refused, never read as a
/// filter that matches nothing.
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct QueryFilters {
    /// Provenances the walk follows: `Extracted`, `Inferred`, `Ambiguous`,
    /// `Manual` or `Tested`, in any letter case. An edge of another
    /// provenance is not followed, and the nodes only it reaches are not
    /// reached. Must not be empty.
    pub provenance: Option<Vec<String>>,
    /// Kinds a node must have to be returned or walked through, by the name
    /// `graph_query` publishes in `kind`, for example `function`, `class` or
    /// `trait`. Must not be empty.
    pub node_kinds: Option<Vec<String>>,
    /// The community, as `graph_communities` numbers them, every node must
    /// belong to. A number the detection does not produce is refused.
    pub community_id: Option<usize>,
    /// Kinds no node may have. Wins over `node_kinds`.
    pub exclude_kinds: Option<Vec<String>>,
}

/// What `graph_query_filtered` answers: the answer of `graph_query` over the
/// narrowed traversal, and what the narrowing took out.
#[derive(Debug, serde::Serialize, JsonSchema)]
pub struct GraphQueryFilteredOutput {
    /// The traversal, its limits, its coverage and its basis, as
    /// `graph_query` publishes them.
    #[serde(flatten)]
    pub answer: GraphQueryOutput,
    /// The filters the walk ran under, one line each.
    pub applied_filters: Vec<String>,
    /// What the filters excluded from the walk.
    pub filtered_out: FilteredOut,
}

fn refuse_empty(name: &str, values: &[String]) -> HandlerResult<()> {
    if values.is_empty() {
        return Err(HandlerError::InvalidInput(format!(
            "the {name} filter is empty: omit it to admit everything"
        )));
    }
    Ok(())
}

fn parse_provenances(values: &[String]) -> HandlerResult<HashSet<Provenance>> {
    const ACCEPTED: [Provenance; 5] = [
        Provenance::Extracted,
        Provenance::Inferred,
        Provenance::Ambiguous,
        Provenance::Manual,
        Provenance::Tested,
    ];
    values
        .iter()
        .map(|value| {
            ACCEPTED
                .into_iter()
                .find(|provenance| provenance.name() == value.to_lowercase())
                .ok_or_else(|| {
                    HandlerError::InvalidInput(format!(
                        "unknown provenance '{value}': expected one of {}",
                        ACCEPTED.map(|p| p.to_string()).join(", ")
                    ))
                })
        })
        .collect()
}

fn parse_kinds(name: &str, values: &[String]) -> HandlerResult<HashSet<SymbolKind>> {
    values
        .iter()
        .map(|value| {
            value.to_lowercase().parse::<SymbolKind>().map_err(|_| {
                HandlerError::InvalidInput(format!(
                    "unknown kind '{value}' in {name}: use the names the answer publishes in `kind`, for example function, class, trait"
                ))
            })
        })
        .collect()
}

/// The community `id`, as `graph_communities` numbers it, or a refusal that
/// says how many communities the graph has.
fn community_scope(graph: &CallGraph, id: usize) -> HandlerResult<(CommunityScope, String)> {
    let detection = CommunityDetector::detect(graph, CommunityDetector::MAX_ITERATIONS);
    let community = u32::try_from(id)
        .ok()
        .and_then(|id| detection.communities.iter().find(|c| c.id == id));
    let Some(community) = community else {
        return Err(HandlerError::InvalidInput(format!(
            "community {id} does not exist: the graph has {} communities (0..{})",
            detection.communities.len(),
            detection.communities.len()
        )));
    };
    let mut line = format!(
        "community {} ({}, {} symbols)",
        community.id,
        community.label,
        community.nodes.len()
    );
    if !detection.converged {
        line.push_str(
            "; the detection did not converge, so the community is the last one it reached",
        );
    }
    let scope = CommunityScope {
        id: community.id,
        members: detection.node_communities,
    };
    Ok((scope, line))
}

/// The filter the input asks for, and one line per filter that narrows the
/// walk.
fn filter_of(
    graph: &CallGraph,
    filters: Option<QueryFilters>,
) -> HandlerResult<(QueryFilter, Vec<String>)> {
    let mut filter = QueryFilter::default();
    let mut applied = Vec::new();
    let Some(filters) = filters else {
        return Ok((filter, applied));
    };
    if let Some(values) = &filters.provenance {
        refuse_empty("provenance", values)?;
        filter.provenance = Some(parse_provenances(values)?);
        applied.push(format!("provenance in [{}]", values.join(", ")));
    }
    if let Some(values) = &filters.node_kinds {
        refuse_empty("node_kinds", values)?;
        filter.kinds = Some(parse_kinds("node_kinds", values)?);
        applied.push(format!("node_kinds in [{}]", values.join(", ")));
    }
    if let Some(values) = &filters.exclude_kinds
        && !values.is_empty()
    {
        filter.excluded_kinds = parse_kinds("exclude_kinds", values)?;
        applied.push(format!("exclude_kinds [{}]", values.join(", ")));
    }
    if let Some(id) = filters.community_id {
        let (scope, line) = community_scope(graph, id)?;
        filter.community = Some(scope);
        applied.push(line);
    }
    Ok((filter, applied))
}

/// Handle `graph_query_filtered` — `graph_query` over a narrowed traversal.
///
/// It runs the same walk as [`handle_graph_query`] (same seeds, same order,
/// same budget and depth rules, same coverage and basis) and applies the
/// filters to what the walk follows. The answer says which filters ran and
/// how many nodes and edges they excluded; an invalid filter is refused
/// rather than answered with nothing.
pub async fn handle_graph_query_filtered(
    ctx: &HandlerContext,
    input: GraphQueryFilteredInput,
) -> HandlerResult<GraphQueryFilteredOutput> {
    let read = read_the_graph(ctx)?;
    let (filter, applied_filters) = filter_of(&read.graph, input.filters)?;
    let (answer, filtered_out) = answer_for(
        read,
        input.question,
        input.max_depth,
        input.limit.unwrap_or_else(default_budget),
        &filter,
    );
    Ok(GraphQueryFilteredOutput {
        answer,
        applied_filters,
        filtered_out,
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
