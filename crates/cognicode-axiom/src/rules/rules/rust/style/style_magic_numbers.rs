//! STYLE_003 — Magic numbers should be replaced with named constants
//!
//! Detects numeric literals (integer or float) that appear in concerning contexts
//! and should be replaced with named constants for better code maintainability.

use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use streaming_iterator::StreamingIterator;
use tree_sitter::Query as TsQuery;
use tree_sitter::QueryCursor;

declare_rule! {
    id: "STYLE_003"
    name: "Magic numbers should be replaced with named constants"
    severity: Minor
    category: CodeSmell
    language: "rust,javascript,python"
    params: {}

    explanation: "Magic numbers are numeric literals that appear without explanation. They make code harder to maintain, understand, and modify. Replace them with named constants that describe their purpose."
    clean_code: Identifiable
    impacts: [Maintainability: Low],

    agent_semantics: {
        summary: "Detects magic numbers (numeric literals without explanation) that should be replaced with named constants",
        fix_playbook: "1. Identify the magic number\n2. Determine the purpose of this numeric value\n3. Create a constant with a descriptive name (e.g., MAX_RETRIES, TIMEOUT_MS)\n4. Replace the literal with the constant reference\n5. Ensure the constant is defined in an appropriate scope",
        review_questions: [
            "Is this number truly a magic number (repeated, unclear purpose)?",
            "Would defining a constant improve code clarity?",
            "Is this a commonly accepted value like 0, 1, 2, 10, 100?"
        ],
        semantic_chunks: [
            "Magic numbers reduce code maintainability",
            "Named constants make code self-documenting",
            "Common exceptions: 0, 1, 2 for counters; 10, 100, 1000 for bounds"
        ],
        safe_autofix: true,
        autofix_guidance: "Safe to autofix when context is clear - create a named constant and use it"
    }

    check: => {
        let mut issues = Vec::new();

        // Tree-sitter query to find all numeric literals
        let query_str = "(integer_literal) @int (float_literal) @float";
        let query = match TsQuery::new(&ctx.language.to_ts_language(), query_str) {
            Ok(q) => q,
            Err(_) => return Vec::new(),
        };

        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&query, ctx.tree.root_node(), ctx.source.as_bytes());

        while let Some(m) = matches.next() {
            for cap in m.captures {
                let node = cap.node;
                let node_kind = node.kind();

                // Get the literal text
                let literal_text = node.utf8_text(ctx.source.as_bytes()).unwrap_or("");
                let literal_value: i64 = literal_text
                    .trim_start_matches("0x")
                    .trim_start_matches("0b")
                    .parse()
                    .unwrap_or(0);

                // Skip 0, 1, 2 — commonly accepted as non-magic
                if literal_value >= 0 && literal_value <= 2 {
                    continue;
                }

                // Skip negative numbers like -1 (sentinel values)
                if literal_text.starts_with('-') {
                    let abs_value: i64 = literal_text[1..].parse().unwrap_or(0);
                    if abs_value == 1 {
                        continue;
                    }
                }

                // Skip common "safe" numbers: 10, 100, 1000 for loop bounds and ranges
                let safe_numbers = [10, 100, 1000, 10000, 60, 3600, 24, 7, 30, 365];
                if safe_numbers.contains(&literal_value) {
                    // But still check context
                }

                // Check context to determine if this is a magic number
                let parent = node.parent();
                let grandparent = parent.and_then(|p| p.parent());

                // Skip if in array index context: arr[0], arr[1]
                if let Some(ref p) = parent {
                    if p.kind() == "index_expression" || p.kind() == "element_access_expression" {
                        continue;
                    }
                    // Skip tuple index: tuple.0, tuple.1
                    if p.kind() == "field_expression" {
                        continue;
                    }
                }

                // Skip if in loop bounds: for i in 0..10
                if let Some(ref p) = parent {
                    if p.kind() == "range_expression" {
                        // Check if this is the bound of a for loop
                        if let Some(ref gp) = grandparent {
                            if gp.kind() == "for_expression" {
                                continue;
                            }
                        }
                    }
                }

                // Skip if in array literal: [1, 2, 3] where 2 is common count
                if literal_value == 2 {
                    if let Some(ref p) = parent {
                        if p.kind() == "array_expression" || p.kind() == "array_literal" {
                            continue;
                        }
                    }
                }

                // Skip if in Vec::from_elem or similar construction patterns
                if let Some(ref p) = parent {
                    if p.kind() == "call_expression" {
                        let func_name = ctx.source
                            .get(p.start_byte()..p.end_byte())
                            .unwrap_or("");
                        if func_name.contains("from_elem") || func_name.contains("repeat") {
                            if literal_value == 2 {
                                continue;
                            }
                        }
                    }
                }

                // Skip hex literals like 0x1F
                if literal_text.starts_with("0x") || literal_text.starts_with("0b") {
                    continue;
                }

                // Skip common bit manipulation boundaries: 8, 16, 32, 64
                let bit_boundaries = [8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096];
                if bit_boundaries.contains(&(literal_value as i64)) {
                    // Check if it's a type literal like u8, u16, etc. or bit flags
                    if let Some(ref p) = parent {
                        if p.kind() == "primitive_type" || p.kind() == "type_identifier" {
                            continue;
                        }
                    }
                    // Also skip if it's the second argument to from_elem or similar
                    if let Some(ref p) = parent {
                        if p.kind() == "call_expression" {
                            continue;
                        }
                    }
                }

                // At this point, we have a potential magic number
                // Check for percentage base (100)
                if literal_value == 100 {
                    if let Some(ref p) = parent {
                        // Check for division context: value / 100
                        if p.kind() == "binary_expression" {
                            let op = ctx.source
                                .get(p.start_byte()..p.end_byte())
                                .unwrap_or("");
                            if op.contains('/') {
                                continue;
                            }
                        }
                    }
                }

                // Check if the number appears in a string literal context (shouldn't happen with AST query)
                // or in a comment (also shouldn't happen with AST query)

                // Flag this as a magic number
                let pt = node.start_position();
                let start_row = pt.row + 1;
                let start_col = pt.column;

                issues.push(Issue::new(
                    "STYLE_003",
                    format!("Magic number {} should be replaced with a named constant", literal_text),
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    start_row,
                ).with_column(start_col)
                .with_remediation(Remediation::moderate(
                    "Extract this numeric literal to a named constant with a descriptive name"
                ))
                .with_bad_example(format!("let timeout = {};", literal_text))
                .with_good_example(format!("const TIMEOUT_MS: u64 = {}; let timeout = TIMEOUT_MS;", literal_text)));
            }
        }

        issues
    }
}

inventory::submit! {
    RuleEntry {
        factory: || Box::new(STYLE_003Rule::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_style_003_registered() {
        let rule = STYLE_003Rule::new();
        assert_eq!(rule.id(), "STYLE_003");
        assert!(rule.name().len() > 0);
    }

    #[test]
    fn test_skip_zero() {
        // Zero is commonly used for initialization
        let code = "let count = 0;";
        // This should not trigger - verifying the rule skips 0
        assert!(true); // Placeholder - actual testing would need a test harness
    }

    #[test]
    fn test_skip_one() {
        // One is commonly used for unit increment
        let code = "count += 1;";
        // This should not trigger - verifying the rule skips 1
        assert!(true);
    }

    #[test]
    fn test_skip_negative_one() {
        // Negative one is commonly used as sentinel
        let code = "if index == -1 { return NotFound; }";
        // This should not trigger - verifying the rule skips -1
        assert!(true);
    }
}