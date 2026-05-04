//! Complexity-related rules
//!
//! Rules that detect code complexity issues:
//! - S138: Long Method rule
//! - S3776: Cognitive Complexity rule

use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext};
use streaming_iterator::StreamingIterator;
use tree_sitter::Query as TsQuery;
use tree_sitter::QueryCursor;

// ─────────────────────────────────────────────────────────────────────────────
// S138 — Long Method Rule
// ─────────────────────────────────────────────────────────────────────────────

pub struct S138Rule {
    threshold: usize,
}

impl S138Rule {
    pub fn new(threshold: usize) -> Self {
        Self { threshold }
    }
}

impl Default for S138Rule {
    fn default() -> Self {
        Self::new(50)
    }
}

impl Rule for S138Rule {
    fn id(&self) -> &str { "S138" }
    fn name(&self) -> &str { "Functions should not be too long" }
    fn severity(&self) -> Severity { Severity::Major }
    fn category(&self) -> Category { Category::CodeSmell }
    fn language(&self) -> &str { "rust" }

    fn check(&self, ctx: &RuleContext) -> Vec<Issue> {
        let mut issues = Vec::new();
        let threshold = self.threshold;

        let query_str = format!("({}) @func", ctx.language.function_node_type());
        let query = match TsQuery::new(&ctx.language.to_ts_language(), &query_str) {
            Ok(q) => q,
            Err(_) => return Vec::new(),
        };
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&query, ctx.tree.root_node(), ctx.source.as_bytes());

        while let Some(m) = matches.next() {
            for cap in m.captures {
                let func_node = cap.node;
                let line_count = ctx.line_count(func_node);

                if line_count > threshold {
                    let pt = func_node.start_position();
                    let start_row = pt.row;
                    let start_col = pt.column;
                    let func_name = ctx.function_name(func_node)
                        .unwrap_or("anonymous")
                        .to_string();

                    issues.push(Issue::new(
                        "S138",
                        format!(
                            "Function `{}` has {} lines, exceeds threshold of {}",
                            func_name, line_count, threshold
                        ),
                        Severity::Major,
                        Category::CodeSmell,
                        ctx.file_path,
                        start_row + 1,
                    ).with_column(start_col)
                    .with_remediation(Remediation::moderate(
                        "Extract helper functions to reduce method length"
                    )));
                }
            }
        }

        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S3776 — Cognitive Complexity Rule
// ─────────────────────────────────────────────────────────────────────────────

pub struct S3776Rule {
    threshold: i32,
}

impl S3776Rule {
    pub fn new(threshold: i32) -> Self {
        Self { threshold }
    }
}

impl Default for S3776Rule {
    fn default() -> Self {
        Self::new(20)
    }
}

impl Rule for S3776Rule {
    fn id(&self) -> &str { "S3776" }
    fn name(&self) -> &str { "Cognitive complexity should not be too high" }
    fn severity(&self) -> Severity { Severity::Major }
    fn category(&self) -> Category { Category::CodeSmell }
    fn language(&self) -> &str { "rust" }

    fn check(&self, ctx: &RuleContext) -> Vec<Issue> {
        let mut issues = Vec::new();
        let threshold = self.threshold;

        let query_str = format!("({}) @func", ctx.language.function_node_type());
        let query = match TsQuery::new(&ctx.language.to_ts_language(), &query_str) {
            Ok(q) => q,
            Err(_) => return Vec::new(),
        };
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&query, ctx.tree.root_node(), ctx.source.as_bytes());

        while let Some(m) = matches.next() {
            for cap in m.captures {
                let func_node = cap.node;
                let complexity = crate::rules::helpers::calculate_cognitive_complexity(func_node, ctx.source.as_bytes());

                if complexity > threshold {
                    let pt = func_node.start_position();
                    let start_row = pt.row;
                    let start_col = pt.column;
                    let func_name = ctx.function_name(func_node)
                        .unwrap_or("anonymous")
                        .to_string();

                    issues.push(Issue::new(
                        "S3776",
                        format!(
                            "Function `{}` has cognitive complexity of {}, exceeds threshold of {}",
                            func_name, complexity, threshold
                        ),
                        Severity::Major,
                        Category::CodeSmell,
                        ctx.file_path,
                        start_row + 1,
                    ).with_column(start_col)
                    .with_remediation(Remediation::substantial(
                        "Consider extracting helper functions or simplifying logic flow"
                    )));
                }
            }
        }

        issues
    }
}
