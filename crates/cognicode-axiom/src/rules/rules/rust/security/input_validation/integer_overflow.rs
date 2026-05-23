//! S1155 — Integer Overflow in Size Calculation Detection
//! Detects arithmetic on sizes/lengths without overflow checking (CWE-190).
//!
//! Languages: Rust
//! Severity: Major
//! Category: Vulnerability
use crate::rules::types::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use std::sync::LazyLock;

/// Rule constant for S1155
const RULE_ID: &str = "S1155";
const RULE_NAME: &str = "Integer overflow in size calculation detected";
const SEVERITY: Severity = Severity::Major;
const CATEGORY: Category = Category::Vulnerability;

// ═══════════════════════════════════════════════════════════════════════════════
// Cached Regex Patterns
// ═══════════════════════════════════════════════════════════════════════════════

/// Pattern for vec! macro with size calculation
static VEC_SIZE_PATTERN: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r#"vec!\s*\[\s*[^;]*;\s*[^;]*(?:\*\s*|\+\s*|\-\s*|\/\s*)"#).unwrap()
});

/// Pattern for Vec::with_capacity with multiplication
static VEC_CAPACITY_PATTERN: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r#"Vec::with_capacity\s*\([^)]*(?:\*\s*|\+\s*|\-\s*|\/\s*)[^)]*\)"#).unwrap()
});

/// Pattern for Vec::resize with size calculation
static VEC_RESIZE_PATTERN: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r#"\.resize\s*\([^)]*(?:\*\s*|\+\s*|\-\s*|\/\s*)[^)]*\)"#).unwrap()
});

/// Pattern for Box::new with size calculation
static BOX_NEW_PATTERN: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r#"Box::new\s*\([^)]*(?:\*\s*|\+\s*|\-\s*|\/\s*)[^)]*\)"#).unwrap()
});

/// Pattern for slice::from_raw_parts with size calculation
static FROM_RAW_PARTS_PATTERN: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r#"slice::from_raw_parts\s*\([^)]*(?:\*\s*|\+\s*|\-\s*|\/\s*)[^)]*\)"#).unwrap()
});

/// Pattern for alloc with size calculation
static ALLOC_PATTERN: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r#"alloc\s*\([^)]*(?:\*\s*|\+\s*|\-\s*|\/\s*)[^)]*\)"#).unwrap()
});

/// Pattern for user-controlled size variables
static USER_SIZE_PATTERN: LazyLock<Vec<regex::Regex>> = LazyLock::new(|| {
    vec![
        regex::Regex::new(r#"(?i)(?:user|input|count|size|len|length|height|width|num|quantity|amount|total|index)"#).unwrap(),
    ]
});

/// Pattern for size_of operations
static SIZEOF_PATTERN: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r#"size_of\s*<\s*\w+\s*>\s*\(\s*\)"#).unwrap()
});

/// Pattern for checked/checked_mul/checked_add operations (overflow-safe)
static OVERFLOW_SAFE_PATTERN: LazyLock<Vec<regex::Regex>> = LazyLock::new(|| {
    vec![
        regex::Regex::new(r#"(?:checked|saturating|wrapping)_(?:mul|add|sub|div)"#).unwrap(),
    ]
});

declare_rule! {
    id: "S1155"
    name: "Integer overflow in size calculation detected"
    severity: Major
    category: Vulnerability
    language: "Rust"
    params: {}

    explanation: "Integer overflow in size calculations can lead to memory corruption, allocation failures, or exploitable vulnerabilities. When user-controlled values are used in size calculations for allocation, attackers can trigger overflows to cause undersized allocations or other memory safety issues."
    clean_code: Trustworthy,
    impacts: [Security: High],
    check: => {
        let mut issues = Vec::new();

        for (line_idx, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();

            // Skip comments
            if trimmed.starts_with("//") || trimmed.starts_with("///")
               || trimmed.starts_with("//!") || trimmed.starts_with("/*")
               || trimmed.starts_with("#") {
                continue;
            }

            // Check for vec! with size calculation
            if VEC_SIZE_PATTERN.is_match(trimmed) {
                let context: String = (0..3)
                    .filter_map(|i| ctx.source.lines().nth(line_idx + i))
                    .take(4)
                    .collect::<Vec<_>>()
                    .join("\n");

                // Check for user-controlled values in the size expression
                if USER_SIZE_PATTERN.iter().any(|re| re.is_match(&context)) {
                    // Check if using overflow-safe operations
                    if !OVERFLOW_SAFE_PATTERN.iter().any(|re| re.is_match(&context)) {
                        issues.push(Issue::new(
                            RULE_ID,
                            "Potential integer overflow: user-controlled size in vec! macro without overflow checking.".to_string(),
                            SEVERITY,
                            CATEGORY,
                            ctx.file_path,
                            line_idx + 1,
                        ).with_remediation(Remediation::substantial(
                            "Use checked_mul, checked_add, or saturating_* for size calculations with user input. Alternatively, validate that the product does not overflow before allocation."
                        )));
                    }
                }
            }

            // Check for Vec::with_capacity with size calculation
            if VEC_CAPACITY_PATTERN.is_match(trimmed) {
                let context: String = (0..3)
                    .filter_map(|i| ctx.source.lines().nth(line_idx + i))
                    .take(4)
                    .collect::<Vec<_>>()
                    .join("\n");

                // Check for user-controlled values
                if USER_SIZE_PATTERN.iter().any(|re| re.is_match(&context)) {
                    if !OVERFLOW_SAFE_PATTERN.iter().any(|re| re.is_match(&context)) {
                        issues.push(Issue::new(
                            RULE_ID,
                            "Potential integer overflow: user-controlled capacity calculation without overflow checking.".to_string(),
                            SEVERITY,
                            CATEGORY,
                            ctx.file_path,
                            line_idx + 1,
                        ).with_remediation(Remediation::substantial(
                            "Use checked_mul or saturating_mul for capacity calculations. Validate that the result fits in usize before allocation."
                        )));
                    }
                }
            }

            // Check for Vec::resize with size calculation
            if VEC_RESIZE_PATTERN.is_match(trimmed) {
                let context: String = (0..3)
                    .filter_map(|i| ctx.source.lines().nth(line_idx + i))
                    .take(4)
                    .collect::<Vec<_>>()
                    .join("\n");

                if USER_SIZE_PATTERN.iter().any(|re| re.is_match(&context)) {
                    if !OVERFLOW_SAFE_PATTERN.iter().any(|re| re.is_match(&context)) {
                        issues.push(Issue::new(
                            RULE_ID,
                            "Potential integer overflow: user-controlled size in Vec::resize without overflow checking.".to_string(),
                            SEVERITY,
                            CATEGORY,
                            ctx.file_path,
                            line_idx + 1,
                        ).with_remediation(Remediation::substantial(
                            "Use checked_mul for resize size calculations or validate input does not cause overflow."
                        )));
                    }
                }
            }

            // Check for Box::new with size calculation
            if BOX_NEW_PATTERN.is_match(trimmed) {
                let context: String = (0..3)
                    .filter_map(|i| ctx.source.lines().nth(line_idx + i))
                    .take(4)
                    .collect::<Vec<_>>()
                    .join("\n");

                if USER_SIZE_PATTERN.iter().any(|re| re.is_match(&context)) {
                    if !OVERFLOW_SAFE_PATTERN.iter().any(|re| re.is_match(&context)) {
                        issues.push(Issue::new(
                            RULE_ID,
                            "Potential integer overflow: user-controlled size in Box::new without overflow checking.".to_string(),
                            SEVERITY,
                            CATEGORY,
                            ctx.file_path,
                            line_idx + 1,
                        ).with_remediation(Remediation::substantial(
                            "Use checked operations for size calculations before Box allocation."
                        )));
                    }
                }
            }

            // Check for slice::from_raw_parts with size calculation
            if FROM_RAW_PARTS_PATTERN.is_match(trimmed) {
                let context: String = (0..3)
                    .filter_map(|i| ctx.source.lines().nth(line_idx + i))
                    .take(4)
                    .collect::<Vec<_>>()
                    .join("\n");

                if USER_SIZE_PATTERN.iter().any(|re| re.is_match(&context)) {
                    if !OVERFLOW_SAFE_PATTERN.iter().any(|re| re.is_match(&context)) {
                        issues.push(Issue::new(
                            RULE_ID,
                            "Potential integer overflow: user-controlled count in slice::from_raw_parts without overflow checking.".to_string(),
                            SEVERITY,
                            CATEGORY,
                            ctx.file_path,
                            line_idx + 1,
                        ).with_remediation(Remediation::substantial(
                            "Validate count and stride multiplication does not overflow before creating slice from raw pointer."
                        )));
                    }
                }
            }

            // Check for alloc with size calculation
            if ALLOC_PATTERN.is_match(trimmed) {
                let context: String = (0..3)
                    .filter_map(|i| ctx.source.lines().nth(line_idx + i))
                    .take(4)
                    .collect::<Vec<_>>()
                    .join("\n");

                if USER_SIZE_PATTERN.iter().any(|re| re.is_match(&context)) {
                    if !OVERFLOW_SAFE_PATTERN.iter().any(|re| re.is_match(&context)) {
                        issues.push(Issue::new(
                            RULE_ID,
                            "Potential integer overflow: user-controlled size in alloc without overflow checking.".to_string(),
                            SEVERITY,
                            CATEGORY,
                            ctx.file_path,
                            line_idx + 1,
                        ).with_remediation(Remediation::substantial(
                            "Use Layout::from_size_align with checked size calculation or validated allocation size."
                        )));
                    }
                }
            }

            // Check for size_of with multiplication that could overflow
            if SIZEOF_PATTERN.is_match(trimmed) && trimmed.contains("*") {
                let context: String = (0..3)
                    .filter_map(|i| ctx.source.lines().nth(line_idx + i))
                    .take(4)
                    .collect::<Vec<_>>()
                    .join("\n");

                if USER_SIZE_PATTERN.iter().any(|re| re.is_match(&context)) {
                    if !OVERFLOW_SAFE_PATTERN.iter().any(|re| re.is_match(&context)) {
                        issues.push(Issue::new(
                            RULE_ID,
                            "Potential integer overflow: size_of<T>() multiplied by user-controlled count without overflow checking.".to_string(),
                            SEVERITY,
                            CATEGORY,
                            ctx.file_path,
                            line_idx + 1,
                        ).with_remediation(Remediation::substantial(
                            "Use checked_mul for size_of * count calculations to prevent overflow."
                        )));
                    }
                }
            }
        }

        issues
    }
}


/// Agent semantics for S1155 - Integer Overflow in Size Calculation
#[derive(Debug, Clone)]
pub struct AgentSemantics {
    pub summary: &'static str,
    pub fix_playbook: &'static str,
    pub review_questions: &'static [&'static str],
    pub agent_actions: &'static [&'static str],
    pub safe_autofix: bool,
}

pub const S1155_AGENT_SEMANTICS: AgentSemantics = AgentSemantics {
    summary: "Detects integer overflow vulnerabilities in size calculations where user-controlled values are used in allocation expressions without overflow checking, potentially causing memory safety issues",
    fix_playbook: "1. Identify all size calculations involving user input\n2. Replace direct arithmetic with checked_mul, checked_add, saturating_mul, or saturating_add\n3. Validate that the product fits within usize bounds before allocation\n4. Use Layout::from_size_align for low-level allocations with size validation\n5. Consider using TryFrom/TryInto for validated conversions\n6. Add explicit overflow checks or use the checked arithmetic variants",
    review_questions: &[
        "Is the size value derived from user input?",
        "Are overflow-safe arithmetic operations (checked_*, saturating_*) being used?",
        "Is there validation that the calculated size fits in usize?",
        "Could malicious input cause an undersized allocation?"
    ],
    agent_actions: &[
        "Identify allocation sites with user-controlled sizes",
        "Check for use of checked/saturating arithmetic operations",
        "Verify size validation before allocation",
        "Suggest overflow-safe alternatives",
        "Recommend using Layout for validated allocations"
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
    fn test_s1155_rule_properties() {
        let rule = INTEGER_OVERFLOW_SIZERule::new();
        assert_eq!(rule.id(), "S1155");
        assert_eq!(rule.name(), "Integer overflow in size calculation detected");
        assert_eq!(rule.severity(), Severity::Major);
        assert_eq!(rule.category(), Category::Vulnerability);
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Positive Detection Tests — Should trigger the rule
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s1155_detects_vec_user_size_mul() {
        let source = r#"
            let user_count = get_user_count();
            let buffer = vec![0u8; user_count * 2];
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = INTEGER_OVERFLOW_SIZERule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect user count * 2 in vec!");
        assert_eq!(issues[0].rule_id, "S1155");
    }

    #[test]
    fn test_s1155_detects_vec_with_capacity_user_mul() {
        let source = r#"
            let rows = user_input.rows;
            let cols = user_input.cols;
            let vector = Vec::with_capacity(rows * cols);
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = INTEGER_OVERFLOW_SIZERule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect rows * cols in Vec::with_capacity");
        assert_eq!(issues[0].rule_id, "S1155");
    }

    #[test]
    fn test_s1155_detects_vec_resize_user_size() {
        let source = r#"
            let new_size = query_param.size;
            buffer.resize(new_size * multiplier, 0);
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = INTEGER_OVERFLOW_SIZERule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect user size * multiplier in resize");
        assert_eq!(issues[0].rule_id, "S1155");
    }

    #[test]
    fn test_s1155_detects_box_new_user_size() {
        let source = r#"
            let item_count = user_input.count;
            let data = Box::new(vec![0u8; item_count * element_size]);
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = INTEGER_OVERFLOW_SIZERule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect user count * size in Box::new");
        assert_eq!(issues[0].rule_id, "S1155");
    }

    #[test]
    fn test_s1155_detects_from_raw_parts_user_count() {
        let source = r#"
            let num_elements = get_element_count();
            let stride = get_stride();
            let slice = slice::from_raw_parts(ptr, num_elements * stride);
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = INTEGER_OVERFLOW_SIZERule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect user count * stride in from_raw_parts");
        assert_eq!(issues[0].rule_id, "S1155");
    }

    #[test]
    fn test_s1155_detects_alloc_user_size() {
        let source = r#"
            let user_len = request.body_length();
            let ptr = alloc(user_len * size_of::<u64>());
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = INTEGER_OVERFLOW_SIZERule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect user length * size_of in alloc");
        assert_eq!(issues[0].rule_id, "S1155");
    }

    #[test]
    fn test_s1155_detects_user_input_in_size_calc() {
        let source = r#"
            let num = param_from_user();
            let arr = vec![0u8; num * 1024];
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = INTEGER_OVERFLOW_SIZERule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Expected to detect user param in size calculation");
        assert_eq!(issues[0].rule_id, "S1155");
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // False Positive Tests — Should NOT trigger the rule
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s1155_false_positive_static_constants() {
        let source = r#"
            let buffer = vec![0u8; 1024 * 1024];
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = INTEGER_OVERFLOW_SIZERule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect static constant multiplication");
    }

    #[test]
    fn test_s1155_false_positive_checked_mul() {
        let source = r#"
            let size = user_input.count.checked_mul(2)?;
            let buffer = vec![0u8; size];
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = INTEGER_OVERFLOW_SIZERule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect checked_mul usage");
    }

    #[test]
    fn test_s1155_false_positive_saturating_mul() {
        let source = r#"
            let safe_size = user_count.saturating_mul(stride);
            let buffer = vec![0u8; safe_size];
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = INTEGER_OVERFLOW_SIZERule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect saturating_mul usage");
    }

    #[test]
    fn test_s1155_false_positive_wrapping_mul() {
        let source = r#"
            let wrapped = count.wrapping_mul(2);
            let buffer = vec![0u8; wrapped];
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = INTEGER_OVERFLOW_SIZERule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect wrapping_mul usage (intentional wrap)");
    }

    #[test]
    fn test_s1155_false_positive_checked_add() {
        let source = r#"
            let size = user_size.checked_add(header_size)?;
            let buffer = vec![0u8; size];
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = INTEGER_OVERFLOW_SIZERule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect checked_add usage");
    }

    #[test]
    fn test_s1155_false_positive_comment() {
        let source = r#"
            // vec![0u8; user_count * 2];
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = INTEGER_OVERFLOW_SIZERule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect vec! in comment");
    }

    #[test]
    fn test_s1155_false_positive_internal_size_calc() {
        let source = r#"
            let internal_count = calculate_buffer_count();
            let buffer = vec![0u8; internal_count * 2];
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = INTEGER_OVERFLOW_SIZERule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT detect internal (non-user) size calculation");
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Edge Case Tests
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_s1155_edge_case_empty_file() {
        let source = "";
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = INTEGER_OVERFLOW_SIZERule::new();
            rule.check(ctx)
        });
        assert!(issues.is_empty(), "Should NOT trigger on empty file");
    }

    #[test]
    fn test_s1155_edge_case_multiple_allocations() {
        let source = r#"
            let user_size = get_user_size();
            let safe_static = 1024 * 1024;
            let buffer1 = vec![0u8; safe_static];
            let buffer2 = vec![0u8; user_size * 2];
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = INTEGER_OVERFLOW_SIZERule::new();
            rule.check(ctx)
        });
        // Should detect only the user_size * 2, not the static constants
        assert!(!issues.is_empty(), "Should detect user-controlled allocation");
    }

    #[test]
    fn test_s1155_edge_case_size_of_with_user() {
        let source = r#"
            let count = user_input.num_elements;
            let total = count * size_of::<u32>();
            let slice = slice::from_raw_parts(ptr, total);
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = INTEGER_OVERFLOW_SIZERule::new();
            rule.check(ctx)
        });
        assert!(!issues.is_empty(), "Should detect count * size_of with user input");
    }

    #[test]
    fn test_s1155_edge_case_nested_user_access() {
        let source = r#"
            let config = parse_user_config();
            let width = config.dimensions.width;
            let height = config.dimensions.height;
            let buffer = vec![0u8; width * height];
        "#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            let rule = INTEGER_OVERFLOW_SIZERule::new();
            rule.check(ctx)
        });
        // Should detect width * height if they are user-derived
        assert!(!issues.is_empty(), "Should detect user-derived nested field multiplication");
    }
}
