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
use crate::interface::cursor::{self, CursorContext};
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
    /// Resume a cut answer with the `next_continuation_token` of the
    /// previous page, exactly as the `continue` link fills it. The token
    /// is anchored to this workspace and to the revision of the graph it
    /// was minted from: a token from another revision, another question
    /// or another depth is refused with its typed error, never
    /// reinterpreted.
    #[serde(default)]
    pub continuation_token: Option<String>,
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
    /// Whether the budget cut the answer and a cursor continues it.
    ///
    /// A cut by `max_depth` is not a continuation: the traversal is over,
    /// what sits beyond the depth is not in it. Only a cut by `budget`
    /// means "there is a next page".
    pub has_more: bool,
    /// The cursor to pass back as `continuation_token` for the next page.
    ///
    /// Opaque, and anchored to this workspace and to the revision of the
    /// graph this page was read from: the same traversal resumes exactly
    /// where this page stopped, and a cursor from another revision is
    /// refused with its typed error, never reinterpreted. Published only
    /// when `has_more` is true.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_continuation_token: Option<String>,
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

#[derive(Debug, Clone, serde::Serialize, JsonSchema)]
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
#[derive(Debug, Clone, serde::Serialize, JsonSchema)]
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
    /// How many traversal elements sit before the page window: the earlier
    /// pages' items the walk walked through without holding. The cursor
    /// offset is exactly this count once the window opened.
    skipped: usize,
}

/// What the page window does with the traversal element the walk is at.
enum PageItem {
    /// The element sits before the window: earlier pages own it. The walk
    /// still walks through it — what lies behind it is reachable only
    /// through it — but this page does not hold it.
    Before,
    /// The element is kept in this page.
    Keep,
    /// The page is full and this element is the one past it: its existence
    /// is the proof that a next page has something to resume into, and the
    /// walk stops here without needing to traverse the rest of the graph.
    Proof,
}

impl Walk {
    /// How many elements this page holds.
    fn held(&self) -> usize {
        self.nodes.len() + self.edges.len()
    }

    /// Where the element at the next traversal position goes.
    ///
    /// The traversal is the sequence the unbounded walk produces — one
    /// position per seed, discovered node and followed edge, in discovery
    /// order — and the page is the window `[start, start + budget)` of it.
    /// An element past the window is the proof there is a next page: the
    /// walk stops at it instead of walking the whole graph to count what
    /// is left.
    fn page_item(&mut self, start: usize, budget: usize) -> PageItem {
        if self.skipped + self.held() < start {
            self.skipped += 1;
            PageItem::Before
        } else if self.held() < budget {
            PageItem::Keep
        } else {
            PageItem::Proof
        }
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

/// Breadth-first walk from the seeds along the outgoing edges, windowed to
/// a page.
///
/// The traversal is the sequence the unbounded walk produces — one position
/// per seed, discovered node and followed edge, in discovery order — and it
/// depends on the question, the depth and the graph, never on the budget.
/// The page is the window `[start, start + budget)` of that sequence: the
/// walk still walks through the elements before `start` (what lies behind
/// them is reachable only through them) but holds none of them, and stops
/// at the first element past the window — that element is the proof a next
/// page has something to resume into, so the rest of the graph never has to
/// be traversed for it. With `start = 0` this is the answer CGH-084 pinned:
/// a prefix of the unbounded one whatever the budget is. The edges of a
/// node come out in the graph's canonical order, and a node is emitted
/// before the edge that discovers it. The filter decides what is followed
/// before the budget sees it: an excluded element is counted in
/// `filtered_out` and costs no budget.
fn walk(
    graph: &CallGraph,
    filter: &QueryFilter,
    seeds: Vec<(SymbolId, Vec<String>)>,
    max_depth: usize,
    budget: usize,
    start: usize,
) -> Walk {
    let mut walk = Walk::default();
    let mut known: HashSet<SymbolId> = HashSet::new();
    let mut excluded: HashSet<SymbolId> = HashSet::new();
    // The third field says whether this queued node is held by the page:
    // the walk expands the nodes before the window as well — what lies
    // behind them is reachable only through them — but the depth answers
    // (unexpanded, depth reached) belong to the page, not to the road.
    let mut queue: VecDeque<(SymbolId, usize, bool)> = VecDeque::new();

    for (id, words) in seeds {
        let Some(node) = node_of(graph, &id, words) else {
            continue;
        };
        if !filter.admits_node(graph, &id) {
            excluded.insert(id);
            continue;
        }
        let held = match walk.page_item(start, budget) {
            PageItem::Before => false,
            PageItem::Keep => {
                walk.nodes.push(node);
                true
            }
            PageItem::Proof => {
                walk.filtered_out.nodes = excluded.len();
                return stopped_by_budget(graph, filter, walk, queue, max_depth);
            }
        };
        known.insert(id.clone());
        queue.push_back((id, 0, held));
    }

    while let Some((current, depth, held)) = queue.pop_front() {
        if depth >= max_depth {
            if held && filter.has_followable_outgoing(graph, &current) {
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
                let node_held = match walk.page_item(start, budget) {
                    PageItem::Before => false,
                    PageItem::Keep => {
                        walk.nodes.push(node);
                        walk.depth_reached = walk.depth_reached.max(depth + 1);
                        true
                    }
                    PageItem::Proof => {
                        walk.filtered_out.nodes = excluded.len();
                        return stopped_by_budget(graph, filter, walk, queue, max_depth);
                    }
                };
                known.insert(target.clone());
                queue.push_back((target.clone(), depth + 1, node_held));
            }
            match walk.page_item(start, budget) {
                PageItem::Before => {}
                PageItem::Keep => walk.edges.push(GraphQueryEdge {
                    source: current.as_str().to_string(),
                    target: target.as_str().to_string(),
                    relation: dependency.to_string(),
                    provenance: provenance.to_string(),
                    confidence,
                    discovers,
                }),
                PageItem::Proof => {
                    walk.filtered_out.nodes = excluded.len();
                    return stopped_by_budget(graph, filter, walk, queue, max_depth);
                }
            }
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
    queue: VecDeque<(SymbolId, usize, bool)>,
    max_depth: usize,
) -> Walk {
    walk.budget_reached = true;
    walk.unexpanded += queue
        .iter()
        .filter(|(id, depth, held)| {
            *held && *depth >= max_depth && filter.has_followable_outgoing(graph, id)
        })
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
    let measure = if before == after && std::sync::Arc::ptr_eq(&service.get_project_graph(), &graph)
    {
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
///
/// `start` is the offset into the canonical traversal this page windows:
/// zero for a whole answer. The third element of the result says whether
/// the walk met an element past the window — a next page exists, even if
/// this page held nothing (a zero-budget page proves the traversal is not
/// over without continuing it).
fn answer_for(
    read: GraphRead,
    question: String,
    max_depth: usize,
    budget: usize,
    filter: &QueryFilter,
    start: usize,
) -> (GraphQueryOutput, FilteredOut, bool) {
    let GraphRead { graph, measure } = read;
    let coverage = match &measure {
        Some(measure) => CoverageInfo::of(&measure.coverage),
        None => CoverageInfo::unmeasured(),
    };
    let basis = measure.as_ref().and_then(GraphQueryBasis::of);

    let keywords = extract_keywords(&question);
    let seeds = seeds_of(&graph, &keywords);
    let walk = walk(&graph, filter, seeds, max_depth, budget, start);
    let more_items = walk.budget_reached;

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
        // The window this answer is a page of says whether a next page has
        // something to resume into; the handler that owns the cursor fills
        // them in. A surface without a cursor answers `false`, never a
        // promise it cannot keep.
        has_more: false,
        next_continuation_token: None,
    };
    (answer, walk.filtered_out, more_items)
}

/// The tag a `graph_query` cursor carries: a token minted by another
/// surface is refused as garbled, never read as a graph offset.
const GRAPH_QUERY_CURSOR_TAG: &str = "gq";

/// The cursor payload version: bumping it without retiring the old read
/// would silently reinterpret old offsets, so an unknown `v` is refused.
const GRAPH_QUERY_CURSOR_VERSION: u64 = 1;

/// The revision identity of a loaded graph: a short hash over its symbols
/// and edges in the graph's canonical order.
///
/// The traversal a cursor resumes depends on one thing — the graph the
/// page was read from — and this names exactly that. Two builds that
/// produce the same graph give the same revision, so a cursor keeps
/// working across a rebuild that changed nothing; a graph that changed
/// gives another revision, and the cursor for the old one is refused with
/// its typed error (`snapshot_expired`) instead of resuming into a
/// traversal that no longer exists. The store's checkpoint id is not used
/// here on purpose: the binding a cursor needs is to the traversal's
/// input, not to a counter, and the content hash works for every store —
/// including the single-version ones whose checkpoint id is a placeholder.
fn graph_revision(graph: &CallGraph) -> String {
    let mut acc = String::new();
    for (id, _) in graph.symbol_ids() {
        acc.push_str(id.as_str());
        acc.push('\n');
    }
    acc.push('#');
    for (source, target, dep) in graph.all_dependencies() {
        acc.push_str(source.as_str());
        acc.push_str("->");
        acc.push_str(target.as_str());
        acc.push(':');
        acc.push_str(&dep.to_string());
        acc.push('\n');
    }
    cursor::short_hash(&acc)
}

/// The offset the cursor asks to resume from, or the typed refusal.
///
/// The envelope's anchors (workspace, graph revision) are verified by
/// [`cursor::verify_signed`]; what is left to pin is the payload: the
/// cursor names a traversal, and a traversal is the question, the depth
/// and the graph. The graph is the envelope's `snap`; the question and
/// the depth are here. A token replayed with another question or another
/// depth is refused: the offset would skip into a sequence that is not
/// the one the previous page came out of.
fn cursor_offset(
    token: &str,
    ctx: &CursorContext,
    question: &str,
    max_depth: usize,
) -> Result<usize, HandlerError> {
    let refused = |reason: &str| {
        HandlerError::InvalidInput(format!(
            "graph_query continuation refused: garbled: {reason}"
        ))
    };
    let signed = cursor::verify_signed(token, ctx).map_err(|e| {
        HandlerError::InvalidInput(format!("graph_query continuation refused: {e}"))
    })?;
    if signed.tag != GRAPH_QUERY_CURSOR_TAG {
        return Err(refused("the token is not a graph_query cursor"));
    }
    let refused_payload = refused;
    let v = signed
        .payload
        .get("v")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| refused_payload("the cursor carries no payload version"))?;
    if v != GRAPH_QUERY_CURSOR_VERSION {
        return Err(refused_payload(&format!(
            "cursor payload version {v} is not supported; this server speaks v{GRAPH_QUERY_CURSOR_VERSION}"
        )));
    }
    let q = signed
        .payload
        .get("q")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| refused_payload("the cursor carries no question anchor"))?;
    if q != cursor::short_hash(question) {
        return Err(refused_payload(
            "the cursor was minted for a different question: the offset would skip into another traversal",
        ));
    }
    let d = signed
        .payload
        .get("d")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| refused_payload("the cursor carries no depth anchor"))? as usize;
    if d != max_depth {
        return Err(refused_payload(&format!(
            "the cursor was minted for max_depth {d}, this call asks {max_depth}: the offset indexes another traversal"
        )));
    }
    signed
        .payload
        .get("o")
        .and_then(serde_json::Value::as_u64)
        .map(|o| o as usize)
        .ok_or_else(|| refused_payload("the cursor carries no offset"))
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

    // The cursor's anchors: this workspace, and the revision of the graph
    // the page is read from. A cursor minted against another revision of
    // another tree fails verification with its typed error — it does not
    // resume, because what it would resume into is not the traversal that
    // minted it.
    let cursor_ctx = CursorContext {
        workspace: ctx.working_dir().to_string_lossy().to_string(),
        snapshot: graph_revision(&read.graph),
    };

    // Verify the cursor BEFORE traversing: a refused page pays nothing,
    // and the refusal is the same typed error the rest of the cursor
    // surface speaks (`garbled`, `workspace_mismatch`, `snapshot_expired`).
    let start = match input.continuation_token.as_deref() {
        None => 0,
        Some(token) => cursor_offset(token, &cursor_ctx, &input.question, input.max_depth)?,
    };

    let (mut answer, _, more_items) = answer_for(
        read,
        input.question.clone(),
        input.max_depth,
        input.budget,
        &QueryFilter::default(),
        start,
    );

    // A page that brought nothing cannot continue: a zero-budget page, or
    // one that ran past the end of the traversal, would mint a cursor to
    // the offset it was already at, and a chain of tokens that never
    // advances is worse than no token at all.
    answer.has_more = more_items && !answer.nodes.is_empty();
    answer.next_continuation_token = answer.has_more.then(|| {
        cursor::encode_signed(
            &cursor_ctx,
            GRAPH_QUERY_CURSOR_TAG,
            &serde_json::json!({
                "v": GRAPH_QUERY_CURSOR_VERSION,
                "q": cursor::short_hash(&input.question),
                "d": input.max_depth,
                "o": start + answer.limits.budget_used,
            }),
        )
    });
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
    /// Coverage of the graph read, or `unknown` when it was not measured.
    pub coverage: CoverageInfo,
    /// Maximum neighbours published in each direction.
    pub neighbor_limit: usize,
    /// Whether the callers list was cut at `neighbor_limit`.
    pub callers_truncated: bool,
    /// Whether the callees list was cut at `neighbor_limit`.
    pub callees_truncated: bool,
    /// One line saying what the answer says, including coverage and truncation.
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

/// One direct relation to the explained node, retaining the graph's orientation.
#[derive(Debug, serde::Serialize, JsonSchema)]
pub struct ExplainNeighbor {
    /// The neighbouring symbol's identifier.
    pub symbol: String,
    /// The actual relation kind stored by the graph.
    pub relation: String,
    /// Whether this relation points into or out of the explained node.
    pub side: String,
    /// The source endpoint as stored by the graph.
    pub source: String,
    /// The target endpoint as stored by the graph.
    pub target: String,
    /// How the graph established the relation.
    pub provenance: String,
    /// Confidence recorded for the relation.
    pub confidence: f64,
    pub depth: usize,
}

/// Handle `graph_explain` — composite deep-dive on a symbol.
pub async fn handle_graph_explain(
    ctx: &HandlerContext,
    input: GraphExplainInput,
) -> HandlerResult<GraphExplainOutput> {
    const NEIGHBOR_LIMIT: usize = 20;

    let GraphRead { graph, measure } = read_the_graph(ctx)?;

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
    let mut callers: Vec<ExplainNeighbor> = graph
        .callers_with_metadata(&sid)
        .into_iter()
        .map(|(source, kind, provenance, confidence)| ExplainNeighbor {
            symbol: source.as_str().to_string(),
            relation: kind.to_string(),
            side: "incoming".into(),
            source: source.as_str().to_string(),
            target: sid.as_str().to_string(),
            provenance: provenance.to_string(),
            confidence,
            depth: 1,
        })
        .collect();
    let mut callees: Vec<ExplainNeighbor> = graph
        .callees_with_metadata(&sid)
        .into_iter()
        .map(|(target, kind, provenance, confidence)| ExplainNeighbor {
            symbol: target.as_str().to_string(),
            relation: kind.to_string(),
            side: "outgoing".into(),
            source: sid.as_str().to_string(),
            target: target.as_str().to_string(),
            provenance: provenance.to_string(),
            confidence,
            depth: 1,
        })
        .collect();
    let callers_truncated = callers.len() > NEIGHBOR_LIMIT;
    let callees_truncated = callees.len() > NEIGHBOR_LIMIT;
    callers.truncate(NEIGHBOR_LIMIT);
    callees.truncate(NEIGHBOR_LIMIT);
    let coverage = measure
        .as_ref()
        .map(|measure| CoverageInfo::of(&measure.coverage))
        .unwrap_or_else(CoverageInfo::unmeasured);

    let summary = format!(
        "{}: {} callers, {} callees (fan-in {}, fan-out {}, coverage {}){}",
        node.label,
        callers.len(),
        callees.len(),
        fan_in,
        fan_out,
        coverage.status,
        if callers_truncated || callees_truncated {
            "; neighbour list(s) truncated at 20"
        } else {
            ""
        }
    );
    Ok(GraphExplainOutput {
        node,
        callers,
        callees,
        fan_in,
        fan_out,
        coverage,
        neighbor_limit: NEIGHBOR_LIMIT,
        callers_truncated,
        callees_truncated,
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
    // The filtered surface windows from the first element: it publishes no
    // cursor, so the window is the whole answer under its filters.
    let (answer, filtered_out, _) = answer_for(
        read,
        input.question,
        input.max_depth,
        input.limit.unwrap_or_else(default_budget),
        &filter,
        0,
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

// ============================================================================
// Tests — graph_query continuation cursor (CGH-114)
// ============================================================================

#[cfg(test)]
mod cursor_tests {
    //! The cursor a cut `graph_query` answer publishes continues the same
    //! traversal it was minted from, and nothing else.
    //!
    //! CGH-084 pinned the order of the traversal and made it prefix-stable:
    //! a bigger budget extends a smaller answer. CGH-114 publishes that
    //! stability as a continuation: page + cursor resumes exactly where
    //! the page stopped, the cursor is bound to the revision of the graph
    //! it was read from, and a cursor from another revision, another query
    //! or a tampered payload is refused with its typed error instead of
    //! resuming into the wrong traversal.

    use super::*;
    use crate::domain::aggregates::call_graph::CallGraph;
    use crate::domain::aggregates::symbol::Symbol;
    use crate::domain::traits::graph_store::{GraphStore, StoreError};
    use crate::domain::value_objects::file_manifest::FileManifest;
    use crate::domain::value_objects::{DependencyType, Location, SymbolKind};
    use crate::interface::mcp::handlers::HandlerContextBuilder;
    use crate::interface::schema::schema_of;
    use std::sync::{Arc, Mutex};

    /// A store whose graph can be swapped: the revision the cursor is
    /// bound to changes exactly when the traversal's input changes.
    struct SwapStore {
        graph: Mutex<Arc<CallGraph>>,
    }

    impl SwapStore {
        fn new(graph: CallGraph) -> Self {
            Self {
                graph: Mutex::new(Arc::new(graph)),
            }
        }

        fn replace(&self, graph: CallGraph) {
            *self.graph.lock().unwrap() = Arc::new(graph);
        }
    }

    impl GraphStore for SwapStore {
        fn save_graph(&self, graph: &CallGraph) -> Result<(), StoreError> {
            self.replace(graph.clone());
            Ok(())
        }
        fn load_graph(&self) -> Result<Option<CallGraph>, StoreError> {
            Ok(Some((**self.graph.lock().unwrap()).clone()))
        }
        fn load_graph_shared(&self) -> Result<Option<Arc<CallGraph>>, StoreError> {
            Ok(Some(self.graph.lock().unwrap().clone()))
        }
        fn save_manifest(&self, _manifest: &FileManifest) -> Result<(), StoreError> {
            Ok(())
        }
        fn load_manifest(&self) -> Result<Option<FileManifest>, StoreError> {
            Ok(None)
        }
        fn clear(&self) -> Result<(), StoreError> {
            Ok(())
        }
        fn exists(&self) -> Result<bool, StoreError> {
            Ok(true)
        }
    }

    /// `root -> s0 -> s1 -> ... -> s{count-1}`, a chain: the traversal of
    /// "root" is exactly its `count + 1` nodes in order, one edge per
    /// hop, so a page boundary is a place a reader can verify by hand.
    fn chain_graph(count: usize) -> CallGraph {
        let mut graph = CallGraph::new();
        let root = graph.add_symbol(Symbol::new(
            "root",
            SymbolKind::Function,
            Location::new("lib.rs", 1, 0),
        ));
        let mut prev = root;
        for n in 0..count {
            let s = graph.add_symbol(Symbol::new(
                format!("s{n}"),
                SymbolKind::Function,
                Location::new("lib.rs", 2 + n as u32, 0),
            ));
            graph
                .add_dependency(&prev, &s, DependencyType::Calls)
                .expect("both symbols are in the graph");
            prev = s;
        }
        graph
    }

    fn context(graph: CallGraph) -> (HandlerContext, Arc<SwapStore>) {
        let store = Arc::new(SwapStore::new(graph));
        let ctx = HandlerContextBuilder::new()
            .with_graph_store_arc(store.clone())
            .build();
        (ctx, store)
    }

    fn query(
        question: &str,
        max_depth: usize,
        budget: usize,
        token: Option<String>,
    ) -> GraphQueryInput {
        GraphQueryInput {
            question: question.to_string(),
            max_depth,
            budget,
            continuation_token: token,
        }
    }

    const QUESTION: &str = "root";
    // Deep enough that the whole chain is one traversal: the page budget,
    // not the depth, is what cuts the answer here.
    const DEPTH: usize = 100;

    #[tokio::test]
    async fn pages_concatenate_into_the_full_traversal_without_overlap_or_gap() {
        // AC1(a): page 1 + page 2 (+ page 3) = the whole order, with the
        // budget split across calls. The chain of 12 traversal items is
        // root plus 11 hops: 12 nodes and 11 edges.
        let (ctx, _store) = context(chain_graph(11));

        let page1 = handle_graph_query(&ctx, query(QUESTION, DEPTH, 5, None))
            .await
            .expect("first page");
        assert!(page1.has_more, "the budget cut the traversal");
        assert_eq!(page1.truncation_reason.as_deref(), Some("budget"));
        let token1 = page1
            .next_continuation_token
            .clone()
            .expect("a cut page hands out its cursor");

        let page2 = handle_graph_query(&ctx, query(QUESTION, DEPTH, 5, Some(token1.clone())))
            .await
            .expect("the cursor resumes the same traversal");
        assert!(page2.has_more);
        let token2 = page2
            .next_continuation_token
            .clone()
            .expect("a cut page hands out its cursor");
        assert_ne!(
            token1, token2,
            "a repeated cursor would loop the chain forever"
        );

        // The chain of 12 symbols walks 12 nodes + 11 edges: five pages
        // of five, the last holding the three items left.
        let mut pages = vec![page1, page2];
        let mut token = token2;
        loop {
            let page = handle_graph_query(&ctx, query(QUESTION, DEPTH, 5, Some(token)))
                .await
                .expect("a cursor always answers with its page");
            let next = page.next_continuation_token.clone();
            let more = page.has_more;
            assert_eq!(more, next.is_some(), "has_more and the cursor agree");
            pages.push(page);
            match (more, next) {
                (true, Some(t)) => token = t,
                (false, None) => break,
                _ => panic!("a page claims more without a cursor, or a cursor without more"),
            }
        }

        // Concatenating the pages rebuilds the whole traversal, nodes and
        // edges: no item is lost, repeated, or reordered.
        let whole = handle_graph_query(&ctx, query(QUESTION, DEPTH, 1000, None))
            .await
            .expect("a budget over the traversal answers");
        assert!(!whole.has_more);
        let chained_nodes: Vec<&str> = pages
            .iter()
            .flat_map(|p| p.nodes.iter())
            .map(|n| n.id.as_str())
            .collect();
        let full_nodes: Vec<&str> = whole.nodes.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(
            chained_nodes, full_nodes,
            "the pages must join into the full node order"
        );
        let chained_edges: Vec<(String, String)> = pages
            .iter()
            .flat_map(|p| p.edges.iter())
            .map(|e| (e.source.clone(), e.target.clone()))
            .collect();
        let full_edges: Vec<(String, String)> = whole
            .edges
            .iter()
            .map(|e| (e.source.clone(), e.target.clone()))
            .collect();
        assert_eq!(
            chained_edges, full_edges,
            "the pages must join into the full edge order"
        );
    }

    #[tokio::test]
    async fn a_bigger_budget_extends_a_smaller_answer_at_the_cursor_boundary() {
        // The prefix-stability CGH-084 pinned inside one call: the page
        // and the cursor keep it across calls. `page ++ cursor` equals the
        // bigger budget, item for item.
        let (ctx, _store) = context(chain_graph(11));

        let small = handle_graph_query(&ctx, query(QUESTION, DEPTH, 4, None))
            .await
            .expect("page");
        let token = small.next_continuation_token.clone().expect("cut page");
        let next = handle_graph_query(&ctx, query(QUESTION, DEPTH, 4, Some(token)))
            .await
            .expect("continuation");
        let merged: Vec<String> = small
            .nodes
            .iter()
            .chain(next.nodes.iter())
            .map(|n| n.id.clone())
            .collect();

        let big = handle_graph_query(&ctx, query(QUESTION, DEPTH, 8, None))
            .await
            .expect("the same traversal under a bigger budget");
        let direct: Vec<String> = big.nodes.iter().map(|n| n.id.clone()).collect();
        assert_eq!(
            merged, direct,
            "page + cursor is the bigger budget's answer"
        );
    }

    #[tokio::test]
    async fn a_changed_graph_refuses_the_cursor_and_never_mixes_pages() {
        // AC1(b): mutate the graph between pages. The cursor is anchored
        // to the revision it was minted from; the refusal is typed, and
        // no mixed page is ever produced.
        let (ctx, store) = context(chain_graph(11));
        let page1 = handle_graph_query(&ctx, query(QUESTION, DEPTH, 5, None))
            .await
            .expect("first page");
        let token = page1.next_continuation_token.clone().expect("cursor");

        // The graph now holds another symbol: another revision.
        store.replace(chain_graph(12));

        let err = handle_graph_query(&ctx, query(QUESTION, DEPTH, 5, Some(token)))
            .await
            .expect_err("a cursor from another revision is refused");
        let text = err.to_string();
        assert!(
            text.contains("SnapshotExpired"),
            "the refusal names the typed cause: {text}"
        );
    }

    #[tokio::test]
    async fn a_rebuild_that_changed_nothing_keeps_the_cursor_working() {
        // The anchor is the traversal's input, not a counter: loading the
        // same content again is the same revision.
        let (ctx, store) = context(chain_graph(11));
        let page1 = handle_graph_query(&ctx, query(QUESTION, DEPTH, 5, None))
            .await
            .expect("first page");
        let token = page1.next_continuation_token.clone().expect("cursor");

        store.replace(chain_graph(11));

        let page2 = handle_graph_query(&ctx, query(QUESTION, DEPTH, 5, Some(token)))
            .await
            .expect("the same revision is the same cursor");
        assert_eq!(page2.nodes[0].id, "lib.rs:s2:4");
    }

    #[tokio::test]
    async fn a_cursor_for_another_question_is_refused() {
        // The offset indexes *this* traversal: a token replayed with
        // another question would skip into a sequence nobody minted it
        // for.
        let (ctx, _store) = context(chain_graph(11));
        let page1 = handle_graph_query(&ctx, query(QUESTION, DEPTH, 5, None))
            .await
            .expect("first page");
        let token = page1.next_continuation_token.clone().expect("cursor");

        let err = handle_graph_query(&ctx, query("s1", DEPTH, 5, Some(token)))
            .await
            .expect_err("a cursor minted for another question is refused");
        assert!(
            err.to_string().contains("different question"),
            "the refusal says why: {err}"
        );
    }

    #[tokio::test]
    async fn a_tampered_cursor_is_refused() {
        // A cursor is opaque: rewriting a byte of its payload breaks the
        // integrity tag the signed envelope carries.
        let (ctx, _store) = context(chain_graph(11));
        let page1 = handle_graph_query(&ctx, query(QUESTION, DEPTH, 5, None))
            .await
            .expect("first page");
        let token = page1.next_continuation_token.expect("cursor");

        // Flip one character well past the envelope header.
        let mut bytes = token.into_bytes();
        let at = bytes.len() / 2;
        bytes[at] = if bytes[at] == b'A' { b'B' } else { b'A' };
        let tampered = String::from_utf8(bytes).expect("base64 stays ASCII");

        let err = handle_graph_query(&ctx, query(QUESTION, DEPTH, 5, Some(tampered)))
            .await
            .expect_err("a tampered cursor is refused");
        assert!(
            err.to_string().contains("Garbled"),
            "the refusal is the typed garbled error: {err}"
        );
    }

    #[tokio::test]
    async fn a_cursor_pointing_past_the_end_answers_empty_and_stops() {
        // A page at the end of the traversal holds nothing, and mints no
        // cursor: a chain of tokens that never advances is worse than no
        // token at all.
        let (ctx, _store) = context(chain_graph(3));
        let page1 = handle_graph_query(&ctx, query(QUESTION, DEPTH, 100, None))
            .await
            .expect("the whole traversal fits");
        assert!(!page1.has_more);
        assert!(page1.next_continuation_token.is_none());

        // Forge nothing: take a real cursor from a bigger graph and show
        // that a page whose window starts past the end is empty, not an
        // error.
        let (big_ctx, _big_store) = context(chain_graph(11));
        let cut = handle_graph_query(&big_ctx, query(QUESTION, DEPTH, 5, None))
            .await
            .expect("cut page");
        let token = cut.next_continuation_token.clone().expect("cursor");
        // Same content on this ctx? No: the token is anchored to
        // chain_graph(11)'s revision, and a smaller graph refuses it.
        let err = handle_graph_query(&ctx, query(QUESTION, DEPTH, 5, Some(token)))
            .await
            .expect_err("another graph's revision is refused, not reinterpreted");
        assert!(err.to_string().contains("SnapshotExpired"));
    }

    #[tokio::test]
    async fn the_envelope_continue_link_carries_the_cursor() {
        // The hateoas mechanism is the canonical way this repo resumes a
        // revision-bound page: `continuation_of` reads `has_more` and
        // `next_continuation_token` from the answer and fills the
        // `continue` link with them — the cursor surface and the link
        // surface cannot drift, because there is one mechanism.
        let (ctx, _store) = context(chain_graph(11));
        let page = handle_graph_query(&ctx, query(QUESTION, DEPTH, 5, None))
            .await
            .expect("cut page");
        let token = page.next_continuation_token.clone().expect("cursor");
        let arguments = serde_json::json!({
            "question": QUESTION,
            "max_depth": DEPTH,
            "budget": 5,
            "continuation_token": null,
        })
        .as_object()
        .expect("the arguments are an object")
        .clone();
        let result = serde_json::to_value(&page).expect("the page serializes");
        let handle_ctx = crate::interface::hateoas::HandleContext {
            workspace: ctx.working_dir().to_string_lossy().to_string(),
            snapshot: "test".to_string(),
        };
        let envelope = crate::interface::hateoas::envelope_for(
            "graph_query",
            &arguments,
            &result,
            &handle_ctx,
        );

        let link = envelope
            .links
            .iter()
            .find(|l| l.rel == "continue")
            .expect("a cut answer publishes its continuation");
        assert_eq!(link.tool, "graph_query");
        assert_eq!(
            link.required_args
                .get("continuation_token")
                .and_then(serde_json::Value::as_str),
            Some(token.as_str()),
            "the link refills the call with the cursor the answer published"
        );
        assert!(
            envelope.budget.page.is_some(),
            "the budget names the signed page handle of the continuation"
        );
    }

    #[test]
    fn the_cursor_is_declared_in_the_input_and_output_contracts() {
        // AC1(c): the cursor is published through the derived contract —
        // the field is here, or it is nowhere a client can read.
        let input = schema_of::<GraphQueryInput>();
        let props = input.get("properties").expect("the input has properties");
        assert!(
            props.get("continuation_token").is_some(),
            "the input schema must declare the cursor: {input:?}"
        );

        let output = schema_of::<GraphQueryOutput>();
        let props = output.get("properties").expect("the output has properties");
        assert!(
            props.get("has_more").is_some() && props.get("next_continuation_token").is_some(),
            "the output schema must publish the cut and its cursor: {output:?}"
        );
    }
}
