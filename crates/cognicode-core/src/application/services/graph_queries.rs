//! The graph questions, answered once.
//!
//! ## Why this module exists
//!
//! The CLI and the MCP are adapters over one service. When each surface
//! answers a graph question on its own, the two answers drift apart, and
//! the reader gets whichever one it happened to ask:
//!
//! - the CLI cut the list of hot paths to the limit **before** it applied
//!   the fan-in threshold, so a project with twenty functions called by
//!   three others published one of them;
//! - the CLI asked the trace for a fully qualified name and answered
//!   "not found" for the simple name the MCP answers;
//! - the CLI cut the entry points and the leaf functions to twenty
//!   without saying so;
//! - the CLI's hierarchy walked a graph of its own, built by another
//!   strategy, and the MCP's hierarchy walked the project graph;
//! - the mermaid diagram had three renderers, and the two surfaces that
//!   publish one published different documents.
//!
//! So the questions live here, and both surfaces ask them here. What
//! belongs here is the *semantics* of a question: which symbols answer
//! it, in which order, and what a refusal looks like. What each surface
//! *shows* — a table, a JSON document, a diagram — belongs to the
//! surface.
//!
//! ## The rule for names
//!
//! A name is resolved by the domain's rule (`resolve_target`): a name one
//! symbol carries resolves to it, a name several carry is ambiguous and
//! says which ones, and a name none carries is not found. The application
//! layer does not pick between homonyms: it cannot prove which one was
//! meant, and an answer through a symbol nobody named is worse than no
//! answer.

use std::collections::VecDeque;

use crate::application::services::analysis_service::AnalysisService;
use crate::domain::aggregates::Symbol;
use crate::domain::aggregates::call_graph::{CallGraph, MermaidOptions, SymbolId};
use crate::domain::services::resolve_target;
use crate::domain::value_objects::TargetResolution;

/// Which way a hierarchy question walks the graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// The symbols that call this one.
    Callers,
    /// The symbols this one calls.
    Callees,
    /// Both sides.
    Both,
}

impl Direction {
    /// The name a surface prints for this direction.
    pub fn as_str(&self) -> &'static str {
        match self {
            Direction::Callers => "callers",
            Direction::Callees => "callees",
            Direction::Both => "both",
        }
    }
}

/// One row of the hot-paths answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HotPath {
    pub name: String,
    pub file: String,
    pub line: u32,
    pub column: u32,
    pub fan_in: usize,
    pub fan_out: usize,
}

/// One entry of a hierarchy answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HierarchyEntry {
    pub name: String,
    pub file: String,
    pub line: u32,
    pub column: u32,
    /// How far from the root the entry is. 1 is a direct neighbour.
    pub depth: u32,
    /// The side the entry was reached from.
    pub direction: Direction,
}

/// A diagram, and what it holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MermaidDiagram {
    /// The diagram itself.
    pub code: String,
    /// The nodes the diagram declares.
    pub node_count: usize,
    /// The edges the diagram declares.
    pub edge_count: usize,
}

/// The name a question could not be answered for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphQueryError {
    /// Several symbols carry the name: the question has no single answer
    /// until it says which one it means.
    Ambiguous {
        name: String,
        candidates: Vec<String>,
    },
    /// No symbol carries the name.
    NotFound { name: String },
}

impl std::fmt::Display for GraphQueryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GraphQueryError::Ambiguous { name, candidates } => {
                write!(f, "Symbol '{name}' is ambiguous: {}", candidates.join(", "))
            }
            GraphQueryError::NotFound { name } => write!(
                f,
                "Symbol '{name}' is not in the project graph; its result is unknown."
            ),
        }
    }
}

impl std::error::Error for GraphQueryError {}

/// The symbols with the most callers.
///
/// The threshold is applied **before** the cut. A project with twenty
/// functions called by three others and one called by nobody answers with
/// the twenty: cutting first and filtering afterwards publishes the one
/// that survived a limit that ran before the question was asked.
///
/// The answer is ordered by fan-in, and the order of equal fan-ins is
/// decided by the name and the file, so two runs over the same project
/// publish the same table.
pub fn hot_paths(graph: &CallGraph, limit: usize, min_fan_in: usize) -> Vec<HotPath> {
    let mut rows: Vec<HotPath> = graph
        .symbols()
        .filter_map(|symbol| {
            let id = SymbolId::new(symbol.fully_qualified_name());
            let fan_in = graph.callers(&id).len();
            let fan_out = graph.callees(&id).len();
            if fan_in < min_fan_in {
                return None;
            }
            Some(HotPath {
                name: symbol.name().to_string(),
                file: symbol.location().file().to_string(),
                line: symbol.location().line(),
                column: symbol.location().column(),
                fan_in,
                fan_out,
            })
        })
        .collect();

    rows.sort_by(|a, b| {
        b.fan_in
            .cmp(&a.fan_in)
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| a.file.cmp(&b.file))
            .then_with(|| a.line.cmp(&b.line))
    });
    rows.truncate(limit);
    rows
}

/// The symbols nothing calls: the entry points.
///
/// Every one of them. A list that stops at twenty without saying so is
/// a list that lies about its own length.
pub fn entry_points(graph: &CallGraph) -> Vec<Symbol> {
    graph
        .roots()
        .iter()
        .filter_map(|id| graph.get_symbol(id).cloned())
        .collect()
}

/// The symbols that call nothing: the leaf functions.
pub fn leaf_functions(graph: &CallGraph) -> Vec<Symbol> {
    graph
        .leaves()
        .iter()
        .filter_map(|id| graph.get_symbol(id).cloned())
        .collect()
}

/// The symbol a name means, or the refusal to guess.
///
/// The rule is the domain's (`resolve_target`), so this surface and every
/// other one answer the question of a name the same way.
pub fn resolve(graph: &CallGraph, name: &str) -> Result<SymbolId, GraphQueryError> {
    match resolve_target(graph, name) {
        TargetResolution::Resolved(id) => Ok(id),
        TargetResolution::Ambiguous { name, candidates } => Err(GraphQueryError::Ambiguous {
            name,
            candidates: candidates
                .iter()
                .map(|candidate| candidate.as_str().to_string())
                .collect(),
        }),
        TargetResolution::Unresolved(_) => Err(GraphQueryError::NotFound {
            name: name.to_string(),
        }),
    }
}

/// The call path from one name to another, if there is one.
///
/// `max_depth` of zero means no limit. The path is the shortest one the
/// graph holds, and it names the symbols by the name the reader gave.
pub fn trace_path(
    graph: &CallGraph,
    from: &str,
    to: &str,
    max_depth: usize,
) -> Result<Option<Vec<Symbol>>, GraphQueryError> {
    let source = resolve(graph, from)?;
    let target = resolve(graph, to)?;
    let path = shortest_path(graph, &source, &target, max_depth);
    Ok(path.map(|ids| {
        ids.iter()
            .filter_map(|id| graph.get_symbol(id).cloned())
            .collect()
    }))
}

/// Every call path from one name to another, up to `max_depth`.
///
/// A symbol is visited at most three times on the way, so a graph with a
/// cycle does not answer with an endless number of paths.
pub fn trace_paths(
    graph: &CallGraph,
    from: &str,
    to: &str,
    max_depth: usize,
) -> Result<Vec<Vec<Symbol>>, GraphQueryError> {
    let source = resolve(graph, from)?;
    let target = resolve(graph, to)?;
    let paths = all_paths(graph, &source, &target, max_depth);
    Ok(paths
        .into_iter()
        .map(|ids| {
            ids.iter()
                .filter_map(|id| graph.get_symbol(id).cloned())
                .collect()
        })
        .collect())
}

/// What a symbol reaches, and what reaches it.
///
/// The walk is breadth-first from the symbol the name means, up to
/// `depth` hops. The root itself is not an entry: the answer is about
/// the neighbours, not about the symbol that was asked about.
pub fn hierarchy(
    graph: &CallGraph,
    symbol: &str,
    depth: u32,
    direction: Direction,
) -> Result<Vec<HierarchyEntry>, GraphQueryError> {
    let root = resolve(graph, symbol)?;
    let mut entries: Vec<HierarchyEntry> = Vec::new();
    let mut visited: std::collections::HashSet<SymbolId> = std::collections::HashSet::new();
    visited.insert(root.clone());

    let mut frontier: Vec<SymbolId> = vec![root];
    for level in 1..=depth {
        let mut next: Vec<SymbolId> = Vec::new();
        for symbol_id in &frontier {
            if direction != Direction::Callees {
                for caller_id in graph.callers(symbol_id) {
                    if visited.insert(caller_id.clone())
                        && let Some(caller) = graph.get_symbol(&caller_id)
                    {
                        entries.push(hierarchy_entry(caller, level, Direction::Callers));
                        next.push(caller_id);
                    }
                }
            }
            if direction != Direction::Callers {
                for (callee_id, _) in graph.callees(symbol_id) {
                    if visited.insert(callee_id.clone())
                        && let Some(callee) = graph.get_symbol(&callee_id)
                    {
                        entries.push(hierarchy_entry(callee, level, Direction::Callees));
                        next.push(callee_id);
                    }
                }
            }
        }
        if next.is_empty() {
            break;
        }
        frontier = next;
    }

    entries.sort_by(|a, b| {
        a.depth
            .cmp(&b.depth)
            .then_with(|| a.direction.as_str().cmp(b.direction.as_str()))
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| a.file.cmp(&b.file))
            .then_with(|| a.line.cmp(&b.line))
    });
    Ok(entries)
}

/// The project as a Mermaid diagram.
///
/// There is one renderer and this is it: the diagram a surface publishes
/// is the one the graph renders, so two surfaces over the same project
/// publish the same document.
pub fn mermaid(graph: &CallGraph, options: &MermaidOptions) -> MermaidDiagram {
    let code = graph.to_mermaid_with_options("Call Graph", options);
    let node_count = code
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            trimmed.contains('[') && trimmed.ends_with(']')
        })
        .count();
    let edge_count = code.lines().filter(|line| line.contains(" --> ")).count();
    MermaidDiagram {
        code,
        node_count,
        edge_count,
    }
}

/// The shortest call path between two symbols, up to `max_depth`.
///
/// `max_depth` of zero means no limit.
pub fn trace_path_between(
    graph: &CallGraph,
    source: &SymbolId,
    target: &SymbolId,
    max_depth: usize,
) -> Option<Vec<Symbol>> {
    shortest_path(graph, source, target, max_depth).map(|ids| {
        ids.iter()
            .filter_map(|id| graph.get_symbol(id).cloned())
            .collect()
    })
}

/// Every simple call path between two symbols, up to `max_depth`.
pub fn trace_paths_between(
    graph: &CallGraph,
    source: &SymbolId,
    target: &SymbolId,
    max_depth: usize,
) -> Vec<Vec<Symbol>> {
    all_paths(graph, source, target, max_depth)
        .into_iter()
        .map(|ids| {
            ids.iter()
                .filter_map(|id| graph.get_symbol(id).cloned())
                .collect()
        })
        .collect()
}

/// The shortest call path between two symbols, up to `max_depth`.
///
/// `max_depth` of zero means no limit.
fn shortest_path(
    graph: &CallGraph,
    source: &SymbolId,
    target: &SymbolId,
    max_depth: usize,
) -> Option<Vec<SymbolId>> {
    let mut queue: VecDeque<(SymbolId, Vec<SymbolId>)> = VecDeque::new();
    let mut visited: std::collections::HashSet<SymbolId> = std::collections::HashSet::new();

    queue.push_back((source.clone(), vec![source.clone()]));
    visited.insert(source.clone());

    while let Some((current, path)) = queue.pop_front() {
        if current == *target {
            return Some(path);
        }
        if max_depth > 0 && path.len() >= max_depth {
            continue;
        }
        for (callee_id, _) in graph.callees(&current) {
            if visited.insert(callee_id.clone()) {
                let mut new_path = path.clone();
                new_path.push(callee_id.clone());
                queue.push_back((callee_id.clone(), new_path));
            }
        }
    }

    None
}

/// Every simple call path between two symbols, up to `max_depth`.
///
/// A symbol is visited at most three times on the way: a graph with a
/// cycle would otherwise answer with an endless number of paths.
fn all_paths(
    graph: &CallGraph,
    source: &SymbolId,
    target: &SymbolId,
    max_depth: usize,
) -> Vec<Vec<SymbolId>> {
    let mut paths: Vec<Vec<SymbolId>> = Vec::new();
    let mut queue: VecDeque<(SymbolId, Vec<SymbolId>)> = VecDeque::new();
    let mut visits: std::collections::HashMap<SymbolId, usize> = std::collections::HashMap::new();

    queue.push_back((source.clone(), vec![source.clone()]));
    *visits.entry(source.clone()).or_insert(0) += 1;

    while let Some((current, path)) = queue.pop_front() {
        if current == *target {
            paths.push(path);
            continue;
        }
        if max_depth > 0 && path.len() >= max_depth {
            continue;
        }
        for (callee_id, _) in graph.callees(&current) {
            let count = *visits.get(&callee_id).unwrap_or(&0);
            if count < 3 {
                *visits.entry(callee_id.clone()).or_insert(0) += 1;
                let mut new_path = path.clone();
                new_path.push(callee_id.clone());
                queue.push_back((callee_id.clone(), new_path));
            }
        }
    }

    paths
}

fn hierarchy_entry(symbol: &Symbol, depth: u32, direction: Direction) -> HierarchyEntry {
    HierarchyEntry {
        name: symbol.name().to_string(),
        file: symbol.location().file().to_string(),
        line: symbol.location().line(),
        column: symbol.location().column(),
        depth,
        direction,
    }
}

/// The service the surfaces ask.
///
/// The CLI command and the MCP tool are adapters over one service, so the
/// question they put has to reach the same answer either way. These are
/// the doors: every surface that asks one of these questions comes in
/// here, and the answer is the one this module computes.
///
/// The functions above are the implementation; these are the name they
/// are asked by. Nothing else implements these questions.
impl AnalysisService {
    /// The symbols with the most callers.
    pub fn hot_paths(graph: &CallGraph, limit: usize, min_fan_in: usize) -> Vec<HotPath> {
        self::hot_paths(graph, limit, min_fan_in)
    }

    /// The symbols nothing calls: the entry points.
    pub fn entry_points(graph: &CallGraph) -> Vec<Symbol> {
        self::entry_points(graph)
    }

    /// The symbols that call nothing: the leaf functions.
    pub fn leaf_functions(graph: &CallGraph) -> Vec<Symbol> {
        self::leaf_functions(graph)
    }

    /// The symbol a name means, or the refusal to guess.
    pub fn resolve_symbol(graph: &CallGraph, name: &str) -> Result<SymbolId, GraphQueryError> {
        self::resolve(graph, name)
    }

    /// The call path from one name to another, if there is one.
    pub fn trace_path_by_name(
        graph: &CallGraph,
        from: &str,
        to: &str,
        max_depth: usize,
    ) -> Result<Option<Vec<Symbol>>, GraphQueryError> {
        self::trace_path(graph, from, to, max_depth)
    }

    /// Every call path from one name to another, up to `max_depth`.
    pub fn trace_paths_by_name(
        graph: &CallGraph,
        from: &str,
        to: &str,
        max_depth: usize,
    ) -> Result<Vec<Vec<Symbol>>, GraphQueryError> {
        self::trace_paths(graph, from, to, max_depth)
    }

    /// The shortest call path between two symbols, up to `max_depth`.
    pub fn trace_path_between(
        graph: &CallGraph,
        source: &SymbolId,
        target: &SymbolId,
        max_depth: usize,
    ) -> Option<Vec<Symbol>> {
        self::trace_path_between(graph, source, target, max_depth)
    }

    /// Every simple call path between two symbols, up to `max_depth`.
    pub fn trace_paths_between(
        graph: &CallGraph,
        source: &SymbolId,
        target: &SymbolId,
        max_depth: usize,
    ) -> Vec<Vec<Symbol>> {
        self::trace_paths_between(graph, source, target, max_depth)
    }

    /// What a symbol reaches, and what reaches it.
    pub fn call_hierarchy(
        graph: &CallGraph,
        symbol: &str,
        depth: u32,
        direction: Direction,
    ) -> Result<Vec<HierarchyEntry>, GraphQueryError> {
        self::hierarchy(graph, symbol, depth, direction)
    }

    /// The project as a Mermaid diagram.
    pub fn mermaid_diagram(graph: &CallGraph, options: &MermaidOptions) -> MermaidDiagram {
        self::mermaid(graph, options)
    }
}
