//! STYLE_001 — Short variable names (1-2 characters) should be avoided
//!
//! Detects variable names with only 1-2 characters that reduce code readability.
//! Some short names like loop counters (i, j, k) are acceptable by convention.
//!
//! Languages: rust, javascript, python, go, java, typescript
//! Severity: Minor
//! Category: CodeSmell

use crate::rules::types::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use streaming_iterator::StreamingIterator;

/// Whitelist of acceptable short variable names by convention
const ACCEPTABLE_SHORT_NAMES: &[&str] = &[
    // Loop counters
    "i", "j", "k",
    // Coordinates
    "x", "y", "z",
    // Mathematical indices
    "n", "m",
    // Color components
    "r", "g", "b", "a",
    // Common abbreviations that are clear
    "id", "ok", "db", "io", "fn", "us", "vs",
    // Error handling
    "e", "err",
    // Single purpose
    "s", "v", "c",
];

/// Common file extensions that should be allowed
const FILE_EXTENSIONS: &[&str] = &[
    "js", "ts", "py", "go", "rs", "java", "c", "cpp", "h", "hpp",
];

declare_rule! {
    id: "STYLE_001"
    name: "Short variable names (1-2 characters) should be avoided"
    severity: Minor
    category: CodeSmell
    language: "*"
    params: {}

    explanation: "Variable names with only 1-2 characters reduce code readability and maintainability. While some short names like loop counters (i, j, k) are acceptable, most variables should have descriptive names."

    clean_code: Identifiable,
    impacts: [Maintainability: Low],

    check: => {
        let mut issues = Vec::new();
        let source_bytes = ctx.source.as_bytes();

        // Find all identifier nodes in the AST
        // Using tree-sitter query to get identifier nodes
        let query_str = "(identifier) @ident";
        let identifiers = ctx.query_nodes(query_str);

        for ident_node in identifiers {
            let ident_text = match ident_node.utf8_text(source_bytes) {
                Ok(text) => text,
                Err(_) => continue,
            };

            // Check if identifier is 1-2 characters long
            let len = ident_text.len();
            if len == 0 || len > 2 {
                continue;
            }

            // Skip if in acceptable short names list
            if ACCEPTABLE_SHORT_NAMES.contains(&ident_text) {
                continue;
            }

            // Skip file extensions
            if FILE_EXTENSIONS.contains(&ident_text) {
                continue;
            }

            // Context-aware checks: determine if this is a concerning use

            // Get parent node to understand context
            if let Some(parent) = ident_node.parent() {
                let parent_kind = parent.kind();

                // Skip function parameters - they're often short by necessity
                if parent_kind == "parameter" {
                    continue;
                }

                // Skip type parameters (generics) like T, U, V
                if parent_kind == "type_parameter" || parent_kind == "type_identifier" {
                    continue;
                }

                // Skip struct field declarations (but not usages)
                if parent_kind == "field_declaration" {
                    continue;
                }
            }

            // Check for loop counter pattern: for i in ...
            // This checks if the identifier is part of a for loop
            if let Some(grandparent) = ident_node.parent().and_then(|p| p.parent()) {
                let gp_kind = grandparent.kind();
                // Check if this identifier appears in a for_in expression pattern like "for i in"
                if gp_kind == "for_in_expression" {
                    // Check if this identifier is the loop variable (first child of for_in)
                    if let Some(first_child) = grandparent.named_child(0) {
                        if first_child.kind() == "identifier" {
                            let first_ident_text = first_child.utf8_text(source_bytes).unwrap_or("");
                            // If this identifier is the loop variable and it's i, j, or k, skip it
                            if ACCEPTABLE_SHORT_NAMES.contains(&first_ident_text) {
                                continue;
                            }
                        }
                    }
                }
            }

            // Skip identifiers that are part of a path/module reference (like crate::foo)
            if let Some(parent) = ident_node.parent() {
                if parent.kind() == "path_segment" || parent.kind() == "scoped_identifier" {
                    continue;
                }
            }

            // Skip type annotations (like fn foo(x: i32))
            if let Some(parent) = ident_node.parent() {
                if parent.kind() == "type_annotation" {
                    continue;
                }
            }

            // Get line number for the issue
            let line = ident_node.start_position().row + 1;

            issues.push(Issue::new(
                "STYLE_001",
                format!("Short variable name '{}' should be more descriptive", ident_text),
                Severity::Minor,
                Category::CodeSmell,
                ctx.file_path,
                line,
            ).with_column(ident_node.start_position().column + 1)
            .with_remediation(Remediation::moderate(
                "Consider renaming to a more descriptive name that indicates the variable's purpose"
            )));
        }

        issues
    }
}

inventory::submit! {
    RuleEntry {
        factory: || Box::new(STYLE_001Rule::new())
    }
}

/// Agent semantics for STYLE_001 - Short Variable Names
#[derive(Debug, Clone)]
pub struct AgentSemantics {
    pub summary: &'static str,
    pub fix_playbook: &'static str,
    pub review_questions: &'static [&'static str],
    pub agent_actions: &'static [&'static str],
    pub safe_autofix: bool,
}

pub const STYLE_001_AGENT_SEMANTICS: AgentSemantics = AgentSemantics {
    summary: "Detects short variable names (1-2 characters) that reduce code readability - exception for loop counters (i, j, k) and common abbreviations (id, ok, db)",
    fix_playbook: "1. Identify short variable name\n2. Analyze context and purpose of variable\n3. Replace with descriptive name (3+ characters recommended)\n4. Ensure the new name reflects the variable's purpose\n5. Update all references to the renamed variable",
    review_questions: &[
        "Is this a conventional loop counter (i, j, k)?",
        "Is this a common abbreviation (id, ok, db)?",
        "Would a longer name improve understanding of this code?",
        "Is this legacy code that should be refactored?"
    ],
    agent_actions: &[
        "Detect 1-2 character identifier names",
        "Check against whitelist of acceptable short names",
        "Skip function parameters and type parameters",
        "Flag concerning uses for human review"
    ],
    safe_autofix: false,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::types::*;
    use cognicode_core::domain::aggregates::call_graph::CallGraph;
    use cognicode_core::infrastructure::parser::Language;
    use std::path::Path;
    use tree_sitter::Parser as TsParser;

    /// Helper closure to run a test with a RuleContext
    fn with_rule_context<F, R>(source: &str, language: Language, f: F) -> R
    where
        F: FnOnce(&RuleContext) -> R,
    {
        let ts_language = language.to_ts_language();
        let mut parser = TsParser::new();
        parser.set_language(&ts_language).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let graph = CallGraph::new();
        let metrics = FileMetrics::new();
        let symbol_table = crate::rules::symbol_table::SymbolTableBuilder::new()
            .build(&tree, source);

        let ctx = RuleContext {
            tree: &tree,
            source,
            file_path: Path::new("test.rs"),
            language: &language,
            graph: &graph,
            metrics: &metrics,
            symbol_table: Some(&symbol_table),
        };

        f(&ctx)
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Rule Properties Tests
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_style_001_rule_properties() {
        let rule = STYLE_001Rule::new();
        assert_eq!(rule.id(), "STYLE_001");
        assert_eq!(rule.name(), "Short variable names (1-2 characters) should be avoided");
        assert_eq!(rule.severity(), Severity::Minor);
        assert_eq!(rule.category(), Category::CodeSmell);
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Positive Detection Tests — Should trigger the rule
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_style_001_detects_single_char_variable_pt() {
        // 'pt' is a short variable name (not whitelisted like 'x' or 'y')
        let source = "let pt = 5;";
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = STYLE_001Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect short variable 'pt'");
        assert_eq!(issues[0].rule_id, "STYLE_001");
        assert_eq!(issues[0].line, 1);
    }

    #[test]
    fn test_style_001_detects_two_char_variable_mx() {
        let source = "let mx = 10;";
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = STYLE_001Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect short variable 'mx'");
        assert_eq!(issues[0].rule_id, "STYLE_001");
    }

    #[test]
    fn test_style_001_detects_two_char_variable_cn() {
        let source = "fn foo() { let cn = get_count(); }";
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = STYLE_001Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect short variable 'cn'");
    }

    #[test]
    fn test_style_001_detects_single_char_in_js() {
        // 'pt' is not whitelisted (only i,j,k,x,y,z,n,m are whitelisted)
        let source = "const pt = getValue();";
        let issues = with_rule_context(source, Language::JavaScript, |ctx| {
            let rule = STYLE_001Rule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect short variable 'pt' in JS");
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Negative Tests — Should NOT trigger the rule
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_style_001_neg_loop_counter_i() {
        let source = "for i in 0..10 { println!(\"{}\", i); }";
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = STYLE_001Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect loop counter 'i'");
    }

    #[test]
    fn test_style_001_neg_loop_counter_j() {
        let source = "for j in 0..5 { sum += j; }";
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = STYLE_001Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect loop counter 'j'");
    }

    #[test]
    fn test_style_001_neg_loop_counter_k() {
        let source = "for k in 0..3 { process(k); }";
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = STYLE_001Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect loop counter 'k'");
    }

    #[test]
    fn test_style_001_neg_coordinate_x() {
        let source = "let x = point.x;";
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = STYLE_001Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect coordinate 'x'");
    }

    #[test]
    fn test_style_001_neg_coordinate_y() {
        let source = "let y = point.y;";
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = STYLE_001Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect coordinate 'y'");
    }

    #[test]
    fn test_style_001_neg_common_abbrev_id() {
        let source = "let id = user.id;";
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = STYLE_001Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect common abbreviation 'id'");
    }

    #[test]
    fn test_style_001_neg_common_abbrev_ok() {
        let source = "let ok = result.is_ok();";
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = STYLE_001Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect common abbreviation 'ok'");
    }

    #[test]
    fn test_style_001_neg_common_abbrev_db() {
        let source = "let db = get_database();";
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = STYLE_001Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect common abbreviation 'db'");
    }

    #[test]
    fn test_style_001_neg_three_char_name_cnt() {
        let source = "let cnt = count();";
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = STYLE_001Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect 'cnt' (3 chars is acceptable)");
    }

    #[test]
    fn test_style_001_neg_four_char_name_max() {
        let source = "let max = values.iter().max();";
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = STYLE_001Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect 'max' (3+ chars is acceptable)");
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Edge Case Tests
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_style_001_edge_empty_file() {
        let source = "";
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = STYLE_001Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT trigger on empty file");
    }

    #[test]
    fn test_style_001_edge_mathematical_context() {
        // Mathematical function with short param name
        let source = "fn factorial(n: u64) -> u64 { if n <= 1 { 1 } else { n * factorial(n-1) } }";
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = STYLE_001Rule::new();
            rule.check(ctx)
        });
        // Note: Function parameters are skipped by the rule
        assert!(issues.is_empty(), "Should NOT detect function parameter 'n'");
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // False Positive Guard Tests — Should NOT trigger
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_style_001_fp_identifier_in_string() {
        // Variable name 'i' inside string literal should not trigger
        let source = "let s = \"iterator i\";";
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = STYLE_001Rule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect 'i' in string literal");
    }

    #[test]
    fn test_style_001_fp_identifier_in_comment() {
        // Variable 'id' inside comment should not trigger
        let source = "// user_id field";
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = STYLE_001Rule::new();
            rule.check(ctx)
        });
        // Comments are not parsed as identifiers by tree-sitter
        assert!(issues.is_empty(), "Should NOT detect 'id' in comment");
    }

    #[test]
    fn test_style_001_fp_function_type_fn() {
        // Built-in type 'fn' for function pointers - 'fn' should not be flagged
        // but 'f' (the variable) should be flagged since it's short and not whitelisted
        let source = "let f: fn() -> i32 = || 42;";
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = STYLE_001Rule::new();
            rule.check(ctx)
        });
        // 'f' should be flagged (short variable name)
        assert!(!issues.is_empty(), "Should detect short variable 'f'");
        // 'fn' is a type and should not be in the flagged identifiers
        for issue in &issues {
            assert!(!issue.message.contains("'fn'"), "Should NOT detect function type 'fn'");
        }
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Performance Test
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_style_001_perf_multiple_violations() {
        // Multiple short variables in function
        let source = "fn process() { let a = 1; let b = 2; let c = 3; let d = 4; let e = 5; }";
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = STYLE_001Rule::new();
            rule.check(ctx)
        });
        // Should detect multiple violations
        assert!(!issues.is_empty(), "Should detect multiple short variable violations");
    }
}
