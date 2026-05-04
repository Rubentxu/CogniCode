//! Style-related rules
//!
//! Rules that detect code style issues:
//! - S2306: God Class rule
//! - S1066: Collapsible If Statements rule
//! - S1192: String Literal Duplicates rule

use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext};
use streaming_iterator::StreamingIterator;
use tree_sitter::Query as TsQuery;
use tree_sitter::QueryCursor;

// ─────────────────────────────────────────────────────────────────────────────
// S2306 — God Class Rule
// ─────────────────────────────────────────────────────────────────────────────

pub struct S2306Rule {
    method_threshold: usize,
    field_threshold: usize,
    wmc_threshold: usize,
}

impl S2306Rule {
    pub fn new(method_threshold: usize, field_threshold: usize, wmc_threshold: usize) -> Self {
        Self { method_threshold, field_threshold, wmc_threshold }
    }
}

impl Default for S2306Rule {
    fn default() -> Self {
        Self::new(10, 10, 50)
    }
}

impl Rule for S2306Rule {
    fn id(&self) -> &str { "S2306" }
    fn name(&self) -> &str { "Classes should not be too large" }
    fn severity(&self) -> Severity { Severity::Critical }
    fn category(&self) -> Category { Category::CodeSmell }
    fn language(&self) -> &str { "rust" }

    fn check(&self, ctx: &RuleContext) -> Vec<Issue> {
        let mut issues = Vec::new();

        let query_str = format!("({}) @func", ctx.language.function_node_type());
        let query = match TsQuery::new(&ctx.language.to_ts_language(), &query_str) {
            Ok(q) => q,
            Err(_) => return Vec::new(),
        };
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&query, ctx.tree.root_node(), ctx.source.as_bytes());

        while let Some(m) = matches.next() {
            for cap in m.captures {
                let item_node = cap.node;

                // Count public methods
                let method_query = match TsQuery::new(&ctx.language.to_ts_language(), "(function_item (visibility_modifier) @vis) @method") {
                    Ok(q) => q,
                    Err(_) => continue,
                };
                let mut method_cursor = QueryCursor::new();
                let mut method_matches = method_cursor.matches(&method_query, item_node, ctx.source.as_bytes());
                let mut public_methods = 0;
                while let Some(_mm) = method_matches.next() {
                    public_methods += 1;
                }

                // Count fields
                let mut fields = 0;
                for i in 0..item_node.child_count() {
                    if let Some(child) = item_node.child(i) {
                        if child.kind() == "field_declaration" {
                            fields += 1;
                        }
                    }
                }

                // Calculate WMC
                let func_query = match TsQuery::new(&ctx.language.to_ts_language(), "(function_item) @func") {
                    Ok(q) => q,
                    Err(_) => continue,
                };
                let mut func_cursor = QueryCursor::new();
                let mut func_matches = func_cursor.matches(&func_query, item_node, ctx.source.as_bytes());
                let mut wmc = 0;
                while let Some(fm) = func_matches.next() {
                    for fcap in fm.captures {
                        let func_node = fcap.node;
                        let bin_query = match TsQuery::new(&ctx.language.to_ts_language(), "(binary_expression) @bin") {
                            Ok(q) => q,
                            Err(_) => continue,
                        };
                        let mut bin_cursor = QueryCursor::new();
                        let mut bin_matches = bin_cursor.matches(&bin_query, func_node, ctx.source.as_bytes());
                        let mut bin_count = 0;
                        while let Some(_bm) = bin_matches.next() {
                            bin_count += 1;
                        }
                        wmc += bin_count + 1;
                    }
                }

                let is_god_class = public_methods > self.method_threshold
                    && fields > self.field_threshold
                    && wmc > self.wmc_threshold;

                if is_god_class {
                    let pt = item_node.start_position();
                    let start_row = pt.row;
                    let start_col = pt.column;
                    let class_name = ctx.function_name(item_node)
                        .unwrap_or("unknown")
                        .to_string();

                    issues.push(Issue::new(
                        "S2306",
                        format!(
                            "Class `{}` has {} public methods, {} fields, WMC={}. Consider splitting into smaller units",
                            class_name, public_methods, fields, wmc
                        ),
                        Severity::Critical,
                        Category::CodeSmell,
                        ctx.file_path,
                        start_row + 1,
                    ).with_column(start_col)
                    .with_remediation(Remediation::substantial(
                        "Split this class into smaller, focused units following Single Responsibility Principle"
                    )));
                }
            }
        }

        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S1066 — Collapsible If Statements Rule
// ─────────────────────────────────────────────────────────────────────────────

pub struct S1066Rule;

impl S1066Rule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for S1066Rule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for S1066Rule {
    fn id(&self) -> &str { "S1066" }
    fn name(&self) -> &str { "Collapsible if statements should be merged" }
    fn severity(&self) -> Severity { Severity::Minor }
    fn category(&self) -> Category { Category::CodeSmell }
    fn language(&self) -> &str { "rust" }

    fn check(&self, ctx: &RuleContext) -> Vec<Issue> {
        let mut issues = Vec::new();

        let query = match TsQuery::new(
            &ctx.language.to_ts_language(),
            "(if_expression) @if"
        ) {
            Ok(q) => q,
            Err(_) => return Vec::new(),
        };
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&query, ctx.tree.root_node(), ctx.source.as_bytes());

        while let Some(m) = matches.next() {
            for cap in m.captures {
                let if_node = cap.node;

                // Check if this if has no else
                let has_else = if_node.child_by_field_name("alternative").is_some();
                if has_else {
                    continue;
                }

                // Find the consequence (then branch)
                if let Some(cons) = if_node.child_by_field_name("consequence") {
                    // Check if the consequence is another if without else
                    if cons.kind() == "if_expression" {
                        let inner_has_else = cons.child_by_field_name("alternative").is_some();
                        if !inner_has_else {
                            let pt = if_node.start_position();
                            let start_row = pt.row;
                            let start_col = pt.column;
                            issues.push(Issue::new(
                                "S1066",
                                "These nested if statements can be collapsed into a single if with &&",
                                Severity::Minor,
                                Category::CodeSmell,
                                ctx.file_path,
                                start_row + 1,
                            ).with_column(start_col)
                            .with_remediation(Remediation::quick(
                                "Combine the conditions with && operator"
                            )));
                        }
                    }
                }
            }
        }

        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S1192 — String Literal Duplicates Rule
// ─────────────────────────────────────────────────────────────────────────────

pub struct S1192Rule {
    min_occurrences: usize,
}

impl S1192Rule {
    pub fn new(min_occurrences: usize) -> Self {
        Self { min_occurrences }
    }
}

impl Default for S1192Rule {
    fn default() -> Self {
        Self::new(3)
    }
}

impl Rule for S1192Rule {
    fn id(&self) -> &str { "S1192" }
    fn name(&self) -> &str { "String literals should not be duplicated" }
    fn severity(&self) -> Severity { Severity::Major }
    fn category(&self) -> Category { Category::CodeSmell }
    fn language(&self) -> &str { "rust" }

    fn check(&self, ctx: &RuleContext) -> Vec<Issue> {
        let mut issues = Vec::new();
        let min_occurrences = self.min_occurrences;

        let mut string_locations: std::collections::HashMap<String, Vec<(usize, usize)>> = std::collections::HashMap::new();
        crate::rules::helpers::collect_string_literals(ctx.tree.root_node(), ctx.source.as_bytes(), &mut string_locations);

        for (text, locations) in string_locations {
            if locations.len() >= min_occurrences {
                for (line, col) in &locations {
                    issues.push(Issue::new(
                        "S1192",
                        format!(
                            "String literal \"{}\" is duplicated {} times",
                            text, locations.len()
                        ),
                        Severity::Major,
                        Category::CodeSmell,
                        ctx.file_path,
                        *line,
                    ).with_column(*col)
                    .with_remediation(Remediation::moderate(
                        "Extract this string to a named constant"
                    )));
                }
            }
        }

        issues
    }
}
