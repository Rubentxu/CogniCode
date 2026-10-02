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

        let calculator = ComplexityCalculator::new();
        let mut max_nesting = 0u32;
        let mut decision_points = Vec::new();
        let mut param_count = 0u32;
        let mut func_start_line = 0u32;
        let mut func_end_line = 0u32;

        find_function_metrics(
            tree.root_node(),
            source,
            function,
            lang.function_node_type(),
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
    if node.kind() == function_type
        && let Some(name) = find_identifier_in_node(node, source)
    {
        let should_process = match target_name {
            Some(target) => name == target,
            None => *func_start_line == 0,
        };

        if should_process {
            *func_start_line = node.start_position().row as u32;
            *func_end_line = node.end_position().row as u32;
            *param_count = count_parameters(node);
            process_decision_points(node, max_nesting, decision_points, current_nesting);
        }
    }

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

fn find_identifier_in_node(node: tree_sitter::Node, source: &str) -> Option<String> {
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            if child.kind() == "identifier" || child.kind() == "type_identifier" {
                return Some(child.utf8_text(source.as_bytes()).unwrap_or("").to_string());
            }
            if let Some(id) = find_identifier_in_node(child, source) {
                return Some(id);
            }
        }
    }
    None
}

fn count_parameters(node: tree_sitter::Node) -> u32 {
    let mut count = 0u32;
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            if child.kind() == "parameters" {
                for j in 0..child.child_count() {
                    if let Some(param) = child.child(j)
                        && param.kind() == "identifier"
                    {
                        count += 1;
                    }
                }
            }
            if child.kind() == "identifier" {
                count += 1;
            }
        }
    }
    count
}

fn process_decision_points(
    node: tree_sitter::Node,
    max_nesting: &mut u32,
    decision_points: &mut Vec<crate::domain::services::DecisionPoint>,
    current_nesting: u32,
) {
    let kind = node.kind();

    match kind {
        "if_statement" | "if_expression" => {
            decision_points.push(crate::domain::services::DecisionPoint::If);
            *max_nesting = (*max_nesting).max(current_nesting + 1);
        }
        "while_statement" | "while_expression" => {
            decision_points.push(crate::domain::services::DecisionPoint::While);
            *max_nesting = (*max_nesting).max(current_nesting + 1);
        }
        "for_statement" | "for_in_statement" => {
            decision_points.push(crate::domain::services::DecisionPoint::For);
            *max_nesting = (*max_nesting).max(current_nesting + 1);
        }
        "case_clause" | "match_expression" => {
            decision_points.push(crate::domain::services::DecisionPoint::Match);
            *max_nesting = (*max_nesting).max(current_nesting + 1);
        }
        _ => {}
    }

    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            process_decision_points(child, max_nesting, decision_points, current_nesting);
        }
    }
}
