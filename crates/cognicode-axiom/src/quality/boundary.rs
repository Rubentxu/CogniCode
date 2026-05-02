//! Boundary Checker — Architecture Boundary Enforcement
//!
//! Validates that modules respect defined architectural boundaries and don't
//! create unintended cross-boundary dependencies.

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

use cognicode_core::domain::aggregates::CallGraph;
use crate::linters::Severity;

/// Definition of an architectural boundary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundaryDefinition {
    /// Unique name for this boundary (e.g., "domain", "application", "infrastructure")
    pub name: String,
    /// Path patterns that identify symbols in this boundary
    pub path_patterns: Vec<String>,
    /// Boundary names this boundary is allowed to depend on
    pub allowed_dependencies: Vec<String>,
}

impl BoundaryDefinition {
    /// Check if a file path matches this boundary
    pub fn matches_path(&self, file_path: &str) -> bool {
        self.path_patterns.iter().any(|pattern| {
            if pattern.ends_with('/') {
                // Directory pattern
                file_path.starts_with(pattern) || file_path.contains(&format!("/{}/", pattern))
            } else {
                // File pattern
                file_path.contains(pattern)
            }
        })
    }

    /// Check if this boundary allows dependency to another boundary
    pub fn allows_dependency(&self, target_boundary: &str) -> bool {
        self.allowed_dependencies.contains(&target_boundary.to_string())
            || self.allowed_dependencies.contains(&"*".to_string())
    }
}

/// Boundary checker for architecture enforcement
pub struct BoundaryChecker {
    boundaries: Vec<BoundaryDefinition>,
}

impl BoundaryChecker {
    /// Create a new boundary checker with given definitions
    pub fn new(boundaries: Vec<BoundaryDefinition>) -> Self {
        Self { boundaries }
    }

    /// Create a boundary checker with DDD-default boundaries
    ///
    /// DDD defaults:
    /// - domain: allowed to depend on nothing (innermost)
    /// - application: allowed to depend on domain
    /// - infrastructure: allowed to depend on all (outermost)
    pub fn with_ddd_defaults() -> Self {
        let boundaries = vec![
            BoundaryDefinition {
                name: "domain".to_string(),
                path_patterns: vec![
                    "src/domain/".to_string(),
                    "domain/".to_string(),
                    "src/core/".to_string(),
                    "core/".to_string(),
                ],
                allowed_dependencies: vec![],
            },
            BoundaryDefinition {
                name: "application".to_string(),
                path_patterns: vec![
                    "src/application/".to_string(),
                    "application/".to_string(),
                    "src/use_cases/".to_string(),
                    "use_cases/".to_string(),
                    "src/services/".to_string(),
                    "services/".to_string(),
                ],
                allowed_dependencies: vec!["domain".to_string()],
            },
            BoundaryDefinition {
                name: "infrastructure".to_string(),
                path_patterns: vec![
                    "src/infrastructure/".to_string(),
                    "infrastructure/".to_string(),
                    "src/adapters/".to_string(),
                    "adapters/".to_string(),
                    "src/persistence/".to_string(),
                    "persistence/".to_string(),
                    "src/external/".to_string(),
                    "external/".to_string(),
                ],
                allowed_dependencies: vec!["*".to_string()],
            },
            BoundaryDefinition {
                name: "presentation".to_string(),
                path_patterns: vec![
                    "src/presentation/".to_string(),
                    "presentation/".to_string(),
                    "src/api/".to_string(),
                    "api/".to_string(),
                    "src/ui/".to_string(),
                    "ui/".to_string(),
                    "src/controllers/".to_string(),
                    "controllers/".to_string(),
                ],
                allowed_dependencies: vec!["application".to_string(), "domain".to_string()],
            },
        ];

        Self::new(boundaries)
    }

    /// Find which boundary a symbol belongs to
    fn find_boundary(&self, file_path: &str) -> Option<&BoundaryDefinition> {
        self.boundaries.iter().find(|b| b.matches_path(file_path))
    }

    /// Check all boundaries and report violations
    pub fn check_violations(&self, graph: &CallGraph) -> Vec<BoundaryViolation> {
        let mut violations = Vec::new();

        for (source_id, target_id, _) in graph.all_dependencies() {
            let source_symbol = match graph.get_symbol(source_id) {
                Some(s) => s,
                None => continue,
            };

            let target_symbol = match graph.get_symbol(target_id) {
                Some(s) => s,
                None => continue,
            };

            let source_file = source_symbol.location().file();
            let target_file = target_symbol.location().file();

            let source_boundary = match self.find_boundary(source_file) {
                Some(b) => b,
                None => continue,
            };

            let target_boundary = match self.find_boundary(target_file) {
                Some(b) => b,
                None => continue,
            };

            if source_boundary.name == target_boundary.name {
                // Same boundary - no violation
                continue;
            }

            // Check if dependency is allowed
            if !source_boundary.allows_dependency(&target_boundary.name) {
                violations.push(BoundaryViolation {
                    from_module: source_boundary.name.clone(),
                    to_module: target_boundary.name.clone(),
                    from_boundary: source_boundary.name.clone(),
                    to_boundary: target_boundary.name.clone(),
                    symbol_name: source_symbol.name().to_string(),
                    target_name: target_symbol.name().to_string(),
                    severity: self.calculate_severity(source_boundary, target_boundary),
                });
            }
        }

        violations
    }

    /// Calculate severity based on boundary depths
    fn calculate_severity(
        &self,
        source: &BoundaryDefinition,
        target: &BoundaryDefinition,
    ) -> Severity {
        // Higher severity for domain -> infrastructure violations
        match (source.name.as_str(), target.name.as_str()) {
            ("domain", "infrastructure") => Severity::Error,
            ("domain", "application") => Severity::Error,
            ("application", "infrastructure") => Severity::Warning,
            _ => Severity::Warning,
        }
    }

    /// Generate a report summarizing boundary health
    pub fn generate_report(&self, violations: &[BoundaryViolation]) -> BoundaryReport {
        let mut by_boundary: HashMap<String, Vec<BoundaryViolation>> = HashMap::new();

        for v in violations {
            by_boundary
                .entry(v.from_boundary.clone())
                .or_insert_with(Vec::new)
                .push(v.clone());
        }

        BoundaryReport {
            total_violations: violations.len(),
            by_boundary,
            summary: format!(
                "Found {} boundary violation{}",
                violations.len(),
                if violations.len() == 1 { "" } else { "s" }
            ),
        }
    }
}

impl Default for BoundaryChecker {
    fn default() -> Self {
        Self::with_ddd_defaults()
    }
}

/// A detected boundary violation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundaryViolation {
    /// Source module name
    pub from_module: String,
    /// Target module name
    pub to_module: String,
    /// Source boundary name
    pub from_boundary: String,
    /// Target boundary name
    pub to_boundary: String,
    /// Symbol causing the violation
    pub symbol_name: String,
    /// Target symbol name
    pub target_name: String,
    /// Severity of the violation
    pub severity: Severity,
}

impl BoundaryViolation {
    /// Human-readable description
    pub fn description(&self) -> String {
        format!(
            "{} in {} depends on {} in {} (via {})",
            self.symbol_name,
            self.from_boundary,
            self.target_name,
            self.to_boundary,
            self.from_module
        )
    }
}

/// Boundary health report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundaryReport {
    pub total_violations: usize,
    pub by_boundary: HashMap<String, Vec<BoundaryViolation>>,
    pub summary: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_boundary_definition_matches_path() {
        let boundary = BoundaryDefinition {
            name: "domain".to_string(),
            path_patterns: vec!["src/domain/".to_string()],
            allowed_dependencies: vec![],
        };

        assert!(boundary.matches_path("src/domain/mod.rs"));
        assert!(boundary.matches_path("src/domain/services/user.rs"));
        assert!(!boundary.matches_path("src/infrastructure/mod.rs"));
    }

    #[test]
    fn test_boundary_definition_allows_dependency() {
        let boundary = BoundaryDefinition {
            name: "application".to_string(),
            path_patterns: vec![],
            allowed_dependencies: vec!["domain".to_string()],
        };

        assert!(boundary.allows_dependency("domain"));
        assert!(!boundary.allows_dependency("infrastructure"));
    }

    #[test]
    fn test_boundary_definition_wildcard() {
        let boundary = BoundaryDefinition {
            name: "infrastructure".to_string(),
            path_patterns: vec![],
            allowed_dependencies: vec!["*".to_string()],
        };

        assert!(boundary.allows_dependency("domain"));
        assert!(boundary.allows_dependency("application"));
    }

    #[test]
    fn test_checker_ddd_defaults() {
        let checker = BoundaryChecker::with_ddd_defaults();
        assert_eq!(checker.boundaries.len(), 4);
    }

    #[test]
    fn test_boundary_violation_description() {
        let violation = BoundaryViolation {
            from_module: "src/domain/mod.rs".to_string(),
            to_module: "src/infrastructure/mod.rs".to_string(),
            from_boundary: "domain".to_string(),
            to_boundary: "infrastructure".to_string(),
            symbol_name: "UserRepository".to_string(),
            target_name: "Database".to_string(),
            severity: Severity::Error,
        };

        let desc = violation.description();
        assert!(desc.contains("domain"));
        assert!(desc.contains("infrastructure"));
        assert!(desc.contains("UserRepository"));
    }

    #[test]
    fn test_boundary_violation_serialization() {
        let violation = BoundaryViolation {
            from_module: "src/domain/mod.rs".to_string(),
            to_module: "src/infrastructure/mod.rs".to_string(),
            from_boundary: "domain".to_string(),
            to_boundary: "infrastructure".to_string(),
            symbol_name: "UserRepository".to_string(),
            target_name: "Database".to_string(),
            severity: Severity::Error,
        };

        let json = serde_json::to_string(&violation).unwrap();
        let deserialized: BoundaryViolation = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.from_boundary, "domain");
        assert_eq!(deserialized.to_boundary, "infrastructure");
    }
}