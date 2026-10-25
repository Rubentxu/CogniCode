//! Analisis del dominio sobre el grafo construido.
//!
//! Ciclos, impacto, complejidad, codigo muerto y dependencias entre modulos.

#![allow(
    clippy::doc_overindented_list_items,
    clippy::len_zero,
    clippy::type_complexity,
    clippy::useless_vec,
    unused_comparisons
)]

use super::AnalysisService;
use crate::application::dto::{
    AnalysisMetadata, DeadCodeEntry, DeadCodeReason, DeadCodeResult, ModuleDependenciesResult,
    ModuleDependency, ModuleDependencyGraph, RelationCandidate, SymbolDto,
};
use crate::domain::aggregates::CallGraph;
use crate::domain::aggregates::call_graph::SymbolId;

impl AnalysisService {
    /// Checks for cycles in the dependency graph
    ///
    /// Returns a cycle detection result with information about any cycles found.
    /// Uses the CycleDetector service to analyze the call graph.
    pub fn check_cycles(&self, graph: &CallGraph) -> crate::domain::services::CycleDetectionResult {
        self.cycle_detector.detect_cycles(graph)
    }

    /// Analyzes the impact of changing a symbol
    ///
    /// Returns an impact report detailing direct and transitive dependents,
    /// as well as files that would be affected by changing this symbol.
    pub fn analyze_impact(
        &self,
        symbol: &crate::domain::aggregates::Symbol,
        graph: &CallGraph,
    ) -> crate::domain::services::ImpactReport {
        self.impact_analyzer.calculate_impact(symbol, graph)
    }

    /// Calculates the cyclomatic complexity based on decision points
    ///
    /// Returns a complexity report with risk assessment.
    pub fn calculate_complexity(
        &self,
        symbol_name: &str,
        decision_points: &[crate::domain::services::DecisionPoint],
        exit_points: usize,
    ) -> crate::domain::services::ComplexityReport {
        let cyclomatic = self
            .complexity_calculator
            .cyclomatic_complexity(decision_points, exit_points);
        let risk = self.complexity_calculator.risk_level(cyclomatic);

        crate::domain::services::ComplexityReport {
            symbol_name: symbol_name.to_string(),
            cyclomatic,
            cognitive: 0, // Cognitive complexity requires AST analysis
            decision_point_count: decision_points.len(),
            max_nesting_depth: 0, // Would require deeper AST analysis
            exit_point_count: exit_points,
            risk,
        }
    }

    /// Checks if it's safe to change a symbol based on impact thresholds
    pub fn is_safe_to_change(
        &self,
        symbol: &crate::domain::aggregates::Symbol,
        graph: &CallGraph,
        threshold: crate::domain::services::ImpactThreshold,
    ) -> bool {
        self.impact_analyzer
            .is_safe_to_change(symbol, graph, threshold)
    }

    /// Finds minimal set of symbols that need to be removed to break all cycles
    ///
    /// This is useful for refactoring cyclic dependencies.
    pub fn find_minimal_feedback_set(&self, graph: &CallGraph) -> Vec<SymbolId> {
        self.cycle_detector.find_minimal_feedback_set(graph)
    }

    /// Gets all entry points in the call graph (symbols with no incoming edges)
    ///
    /// Entry points are functions that are not called by any other function in the
    /// codebase. These are typically main functions, exported APIs, or independent
    /// utilities.
    pub fn get_entry_points(&self) -> Vec<SymbolDto> {
        let graph = self.get_project_graph();
        graph
            .roots()
            .iter()
            .filter_map(|id| graph.get_symbol(id).map(SymbolDto::from_symbol))
            .collect()
    }

    /// Gets all leaf functions in the call graph (symbols with no outgoing edges)
    ///
    /// Leaf functions are functions that don't call any other functions. These are
    /// typically leaf-node utilities, simple getters, or functions that interact
    /// with external systems directly.
    pub fn get_leaf_functions(&self) -> Vec<SymbolDto> {
        let graph = self.get_project_graph();
        graph
            .leaves()
            .iter()
            .filter_map(|id| graph.get_symbol(id).map(SymbolDto::from_symbol))
            .collect()
    }

    /// Detects potentially dead code in the call graph.
    ///
    /// Dead code = callable or type definition symbols that are NOT reachable
    /// from any entry point (root symbols with no incoming edges).
    ///
    /// Confidence scoring:
    /// - 0.5: Symbol is public (may be API used externally)
    /// - 0.95: Symbol is private (unlikely to be called externally)
    ///
    /// Note: This only detects functions/types that have NO incoming edges.
    /// For more accurate detection with references (F3.2), more edges need
    /// to be populated.
    pub fn detect_dead_code(&self) -> DeadCodeResult {
        let start = std::time::Instant::now();
        let graph = self.get_project_graph();

        let dead_ids = graph.find_dead_code();
        let total_symbols = graph.symbols().count();

        let dead_code: Vec<DeadCodeEntry> = dead_ids
            .iter()
            .filter_map(|id| {
                let symbol = graph.get_symbol(id)?;
                let kind_str = symbol.kind().to_string();
                let kind = match kind_str.as_str() {
                    "function" => crate::application::dto::SymbolKind::Function,
                    "method" => crate::application::dto::SymbolKind::Method,
                    "constructor" => crate::application::dto::SymbolKind::Constructor,
                    "class" => crate::application::dto::SymbolKind::Class,
                    "struct" => crate::application::dto::SymbolKind::Struct,
                    "enum" => crate::application::dto::SymbolKind::Enum,
                    "trait" => crate::application::dto::SymbolKind::Trait,
                    "type" | "interface" => crate::application::dto::SymbolKind::TypeAlias,
                    _ => crate::application::dto::SymbolKind::Unknown,
                };

                // For now, all dead code gets NoIncomingEdges reason
                // Visibility-based confidence could be added later
                let confidence = 0.5;

                Some(DeadCodeEntry {
                    symbol: symbol.fully_qualified_name().to_string(),
                    file: symbol.location().file().to_string(),
                    line: symbol.location().line() + 1,
                    column: symbol.location().column() + 1,
                    kind,
                    reason: DeadCodeReason::NoIncomingEdges,
                    confidence,
                })
            })
            .collect();

        let total_dead = dead_code.len();
        let dead_code_percent = if total_symbols > 0 {
            (total_dead as f32 / total_symbols as f32) * 100.0
        } else {
            0.0
        };

        DeadCodeResult {
            dead_code,
            total_dead,
            total_symbols,
            dead_code_percent,
            metadata: AnalysisMetadata {
                total_calls: graph.edge_count(),
                analysis_time_ms: start.elapsed().as_millis() as u64,
            },
        }
    }

    /// Suggests potential relation candidates for a symbol that appears dead.
    ///
    /// A symbol is considered a candidate for missing relations if it has no callers.
    /// This method uses three heuristics to find candidate symbols that might be
    /// the missing callers:
    ///
    /// 1. **Same file** (confidence 0.7): Other symbols in the same source file
    /// 2. **Same community** (confidence 0.5): Symbols in the same community via Label Propagation
    /// 3. **Name match** (confidence 0.3): Symbols sharing tokenized name components
    ///
    /// Returns an empty vec if the symbol already has callers (not dead).
    pub fn suggest_relation_candidates(&self, symbol_id: &str) -> Vec<RelationCandidate> {
        use crate::application::services::relation_candidates::CandidateFinder;

        let graph = self.get_project_graph();
        CandidateFinder::find_candidates(&graph, symbol_id)
    }

    /// Detects module-level dependencies and cycles in the call graph.
    ///
    /// Groups symbol-level edges into module-level edges, detects cycles using
    /// Tarjan SCC, and calculates coupling scores.
    pub fn detect_module_dependencies(&self) -> ModuleDependenciesResult {
        let start = std::time::Instant::now();
        let graph = self.get_project_graph();

        let (modules_data, cycles, coupling_matrix) = graph.find_module_dependencies();

        let total_cross_module_edges: usize = coupling_matrix.values().sum();
        let total_modules = modules_data.len();

        let modules: Vec<ModuleDependency> = modules_data
            .into_iter()
            .map(|(module, depends_on, depended_by, coupling_score)| {
                let total_deps = depends_on.len() + depended_by.len();
                let stability = if total_deps > 0 {
                    depended_by.len() as f32 / total_deps as f32
                } else {
                    0.5 // Isolated module
                };
                ModuleDependency {
                    module,
                    depends_on,
                    depended_by,
                    coupling_score,
                    stability,
                }
            })
            .collect();

        let coupling_matrix_vec: Vec<(String, String, usize)> = coupling_matrix
            .into_iter()
            .map(|((from, to), count)| (from, to, count))
            .collect();

        ModuleDependenciesResult {
            graph: ModuleDependencyGraph {
                modules,
                cycles: cycles.clone(),
                coupling_matrix: coupling_matrix_vec,
            },
            total_modules,
            total_cross_module_edges,
            cycle_count: cycles.len(),
            metadata: AnalysisMetadata {
                total_calls: graph.edge_count(),
                analysis_time_ms: start.elapsed().as_millis() as u64,
            },
        }
    }
}
