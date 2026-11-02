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
            function_metrics(&tree, source, function, lang.symbols().function_types);

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
/// call this, so they cannot disagree about the same source (TASK-39). A
/// language declares several function kinds (methods, constructors, arrows);
/// all of them are looked up, not only the first (TASK-42).
pub(crate) fn function_metrics(
    tree: &tree_sitter::Tree,
    source: &str,
    function_name: Option<&str>,
    function_types: &[&str],
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
        function_types,
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
    function_types: &[&str],
    max_nesting: &mut u32,
    decision_points: &mut Vec<crate::domain::services::DecisionPoint>,
    param_count: &mut u32,
    func_start_line: &mut u32,
    func_end_line: &mut u32,
    current_nesting: u32,
) {
    // Check if this is a function definition
    if function_types.contains(&node.kind()) {
        // Get function name
        if let Some(name) = function_name(node, source) {
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
                *param_count = count_parameters(node, source);

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
                function_types,
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

/// The declared name of a function node. The `name` field comes first: the
/// first identifier found by walking the node is the receiver of a Go method
/// or a parameter. An arrow function has no name of its own; it takes the one
/// of the variable it is bound to.
fn function_name(node: tree_sitter::Node, source: &str) -> Option<String> {
    let named = |n: tree_sitter::Node| {
        n.child_by_field_name("name")
            .and_then(|id| id.utf8_text(source.as_bytes()).ok())
            .map(String::from)
    };
    named(node)
        .or_else(|| {
            node.parent()
                .filter(|p| p.kind() == "variable_declarator")
                .and_then(named)
        })
        .or_else(|| naming::identifier_text(&node, source.as_bytes()))
}

/// The node whose children hold the parameter list.
fn parameter_owner(node: tree_sitter::Node) -> tree_sitter::Node {
    let mut current = node;
    while let Some(declarator) = current.child_by_field_name("declarator") {
        if declarator.kind() == "function_declarator" {
            return declarator;
        }
        current = declarator;
    }
    node
}

/// Counts parameters in a function definition.
///
/// Each grammar wraps its parameters in a list node and emits one named child per
/// parameter, under a different kind (`parameter`, `typed_parameter`,
/// `formal_parameter`, ...), so the list is counted by its named children rather
/// than by one kind. Comments and the Python `/` and `*` separators are not
/// parameters. A Go `parameter_declaration` such as `a, b int` declares one name
/// per `name` field.
fn count_parameters(node: tree_sitter::Node, source: &str) -> u32 {
    let mut count = 0u32;
    // C and C++ hang the list off the `function_declarator`, below any pointer
    // declarator; every other grammar keeps it under the function node.
    let owner = parameter_owner(node);
    // A Go method has two `parameter_list`s; the first is its receiver.
    let receiver = node.child_by_field_name("receiver");
    for i in 0..owner.child_count() {
        let Some(list) = owner.child(i) else { continue };
        if Some(list) == receiver {
            continue;
        }
        // Swift puts each `parameter` directly under the function.
        if list.kind() == "parameter" {
            count += 1;
            continue;
        }
        if !matches!(
            list.kind(),
            "parameters" | "formal_parameters" | "parameter_list" | "method_parameters"
        ) {
            continue;
        }
        let mut cursor = list.walk();
        for param in list.named_children(&mut cursor) {
            count += match param.kind() {
                "comment" | "line_comment" | "block_comment" => 0,
                "positional_separator" | "keyword_separator" => 0,
                // `f(void)` declares no parameter: the type stands alone.
                "parameter_declaration"
                    if param.child_by_field_name("declarator").is_none()
                        && param.utf8_text(source.as_bytes()) == Ok("void") =>
                {
                    0
                }
                "parameter_declaration" | "variadic_parameter_declaration" => (param
                    .children_by_field_name("name", &mut param.walk())
                    .count()
                    as u32)
                    .max(1),
                _ => 1,
            };
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

    #[test]
    fn every_declared_parameter_is_counted_whatever_its_grammar_node() {
        // One row per parameter node kind each grammar emits (read off the
        // tree, not assumed): Rust `parameter`/`self_parameter`, Python
        // `typed_parameter`/`default_parameter`/`typed_default_parameter`/
        // `list_splat_pattern`, TS `required_parameter`/`optional_parameter`,
        // Go `parameter_declaration` (one node may declare several names),
        // Java `formal_parameter`, Ruby `optional_parameter`/`splat_parameter`/
        // `keyword_parameter`/`block_parameter`. Separators are not parameters.
        let cases = [
            ("rust", "fn f(a: i32, b: i32) {}\n", 2),
            (
                "rust",
                "struct S;\nimpl S { fn f(&self, a: i32, mut b: i32) {} }\n",
                3,
            ),
            ("python", "def f(a: int, b):\n    pass\n", 2),
            (
                "python",
                "def f(a, b=1, c: int = 2, *args, **kw):\n    pass\n",
                5,
            ),
            ("python", "def f(a, /, b, *, c):\n    pass\n", 3),
            ("javascript", "function f(a, b = 1, ...r) {}\n", 3),
            (
                "typescript",
                "function f(a: number, b?: string, ...r: number[]) {}\n",
                3,
            ),
            ("go", "package main\nfunc f(a, b int, c string) {}\n", 3),
            ("java", "class C { void f(int a, String b) {} }\n", 2),
            ("ruby", "def f(a, b = 1, *r, k:, &blk)\nend\n", 5),
        ];
        let wrong: Vec<String> = cases
            .iter()
            .filter_map(|(language, source, expected)| {
                let got = parameter_count(language, source, "f");
                (got != *expected)
                    .then(|| format!("{language}: expected {expected}, got {got} for {source:?}"))
            })
            .collect();
        assert!(
            wrong.is_empty(),
            "parameter counts disagree:\n{}",
            wrong.join("\n")
        );
    }

    #[test]
    fn a_function_of_any_declared_function_type_is_measured() {
        // (language, source, function, parameters): each function's node kind is
        // NOT the first entry of `function_types`, so a lookup that keeps only
        // the first kind never finds it and reports the defaults (1 line, 0
        // parameters) as if they had been measured.
        let cases = [
            (
                "go",
                "package main\nfunc (r R) f(a, b int) {\n\tif a > 0 {\n\t}\n}\n",
                "f",
                2,
            ),
            (
                "javascript",
                "class C {\n  f(a, b) {\n    if (a) {}\n  }\n}\n",
                "f",
                2,
            ),
            (
                "typescript",
                "class C {\n  f(a: number, b: number) {\n    if (a) {}\n  }\n}\n",
                "f",
                2,
            ),
            (
                "javascript",
                "const h = (a, b) => {\n  if (a) {}\n};\n",
                "h",
                2,
            ),
            (
                "ruby",
                "class C\n  def self.f(a, b)\n    if a\n    end\n  end\nend\n",
                "f",
                2,
            ),
            (
                "java",
                "class C {\n  C(int a, int b) {\n    if (a > 0) {}\n  }\n}\n",
                "C",
                2,
            ),
            (
                "php",
                "<?php\nclass C {\n  function f($a, $b) {\n    if ($a) {}\n  }\n}\n",
                "f",
                2,
            ),
        ];
        for (language, source, function, parameters) in cases {
            let m = TreeSitterComplexity
                .measure(language, source, Some(function))
                .unwrap_or_else(|e| panic!("{language}: measure failed: {e}"));
            assert_eq!(
                m.parameter_count, parameters,
                "{language} {function}: parameters"
            );
            assert!(
                m.lines_of_code > 1,
                "{language} {function}: the function was not found"
            );
        }
    }

    #[test]
    fn the_first_function_type_keeps_its_measurement() {
        let m = TreeSitterComplexity
            .measure(
                "go",
                "package main\nfunc f(a, b int) {\n\tif a > 0 {\n\t}\n}\n",
                Some("f"),
            )
            .unwrap();
        assert_eq!((m.parameter_count, m.cyclomatic), (2, 2));
    }

    #[test]
    fn parameters_not_hanging_from_the_function_node_are_counted() {
        // C and C++ nest the list under `function_declarator`; Swift puts each
        // `parameter` directly under the function. A count that only looks at
        // known list children under the function reports 0 as if measured.
        let cases = [
            ("c", "int f(int a, int b) {\n  return a;\n}\n", 2),
            ("c++", "int f(int a, int b) {\n  return a;\n}\n", 2),
            (
                "swift",
                "func f(a: Int, b: Int) -> Int {\n  return a\n}\n",
                2,
            ),
            ("c", "int f(void) {\n  return 0;\n}\n", 0),
            ("c", "int f() {\n  return 0;\n}\n", 0),
            ("swift", "func f() {\n}\n", 0),
        ];
        for (language, source, parameters) in cases {
            assert_eq!(
                parameter_count(language, source, "f"),
                parameters,
                "{language}: {source}"
            );
        }
    }
}
