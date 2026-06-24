//! Impact DTO - Data Transfer Objects for impact analysis

use crate::domain::aggregates::call_graph::SymbolId;
use serde::{Deserialize, Serialize};

/// DTO for impact analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactDto {
    /// Symbol that was analyzed
    pub symbol_id: String,
    /// Number of symbols that would be impacted
    pub impacted_count: usize,
    /// List of impacted symbol IDs
    pub impacted_symbols: Vec<String>,
    /// Impact score (0-10)
    pub impact_score: u8,
    /// Risk level description
    pub risk_level: String,
}

impl ImpactDto {
    /// Creates a new ImpactDto
    pub fn new(symbol_id: impl Into<String>, impacted: usize, score: u8) -> Self {
        let risk_level = match score {
            0..=3 => "low",
            4..=6 => "medium",
            7..=9 => "high",
            _ => "critical",
        };

        Self {
            symbol_id: symbol_id.into(),
            impacted_count: impacted,
            impacted_symbols: Vec::new(),
            impact_score: score,
            risk_level: risk_level.to_string(),
        }
    }

    /// Sets the impacted symbols
    pub fn with_impacted_symbols(mut self, symbols: Vec<SymbolId>) -> Self {
        self.impacted_symbols = symbols.iter().map(|s| s.as_str().to_string()).collect();
        self
    }
}

/// DTO for cycle detection result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CycleDto {
    /// Whether cycles were detected
    pub has_cycles: bool,
    /// Number of cycles detected
    pub cycle_count: usize,
    /// List of cycles (each cycle is a list of symbol IDs)
    pub cycles: Vec<Vec<String>>,
}

impl CycleDto {
    /// Creates a new CycleDto
    pub fn new(has_cycles: bool, cycle_count: usize) -> Self {
        Self {
            has_cycles,
            cycle_count,
            cycles: Vec::new(),
        }
    }

    /// Sets the cycles
    pub fn with_cycles(mut self, cycles: Vec<Vec<SymbolId>>) -> Self {
        self.cycles = cycles
            .into_iter()
            .map(|c| c.into_iter().map(|s| s.as_str().to_string()).collect())
            .collect();
        self
    }
}

/// DTO for the result of a shortest-path query between two symbols.
///
/// `path` is the ordered list of `SymbolId`s along the cheapest confidence-
/// weighted route from `from` to `to`. `total_cost` is the sum of
/// `1.0 - confidence` along the path (so a high-confidence path has a low
/// cost). `found` is always `true` when the DTO was built through
/// [`PathResultDto::from_path`]; the field is preserved on the wire so
/// consumers can round-trip an "unreachable" answer as well.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathResultDto {
    /// Ordered list of symbol ids from source to target.
    pub path: Vec<String>,
    /// Sum of edge costs along the path. Always non-negative and finite
    /// for paths built through [`PathResultDto::from_path`].
    pub total_cost: f64,
    /// Whether a path was found. `from_path` always sets this to `true`.
    pub found: bool,
}

impl PathResultDto {
    /// Build a `PathResultDto` from a confirmed path and its cost.
    ///
    /// `SymbolId`s are converted to strings via [`SymbolId::as_str`], the
    /// project-wide convention used by the rest of the impact DTOs.
    pub fn from_path(path: Vec<SymbolId>, cost: f64) -> Self {
        Self {
            path: path.iter().map(|s| s.as_str().to_string()).collect(),
            total_cost: cost,
            found: true,
        }
    }
}

/// DTO for a strongly connected component (cycle group) found in the
/// call graph.
///
/// `members` is the set of symbols forming the SCC. The conversion keeps
/// the raw ordering returned by the underlying algorithm (Tarjan SCC).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SccDto {
    /// Symbols in the SCC, in the order returned by the algorithm.
    pub members: Vec<String>,
    /// Number of members. Equal to `members.len()` by construction.
    pub size: usize,
}

impl SccDto {
    /// Build an `SccDto` from the raw SCC members.
    ///
    /// `size` is computed from the converted `members` list so it stays
    /// in sync even if the input contained duplicates.
    pub fn from_scc(members: Vec<SymbolId>) -> Self {
        let strings: Vec<String> = members.iter().map(|s| s.as_str().to_string()).collect();
        let size = strings.len();
        Self {
            members: strings,
            size,
        }
    }
}

// ============================================================================
// mcp-graph-primitives DTOs — graph_subgraph, graph_cluster, graph_explain.
// ============================================================================

use crate::infrastructure::graph::{ExplanationView, SubgraphView};

/// DTO for a single edge in a [`SubgraphResultDto`].
///
/// Fields are all wire-friendly: the symbol ids are strings (preserving
/// the project convention) and the `dependency_type` is the canonical
/// `Display` form of the variant (e.g. `"calls"`, `"uses_generic"`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubgraphEdgeDto {
    /// Source symbol id.
    pub source: String,
    /// Target symbol id.
    pub target: String,
    /// Canonical `Display` form of the edge's [`DependencyType`].
    pub dependency_type: String,
    /// Sanitized edge confidence in `[0.0, 1.0]`.
    pub confidence: f64,
}

/// DTO for the result of a `graph_subgraph` query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubgraphResultDto {
    /// Symbol ids in BFS visit order (root first).
    pub nodes: Vec<String>,
    /// Edges traversed during the BFS, in visit order.
    pub edges: Vec<SubgraphEdgeDto>,
}

impl SubgraphResultDto {
    /// Build a `SubgraphResultDto` from a projection-level view.
    pub fn from_view(view: SubgraphView) -> Self {
        let nodes = view.nodes.iter().map(|s| s.as_str().to_string()).collect();
        let edges = view
            .edges
            .into_iter()
            .map(|e| SubgraphEdgeDto {
                source: e.source.as_str().to_string(),
                target: e.target.as_str().to_string(),
                dependency_type: e.dependency_type.to_string(),
                confidence: e.confidence,
            })
            .collect();
        Self { nodes, edges }
    }
}

/// DTO for a single cluster in a [`ClusterResultDto`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterDto {
    /// Symbols in the cluster, in the order returned by the algorithm.
    pub members: Vec<String>,
    /// Number of members. Equal to `members.len()` by construction.
    pub size: usize,
}

impl ClusterDto {
    /// Build a `ClusterDto` from a raw list of `SymbolId`s. `size` is
    /// computed from the converted list to stay in sync.
    pub fn from_members(members: Vec<SymbolId>) -> Self {
        let strings: Vec<String> = members.iter().map(|s| s.as_str().to_string()).collect();
        let size = strings.len();
        Self {
            members: strings,
            size,
        }
    }
}

/// DTO for the result of a `graph_cluster` query.
///
/// Wraps a `Vec<ClusterDto>` so the wire format is the inner vector
/// (the newtype adds no JSON noise because the field is the only
/// public one).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterResultDto(pub Vec<ClusterDto>);

impl ClusterResultDto {
    /// Build a `ClusterResultDto` from a list of strongly connected
    /// components (or undirected components).
    pub fn from_clusters(clusters: Vec<Vec<SymbolId>>) -> Self {
        Self(clusters.into_iter().map(ClusterDto::from_members).collect())
    }
}

/// DTO for a single hop on an `ExplainResultDto`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplainHopDto {
    /// Source symbol id of the hop.
    pub from: String,
    /// Target symbol id of the hop.
    pub to: String,
    /// Canonical `Display` form of the edge's [`DependencyType`].
    pub dependency_type: String,
    /// Sanitized edge confidence in `[0.0, 1.0]`.
    pub confidence: f64,
    /// Human-readable verb phrase (e.g. `"calls"`, `"inherits from"`).
    pub rationale: String,
}

/// DTO for the result of a `graph_explain` query.
///
/// `found` is `true` when the service located a path; `hops` carries
/// the per-hop metadata, `total_cost` is the sum of edge costs along
/// the path, and `summary` is a one-line description suitable for
/// agent consumption.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplainResultDto {
    /// Whether a path was found. `false` means "no path" (NOT a
    /// service error — the MCP tool returns `is_error=false` for
    /// `found=false`).
    pub found: bool,
    /// Per-hop metadata along the chosen path. Empty for self-paths
    /// and for `found=false`.
    pub hops: Vec<ExplainHopDto>,
    /// Sum of edge costs along the path. `0.0` for self-paths and
    /// for `found=false`.
    pub total_cost: f64,
    /// One-line human summary (e.g. `"3 hop(s)"`, `"no path"`).
    pub summary: String,
}

impl ExplainResultDto {
    /// Build an `ExplainResultDto` from a projection-level view and a
    /// `found` flag. `summary` is auto-derived from the hop count.
    pub fn from_view(view: &ExplanationView, found: bool) -> Self {
        let hops = view
            .hops
            .iter()
            .map(|h| ExplainHopDto {
                from: h.from.as_str().to_string(),
                to: h.to.as_str().to_string(),
                dependency_type: h.dependency_type.to_string(),
                confidence: h.confidence,
                rationale: h.rationale.clone(),
            })
            .collect();
        let summary = format!("{} hop(s)", view.hops.len());
        Self {
            found,
            hops,
            total_cost: view.total_cost,
            summary,
        }
    }

    /// Build the canonical "no path" answer. Service uses this to wrap
    /// the projection's `None` so MCP returns a structured payload
    /// (not an `is_error=true` result).
    pub fn not_found() -> Self {
        Self {
            found: false,
            hops: Vec::new(),
            total_cost: 0.0,
            summary: "no path".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::aggregates::call_graph::SymbolId;

    #[test]
    fn test_impact_dto_new_low_risk() {
        let dto = ImpactDto::new("symbol_1", 2, 2);
        assert_eq!(dto.symbol_id, "symbol_1");
        assert_eq!(dto.impacted_count, 2);
        assert_eq!(dto.impact_score, 2);
        assert_eq!(dto.risk_level, "low");
        assert!(dto.impacted_symbols.is_empty());
    }

    #[test]
    fn test_impact_dto_new_medium_risk() {
        let dto = ImpactDto::new("func_a", 5, 5);
        assert_eq!(dto.risk_level, "medium");
    }

    #[test]
    fn test_impact_dto_new_high_risk() {
        let dto = ImpactDto::new("core", 20, 8);
        assert_eq!(dto.risk_level, "high");
    }

    #[test]
    fn test_impact_dto_critical_risk() {
        let dto = ImpactDto::new("critical", 100, 10);
        assert_eq!(dto.risk_level, "critical");
    }

    #[test]
    fn test_impact_dto_with_impacted_symbols() {
        let symbols = vec![
            SymbolId::new("sym_1"),
            SymbolId::new("sym_2"),
            SymbolId::new("sym_3"),
        ];
        let dto = ImpactDto::new("target", 3, 5).with_impacted_symbols(symbols);
        assert_eq!(dto.impacted_symbols.len(), 3);
        assert!(dto.impacted_symbols.contains(&"sym_1".to_string()));
    }

    #[test]
    fn test_cycle_dto_new_no_cycles() {
        let dto = CycleDto::new(false, 0);
        assert!(!dto.has_cycles);
        assert_eq!(dto.cycle_count, 0);
        assert!(dto.cycles.is_empty());
    }

    #[test]
    fn test_cycle_dto_new_with_cycles() {
        let dto = CycleDto::new(true, 2);
        assert!(dto.has_cycles);
        assert_eq!(dto.cycle_count, 2);
    }

    #[test]
    fn test_cycle_dto_with_cycles() {
        let cycles = vec![
            vec![SymbolId::new("a"), SymbolId::new("b"), SymbolId::new("a")],
            vec![
                SymbolId::new("x"),
                SymbolId::new("y"),
                SymbolId::new("z"),
                SymbolId::new("x"),
            ],
        ];
        let dto = CycleDto::new(true, 2).with_cycles(cycles);
        assert_eq!(dto.cycles.len(), 2);
        assert_eq!(dto.cycles[0], vec!["a", "b", "a"]);
        assert_eq!(dto.cycles[1], vec!["x", "y", "z", "x"]);
    }
}
