//! Complexity measurement over source text.
//!
//! This is where the tree walk behind `get_complexity` came to live. It used to
//! be four `&self` methods on `WorkspaceSession`, each taking a
//! `tree_sitter::Node`, called after the session built a `TreeSitterParser`
//! itself.
//!
//! Putting a parser behind a `dyn` would not have fixed that. The signatures
//! still said `tree_sitter::Node`, so application would have been walking
//! tree-sitter's AST while claiming to know only the port. The walk *is* the
//! coupling, so the walk moved and the port returns numbers.
//!
//! The language arrives as a lower-case name for the same reason
//! `SyntaxAnalysis` takes one: the application classifies a file by extension
//! and states what it concluded, instead of handing over a path for the adapter
//! to classify a second time.

use crate::application::dto::ComplexityResult;
use crate::application::ports::ComplexityAnalysis;
use crate::application::{AppError, AppResult};
use crate::domain::services::ComplexityCalculator;
use crate::domain::value_objects::Language;
use crate::infrastructure::parser::TreeSitterParser;
use crate::infrastructure::parser::naming;

/// Measures cyclomatic, cognitive and structural complexity of source text.
#[derive(Debug, Default, Clone, Copy)]
pub struct TreeSitterComplexity;

impl ComplexityAnalysis for TreeSitterComplexity {
    fn measure(
        &self,
        language: &str,
        source: &str,
        function: Option<&str>,
    ) -> AppResult<ComplexityResult> {
        let lang = Language::all_languages()
            .iter()
            .copied()
            .find(|l| l.name().eq_ignore_ascii_case(language))
            .ok_or_else(|| {
                AppError::InvalidParameter(format!("Unsupported language: {language}"))
            })?;

        let parser =
            TreeSitterParser::new(lang).map_err(|e| AppError::AnalysisError(e.to_string()))?;
        let tree = parser
            .parse_tree(source)
            .map_err(|e| AppError::AnalysisError(format!("Parse error: {e}")))?;

        let (cyclomatic, cognitive, max_nesting, param_count, lines_of_code) =
            function_metrics(&tree, source, function, lang.function_node_type());

        Ok(ComplexityResult {
            cyclomatic,
            cognitive,
            lines_of_code,
            parameter_count: param_count,
            nesting_depth: max_nesting,
            function_name: function.map(String::from),
        })
    }
}

/// The one measurement of a function: `(cyclomatic, cognitive, nesting_depth,
/// parameter_count, lines_of_code)`. The CLI/rig port and the MCP handler both
/// call this, so they cannot disagree about the same source (TASK-39).
pub(crate) fn function_metrics(
    tree: &tree_sitter::Tree,
    source: &str,
    function_name: Option<&str>,
    function_type: &str,
) -> (u32, u32, u32, u32, u32) {
    let calculator = ComplexityCalculator::new();
    let mut max_nesting = 0u32;
    let mut decision_points = Vec::new();
    let mut param_count = 0u32;
    let mut func_start_line = 0u32;
    let mut func_end_line = 0u32;

    find_function_metrics(
        tree.root_node(),
        source,
        function_name,
        function_type,
        &mut max_nesting,
        &mut decision_points,
        &mut param_count,
        &mut func_start_line,
        &mut func_end_line,
        0,
    );

    let cyclomatic = calculator.cyclomatic_complexity(&decision_points, 1);
    let cognitive = calculator.cognitive_complexity(max_nesting, &decision_points, 0);
    let lines_of_code = if func_end_line > func_start_line {
        func_end_line - func_start_line
    } else {
        1
    };
    (
        cyclomatic,
        cognitive,
        max_nesting,
        param_count,
        lines_of_code,
    )
}

/// Recursively finds a function and its metrics
#[allow(clippy::too_many_arguments)]
fn find_function_metrics(
    node: tree_sitter::Node,
    source: &str,
    target_name: Option<&str>,
    function_type: &str,
    max_nesting: &mut u32,
    decision_points: &mut Vec<crate::domain::services::DecisionPoint>,
    param_count: &mut u32,
    func_start_line: &mut u32,
    func_end_line: &mut u32,
    current_nesting: u32,
) {
    // Check if this is a function definition
    if node.kind() == function_type {
        // Get function name
        if let Some(name) = naming::identifier_text(&node, source.as_bytes()) {
            // If we have a target name, only process that function
            // Otherwise, process the first/only function
            let should_process = match target_name {
                Some(target) => name == target,
                None => *func_start_line == 0, // Process first function if no target
            };

            if should_process {
                *func_start_line = node.start_position().row as u32;
                *func_end_line = node.end_position().row as u32;

                // Count parameters
                *param_count = count_parameters(node);

                // Process function body for decision points
                process_decision_points(
                    node,
                    source,
                    max_nesting,
                    decision_points,
                    current_nesting,
                );
            }
        }
    }

    // Recurse into children
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            find_function_metrics(
                child,
                source,
                target_name,
                function_type,
                max_nesting,
                decision_points,
                param_count,
                func_start_line,
                func_end_line,
                current_nesting,
            );
        }
    }
}

/// Counts parameters in a function definition
fn count_parameters(node: tree_sitter::Node) -> u32 {
    let mut count = 0u32;
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            // Handle Python parameters (within parentheses after function name)
            if child.kind() == "parameters" {
                for j in 0..child.child_count() {
                    if let Some(param) = child.child(j)
                        && param.kind() == "identifier"
                    {
                        count += 1;
                    }
                }
            }
        }
    }
    count
}

/// Processes a node tree to find decision points
fn process_decision_points(
    node: tree_sitter::Node,
    source: &str,
    max_nesting: &mut u32,
    decision_points: &mut Vec<crate::domain::services::DecisionPoint>,
    current_nesting: u32,
) {
    let kind = node.kind();

    // Check for decision points
    match kind {
        "if_statement" | "if_expression" => {
            decision_points.push(crate::domain::services::DecisionPoint::If);
            *max_nesting = (*max_nesting).max(current_nesting + 1);
        }
        "elif_clause" | "else_if_clause" => {
            decision_points.push(crate::domain::services::DecisionPoint::ElseIf);
        }
        "while_statement" | "while_expression" => {
            decision_points.push(crate::domain::services::DecisionPoint::While);
            *max_nesting = (*max_nesting).max(current_nesting + 1);
        }
        "for_statement" | "for_expression" | "for_in_statement" => {
            decision_points.push(crate::domain::services::DecisionPoint::For);
            *max_nesting = (*max_nesting).max(current_nesting + 1);
        }
        "match_expression" | "match_statement" | "switch_statement" => {
            decision_points.push(crate::domain::services::DecisionPoint::Match);
            *max_nesting = (*max_nesting).max(current_nesting + 1);
        }
        "binary_expression" => {
            // Check for && or ||
            if let Ok(text) = node.utf8_text(source.as_bytes()) {
                if text.contains("&&") {
                    decision_points.push(crate::domain::services::DecisionPoint::And);
                }
                if text.contains("||") {
                    decision_points.push(crate::domain::services::DecisionPoint::Or);
                }
            }
        }
        "conditional_expression" | "ternary_expression" => {
            decision_points.push(crate::domain::services::DecisionPoint::Ternary);
        }
        "catch_clause" | "except_clause" => {
            decision_points.push(crate::domain::services::DecisionPoint::Catch);
        }
        _ => {}
    }

    // Recurse into children with updated nesting
    let child_nesting = if matches!(
        kind,
        "if_statement"
            | "if_expression"
            | "elif_clause"
            | "else_if_clause"
            | "while_statement"
            | "while_expression"
            | "for_statement"
            | "for_expression"
            | "for_in_statement"
            | "match_expression"
            | "match_statement"
            | "switch_statement"
    ) {
        current_nesting + 1
    } else {
        current_nesting
    };

    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            process_decision_points(child, source, max_nesting, decision_points, child_nesting);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parameter_count(language: &str, source: &str, function: &str) -> u32 {
        TreeSitterComplexity
            .measure(language, source, Some(function))
            .unwrap_or_else(|e| panic!("{language}: measure failed: {e}"))
            .parameter_count
    }

    #[test]
    fn python_parameter_count_excludes_the_function_name() {
        assert_eq!(
            parameter_count("python", "def f(a, b):\n    pass\n", "f"),
            2
        );
    }

    #[test]
    fn a_function_without_parameters_reports_zero_in_every_language() {
        // The name of a function is never one of its parameters. With no
        // parameters declared, any non-zero count is the name being counted.
        let cases = [
            ("rust", "fn f() {}\n"),
            ("python", "def f():\n    pass\n"),
            ("javascript", "function f() {}\n"),
            ("typescript", "function f() {}\n"),
            ("go", "package main\nfunc f() {}\n"),
            ("java", "class C { void f() {} }\n"),
            ("ruby", "def f\nend\n"),
        ];
        for (language, source) in cases {
            assert_eq!(
                parameter_count(language, source, "f"),
                0,
                "{language}: the function name was counted as a parameter"
            );
        }
    }
}
