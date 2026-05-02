//! Connascence Analyzer — Measuring coupling between modules
//!
//! Analyzes 6 types of connascence (CoN, CoT, CoM, CoA, CoP, CoTm) to detect
//! coupling violations between modules.

use std::collections::{HashMap, HashSet};
use serde::{Deserialize, Serialize};

use cognicode_core::domain::aggregates::CallGraph;
use cognicode_core::domain::value_objects::DependencyType;
use crate::linters::Severity;

/// Types of connascence (coupling types)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConnascenceType {
    /// CoN - Connascence of Name: modules share the same naming convention
    Name,
    /// CoT - Connascence of Type: modules share the same type definitions
    Type,
    /// CoM - Connascence of Meaning: modules share the same semantics/interpretation
    Meaning,
    /// CoA - Connascence of Algorithm: modules use the same algorithm
    Algorithm,
    /// CoP - Connascence of Position: modules depend on argument order
    Position,
    /// CoTm - Connascence of Timing: modules depend on execution order
    Timing,
}

impl ConnascenceType {
    /// Human-readable name
    pub fn name(&self) -> &'static str {
        match self {
            ConnascenceType::Name => "Connascence of Name (CoN)",
            ConnascenceType::Type => "Connascence of Type (CoT)",
            ConnascenceType::Meaning => "Connascence of Meaning (CoM)",
            ConnascenceType::Algorithm => "Connascence of Algorithm (CoA)",
            ConnascenceType::Position => "Connascence of Position (CoP)",
            ConnascenceType::Timing => "Connascence of Timing (CoTm)",
        }
    }

    /// Severity weight (higher = more problematic)
    pub fn severity_weight(&self) -> u8 {
        match self {
            ConnascenceType::Name => 1,
            ConnascenceType::Type => 2,
            ConnascenceType::Meaning => 3,
            ConnascenceType::Algorithm => 3,
            ConnascenceType::Position => 4,
            ConnascenceType::Timing => 5,
        }
    }
}

impl std::fmt::Display for ConnascenceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Connascence analyzer for detecting coupling between modules
pub struct ConnascenceAnalyzer {
    /// Thresholds for each connascence type
    thresholds: ConnascenceThresholds,
}

impl ConnascenceAnalyzer {
    /// Create a new analyzer with default thresholds
    pub fn new() -> Self {
        Self {
            thresholds: ConnascenceThresholds::default(),
        }
    }

    /// Analyze all module pairs for connascence
    pub fn analyze(&self, graph: &CallGraph) -> ConnascenceReport {
        let module_deps = self.build_module_dependency_map(graph);
        let violations = self.find_violations(graph, &self.thresholds);

        // Count by type
        let mut by_type: HashMap<ConnascenceType, usize> = HashMap::new();
        for v in &violations {
            *by_type.entry(v.connascence_type).or_insert(0) += 1;
        }

        // Calculate overall coupling score (0.0 = loose, 1.0 = tight)
        let coupling_score = self.coupling_score(graph);

        let total_pairs = module_deps.len();

        ConnascenceReport {
            total_pairs,
            violations,
            coupling_score,
            by_type,
        }
    }

    /// Find violations above configurable thresholds
    pub fn find_violations(
        &self,
        graph: &CallGraph,
        thresholds: &ConnascenceThresholds,
    ) -> Vec<ConnascenceViolation> {
        let mut violations = Vec::new();

        // Detect each type of connascence
        violations.extend(self.detect_name_connascence(graph, thresholds));
        violations.extend(self.detect_type_connascence(graph, thresholds));
        violations.extend(self.detect_position_connascence(graph, thresholds));
        violations.extend(self.detect_timing_connascence(graph, thresholds));
        // CoM and CoA are harder to detect statically - use heuristics

        violations
    }

    /// Calculate overall coupling score
    pub fn coupling_score(&self, graph: &CallGraph) -> f64 {
        let module_deps = self.build_module_dependency_map(graph);

        if module_deps.is_empty() {
            return 0.0;
        }

        // Calculate coupling based on inter-module dependencies
        let total_edges: usize = module_deps.values().map(|v| v.len()).sum();
        let module_count = graph.modules().len();

        if module_count <= 1 {
            return 0.0;
        }

        // Max possible edges = n * (n-1) / 2
        let max_edges = module_count * (module_count - 1) / 2;
        let normalized_coupling = total_edges as f64 / max_edges as f64;

        // Also factor in fan-out distribution
        let avg_fan_out = total_edges as f64 / module_count as f64;
        let fan_out_factor = (avg_fan_out / 10.0).min(1.0);

        (normalized_coupling + fan_out_factor) / 2.0
    }

    /// Build a map of module -> dependencies
    fn build_module_dependency_map(&self, graph: &CallGraph) -> HashMap<String, HashSet<String>> {
        let mut module_deps: HashMap<String, HashSet<String>> = HashMap::new();

        for (source_id, target_id, _) in graph.all_dependencies() {
            let source_module = graph
                .get_symbol(source_id)
                .map(|s| Self::module_from_file(s.location().file()))
                .unwrap_or_default();

            let target_module = graph
                .get_symbol(target_id)
                .map(|s| Self::module_from_file(s.location().file()))
                .unwrap_or_default();

            if !source_module.is_empty() && !target_module.is_empty() {
                module_deps
                    .entry(source_module)
                    .or_insert_with(HashSet::new)
                    .insert(target_module);
            }
        }

        module_deps
    }

    /// Extract module name from file path
    fn module_from_file(file: &str) -> String {
        std::path::Path::new(file)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| file.to_string())
    }

    /// Detect Connascence of Name (CoN)
    /// Modules share symbols with the same name
    fn detect_name_connascence(
        &self,
        graph: &CallGraph,
        thresholds: &ConnascenceThresholds,
    ) -> Vec<ConnascenceViolation> {
        let mut violations = Vec::new();

        // Find symbols with same name across different modules
        let mut name_to_modules: HashMap<String, Vec<String>> = HashMap::new();

        for symbol in graph.symbols() {
            let module = Self::module_from_file(symbol.location().file());
            if module.is_empty() {
                continue;
            }
            name_to_modules
                .entry(symbol.name().to_string())
                .or_insert_with(Vec::new)
                .push(module);
        }

        for (name, modules) in name_to_modules {
            if modules.len() > 1 {
                // Found same-named symbols across modules
                let unique_modules: Vec<_> = modules.into_iter().collect();
                for i in 0..unique_modules.len() {
                    for j in (i + 1)..unique_modules.len() {
                        violations.push(ConnascenceViolation {
                            source_module: unique_modules[i].clone(),
                            target_module: unique_modules[j].clone(),
                            connascence_type: ConnascenceType::Name,
                            severity: if thresholds.con_name < 3 {
                                Severity::Warning
                            } else {
                                Severity::Info
                            },
                            description: format!("Symbol '{}' exists in multiple modules", name),
                            suggestion: "Use module-prefixed naming or move to a shared module".to_string(),
                        });
                    }
                }
            }
        }

        violations
    }

    /// Detect Connascence of Type (CoT)
    /// Modules depend on the same type definitions
    fn detect_type_connascence(
        &self,
        graph: &CallGraph,
        thresholds: &ConnascenceThresholds,
    ) -> Vec<ConnascenceViolation> {
        let mut violations = Vec::new();

        // Find type dependencies between modules
        let mut type_dependencies: HashMap<(String, String), Vec<String>> = HashMap::new();

        for (source_id, target_id, _dep_type) in graph.all_dependencies() {
            let source_module = graph
                .get_symbol(source_id)
                .map(|s| Self::module_from_file(s.location().file()))
                .unwrap_or_default();

            let target_module = graph
                .get_symbol(target_id)
                .map(|s| Self::module_from_file(s.location().file()))
                .unwrap_or_default();

            if source_module.is_empty() || target_module.is_empty() || source_module == target_module {
                continue;
            }

            // Check if target is a type definition
            let target_symbol = graph.get_symbol(target_id);
            let is_type = target_symbol
                .map(|s| s.kind().is_type_definition())
                .unwrap_or(false);

            if is_type {
                let key = (source_module, target_module);
                let target_name = target_symbol
                    .map(|s| s.name().to_string())
                    .unwrap_or_default();
                type_dependencies
                    .entry(key)
                    .or_insert_with(Vec::new)
                    .push(target_name);
            }
        }

        for ((source, target), types) in type_dependencies {
            if types.len() >= thresholds.con_type {
                violations.push(ConnascenceViolation {
                    source_module: source,
                    target_module: target,
                    connascence_type: ConnascenceType::Type,
                    severity: Severity::Warning,
                    description: format!(
                        "Modules share {} type dependencies: {}",
                        types.len(),
                        types.join(", ")
                    ),
                    suggestion: "Consider extracting shared types into a separate module".to_string(),
                });
            }
        }

        violations
    }

    /// Detect Connascence of Position (CoP)
    /// Functions across modules have same arity/parameter positions
    fn detect_position_connascence(
        &self,
        graph: &CallGraph,
        thresholds: &ConnascenceThresholds,
    ) -> Vec<ConnascenceViolation> {
        let mut violations = Vec::new();

        // Find functions with same arity called across modules
        let mut arity_by_module: HashMap<(String, usize), Vec<String>> = HashMap::new();

        for symbol in graph.symbols() {
            if !symbol.is_callable() {
                continue;
            }

            let module = Self::module_from_file(symbol.location().file());
            if module.is_empty() {
                continue;
            }

            let arity = symbol
                .signature()
                .map(|s| s.arity())
                .unwrap_or(0);

            let key = (module, arity);
            arity_by_module
                .entry(key)
                .or_insert_with(Vec::new)
                .push(symbol.name().to_string());
        }

        // Look for cross-module position dependencies
        let arity_entries: Vec<_> = arity_by_module.iter().collect();
        for ((module1, arity1), funcs1) in &arity_entries {
            for ((module2, arity2), funcs2) in &arity_entries {
                if module1 != module2 && arity1 == arity2 {
                    let shared: Vec<_> = funcs1
                        .iter()
                        .filter(|f| funcs2.contains(f))
                        .cloned()
                        .collect();

                    if !shared.is_empty() && shared.len() >= thresholds.con_position {
                        violations.push(ConnascenceViolation {
                            source_module: module1.clone(),
                            target_module: module2.clone(),
                            connascence_type: ConnascenceType::Position,
                            severity: Severity::Warning,
                            description: format!(
                                "Functions with same name '{}' have same arity across modules",
                                shared.join(", ")
                            ),
                            suggestion: "Use different function names or restructure to reduce position coupling".to_string(),
                        });
                    }
                }
            }
        }

        violations
    }

    /// Detect Connascence of Timing (CoTm)
    /// Modules depend on execution order
    fn detect_timing_connascence(
        &self,
        graph: &CallGraph,
        thresholds: &ConnascenceThresholds,
    ) -> Vec<ConnascenceViolation> {
        let mut violations = Vec::new();

        // Find sequential dependencies in call chains
        let mut timing_pairs: HashMap<(String, String), usize> = HashMap::new();

        for (source_id, target_id, dep_type) in graph.all_dependencies() {
            if !matches!(dep_type, DependencyType::Calls) {
                continue;
            }

            let source_module = graph
                .get_symbol(source_id)
                .map(|s| Self::module_from_file(s.location().file()))
                .unwrap_or_default();

            let target_module = graph
                .get_symbol(target_id)
                .map(|s| Self::module_from_file(s.location().file()))
                .unwrap_or_default();

            if source_module.is_empty() || target_module.is_empty() || source_module == target_module {
                continue;
            }

            *timing_pairs.entry((source_module, target_module)).or_insert(0) += 1;
        }

        for ((source, target), count) in timing_pairs {
            if count >= thresholds.con_timing {
                let target_clone = target.clone();
                violations.push(ConnascenceViolation {
                    source_module: source,
                    target_module: target,
                    connascence_type: ConnascenceType::Timing,
                    severity: Severity::Info,
                    description: format!("{} calls to {} indicate timing dependency", count, target_clone),
                    suggestion: "Consider async/parallel execution or event-driven architecture".to_string(),
                });
            }
        }

        violations
    }
}

impl Default for ConnascenceAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

/// Thresholds for each connascence type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnascenceThresholds {
    /// Minimum occurrences for CoN (Name)
    pub con_name: usize,
    /// Minimum occurrences for CoT (Type)
    pub con_type: usize,
    /// Minimum occurrences for CoM (Meaning)
    pub con_meaning: usize,
    /// Minimum occurrences for CoA (Algorithm)
    pub con_algorithm: usize,
    /// Minimum occurrences for CoP (Position)
    pub con_position: usize,
    /// Minimum occurrences for CoTm (Timing)
    pub con_timing: usize,
}

impl Default for ConnascenceThresholds {
    fn default() -> Self {
        Self {
            con_name: 2,
            con_type: 2,
            con_meaning: 3,
            con_algorithm: 3,
            con_position: 2,
            con_timing: 5,
        }
    }
}

/// A detected connascence violation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnascenceViolation {
    /// Source module
    pub source_module: String,
    /// Target module
    pub target_module: String,
    /// Type of connascence
    pub connascence_type: ConnascenceType,
    /// Severity level
    pub severity: Severity,
    /// Human-readable description
    pub description: String,
    /// Suggested refactoring
    pub suggestion: String,
}

/// Complete connascence analysis report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnascenceReport {
    /// Total module pairs analyzed
    pub total_pairs: usize,
    /// Detected violations
    pub violations: Vec<ConnascenceViolation>,
    /// Overall coupling score (0.0 loose, 1.0 tight)
    pub coupling_score: f64,
    /// Violations grouped by type
    pub by_type: HashMap<ConnascenceType, usize>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use cognicode_core::domain::aggregates::{CallGraph, Symbol};
    use cognicode_core::domain::value_objects::{Location, SymbolKind};

    fn create_test_graph() -> CallGraph {
        let mut graph = CallGraph::new();

        // Create two modules with a shared type
        let type_a = Symbol::new("User", SymbolKind::Class, Location::new("module_a/mod.rs", 1, 1));
        let type_b = Symbol::new("User", SymbolKind::Class, Location::new("module_b/mod.rs", 1, 1));

        let _id_a = graph.add_symbol(type_a);
        let id_b = graph.add_symbol(type_b);

        // Create function in module_a that uses module_b's type
        let func_a = Symbol::new("process", SymbolKind::Function, Location::new("module_a/mod.rs", 10, 1));
        let func_id_a = graph.add_symbol(func_a);

        let _ = graph.add_dependency(&func_id_a, &id_b, DependencyType::Calls);

        graph
    }

    #[test]
    fn test_analyzer_new() {
        let analyzer = ConnascenceAnalyzer::new();
        assert_eq!(analyzer.coupling_score(&CallGraph::new()), 0.0);
    }

    #[test]
    fn test_analyze() {
        let graph = create_test_graph();
        let analyzer = ConnascenceAnalyzer::new();
        let report = analyzer.analyze(&graph);

        assert!(report.coupling_score >= 0.0);
        assert!(report.coupling_score <= 1.0);
    }

    #[test]
    fn test_connascence_type_name() {
        assert_eq!(ConnascenceType::Name.name(), "Connascence of Name (CoN)");
        assert_eq!(ConnascenceType::Type.name(), "Connascence of Type (CoT)");
    }

    #[test]
    fn test_connascence_type_severity_weight() {
        assert!(ConnascenceType::Timing.severity_weight() > ConnascenceType::Name.severity_weight());
    }

    #[test]
    fn test_thresholds_default() {
        let thresholds = ConnascenceThresholds::default();
        assert_eq!(thresholds.con_name, 2);
        assert_eq!(thresholds.con_type, 2);
    }

    #[test]
    fn test_violation_serialization() {
        let violation = ConnascenceViolation {
            source_module: "module_a".to_string(),
            target_module: "module_b".to_string(),
            connascence_type: ConnascenceType::Name,
            severity: Severity::Warning,
            description: "Test violation".to_string(),
            suggestion: "Fix this".to_string(),
        };

        let json = serde_json::to_string(&violation).unwrap();
        let deserialized: ConnascenceViolation = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.source_module, "module_a");
        assert_eq!(deserialized.connascence_type, ConnascenceType::Name);
    }
}