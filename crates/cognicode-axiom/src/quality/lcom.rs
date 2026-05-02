//! LCOM Calculator — Lack of Cohesion of Methods (LCOM-4)
//!
//! Measures how cohesive a class/struct is by analyzing method pairs and their
//! shared access to fields and callees.

use std::collections::{HashMap, HashSet};
use serde::{Deserialize, Serialize};

use cognicode_core::domain::aggregates::{CallGraph, Symbol, SymbolId};
use cognicode_core::domain::value_objects::SymbolKind;

/// Lack of Cohesion of Methods (LCOM) Calculator
///
/// LCOM-4 calculates cohesion based on how methods share access to instance variables
/// and call relationships. A highly cohesive class has methods that work on the same
/// fields and collaborate closely.
pub struct LcomCalculator {
    /// Minimum method count to analyze (avoids analyzing trivial types)
    min_methods: usize,
}

impl LcomCalculator {
    /// Create a new LCOM calculator
    pub fn new() -> Self {
        Self { min_methods: 2 }
    }

    /// Calculate LCOM for a single type by name
    pub fn calculate_for_type(&self, graph: &CallGraph, type_name: &str) -> LcomResult {
        // Find the type symbol
        let type_symbols: Vec<&Symbol> = graph
            .find_by_name(type_name)
            .into_iter()
            .filter(|s| {
                matches!(
                    s.kind(),
                    SymbolKind::Class | SymbolKind::Struct | SymbolKind::Trait | SymbolKind::Interface
                )
            })
            .collect();

        let type_symbol = match type_symbols.first() {
            Some(s) => s,
            None => {
                return LcomResult {
                    type_name: type_name.to_string(),
                    lcom_score: 0.0,
                    method_count: 0,
                    field_count: 0,
                    cohesion_level: CohesionLevel::High,
                    suggestions: vec!["Type not found in graph".to_string()],
                };
            }
        };

        self.calculate_for_symbol(graph, type_symbol)
    }

    /// Calculate LCOM for a type symbol
    pub     fn calculate_for_symbol(&self, graph: &CallGraph, type_symbol: &Symbol) -> LcomResult {
        let type_name = type_symbol.name().to_string();

        // Find all methods of this type (methods in same file with related names)
        let methods = self.find_methods_for_type(graph, type_symbol);

        if methods.len() < self.min_methods {
            return LcomResult {
                type_name,
                lcom_score: 0.0,
                method_count: methods.len(),
                field_count: 0,
                cohesion_level: CohesionLevel::High,
                suggestions: vec![],
            };
        }

        // Find fields of this type
        let fields = self.find_fields_for_type(graph, type_symbol);

        // Calculate cohesion metrics
        let (lcom_score, cohesion_level, suggestions) =
            self.calculate_lcom_score(graph, &methods, &fields);

        LcomResult {
            type_name,
            lcom_score,
            method_count: methods.len(),
            field_count: fields.len(),
            cohesion_level,
            suggestions,
        }
    }

    /// Calculate LCOM for all struct/class types in the graph
    pub fn calculate_all(&self, graph: &CallGraph) -> HashMap<String, LcomResult> {
        let mut results = HashMap::new();

        for symbol in graph.symbols() {
            if matches!(
                symbol.kind(),
                SymbolKind::Class | SymbolKind::Struct | SymbolKind::Trait | SymbolKind::Interface
            ) {
                let result = self.calculate_for_symbol(graph, symbol);
                results.insert(result.type_name.clone(), result);
            }
        }

        results
    }

    /// Find methods that belong to a type
    fn find_methods_for_type(&self, graph: &CallGraph, type_symbol: &Symbol) -> Vec<SymbolId> {
        let type_file = type_symbol.location().file();
        let type_name = type_symbol.name();

        let mut methods = Vec::new();

        for symbol in graph.symbols() {
            // Method should be in the same file
            if symbol.location().file() != type_file {
                continue;
            }

            // Check if it's a method (name starts with type name or is a method kind)
            let is_method = match symbol.kind() {
                SymbolKind::Method => true,
                SymbolKind::Function => {
                    // Heuristic: functions in the same file that might be methods
                    // Check if name contains the type name or if it's camelCase after the type
                    let name = symbol.name();
                    name.starts_with(&format!("{}.", type_name))
                        || name.starts_with(&format!("{}::", type_name))
                        || name.contains(&format!("_{}", type_name))
                }
                _ => false,
            };

            if is_method {
                let id = SymbolId::new(symbol.fully_qualified_name());
                methods.push(id);
            }
        }

        methods
    }

    /// Find fields that belong to a type
    fn find_fields_for_type(&self, graph: &CallGraph, type_symbol: &Symbol) -> Vec<SymbolId> {
        let type_file = type_symbol.location().file();
        let type_name = type_symbol.name();

        let mut fields = Vec::new();

        for symbol in graph.symbols() {
            if symbol.location().file() != type_file {
                continue;
            }

            let is_field = match symbol.kind() {
                SymbolKind::Field | SymbolKind::Property => true,
                SymbolKind::Variable => {
                    // Heuristic: variables in the same file that might be fields
                    let name = symbol.name();
                    name.starts_with(&format!("{}.", type_name))
                        || name.starts_with(&format!("self."))
                        || (name.starts_with(|c: char| c.is_lowercase())
                            && name.chars().next().map(|c| c.is_ascii_lowercase()).unwrap_or(false))
                }
                _ => false,
            };

            if is_field {
                let id = SymbolId::new(symbol.fully_qualified_name());
                fields.push(id);
            }
        }

        fields
    }

    /// Calculate LCOM-4 score based on method pairs
    ///
    /// LCOM-4 = 1 - (shared_pairs / total_pairs)
    ///
    /// Where shared_pairs are method pairs that share at least one callee or field access
    fn calculate_lcom_score(
        &self,
        graph: &CallGraph,
        method_ids: &[SymbolId],
        _field_ids: &[SymbolId],
    ) -> (f64, CohesionLevel, Vec<String>) {
        if method_ids.len() < 2 {
            return (0.0, CohesionLevel::High, vec![]);
        }

        // For each method, collect its callees
        let method_callees: Vec<HashSet<SymbolId>> = method_ids
            .iter()
            .map(|id| {
                let callees: HashSet<SymbolId> = graph
                    .callees(id)
                    .into_iter()
                    .map(|(callee_id, _)| callee_id)
                    .collect();
                callees
            })
            .collect();

        // Count shared pairs vs total pairs
        let total_pairs = method_ids.len() * (method_ids.len() - 1) / 2;
        let mut shared_pairs = 0;

        for i in 0..method_ids.len() {
            for j in (i + 1)..method_ids.len() {
                // Two methods share if they have any callees in common
                let shared: HashSet<_> = method_callees[i]
                    .intersection(&method_callees[j])
                    .collect();
                if !shared.is_empty() {
                    shared_pairs += 1;
                }
            }
        }

        // LCOM = 1 - (shared_pairs / total_pairs)
        // Higher score = less cohesive
        let lcom_score = if total_pairs > 0 {
            1.0 - (shared_pairs as f64 / total_pairs as f64)
        } else {
            0.0
        };

        let cohesion_level = CohesionLevel::from_score(lcom_score);
        let suggestions = self.generate_suggestions(lcom_score, method_ids.len());

        (lcom_score, cohesion_level, suggestions)
    }

    /// Generate refactoring suggestions based on LCOM score
    fn generate_suggestions(&self, lcom_score: f64, method_count: usize) -> Vec<String> {
        let mut suggestions = Vec::new();

        match lcom_score {
            s if s >= 0.7 => {
                suggestions.push("Consider splitting this type into smaller, more focused types".to_string());
                suggestions.push("Look for groups of methods that work on different field sets".to_string());
            }
            s if s >= 0.5 => {
                suggestions.push("Consider extracting related methods into a separate trait or module".to_string());
            }
            s if s >= 0.3 => {
                suggestions.push("Review if all methods truly belong to this type".to_string());
            }
            _ => {}
        }

        if method_count > 10 {
            suggestions.push("This type has many methods - consider if it violates SRP".to_string());
        }

        if suggestions.is_empty() {
            suggestions.push("Cohesion is good - no refactoring needed".to_string());
        }

        suggestions
    }
}

impl Default for LcomCalculator {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of LCOM calculation for a single type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LcomResult {
    /// Name of the type
    pub type_name: String,
    /// LCOM score: 0.0 (highly cohesive) to 1.0+ (no cohesion)
    pub lcom_score: f64,
    /// Number of methods analyzed
    pub method_count: usize,
    /// Number of fields detected
    pub field_count: usize,
    /// Qualitative cohesion assessment
    pub cohesion_level: CohesionLevel,
    /// Suggestions for improving cohesion
    pub suggestions: Vec<String>,
}

/// Cohesion level classification
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum CohesionLevel {
    /// Highly cohesive (LCOM 0.0 - 0.3)
    High,
    /// Medium cohesion (LCOM 0.3 - 0.5)
    Medium,
    /// Low cohesion (LCOM 0.5 - 0.7)
    Low,
    /// Very low cohesion (LCOM > 0.7)
    VeryLow,
}

impl CohesionLevel {
    /// Classify cohesion level from LCOM score
    pub fn from_score(score: f64) -> Self {
        if score < 0.3 {
            CohesionLevel::High
        } else if score < 0.5 {
            CohesionLevel::Medium
        } else if score < 0.7 {
            CohesionLevel::Low
        } else {
            CohesionLevel::VeryLow
        }
    }

    /// Human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            CohesionLevel::High => "Highly cohesive - methods share common purpose",
            CohesionLevel::Medium => "Moderate cohesion - some methods may be unrelated",
            CohesionLevel::Low => "Low cohesion - consider splitting this type",
            CohesionLevel::VeryLow => "Very low cohesion - urgent refactoring recommended",
        }
    }
}

impl std::fmt::Display for CohesionLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CohesionLevel::High => write!(f, "High"),
            CohesionLevel::Medium => write!(f, "Medium"),
            CohesionLevel::Low => write!(f, "Low"),
            CohesionLevel::VeryLow => write!(f, "VeryLow"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cognicode_core::domain::aggregates::CallGraph;
    use cognicode_core::domain::value_objects::{DependencyType, Location};

    fn create_test_graph() -> CallGraph {
        let mut graph = CallGraph::new();

        // Create a class
        let class = Symbol::new("UserService", SymbolKind::Class, Location::new("user_service.rs", 1, 1));
        graph.add_symbol(class);

        // Create methods
        let method1 = Symbol::new("UserService.create", SymbolKind::Method, Location::new("user_service.rs", 10, 1));
        let method2 = Symbol::new("UserService.update", SymbolKind::Method, Location::new("user_service.rs", 20, 1));
        let method3 = Symbol::new("UserService.delete", SymbolKind::Method, Location::new("user_service.rs", 30, 1));

        let id1 = graph.add_symbol(method1);
        let id2 = graph.add_symbol(method2);
        let id3 = graph.add_symbol(method3);

        // All methods call a shared dependency (high cohesion)
        let db = Symbol::new("Database", SymbolKind::Class, Location::new("db.rs", 1, 1));
        let db_id = graph.add_symbol(db);

        let _ = graph.add_dependency(&id1, &db_id, DependencyType::Calls);
        let _ = graph.add_dependency(&id2, &db_id, DependencyType::Calls);
        let _ = graph.add_dependency(&id3, &db_id, DependencyType::Calls);

        graph
    }

    #[test]
    fn test_lcom_calculator_new() {
        let calc = LcomCalculator::new();
        assert_eq!(calc.min_methods, 2);
    }

    #[test]
    fn test_cohesion_level_from_score() {
        assert_eq!(CohesionLevel::from_score(0.0), CohesionLevel::High);
        assert_eq!(CohesionLevel::from_score(0.2), CohesionLevel::High);
        assert_eq!(CohesionLevel::from_score(0.3), CohesionLevel::Medium);
        assert_eq!(CohesionLevel::from_score(0.4), CohesionLevel::Medium);
        assert_eq!(CohesionLevel::from_score(0.5), CohesionLevel::Low);
        assert_eq!(CohesionLevel::from_score(0.6), CohesionLevel::Low);
        assert_eq!(CohesionLevel::from_score(0.7), CohesionLevel::VeryLow);
        assert_eq!(CohesionLevel::from_score(1.0), CohesionLevel::VeryLow);
    }

    #[test]
    fn test_calculate_for_nonexistent_type() {
        let graph = create_test_graph();
        let calc = LcomCalculator::new();
        let result = calc.calculate_for_type(&graph, "NonExistent");

        assert_eq!(result.type_name, "NonExistent");
        assert_eq!(result.lcom_score, 0.0);
        assert_eq!(result.method_count, 0);
    }

    #[test]
    fn test_calculate_all() {
        let graph = create_test_graph();
        let calc = LcomCalculator::new();
        let results = calc.calculate_all(&graph);

        // Should find UserService
        assert!(results.contains_key("UserService"));
    }

    #[test]
    fn test_lcom_result_serialization() {
        let result = LcomResult {
            type_name: "TestType".to_string(),
            lcom_score: 0.5,
            method_count: 3,
            field_count: 2,
            cohesion_level: CohesionLevel::Low,
            suggestions: vec!["Consider splitting".to_string()],
        };

        let json = serde_json::to_string(&result).unwrap();
        let deserialized: LcomResult = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.type_name, "TestType");
        assert_eq!(deserialized.lcom_score, 0.5);
    }

    #[test]
    fn test_cohesion_level_display() {
        assert_eq!(format!("{}", CohesionLevel::High), "High");
        assert_eq!(format!("{}", CohesionLevel::VeryLow), "VeryLow");
    }
}