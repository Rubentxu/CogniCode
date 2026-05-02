//! SOLID Principle Checker
//!
//! Heuristic-based SOLID principle analysis using call graph data.
//! Provides SRP, OCP, LSP, ISP, and DIP violation detection.

use std::collections::{HashMap, HashSet};
use serde::{Deserialize, Serialize};

use cognicode_core::domain::aggregates::{CallGraph, SymbolId};
use cognicode_core::domain::value_objects::SymbolKind;
use crate::linters::Severity;
use crate::quality::lcom::LcomCalculator;

/// SOLID principle checker
pub struct SolidChecker {
    lcom_calculator: LcomCalculator,
}

impl SolidChecker {
    /// Create a new SOLID checker
    pub fn new() -> Self {
        Self {
            lcom_calculator: LcomCalculator::new(),
        }
    }

    /// Check all SOLID principles for all types in the graph
    pub fn check_all(&self, graph: &CallGraph) -> SolidReport {
        let mut violations = Vec::new();
        let mut type_scores: HashMap<String, f64> = HashMap::new();

        // Analyze each type
        for symbol in graph.symbols() {
            if symbol.kind().is_type_definition() {
                let type_report = self.check_type(graph, symbol.name());
                violations.extend(type_report.violations);
                type_scores.insert(
                    symbol.name().to_string(),
                    type_report.srp_score,
                );
            }
        }

        let total_types = type_scores.len();

        // Calculate aggregate scores
        let scores = self.calculate_scores(&violations, total_types);

        let summary = Self::generate_summary(&scores, &violations);

        SolidReport {
            total_types,
            violations,
            scores,
            summary,
        }
    }

    /// Check a single type against all SOLID principles
    pub fn check_type(&self, graph: &CallGraph, type_name: &str) -> TypeSolidReport {
        let mut violations = Vec::new();

        // Find the type symbol
        let type_symbols: Vec<_> = graph
            .find_by_name(type_name)
            .into_iter()
            .filter(|s| s.kind().is_type_definition())
            .collect();

        let type_symbol = match type_symbols.first() {
            Some(s) => s,
            None => {
                return TypeSolidReport {
                    type_name: type_name.to_string(),
                    violations: vec![],
                    srp_score: 0.0,
                };
            }
        };

        // SRP: Check via LCOM
        let lcom_result = self.lcom_calculator.calculate_for_symbol(graph, type_symbol);
        let srp_score = lcom_result.lcom_score;

        if lcom_result.lcom_score > 0.5 {
            violations.push(SolidViolation {
                principle: SolidPrinciple::SRP,
                type_name: type_name.to_string(),
                severity: if lcom_result.lcom_score > 0.7 {
                    Severity::Error
                } else {
                    Severity::Warning
                },
                description: format!(
                    "Type has low cohesion (LCOM={:.2}), indicating SRP violation",
                    lcom_result.lcom_score
                ),
                suggestion: "Consider splitting into smaller types with single responsibilities".to_string(),
                evidence: vec![format!("{} methods, LCOM score {:.2}", lcom_result.method_count, lcom_result.lcom_score)],
            });
        }

        // OCP: Check for extension points (traits/interfaces)
        let ocp_violations = self.check_ocp(graph, type_symbol);
        violations.extend(ocp_violations);

        // LSP: Check trait implementations
        let lsp_violations = self.check_lsp(graph, type_symbol);
        violations.extend(lsp_violations);

        // ISP: Check interface segregation
        let isp_violations = self.check_isp(graph, type_symbol);
        violations.extend(isp_violations);

        // DIP: Check dependency direction
        let dip_violations = self.check_dip(graph, type_symbol);
        violations.extend(dip_violations);

        TypeSolidReport {
            type_name: type_name.to_string(),
            violations,
            srp_score,
        }
    }

    /// Check Open/Closed Principle
    /// Look for types that modify behavior rather than extend it
    fn check_ocp(&self, graph: &CallGraph, type_symbol: &cognicode_core::domain::aggregates::Symbol) -> Vec<SolidViolation> {
        let mut violations = Vec::new();

        // Heuristic: If a type's methods switch on kind/type, it may violate OCP
        let id = SymbolId::new(type_symbol.fully_qualified_name());
        let callers = graph.callers(&id);

        // Count how many different types call this
        let mut caller_modules: HashSet<String> = HashSet::new();
        for caller_id in &callers {
            if let Some(caller) = graph.get_symbol(caller_id) {
                let module = Self::module_from_file(caller.location().file());
                caller_modules.insert(module);
            }
        }

        // If many callers from different modules depend on this, consider OCP implications
        if caller_modules.len() > 3 {
            violations.push(SolidViolation {
                principle: SolidPrinciple::OCP,
                type_name: type_symbol.name().to_string(),
                severity: Severity::Info,
                description: "Many modules depend on this type - ensure it's stable for extension".to_string(),
                suggestion: "Consider if behavior changes would break dependents".to_string(),
                evidence: caller_modules.iter().take(5).cloned().collect(),
            });
        }

        violations
    }

    /// Check Liskov Substitution Principle
    /// Look for trait implementations with mismatched signatures
    fn check_lsp(&self, graph: &CallGraph, type_symbol: &cognicode_core::domain::aggregates::Symbol) -> Vec<SolidViolation> {
        let mut violations = Vec::new();

        let id = SymbolId::new(type_symbol.fully_qualified_name());

        // Find if this type implements any traits
        for (dep_id, dep_type) in graph.dependencies(&id) {
            if matches!(dep_type, cognicode_core::domain::value_objects::DependencyType::Inherits) {
                if let Some(dep_symbol) = graph.get_symbol(dep_id) {
                    if dep_symbol.kind() == &SymbolKind::Trait {
                        // Check if methods have consistent signatures
                        let trait_methods = graph.callees(&id);
                        let mut arities: HashSet<usize> = HashSet::new();

                        for (method_id, _) in trait_methods {
                            if let Some(method) = graph.get_symbol(&method_id) {
                                if let Some(sig) = method.signature() {
                                    arities.insert(sig.arity());
                                }
                            }
                        }

                        // If there's high variance in arity, might indicate LSP issues
                        if arities.len() > 5 {
                            violations.push(SolidViolation {
                                principle: SolidPrinciple::LSP,
                                type_name: type_symbol.name().to_string(),
                                severity: Severity::Warning,
                                description: "Trait implementation has high method signature variance".to_string(),
                                suggestion: "Ensure overridden methods maintain consistent contracts".to_string(),
                                evidence: vec![format!("{} different arities", arities.len())],
                            });
                        }
                    }
                }
            }
        }

        violations
    }

    /// Check Interface Segregation Principle
    /// Look for types that implement interfaces but don't use all methods
    fn check_isp(&self, graph: &CallGraph, type_symbol: &cognicode_core::domain::aggregates::Symbol) -> Vec<SolidViolation> {
        let mut violations = Vec::new();

        let id = SymbolId::new(type_symbol.fully_qualified_name());

        // Find trait implementations
        for (dep_id, dep_type) in graph.dependencies(&id) {
            if matches!(dep_type, cognicode_core::domain::value_objects::DependencyType::Inherits) {
                if let Some(dep_symbol) = graph.get_symbol(dep_id) {
                    if dep_symbol.kind() == &SymbolKind::Trait {
                        // Count trait methods vs actual callees
                        let callees: Vec<(SymbolId, cognicode_core::domain::value_objects::DependencyType)> = graph.callees(&id);
                        let callee_ids: HashSet<SymbolId> = callees
                            .iter()
                            .map(|(callee_id, _)| callee_id.clone())
                            .collect();

                        // If this type calls very few methods from the trait interface
                        if callee_ids.is_empty() && graph.fan_out(&id) < 2 {
                            violations.push(SolidViolation {
                                principle: SolidPrinciple::ISP,
                                type_name: type_symbol.name().to_string(),
                                severity: Severity::Info,
                                description: format!("Type implements trait '{}' but may not use its methods", dep_symbol.name()),
                                suggestion: "Consider if the trait is too broad for this type".to_string(),
                                evidence: vec![dep_symbol.name().to_string()],
                            });
                        }
                    }
                }
            }
        }

        violations
    }

    /// Check Dependency Inversion Principle
    /// Look for domain code depending on infrastructure
    fn check_dip(&self, graph: &CallGraph, type_symbol: &cognicode_core::domain::aggregates::Symbol) -> Vec<SolidViolation> {
        let mut violations = Vec::new();

        let type_file = type_symbol.location().file();
        let type_module = Self::module_from_file(type_file);

        // Heuristic: domain modules should not depend on infrastructure modules
        let is_domain = type_module.contains("domain") || type_module.contains("core");

        if is_domain {
            let id = SymbolId::new(type_symbol.fully_qualified_name());
            let callees: Vec<_> = graph.callees(&id);

            for (callee_id, _) in callees {
                if let Some(callee) = graph.get_symbol(&callee_id) {
                    let callee_module = Self::module_from_file(callee.location().file());
                    let callee_is_infra = callee_module.contains("infrastructure")
                        || callee_module.contains("infra")
                        || callee_module.contains("adapter");

                    if callee_is_infra {
                        violations.push(SolidViolation {
                            principle: SolidPrinciple::DIP,
                            type_name: type_symbol.name().to_string(),
                            severity: Severity::Warning,
                            description: "Domain type depends on infrastructure".to_string(),
                            suggestion: "Introduce abstraction layer (use trait instead of concrete type)".to_string(),
                            evidence: vec![format!("-> {}", callee.name())],
                        });
                    }
                }
            }
        }

        violations
    }

    /// Extract module name from file path
    fn module_from_file(file: &str) -> String {
        std::path::Path::new(file)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| file.to_string())
    }

    /// Calculate aggregate SOLID scores
    fn calculate_scores(&self, violations: &[SolidViolation], total_types: usize) -> SolidScores {
        if total_types == 0 {
            return SolidScores {
                srp_score: 0.0,
                ocp_score: 0.0,
                lsp_score: 0.0,
                isp_score: 0.0,
                dip_score: 0.0,
                overall: 0.0,
            };
        }

        let srp_score = Self::principle_score(violations, SolidPrinciple::SRP, total_types);
        let ocp_score = Self::principle_score(violations, SolidPrinciple::OCP, total_types);
        let lsp_score = Self::principle_score(violations, SolidPrinciple::LSP, total_types);
        let isp_score = Self::principle_score(violations, SolidPrinciple::ISP, total_types);
        let dip_score = Self::principle_score(violations, SolidPrinciple::DIP, total_types);

        let overall = (srp_score + ocp_score + lsp_score + isp_score + dip_score) / 5.0;

        SolidScores {
            srp_score,
            ocp_score,
            lsp_score,
            isp_score,
            dip_score,
            overall,
        }
    }

    fn principle_score(violations: &[SolidViolation], principle: SolidPrinciple, total_types: usize) -> f64 {
        let count = violations
            .iter()
            .filter(|v| v.principle == principle)
            .count();

        count as f64 / total_types as f64
    }

    /// Generate a human-readable summary
    fn generate_summary(scores: &SolidScores, violations: &[SolidViolation]) -> String {
        let mut parts = Vec::new();

        if scores.srp_score > 0.3 {
            parts.push(format!("SRP concerns in {:.0}% of types", scores.srp_score * 100.0));
        }
        if scores.dip_score > 0.3 {
            parts.push(format!("DIP concerns in {:.0}% of types", scores.dip_score * 100.0));
        }

        if parts.is_empty() {
            format!(
                "SOLID compliance is good (overall score: {:.1}%). {} violations found.",
                (1.0 - scores.overall) * 100.0,
                violations.len()
            )
        } else {
            format!(
                "{} {} violations total.",
                parts.join("; "),
                violations.len()
            )
        }
    }
}

impl Default for SolidChecker {
    fn default() -> Self {
        Self::new()
    }
}

/// SOLID principle
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SolidPrinciple {
    /// Single Responsibility Principle
    SRP,
    /// Open/Closed Principle
    OCP,
    /// Liskov Substitution Principle
    LSP,
    /// Interface Segregation Principle
    ISP,
    /// Dependency Inversion Principle
    DIP,
}

impl SolidPrinciple {
    /// Human-readable name
    pub fn name(&self) -> &'static str {
        match self {
            SolidPrinciple::SRP => "Single Responsibility Principle (SRP)",
            SolidPrinciple::OCP => "Open/Closed Principle (OCP)",
            SolidPrinciple::LSP => "Liskov Substitution Principle (LSP)",
            SolidPrinciple::ISP => "Interface Segregation Principle (ISP)",
            SolidPrinciple::DIP => "Dependency Inversion Principle (DIP)",
        }
    }
}

impl std::fmt::Display for SolidPrinciple {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// A detected SOLID violation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolidViolation {
    /// The violated principle
    pub principle: SolidPrinciple,
    /// Name of the type with violation
    pub type_name: String,
    /// Severity of the violation
    pub severity: Severity,
    /// Description of the violation
    pub description: String,
    /// Suggested fix
    pub suggestion: String,
    /// Evidence supporting the violation detection
    pub evidence: Vec<String>,
}

/// SOLID principle scores
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolidScores {
    /// SRP violation ratio (0.0 = good, 1.0 = bad)
    pub srp_score: f64,
    /// OCP violation ratio
    pub ocp_score: f64,
    /// LSP violation ratio
    pub lsp_score: f64,
    /// ISP violation ratio
    pub isp_score: f64,
    /// DIP violation ratio
    pub dip_score: f64,
    /// Overall SOLID compliance score
    pub overall: f64,
}

/// Complete SOLID analysis report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolidReport {
    /// Total types analyzed
    pub total_types: usize,
    /// All detected violations
    pub violations: Vec<SolidViolation>,
    /// Principle scores
    pub scores: SolidScores,
    /// Human-readable summary
    pub summary: String,
}

/// Report for a single type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeSolidReport {
    /// Type name
    pub type_name: String,
    /// Violations for this type
    pub violations: Vec<SolidViolation>,
    /// SRP score (LCOM-based)
    pub srp_score: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use cognicode_core::domain::aggregates::CallGraph;

    #[test]
    fn test_checker_new() {
        let checker = SolidChecker::new();
        let graph = CallGraph::new();
        let report = checker.check_all(&graph);
        assert_eq!(report.total_types, 0);
    }

    #[test]
    fn test_principle_name() {
        assert_eq!(SolidPrinciple::SRP.name(), "Single Responsibility Principle (SRP)");
        assert_eq!(SolidPrinciple::DIP.name(), "Dependency Inversion Principle (DIP)");
    }

    #[test]
    fn test_principle_display() {
        assert_eq!(format!("{}", SolidPrinciple::OCP), "Open/Closed Principle (OCP)");
    }

    #[test]
    fn test_solid_scores_serialization() {
        let scores = SolidScores {
            srp_score: 0.2,
            ocp_score: 0.1,
            lsp_score: 0.0,
            isp_score: 0.3,
            dip_score: 0.15,
            overall: 0.15,
        };

        let json = serde_json::to_string(&scores).unwrap();
        let deserialized: SolidScores = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.srp_score, 0.2);
        assert_eq!(deserialized.overall, 0.15);
    }

    #[test]
    fn test_solid_violation_serialization() {
        let violation = SolidViolation {
            principle: SolidPrinciple::SRP,
            type_name: "UserService".to_string(),
            severity: Severity::Warning,
            description: "Low cohesion detected".to_string(),
            suggestion: "Split the type".to_string(),
            evidence: vec!["method1".to_string(), "method2".to_string()],
        };

        let json = serde_json::to_string(&violation).unwrap();
        let deserialized: SolidViolation = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.type_name, "UserService");
        assert_eq!(deserialized.principle, SolidPrinciple::SRP);
    }

    #[test]
    fn test_principle_score_empty() {
        let violations = vec![];
        let score = SolidChecker::principle_score(&violations, SolidPrinciple::SRP, 10);
        assert_eq!(score, 0.0);
    }

    #[test]
    fn test_module_from_file() {
        let path = "src/domain/services/mod.rs";
        let module = SolidChecker::module_from_file(path);
        assert!(module.contains("domain"));
    }
}