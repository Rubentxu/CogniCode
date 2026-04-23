//! Service for calculating code complexity metrics
//!
//! Provides cyclomatic complexity calculation and other code quality metrics.

/// Service for calculating code complexity
pub struct ComplexityCalculator;

impl ComplexityCalculator {
    /// Creates a new ComplexityCalculator
    pub fn new() -> Self {
        Self
    }

    /// Calculates the cyclomatic complexity of a function
    ///
    /// Cyclomatic Complexity = M = E - N + 2P
    /// Where:
    /// - E = number of edges in the control flow graph
    /// - N = number of nodes in the graph
    /// - P = number of connected components (usually 1 for a function)
    ///
    /// Simplified: M = number of decision points + 1
    pub fn cyclomatic_complexity(
        &self,
        decision_points: &[DecisionPoint],
        exit_points: usize,
    ) -> u32 {
        if decision_points.is_empty() && exit_points <= 1 {
            return 1;
        }

        // M = E - N + 2P
        // For a function with D decision points and E exit points:
        // M = D + E + 1 (simplified formula)
        let base_complexity = decision_points.len() as u32 + 1;

        // Additional complexity for multiple exit points
        if exit_points > 1 {
            base_complexity + (exit_points as u32 - 1)
        } else {
            base_complexity
        }
    }

    /// Calculates cyclomatic complexity from control flow graph edges and nodes
    pub fn from_graph(edges: usize, nodes: usize, components: usize) -> u32 {
        if nodes == 0 {
            return 0;
        }
        let e = edges as i32;
        let n = nodes as i32;
        let p = components as i32;
        ((e - n + 2 * p).max(1)) as u32
    }

    /// Categorizes complexity into risk levels
    pub fn risk_level(&self, complexity: u32) -> ComplexityRisk {
        match complexity {
            1..=10 => ComplexityRisk::Low,
            11..=20 => ComplexityRisk::Moderate,
            21..=50 => ComplexityRisk::High,
            _ => ComplexityRisk::VeryHigh,
        }
    }

    /// Calculates cognitive complexity (a more modern alternative)
    /// This is a simplified version based on the main principles
    pub fn cognitive_complexity(
        &self,
        nesting_depth: u32,
        decision_points: &[DecisionPoint],
        recursion_depth: u32,
    ) -> u32 {
        let mut complexity = 0;

        // Increments for nesting
        for dp in decision_points.iter() {
            // Structural complexity increases with nesting
            let nesting_bonus = std::cmp::min(nesting_depth, 3) as u32;
            let base = match dp {
                DecisionPoint::If | DecisionPoint::ElseIf => 1,
                DecisionPoint::While | DecisionPoint::For => 1,
                DecisionPoint::Match => 1,
                DecisionPoint::And | DecisionPoint::Or => 1,
                DecisionPoint::Ternary => 1,
                DecisionPoint::Catch => 1,
                DecisionPoint::When => 1,
            };
            complexity += base + nesting_bonus;
        }

        // Recursion adds significant complexity
        if recursion_depth > 0 {
            complexity += recursion_depth * 2;
        }

        complexity.max(1)
    }
}

impl Default for ComplexityCalculator {
    fn default() -> Self {
        Self::new()
    }
}

/// Types of decision points that increase complexity
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DecisionPoint {
    /// if statement
    If,
    /// else if (increases complexity)
    ElseIf,
    /// while loop
    While,
    /// for loop
    For,
    /// match expression
    Match,
    /// logical AND (&&)
    And,
    /// logical OR (||)
    Or,
    /// ternary operator
    Ternary,
    /// catch clause
    Catch,
    /// when clause (Kotlin/Rust)
    When,
}

impl DecisionPoint {
    /// Returns the base complexity increment for this decision point
    pub fn base_increment(&self) -> u32 {
        match self {
            DecisionPoint::If => 1,
            DecisionPoint::ElseIf => 1,
            DecisionPoint::While => 1,
            DecisionPoint::For => 1,
            DecisionPoint::Match => 1,
            DecisionPoint::And => 1,
            DecisionPoint::Or => 1,
            DecisionPoint::Ternary => 1,
            DecisionPoint::Catch => 1,
            DecisionPoint::When => 1,
        }
    }
}

/// Risk level based on complexity
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ComplexityRisk {
    /// Low risk (1-10 complexity)
    Low,
    /// Moderate risk (11-20 complexity)
    Moderate,
    /// High risk (21-50 complexity)
    High,
    /// Very high risk (>50 complexity)
    VeryHigh,
}

impl ComplexityRisk {
    /// Returns a description of this risk level
    pub fn description(&self) -> &'static str {
        match self {
            ComplexityRisk::Low => "Low risk - simple, well-structured code",
            ComplexityRisk::Moderate => "Moderate risk - some complexity, consider refactoring",
            ComplexityRisk::High => "High risk - complex code, testing should be thorough",
            ComplexityRisk::VeryHigh => {
                "Very high risk - highly complex, urgent refactoring recommended"
            }
        }
    }

    /// Returns the recommended maximum complexity
    pub fn recommended_max(&self) -> u32 {
        match self {
            ComplexityRisk::Low => 10,
            ComplexityRisk::Moderate => 20,
            ComplexityRisk::High => 50,
            ComplexityRisk::VeryHigh => 100,
        }
    }
}

impl fmt::Display for ComplexityRisk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// Complete complexity report for a symbol
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComplexityReport {
    /// Symbol identifier
    pub symbol_name: String,
    /// Cyclomatic complexity
    pub cyclomatic: u32,
    /// Cognitive complexity
    pub cognitive: u32,
    /// Number of decision points
    pub decision_point_count: usize,
    /// Nesting depth
    pub max_nesting_depth: u32,
    /// Number of exit points
    pub exit_point_count: usize,
    /// Risk assessment
    pub risk: ComplexityRisk,
}

impl ComplexityReport {
    /// Returns true if this complexity is within acceptable limits
    pub fn is_acceptable(&self, threshold: u32) -> bool {
        self.cyclomatic <= threshold
    }

    /// Returns a summary string
    pub fn summary(&self) -> String {
        format!(
            "{}: cyclomatic={}, cognitive={}, risk={}",
            self.symbol_name, self.cyclomatic, self.cognitive, self.risk
        )
    }
}

/// Control flow graph node
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CFGNode {
    pub id: u32,
    pub node_type: CFGNodeType,
}

impl CFGNode {
    pub fn new(id: u32, node_type: CFGNodeType) -> Self {
        Self { id, node_type }
    }
}

/// Types of nodes in a control flow graph
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CFGNodeType {
    /// Entry node
    Entry,
    /// Exit node
    Exit,
    /// Regular statement
    Statement,
    /// Decision point (if, while, etc.)
    Decision,
    /// Compound node (multiple statements)
    Compound,
}

use std::fmt;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cyclomatic_simple_function() {
        let calculator = ComplexityCalculator::new();

        // Simple function with no decision points
        let complexity = calculator.cyclomatic_complexity(&[], 1);
        assert_eq!(complexity, 1);
    }

    #[test]
    fn test_cyclomatic_with_single_if() {
        let calculator = ComplexityCalculator::new();

        // Function with one if statement
        let decision_points = vec![DecisionPoint::If];
        let complexity = calculator.cyclomatic_complexity(&decision_points, 1);
        assert_eq!(complexity, 2); // 1 (base) + 1 (if)
    }

    #[test]
    fn test_cyclomatic_with_multiple_decisions() {
        let calculator = ComplexityCalculator::new();

        // Function with if, while, and for
        let decision_points = vec![DecisionPoint::If, DecisionPoint::While, DecisionPoint::For];
        let complexity = calculator.cyclomatic_complexity(&decision_points, 1);
        assert_eq!(complexity, 4); // 1 (base) + 3 (decision points)
    }

    #[test]
    fn test_cyclomatic_with_multiple_exits() {
        let calculator = ComplexityCalculator::new();

        // Function with multiple exit points
        let decision_points = vec![DecisionPoint::If];
        let complexity = calculator.cyclomatic_complexity(&decision_points, 3);
        assert_eq!(complexity, 4); // 1 (base) + 1 (if) + (3-1) additional for multiple exits
    }

    #[test]
    fn test_risk_level() {
        let calculator = ComplexityCalculator::new();

        assert_eq!(calculator.risk_level(5), ComplexityRisk::Low);
        assert_eq!(calculator.risk_level(15), ComplexityRisk::Moderate);
        assert_eq!(calculator.risk_level(30), ComplexityRisk::High);
        assert_eq!(calculator.risk_level(60), ComplexityRisk::VeryHigh);
    }

    #[test]
    fn test_from_graph() {
        // Linear graph: Entry -> Statement -> Exit
        // 2 edges, 3 nodes, 1 component
        let complexity = ComplexityCalculator::from_graph(2, 3, 1);
        assert_eq!(complexity, 1);

        // Graph with one decision: Entry -> Decision -> Exit (x2)
        // 4 edges, 4 nodes, 1 component
        let complexity = ComplexityCalculator::from_graph(4, 4, 1);
        assert_eq!(complexity, 2);
    }

    #[test]
    fn test_cognitive_complexity() {
        let calculator = ComplexityCalculator::new();

        // Simple function
        let cognitive = calculator.cognitive_complexity(0, &[], 0);
        assert_eq!(cognitive, 1);

        // Function with nested decisions
        let decision_points = vec![DecisionPoint::If, DecisionPoint::While, DecisionPoint::For];
        let cognitive = calculator.cognitive_complexity(2, &decision_points, 0);
        assert!(cognitive > 3); // Should be higher due to nesting
    }

    #[test]
    fn test_decision_point_increment() {
        assert_eq!(DecisionPoint::If.base_increment(), 1);
        assert_eq!(DecisionPoint::While.base_increment(), 1);
        assert_eq!(DecisionPoint::And.base_increment(), 1);
        assert_eq!(DecisionPoint::Catch.base_increment(), 1);
    }

    #[test]
    fn test_complexity_report() {
        let report = ComplexityReport {
            symbol_name: "test_function".to_string(),
            cyclomatic: 15,
            cognitive: 12,
            decision_point_count: 5,
            max_nesting_depth: 3,
            exit_point_count: 2,
            risk: ComplexityRisk::Moderate,
        };

        assert!(report.is_acceptable(20));
        assert!(!report.is_acceptable(10));
        assert!(report.summary().contains("test_function"));
    }

    #[test]
    fn test_cfg_node() {
        let entry = CFGNode::new(1, CFGNodeType::Entry);
        assert_eq!(entry.id, 1);
        assert_eq!(entry.node_type, CFGNodeType::Entry);

        let decision = CFGNode::new(2, CFGNodeType::Decision);
        assert_eq!(decision.node_type, CFGNodeType::Decision);
    }
}
