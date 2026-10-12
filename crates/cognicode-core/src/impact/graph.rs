//! B4.W1 — Impact graph with evidence and provenance.
//!
//! Estado pre-B4.W1: las 6 tools de impacto emitían `weight`/
//! `confidence` pero sin discriminador de dirección ni procedencia.
//! Un grafo parcial podía leerse como cobertura completa (R-B4-W1).
//!
//! Política B4.W1:
//!   - Todo edge lleva `direction` y `provenance`. El módulo NO
//!     implementa `Default` para `Direction` ni para `Provenance`,
//!     así un edge sin esos campos NO compila.
//!   - `coverage` ∈ [0.0, 1.0]; un path truncated lleva
//!     `complete=false` Y `coverage < 1.0`.

use serde::{Deserialize, Serialize};

use crate::interface::symbol_identity::SymbolId;

/// Direction of a dependency edge.
///
/// `CallerToCallee`: A references/uses B (A is impacted by changes to B).
/// `CalleeToCaller`: A is the consumer of B (B is impacted by changes to A).
/// `Bidirectional`:  A and B reference each other.
/// `Undirected`: no orientation is meaningful.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    CallerToCallee,
    CalleeToCaller,
    Bidirectional,
    Undirected,
}

impl Direction {
    pub fn as_str(&self) -> &'static str {
        match self {
            Direction::CallerToCallee => "caller_to_callee",
            Direction::CalleeToCaller => "callee_to_caller",
            Direction::Bidirectional => "bidirectional",
            Direction::Undirected => "undirected",
        }
    }
}

/// Provenance of an edge.
///
/// `Observed`: from a concrete code construct (LSP, parser).
/// `Inferred`: from a graph projection grounded in observed facts.
/// `Heuristic`: from a heuristic algorithm — MUST NOT be presented
///              with the same authority as `Observed` (AAT-B4-03).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum Provenance {
    Observed,
    Inferred,
    Heuristic,
}

impl Provenance {
    pub fn as_str(&self) -> &'static str {
        match self {
            Provenance::Observed => "observed",
            Provenance::Inferred => "inferred",
            Provenance::Heuristic => "heuristic",
        }
    }
}

// No `impl Default` for Direction/Provenance — a forgotten field
// must NOT silently take a default.

/// Single edge in the impact graph. Reuses `SymbolId` from B3.W1.
/// `weight` is confidence/cost in `[0.0, 1.0]` (lower = stronger).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ImpactEdge {
    pub from: SymbolId,
    pub to: SymbolId,
    pub direction: Direction,
    pub provenance: Provenance,
    pub weight: f64,
    pub evidence: Option<String>,
}

/// A traversable path. `complete=false` means the BFS/DFS was
/// stopped before reaching the requested target — consumers MUST
/// treat it as a partial-coverage signal (AAT-B4-01).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ImpactPath {
    pub edges: Vec<ImpactEdge>,
    pub complete: bool,
    pub coverage: f32,
}

/// Envelope returned by the 6 impact tools.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ImpactResult {
    pub paths: Vec<ImpactPath>,
    pub truncated: bool,
    pub total_visited: usize,
}

/// Declarative budget for the impact tools.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImpactLimits {
    pub max_depth: usize,
    pub max_nodes: usize,
    pub direction: Direction,
}

impl Default for ImpactLimits {
    fn default() -> Self {
        Self {
            max_depth: 10,
            max_nodes: 1500,
            direction: Direction::CallerToCallee,
        }
    }
}

/// Build an `ImpactResult` from a stream of edges under the given
/// limits. O(n).
pub fn build_impact_result(
    edges: impl IntoIterator<Item = ImpactEdge>,
    limits: &ImpactLimits,
) -> ImpactResult {
    let mut collected: Vec<ImpactEdge> = edges.into_iter().collect();

    let mut visited_set: std::collections::HashSet<(String, String)> =
        std::collections::HashSet::new();
    for edge in &collected {
        visited_set.insert((
            edge.from.qualified_name.clone(),
            edge.from.file_path.to_string_lossy().into_owned(),
        ));
        visited_set.insert((
            edge.to.qualified_name.clone(),
            edge.to.file_path.to_string_lossy().into_owned(),
        ));
    }
    let total_visited = visited_set.len();

    let max_nodes = limits.max_nodes.max(1);
    let max_depth = limits.max_depth.max(1);
    let truncated = total_visited > max_nodes;

    if collected.is_empty() {
        return ImpactResult {
            paths: Vec::new(),
            truncated: false,
            total_visited: 0,
        };
    }

    let path_complete = !truncated && collected.len() <= max_depth;
    let coverage = if truncated {
        (max_nodes as f32 / total_visited.max(1) as f32).clamp(0.0, 1.0)
    } else if collected.len() > max_depth {
        (max_depth as f32 / collected.len().max(1) as f32).clamp(0.0, 1.0)
    } else {
        1.0
    };

    if collected.len() > max_depth {
        collected.truncate(max_depth);
    }

    ImpactResult {
        paths: vec![ImpactPath {
            edges: collected,
            complete: path_complete,
            coverage,
        }],
        truncated,
        total_visited,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn sym(name: &str) -> SymbolId {
        SymbolId::from_lsp_location(
            name.to_string(),
            PathBuf::from("src/lib.rs"),
            0..10,
        )
    }

    fn edge(from: &str, to: &str, dir: Direction, prov: Provenance) -> ImpactEdge {
        ImpactEdge {
            from: sym(from),
            to: sym(to),
            direction: dir,
            provenance: prov,
            weight: 1.0,
            evidence: None,
        }
    }

    #[test]
    fn direction_inversion_via_serde() {
        let original = Direction::CallerToCallee;
        let json = serde_json::to_string(&original).unwrap();
        assert_eq!(json, "\"caller_to_callee\"");
        let back: Direction = serde_json::from_str(&json).unwrap();
        assert_eq!(back, original);
        assert_ne!(Direction::CallerToCallee, Direction::CalleeToCaller);
    }

    #[test]
    fn partial_coverage_flag() {
        let mut edges: Vec<ImpactEdge> = Vec::new();
        for i in 0..6 {
            let from = format!("n{i}");
            let to = format!("n{i}_dep");
            edges.push(edge(&from, &to, Direction::CallerToCallee, Provenance::Observed));
        }
        let limits = ImpactLimits { max_depth: 100, max_nodes: 2, direction: Direction::CallerToCallee };
        let result = build_impact_result(edges, &limits);
        assert!(result.truncated);
        let path = result.paths.first().expect("at least one path");
        assert!(!path.complete);
        assert!(path.coverage < 1.0);
    }

    #[test]
    fn truncated_flag_only_when_node_limit_hit() {
        let mut edges: Vec<ImpactEdge> = Vec::new();
        for i in 0..20 {
            let from = format!("n{i}");
            let to = format!("n{i}_dep");
            edges.push(edge(&from, &to, Direction::CallerToCallee, Provenance::Observed));
        }
        let limits = ImpactLimits { max_depth: 3, max_nodes: 100, direction: Direction::CallerToCallee };
        let result = build_impact_result(edges, &limits);
        assert!(!result.truncated);
        let path = result.paths.first().expect("at least one path");
        assert_eq!(path.edges.len(), 3);
        assert!(!path.complete);
    }

    #[test]
    fn depth_limit_honored_for_path_lengths() {
        let chain: Vec<ImpactEdge> = (0..5)
            .map(|i| {
                edge(
                    &format!("n{i}"),
                    &format!("n{}", i + 1),
                    Direction::CallerToCallee,
                    Provenance::Observed,
                )
            })
            .collect();
        let limits = ImpactLimits { max_depth: 2, max_nodes: 100, direction: Direction::CallerToCallee };
        let result = build_impact_result(chain, &limits);
        for path in &result.paths {
            assert!(path.edges.len() <= 2);
        }
    }

    #[test]
    fn provenance_discriminator_travels_on_edges() {
        let edge = edge("a", "b", Direction::CallerToCallee, Provenance::Heuristic);
        let limits = ImpactLimits { max_depth: 10, max_nodes: 100, direction: Direction::CallerToCallee };
        let result = build_impact_result(vec![edge], &limits);
        let serialized = serde_json::to_string(&result).unwrap();
        assert!(serialized.contains("\"provenance\":\"heuristic\""));
        assert!(!serialized.contains("\"provenance\":\"observed\""));
    }

    #[test]
    fn edge_weight_ordering_preserved() {
        let edges = vec![
            ImpactEdge { from: sym("a"), to: sym("b"), direction: Direction::CallerToCallee, provenance: Provenance::Observed, weight: 0.1, evidence: None },
            ImpactEdge { from: sym("b"), to: sym("c"), direction: Direction::CallerToCallee, provenance: Provenance::Observed, weight: 0.5, evidence: None },
            ImpactEdge { from: sym("c"), to: sym("d"), direction: Direction::CallerToCallee, provenance: Provenance::Observed, weight: 0.9, evidence: None },
        ];
        let limits = ImpactLimits { max_depth: 100, max_nodes: 100, direction: Direction::CallerToCallee };
        let result = build_impact_result(edges, &limits);
        let path = result.paths.first().unwrap();
        let weights: Vec<f64> = path.edges.iter().map(|e| e.weight).collect();
        assert_eq!(weights, vec![0.1, 0.5, 0.9]);
    }
}
