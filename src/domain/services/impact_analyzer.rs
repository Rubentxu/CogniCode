//! Service for analyzing the impact of changes to symbols
//!
//! Calculates impact reports and safety assessments for refactoring operations.

use std::collections::HashSet;
use std::path::PathBuf;

use crate::domain::aggregates::{CallGraph, Symbol, SymbolId};

/// Service for analyzing the impact of symbol changes
pub struct ImpactAnalyzer;

impl ImpactAnalyzer {
    /// Creates a new ImpactAnalyzer
    pub fn new() -> Self {
        Self
    }

    /// Calculates the impact report for changing a symbol
    pub fn calculate_impact(&self, symbol: &Symbol, graph: &CallGraph) -> ImpactReport {
        let symbol_id = SymbolId::new(symbol.fully_qualified_name());

        let direct_dependents = graph.callers(&symbol_id).len();
        let transitive_dependents = graph.find_all_dependents(&symbol_id).len();
        let direct_dependencies = graph.callees(&symbol_id).len();
        let transitive_dependencies = graph.find_all_dependencies(&symbol_id).len();

        let impact_level = self.determine_impact_level(
            direct_dependents,
            transitive_dependents,
            symbol.is_type_definition(),
        );

        ImpactReport {
            symbol_id,
            direct_dependents,
            transitive_dependents,
            direct_dependencies,
            transitive_dependencies,
            impact_level,
            affected_files: self.collect_affected_files(symbol, graph),
        }
    }

    /// Determines if it's safe to change a symbol given an impact threshold
    pub fn is_safe_to_change(
        &self,
        symbol: &Symbol,
        graph: &CallGraph,
        threshold: ImpactThreshold,
    ) -> bool {
        let report = self.calculate_impact(symbol, graph);
        report.impact_level <= threshold.max_level
            && report.transitive_dependents <= threshold.max_dependents
    }

    /// Determines the impact level based on dependent counts
    fn determine_impact_level(
        &self,
        direct_dependents: usize,
        transitive_dependents: usize,
        is_type_definition: bool,
    ) -> ImpactLevel {
        // Type definitions have higher impact
        let multiplier = if is_type_definition { 2 } else { 1 };
        let adjusted_transitive = transitive_dependents * multiplier;

        if direct_dependents == 0 && transitive_dependents == 0 {
            ImpactLevel::Minimal
        } else if direct_dependents <= 2 && adjusted_transitive <= 5 {
            ImpactLevel::Low
        } else if direct_dependents <= 5 && adjusted_transitive <= 15 {
            ImpactLevel::Medium
        } else if direct_dependents <= 10 && adjusted_transitive <= 30 {
            ImpactLevel::High
        } else {
            ImpactLevel::Critical
        }
    }

    /// Collects all files that would be affected by changing this symbol
    fn collect_affected_files(
        &self,
        symbol: &Symbol,
        graph: &CallGraph,
    ) -> Vec<std::path::PathBuf> {
        let symbol_id = SymbolId::new(symbol.fully_qualified_name());
        let mut files: Vec<std::path::PathBuf> =
            vec![std::path::PathBuf::from(symbol.location().file())];

        for dependent_id in graph.find_all_dependents(&symbol_id) {
            if let Some(dependent_symbol) = graph.get_symbol(&dependent_id) {
                let path = std::path::PathBuf::from(dependent_symbol.location().file());
                if !files.contains(&path) {
                    files.push(path);
                }
            }
        }

        files
    }
}

impl Default for ImpactAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

/// Report detailing the impact of changing a symbol
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImpactReport {
    /// The symbol being analyzed
    pub symbol_id: SymbolId,
    /// Number of direct callers/dependents
    pub direct_dependents: usize,
    /// Number of transitive callers/dependents
    pub transitive_dependents: usize,
    /// Number of direct dependencies
    pub direct_dependencies: usize,
    /// Number of transitive dependencies
    pub transitive_dependencies: usize,
    /// The assessed impact level
    pub impact_level: ImpactLevel,
    /// Files that would be affected
    pub affected_files: Vec<std::path::PathBuf>,
}

impl ImpactReport {
    /// Returns the total number of affected symbols (direct + transitive)
    pub fn total_affected_symbols(&self) -> usize {
        self.direct_dependents
            .saturating_add(self.transitive_dependents)
    }

    /// Returns true if this impact is within acceptable limits
    pub fn is_acceptable(&self, threshold: &ImpactThreshold) -> bool {
        self.impact_level <= threshold.max_level
            && self.transitive_dependents <= threshold.max_dependents
    }
}

/// Level of impact
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ImpactLevel {
    /// No dependents, safe to change
    Minimal,
    /// Few dependents, low risk
    Low,
    /// Moderate impact
    Medium,
    /// High impact, significant testing needed
    High,
    /// Very high impact, extensive changes required
    Critical,
}

impl ImpactLevel {
    /// Returns a description of this impact level
    pub fn description(&self) -> &'static str {
        match self {
            ImpactLevel::Minimal => "Minimal impact - no dependents",
            ImpactLevel::Low => "Low impact - few dependents",
            ImpactLevel::Medium => "Medium impact - moderate number of dependents",
            ImpactLevel::High => "High impact - many dependents",
            ImpactLevel::Critical => "Critical impact - extensive changes required",
        }
    }

    /// Returns the risk color (for UI purposes)
    pub fn risk_color(&self) -> &'static str {
        match self {
            ImpactLevel::Minimal => "green",
            ImpactLevel::Low => "green",
            ImpactLevel::Medium => "yellow",
            ImpactLevel::High => "orange",
            ImpactLevel::Critical => "red",
        }
    }
}

impl fmt::Display for ImpactLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// Threshold for acceptable impact
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImpactThreshold {
    /// Maximum acceptable impact level
    pub max_level: ImpactLevel,
    /// Maximum number of transitive dependents
    pub max_dependents: usize,
}

impl ImpactThreshold {
    /// Creates a conservative threshold
    pub fn conservative() -> Self {
        Self {
            max_level: ImpactLevel::Low,
            max_dependents: 10,
        }
    }

    /// Creates a moderate threshold
    pub fn moderate() -> Self {
        Self {
            max_level: ImpactLevel::Medium,
            max_dependents: 25,
        }
    }

    /// Creates a permissive threshold
    pub fn permissive() -> Self {
        Self {
            max_level: ImpactLevel::High,
            max_dependents: 50,
        }
    }

    /// Creates a threshold that allows any change
    pub fn unlimited() -> Self {
        Self {
            max_level: ImpactLevel::Critical,
            max_dependents: usize::MAX,
        }
    }
}

impl Default for ImpactThreshold {
    fn default() -> Self {
        Self::moderate()
    }
}

use std::fmt;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::value_objects::{Location, SymbolKind};

    fn create_test_graph() -> CallGraph {
        let mut graph = CallGraph::new();

        // Create a function 'main' that calls 'helper'
        let main = Symbol::new("main", SymbolKind::Function, Location::new("main.rs", 1, 1));
        let helper = Symbol::new(
            "helper",
            SymbolKind::Function,
            Location::new("helper.rs", 5, 1),
        );
        let utility = Symbol::new(
            "utility",
            SymbolKind::Function,
            Location::new("util.rs", 10, 1),
        );

        let main_id = graph.add_symbol(main);
        let helper_id = graph.add_symbol(helper);
        let _utility_id = graph.add_symbol(utility);

        // main calls helper
        graph
            .add_dependency(&main_id, &helper_id, DependencyType::Calls)
            .unwrap();

        graph
    }

    #[test]
    fn test_impact_analyzer_no_dependents() {
        let analyzer = ImpactAnalyzer::new();
        let graph = CallGraph::new();

        let symbol = Symbol::new(
            "orphan",
            SymbolKind::Function,
            Location::new("test.rs", 1, 1),
        );
        let report = analyzer.calculate_impact(&symbol, &graph);

        assert_eq!(report.direct_dependents, 0);
        assert_eq!(report.transitive_dependents, 0);
        assert_eq!(report.impact_level, ImpactLevel::Minimal);
    }

    #[test]
    fn test_impact_analyzer_with_dependents() {
        let analyzer = ImpactAnalyzer::new();
        let graph = create_test_graph();

        let helper_symbol = Symbol::new(
            "helper",
            SymbolKind::Function,
            Location::new("helper.rs", 5, 1),
        );
        let report = analyzer.calculate_impact(&helper_symbol, &graph);

        assert_eq!(report.direct_dependents, 1); // main
        assert_eq!(report.transitive_dependents, 1);
        assert_eq!(report.impact_level, ImpactLevel::Low);
    }

    #[test]
    fn test_is_safe_to_change() {
        let analyzer = ImpactAnalyzer::new();
        let graph = create_test_graph();

        let helper_symbol = Symbol::new(
            "helper",
            SymbolKind::Function,
            Location::new("helper.rs", 5, 1),
        );

        // With conservative threshold
        assert!(analyzer.is_safe_to_change(
            &helper_symbol,
            &graph,
            ImpactThreshold::conservative()
        ));

        // With very strict threshold
        let strict = ImpactThreshold {
            max_level: ImpactLevel::Minimal,
            max_dependents: 0,
        };
        assert!(!analyzer.is_safe_to_change(&helper_symbol, &graph, strict));
    }

    #[test]
    fn test_impact_report_affected_files() {
        let analyzer = ImpactAnalyzer::new();
        let graph = create_test_graph();

        let helper_symbol = Symbol::new(
            "helper",
            SymbolKind::Function,
            Location::new("helper.rs", 5, 1),
        );
        let report = analyzer.calculate_impact(&helper_symbol, &graph);

        assert!(report
            .affected_files
            .contains(&std::path::PathBuf::from("helper.rs")));
        assert!(report
            .affected_files
            .contains(&std::path::PathBuf::from("main.rs")));
    }

    #[test]
    fn test_impact_level_ordering() {
        assert!(ImpactLevel::Critical > ImpactLevel::High);
        assert!(ImpactLevel::High > ImpactLevel::Medium);
        assert!(ImpactLevel::Medium > ImpactLevel::Low);
        assert!(ImpactLevel::Low > ImpactLevel::Minimal);
    }

    use crate::domain::value_objects::DependencyType;
}
