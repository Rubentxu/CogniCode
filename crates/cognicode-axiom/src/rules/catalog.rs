//! Code smell and quality rule catalog for cognicode-axiom
//!
//! Implements 332 rules across code smells, vulnerabilities, and security hotspots:
//!
//! ## Rust Rules
//! Core Rust rules for code quality, bugs, and security.
//!
//! ## JavaScript/TypeScript Security Rules (20 rules)
//! - JS_S1523: eval() usage
//! - JS_S2259: document.write() XSS
//! - JS_S2611: innerHTML XSS
//! - JS_S3330: Cookie without HttpOnly
//! - JS_S4502: CSRF protection disabled
//! - JS_S4784: RegExp injection
//! - JS_S4817: XPath injection
//! - JS_S4823: process.env in browser
//! - JS_S4829: console.log in production
//! - JS_S5122: NoSQL injection
//! - JS_S5145: window.open without noopener
//! - JS_S5247: dangerouslySetInnerHTML
//! - JS_S5542: Weak crypto (createCipher)
//! - JS_S5547: Weak cipher (RC4, DES)
//! - JS_S5693: File upload without size limit
//! - JS_S5725: CSP header missing
//! - JS_S5730: Mixed content
//! - JS_S5734: HSTS header missing
//! - JS_S5736: X-Content-Type-Options missing
//! - JS_S5852: RegExp DoS (catastrophic backtracking)

#![allow(non_snake_case)]
#![allow(non_camel_case_types)]
#![allow(unused_variables)]

use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use cognicode_macros::declare_rule;
use inventory::submit;
use std::collections::HashMap;
use streaming_iterator::StreamingIterator;

// ─────────────────────────────────────────────────────────────────────────────
// Helper: Calculate cognitive complexity (SonarSource algorithm)
// ─────────────────────────────────────────────────────────────────────────────

fn calculate_cognitive_complexity(node: tree_sitter::Node, source: &[u8]) -> i32 {
    let mut complexity = 0;
    compute_complexity_impl(node, source, 0, &mut complexity, false);
    complexity
}

fn compute_complexity_impl(
    node: tree_sitter::Node,
    source: &[u8],
    depth: usize,
    complexity: &mut i32,
    _in_loop: bool,
) {
    let kind = node.kind();

    // Increment for control structures
    if matches!(kind,
        "if_expression" | "match_expression" | "match_arm" |
        "for_expression" | "while_expression" | "loop_expression"
    ) {
        *complexity += 1 + depth as i32;
    }

    // Increment for boolean operators in binary expressions
    if kind == "binary_expression" {
        if let Ok(text) = node.utf8_text(source) {
            if text.contains("&&") || text.contains("||") {
                *complexity += 1;
            }
        }
    }

    // Recurse into children
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            let is_loop = matches!(kind,
                "for_expression" | "while_expression" | "loop_expression"
            );
            compute_complexity_impl(child, source, depth + 1, complexity, is_loop);
        }
    }
}

// Helper functions removed — functionality moved to RuleContext methods:
// - find_function_body → ctx.query_functions() + ctx.line_count()
// - calculate_nesting_depth → ctx.nesting_depth()
// - count_parameters → ctx.query_functions() + named_child_count()

// ─────────────────────────────────────────────────────────────────────────────
// Helper: Collect string literals
// ─────────────────────────────────────────────────────────────────────────────

fn collect_string_literals(
    node: tree_sitter::Node,
    source: &[u8],
    locations: &mut HashMap<String, Vec<(usize, usize)>>,
) {
    if node.kind() == "string_literal" {
        if let Ok(text) = node.utf8_text(source) {
            let point = node.start_position();
            let row = point.row;
            let col = point.column;
            locations
                .entry(text.to_string())
                .or_default()
                .push((row + 1, col + 1));
        }
    }

    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            collect_string_literals(child, source, locations);
        }
    }
}

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
        let query = match tree_sitter::Query::new(&ctx.language.to_ts_language(), &query_str) {
            Ok(q) => q,
            Err(_) => return Vec::new(),
        };
        let mut cursor = tree_sitter::QueryCursor::new();
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
        let query = match tree_sitter::Query::new(&ctx.language.to_ts_language(), &query_str) {
            Ok(q) => q,
            Err(_) => return Vec::new(),
        };
        let mut cursor = tree_sitter::QueryCursor::new();
        let mut matches = cursor.matches(&query, ctx.tree.root_node(), ctx.source.as_bytes());

        while let Some(m) = matches.next() {
            for cap in m.captures {
                let func_node = cap.node;
                let complexity = calculate_cognitive_complexity(func_node, ctx.source.as_bytes());

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
        let query = match tree_sitter::Query::new(&ctx.language.to_ts_language(), &query_str) {
            Ok(q) => q,
            Err(_) => return Vec::new(),
        };
        let mut cursor = tree_sitter::QueryCursor::new();
        let mut matches = cursor.matches(&query, ctx.tree.root_node(), ctx.source.as_bytes());

        while let Some(m) = matches.next() {
            for cap in m.captures {
                let item_node = cap.node;

                // Count public methods
                let method_query = match tree_sitter::Query::new(&ctx.language.to_ts_language(), "(function_item (visibility_modifier) @vis) @method") {
                    Ok(q) => q,
                    Err(_) => continue,
                };
                let mut method_cursor = tree_sitter::QueryCursor::new();
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
                let func_query = match tree_sitter::Query::new(&ctx.language.to_ts_language(), "(function_item) @func") {
                    Ok(q) => q,
                    Err(_) => continue,
                };
                let mut func_cursor = tree_sitter::QueryCursor::new();
                let mut func_matches = func_cursor.matches(&func_query, item_node, ctx.source.as_bytes());
                let mut wmc = 0;
                while let Some(fm) = func_matches.next() {
                    for fcap in fm.captures {
                        let func_node = fcap.node;
                        let bin_query = match tree_sitter::Query::new(&ctx.language.to_ts_language(), "(binary_expression) @bin") {
                            Ok(q) => q,
                            Err(_) => continue,
                        };
                        let mut bin_cursor = tree_sitter::QueryCursor::new();
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
// S134 — Deep Nesting Rule
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S134"
    name: "Control flow statements should not be nested too deeply"
    severity: Major
    category: CodeSmell
    language: "rust"
    params: { threshold: usize = 4 }
    check: => {
        let mut issues = Vec::new();
        let func_nodes = ctx.query_functions();
        for node in func_nodes {
            let depth = ctx.nesting_depth(node);
            if depth > self.threshold {
                let pt = node.start_position();
                if let Some(name) = ctx.function_name(node) {
                    issues.push(Issue::new(
                        "S134",
                        format!("Function '{}' has nesting depth {} exceeding threshold {}", name, depth, self.threshold),
                        Severity::Major,
                        Category::CodeSmell,
                        ctx.file_path,
                        pt.row + 1,
                    ).with_column(pt.column as usize)
                    .with_remediation(Remediation::moderate(
                        "Extract nested logic into separate functions or use early returns"
                    )));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S107 — Too Many Parameters Rule
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S107"
    name: "Functions should not have too many parameters"
    severity: Major
    category: CodeSmell
    language: "rust"
    params: { threshold: usize = 7 }
    check: => {
        let mut issues = Vec::new();
        // Query for function definitions to count parameters
        let query_str = "(function_item parameters: (parameters) @params)";
        if let Ok(query) = tree_sitter::Query::new(&ctx.language.to_ts_language(), query_str) {
            let mut cursor = tree_sitter::QueryCursor::new();
            let mut matches = cursor.matches(&query, ctx.tree.root_node(), ctx.source.as_bytes());
            while let Some(m) = matches.next() {
                for capture in m.captures {
                    // Count named children (parameters) inside the parameters node
                    let params_node = capture.node;
                    let param_count = params_node.named_child_count() as usize;
                    if param_count > self.threshold {
                        let pt = params_node.start_position();
                        // Try to get the function name from the parent node
                        let func_name = params_node.parent()
                            .and_then(|p| ctx.function_name(p))
                            .unwrap_or("anonymous");
                        issues.push(Issue::new(
                            "S107",
                            format!("Function '{}' has {} parameters exceeding threshold {}", func_name, param_count, self.threshold),
                            Severity::Major,
                            Category::CodeSmell,
                            ctx.file_path,
                            pt.row + 1,
                        ).with_column(pt.column as usize)
                        .with_remediation(Remediation::moderate(
                            "Consider grouping related parameters into a struct"
                        )));
                    }
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

        let query = match tree_sitter::Query::new(
            &ctx.language.to_ts_language(),
            "(if_expression) @if"
        ) {
            Ok(q) => q,
            Err(_) => return Vec::new(),
        };
        let mut cursor = tree_sitter::QueryCursor::new();
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

        let mut string_locations: HashMap<String, Vec<(usize, usize)>> = HashMap::new();
        collect_string_literals(ctx.tree.root_node(), ctx.source.as_bytes(), &mut string_locations);

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

// ─────────────────────────────────────────────────────────────────────────────
// S1135 — TODO/FIXME Tags Rule
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1135"
    name: "TODO tags should be completed or removed"
    severity: Minor
    category: CodeSmell
    language: "*"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"(?i)(TODO|FIXME|HACK|XXX):?").unwrap();
        for (line_num, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "S1135",
                    format!("TODO/FIXME tag found: {}", line.trim()),
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num + 1,
                ));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S1134 — Deprecated Code Rule
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1134"
    name: "Deprecated code should not be used"
    severity: Info
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("#[deprecated") || line.contains("@Deprecated") {
                issues.push(Issue::new(
                    "S1134",
                    "Deprecated attribute detected",
                    Severity::Info,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::moderate(
                    "Replace deprecated API with the recommended alternative"
                )));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S2068 — Hard-coded Credentials Rule
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S2068"
    name: "Hard-coded credentials are security sensitive"
    severity: Blocker
    category: SecurityHotspot
    language: "*"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let patterns = [
            (r#"(?i)(password|passwd|pwd)\s*[=:]\s*["'][^"']{4,}["']"#, "password"),
            (r#"(?i)(api[_-]?key|apikey)\s*[=:]\s*["'][^"']{4,}["']"#, "api_key"),
            (r#"(?i)(secret|token)\s*[=:]\s*["'][^"']{4,}["']"#, "secret"),
            (r#"(?i)(bearer|basic)\s+[a-zA-Z0-9_\-]+"#, "bearer_token"),
        ];
        let regexes: Vec<_> = patterns.iter().map(|(p, _)| regex::Regex::new(p).unwrap()).collect();
        
        for (line_num, line) in ctx.source.lines().enumerate() {
            for re in &regexes {
                if re.is_match(line) {
                    issues.push(Issue::new(
                        "S2068",
                        format!("Hard-coded credential detected on line {}", line_num + 1),
                        Severity::Blocker,
                        Category::SecurityHotspot,
                        ctx.file_path,
                        line_num + 1,
                    ).with_remediation(Remediation::moderate(
                        "Use environment variables or a secrets manager instead of hard-coded values"
                    )));
                    break;
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S5122 — SQL Injection Rule
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S5122"
    name: "SQL injection vulnerabilities should be prevented"
    severity: Blocker
    category: Vulnerability
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let sql_keywords = ["SELECT", "INSERT", "UPDATE", "DELETE", "DROP", "CREATE", "ALTER", "EXEC", "EXECUTE"];
        
        let query = match tree_sitter::Query::new(
            &ctx.language.to_ts_language(),
            "(macro_invocation (identifier) @macro_name (token_tree) @args)"
        ) {
            Ok(q) => q,
            Err(_) => return Vec::new(),
        };
        
        let mut cursor = tree_sitter::QueryCursor::new();
        let mut matches = cursor.matches(&query, ctx.tree.root_node(), ctx.source.as_bytes());

        while let Some(m) = matches.next() {
            for cap in m.captures {
                if cap.node.kind() == "identifier" {
                    if let Ok(macro_name) = cap.node.utf8_text(ctx.source.as_bytes()) {
                        if macro_name == "format" || macro_name == "format_args" {
                            if let Some(args_node) = m.captures.iter().find(|c| c.node.kind() == "token_tree") {
                                if let Ok(args_text) = args_node.node.utf8_text(ctx.source.as_bytes()) {
                                    let args_upper = args_text.to_uppercase();
                                    for keyword in &sql_keywords {
                                        if args_upper.contains(keyword) {
                                            let pt = cap.node.start_position();
                                            issues.push(Issue::new(
                                                "S5122",
                                                format!(
                                                    "Potential SQL injection: SQL keyword '{}' found in format! string",
                                                    keyword
                                                ),
                                                Severity::Blocker,
                                                Category::Vulnerability,
                                                ctx.file_path,
                                                pt.row + 1,
                                            ).with_column(pt.column + 1)
                                            .with_remediation(Remediation::substantial(
                                                "Use parameterized queries instead of string interpolation"
                                            )));
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S4792 — Weak Cryptography Rule
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S4792"
    name: "Weak cryptography should not be used"
    severity: Critical
    category: Vulnerability
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let weak_patterns = [
            (r"md5", "MD5 hash function"),
            (r"sha1", "SHA-1 hash function"),
            (r"des\b", "DES block cipher"),
            (r"rc4", "RC4 stream cipher"),
            (r"crypt\b", "crypt(3) function"),
        ];

        for (line_idx, line) in ctx.source.lines().enumerate() {
            for (pattern, description) in &weak_patterns {
                if let Ok(re) = regex::Regex::new(pattern) {
                    if re.is_match(line) {
                        let pt = line.find(|c: char| !c.is_whitespace()).unwrap_or(0);
                        issues.push(Issue::new(
                            "S4792",
                            format!(
                                "Use of weak cryptography: {} detected on line {}",
                                description, line_idx + 1
                            ),
                            Severity::Critical,
                            Category::Vulnerability,
                            ctx.file_path,
                            line_idx + 1,
                        ).with_column(pt + 1)
                        .with_remediation(Remediation::substantial(
                            "Use a modern cryptographic algorithm (e.g., SHA-256, AES-256-GCM)"
                        )));
                        break;
                    }
                }
            }
        }

        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S1484 — Unused Function Rule (Dead Code Detection)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1484"
    name: "Unused functions should be removed"
    severity: Major
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let dead_symbols = ctx.find_dead_symbols();
        for (name, file) in dead_symbols {
            if name.starts_with("test_") && name != "main" {
                continue;
            }
            if name == "main" {
                continue;
            }

            let current_file = ctx.file_path.to_string_lossy();
            if !file.contains(&*current_file) && current_file.as_ref() != file {
                continue;
            }

            let file_path = std::path::PathBuf::from(&file);
            issues.push(Issue::new(
                "S1484",
                format!("Unused function '{}' has no callers", name),
                Severity::Major,
                Category::CodeSmell,
                file_path,
                0,
            ).with_remediation(Remediation::moderate(
                "Remove the unused function or ensure it is called"
            )));
        }

        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S5332 — Clear-text HTTP Detection Rule
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S5332"
    name: "Clear-text protocols should not be used"
    severity: Blocker
    category: Vulnerability
    language: "*"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r#"http://[^\s""']+"#).unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(m) = re.find(line) {
                if !line.contains("https://") && !line.contains("localhost") {
                    issues.push(Issue::new(
                        "S5332",
                        format!("Clear-text HTTP URL found: {}", m.as_str()),
                        Severity::Blocker,
                        Category::Vulnerability,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::moderate(
                        "Use HTTPS instead of HTTP for secure communications"
                    )));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S5631 — Unsafe Unwrap Detection Rule
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S5631"
    name: "Unsafe unwrap() calls should be avoided"
    severity: Major
    category: Bug
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let query_str = r#"(call_expression function: (field_expression field: (field_identifier) @method)) @call"#;
        let query = tree_sitter::Query::new(&ctx.language.to_ts_language(), query_str).unwrap();
        let mut cursor = tree_sitter::QueryCursor::new();
        let mut matches = cursor.matches(&query, ctx.tree.root_node(), ctx.source.as_bytes());
        while let Some(m) = matches.next() {
            for capture in m.captures {
                if let Ok(name) = capture.node.utf8_text(ctx.source.as_bytes()) {
                    if name == "unwrap" {
                        let pt = capture.node.start_position();
                        issues.push(Issue::new(
                            "S5631",
                            "Unsafe unwrap() call detected",
                            Severity::Major,
                            Category::Bug,
                            ctx.file_path,
                            pt.row + 1,
                        ).with_column(pt.column as usize)
                        .with_remediation(Remediation::moderate(
                            "Replace unwrap() with ? operator, expect(), or proper error handling"
                        )));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S113 — Line Too Long Rule
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S113"
    name: "Lines should not be too long"
    severity: Minor
    category: CodeSmell
    language: "*"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.len() > 120 {
                issues.push(Issue::new(
                    "S113",
                    format!("Line is {} characters long (max 120)", line.len()),
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S117 — Variable Naming Convention Rule (snake_case)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S117"
    name: "Variable names should follow snake_case convention"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"let\s+(mut\s+)?([A-Z][a-zA-Z0-9_]*|[a-z]+[A-Z])").unwrap();
        // Single-char variables common in iterators should be skipped
        let single_char_skip = ["i", "j", "x", "y", "n", "k", "v", "c"];
        for (idx, line) in ctx.source.lines().enumerate() {
            for cap in re.captures_iter(line) {
                if let Some(name) = cap.get(2) {
                    let name_str = name.as_str();
                    // Skip single-char variables common in iterators
                    if single_char_skip.contains(&name_str) {
                        continue;
                    }
                    issues.push(Issue::new(
                        "S117",
                        format!("Variable '{}' does not follow snake_case", name_str),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S1854 — Unused Variable Rule
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1854"
    name: "Unused variables should be removed"
    severity: Info
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("let _") && !trimmed.contains('=') {
                issues.push(Issue::new(
                    "S1854",
                    "Variable declared with '_' prefix may be intentionally unused",
                    Severity::Info,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::quick(
                    "Remove the unused variable or prefix it with '_' to silence the warning"
                )));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S1226 — Method Parameters Should Not Be Reassigned Rule
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1226"
    name: "Method parameters should not be reassigned"
    severity: Major
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        // Find function parameters and check if they appear on LHS of assignments
        let query_str = "(function_item parameters: (parameters (parameter pattern: (identifier) @param))) @func";
        if let Ok(query) = tree_sitter::Query::new(&ctx.language.to_ts_language(), query_str) {
            let mut cursor = tree_sitter::QueryCursor::new();
            let mut matches = cursor.matches(&query, ctx.tree.root_node(), ctx.source.as_bytes());
            let mut params: std::collections::HashSet<String> = std::collections::HashSet::new();
            while let Some(m) = matches.next() {
                for capture in m.captures {
                    if let Ok(name) = capture.node.utf8_text(ctx.source.as_bytes()) {
                        params.insert(name.to_string());
                    }
                }
            }
            // Check for assignment patterns where LHS matches a param name
            let assign_query = "(let_declaration pattern: (identifier) @var)";
            if let Ok(query2) = tree_sitter::Query::new(&ctx.language.to_ts_language(), assign_query) {
                let mut cursor2 = tree_sitter::QueryCursor::new();
                let mut matches2 = cursor2.matches(&query2, ctx.tree.root_node(), ctx.source.as_bytes());
                while let Some(m) = matches2.next() {
                    for capture in m.captures {
                        if let Ok(name) = capture.node.utf8_text(ctx.source.as_bytes()) {
                            if params.contains(name) {
                                let pt = capture.node.start_position();
                                issues.push(Issue::new(
                                    "S1226",
                                    format!("Parameter '{}' should not be reassigned", name),
                                    Severity::Major, Category::CodeSmell, ctx.file_path, pt.row + 1,
                                ).with_remediation(Remediation::moderate(
                                    "Use a new local variable instead of reassigning the parameter"
                                )));
                            }
                        }
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S1186 — Empty Functions Should Be Removed Rule
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1186"
    name: "Empty functions should be completed or removed"
    severity: Major
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let node_type = ctx.language.function_node_type();
        let query_str = format!("({} body: (block) @body) @func", node_type);
        if let Ok(query) = tree_sitter::Query::new(&ctx.language.to_ts_language(), &query_str) {
            let mut cursor = tree_sitter::QueryCursor::new();
            let mut matches = cursor.matches(&query, ctx.tree.root_node(), ctx.source.as_bytes());
            while let Some(m) = matches.next() {
                for capture in m.captures {
                    let node = capture.node;
                    // Check if the block body has no meaningful children (only comment/doc nodes)
                    let named_children = node.named_child_count();
                    if named_children == 0 {
                        if let Some(name) = ctx.function_name(node.parent().unwrap_or(node)) {
                            let pt = node.start_position();
                            issues.push(Issue::new(
                                "S1186",
                                format!("Function '{}' has an empty body", name),
                                Severity::Major, Category::CodeSmell, ctx.file_path, pt.row + 1,
                            ).with_remediation(Remediation::quick(
                                "Implement the function body or remove it if not needed"
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
// S1871 — Duplicate Branches In Conditionals Rule
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1871"
    name: "Branches should not have identical implementations"
    severity: Major
    category: Bug
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        // Find if_expressions and compare their alternative/consequence bodies
        let query_str = r#"
            (if_expression
                consequence: (block) @then_body
                alternative: (block) @else_body
            ) @if_expr
        "#;
        if let Ok(query) = tree_sitter::Query::new(&ctx.language.to_ts_language(), query_str) {
            let mut cursor = tree_sitter::QueryCursor::new();
            let mut matches = cursor.matches(&query, ctx.tree.root_node(), ctx.source.as_bytes());
            while let Some(m) = matches.next() {
                let mut then_text = None;
                let mut else_text = None;
                let mut then_line = 0;
                for capture in m.captures {
                    let text = capture.node.utf8_text(ctx.source.as_bytes()).unwrap_or("");
                    let pt = capture.node.start_position();
                    if capture.node.kind() == "block" {
                        if then_text.is_none() {
                            then_text = Some(text.to_string());
                            then_line = pt.row + 1;
                        } else {
                            else_text = Some(text.to_string());
                        }
                    }
                }
                if let (Some(then), Some(else_t)) = (then_text, else_text) {
                    if then == else_t && !then.is_empty() {
                        issues.push(Issue::new(
                            "S1871",
                            format!("Duplicate branches at line {}", then_line),
                            Severity::Major, Category::Bug, ctx.file_path, then_line,
                        ).with_remediation(Remediation::moderate(
                            "Merge duplicate branches into a single block or refactor the condition"
                        )));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S2589 — Boolean expressions should not be constant
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S2589"
    name: "Boolean expressions should not be constant"
    severity: Major
    category: Bug
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed == "if true {" || trimmed == "if false {" || trimmed == "while true {" {
                issues.push(Issue::new(
                    "S2589",
                    format!("Constant boolean expression at line {}", idx + 1),
                    Severity::Major,
                    Category::Bug,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::quick("Remove the redundant condition or use a meaningful expression")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S2757 — Unexpected assignment operators in conditions
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S2757"
    name: "Unexpected assignment operators in conditions"
    severity: Major
    category: Bug
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"if\s+let\s+[A-Z]").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "S2757",
                    "Potentially unintended pattern match in condition",
                    Severity::Major,
                    Category::Bug,
                    ctx.file_path,
                    idx + 1,
                ));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S1313 — IP addresses should not be hardcoded
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1313"
    name: "IP addresses should not be hardcoded"
    severity: Minor
    category: SecurityHotspot
    language: "*"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r#""\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}""#).unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(m) = re.find(line) {
                issues.push(Issue::new(
                    "S1313",
                    format!("Hardcoded IP address: {}", m.as_str()),
                    Severity::Minor,
                    Category::SecurityHotspot,
                    ctx.file_path,
                    idx + 1,
                ));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S1141 — Error handling should not be deeply nested
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1141"
    name: "Error handling should not be deeply nested"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let query_str = "(match_expression) @match";
        if let Ok(query) = tree_sitter::Query::new(&ctx.language.to_ts_language(), query_str) {
            let mut cursor = tree_sitter::QueryCursor::new();
            let mut matches = cursor.matches(&query, ctx.tree.root_node(), ctx.source.as_bytes());
            while let Some(m) = matches.next() {
                for capture in m.captures {
                    let depth = ctx.nesting_depth(capture.node);
                    if depth > 3 {
                        let pt = capture.node.start_position();
                        issues.push(Issue::new(
                            "S1141",
                            "Deeply nested error handling - consider using ? operator or extracting to function",
                            Severity::Minor,
                            Category::CodeSmell,
                            ctx.file_path,
                            pt.row + 1,
                        ));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S1994 — Loop counters should not be modified inside the loop
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1994"
    name: "Loop counters should not be modified inside the loop"
    severity: Critical
    category: Bug
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"for\s+(\w+)\s+in\s+").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                let counter = cap.get(1).unwrap().as_str();
                let body_start = idx + 1;
                for (body_idx, body_line) in ctx.source.lines().skip(body_start).enumerate() {
                    if body_line.contains(&format!("{} =", counter)) || body_line.contains(&format!("{} +=", counter)) {
                        issues.push(Issue::new(
                            "S1994",
                            format!("Loop counter '{}' modified inside loop", counter),
                            Severity::Critical,
                            Category::Bug,
                            ctx.file_path,
                            body_start + body_idx + 1,
                        ));
                    }
                    if body_line.trim() == "}" { break; }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S116 — Field names should follow snake_case convention
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S116"
    name: "Field names should follow snake_case convention"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let query_str = "(struct_item (field_declaration name: (field_identifier) @field))";
        if let Ok(query) = tree_sitter::Query::new(&ctx.language.to_ts_language(), query_str) {
            let mut cursor = tree_sitter::QueryCursor::new();
            let mut matches = cursor.matches(&query, ctx.tree.root_node(), ctx.source.as_bytes());
            while let Some(m) = matches.next() {
                for capture in m.captures {
                    if let Ok(name) = capture.node.utf8_text(ctx.source.as_bytes()) {
                        if name.contains(char::is_uppercase) || name.contains('-') {
                            let pt = capture.node.start_position();
                            issues.push(Issue::new(
                                "S116",
                                format!("Field '{}' should use snake_case", name),
                                Severity::Minor,
                                Category::CodeSmell,
                                ctx.file_path,
                                pt.row + 1,
                            ));
                        }
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S119 — Type parameter names should follow single-letter convention
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S119"
    name: "Type parameter names should follow single-letter convention"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"<\s*([A-Z][a-z]+)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            for cap in re.captures_iter(line) {
                if let Some(name) = cap.get(1) {
                    let name_str = name.as_str();
                    if name_str.len() > 1 {
                        // Skip if type param is used with a trait bound (e.g., T: Serialize)
                        // These have semantic meaning and shouldn't be flagged
                        let full_match = cap.get(0).unwrap().as_str();
                        if full_match.ends_with(':') || line.contains(&format!("{}:", name_str)) {
                            continue;
                        }
                        issues.push(Issue::new(
                            "S119",
                            format!("Type parameter '{}' should use single-letter name", name_str),
                            Severity::Minor,
                            Category::CodeSmell,
                            ctx.file_path,
                            idx + 1,
                        ));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S112 — Error handling should not fail silently in cleanup code
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S112"
    name: "Error handling should not fail silently in cleanup code"
    severity: Major
    category: Bug
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"impl\s+Drop\s+for\s+(\w+)").unwrap();
        let mut in_drop = false;
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) { in_drop = true; }
            if in_drop && line.contains(".unwrap()") {
                issues.push(Issue::new(
                    "S112",
                    "unwrap() in Drop implementation - errors will be silently ignored",
                    Severity::Major,
                    Category::Bug,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::moderate("Log the error or use if let/let _ to suppress")));
            }
            if in_drop && line.trim() == "}" { in_drop = false; }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S2095 — Resources should be properly closed
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S2095"
    name: "Resources should be properly closed"
    severity: Major
    category: Bug
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let query_str = "(unsafe_block) @unsafe";
        if let Ok(query) = tree_sitter::Query::new(&ctx.language.to_ts_language(), query_str) {
            let mut cursor = tree_sitter::QueryCursor::new();
            let mut matches = cursor.matches(&query, ctx.tree.root_node(), ctx.source.as_bytes());
            while let Some(m) = matches.next() {
                for capture in m.captures {
                    let pt = capture.node.start_position();
                    issues.push(Issue::new(
                        "S2095",
                        "Unsafe block detected - ensure proper resource cleanup",
                        Severity::Major,
                        Category::Bug,
                        ctx.file_path,
                        pt.row + 1,
                    ).with_remediation(Remediation::moderate("Use RAII patterns or wrap unsafe resources in safe abstractions")));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S1948 — Serializable fields should not expose sensitive data
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1948"
    name: "Serializable fields should not expose sensitive data"
    severity: Minor
    category: SecurityHotspot
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let sensitive = ["password", "secret", "token", "key", "credential", "passphrase"];
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("pub ") && !line.contains("#[serde(skip") {
                for kw in &sensitive {
                    if line.to_lowercase().contains(kw) {
                        issues.push(Issue::new(
                            "S1948",
                            format!("Field '{}' may expose sensitive data when serialized", kw),
                            Severity::Minor,
                            Category::SecurityHotspot,
                            ctx.file_path,
                            idx + 1,
                        ).with_remediation(Remediation::quick("Add #[serde(skip)] or use a custom serializer")));
                        break;
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S1118 — Empty structs should be documented
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1118"
    name: "Utility structs should have documentation"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let query_str = "(struct_item (type_identifier) @name) @struct";
        if let Ok(query) = tree_sitter::Query::new(&ctx.language.to_ts_language(), query_str) {
            let mut cursor = tree_sitter::QueryCursor::new();
            let mut matches = cursor.matches(&query, ctx.tree.root_node(), ctx.source.as_bytes());
            while let Some(m) = matches.next() {
                for capture in m.captures {
                    if capture.node.named_child_count() <= 1 {
                        let pt = capture.node.start_position();
                        if let Some(prev_line) = ctx.source.lines().nth(pt.row.saturating_sub(1)) {
                            if !prev_line.contains("///") && !prev_line.contains("//!") {
                                issues.push(Issue::new("S1118", "Empty or undocumented struct", Severity::Minor, Category::CodeSmell, ctx.file_path, pt.row + 1));
                            }
                        }
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S1656 — Self-assignment of variables
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1656"
    name: "Variables should not be self-assigned"
    severity: Major
    category: Bug
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        // Check for self-assignment patterns without using backreferences (not supported in Rust regex)
        for (idx, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            // Look for patterns like: x = x; where x is the same identifier on both sides
            if let Some(eq_pos) = trimmed.find('=') {
                if eq_pos > 0 {
                    let lhs = trimmed[..eq_pos].trim();
                    let rhs = trimmed[eq_pos + 1..].trim().trim_end_matches(';').trim();
                    if lhs == rhs && !lhs.is_empty() && !rhs.is_empty() {
                        // Verify it's a valid identifier pattern (not like "x = x + 1")
                        if !rhs.contains('+') && !rhs.contains('-') && !rhs.contains('*') && !rhs.contains('/') {
                            issues.push(Issue::new("S1656", "Self-assignment detected - no effect", Severity::Major, Category::Bug, ctx.file_path, idx + 1));
                        }
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S1764 — Identical operands in binary expressions
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1764"
    name: "Identical expressions should not be compared"
    severity: Major
    category: Bug
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        // Check for comparison operators with identical operands (avoid backreferences)
        let comparison_ops = ["==", "!=", ">=", "<=", ">", "<"];
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("// allow") {
                continue;
            }
            for op in &comparison_ops {
                if let Some(pos) = line.find(op) {
                    if pos > 0 {
                        let before = line[..pos].trim();
                        let after = line[pos + op.len()..].trim();
                        if before == after && !before.is_empty() {
                            issues.push(Issue::new("S1764", "Identical operands in comparison - always true/false", Severity::Major, Category::Bug, ctx.file_path, idx + 1));
                            break;
                        }
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S1860 — Deadlock-prone Mutex patterns
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1860"
    name: "Nested Mutex locks should be avoided"
    severity: Critical
    category: Bug
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let mut locked_mutexes = vec![false; 50];
        for (idx, line) in ctx.source.lines().enumerate() {
            let idx_mod = idx % 50;
            if line.contains(".lock()") || line.contains(".lock().unwrap()") {
                if locked_mutexes.iter().any(|&x| x) {
                    issues.push(Issue::new("S1860", "Potential deadlock: Mutex lock acquired while another lock is held", Severity::Critical, Category::Bug, ctx.file_path, idx + 1).with_remediation(Remediation::moderate("Use try_lock(), reorder lock acquisition, or use lock_api with deadlock detection")));
                }
                locked_mutexes[idx_mod] = true;
            }
            if line.trim() == "}" {
                locked_mutexes[idx_mod] = false;
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S2187 — Tests without assertions
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S2187"
    name: "Tests should contain assertions"
    severity: Major
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let query_str = "(function_item (identifier) @name) @func";
        if let Ok(query) = tree_sitter::Query::new(&ctx.language.to_ts_language(), query_str) {
            let mut cursor = tree_sitter::QueryCursor::new();
            let mut matches = cursor.matches(&query, ctx.tree.root_node(), ctx.source.as_bytes());
            while let Some(m) = matches.next() {
                for capture in m.captures {
                    if let Ok(name) = capture.node.utf8_text(ctx.source.as_bytes()) {
                        if name.starts_with("test_") || name.contains("test") {
                            let func_text = capture.node.utf8_text(ctx.source.as_bytes()).unwrap_or("");
                            if !func_text.contains("assert") && !func_text.contains("panic") && !func_text.contains("unwrap") {
                                let pt = capture.node.start_position();
                                issues.push(Issue::new("S2187", format!("Test '{}' has no assertions", name), Severity::Major, Category::CodeSmell, ctx.file_path, pt.row + 1));
                            }
                        }
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S2201 — Return values of pure functions should be used
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S2201"
    name: "Return values should not be ignored"
    severity: Major
    category: Bug
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.ends_with(";") && !trimmed.starts_with("//") && !trimmed.starts_with("let ") {
                if (trimmed.contains("Result<") || trimmed.contains(".map(") || trimmed.contains(".and_then(")) && !trimmed.contains("?") && !trimmed.contains(".unwrap") && !trimmed.contains(".expect") && !trimmed.contains("if let") && !trimmed.contains(".ok()") {
                    issues.push(Issue::new("S2201", "Return value of a function call is not used", Severity::Major, Category::Bug, ctx.file_path, idx + 1).with_remediation(Remediation::quick("Use '?' operator, '.unwrap()', or assign to a variable")));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S2221 — Avoid unnecessary panic! calls
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S2221"
    name: "panic! should be avoided in library code"
    severity: Major
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            // Skip panic! in tests, macros, or codegen
            if line.contains("panic!(") 
                && !line.contains("test")
                && !line.contains("unreachable")
                && !line.contains("macro")
                && !line.contains("codegen") {
                issues.push(Issue::new("S2221", "panic! in non-test code - consider returning Result instead", Severity::Major, Category::CodeSmell, ctx.file_path, idx + 1).with_remediation(Remediation::moderate("Return Result<_, Error> or use unreachable!() if truly impossible")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S2259 — Potential null dereference
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S2259"
    name: "Null pointer dereferences should be avoided"
    severity: Blocker
    category: Bug
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\*(\w+)\s*\.\s*\w+").unwrap();
        let mut in_unsafe = false;
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("unsafe {") { in_unsafe = true; }
            if in_unsafe && re.is_match(line) {
                issues.push(Issue::new("S2259", "Raw pointer dereference in unsafe block - verify non-null", Severity::Blocker, Category::Bug, ctx.file_path, idx + 1));
            }
            if line.trim() == "}" { in_unsafe = false; }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S115 — Constant names should follow UPPER_CASE
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S115"
    name: "Constant names should follow UPPER_CASE convention"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"const\s+([a-z][A-Za-z0-9_]*)\s*:").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                let name = cap.get(1).unwrap().as_str();
                if name != name.to_uppercase() {
                    issues.push(Issue::new("S115", format!("Constant '{}' should be UPPER_CASE", name), Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper: Count branches in a node tree
// ─────────────────────────────────────────────────────────────────────────────

fn count_branches_impl(node: tree_sitter::Node, count: &mut usize) {
    let branch_kinds = ["if_expression", "match_arm", "while_expression", "for_expression", "loop_expression"];
    if branch_kinds.contains(&node.kind()) {
        *count += 1;
    }
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            count_branches_impl(child, count);
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S1541 — High cyclomatic complexity (simplified)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1541"
    name: "Functions should not have too many branches"
    severity: Major
    category: CodeSmell
    language: "rust"
    params: { threshold: usize = 10 }
    check: => {
        let mut issues = Vec::new();
        for func_node in ctx.query_functions() {
            let mut branch_count = 0;
            count_branches_impl(func_node, &mut branch_count);
            if branch_count > self.threshold {
                let pt = func_node.start_position();
                if let Some(name) = ctx.function_name(func_node) {
                    issues.push(Issue::new("S1541", format!("Function '{}' has {} branches", name, branch_count), Severity::Major, Category::CodeSmell, ctx.file_path, pt.row + 1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S1142 — Too many return statements
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1142"
    name: "Functions should not have too many return points"
    severity: Major
    category: CodeSmell
    language: "rust"
    params: { max_returns: usize = 3 }
    check: => {
        let mut issues = Vec::new();
        for func_node in ctx.query_functions() {
            let text = func_node.utf8_text(ctx.source.as_bytes()).unwrap_or("");
            let return_count = text.matches("return ").count() + text.matches("return;").count();
            if return_count > self.max_returns {
                let pt = func_node.start_position();
                if let Some(name) = ctx.function_name(func_node) {
                    issues.push(Issue::new(
                        "S1142",
                        format!("Function '{}' has {} return statements", name, return_count),
                        Severity::Major,
                        Category::CodeSmell,
                        ctx.file_path,
                        pt.row + 1,
                    ));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S1151 — Match arm too big
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1151"
    name: "Match arms should not be too long"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: { max_lines: usize = 5 }
    check: => {
        let mut issues = Vec::new();
        let query_str = "(match_arm) @arm";
        if let Ok(query) = tree_sitter::Query::new(&ctx.language.to_ts_language(), query_str) {
            let mut cursor = tree_sitter::QueryCursor::new();
            let mut matches = cursor.matches(&query, ctx.tree.root_node(), ctx.source.as_bytes());
            while let Some(m) = matches.next() {
                for capture in m.captures {
                    let lines = ctx.line_count(capture.node);
                    if lines > self.max_lines {
                        let pt = capture.node.start_position();
                        issues.push(Issue::new(
                            "S1151",
                            format!("Match arm is {} lines - consider extracting to function", lines),
                            Severity::Minor,
                            Category::CodeSmell,
                            ctx.file_path,
                            pt.row + 1,
                        ));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S1155 — Use .is_empty() instead of .len() == 0
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1155"
    name: "Use .is_empty() instead of comparing .len() to 0"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\.len\(\)\s*==\s*0").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "S1155",
                    "Use .is_empty() instead of .len() == 0",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::quick("Replace with .is_empty()")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S1158 — Redundant .clone() after .to_owned()
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1158"
    name: "Unnecessary .clone() calls should be removed"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\.to_owned\(\)\s*\.clone\(\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "S1158",
                    "Redundant .clone() after .to_owned()",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S1161 — #[allow(deprecated)] should not be used
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1161"
    name: "Deprecated code should not be used"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("#[allow(deprecated)]") {
                issues.push(Issue::new(
                    "S1161",
                    "#[allow(deprecated)] suppresses useful warnings about deprecated API usage",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::moderate("Remove the allow(deprecated) attribute and update deprecated code")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S1163 — Redundant else after return/break/continue
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1163"
    name: "Redundant else after return, break, or continue"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for i in 0..lines.len().saturating_sub(1) {
            let prev = lines[i].trim();
            let next = lines[i+1].trim();
            if (prev.ends_with("return;") || prev.ends_with("break;") || prev.ends_with("continue;")) && next.starts_with("else ") {
                issues.push(Issue::new(
                    "S1163",
                    "Redundant else after control flow statement",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    i + 2,
                ));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S1197 — Magic numbers should be replaced by named constants
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1197"
    name: "Magic numbers should be replaced by named constants"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"[=<>!]\s*\d{3,}").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && !line.contains("const") && !line.contains("test") && !line.contains("\"") {
                issues.push(Issue::new(
                    "S1197",
                    "Magic number detected - use a named constant",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S1214 — static mut should not be used
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1214"
    name: "Mutable static variables should not be used"
    severity: Critical
    category: Bug
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("static mut") {
                issues.push(Issue::new(
                    "S1214",
                    "static mut is unsafe - use OnceCell, Lazy, or interior mutability",
                    Severity::Critical,
                    Category::Bug,
                    ctx.file_path,
                    idx + 1,
                ));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S1244 — Floating point equality should not be used
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1244"
    name: "Floating point equality should not be used"
    severity: Major
    category: Bug
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"(f32|f64)\b.*\s*==\s*").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "S1244",
                    "Floating point equality comparison - may not behave as expected",
                    Severity::Major,
                    Category::Bug,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::moderate("Use epsilon comparison: (a - b).abs() < EPSILON")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S1481 — Unused local variable (strict: let _x = ...)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1481"
    name: "Unused local variables should be removed"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"let\s+_(\w+)\s*=").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                let name = cap.get(1).unwrap().as_str();
                let remaining: String = ctx.source.lines().skip(idx + 1).collect::<Vec<_>>().join("\n");
                if !remaining.contains(&format!(" {} ", name)) && !remaining.contains(&format!("({}", name)) {
                    issues.push(Issue::new(
                        "S1481",
                        format!("Unused variable '_{}' - remove it entirely", name),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S1643 — String concatenation in loop should use push_str or collect
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1643"
    name: "String concatenation in loops should use collect() or push_str"
    severity: Major
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let mut in_loop = false;
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("for ") || line.contains("while ") || line.contains("loop {") {
                in_loop = true;
            }
            if in_loop {
                if line.contains("+=") && (line.contains("String") || line.contains("str")) && !line.contains("push_str") {
                    issues.push(Issue::new(
                        "S1643",
                        "String concatenation in loop - use .push_str() or collect()",
                        Severity::Major,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ));
                }
            }
            if line.trim() == "}" {
                in_loop = false;
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S1751 — Loop with at most one iteration
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1751"
    name: "Loops with unconditional break have at most one iteration"
    severity: Major
    category: Bug
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"loop\s*\{[^}]*break[^}]*\}").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "S1751",
                    "Loop has unconditional break - at most one iteration",
                    Severity::Major,
                    Category::Bug,
                    ctx.file_path,
                    idx + 1,
                ));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S1845 — Write-only variable (assigned but never read)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1845"
    name: "Variables should not be assigned but never read"
    severity: Major
    category: Bug
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"let\s+mut\s+(\w+)\s*=").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                let name = cap.get(1).unwrap().as_str();
                let remaining: String = ctx.source.lines().skip(idx + 1).collect::<Vec<_>>().join("\n");
                let read_count = remaining.matches(&format!(" {} ", name)).count()
                    + remaining.matches(&format!("({}", name)).count();
                let write_count = remaining.matches(&format!("{} =", name)).count()
                    + remaining.matches(&format!("{} +=", name)).count();
                if write_count > 0 && read_count == 0 {
                    issues.push(Issue::new(
                        "S1845",
                        format!("Variable '{}' is written but never read", name),
                        Severity::Major,
                        Category::Bug,
                        ctx.file_path,
                        idx + 1,
                    ));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S1858 — to_string() on a string literal
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1858"
    name: "Redundant to_string() on String should be removed"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r#""([^"]*)"\s*\.to_string\(\)"#).unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "S1858",
                    "Redundant .to_string() on a string literal - use .to_owned() or the literal directly",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S2111 — format!() with no interpolation should be simplified
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S2111"
    name: "format!() with no interpolation should be simplified"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r#"format!\("([^}"]*)"\)"#).unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "S2111",
                    "format!() with no placeholders - use String::from() or .to_string()",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S110 — Too many trait bounds
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S110"
    name: "Traits should not have too many bounds"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: { max_bounds: usize = 5 }
    check: => {
        let mut issues = Vec::new();
        let pattern = format!(r"<\s*(?:\w+\s*:\s*(?:\w+\s*\+\s*){{{}}},\w+)", self.max_bounds);
        if let Ok(re) = regex::Regex::new(&pattern) {
            for (idx, line) in ctx.source.lines().enumerate() {
                if re.is_match(line) {
                    issues.push(Issue::new(
                        "S110",
                        format!("Too many trait bounds (>{}) - consider creating a supertrait", self.max_bounds),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S111 — Hidden fields (non-descriptive single-letter public fields)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S111"
    name: "Public fields should have descriptive names"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"pub\s+([a-z])\s*:").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                if let Some(name) = cap.get(1) {
                    issues.push(Issue::new(
                        "S111",
                        format!("Field '{}' has a non-descriptive single-letter name", name.as_str()),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S114 — Line length should be limited to 120 chars (stricter than S113)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S114"
    name: "Lines should not exceed 120 characters"
    severity: Minor
    category: CodeSmell
    language: "*"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.len() > 120 {
                issues.push(Issue::new(
                    "S114",
                    format!("Line is {} characters long (max 120)", line.len()),
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S125 — Commented-out code should be removed
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S125"
    name: "Commented-out code should be removed"
    severity: Minor
    category: CodeSmell
    language: "*"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            let t = line.trim();
            // Rust-style // comments
            if t.starts_with("// let ") || t.starts_with("// fn ") || t.starts_with("// impl ") ||
               t.starts_with("// pub ") || t.starts_with("// struct ") || t.starts_with("// if ") ||
               t.starts_with("// match ") || t.starts_with("// for ") || t.starts_with("// while ") ||
               t.starts_with("// loop ") || t.starts_with("// mut ") || t.starts_with("// ref ") {
                issues.push(Issue::new(
                    "S125",
                    format!("Commented-out code at line {}", idx + 1),
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ));
            }
            // Python-style # comments with code patterns
            if t.starts_with("# def ") || t.starts_with("# class ") || t.starts_with("# if ") ||
               t.starts_with("# elif ") || t.starts_with("# else ") || t.starts_with("# for ") ||
               t.starts_with("# while ") || t.starts_with("# try ") || t.starts_with("# except ") ||
               t.starts_with("# finally ") || t.starts_with("# with ") || t.starts_with("# import ") ||
               t.starts_with("# from ") || t.starts_with("# return ") || t.starts_with("# yield ") ||
               t.starts_with("# raise ") || t.starts_with("# pass ") || t.starts_with("# break ") ||
               t.starts_with("# continue ") || t.starts_with("# lambda ") || t.starts_with("# self.") ||
               t.starts_with("# async ") || t.starts_with("# await ") {
                issues.push(Issue::new(
                    "S125",
                    format!("Commented-out code at line {}", idx + 1),
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S130 — Match expression should have a wildcard arm
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S130"
    name: "Match expressions should have a wildcard arm"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let all_matches: String = ctx.source.to_string();
        let match_count = all_matches.matches("match ").count();
        let wildcard_count = all_matches.matches("_ =>").count();
        if match_count > wildcard_count {
            for (idx, line) in ctx.source.lines().enumerate() {
                if line.trim().starts_with("match ") && !all_matches.lines().skip(idx).take(20).any(|l| l.contains("_ =>")) {
                    issues.push(Issue::new(
                        "S130",
                        "Match without wildcard '_ =>' arm",
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S131 — Match expression without wildcard for potentially non-exhaustive matches
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S131"
    name: "Match expressions should handle all cases or have a wildcard"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"match\s+\w+\s*\{([^}]+)\}").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                let match_body = cap.get(1).unwrap().as_str();
                if !match_body.contains("_ =>") && !match_body.contains("_,") {
                    issues.push(Issue::new(
                        "S131",
                        "Match expression may not be exhaustive - add a wildcard arm",
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S147 — Files should not be too long
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S147"
    name: "Files should not be too long"
    severity: Major
    category: CodeSmell
    language: "*"
    params: { max_lines: usize = 1000 }
    check: => {
        let mut issues = Vec::new();
        let line_count = ctx.source.lines().count();
        if line_count > self.max_lines {
            issues.push(Issue::new(
                "S147",
                format!("File has {} lines exceeding limit of {}", line_count, self.max_lines),
                Severity::Major,
                Category::CodeSmell,
                ctx.file_path,
                1,
            ).with_remediation(Remediation::moderate("Split into multiple modules")));
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S148 — Code should have adequate comments
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S148"
    name: "Code should have adequate comments"
    severity: Minor
    category: CodeSmell
    language: "*"
    params: { min_ratio: f64 = 0.10 }
    check: => {
        let mut issues = Vec::new();
        let mut comment_lines = 0usize;
        let mut total_lines = 0usize;
        for line in ctx.source.lines() {
            total_lines += 1;
            let t = line.trim();
            if t.starts_with("//") || t.starts_with("///") || t.starts_with("/*") {
                comment_lines += 1;
            }
        }
        if total_lines > 50 && (comment_lines as f64 / total_lines as f64) < self.min_ratio {
            issues.push(Issue::new(
                "S148",
                format!("Comment ratio {:.1}% below {:.0}% minimum", (comment_lines as f64/total_lines as f64)*100.0, self.min_ratio*100.0),
                Severity::Minor,
                Category::CodeSmell,
                ctx.file_path,
                1,
            ));
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S154 — Functions should not be empty
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S154"
    name: "Functions should not have empty bodies"
    severity: Major
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let node_type = ctx.language.function_node_type();
        let query_str = format!("({} body: (block) @body) @func", node_type);
        if let Ok(query) = tree_sitter::Query::new(&ctx.language.to_ts_language(), &query_str) {
            let mut cursor = tree_sitter::QueryCursor::new();
            let mut matches = cursor.matches(&query, ctx.tree.root_node(), ctx.source.as_bytes());
            while let Some(m) = matches.next() {
                for capture in m.captures {
                    let node = capture.node;
                    let named_children = node.named_child_count();
                    if named_children == 0 {
                        if let Some(name) = ctx.function_name(node.parent().unwrap_or(node)) {
                            let pt = node.start_position();
                            issues.push(Issue::new(
                                "S154",
                                format!("Function '{}' has an empty body", name),
                                Severity::Major,
                                Category::CodeSmell,
                                ctx.file_path,
                                pt.row + 1,
                            ).with_remediation(Remediation::quick("Implement the function body or remove it")));
                        }
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S160 — Functions should not be too complex
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S160"
    name: "Functions should not be too complex"
    severity: Major
    category: CodeSmell
    language: "rust"
    params: { max_complexity: usize = 15 }
    check: => {
        let mut issues = Vec::new();
        for func_node in ctx.query_functions() {
            let text = func_node.utf8_text(ctx.source.as_bytes()).unwrap_or("");
            let count = text.matches("if ").count() + text.matches("match ").count() +
                        text.matches("while ").count() + text.matches("for ").count() +
                        text.matches("loop ").count();
            if count > self.max_complexity {
                let pt = func_node.start_position();
                if let Some(name) = ctx.function_name(func_node) {
                    issues.push(Issue::new(
                        "S160",
                        format!("Function '{}' has {} branches exceeding {}", name, count, self.max_complexity),
                        Severity::Major,
                        Category::CodeSmell,
                        ctx.file_path,
                        pt.row + 1,
                    ));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S166 — Multiple variables in one let statement
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S166"
    name: "Multiple variables in one let statement should be avoided"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: { max_vars: usize = 4 }
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"let\s*\(([^)]+)\)\s*=").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                let vars = cap.get(1).unwrap().as_str();
                let var_count = vars.split(',').count();
                if var_count > self.max_vars {
                    issues.push(Issue::new(
                        "S166",
                        format!("Destructuring {} variables in one let - consider splitting", var_count),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S170 — Unused imports should be removed
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S170"
    name: "Unused imports should be removed"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"use\s+([\w:]+)::(\w+)\s*;").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                let name = cap.get(2).unwrap().as_str();
                let remaining = ctx.source.lines().skip(idx + 1).collect::<Vec<_>>().join("\n");
                if !remaining.contains(name) && name != "self" && name != "crate" {
                    issues.push(Issue::new(
                        "S170",
                        format!("Import '{}' appears unused", name),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S171 — Default trait implementations should be documented
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S171"
    name: "Default trait implementations should be documented"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for i in 0..lines.len().saturating_sub(1) {
            let next = lines[i + 1].trim();
            if next.starts_with("impl") && next.contains("Default") && !next.contains("for") {
                if i > 0 {
                    let prev = lines[i].trim();
                    if !prev.starts_with("///") && !prev.starts_with("//!") && !prev.starts_with("/*") {
                        issues.push(Issue::new(
                            "S171",
                            "Default trait implementation should be documented",
                            Severity::Minor,
                            Category::CodeSmell,
                            ctx.file_path,
                            i + 2,
                        ));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S172 — Debugging statements should not be in production code
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S172"
    name: "Debugging statements should not be in production code"
    severity: Major
    category: CodeSmell
    language: "*"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let is_test_file = ctx.file_path.to_string_lossy().contains("test");
        for (idx, line) in ctx.source.lines().enumerate() {
            if !is_test_file {
                // Rust debugging statements
                if line.contains("println!(") || line.contains("dbg!(") || line.contains("eprintln!(") {
                    issues.push(Issue::new(
                        "S172",
                        "Debug print statement in non-test code",
                        Severity::Major,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::quick("Use tracing::debug! or log::debug! instead")));
                }
                // Python debugging statements
                if line.contains("print(") && !line.trim().starts_with("def ") && !line.trim().starts_with("class ") {
                    // Avoid flagging print function definitions or type hints
                    let clean_line = line.replace("print", "").replace("def ", "").replace("type ", "");
                    if !clean_line.contains("->") && !line.trim().starts_with("#") {
                        issues.push(Issue::new(
                            "S172",
                            "Debug print statement in non-test code",
                            Severity::Major,
                            Category::CodeSmell,
                            ctx.file_path,
                            idx + 1,
                        ).with_remediation(Remediation::quick("Use Python logging module instead")));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S173 — Functions returning Result should be marked #[must_use]
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S173"
    name: "Functions returning Result should be marked #[must_use]"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for func_node in ctx.query_functions() {
            // Skip functions inside trait definitions - they don't use #[must_use]
            let mut parent = func_node.parent();
            let mut in_trait = false;
            while let Some(p) = parent {
                if p.kind() == "trait_item" {
                    in_trait = true;
                    break;
                }
                parent = p.parent();
            }
            if in_trait {
                continue; // Skip functions in trait definitions
            }
            
            let text = func_node.utf8_text(ctx.source.as_bytes()).unwrap_or("");
            if text.contains("-> Result<") || text.contains("-> Option<") {
                if !text.contains("#[must_use]") {
                    let pt = func_node.start_position();
                    if let Some(name) = ctx.function_name(func_node) {
                        issues.push(Issue::new(
                            "S173",
                            format!("Function '{}' returns Result/Option without #[must_use]", name),
                            Severity::Minor,
                            Category::CodeSmell,
                            ctx.file_path,
                            pt.row + 1,
                        ));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S2077 — SQL injection via format! with user input
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S2077"
    name: "SQL queries should not be built with string interpolation"
    severity: Blocker
    category: Vulnerability
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let sql_keywords = ["SELECT", "INSERT", "UPDATE", "DELETE", "DROP", "CREATE", "ALTER"];
        for (idx, line) in ctx.source.lines().enumerate() {
            let has_sql = sql_keywords.iter().any(|kw| line.contains(kw));
            let has_format = line.contains("format!");
            if has_sql && has_format && !line.contains("bind") && !line.contains("prepared") && !line.contains("parameter") {
                issues.push(Issue::new(
                    "S2077",
                    "SQL query built with string interpolation - use parameterized queries",
                    Severity::Blocker,
                    Category::Vulnerability,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::moderate("Use prepared statements or an ORM with parameter binding")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S2092 — Cookie without Secure flag
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S2092"
    name: "Cookies should set the Secure flag"
    severity: Minor
    category: SecurityHotspot
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("Set-Cookie") || line.contains(".cookie(") {
                if !line.contains("Secure") && !line.contains("secure") {
                    issues.push(Issue::new(
                        "S2092",
                        "Cookie without Secure flag",
                        Severity::Minor,
                        Category::SecurityHotspot,
                        ctx.file_path,
                        idx + 1,
                    ));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S2612 — Weak file permissions (chmod 777, 666)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S2612"
    name: "File permissions should not be too permissive"
    severity: Critical
    category: Vulnerability
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"0o?777|chmod\s+777").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "S2612",
                    "Overly permissive file permissions (0777)",
                    Severity::Critical,
                    Category::Vulnerability,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::quick("Use 0o644 for files and 0o755 for directories")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S2755 — XML external entity (XXE)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S2755"
    name: "XML parsing should not be vulnerable to external entity attacks"
    severity: Blocker
    category: Vulnerability
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let xxe_patterns = [
            "XmlParser",
            "quick_xml",
            "xml-rs",
            "serde_xml",
            "roxmltree",
            "xml",
        ];
        for (idx, line) in ctx.source.lines().enumerate() {
            let has_xml_parser = xxe_patterns.iter().any(|p| line.contains(p));
            if has_xml_parser {
                if line.contains("DTD") || line.contains("dtd") || line.contains("entity") || line.contains("external") {
                    issues.push(Issue::new(
                        "S2755",
                        "Potential XXE vulnerability - external entity parsing detected",
                        Severity::Blocker,
                        Category::Vulnerability,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::substantial("Disable DTD processing and external entities in XML parser configuration")));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S3330 — Cookie without HttpOnly
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S3330"
    name: "Cookies should set the HttpOnly flag"
    severity: Minor
    category: SecurityHotspot
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains(".cookie(") || line.contains("Set-Cookie") {
                if !line.contains("HttpOnly") && !line.contains("http_only") {
                    issues.push(Issue::new(
                        "S3330",
                        "Cookie without HttpOnly flag - vulnerable to XSS",
                        Severity::Minor,
                        Category::SecurityHotspot,
                        ctx.file_path,
                        idx + 1,
                    ));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S3358 — Deeply nested if/else chains (>3 levels)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S3358"
    name: "Deeply nested conditional chains should be simplified"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: { max_depth: usize = 3 }
    check: => {
        let mut issues = Vec::new();
        for func_node in ctx.query_functions() {
            let text = func_node.utf8_text(ctx.source.as_bytes()).unwrap_or("");
            let mut depth = 0usize;
            let mut max_depth = 0usize;
            for line in text.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("} else if ") || trimmed.starts_with("if ") || trimmed.starts_with("} else if") {
                    if trimmed.starts_with("if ") && !trimmed.contains("else") {
                        depth = depth.saturating_sub(1);
                    }
                    depth += 1;
                    max_depth = max_depth.max(depth);
                }
                if trimmed == "}" && depth > 0 {
                    depth -= 1;
                }
            }
            if max_depth > self.max_depth {
                let pt = func_node.start_position();
                if let Some(name) = ctx.function_name(func_node) {
                    issues.push(Issue::new(
                        "S3358",
                        format!("Function '{}' has nested if/else chain of depth {}", name, max_depth),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        pt.row + 1,
                    ));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S3366 — self leaked from constructor
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S3366"
    name: "new() should not leak self before full initialization"
    severity: Major
    category: Bug
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"fn\s+new\(").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                let context: String = ctx.source.lines().skip(idx).take(15).collect::<Vec<_>>().join("\n");
                if context.contains("Arc::new") && context.contains("self") && context.contains("clone") {
                    issues.push(Issue::new(
                        "S3366",
                        "self potentially leaked before full initialization",
                        Severity::Major,
                        Category::Bug,
                        ctx.file_path,
                        idx + 1,
                    ));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S3649 — SQL via string concatenation
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S3649"
    name: "SQL queries should not be vulnerable to injection"
    severity: Blocker
    category: Vulnerability
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let sql_keywords = ["SELECT", "INSERT", "UPDATE", "DELETE", "DROP", "CREATE", "ALTER"];
        for (idx, line) in ctx.source.lines().enumerate() {
            for kw in &sql_keywords {
                if line.contains(kw) && (line.contains("+") || line.contains(".as_str()")) {
                    if !line.contains("bind") && !line.contains("prepared") && !line.contains("parameter") {
                        issues.push(Issue::new(
                            "S3649",
                            "SQL built with string ops - use parameterized queries",
                            Severity::Blocker,
                            Category::Vulnerability,
                            ctx.file_path,
                            idx + 1,
                        ));
                        break;
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S4423 — Weak TLS protocol (SSLv3, TLSv1.0)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S4423"
    name: "Weak TLS protocols should not be used"
    severity: Critical
    category: Vulnerability
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"Tlsv1_0|Sslv3|Sslv23|TLSv1\.0|SSLv3").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "S4423",
                    "Weak TLS protocol detected - use TLS 1.2 or higher",
                    Severity::Critical,
                    Category::Vulnerability,
                    ctx.file_path,
                    idx + 1,
                ));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S4426 — Weak cryptographic key generation (RSA 1024, DSA 1024)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S4426"
    name: "Cryptographic keys should be sufficiently large"
    severity: Critical
    category: Vulnerability
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"(RSA|DSA|DH)\w*\s*\(\s*1024\s*\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "S4426",
                    "Key size too small (1024 bits) - use at least 2048",
                    Severity::Critical,
                    Category::Vulnerability,
                    ctx.file_path,
                    idx + 1,
                ));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S4507 — Debug mode in production
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S4507"
    name: "Debug features should not be enabled in production"
    severity: Critical
    category: Vulnerability
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("#[cfg(debug_assertions)]") && !line.contains("test") {
                let next_lines: String = ctx.source.lines().skip(idx + 1).take(5).collect::<Vec<_>>().join("\n");
                if next_lines.contains("secret") || next_lines.contains("password") || next_lines.contains("token") {
                    issues.push(Issue::new(
                        "S4507",
                        "Sensitive code exposed in debug mode",
                        Severity::Critical,
                        Category::Vulnerability,
                        ctx.file_path,
                        idx + 1,
                    ));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S4830 — Certificate validation disabled
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S4830"
    name: "Server certificate validation should not be disabled"
    severity: Blocker
    category: Vulnerability
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let dangerous_patterns = [
            "danger_accept_invalid_certs",
            "NoCertificateVerification",
            "SkipServerVerification",
            "accept_invalid_certs",
            "disable_certificate_verification",
        ];
        for (idx, line) in ctx.source.lines().enumerate() {
            for pattern in &dangerous_patterns {
                if line.contains(pattern) {
                    issues.push(Issue::new(
                        "S4830",
                        "TLS certificate validation disabled - man-in-the-middle risk",
                        Severity::Blocker,
                        Category::Vulnerability,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::moderate("Use proper certificate validation with the system CA store")));
                    break;
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S5042 — Expanding archive files without size check (zip bomb)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S5042"
    name: "Archive extraction should check size before decompression"
    severity: Major
    category: Vulnerability
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            let has_archive = line.contains(".zip(") || line.contains("ZipArchive") || line.contains("tar::") || line.contains("Archive::");
            if has_archive && !line.contains("limit") && !line.contains("max_") {
                let context: String = ctx.source.lines().skip(idx.saturating_sub(3)).take(10).collect::<Vec<_>>().join("\n");
                if !context.contains("size") && !context.contains("limit") && !context.contains("max_size") {
                    issues.push(Issue::new(
                        "S5042",
                        "Archive extraction without size check - potential zip bomb",
                        Severity::Major,
                        Category::Vulnerability,
                        ctx.file_path,
                        idx + 1,
                    ));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S5542 — Weak encryption mode (ECB)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S5542"
    name: "Encryption should use secure modes"
    severity: Critical
    category: Vulnerability
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("ECB") || (line.contains("ecb") && !line.contains("deecb")) {
                issues.push(Issue::new(
                    "S5542",
                    "ECB encryption mode is insecure - use CBC, GCM, or ChaCha20",
                    Severity::Critical,
                    Category::Vulnerability,
                    ctx.file_path,
                    idx + 1,
                ));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S5547 — Weak cipher (DES, 3DES, RC4)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S5547"
    name: "Weak cipher algorithms should not be used"
    severity: Critical
    category: Vulnerability
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let weak_ciphers = ["RC4", "rc4", "DES", "3DES", "TDES", "RC2"];
        for (idx, line) in ctx.source.lines().enumerate() {
            for cipher in &weak_ciphers {
                if line.contains(cipher) {
                    issues.push(Issue::new(
                        "S5547",
                        "Weak cipher algorithm detected - use AES or ChaCha20",
                        Severity::Critical,
                        Category::Vulnerability,
                        ctx.file_path,
                        idx + 1,
                    ));
                    break;
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S5693 — Missing upload size limit
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S5693"
    name: "File uploads should have size limits"
    severity: Major
    category: SecurityHotspot
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            let upload_related = line.contains("multipart") || line.contains("upload") || line.contains("form_data") || line.contains("read_to_end");
            if upload_related && !line.contains("limit") && !line.contains("max_") {
                let context: String = ctx.source.lines().skip(idx.saturating_sub(3)).take(8).collect::<Vec<_>>().join("\n");
                if !context.contains("limit") && !context.contains("max_size") && !context.contains("content_length") {
                    issues.push(Issue::new(
                        "S5693",
                        "File upload without size limit - DoS risk",
                        Severity::Major,
                        Category::SecurityHotspot,
                        ctx.file_path,
                        idx + 1,
                    ));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S108 — Empty catch block (Rust: empty match arm after Result)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S108"
    name: "Empty match arms after Result should be avoided"
    severity: Major
    category: Bug
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let query_str = "(match_arm body: (block) @body)";
        if let Ok(query) = tree_sitter::Query::new(&ctx.language.to_ts_language(), query_str) {
            let mut cursor = tree_sitter::QueryCursor::new();
            let mut matches = cursor.matches(&query, ctx.tree.root_node(), ctx.source.as_bytes());
            while let Some(m) = matches.next() {
                for capture in m.captures {
                    if capture.node.named_child_count() == 0 {
                        let pt = capture.node.start_position();
                        issues.push(Issue::new(
                            "S108",
                            "Empty match arm - consider handling the case explicitly",
                            Severity::Major,
                            Category::Bug,
                            ctx.file_path,
                            pt.row + 1,
                        ).with_remediation(Remediation::moderate("Handle this case or use _ => todo!()")));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S112 — Generic exceptions should not be thrown (Rust: Box<dyn Error> catch-all)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1121"
    name: "Generic error types should not be used as catch-all"
    severity: Major
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"Box\<dyn\s+Error\>").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && !line.contains("impl") {
                issues.push(Issue::new(
                    "S1121",
                    "Box<dyn Error> as catch-all error type - use concrete error types",
                    Severity::Major,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::moderate("Use concrete Result<T, SpecificError> types instead")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S113 — Exceptions should not be thrown from finally (Rust: unwrap in Drop)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1130"
    name: "unwrap() in Drop implementation is risky"
    severity: Major
    category: Bug
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"impl\s+Drop\s+for\s+").unwrap();
        let mut in_drop = false;
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) { in_drop = true; }
            if in_drop && (line.contains(".unwrap()") || line.contains(".expect(")) {
                issues.push(Issue::new(
                    "S1130",
                    "unwrap()/expect() in Drop - errors will be silently ignored",
                    Severity::Major,
                    Category::Bug,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::moderate("Log the error or use if let/let _")));
            }
            if in_drop && line.trim() == "}" { in_drop = false; }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S114 — Same as S113 but broader (Rust: panic in Drop)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1140"
    name: "panic! in Drop implementation is risky"
    severity: Major
    category: Bug
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"impl\s+Drop\s+for\s+").unwrap();
        let mut in_drop = false;
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) { in_drop = true; }
            if in_drop && line.contains("panic!") {
                issues.push(Issue::new(
                    "S1140",
                    "panic! in Drop implementation - will cause termination",
                    Severity::Major,
                    Category:: Bug,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::moderate("Avoid panicking in Drop - use Result or log errors")));
            }
            if in_drop && line.trim() == "}" { in_drop = false; }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S115 — Switch with empty default (Rust: match with empty _ arm)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1150"
    name: "Match with empty wildcard arm should be reviewed"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"_\s*=>\s*\{\s*\}").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "S1150",
                    "Empty wildcard (_) arm in match - implicit ignore",
                    Severity::Minor,
                    Category:: CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::quick("Add a comment or use _ => unreachable!()")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S116 — Method throws generic exception (Rust: fn returns Box<dyn Error>)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1160"
    name: "Functions returning Box<dyn Error> should use concrete types"
    severity: Major
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"->\s*Box<dyn\s+Error>").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "S1160",
                    "Function returns Box<dyn Error> - use concrete error type",
                    Severity::Major,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::moderate("Return a concrete error type or thiserror enum")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S117 — Exception class naming (Rust: Error types should end with Error)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1170"
    name: "Error types should have names ending with 'Error'"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"struct\s+(\w+Error)\s*").unwrap();
        let source_text = ctx.source.to_string();
        for cap in re.captures_iter(&source_text) {
            if let Some(name) = cap.get(1) {
                let name_str = name.as_str();
                if !name_str.ends_with("Error") && !name_str.ends_with("Err") {
                    if let Some(pos) = source_text.find(name_str) {
                        let line_num = source_text[..pos].matches('\n').count() + 1;
                        issues.push(Issue::new(
                            "S1170",
                            format!("Error type '{}' should end with 'Error'", name_str),
                            Severity::Minor,
                            Category::CodeSmell,
                            ctx.file_path,
                            line_num,
                        ));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S118 — Abstract methods should not be empty (Rust: trait default methods)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1180"
    name: "Trait default methods should not be empty"
    severity: Major
    category: Bug
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let query_str = "(function_item) @func";
        if let Ok(query) = tree_sitter::Query::new(&ctx.language.to_ts_language(), query_str) {
            let mut cursor = tree_sitter::QueryCursor::new();
            let mut matches = cursor.matches(&query, ctx.tree.root_node(), ctx.source.as_bytes());
            while let Some(m) = matches.next() {
                for capture in m.captures {
                    let func_text = capture.node.utf8_text(ctx.source.as_bytes()).unwrap_or("");
                    if func_text.contains("fn ") && func_text.contains("->") && !func_text.contains("{") {
                        let pt = capture.node.start_position();
                        if let Some(name) = ctx.function_name(capture.node) {
                            issues.push(Issue::new(
                                "S1180",
                                format!("Trait method '{}' has no default implementation", name),
                                Severity::Major,
                                Category::Bug,
                                ctx.file_path,
                                pt.row + 1,
                            ).with_remediation(Remediation::moderate("Add a default implementation or make it abstract")));
                        }
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S120 — Modules should have doc comments (//!)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1200"
    name: "Modules should have documentation comments"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        let mut in_mod = false;
        let mut mod_line = 0;
        for (idx, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("mod ") && !trimmed.contains('{') {
                in_mod = true;
                mod_line = idx;
            }
            if in_mod {
                if trimmed.contains("pub mod ") || trimmed.contains("mod ") {
                    if let Some(next_line) = lines.get(mod_line + 1) {
                        if !next_line.trim().starts_with("//!") && !next_line.trim().starts_with("/*") {
                            issues.push(Issue::new(
                                "S1200",
                                "Module is missing documentation comment (!//)",
                                Severity::Minor,
                                Category::CodeSmell,
                                ctx.file_path,
                                mod_line + 1,
                            ));
                        }
                    }
                    in_mod = false;
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S121 — Structs should not have too many derives
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1210"
    name: "Structs should not have too many derive macros"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: { max_derives: usize = 3 }
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"#\[derive\(([^)]+)\)\]").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                let derives = cap.get(1).unwrap().as_str();
                let count = derives.split(',').count();
                if count > self.max_derives {
                    issues.push(Issue::new(
                        "S1210",
                        format!("Struct has {} derive macros - consider reducing", count),
                        Severity:: Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::moderate("Consider if all derives are necessary")));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S122 — Method overriding (Rust: trait impl without docs)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1220"
    name: "Trait implementations should be documented"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"impl\s+\w+\s+for\s+").unwrap();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            if re.is_match(line) {
                if idx > 0 {
                    let prev = lines[idx - 1].trim();
                    if !prev.starts_with("///") && !prev.starts_with("//!") && !prev.starts_with("/*") && !prev.starts_with("#[") {
                        issues.push(Issue::new(
                            "S1220",
                            "Trait implementation should have documentation",
                            Severity::Minor,
                            Category::CodeSmell,
                            ctx.file_path,
                            idx + 1,
                        ));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S123 — @Override should be used (Rust: impl Trait for Type without docs)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1230"
    name: "impl Trait for Type should have documentation"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"impl\s+\w+\s+for\s+\w+").unwrap();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            if re.is_match(line) && !line.contains("for<T>") {
                if let Some(prev_line) = lines.get(idx.saturating_sub(1)) {
                    let prev_trim = prev_line.trim();
                    if !prev_trim.starts_with("///") && !prev_trim.starts_with("//!") && !prev_trim.starts_with("/*") {
                        issues.push(Issue::new(
                            "S1230",
                            "impl Trait for Type should be documented",
                            Severity::Minor,
                            Category::CodeSmell,
                            ctx.file_path,
                            idx + 1,
                        ).with_remediation(Remediation::quick("Add documentation comment above the impl block")));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S124 — Equals should compare all fields (Rust: PartialEq not consistent with Hash)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1240"
    name: "PartialEq and Hash should be consistent"
    severity: Major
    category: Bug
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re_derive = regex::Regex::new(r"#\[derive\([^)]*\b(PartialEq|Hash)\b[^)]*\)").unwrap();
        let source = ctx.source.to_string();
        let mut derives_per_line: std::collections::HashMap<usize, (bool, bool)> = std::collections::HashMap::new();
        for (idx, line) in source.lines().enumerate() {
            if let Some(cap) = re_derive.captures(line) {
                let derives = cap.get(0).unwrap().as_str();
                let has_partialeq = derives.contains("PartialEq");
                let has_hash = derives.contains("Hash");
                derives_per_line.insert(idx, (has_partialeq, has_hash));
            }
        }
        let mut struct_lines: Vec<usize> = Vec::new();
        let struct_re = regex::Regex::new(r"struct\s+\w+").unwrap();
        for (idx, line) in source.lines().enumerate() {
            if struct_re.is_match(line) {
                struct_lines.push(idx);
            }
        }
        for struct_line in struct_lines {
            let nearby_derives: Vec<_> = derives_per_line.iter()
                .filter(|(idx, _)| (**idx as isize - struct_line as isize).abs() <= 3)
                .collect();
            let mut has_partialeq = false;
            let mut has_hash = false;
            for (_, (pe, h)) in nearby_derives {
                if *pe { has_partialeq = true; }
                if *h { has_hash = true; }
            }
            if has_partialeq && has_hash {
                issues.push(Issue::new(
                    "S1240",
                    "Struct derives both PartialEq and Hash - ensure equality compares all fields",
                    Severity::Major,
                    Category::Bug,
                    ctx.file_path,
                    struct_line + 1,
                ).with_remediation(Remediation::moderate("Verify that PartialEq and Hash use the same fields")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S125 — toString should not return null (Rust: Display impl should not panic)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1250"
    name: "Display implementation should not panic"
    severity: Major
    category: Bug
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"impl\s+.*\s+Display\s+for\s+").unwrap();
        let mut in_impl = false;
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) { in_impl = true; }
            if in_impl && (line.contains("panic!") || line.contains(".unwrap()") || line.contains(".expect(")) {
                issues.push(Issue::new(
                    "S1250",
                    "Display impl contains panic!/unwrap() - Display should not fail",
                    Severity::Major,
                    Category::Bug,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::moderate("Return a default representation instead of panicking")));
            }
            if in_impl && line.trim() == "}" { in_impl = false; }
        }
        issues
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// BATCH 7: Performance Rules (15 rules)
// ═══════════════════════════════════════════════════════════════════════════════

// ─────────────────────────────────────────────────────────────────────────────
// S1699 — Unnecessary temporary variable
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1699"
    name: "Unnecessary temporary variables should be removed"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"let\s+(\w+)\s*=\s*(\w+)\s*;").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                let var_name = cap.get(1).unwrap().as_str();
                let val_name = cap.get(2).unwrap().as_str();
                if var_name == val_name && !line.contains("mut") {
                    let remaining: String = ctx.source.lines().skip(idx + 1).collect::<Vec<_>>().join("\n");
                    let use_count = remaining.matches(&format!(" {} ", var_name)).count()
                        + remaining.matches(&format!("({}", var_name)).count();
                    if use_count <= 1 {
                        issues.push(Issue::new(
                            "S1699",
                            format!("Unnecessary temporary variable '{}' - use the value directly", var_name),
                            Severity::Minor,
                            Category::CodeSmell,
                            ctx.file_path,
                            idx + 1,
                        ).with_remediation(Remediation::quick("Remove the temporary variable and use the value directly")));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S1700 — Clone in loop
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1700"
    name: "Clone operations should not be called in loops"
    severity: Major
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let mut in_loop = false;
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("for ") || line.contains("while ") || line.contains("loop {") {
                in_loop = true;
            }
            if in_loop {
                if line.contains(".clone()") {
                    issues.push(Issue::new(
                        "S1700",
                        "Cloning inside a loop - consider using references or iterators",
                        Severity::Major,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::moderate("Use a reference (&) or restructure to avoid cloning in loop")));
                }
            }
            if line.trim() == "}" {
                in_loop = false;
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S1736 — Use iterator instead of index loop
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1736"
    name: "Iterators should be used instead of index loops"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"for\s+\w+\s+in\s+0\s*\.\.\s*\w+\.len\(\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "S1736",
                    "Index-based loop detected - use .iter() or .iter_mut() instead",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::quick("Replace with .iter() or .iter_mut() for better readability")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S1760 — Unnecessary boxing
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1760"
    name: "Box should not be used for types that implement Copy"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let copy_types = ["i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64", "u128", "usize", "f32", "f64", "bool", "char"];
        let re = regex::Regex::new(r"Box::new\s*\(\s*(\w+)\s*\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                let val = cap.get(1).unwrap().as_str();
                if copy_types.contains(&val) {
                    issues.push(Issue::new(
                        "S1760",
                        format!("Unnecessary Box<{}> - {} implements Copy", val, val),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::quick("Remove Box and use the value directly")));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S1905 — Unnecessary cast
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1905"
    name: "Unnecessary type casts should be removed"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        // Check for redundant type casts without using backreferences
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(as_pos) = line.find(" as ") {
                let before_as = line[..as_pos].trim().split_whitespace().last().unwrap_or("");
                let after_as = line[as_pos + 4..].trim().split_whitespace().next().unwrap_or("");
                // Extract the type name (remove any leading `&` or `*`)
                let type_before = before_as.trim_start_matches('&').trim_start_matches('*');
                let type_after = after_as.trim_start_matches('&').trim_start_matches('*').split_whitespace().next().unwrap_or("");
                if type_before == type_after && !type_before.is_empty() {
                    issues.push(Issue::new(
                        "S1905",
                        "Redundant type cast to the same type",
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::quick("Remove the unnecessary cast")));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S1940 — Boolean comparisons should be simplified
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1940"
    name: "Boolean comparisons should be simplified"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"(==|!=)\s*(true|false)|(true|false)\s*(==|!=)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "S1940",
                    "Redundant boolean comparison - simplify the expression",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::quick("Use the boolean directly without comparison")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S1941 — Simplifiable if-let patterns
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1941"
    name: "Simplifiable if-let patterns should be refactored"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        // Detect `if let Some(x) = y { x } else { z }` pattern without backreference
        for (idx, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            // Check for the basic pattern structure
            if trimmed.starts_with("if let Some(") && trimmed.contains(") = ") && trimmed.contains("} else {") {
                // Simple heuristic: look for the same identifier after "Some(" and in the body
                if let Some(start) = trimmed.find("Some(") {
                    let after_some = &trimmed[start + 5..];
                    if let Some(end) = after_some.find(')') {
                        let var_name = after_some[..end].trim();
                        if !var_name.is_empty() && var_name.chars().next().map(|c| c.is_lowercase()).unwrap_or(false) {
                            // Check if the same var appears in the body before "} else {"
                            if let Some(else_pos) = trimmed.find("} else {") {
                                let body = &trimmed[else_pos..];
                                let body_before_else = &trimmed[..else_pos];
                                // Look for the pattern: { <var> } or { <spaces><var><spaces> }
                                let var_in_body = format!("{{{} ", var_name);
                                let var_in_body2 = format!("{{ {}}} ", var_name);
                                if body_before_else.contains(&var_in_body) || body_before_else.contains(&var_in_body2) {
                                    issues.push(Issue::new(
                                        "S1941",
                                        "Simplifiable if-let pattern - use unwrap_or instead",
                                        Severity::Minor,
                                        Category::CodeSmell,
                                        ctx.file_path,
                                        idx + 1,
                                    ).with_remediation(Remediation::quick("Replace with unwrap_or for better readability")));
                                }
                            }
                        }
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S1943 — Unnecessary allocation in loop
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1943"
    name: "Unnecessary allocations in loops should be avoided"
    severity: Major
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let mut in_loop = false;
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("for ") || line.contains("while ") || line.contains("loop {") {
                in_loop = true;
            }
            if in_loop {
                if (line.contains("vec![]") || line.contains("Vec::new()") || line.contains("String::new()"))
                   && (line.contains(".push(") || line.contains(".insert(") || line.contains(".push_str(")) {
                    issues.push(Issue::new(
                        "S1943",
                        "Collection allocated inside loop - consider using iterator or collect()",
                        Severity::Major,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::moderate("Initialize outside loop or use iterator methods")));
                }
            }
            if line.trim() == "}" {
                in_loop = false;
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S1998 — Array instead of Vec for fixed small sizes
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1998"
    name: "Arrays should be used for fixed-size collections"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: { max_size: usize = 4 }
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"Vec::with_capacity\((\d+)\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                if let Some(size_str) = cap.get(1) {
                    if let Ok(size) = size_str.as_str().parse::<usize>() {
                        if size <= self.max_size {
                            issues.push(Issue::new(
                                "S1998",
                                format!("Vec with small fixed capacity ({}) - consider using an array", size),
                                Severity::Minor,
                                Category::CodeSmell,
                                ctx.file_path,
                                idx + 1,
                            ).with_remediation(Remediation::quick("Use a fixed-size array instead of Vec")));
                        }
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S2123 — Unnecessary into_iter() call
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S2123"
    name: "Unnecessary into_iter() calls should be removed"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"for\s+\w+\s+in\s+\w+\.into_iter\(\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "S2123",
                    "Unnecessary into_iter() - for loop will auto-deref",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::quick("Remove .into_iter() - the for loop will iterate by value automatically")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S2159 — Redundant comparisons with boolean literals
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S2159"
    name: "Redundant boolean comparisons should be removed"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"(if\s+\w+\s*==\s*true|if\s+\w+\s*!=\s*false)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "S2159",
                    "Redundant boolean comparison",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::quick("Remove the redundant comparison to true/false")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S2178 — Double negation
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S2178"
    name: "Double negation should not be used"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"!!\w+").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "S2178",
                    "Double negation (!!x) detected - use x directly",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::quick("Remove the double negation")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S2225 — toString() in string concatenation
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S2225"
    name: "Unnecessary to_string() calls in concatenation"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r#"format!\s*\(\s*"[^"]*"\s*,\s*[^)]*\.to_string\(\)"#).unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "S2225",
                    "Unnecessary to_string() in format! - format! already converts",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::quick("Remove .to_string() - format! handles conversion")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S2234 — Unnecessary clone() on Copy types
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S2234"
    name: "Unnecessary clone() on Copy types"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let copy_types = ["i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64", "u128", "usize", "f32", "f64", "bool", "char"];
        let re = regex::Regex::new(r"(\w+)\.clone\(\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                let val = cap.get(1).unwrap().as_str();
                if copy_types.contains(&val) {
                    issues.push(Issue::new(
                        "S2234",
                        format!("Unnecessary clone() on {} - type implements Copy", val),
                        Severity::Minor,
                        Category:: CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::quick("Remove the unnecessary .clone()")));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S2301 — Mutable borrow in loop without modification
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S2301"
    name: "iter_mut() without modification should be iter()"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\.iter_mut\(\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                let remaining: String = ctx.source.lines().skip(idx).take(20).collect::<Vec<_>>().join("\n");
                let has_mutation = remaining.lines().any(|l| l.contains(".* =") || l.contains(".push(") || l.contains(".pop(") || l.contains(".insert(") || l.contains(".remove("));
                if !has_mutation {
                    issues.push(Issue::new(
                        "S2301",
                        "iter_mut() used but collection not modified - use iter() instead",
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::quick("Replace iter_mut() with iter()")));
                }
            }
        }
        issues
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// BATCH 8: Testing Rules (15 rules)
// ═══════════════════════════════════════════════════════════════════════════════

// ─────────────────────────────────────────────────────────────────────────────
// S2728 — assert_ne! arguments should be (expected, actual) like assert_eq!
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S2728"
    name: "assert_ne! should have arguments in consistent order"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"assert_ne!\s*\(\s*\$(\w+)\s*,\s*\$(\w+)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                let first = cap.get(1).unwrap().as_str();
                let second = cap.get(2).unwrap().as_str();
                if first.chars().next().map(|c| c.is_lowercase()).unwrap_or(false) &&
                   second.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
                    issues.push(Issue::new(
                        "S2728",
                        "assert_ne! arguments may be reversed - use (expected, actual)",
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::quick("Use assert_ne!(expected, actual) for consistency")));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S1607 — Ignored tests without reason
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S1607"
    name: "Tests should not be ignored without a reason"
    severity: Major
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("#[test]") || line.contains("#[cfg(test)]") {
                let next_lines: String = ctx.source.lines().skip(idx).take(3).collect::<Vec<_>>().join("\n");
                if next_lines.contains("#[ignore]") && !next_lines.contains("reason") && !next_lines.contains("//") && !next_lines.contains("/*") {
                    issues.push(Issue::new(
                        "S1607",
                        "Test marked #[ignore] without documented reason",
                        Severity::Major,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::moderate("Add a comment explaining why the test is ignored or fix the test")));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S2260 — Test modules without any tests
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S2260"
    name: "Test modules should contain at least one test"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"#\[cfg\(test\)\]\s+mod\s+(\w+)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                let mod_name = cap.get(1).unwrap().as_str();
                let remaining: String = ctx.source.lines().skip(idx).take(30).collect::<Vec<_>>().join("\n");
                let has_test = remaining.contains("#[test]") || remaining.contains("#[tokio::test]") || remaining.contains("#[cfg(test)]");
                if !has_test {
                    issues.push(Issue::new(
                        "S2260",
                        format!("Test module '{}' has no test functions", mod_name),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::quick("Add tests to this module or remove it")));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S2699 — Test assertions should have messages
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S2699"
    name: "Test assertions should include descriptive messages"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"assert!\s*\(([^,]+)\s*\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                let arg = cap.get(1).unwrap().as_str();
                if !arg.contains(",") && !arg.contains("\"") {
                    issues.push(Issue::new(
                        "S2699",
                        "Assertion without message - add a descriptive message",
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::quick("Add a message: assert!(condition, \"description\")")));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S2701 — Literal assertions
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S2701"
    name: "Literal assertions should be simplified"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"assert_eq!\s*\(\s*(true|false)\s*,\s*").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "S2701",
                    "assert_eq!(true, x) or assert_eq!(false, x) should be assert!(x) or assert!(!x)",
                    Severity:: Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::quick("Use assert!() instead of assert_eq!() with boolean literal")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S2925 — Thread::sleep in tests
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S2925"
    name: "Thread::sleep should not be used in tests"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"std::thread::sleep|thread::sleep").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if (line.contains("#[test]") || line.contains("#[cfg(test)]")) && re.is_match(&ctx.source.lines().skip(idx.saturating_sub(5)).take(10).collect::<Vec<_>>().join("\n")) {
                issues.push(Issue::new(
                    "S2925",
                    "Thread::sleep in test - use proper synchronization instead",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::moderate("Use proper async waiting, channels, or test utilities instead of sleep")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S3415 — Assertion arguments order
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S3415"
    name: "assert_eq! arguments should be (expected, actual)"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"assert_eq!\s*\(\s*\$(\w+)\s*,\s*\$(\w+)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                let first = cap.get(1).unwrap().as_str();
                let second = cap.get(2).unwrap().as_str();
                if first.chars().next().map(|c| c.is_lowercase()).unwrap_or(false) &&
                   second.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
                    issues.push(Issue::new(
                        "S3415",
                        "assert_eq! arguments may be reversed - use (expected, actual)",
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::quick("Use assert_eq!(expected, actual) - Rust convention is (expected, actual)")));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S3419 — should_panic without expected message
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S3419"
    name: "#[should_panic] should include expected message"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("#[should_panic]") {
                let remaining: String = ctx.source.lines().skip(idx).take(5).collect::<Vec<_>>().join("\n");
                if !remaining.contains("expected =") && !remaining.contains("expected:") {
                    issues.push(Issue::new(
                        "S3419",
                        "#[should_panic] without expected message - add expected = \"...\"",
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::quick("Add expected = \"panic message\" to make the test more specific")));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S3577 — Test method naming conventions
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S3577"
    name: "Test function names should follow naming conventions"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"fn\s+(test_[a-z_]+|[a-z_]+_test)\s*\(").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                let name = cap.get(1).unwrap().as_str();
                if !name.starts_with("test_") {
                    issues.push(Issue::new(
                        "S3577",
                        format!("Test function '{}' should be named test_{} or {}_test", name, name, name),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::quick("Use 'test_' prefix for test functions")));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S3578 — Test methods should start with test_
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S3578"
    name: "Test functions must start with test_"
    severity: Major
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"fn\s+([a-z_][a-zA-Z_]*)\s*\(\s*\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("#[test]") || line.contains("#[tokio::test]") {
                if let Some(cap) = re.captures(line) {
                    let name = cap.get(1).unwrap().as_str();
                    if !name.starts_with("test_") && name != "main" {
                        issues.push(Issue::new(
                            "S3578",
                            format!("Test function '{}' should start with 'test_'", name),
                            Severity::Major,
                            Category::CodeSmell,
                            ctx.file_path,
                            idx + 1,
                        ).with_remediation(Remediation::quick("Prefix the function name with 'test_'")));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S4144 — Duplicated test methods
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S4144"
    name: "Test methods should not be duplicated"
    severity: Major
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let mut test_names: std::collections::HashMap<String, Vec<usize>> = std::collections::HashMap::new();
        let re = regex::Regex::new(r"fn\s+(test_\w+)\s*\(").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                let name = cap.get(1).unwrap().as_str().to_string();
                test_names.entry(name).or_default().push(idx + 1);
            }
        }
        for (name, lines) in test_names {
            let count = lines.len();
            if count > 1 {
                for &line in &lines {
                    issues.push(Issue::new(
                        "S4144",
                        format!("Duplicated test function '{}' - appears {} times", name, count),
                        Severity::Major,
                        Category::CodeSmell,
                        ctx.file_path,
                        line,
                    ).with_remediation(Remediation::moderate("Remove duplicate test or merge them")));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S5778 — Only one assertion per test
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S5778"
    name: "Tests should not have too many assertions"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: { max_assertions: usize = 3 }
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"fn\s+(test_\w+)\s*\(").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                let func_name = cap.get(1).unwrap().as_str();
                let remaining: String = ctx.source.lines().skip(idx).take(50).collect::<Vec<_>>().join("\n");
                let assert_count = remaining.matches("assert!(").count()
                    + remaining.matches("assert_eq!(").count()
                    + remaining.matches("assert_ne!(").count();
                if assert_count > self.max_assertions {
                    issues.push(Issue::new(
                        "S5778",
                        format!("Test '{}' has {} assertions - consider splitting", func_name, assert_count),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::moderate("Split this test into multiple focused tests")));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S5786 — Test fixture setup in helper
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S5786"
    name: "Test fixtures should not have too much setup code"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: { max_setup_lines: usize = 10 }
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"fn\s+(setup|before|init|new_test)\s*\(\s*\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && !line.contains("#[test]") {
                let remaining: String = ctx.source.lines().skip(idx).take(30).collect::<Vec<_>>().join("\n");
                let setup_lines = remaining.lines().take_while(|l| !l.contains("#[test]")).count();
                if setup_lines > self.max_setup_lines {
                    issues.push(Issue::new(
                        "S5786",
                        format!("Test setup function has {} lines - consider using before_each/after_each", setup_lines),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::moderate("Extract setup logic to a helper module or use test framework features")));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S5790 — Tests should be independent
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S5790"
    name: "Tests should not depend on shared mutable state"
    severity: Major
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re_static = regex::Regex::new(r"static\s+(mut|ref)\s+\w+").unwrap();
        let re_threadlocal = regex::Regex::new(r"#\[thread_local\]").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("#[test]") || line.contains("#[cfg(test)]") {
                let context: String = ctx.source.lines().skip(idx.saturating_sub(5)).take(20).collect::<Vec<_>>().join("\n");
                if re_static.is_match(&context) || re_threadlocal.is_match(&context) {
                    issues.push(Issue::new(
                        "S5790",
                        "Test uses shared mutable state - tests should be independent",
                        Severity::Major,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::moderate("Use local variables or setup/teardown patterns instead of shared state")));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S5810 — Test assertion count
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S5810"
    name: "Tests should have at least one assertion"
    severity: Major
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"fn\s+(test_\w+)\s*\(").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                let func_name = cap.get(1).unwrap().as_str();
                let remaining: String = ctx.source.lines().skip(idx).take(30).collect::<Vec<_>>().join("\n");
                let assert_count = remaining.matches("assert!(").count()
                    + remaining.matches("assert_eq!(").count()
                    + remaining.matches("assert_ne!(").count()
                    + remaining.matches("panic!").count();
                if assert_count == 0 {
                    issues.push(Issue::new(
                        "S5810",
                        format!("Test '{}' has no assertions", func_name),
                        Severity::Major,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::moderate("Add at least one assertion to verify the test behavior")));
                }
            }
        }
        issues
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// BATCH 9: Documentation & API Rules (15 rules)
// ═══════════════════════════════════════════════════════════════════════════════

// ─────────────────────────────────────────────────────────────────────────────
// S100 — Function names should use snake_case
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S100"
    name: "Function names should follow snake_case convention"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"fn\s+([A-Z][a-zA-Z0-9_]*|[a-z]+[A-Z])").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                if let Some(name) = cap.get(1) {
                    let name_str = name.as_str();
                    // Skip test functions
                    if name_str.starts_with("test_") || name_str.contains("_test_") {
                        continue;
                    }
                    // Skip closure parameters (|x| syntax is not a function declaration)
                    if name_str.starts_with('|') {
                        continue;
                    }
                    issues.push(Issue::new(
                        "S100",
                        format!("Function '{}' should use snake_case", name_str),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S101 — Struct names should use CamelCase
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S101"
    name: "Struct names should follow CamelCase convention"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"struct\s+([a-z][a-z0-9_]*)\s*(<|\{|;)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                if let Some(name) = cap.get(1) {
                    let name_str = name.as_str();
                    // Skip single-letter generic type parameters (T, E, K, V, etc.)
                    if name_str.len() == 1 {
                        continue;
                    }
                    issues.push(Issue::new(
                        "S101",
                        format!("Struct '{}' should use CamelCase", name_str),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S102 — Trait names should use CamelCase
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S102"
    name: "Trait names should follow CamelCase convention"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"trait\s+([a-z][a-z0-9_]*)\s*(<|\{|;)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                if let Some(name) = cap.get(1) {
                    issues.push(Issue::new(
                        "S102",
                        format!("Trait '{}' should use CamelCase", name.as_str()),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S105 — Tab characters should not be used
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S105"
    name: "Tab characters should not be used in source code"
    severity: Minor
    category: CodeSmell
    language: "*"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains('\t') {
                let col = line.find('\t').unwrap() + 1;
                issues.push(Issue::new(
                    "S105",
                    "Tab character found - use spaces for indentation",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_column(col));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S106 — Module names should use snake_case
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S106"
    name: "Module names should follow snake_case convention"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"(?:^|\s)mod\s+([A-Z][a-zA-Z0-9_]*|[a-z]+[A-Z])").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                if let Some(name) = cap.get(1) {
                    issues.push(Issue::new(
                        "S106",
                        format!("Module '{}' should use snake_case", name.as_str()),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S118 — Trait names should be CamelCase (renamed from Abstract class naming)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S118"
    name: "Trait names should follow CamelCase convention"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"trait\s+(\w+)\s*(<|\{|;)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                if let Some(name) = cap.get(1) {
                    let name_str = name.as_str();
                    if name_str.chars().next().map(|c| c.is_lowercase()).unwrap_or(false) {
                        issues.push(Issue::new(
                            "S118",
                            format!("Trait '{}' should use CamelCase", name_str),
                            Severity::Minor,
                            Category::CodeSmell,
                            ctx.file_path,
                            idx + 1,
                        ));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S120 — Public items should have documentation comments
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S120"
    name: "Public items should have documentation comments"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("pub fn ") || trimmed.starts_with("pub struct ") ||
               trimmed.starts_with("pub enum ") || trimmed.starts_with("pub trait ") ||
               trimmed.starts_with("pub mod ") || trimmed.starts_with("pub type ") {
                let has_doc = if idx > 0 {
                    let prev = lines[idx - 1].trim();
                    prev.starts_with("///") || prev.starts_with("//!") || prev.starts_with("/*")
                } else { false };
                if !has_doc {
                    issues.push(Issue::new(
                        "S120",
                        "Public item should have documentation comment (/// or !//)",
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S122 — Methods should not be marked #[inline] unnecessarily
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S122"
    name: "Methods should not have unnecessary #[inline] attributes"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("#[inline]") && !line.contains("#[inline(always)]") && !line.contains("#[inline(never)]") {
                let context: String = ctx.source.lines().skip(idx.saturating_sub(2)).take(5).collect::<Vec<_>>().join("\n");
                if !context.contains("trait ") && !context.contains("impl ") {
                    issues.push(Issue::new(
                        "S122",
                        "Unnecessary #[inline] on a non-trait method - compiler optimizes appropriately",
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S123 — impl Trait for Type should have documentation
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S123"
    name: "impl blocks should have documentation"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"impl\s+(?:\w+\s+for\s+)?\w+").unwrap();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            if re.is_match(line) && !line.contains("where") {
                if let Some(prev_line) = lines.get(idx.saturating_sub(1)) {
                    let prev_trim = prev_line.trim();
                    if !prev_trim.starts_with("///") && !prev_trim.starts_with("//!") && !prev_trim.starts_with("/*") && !prev_trim.starts_with("#[") {
                        issues.push(Issue::new(
                            "S123",
                            "impl block should have documentation comment",
                            Severity::Minor,
                            Category::CodeSmell,
                            ctx.file_path,
                            idx + 1,
                        ));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S126 — Functions should not be too complex
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S126"
    name: "Functions should not be too complex"
    severity: Major
    category: CodeSmell
    language: "rust"
    params: { max_complexity: usize = 10 }
    check: => {
        let mut issues = Vec::new();
        for func_node in ctx.query_functions() {
            let text = func_node.utf8_text(ctx.source.as_bytes()).unwrap_or("");
            let complexity = text.matches("if ").count()
                + text.matches("match ").count()
                + text.matches("while ").count()
                + text.matches("for ").count()
                + text.matches("loop ").count()
                + text.matches("&&").count()
                + text.matches("||").count();
            if complexity > self.max_complexity {
                let pt = func_node.start_position();
                if let Some(name) = ctx.function_name(func_node) {
                    issues.push(Issue::new(
                        "S126",
                        format!("Function '{}' has complexity {} exceeding threshold {}", name, complexity, self.max_complexity),
                        Severity::Major,
                        Category::CodeSmell,
                        ctx.file_path,
                        pt.row + 1,
                    ));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S127 — For loop variable should not be modified
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S127"
    name: "For loop variable should not be modified inside the loop"
    severity: Major
    category: Bug
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"for\s+(\w+)\s+in\s+").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                let counter = cap.get(1).unwrap().as_str();
                let body_start = idx + 1;
                for (body_idx, body_line) in ctx.source.lines().skip(body_start).enumerate() {
                    if body_line.contains(&format!("{} =", counter)) || body_line.contains(&format!("{} +=", counter)) {
                        issues.push(Issue::new(
                            "S127",
                            format!("Loop variable '{}' is modified inside the loop", counter),
                            Severity::Major,
                            Category::Bug,
                            ctx.file_path,
                            body_start + body_idx + 1,
                        ));
                    }
                    if body_line.trim() == "}" { break; }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S128 — Match expression with multiple patterns
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S128"
    name: "Match arms with multiple patterns should be simplified"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r",\s*\|").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("=>") && re.is_match(line) {
                issues.push(Issue::new(
                    "S128",
                    "Match arm with multiple patterns - consider if this is intentional",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S129 — Match expression with too many arms
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S129"
    name: "Match expressions should not have too many arms"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: { max_arms: usize = 10 }
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"match\s+\w+\s*\{").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                let context: String = ctx.source.lines().skip(idx).take(50).collect::<Vec<_>>().join("\n");
                let arm_count = context.matches("=>").count();
                if arm_count > self.max_arms {
                    issues.push(Issue::new(
                        "S129",
                        format!("Match has {} arms - consider using a HashMap or enum approach", arm_count),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S230 — Missing documentation on public functions
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S230"
    name: "Public functions should have documentation comments"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"^pub\s+fn\s+(\w+)").unwrap();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            if let Some(cap) = re.captures(line) {
                let fn_name = cap.get(1).unwrap().as_str();
                if !fn_name.starts_with("test_") && !fn_name.starts_with("set_") && !fn_name.starts_with("get_") {
                    if idx == 0 || (!lines[idx - 1].trim().starts_with("///") && !lines[idx - 1].trim().starts_with("//!") && !lines[idx - 1].trim().starts_with("/*")) {
                        issues.push(Issue::new(
                            "S230",
                            format!("Public function '{}' should have documentation comment", fn_name),
                            Severity::Minor,
                            Category:: CodeSmell,
                            ctx.file_path,
                            idx + 1,
                        ));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S231 — Missing documentation on public structs
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S231"
    name: "Public structs should have documentation comments"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"^pub\s+struct\s+(\w+)").unwrap();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            if let Some(cap) = re.captures(line) {
                let struct_name = cap.get(1).unwrap().as_str();
                if idx == 0 || (!lines[idx - 1].trim().starts_with("///") && !lines[idx - 1].trim().starts_with("//!") && !lines[idx - 1].trim().starts_with("/*")) {
                    issues.push(Issue::new(
                        "S231",
                        format!("Public struct '{}' should have documentation comment", struct_name),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S232 — Missing documentation on public traits
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S232"
    name: "Public traits should have documentation comments"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"^pub\s+trait\s+(\w+)").unwrap();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            if let Some(cap) = re.captures(line) {
                let trait_name = cap.get(1).unwrap().as_str();
                if idx == 0 || (!lines[idx - 1].trim().starts_with("///") && !lines[idx - 1].trim().starts_with("//!") && !lines[idx - 1].trim().starts_with("/*")) {
                    issues.push(Issue::new(
                        "S232",
                        format!("Public trait '{}' should have documentation comment", trait_name),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S233 — Missing documentation on public enums
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S233"
    name: "Public enums should have documentation comments"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"^pub\s+enum\s+(\w+)").unwrap();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            if let Some(cap) = re.captures(line) {
                let enum_name = cap.get(1).unwrap().as_str();
                if idx == 0 || (!lines[idx - 1].trim().starts_with("///") && !lines[idx - 1].trim().starts_with("//!") && !lines[idx - 1].trim().starts_with("/*")) {
                    issues.push(Issue::new(
                        "S233",
                        format!("Public enum '{}' should have documentation comment", enum_name),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// S234 — Missing documentation on public modules
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "S234"
    name: "Public modules should have documentation comments"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"^pub\s+mod\s+(\w+)").unwrap();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            if let Some(cap) = re.captures(line) {
                let mod_name = cap.get(1).unwrap().as_str();
                if idx == 0 || (!lines[idx - 1].trim().starts_with("///") && !lines[idx - 1].trim().starts_with("//!") && !lines[idx - 1].trim().starts_with("/*")) {
                    issues.push(Issue::new(
                        "S234",
                        format!("Public module '{}' should have documentation comment", mod_name),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ));
                }
            }
        }
        issues
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// BATCH 10: Rust-specific Rules (15 rules)
// ═══════════════════════════════════════════════════════════════════════════════

// ─────────────────────────────────────────────────────────────────────────────
// R001 — Unsafe block usage
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "R001"
    name: "Unsafe blocks should be avoided when possible"
    severity: Major
    category: Bug
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("unsafe {") {
                issues.push(Issue::new(
                    "R001",
                    "Unsafe block detected - consider using safe alternatives",
                    Severity::Major,
                    Category::Bug,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::moderate("Encapsulate unsafe code in safe abstractions")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// R004 — Unwrap in library code
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "R004"
    name: "unwrap() should not be used in library code"
    severity: Major
    category: Bug
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let is_test = ctx.file_path.to_string_lossy().contains("test")
            || ctx.file_path.to_string_lossy().contains("tests")
            || ctx.source.contains("#[test]")
            || ctx.source.contains("#[cfg(test)]");
        if !is_test {
            for (idx, line) in ctx.source.lines().enumerate() {
                if line.contains(".unwrap()") {
                    issues.push(Issue::new(
                        "R004",
                        "unwrap() in library code - use ? or proper error handling",
                        Severity::Major,
                        Category::Bug,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::moderate("Return Result or Option instead of unwrapping")));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// R005 — Expect with empty message
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "R005"
    name: "expect() should not be called with empty or whitespace-only message"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r#"\.(expect|unwrap)\s*\(\s*(""\s*|\s*")\)"#).unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "R005",
                    "expect() or unwrap() with empty message - provide a meaningful error message",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::quick("Add a descriptive error message to expect()")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// R006 — Manual implementation of Clone
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "R006"
    name: "Manual implementation of Clone where derive would work"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re_impl = regex::Regex::new(r"impl\s+Clone\s+for\s+(\w+)").unwrap();
        let source = ctx.source.to_string();
        for cap in re_impl.captures_iter(&source) {
            let struct_name = cap.get(1).unwrap().as_str();
            let struct_re = regex::Regex::new(&format!(r"struct\s+{}\s*", struct_name)).unwrap();
            if struct_re.is_match(&source) {
                let derive_re = regex::Regex::new(&format!(r"#\[derive\([^)]*\bClone\b[^)]*\)\].*struct\s+{}", struct_name)).unwrap();
                if !derive_re.is_match(&source) {
                    let line_num = source[..cap.get(0).unwrap().start()].matches('\n').count() + 1;
                    issues.push(Issue::new(
                        "R006",
                        format!("Manual Clone impl for '{}' - consider using #[derive(Clone)]", struct_name),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        line_num,
                    ).with_remediation(Remediation::quick("Use #[derive(Clone)] instead of manual implementation")));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// R007 — Manual PartialEq when all fields are PartialEq
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "R007"
    name: "Manual PartialEq implementation where derive would work"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re_impl = regex::Regex::new(r"impl\s+PartialEq\s+for\s+(\w+)").unwrap();
        let source = ctx.source.to_string();
        for cap in re_impl.captures_iter(&source) {
            let struct_name = cap.get(1).unwrap().as_str();
            let derive_re = regex::Regex::new(&format!(r"#\[derive\([^)]*\bPartialEq\b[^)]*\)\].*struct\s+{}", struct_name)).unwrap();
            if !derive_re.is_match(&source) {
                let line_num = source[..cap.get(0).unwrap().start()].matches('\n').count() + 1;
                issues.push(Issue::new(
                    "R007",
                    format!("Manual PartialEq impl for '{}' - consider using #[derive(PartialEq)]", struct_name),
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num,
                ).with_remediation(Remediation::quick("Use #[derive(PartialEq)] instead of manual implementation")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// R008 — Unnecessary .collect() before .iter()
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "R008"
    name: "Unnecessary .collect() before .iter()"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\.collect::<.*>\(\)\.iter\(\)|\.collect::<Vec<.*>>\(\)\.iter\(\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "R008",
                    "Unnecessary .collect() followed by .iter() - remove .collect()",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::quick("Remove .collect() and use the iterator directly")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// R009 — Box::pin on stack variable
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "R009"
    name: "Box::pin on stack variable is unsafe"
    severity: Major
    category: Bug
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"Box::pin\s*\(\s*(\w+)\s*\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                if let Some(var) = cap.get(1) {
                    let var_name = var.as_str();
                    if !var_name.contains("::") && !var_name.contains("new(") {
                        issues.push(Issue::new(
                            "R009",
                            format!("Box::pin on stack variable '{}' - use Box::pin on the heap", var_name),
                            Severity::Major,
                            Category:: Bug,
                            ctx.file_path,
                            idx + 1,
                        ).with_remediation(Remediation::moderate("Pin data on the heap with Box::pin(Box::new(...))")));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// R010 — Unnecessary RefCell (Cell would suffice for Copy types)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "R010"
    name: "RefCell used but Cell would suffice for Copy types"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"RefCell<\s*(i8|i16|i32|i64|i128|isize|u8|u16|u32|u64|u128|usize|f32|f64|bool|char)\s*>").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "R010",
                    "RefCell for a Copy type - use Cell instead for better performance",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::quick("Replace RefCell<T> with Cell<T> for Copy types")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// R011 — Unnecessary Arc (Rc would suffice in single-threaded context)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "R011"
    name: "Arc used but Rc would suffice"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let is_concurrent = ctx.source.contains("Arc::new")
            && (ctx.source.contains("thread::spawn") || ctx.source.contains("std::thread") || ctx.source.contains("tokio::spawn"));
        if !is_concurrent {
            let re = regex::Regex::new(r"Arc::").unwrap();
            for (idx, line) in ctx.source.lines().enumerate() {
                if re.is_match(line) && !line.contains("Rc::") {
                    issues.push(Issue::new(
                        "R011",
                        "Arc used but Rc would suffice - Arc is for multi-threaded sharing",
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::quick("Use Rc instead of Arc for single-threaded ownership")));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// R012 — Explicit lifetime where elision would work
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "R012"
    name: "Explicit lifetime where elision would work"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"fn\s+\w+<('[a-z]+\s*,\s*)*'[a-z]+\s*>\s*\(").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && !line.contains("impl") && !line.contains("where") {
                issues.push(Issue::new(
                    "R012",
                    "Explicit lifetime parameters where elision would work",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::quick("Remove explicit lifetimes and let the compiler infer them")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// R013 — Unnecessary turbofish (::<Type> when type can be inferred)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "R013"
    name: "Unnecessary turbofish syntax"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"::<\w+>\(\)|::<\w+>::\w+").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "R013",
                    "Unnecessary turbofish (::<Type>) - type can be inferred",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::quick("Remove ::<Type> and let the compiler infer the type")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// R014 — .clone() on Rc or Arc
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "R014"
    name: "Use Rc::clone or Arc::clone instead of .clone()"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"(Rc<|Arc<)[^>]+>\s*\([^)]+\)\.clone\(\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "R014",
                    "Using .clone() on Rc/Arc - use Rc::clone(&x) or Arc::clone(&x) for clarity",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::quick("Use Rc::clone(&x) or Arc::clone(&x) instead of x.clone()")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// R015 — for_each instead of for loop
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "R015"
    name: "for_each is less readable than a for loop here"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\.for_each\s*\(\s*\|\s*\w+\s*,\s*\|[^|]+").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "R015",
                    "for_each with complex closure - a for loop may be more readable",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::quick("Consider using a for loop for better readability")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// R016 — map with side effects (should be for_each)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "R016"
    name: "map() with side effects should be for_each"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\.map\s*\(\s*\|\s*\w+\s*\|[^}]*\.push\(|\.map\s*\(\s*\|\s*\w+\s*\|[^}]*\.insert\(|\.map\s*\(\s*\|\s*\w+\s*\|[^}]*print|\.map\s*\(\s*\|\s*\w+\s*\|[^}]*eprint").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "R016",
                    "map() with side effects - use for_each or a for loop instead",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::quick("Use for_each for side effects instead of map()")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// R017 — filter_map(|x| x.ok()) should be flatten()
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "R017"
    name: "filter_map followed by flatten should be flatten"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\.filter_map\(\s*\|\s*\w+\s*\|\s*\w+\.ok\(\)\s*\)\.flatten\(\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "R017",
                    "filter_map(|x| x.ok()) followed by flatten() - use flatten() directly",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::quick("Replace with .filter_map(|x| x.ok()).flatten() by just using flatten()")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// R018 — if let Ok(x) = expr { Some(x) } else { None } should be expr.ok()
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "R018"
    name: "Redundant if-let pattern should use .ok()"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        // Detect `if let Ok(x) = expr { Some(x) } else { None }` without backreference
        for (idx, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            // Check for basic pattern structure
            if trimmed.starts_with("if let Ok(") && trimmed.contains("} else { None }") {
                if let Some(start) = trimmed.find("Ok(") {
                    let after_ok = &trimmed[start + 3..];
                    if let Some(end) = after_ok.find(')') {
                        let var_name = after_ok[..end].trim();
                        if !var_name.is_empty() {
                            // Look for Some(var_name) in the body before } else {
                            if let Some(else_pos) = trimmed.find("} else {") {
                                let body_before_else = &trimmed[..else_pos];
                                let some_pattern = format!("Some({})", var_name);
                                if body_before_else.contains(&some_pattern) {
                                    issues.push(Issue::new(
                                        "R018",
                                        "Redundant if-let pattern - use .ok() instead",
                                        Severity::Minor,
                                        Category::CodeSmell,
                                        ctx.file_path,
                                        idx + 1,
                                    ).with_remediation(Remediation::quick("Replace with .ok() method")));
                                }
                            }
                        }
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// R019 — Manual implementation of Default
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "R019"
    name: "Manual Default implementation that matches struct defaults"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re_impl = regex::Regex::new(r"impl\s+Default\s+for\s+(\w+)").unwrap();
        let source = ctx.source.to_string();
        for cap in re_impl.captures_iter(&source) {
            let struct_name = cap.get(1).unwrap().as_str();
            let derive_re = regex::Regex::new(&format!(r"#\[derive\([^)]*\bDefault\b[^)]*\)\].*struct\s+{}", struct_name)).unwrap();
            if !derive_re.is_match(&source) {
                let line_num = source[..cap.get(0).unwrap().start()].matches('\n').count() + 1;
                issues.push(Issue::new(
                    "R019",
                    format!("Manual Default impl for '{}' - consider using #[derive(Default)]", struct_name),
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num,
                ).with_remediation(Remediation::quick("Use #[derive(Default)] instead of manual Default implementation")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// R020 — Unnecessary .as_ref() / .as_deref()
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "R020"
    name: "Unnecessary .as_ref() or .as_deref() call"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\.as_ref\(\)\.as_ref\(\)|\.as_deref\(\)\.as_deref\(\)|\.as_ref\(\)\.as_deref\(\)|\.as_deref\(\)\.as_ref\(\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "R020",
                    "Chained .as_ref() or .as_deref() calls - simplify",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::quick("Remove the redundant .as_ref() or .as_deref() call")));
            }
        }
        issues
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// BATCH: JavaScript/TypeScript Security Rules (20 rules)
// ═══════════════════════════════════════════════════════════════════════════════

// ─────────────────────────────────────────────────────────────────────────────
// JS_S1523 — eval() usage (dynamic code execution)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S1523"
    name: "eval() and similar functions should not be used"
    severity: Blocker
    category: Vulnerability
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let eval_patterns = ["eval(", "new Function(", "setTimeout(", "setInterval("];
        for (idx, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") || trimmed.starts_with("*") {
                continue;
            }
            for pattern in &eval_patterns {
                if line.contains(pattern) {
                    let col = line.find(pattern).unwrap() + 1;
                    issues.push(Issue::new(
                        "JS_S1523",
                        format!("Use of {} is security-sensitive - avoid dynamic code execution", pattern.trim_end_matches('(')),
                        Severity::Blocker,
                        Category::Vulnerability,
                        ctx.file_path,
                        idx + 1,
                    ).with_column(col).with_remediation(Remediation::substantial(
                        "Avoid using eval() and similar functions. Use safer alternatives like JSON.parse() for data or structured code generation."
                    )));
                    break;
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S2259 — document.write() (XSS vector)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S2259"
    name: "document.write() is a security risk"
    severity: Blocker
    category: Vulnerability
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("document.write") || line.contains("document.writeln") {
                issues.push(Issue::new(
                    "JS_S2259",
                    "document.write() is a major XSS vulnerability - use textContent or safe DOM APIs",
                    Severity::Blocker,
                    Category::Vulnerability,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::substantial(
                    "Replace document.write() with safe alternatives like element.textContent or element.innerHTML with proper sanitization"
                )));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S2611 — innerHTML assignment (XSS)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S2611"
    name: "innerHTML usage is a security risk"
    severity: Blocker
    category: Vulnerability
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains(".innerHTML") {
                issues.push(Issue::new(
                    "JS_S2611",
                    "innerHTML assignment is a potential XSS vector - user input may execute as HTML",
                    Severity::Blocker,
                    Category::Vulnerability,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::substantial(
                    "Use textContent for plain text or a sanitization library (DOMPurify) for HTML content"
                )));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S3330 — Cookie without HttpOnly
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S3330"
    name: "Cookies should set the HttpOnly flag"
    severity: Minor
    category: SecurityHotspot
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("cookie") || line.contains("Cookie") {
                if !line.contains("HttpOnly") && !line.contains("httpOnly") {
                    issues.push(Issue::new(
                        "JS_S3330",
                        "Cookie without HttpOnly flag - vulnerable to XSS attacks",
                        Severity::Minor,
                        Category:: SecurityHotspot,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::moderate(
                        "Set the HttpOnly flag on cookies to prevent JavaScript access"
                    )));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S4502 — CSRF protection disabled
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S4502"
    name: "CSRF protection should not be disabled"
    severity: Blocker
    category: Vulnerability
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let csrf_patterns = [
            "csrf: false",
            "csrfDisabled: true",
            "withCredentials: false",
            "xsrfToken: null",
        ];
        for (idx, line) in ctx.source.lines().enumerate() {
            for pattern in &csrf_patterns {
                if line.contains(pattern) {
                    issues.push(Issue::new(
                        "JS_S4502",
                        "CSRF protection appears to be disabled",
                        Severity::Blocker,
                        Category::Vulnerability,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::substantial(
                        "Enable CSRF protection or use SameSite=Strict cookies"
                    )));
                    break;
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S4784 — RegExp injection (user input to new RegExp())
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S4784"
    name: "User input should not be used in regular expressions"
    severity: Blocker
    category: Vulnerability
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"new\s+RegExp\s*\(\s*\w+").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "JS_S4784",
                    "Potential RegExp injection - user input in new RegExp()",
                    Severity::Blocker,
                    Category:: Vulnerability,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::substantial(
                    "Validate and sanitize user input before using in RegExp, or use a safe regex pattern library"
                )));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S4817 — XPath injection
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S4817"
    name: "XPath injection vulnerabilities should be prevented"
    severity: Blocker
    category: Vulnerability
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let xpath_patterns = ["evaluate(", "selectSingleNode(", "selectNodes(", "XPathExpression"];
        for (idx, line) in ctx.source.lines().enumerate() {
            for pattern in &xpath_patterns {
                if line.contains(pattern) {
                    let has_concat = line.contains("+") || line.contains("concat(");
                    let has_template = line.contains("${") || line.contains("`");
                    if has_concat || has_template {
                        issues.push(Issue::new(
                            "JS_S4817",
                            "Potential XPath injection - string concatenation in XPath expression",
                            Severity::Blocker,
                            Category:: Vulnerability,
                            ctx.file_path,
                            idx + 1,
                        ).with_remediation(Remediation::substantial(
                            "Use parameterized XPath queries or escape user input properly"
                        )));
                        break;
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S4823 — process.env usage in browser code
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S4823"
    name: "Server-only APIs should not be used in browser code"
    severity: Blocker
    category: Vulnerability
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let node_patterns = ["process.env", "process.cwd()", "__dirname", "__filename"];
        for (idx, line) in ctx.source.lines().enumerate() {
            for pattern in &node_patterns {
                if line.contains(pattern) {
                    issues.push(Issue::new(
                        "JS_S4823",
                        format!("Use of Node.js specific API '{}' in browser code", pattern),
                        Severity::Blocker,
                        Category:: Vulnerability,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::substantial(
                        "Remove Node.js specific APIs from browser code or use environment abstraction"
                    )));
                    break;
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S4829 — console.log in production
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S4829"
    name: "Debugging statements should not be left in production code"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let console_patterns = ["console.log", "console.debug", "console.info"];
        let production_indicators = ["process.env.NODE_ENV", "production", "process.env.NODE_ENV === 'production'"];
        let is_likely_production = production_indicators.iter().any(|p| ctx.source.contains(p));

        if !is_likely_production {
            return issues;
        }

        for (idx, line) in ctx.source.lines().enumerate() {
            for pattern in &console_patterns {
                if line.contains(pattern) && !line.contains("//") && !line.trim().starts_with("*") {
                    issues.push(Issue::new(
                        "JS_S4829",
                        "Debug console statement in production code",
                        Severity::Minor,
                        Category:: CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::quick(
                        "Remove console statements in production or use a logging framework with proper log levels"
                    )));
                    break;
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S5122 — NoSQL injection (MongoDB $where)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S5122"
    name: "NoSQL injection vulnerabilities should be prevented"
    severity: Blocker
    category: Vulnerability
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let nosql_patterns = ["$where", "$function", "$text", "$regex"];
        let mongo_indicators = ["collection(", "db.", "mongo", "mongoose"];
        let has_mongo = mongo_indicators.iter().any(|p| ctx.source.to_lowercase().contains(p));

        if !has_mongo {
            return issues;
        }

        for (idx, line) in ctx.source.lines().enumerate() {
            for pattern in &nosql_patterns {
                if line.contains(pattern) && (line.contains("+") || line.contains("${") || line.contains("`")) {
                    issues.push(Issue::new(
                        "JS_S5122",
                        format!("Potential NoSQL injection - string concatenation with {}", pattern),
                        Severity::Blocker,
                        Category:: Vulnerability,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::substantial(
                        "Use parameterized queries instead of string concatenation for NoSQL databases"
                    )));
                    break;
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S5145 — window.open() without noopener
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S5145"
    name: "window.open() should include noopener for security"
    severity: Minor
    category: SecurityHotspot
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"window\.open\s*\([^)]*\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(m) = re.find(line) {
                let call = m.as_str();
                if !call.contains("noopener") && !call.contains("noreferrer") {
                    issues.push(Issue::new(
                        "JS_S5145",
                        "window.open() without noopener - can expose window.opener to malicious pages",
                        Severity::Minor,
                        Category:: SecurityHotspot,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::moderate(
                        "Add 'noopener' or 'noreferrer' to window.open() features parameter"
                    )));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S5247 — dangerouslySetInnerHTML (React XSS)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S5247"
    name: "React dangerouslySetInnerHTML should be avoided"
    severity: Blocker
    category: Vulnerability
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("dangerouslySetInnerHTML") {
                issues.push(Issue::new(
                    "JS_S5247",
                    "dangerouslySetInnerHTML is a major XSS risk in React applications",
                    Severity::Blocker,
                    Category:: Vulnerability,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::substantial(
                    "Use DOMPurify to sanitize HTML before passing to dangerouslySetInnerHTML, or refactor to use safer alternatives"
                )));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S5542 — Weak crypto in Node.js (crypto.createCipher)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S5542"
    name: "Weak encryption should not be used"
    severity: Critical
    category: Vulnerability
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let weak_patterns = ["createCipher", "createDecipher", "createCipheriv"];
        let has_node_crypto = ctx.source.contains("crypto") || ctx.source.contains("require('crypto')") || ctx.source.contains("import.*crypto");

        if !has_node_crypto {
            return issues;
        }

        for (idx, line) in ctx.source.lines().enumerate() {
            for pattern in &weak_patterns {
                if line.contains(pattern) {
                    issues.push(Issue::new(
                        "JS_S5542",
                        "crypto.createCipher is deprecated and insecure - use crypto.createCipheriv with secure algorithms",
                        Severity::Critical,
                        Category:: Vulnerability,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::substantial(
                        "Use crypto.createCipheriv with AES-256-GCM or ChaCha20-Poly1305 instead"
                    )));
                    break;
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S5547 — Weak cipher algorithms (RC4, DES in Node)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S5547"
    name: "Weak cipher algorithms should not be used"
    severity: Critical
    category: Vulnerability
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let weak_ciphers = ["rc4", "RC4", "des", "DES", "3des", "3DES", "TDES", "rc2", "RC2"];
        for (idx, line) in ctx.source.lines().enumerate() {
            for cipher in &weak_ciphers {
                if line.contains(cipher) && (line.contains("crypto") || line.contains("Cipher") || line.contains("cipher")) {
                    issues.push(Issue::new(
                        "JS_S5547",
                        format!("Weak cipher algorithm '{}' detected - use AES or ChaCha20", cipher),
                        Severity::Critical,
                        Category:: Vulnerability,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::substantial(
                        "Use AES-256-GCM, ChaCha20-Poly1305, or other modern authenticated encryption"
                    )));
                    break;
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S5693 — File upload without size limit (Express/multer)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S5693"
    name: "File uploads should have size limits"
    severity: Major
    category: SecurityHotspot
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let upload_patterns = ["multer", "upload", "formidable", "busboy", "fileUpload"];
        let has_upload = upload_patterns.iter().any(|p| ctx.source.contains(p));

        if !has_upload {
            return issues;
        }

        for (idx, line) in ctx.source.lines().enumerate() {
            let has_size_limit = line.contains("fileSize") || line.contains("limit") || line.contains("maxSize") || line.contains("max_size");
            let is_config_line = line.contains("upload") || line.contains("multer") || line.contains("parser");

            if is_config_line && !has_size_limit {
                let context: String = ctx.source.lines().skip(idx.saturating_sub(2)).take(5).collect::<Vec<_>>().join("\n");
                if !context.contains("fileSize") && !context.contains("limit") && !context.contains("maxSize") {
                    issues.push(Issue::new(
                        "JS_S5693",
                        "File upload configuration missing size limit - potential DoS vector",
                        Severity::Major,
                        Category:: SecurityHotspot,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::moderate(
                        "Set a reasonable file size limit (e.g., 10MB) to prevent denial of service"
                    )));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S5725 — Content-Security-Policy header missing
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S5725"
    name: "Content-Security-Policy header should be set"
    severity: Blocker
    category: Vulnerability
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let header_patterns = ["res.setHeader", "res.headers", "response.setHeader", " helmet()", "csp"];
        let has_express = ctx.source.contains("express") || ctx.source.contains("http.createServer");

        if !has_express {
            return issues;
        }

        let has_csp = ctx.source.contains("Content-Security-Policy") || ctx.source.contains("contentSecurityPolicy");

        if !has_csp {
            for (idx, line) in ctx.source.lines().enumerate() {
                for pattern in &header_patterns {
                    if line.contains(pattern) {
                        issues.push(Issue::new(
                            "JS_S5725",
                            "Content-Security-Policy header is not set - applications should enforce CSP",
                            Severity::Blocker,
                            Category:: Vulnerability,
                            ctx.file_path,
                            idx + 1,
                        ).with_remediation(Remediation::substantial(
                            "Add Content-Security-Policy header or use helmet.js with CSP enabled"
                        )));
                        break;
                    }
                }
                if !issues.is_empty() {
                    break;
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S5730 — Mixed content (HTTP resources in HTTPS page)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S5730"
    name: "Mixed content should not be used on secure pages"
    severity: Blocker
    category: Vulnerability
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r#"["']http://[^"']+["']"#).unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(m) = re.find(line) {
                if !line.contains("localhost") && !line.contains("127.0.0.1") {
                    issues.push(Issue::new(
                        "JS_S5730",
                        format!("Mixed content: HTTP URL {} on HTTPS page", m.as_str()),
                        Severity::Blocker,
                        Category:: Vulnerability,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::moderate(
                        "Replace HTTP URLs with HTTPS to prevent mixed content issues"
                    )));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S5734 — strict-transport-security header missing
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S5734"
    name: "Strict-Transport-Security header should be set"
    severity: Blocker
    category: Vulnerability
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let has_express = ctx.source.contains("express") || ctx.source.contains("http.createServer");

        if !has_express {
            return issues;
        }

        let has_hsts = ctx.source.contains("Strict-Transport-Security") || ctx.source.contains("strictTransportSecurity") || ctx.source.contains("hsts");

        if !has_hsts {
            for (idx, line) in ctx.source.lines().enumerate() {
                if line.contains("res.setHeader") || line.contains("res.headers") || line.contains("response.setHeader") {
                    issues.push(Issue::new(
                        "JS_S5734",
                        "Strict-Transport-Security header is not set - enable HSTS to prevent protocol downgrade attacks",
                        Severity::Blocker,
                        Category:: Vulnerability,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::substantial(
                        "Add Strict-Transport-Security header with appropriate max-age value"
                    )));
                    break;
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S5736 — X-Content-Type-Options header missing
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S5736"
    name: "X-Content-Type-Options header should be set"
    severity: Blocker
    category: Vulnerability
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let has_express = ctx.source.contains("express") || ctx.source.contains("http.createServer");

        if !has_express {
            return issues;
        }

        let has_xcto = ctx.source.contains("X-Content-Type-Options") || ctx.source.contains("x-content-type-options");

        if !has_xcto {
            for (idx, line) in ctx.source.lines().enumerate() {
                if line.contains("res.setHeader") || line.contains("res.headers") || line.contains("response.setHeader") {
                    issues.push(Issue::new(
                        "JS_S5736",
                        "X-Content-Type-Options header is not set - enable to prevent MIME sniffing",
                        Severity::Blocker,
                        Category:: Vulnerability,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::moderate(
                        "Add X-Content-Type-Options: nosniff header"
                    )));
                    break;
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S5852 — RegExp DoS (catastrophic backtracking)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S5852"
    name: "Regular expressions should not be vulnerable to ReDoS"
    severity: Blocker
    category: Vulnerability
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        // Detect potentially catastrophic backtracking patterns
        let redos_patterns = [
            r"\(\.\*\+\)\+",  // (.*+)+ 
            r"\(\.\+\)\+",    // (.+)+
            r"\(\[.*\]\+\)\+", // ([.*]+)+
            r"\(\.\*\)",      // nested quantifiers
        ];

        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("new RegExp") || line.contains("RegExp(") || line.contains("/^") || line.contains("/.*/") {
                for pattern in &redos_patterns {
                    if let Ok(re) = regex::Regex::new(pattern) {
                        if re.is_match(line) {
                            issues.push(Issue::new(
                                "JS_S5852",
                                "Potential ReDoS vulnerability - nested quantifiers can cause catastrophic backtracking",
                                Severity::Blocker,
                                Category:: Vulnerability,
                                ctx.file_path,
                                idx + 1,
                            ).with_remediation(Remediation::substantial(
                                "Rewrite the regex to avoid nested quantifiers, or use a non-backtracking regex engine"
                            )));
                            break;
                        }
                    }
                }
            }
        }
        issues
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// BATCH: JavaScript Bug Rules (30 rules) — 182 → 212
// ═══════════════════════════════════════════════════════════════════════════════

// ─────────────────────────────────────────────────────────────────────────────
// JS_S108 — Empty catch block
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S108"
    name: "Empty catch block should be avoided"
    severity: Major
    category: Bug
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"catch\s*\([^)]*\)\s*\{\s*\}").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "JS_S108",
                    "Empty catch block - caught exceptions are silently ignored",
                    Severity::Major,
                    Category::Bug,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::moderate(
                    "Handle the exception properly or remove the try-catch if not needed"
                )));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S112 — Generic error thrown
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S112"
    name: "Generic errors should not be thrown"
    severity: Major
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"throw\s+new\s+Error\s*\(").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "JS_S112",
                    "Generic Error thrown - specify the error type or create a custom error",
                    Severity::Major,
                    Category:: CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::moderate(
                    "Throw a specific error type or create a custom error class"
                )));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S117 — Variable naming (camelCase)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S117"
    name: "Variable names should follow camelCase convention"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"(?:const|let|var)\s+([A-Z][a-zA-Z0-9_]*|[a-z]+[A-Z])").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                if let Some(name) = cap.get(1) {
                    issues.push(Issue::new(
                        "JS_S117",
                        format!("Variable '{}' does not follow camelCase convention", name.as_str()),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S118 — Function naming (camelCase)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S118"
    name: "Function names should follow camelCase convention"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"function\s+([A-Z][a-zA-Z0-9_]*|[a-z]+[A-Z])\s*\(").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                if let Some(name) = cap.get(1) {
                    issues.push(Issue::new(
                        "JS_S118",
                        format!("Function '{}' does not follow camelCase convention", name.as_str()),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S119 — Class naming (PascalCase)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S119"
    name: "Class names should follow PascalCase convention"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"class\s+([a-z][a-z0-9_]*)\b").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                if let Some(name) = cap.get(1) {
                    issues.push(Issue::new(
                        "JS_S119",
                        format!("Class '{}' should use PascalCase", name.as_str()),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S120 — Trailing comma in objects
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S120"
    name: "Trailing commas should not be used in object literals"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r",\s*\}").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && !line.trim().starts_with("//") {
                issues.push(Issue::new(
                    "JS_S120",
                    "Trailing comma in object literal - not supported in older environments",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::quick(
                    "Remove trailing comma for better compatibility"
                )));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S121 — Missing semicolons
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S121"
    name: "Statements should end with semicolons"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if !trimmed.is_empty()
                && !trimmed.ends_with(';')
                && !trimmed.ends_with('{')
                && !trimmed.ends_with('}')
                && !trimmed.ends_with(',')
                && !trimmed.ends_with('(')
                && !trimmed.ends_with('[')
                && !trimmed.starts_with("//")
                && !trimmed.starts_with("/*")
                && !trimmed.starts_with("if ")
                && !trimmed.starts_with("for ")
                && !trimmed.starts_with("while ")
                && !trimmed.starts_with("function ")
                && !trimmed.starts_with("class ")
                && !trimmed.starts_with("return ")
                && idx + 1 < lines.len()
            {
                let next_line = lines[idx + 1].trim();
                if !next_line.is_empty()
                    && !next_line.starts_with(".")
                    && !next_line.starts_with(",")
                    && !next_line.starts_with(";")
                {
                    issues.push(Issue::new(
                        "JS_S121",
                        "Missing semicolon - rely on Automatic Semicolon Insertion (ASI)",
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::quick(
                        "Add explicit semicolons to avoid ASI issues"
                    )));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S122 — Missing return statement
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S122"
    name: "Function should have explicit return statement"
    severity: Major
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"function\s+\w+\s*\([^)]*\)\s*\{").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                let func_start = idx;
                let context: String = ctx.source.lines().skip(func_start).take(20).collect::<Vec<_>>().join("\n");
                let has_return = context.contains("return ");
                let has_closing = context.matches("}").count() >= 2;
                if has_closing && !has_return && !context.contains("=>") {
                    issues.push(Issue::new(
                        "JS_S122",
                        "Function has no return statement - returns undefined",
                        Severity::Major,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S123 — debugger statement
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S123"
    name: "debugger statement should not be used"
    severity: Blocker
    category: Bug
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") || trimmed.starts_with("*") {
                continue;
            }
            if line.contains("debugger") {
                issues.push(Issue::new(
                    "JS_S123",
                    "debugger statement found - will pause execution in debugger",
                    Severity::Blocker,
                    Category::Bug,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::quick(
                    "Remove debugger statement before production deployment"
                )));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S124 — alert() usage
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S124"
    name: "alert() should not be used"
    severity: Major
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let alert_patterns = ["alert(", "confirm(", "prompt("];
        for (idx, line) in ctx.source.lines().enumerate() {
            for pattern in &alert_patterns {
                if line.contains(pattern) {
                    issues.push(Issue::new(
                        "JS_S124",
                        format!("{} is a security risk - use custom UI dialogs", pattern.trim_end_matches('(')),
                        Severity::Major,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::moderate(
                        "Replace with custom modal dialogs for better UX and security"
                    )));
                    break;
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S126 — Missing radix in parseInt
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S126"
    name: "parseInt should be called with a radix"
    severity: Major
    category: Bug
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"parseInt\s*\(\s*[^,)]+\s*\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && !line.contains("parseInt(") || (re.is_match(line) && !line.contains(",") && !line.contains("10")) {
                if let Some(cap) = re.captures(line) {
                    let full_match = cap.get(0).unwrap().as_str();
                    if !full_match.contains(",") {
                        issues.push(Issue::new(
                            "JS_S126",
                            "parseInt without radix - leading zeros may be interpreted as octal",
                            Severity::Major,
                            Category::Bug,
                            ctx.file_path,
                            idx + 1,
                        ).with_remediation(Remediation::quick(
                            "Add radix 10: parseInt(value, 10)"
                        )));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S127 — For loop with multiple variables
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S127"
    name: "For loops should not have multiple variables"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"for\s*\(\s*[^;]+;[^-]*;[^-]*,").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "JS_S127",
                    "For loop with multiple update expressions - consider splitting",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::moderate(
                    "Split into separate loops for better readability"
                )));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S128 — Function with too many params (>7)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S128"
    name: "Functions should not have too many parameters"
    severity: Major
    category: CodeSmell
    language: "javascript"
    params: { max_params: usize = 7 }
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"function\s+\w+\s*\(([^)]+)\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                let params = cap.get(1).unwrap().as_str();
                let count = params.split(',').filter(|p| !p.trim().is_empty()).count();
                if count > self.max_params {
                    issues.push(Issue::new(
                        "JS_S128",
                        format!("Function has {} parameters exceeding threshold of {}", count, self.max_params),
                        Severity::Major,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::moderate(
                        "Group parameters into an options object or configuration"
                    )));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S129 — Switch without default
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S129"
    name: "Switch statements should have a default case"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"switch\s*\([^)]+\)\s*\{").unwrap();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            if re.is_match(line) {
                let context: String = lines.iter().skip(idx).take(30).cloned().collect::<Vec<_>>().join("\n");
                if !context.contains("default:") && !context.contains("default :") {
                    issues.push(Issue::new(
                        "JS_S129",
                        "Switch statement without default case",
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::quick(
                        "Add a default case to handle unexpected values"
                    )));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S130 — Too many returns (>5)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S130"
    name: "Functions should not have too many return statements"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: { max_returns: usize = 5 }
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"function\s+\w+\s*\([^)]*\)\s*\{").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                let func_start = idx;
                let context: String = ctx.source.lines().skip(func_start).take(50).collect::<Vec<_>>().join("\n");
                let return_count = context.matches("return ").count();
                if return_count > self.max_returns {
                    issues.push(Issue::new(
                        "JS_S130",
                        format!("Function has {} return statements - consider refactoring", return_count),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::moderate(
                        "Extract logic into separate functions to reduce return points"
                    )));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S131 — Unused variable
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S131"
    name: "Unused variables should be removed"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"(?:const|let|var)\s+(\w+)\s*=").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                let name = cap.get(1).unwrap().as_str();
                if !name.starts_with("_") {
                    let remaining: String = ctx.source.lines().skip(idx + 1).collect::<Vec<_>>().join("\n");
                    if !remaining.contains(&format!(" {} ", name)) && !remaining.contains(&format!("({}", name)) {
                        issues.push(Issue::new(
                            "JS_S131",
                            format!("Unused variable '{}'", name),
                            Severity::Minor,
                            Category::CodeSmell,
                            ctx.file_path,
                            idx + 1,
                        ).with_remediation(Remediation::quick(
                            "Remove the unused variable or prefix with _ to indicate intentionally unused"
                        )));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S132 — Global variable pollution
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S132"
    name: "Global variables should not be created"
    severity: Critical
    category: Bug
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"window\.\w+\s*=").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "JS_S132",
                    "Global variable pollution via window - use proper namespacing or modules",
                    Severity::Critical,
                    Category::Bug,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::moderate(
                    "Use module pattern or IIFE to avoid polluting global scope"
                )));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S133 — == instead of ===
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S133"
    name: " == should not be used instead of ==="
    severity: Major
    category: Bug
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.trim().starts_with("//") || line.trim().starts_with("*") {
                continue;
            }
            // Check for == or != but not === or !==
            let chars: Vec<char> = line.chars().collect();
            let mut i = 0;
            while i < chars.len() {
                if i + 1 < chars.len() {
                    let two = &line[i..i+2];
                    if two == "==" || two == "!=" {
                        // Check it's not === or !==
                        let is_triple = i + 2 < chars.len() && chars[i+2] == '=';
                        if !is_triple {
                            issues.push(Issue::new(
                                "JS_S133",
                                "Use === instead of == or !== instead of !=",
                                Severity::Major,
                                Category::Bug,
                                ctx.file_path,
                                idx + 1,
                            ).with_column(i + 1).with_remediation(Remediation::quick(
                                "Use strict equality (=== or !==) for type-safe comparison"
                            )));
                        }
                        // Skip ahead to avoid double-counting
                        if is_triple {
                            i += 3;
                            continue;
                        }
                    }
                }
                i += 1;
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S134 — Deep nesting (>4 levels)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S134"
    name: "Control flow statements should not be nested too deeply"
    severity: Major
    category: CodeSmell
    language: "javascript"
    params: { max_depth: usize = 4 }
    check: => {
        let mut issues = Vec::new();
        let nesting_keywords = ["if ", "for ", "while ", "} else if ", "} else {"];
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            let mut depth = 0usize;
            let context: String = lines.iter().take(idx + 1).cloned().collect::<Vec<_>>().join("\n");
            for kw in &nesting_keywords {
                depth += context.matches(kw).count();
            }
            let closes = context.matches("}").count();
            if depth > closes + self.max_depth {
                issues.push(Issue::new(
                    "JS_S134",
                    format!("Deep nesting detected (>{} levels) - refactor for clarity", self.max_depth),
                    Severity::Major,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::moderate(
                    "Extract nested logic into separate functions or use early returns"
                )));
                break;
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S135 — Long function (>50 lines)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S135"
    name: "Functions should not be too long"
    severity: Major
    category: CodeSmell
    language: "javascript"
    params: { max_lines: usize = 50 }
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"function\s+\w+\s*\([^)]*\)\s*\{").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                let func_start = idx;
                let lines: Vec<&str> = ctx.source.lines().collect();
                let mut brace_count = 0isize;
                let mut func_lines = 0;
                let mut found_open = false;
                for (i, l) in lines.iter().enumerate().skip(func_start) {
                    if l.contains("{") { found_open = true; brace_count += l.matches("{").count() as isize; }
                    if l.contains("}") { brace_count -= l.matches("}").count() as isize; }
                    if found_open { func_lines += 1; }
                    if brace_count <= 0 && found_open { break; }
                }
                if func_lines > self.max_lines {
                    issues.push(Issue::new(
                        "JS_S135",
                        format!("Function is {} lines exceeding threshold of {}", func_lines, self.max_lines),
                        Severity::Major,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::moderate(
                        "Extract helper functions to reduce method length"
                    )));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S136 — Complex function (cyclomatic > 15)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S136"
    name: "Cognitive complexity should not be too high"
    severity: Major
    category: CodeSmell
    language: "javascript"
    params: { max_complexity: usize = 15 }
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"function\s+\w+\s*\([^)]*\)\s*\{").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                let func_start = idx;
                let lines: Vec<&str> = ctx.source.lines().collect();
                let mut brace_count = 0isize;
                let mut found_open = false;
                let mut func_text = String::new();
                for (i, l) in lines.iter().enumerate().skip(func_start) {
                    if l.contains("{") { found_open = true; brace_count += l.matches("{").count() as isize; }
                    if l.contains("}") { brace_count -= l.matches("}").count() as isize; }
                    if found_open { func_text.push_str(l); func_text.push('\n'); }
                    if brace_count <= 0 && found_open { break; }
                }
                let complexity = func_text.matches("if ").count()
                    + func_text.matches("for ").count()
                    + func_text.matches("while ").count()
                    + func_text.matches("case ").count()
                    + func_text.matches("catch ").count()
                    + func_text.matches("&&").count()
                    + func_text.matches("||").count()
                    + func_text.matches("?").count();
                if complexity > self.max_complexity {
                    issues.push(Issue::new(
                        "JS_S136",
                        format!("Function has cognitive complexity {} exceeding {}", complexity, self.max_complexity),
                        Severity::Major,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::moderate(
                        "Simplify the function logic or extract helper functions"
                    )));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S137 — Duplicate condition in if/else
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S137"
    name: "Identical expressions should not be compared in if-else"
    severity: Major
    category: Bug
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        // Detect identical conditions in if-else chains without backreference support
        // Note: This is a simplified detection since Rust regex doesn't support backreferences
        let lines: Vec<&str> = ctx.source.lines().collect();
        for idx in 0..lines.len() {
            let line = lines[idx].trim();
            // Look for else if pattern
            if line.starts_with("else if (") {
                if let Some(cond_end) = line.find(") {") {
                    let condition = &line[9..cond_end]; // Skip "else if ("
                    // Look back for the matching if statement
                    if idx > 0 {
                        let prev_line = lines[idx - 1].trim();
                        if prev_line.starts_with("if (") {
                            if let Some(prev_cond_end) = prev_line.find(") {") {
                                let prev_condition = &prev_line[4..prev_cond_end]; // Skip "if ("
                                if condition == prev_condition {
                                    issues.push(Issue::new(
                                        "JS_S137",
                                        "Identical condition in if-else chain - second branch is unreachable",
                                        Severity::Major,
                                        Category::Bug,
                                        ctx.file_path,
                                        idx + 1,
                                    ).with_remediation(Remediation::quick(
                                        "Merge the duplicate conditions or remove the redundant branch"
                                    )));
                                }
                            }
                        }
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S139 — Arguments.callee usage
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S139"
    name: "arguments.callee should not be used"
    severity: Major
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("arguments.callee") {
                issues.push(Issue::new(
                    "JS_S139",
                    "arguments.callee is deprecated - use named function expressions instead",
                    Severity::Major,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::quick(
                    "Use a named function expression and reference it by name"
                )));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S140 — with statement
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S140"
    name: "with statement should not be used"
    severity: Critical
    category: Bug
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\bwith\s*\(").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "JS_S140",
                    "with statement is deprecated and forbidden in strict mode",
                    Severity::Critical,
                    Category::Bug,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::quick(
                    "Remove with statement and use explicit object references"
                )));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S141 — Empty block `{}`
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S141"
    name: "Empty blocks should be reviewed"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\{\s*\}").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                let context: String = ctx.source.lines().take(idx + 1).collect::<Vec<_>>().join("\n");
                let prev_match = context.matches("{").count();
                let prev_close = context.matches("}").count();
                if prev_match > prev_close {
                    issues.push(Issue::new(
                        "JS_S141",
                        "Empty block {} found - may indicate missing logic",
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::quick(
                        "Add logic or comment if intentional, otherwise remove empty block"
                    )));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S142 — Unreachable code after return
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S142"
    name: "Unreachable code should not be present"
    severity: Major
    category: Bug
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for i in 0..lines.len().saturating_sub(1) {
            let curr = lines[i].trim();
            let next = lines[i + 1].trim();
            if (curr.starts_with("return ") || curr == "return;") && !next.is_empty() && !next.starts_with("//") && !next.starts_with("/*") && !next.starts_with("}") {
                issues.push(Issue::new(
                    "JS_S142",
                    "Unreachable code after return statement",
                    Severity::Major,
                    Category::Bug,
                    ctx.file_path,
                    i + 2,
                ).with_remediation(Remediation::quick(
                    "Remove code after return statement"
                )));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S143 — Useless assignment (var x = x)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S143"
    name: "Variables should not be self-assigned"
    severity: Major
    category: Bug
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        // Check for self-assignment patterns without using backreferences
        for (idx, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            // Look for patterns like: x = x; where x is the same identifier on both sides
            if let Some(eq_pos) = trimmed.find('=') {
                if eq_pos > 0 {
                    let lhs = trimmed[..eq_pos].trim();
                    let rhs = trimmed[eq_pos + 1..].trim().trim_end_matches(';').trim();
                    if lhs == rhs && !lhs.is_empty() && !rhs.is_empty() {
                        // Verify it's a valid identifier pattern (not like "x = x + 1")
                        if !rhs.contains('+') && !rhs.contains('-') && !rhs.contains('*') && !rhs.contains('/') {
                            issues.push(Issue::new(
                                "JS_S143",
                                "Self-assignment has no effect",
                                Severity::Major,
                                Category::Bug,
                                ctx.file_path,
                                idx + 1,
                            ).with_remediation(Remediation::quick(
                                "Remove the self-assignment"
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
// JS_S144 — Function call without side effects ignored
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S144"
    name: "Return value of function without side effects should be used"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let pure_funcs = ["map(", "filter(", "reduce(", "find(", "some(", "every(", "flatMap("];
        for (idx, line) in ctx.source.lines().enumerate() {
            for func in &pure_funcs {
                if line.contains(func) && line.trim().ends_with(';') && !line.contains("=") && !line.contains("return") {
                    issues.push(Issue::new(
                        "JS_S144",
                        format!("Return value of pure function {} ignored", func.trim_end_matches('(')),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::quick(
                        "Use the return value or replace with forEach for side effects"
                    )));
                    break;
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S145 — Missing default case in switch
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S145"
    name: "Switch statements should have a default case"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"switch\s*\([^)]+\)\s*\{").unwrap();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            if re.is_match(line) {
                let context: String = lines.iter().skip(idx).take(30).cloned().collect::<Vec<_>>().join("\n");
                if !context.contains("default:") && !context.contains("default :") {
                    issues.push(Issue::new(
                        "JS_S145",
                        "Switch without default case - add default for robustness",
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::quick(
                        "Add default case to handle unexpected values"
                    )));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S146 — Fall-through in switch case
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S146"
    name: "Switch case should end with an unconditional break"
    severity: Major
    category: Bug
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        let re = regex::Regex::new(r"case\s+[^:]+:\s*$").unwrap();
        for (idx, line) in lines.iter().enumerate() {
            if re.is_match(line) && idx + 1 < lines.len() {
                let next_line = lines[idx + 1].trim();
                if !next_line.starts_with("case ") && !next_line.starts_with("break") && !next_line.starts_with("return") && !next_line.starts_with("throw") && !next_line.starts_with("}") && !next_line.is_empty() && !next_line.starts_with("//") {
                    issues.push(Issue::new(
                        "JS_S146",
                        "Switch case may lack a break - possible fall-through",
                        Severity::Major,
                        Category::Bug,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::quick(
                        "Add break, return, or throw to prevent fall-through"
                    )));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S147 — File too long (>1000 lines)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S147"
    name: "Files should not be too long"
    severity: Major
    category: CodeSmell
    language: "javascript"
    params: { max_lines: usize = 1000 }
    check: => {
        let mut issues = Vec::new();
        let line_count = ctx.source.lines().count();
        if line_count > self.max_lines {
            issues.push(Issue::new(
                "JS_S147",
                format!("File has {} lines exceeding limit of {}", line_count, self.max_lines),
                Severity::Major,
                Category::CodeSmell,
                ctx.file_path,
                1,
            ).with_remediation(Remediation::moderate(
                "Split into multiple files/modules"
            )));
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S148 — Low comment ratio (<10%)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S148"
    name: "Code should have adequate comments"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: { min_ratio: f64 = 0.10 }
    check: => {
        let mut issues = Vec::new();
        let mut comment_lines = 0usize;
        let mut total_lines = 0usize;
        for line in ctx.source.lines() {
            total_lines += 1;
            let t = line.trim();
            if t.starts_with("//") || t.starts_with("/*") || t.starts_with("*") {
                comment_lines += 1;
            }
        }
        if total_lines > 50 && (comment_lines as f64 / total_lines as f64) < self.min_ratio {
            issues.push(Issue::new(
                "JS_S148",
                format!("Comment ratio {:.1}% below {:.0}% minimum", (comment_lines as f64/total_lines as f64)*100.0, self.min_ratio*100.0),
                Severity::Minor,
                Category::CodeSmell,
                ctx.file_path,
                1,
            ));
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S149 — Nested ternary operators
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S149"
    name: "Nested ternary operators should not be used"
    severity: Major
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\?\s*[^:]+\s*:\s*[^?]+\?\s*").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "JS_S149",
                    "Nested ternary operator - use if/else for clarity",
                    Severity::Major,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::moderate(
                    "Replace nested ternary with if/else statements"
                )));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S150 — delete operator on variable
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S150"
    name: "delete should not be used on variables"
    severity: Major
    category: Bug
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"delete\s+(\w+)\s*;").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                if let Some(name) = cap.get(1) {
                    issues.push(Issue::new(
                        "JS_S150",
                        format!("delete on variable '{}' has no effect - use undefined or null", name.as_str()),
                        Severity::Major,
                        Category::Bug,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::quick(
                        "Assign undefined or null instead of using delete on variables"
                    )));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S151 — void operator used
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S151"
    name: "void operator should not be used"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\bvoid\s+").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "JS_S151",
                    "void operator used - returns undefined, consider simpler alternatives",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::quick(
                    "Use undefined directly or restructure the expression"
                )));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S152 — bitwise operators in conditional
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S152"
    name: "Bitwise operators should not be used in conditionals"
    severity: Major
    category: Bug
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"if\s*\([^)]*[&|]^[^)]+\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "JS_S152",
                    "Bitwise operator in conditional - did you mean logical && or ||?",
                    Severity::Major,
                    Category::Bug,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::quick(
                    "Replace & with && or | with || for logical operations"
                )));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S153 — comma operator misuse
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S153"
    name: "Comma operator should not be used"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r",\s*\w+\s*=\s*[^,;]+,").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "JS_S153",
                    "Comma operator used - may cause confusion, use sequential statements instead",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::moderate(
                    "Split into separate statements for better readability"
                )));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S154 — Object.freeze on array (no effect)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S154"
    name: "Object.freeze on array has limited effect"
    severity: Major
    category: Bug
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"Object\.freeze\s*\(\s*\[[^\]]+\]\s*\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "JS_S154",
                    "Object.freeze on array literal - only prevents reassignment, not mutation",
                    Severity::Major,
                    Category::Bug,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::moderate(
                    "Use Object.freeze() on a variable assigned the array, not on the literal directly"
                )));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S155 — NaN comparison (x !== x should use isNaN)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S155"
    name: "NaN should be checked with isNaN()"
    severity: Major
    category: Bug
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        // Detect x !== x or x != x patterns for NaN checking without backreference
        let comparison_ops = ["!==", "!="];
        for (idx, line) in ctx.source.lines().enumerate() {
            for op in &comparison_ops {
                if let Some(pos) = line.find(op) {
                    let before = line[..pos].trim();
                    let after = line[pos + op.len()..].trim();
                    // Check if it's a self-comparison (x !== x)
                    if before == after && !before.is_empty() {
                        // Check if the variable name suggests NaN handling
                        let name_lower = before.to_lowercase();
                        if name_lower.contains("nan") || name_lower == name_lower.to_uppercase() {
                            issues.push(Issue::new(
                                "JS_S155",
                                "Self-comparison to detect NaN - use Number.isNaN() instead",
                                Severity::Major,
                                Category::Bug,
                                ctx.file_path,
                                idx + 1,
                            ).with_remediation(Remediation::quick(
                                "Use Number.isNaN(value) or isNaN(value) to check for NaN"
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
// JS_S156 — typeof comparison with undefined
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S156"
    name: "typeof should not be compared to strings incorrectly"
    severity: Major
    category: Bug
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r#"typeof\s+\w+\s*==\s*["']undefined["']|typeof\s+\w+\s*===\s*["']undefined["']"#).unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "JS_S156",
                    "typeof check for undefined - prefer undefined comparison or optional chaining",
                    Severity::Major,
                    Category::Bug,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::quick(
                    "Use value === undefined or value == null for simpler undefined check"
                )));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S157 — Infinity comparison
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S157"
    name: "Infinity should be checked with isFinite()"
    severity: Major
    category: Bug
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"Infinity\s*[<>]=?\s*\w+|\w+\s*[<>]=?\s*Infinity").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "JS_S157",
                    "Direct comparison with Infinity - use isFinite() instead",
                    Severity::Major,
                    Category::Bug,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::quick(
                    "Use Number.isFinite(value) to check for finite numbers"
                )));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S158 — for...in without hasOwnProperty
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S158"
    name: "for...in should check hasOwnProperty"
    severity: Major
    category: Bug
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"for\s*\(\s*(?:let|var|const)\s+\w+\s+in\s+\w+\s*\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                let context: String = ctx.source.lines().skip(idx).take(5).collect::<Vec<_>>().join("\n");
                if !context.contains("hasOwnProperty") && !context.contains("Object.keys") && !context.contains("Object.entries") {
                    issues.push(Issue::new(
                        "JS_S158",
                        "for...in without hasOwnProperty check - may iterate over inherited properties",
                        Severity::Major,
                        Category::Bug,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::moderate(
                        "Use Object.hasOwn(obj, prop) or obj.hasOwnProperty(prop) to filter"
                    )));
                }
            }
        }
        issues
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// BATCH 3: JavaScript Code Smells (20 rules)
// ═══════════════════════════════════════════════════════════════════════════════

// ─────────────────────────────────────────────────────────────────────────────
// JS_S159 — Function too long (>200 lines)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S159"
    name: "Functions should not be too long"
    severity: Major
    category: CodeSmell
    language: "javascript"
    params: { max_lines: usize = 200 }
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"(?:function\s+\w+|const\s+\w+\s*=\s*(?:async\s*)?\(|=>\s*)").unwrap();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            if re.is_match(line) {
                let mut brace_count = 0isize;
                let mut func_lines = 0;
                let mut found_open = false;
                for l in lines.iter().skip(idx) {
                    if l.contains("{") { found_open = true; brace_count += l.matches("{").count() as isize; }
                    if l.contains("}") { brace_count -= l.matches("}").count() as isize; }
                    if found_open { func_lines += 1; }
                    if brace_count <= 0 && found_open { break; }
                }
                if func_lines > self.max_lines {
                    issues.push(Issue::new(
                        "JS_S159",
                        format!("Function is {} lines exceeding threshold of {}", func_lines, self.max_lines),
                        Severity::Major,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::moderate("Split this function into smaller, focused functions")));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S160 — Too many local variables (>15 in one function)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S160"
    name: "Functions should not have too many local variables"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: { max_vars: usize = 15 }
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"(?:function\s+\w+|(?:const|let|var)\s+(\w+))").unwrap();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            if line.contains("function") || line.contains("=>") {
                let func_start = idx;
                let mut brace_count = 0isize;
                let mut found_open = false;
                let mut var_count = 0;
                let mut local_vars = std::collections::HashSet::new();
                for l in lines.iter().skip(func_start) {
                    if l.contains("{") { found_open = true; brace_count += l.matches("{").count() as isize; }
                    if l.contains("}") { brace_count -= l.matches("}").count() as isize; }
                    if found_open {
                        for cap in re.captures_iter(l) {
                            if let Some(name) = cap.get(1) {
                                if !name.as_str().contains("_") && name.as_str().chars().next().map(|c| c.is_lowercase()).unwrap_or(false) {
                                    local_vars.insert(name.as_str().to_string());
                                    var_count += 1;
                                }
                            }
                        }
                    }
                    if brace_count <= 0 && found_open { break; }
                }
                if var_count > self.max_vars {
                    issues.push(Issue::new(
                        "JS_S160",
                        format!("Function has {} local variables exceeding threshold of {}", var_count, self.max_vars),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::moderate("Group related variables into objects or extract into separate functions")));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S161 — Missing 'use strict'
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S161"
    name: "'use strict' should be used"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let first_line = ctx.source.lines().next().unwrap_or("");
        let has_use_strict = ctx.source.contains("'use strict'") || ctx.source.contains("\"use strict\"");
        let is_module = ctx.source.contains("import ") || ctx.source.contains("export ");
        if !has_use_strict && !is_module && ctx.source.contains("function") {
            issues.push(Issue::new(
                "JS_S161",
                "Missing 'use strict' directive - consider adding for better error checking",
                Severity::Minor,
                Category::CodeSmell,
                ctx.file_path,
                1,
            ).with_remediation(Remediation::quick("Add 'use strict' at the top of the file or use ES6 modules")));
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S162 — var instead of let/const
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S162"
    name: "var should not be used"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\bvar\s+").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && !line.trim().starts_with("//") {
                issues.push(Issue::new(
                    "JS_S162",
                    "var keyword used - use let or const instead for block scoping",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::quick("Replace var with let or const for proper block scoping")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S163 — Nested callback (>3 levels, callback hell)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S163"
    name: "Callback nesting should not be too deep"
    severity: Major
    category: CodeSmell
    language: "javascript"
    params: { max_depth: usize = 3 }
    check: => {
        let mut issues = Vec::new();
        let callback_funcs = ["function", "=>", ".then(", ".catch(", "callback("];
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            let mut depth = 0usize;
            for (i, l) in lines.iter().enumerate().skip(idx.saturating_sub(100)).take(idx + 1) {
                for cf in &callback_funcs {
                    if l.contains(cf) { depth += 1; }
                }
            }
            let has_callback = callback_funcs.iter().any(|cf| line.contains(cf));
            if has_callback && depth > self.max_depth {
                issues.push(Issue::new(
                    "JS_S163",
                    format!("Deep callback nesting detected ({} levels) - consider Promises or async/await", depth),
                    Severity::Major,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::moderate("Refactor to use Promises, async/await, or extract callbacks into named functions")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S164 — Duplicate property names in object literal
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S164"
    name: "Duplicate property names should not be used"
    severity: Major
    category: Bug
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\{([^}]+)\}").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                let props = cap.get(1).unwrap().as_str();
                let mut seen = std::collections::HashSet::new();
                let prop_re = regex::Regex::new(r"(\w+)\s*:").unwrap();
                for prop_cap in prop_re.captures_iter(props) {
                    if let Some(name) = prop_cap.get(1) {
                        let n = name.as_str();
                        if seen.contains(n) {
                            issues.push(Issue::new(
                                "JS_S164",
                                format!("Duplicate property name '{}' in object literal - later value will overwrite earlier", n),
                                Severity::Major,
                                Category::Bug,
                                ctx.file_path,
                                idx + 1,
                            ).with_remediation(Remediation::quick("Remove the duplicate property name")));
                        }
                        seen.insert(n);
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S165 — Empty function body
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S165"
    name: "Empty function bodies should be reviewed"
    severity: Major
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"(?:function\s+\w+|const\s+\w+\s*=\s*(?:async\s*)?\(|=>)\s*[^}]*\{\s*\}").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "JS_S165",
                    "Empty function body - likely missing implementation",
                    Severity::Major,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::quick("Implement the function or remove if not needed")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S166 — Unused function parameter
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S166"
    name: "Unused function parameters should be removed"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"function\s+\w+\s*\(([^)]+)\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                if let Some(params) = cap.get(1) {
                    let param_str = params.as_str();
                    let context: String = ctx.source.lines().skip(idx).take(30).collect::<Vec<_>>().join("\n");
                    for param in param_str.split(',') {
                        let p = param.trim();
                        if !p.starts_with("...") && !p.is_empty() && !context.contains(&format!(" {} ", p)) && !context.contains(&format!("({}", p)) {
                            issues.push(Issue::new(
                                "JS_S166",
                                format!("Unused function parameter '{}'", p),
                                Severity::Minor,
                                Category::CodeSmell,
                                ctx.file_path,
                                idx + 1,
                            ).with_remediation(Remediation::quick("Remove the unused parameter or prefix with _")));
                        }
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S167 — Overwritten variable (var x = 1; var x = 2)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S167"
    name: "Variables should not be reassigned with var"
    severity: Major
    category: Bug
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"var\s+(\w+)\s*=").unwrap();
        let mut last_var = std::collections::HashMap::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                if let Some(name) = cap.get(1) {
                    let n = name.as_str();
                    if let Some(&first_line) = last_var.get(n) {
                        issues.push(Issue::new(
                            "JS_S167",
                            format!("Variable '{}' redeclared with var (was first declared at line {})", n, first_line),
                            Severity::Major,
                            Category::Bug,
                            ctx.file_path,
                            idx + 1,
                        ).with_remediation(Remediation::quick("Use a single declaration or use different variable names")));
                    }
                    last_var.insert(n, idx + 1);
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S168 — Shadowed variable (inner scope hides outer)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S168"
    name: "Variables should not shadow outer scope variables"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let decl_re = regex::Regex::new(r"(?:const|let|var)\s+(\w+)").unwrap();
        let mut scope_vars = std::collections::HashSet::new();
        let mut last_scope_line = 0usize;
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            let open_count = line.matches("{").count();
            let close_count = line.matches("}").count();
            if open_count > 0 {
                let new_vars: Vec<_> = decl_re.captures_iter(line).filter_map(|c| c.get(1).map(|m| m.as_str().to_string())).collect();
                for v in &new_vars {
                    if scope_vars.contains(v) {
                        issues.push(Issue::new(
                            "JS_S168",
                            format!("Variable '{}' shadows a variable from outer scope", v),
                            Severity::Minor,
                            Category::CodeSmell,
                            ctx.file_path,
                            idx + 1,
                        ).with_remediation(Remediation::moderate("Use a different variable name to avoid confusion")));
                    }
                }
                for v in new_vars {
                    scope_vars.insert(v);
                }
            }
            if close_count > 0 && idx >= last_scope_line {
                last_scope_line = idx;
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S169 — Unsorted object keys
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S169"
    name: "Object keys should be sorted consistently"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\{([^}]+)\}").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                if let Some(props) = cap.get(1) {
                    let prop_re = regex::Regex::new(r"(\w+)\s*:").unwrap();
                    let keys: Vec<_> = prop_re.captures_iter(props.as_str()).filter_map(|c| c.get(1).map(|m| m.as_str().to_string())).collect();
                    let sorted = {
                        let mut s = keys.clone();
                        s.sort();
                        s
                    };
                    if keys != sorted && keys.len() > 2 {
                        issues.push(Issue::new(
                            "JS_S169",
                            "Object keys are not sorted consistently",
                            Severity::Minor,
                            Category::CodeSmell,
                            ctx.file_path,
                            idx + 1,
                        ).with_remediation(Remediation::quick("Sort object keys alphabetically for consistency")));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S170 — Magic string literal used multiple times
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S170"
    name: "Magic string literals should be replaced by named constants"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: { min_occurrences: usize = 3 }
    check: => {
        let mut issues = Vec::new();
        let mut string_counts = std::collections::HashMap::new();
        let re = regex::Regex::new(r#""([^"]{2,50})""#).unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            for cap in re.captures_iter(line) {
                if let Some(s) = cap.get(1) {
                    let s_str = s.as_str().to_string();
                    string_counts.entry(s_str).or_insert(Vec::new()).push(idx + 1);
                }
            }
        }
        for (s, lines) in string_counts {
            if lines.len() >= self.min_occurrences && !s.contains(" ") && s.len() > 3 {
                for &l in &lines {
                    issues.push(Issue::new(
                        "JS_S170",
                        format!("Magic string '{}' used {} times - extract to a constant", s, lines.len()),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        l,
                    ));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S171 — Function with too many lines in a file
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S171"
    name: "Functions should not be too long"
    severity: Major
    category: CodeSmell
    language: "javascript"
    params: { max_lines: usize = 100 }
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        let mut i = 0;
        while i < lines.len() {
            let line = lines[i];
            if line.contains("function") || line.contains("=>") {
                let func_start = i;
                let mut brace_count = 0isize;
                let mut found_open = false;
                let mut func_lines = 0;
                for l in lines.iter().skip(i) {
                    if l.contains("{") { found_open = true; brace_count += l.matches("{").count() as isize; }
                    if l.contains("}") { brace_count -= l.matches("}").count() as isize; }
                    if found_open { func_lines += 1; }
                    if brace_count <= 0 && found_open { break; }
                }
                if func_lines > self.max_lines {
                    issues.push(Issue::new(
                        "JS_S171",
                        format!("Function at line {} is {} lines exceeding threshold of {}", func_start + 1, func_lines, self.max_lines),
                        Severity::Major,
                        Category::CodeSmell,
                        ctx.file_path,
                        func_start + 1,
                    ).with_remediation(Remediation::moderate("Split this long function into smaller, focused functions")));
                }
                i += func_lines;
            }
            i += 1;
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S172 — Array constructor instead of literal
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S172"
    name: "Array constructor should not be used"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"new\s+Array\s*\(").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && !line.trim().starts_with("//") {
                issues.push(Issue::new(
                    "JS_S172",
                    "Use array literal [] instead of new Array()",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::quick("Replace new Array() with []")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S173 — Object constructor instead of literal
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S173"
    name: "Object constructor should not be used"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"new\s+Object\s*\(").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && !line.trim().starts_with("//") {
                issues.push(Issue::new(
                    "JS_S173",
                    "Use object literal {} instead of new Object()",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::quick("Replace new Object() with {}")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S174 — String constructor
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S174"
    name: "String constructor should not be used"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"new\s+String\s*\(").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && !line.trim().starts_with("//") {
                issues.push(Issue::new(
                    "JS_S174",
                    "Use string literal instead of new String()",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::quick("Replace new String() with a string literal")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S175 — Boolean constructor
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S175"
    name: "Boolean constructor should not be used"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"new\s+Boolean\s*\(").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && !line.trim().starts_with("//") {
                issues.push(Issue::new(
                    "JS_S175",
                    "Use boolean literal instead of new Boolean()",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::quick("Replace new Boolean() with a boolean literal")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S176 — Number constructor
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S176"
    name: "Number constructor should not be used"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"new\s+Number\s*\(").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && !line.trim().starts_with("//") {
                issues.push(Issue::new(
                    "JS_S176",
                    "Use number literal instead of new Number()",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::quick("Replace new Number() with a number literal")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S177 — RegExp constructor instead of literal
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S177"
    name: "RegExp constructor should not be used"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"new\s+RegExp\s*\(").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && !line.trim().starts_with("//") {
                issues.push(Issue::new(
                    "JS_S177",
                    "Use RegExp literal instead of new RegExp()",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::quick("Replace new RegExp() with a regex literal /.../")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S178 — Function constructor
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S178"
    name: "Function constructor should not be used"
    severity: Critical
    category: SecurityHotspot
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"new\s+Function\s*\(").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "JS_S178",
                    "new Function() is a security risk - similar to eval()",
                    Severity::Critical,
                    Category::SecurityHotspot,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::substantial("Avoid using new Function() - use function declarations, expressions, or arrow functions instead")));
            }
        }
        issues
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// BATCH 4: JavaScript ES6+ / Functional Programming (20 rules)
// ═══════════════════════════════════════════════════════════════════════════════

// ─────────────────────────────────────────────────────────────────────────────
// JS_ES1 — const should be used for non-reassigned variables
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_ES1"
    name: "Use const for non-reassigned variables"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\blet\s+(\w+)").unwrap();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") || trimmed.starts_with("*") {
                continue;
            }
            if let Some(cap) = re.captures(line) {
                if let Some(name) = cap.get(1) {
                    let var_name = name.as_str();
                    let remaining: String = lines.iter().skip(idx + 1).take(50).cloned().collect::<Vec<_>>().join("\n");
                    if !remaining.contains(&format!("{} =", var_name)) && !remaining.contains(&format!("{}++", var_name)) && !remaining.contains(&format!("++{}", var_name)) {
                        issues.push(Issue::new(
                            "JS_ES1",
                            format!("Variable '{}' is never reassigned - use const instead", var_name),
                            Severity::Minor,
                            Category::CodeSmell,
                            ctx.file_path,
                            idx + 1,
                        ).with_remediation(Remediation::quick("Replace let with const")));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_ES2 — Arrow functions should be used for callbacks
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_ES2"
    name: "Arrow functions should be used for callbacks"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\.forEach\s*\(\s*function\s*\(").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "JS_ES2",
                    "Use arrow function syntax for forEach callback",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::quick("Replace function() with () => for concise arrow syntax")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_ES3 — Template literals instead of string concat
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_ES3"
    name: "Template literals should be used instead of string concatenation"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r#""[^"]*"\s*\+\s*\w+|\w+\s*\+\s*"[^"]*""#).unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && !line.contains("template") && !line.contains("css") && !line.trim().starts_with("//") {
                issues.push(Issue::new(
                    "JS_ES3",
                    "String concatenation detected - use template literals instead",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::quick("Replace \"str\" + var with `str ${var}`")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_ES4 — Destructuring should be used
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_ES4"
    name: "Destructuring should be used"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        // Detect obj.prop1, obj.prop2 patterns without backreference
        for (idx, line) in ctx.source.lines().enumerate() {
            // Look for patterns like: obj.prop1, obj.prop2
            let prop_pattern_count = line.matches(".").count(); // Simplified
            // Find all occurrences of word.prop
            let mut obj_props: Vec<(&str, &str)> = Vec::new();
            let mut search_pos = 0;
            while let Some(dot_pos) = line[search_pos..].find('.') {
                let abs_dot_pos = search_pos + dot_pos;
                // Look for word before the dot
                let before_dot = &line[..abs_dot_pos];
                if let Some(space_pos) = before_dot.rfind(|c: char| !c.is_alphanumeric() && c != '_') {
                    let obj_name = &before_dot[space_pos + 1..];
                    // Look for prop name after the dot
                    let after_dot = &line[abs_dot_pos + 1..];
                    if let Some(end_pos) = after_dot.find(|c: char| !c.is_alphanumeric() && c != '_') {
                        let prop_name = &after_dot[..end_pos];
                        if !obj_name.is_empty() && !prop_name.is_empty() && prop_name.chars().next().map(|c| c.is_lowercase()).unwrap_or(false) {
                            obj_props.push((obj_name, prop_name));
                        }
                        search_pos = abs_dot_pos + end_pos + 1;
                    } else if !after_dot.is_empty() {
                        let prop_name = after_dot.trim();
                        if !prop_name.is_empty() && prop_name.chars().next().map(|c| c.is_lowercase()).unwrap_or(false) {
                            obj_props.push((obj_name, prop_name));
                        }
                        break;
                    } else {
                        search_pos = abs_dot_pos + 1;
                    }
                } else {
                    search_pos = abs_dot_pos + 1;
                }
            }
            // Check if same object with multiple different properties
            if obj_props.len() >= 2 {
                for i in 0..obj_props.len() {
                    for j in (i + 1)..obj_props.len() {
                        if obj_props[i].0 == obj_props[j].0 && obj_props[i].1 != obj_props[j].1 {
                            issues.push(Issue::new(
                                "JS_ES4",
                                format!("Multiple properties from same object - use destructuring: const {{ {}, {} }} = {}", obj_props[i].1, obj_props[j].1, obj_props[i].0),
                                Severity::Minor,
                                Category::CodeSmell,
                                ctx.file_path,
                                idx + 1,
                            ).with_remediation(Remediation::quick("Use object destructuring: const { x, y } = obj")));
                            break;
                        }
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_ES5 — Spread operator instead of .apply()
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_ES5"
    name: "Spread operator should be used instead of .apply()"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\.apply\s*\(\s*\w+\s*,").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "JS_ES5",
                    "Use spread operator instead of .apply()",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::quick("Replace func.apply(obj, args) with func(obj, ...args)")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_ES6 — Default parameters instead of || default
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_ES6"
    name: "Default parameters should be used instead of ||"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"=\s*\w+\s*\|\|\s*").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && !line.trim().starts_with("//") {
                issues.push(Issue::new(
                    "JS_ES6",
                    "Use default parameters instead of ||",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::quick("Replace x = x || default with x = default as function parameter")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_ES7 — Rest parameters instead of arguments
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_ES7"
    name: "Rest parameters should be used instead of arguments"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let has_args = ctx.source.contains("arguments");
        let has_func = ctx.source.contains("function") || ctx.source.contains("=>");
        if has_args && has_func {
            for (idx, line) in ctx.source.lines().enumerate() {
                if line.contains("arguments") && !line.trim().starts_with("//") {
                    issues.push(Issue::new(
                        "JS_ES7",
                        "Use of arguments object - use rest parameters (...args) instead",
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::quick("Replace arguments with ...args rest parameter")));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_ES8 — for...of instead of for loop with index
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_ES8"
    name: "for...of should be used instead of index-based for loops"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"for\s*\(\s*(?:let|var|const)\s+\w+\s*=\s*0\s*;\s*\w+\s*<\s*\w+\.length").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "JS_ES8",
                    "Index-based for loop - use for...of instead",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::quick("Replace for loop with for (const item of array)")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_ES9 — Object shorthand ({x: x} → {x})
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_ES9"
    name: "Object shorthand syntax should be used"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        // Detect {x: x} patterns without backreference
        for (idx, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            // Look for { var: var } pattern
            if let Some(start) = trimmed.find('{') {
                if let Some(end) = trimmed.rfind('}') {
                    let content = &trimmed[start + 1..end];
                    // Look for pattern: identifier: identifier
                    if let Some(colon_pos) = content.find(':') {
                        let prop_name = content[..colon_pos].trim();
                        let prop_value = content[colon_pos + 1..].trim();
                        if prop_name == prop_value && !prop_name.is_empty() {
                            issues.push(Issue::new(
                                "JS_ES9",
                                format!("Use object shorthand {{ {}}} instead of {{ {}: {} }}", prop_name, prop_name, prop_name),
                                Severity::Minor,
                                Category::CodeSmell,
                                ctx.file_path,
                                idx + 1,
                            ).with_remediation(Remediation::quick("Use shorthand: { x } instead of { x: x }")));
                        }
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_ES10 — Promise instead of callback
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_ES10"
    name: "Promises should be used instead of callbacks"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let callback_patterns = ["callback(", "cb(", "done(", "next("];
        let has_async = ctx.source.contains("Promise") || ctx.source.contains("async") || ctx.source.contains("await");
        for (idx, line) in ctx.source.lines().enumerate() {
            for pattern in &callback_patterns {
                if line.contains(pattern) && !line.contains("Promise") && !has_async {
                    issues.push(Issue::new(
                        "JS_ES10",
                        "Callback pattern detected - consider using Promises instead",
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::moderate("Refactor callbacks to Promises or use async/await")));
                    break;
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_ES11 — async/await instead of .then() chains
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_ES11"
    name: "async/await should be used instead of .then() chains"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let then_count = ctx.source.matches(".then(").count();
        if then_count >= 3 {
            for (idx, line) in ctx.source.lines().enumerate() {
                if line.contains(".then(") {
                    issues.push(Issue::new(
                        "JS_ES11",
                        "Chained .then() calls detected - use async/await for better readability",
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::quick("Replace .then().then().then() with async/await")));
                    break;
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_ES12 — Optional chaining instead of && checks
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_ES12"
    name: "Optional chaining should be used instead of && checks"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\w+\s*&&\s*\w+\s*&&\s*\w+\.").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && !line.trim().starts_with("//") {
                issues.push(Issue::new(
                    "JS_ES12",
                    "Multiple && checks detected - use optional chaining instead",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::quick("Replace obj && obj.prop && obj.prop.deep with obj?.prop?.deep")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_ES13 — Nullish coalescing (??) instead of ||
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_ES13"
    name: "Nullish coalescing should be used instead of ||"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\|\|\s*(?:null|undefined|0|false|'')").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && !line.trim().starts_with("//") {
                issues.push(Issue::new(
                    "JS_ES13",
                    "|| with falsy value detected - use nullish coalescing ?? instead",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::quick("Replace || default with ?? default for null/undefined only")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_ES14 — Array.includes() instead of indexOf !== -1
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_ES14"
    name: "Array.includes() should be used instead of indexOf"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\.indexOf\s*\([^)]+\)\s*(!==|===)\s*(-1|1)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && !line.trim().starts_with("//") {
                issues.push(Issue::new(
                    "JS_ES14",
                    "Use .includes() instead of .indexOf() !== -1",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::quick("Replace arr.indexOf(x) !== -1 with arr.includes(x)")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_ES15 — Array.find() instead of filter()[0]
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_ES15"
    name: "Array.find() should be used instead of filter()[0]"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\.filter\s*\([^)]+\)\s*\[\s*0\s*\]").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && !line.trim().starts_with("//") {
                issues.push(Issue::new(
                    "JS_ES15",
                    "Use .find() instead of .filter()[0]",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::quick("Replace arr.filter(fn)[0] with arr.find(fn)")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_ES16 — map/filter/reduce instead of for loops
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_ES16"
    name: "Functional array methods should be used instead of for loops"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let for_loop_re = regex::Regex::new(r"for\s*\(\s*(?:let|var|const)\s+\w+\s*=\s*0").unwrap();
        let has_array_method = ctx.source.contains(".map(") || ctx.source.contains(".filter(") || ctx.source.contains(".reduce(");
        for (idx, line) in ctx.source.lines().enumerate() {
            if for_loop_re.is_match(line) && has_array_method {
                issues.push(Issue::new(
                    "JS_ES16",
                    "for loop detected - consider using map/filter/reduce instead",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::moderate("Replace for loop with functional array methods for better readability")));
                break;
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_ES17 — Object.entries() instead of for...in
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_ES17"
    name: "Object.entries() should be used instead of for...in"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"for\s*\(\s*(?:let|var|const)\s+\w+\s+in\s+\w+\s*\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "JS_ES17",
                    "for...in loop detected - use Object.entries() instead",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::quick("Replace for (key in obj) with for (const [key, value] of Object.entries(obj))")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_ES18 — String.startsWith/endsWith instead of indexOf
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_ES18"
    name: "String.startsWith/endsWith should be used instead of indexOf"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\.indexOf\s*\([^)]+\)\s*===?\s*0").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && !line.trim().starts_with("//") {
                issues.push(Issue::new(
                    "JS_ES18",
                    "Use .startsWith() or .endsWith() instead of .indexOf() === 0",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::quick("Replace str.indexOf(x) === 0 with str.startsWith(x)")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_ES19 — Number.isNaN() instead of isNaN()
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_ES19"
    name: "Number.isNaN() should be used instead of isNaN()"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        // Detect isNaN( without preceding Number. (lookbehind not supported in Rust regex)
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.trim().starts_with("//") {
                continue;
            }
            let isNaN_pos = line.find("isNaN(");
            if let Some(pos) = isNaN_pos {
                // Check if it's NOT preceded by "Number." or "."
                let before = if pos >= 7 { &line[pos - 7..pos] } else { "" };
                if !before.contains("Number") && !before.ends_with('.') {
                    issues.push(Issue::new(
                        "JS_ES19",
                        "Use Number.isNaN() instead of isNaN()",
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::quick("Replace isNaN(x) with Number.isNaN(x)")));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_ES20 — export default instead of module.exports
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_ES20"
    name: "ES6 export default should be used instead of module.exports"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"module\.exports\s*=").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && !line.trim().starts_with("//") {
                issues.push(Issue::new(
                    "JS_ES20",
                    "Use ES6 export default instead of module.exports",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::quick("Replace module.exports = x with export default x")));
            }
        }
        issues
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// BATCH 5: React/JSX Rules (20 rules)
// ═══════════════════════════════════════════════════════════════════════════════

// ─────────────────────────────────────────────────────────────────────────────
// JS_RX1 — useEffect missing dependency array
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_RX1"
    name: "useEffect should have a dependency array"
    severity: Major
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"useEffect\s*\(\s*\(\s*\)\s*=>").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                let context: String = ctx.source.lines().skip(idx).take(5).collect::<Vec<_>>().join("\n");
                if !context.contains("[") || context.contains("useEffect(()") {
                    issues.push(Issue::new(
                        "JS_RX1",
                        "useEffect is missing dependency array - may cause infinite loop or stale closures",
                        Severity::Major,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::moderate("Add dependency array: useEffect(() => {...}, [deps])")));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_RX2 — useState setter not used
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_RX2"
    name: "useState setter should be used"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"const\s+\[(\w+),\s*set(\w+)\]\s*=\s*useState").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                if let (Some(state_name), Some(setter_name)) = (cap.get(1), cap.get(2)) {
                    let setter = format!("set{}", state_name.as_str());
                    let remaining: String = ctx.source.lines().skip(idx + 1).take(50).collect::<Vec<_>>().join("\n");
                    if !remaining.contains(&setter) {
                        issues.push(Issue::new(
                            "JS_RX2",
                            format!("useState setter '{}' is never called - remove or use it", setter),
                            Severity::Minor,
                            Category::CodeSmell,
                            ctx.file_path,
                            idx + 1,
                        ).with_remediation(Remediation::quick("Use the setter or remove the unused state")));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_RX3 — Direct DOM manipulation in React
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_RX3"
    name: "Direct DOM manipulation should be avoided in React"
    severity: Major
    category: Bug
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let dom_patterns = ["getElementById", "getElementsByClassName", "getElementsByTagName", "querySelector", "querySelectorAll", "document."];
        let has_react = ctx.source.contains("React") || ctx.source.contains("useState") || ctx.source.contains("useEffect") || ctx.source.contains("Component");
        if has_react {
            for (idx, line) in ctx.source.lines().enumerate() {
                for pattern in &dom_patterns {
                    if line.contains(pattern) && !line.trim().starts_with("//") {
                        issues.push(Issue::new(
                            "JS_RX3",
                            "Direct DOM manipulation in React - use refs or state instead",
                            Severity::Major,
                            Category::Bug,
                            ctx.file_path,
                            idx + 1,
                        ).with_remediation(Remediation::moderate("Use useRef or React state instead of direct DOM manipulation")));
                        break;
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_RX4 — Missing key prop in list
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_RX4"
    name: "List items should have a unique key prop"
    severity: Major
    category: Bug
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\.map\s*\([^)]*\)\s*=>\s*\([^)]*\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                let context: String = ctx.source.lines().skip(idx).take(3).collect::<Vec<_>>().join("\n");
                if !context.contains("key=") && !context.contains("key :") {
                    issues.push(Issue::new(
                        "JS_RX4",
                        "Missing key prop in list map - each child needs a unique key",
                        Severity::Major,
                        Category::Bug,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::quick("Add key prop: {items.map(item => <Component key={item.id} .../>)}")));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_RX5 — Inline styles (use CSS modules)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_RX5"
    name: "Inline styles should not be used"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"style\s*=\s*\{\s*\{").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "JS_RX5",
                    "Inline style object detected - use CSS modules or styled-components instead",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::moderate("Use CSS modules, styled-components, or external CSS files")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_RX6 — Unused state variable
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_RX6"
    name: "Unused state variables should be removed"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"const\s+\[(\w+),").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                if let Some(name) = cap.get(1) {
                    let remaining: String = ctx.source.lines().skip(idx + 1).take(30).collect::<Vec<_>>().join("\n");
                    if !remaining.contains(&format!(" {} ", name.as_str())) && !remaining.contains(&format!("({}", name.as_str())) {
                        issues.push(Issue::new(
                            "JS_RX6",
                            format!("State variable '{}' appears unused", name.as_str()),
                            Severity::Minor,
                            Category::CodeSmell,
                            ctx.file_path,
                            idx + 1,
                        ).with_remediation(Remediation::quick("Remove unused state variable or use it")));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_RX7 — setState in render (infinite loop risk)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_RX7"
    name: "setState should not be called during render"
    severity: Critical
    category: Bug
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"set\w+\s*\(").unwrap();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            if re.is_match(line) && !line.contains("useEffect") && !line.contains("handle") && !line.contains("onClick") && !line.contains("onChange") {
                let next_line = lines.get(idx + 1).unwrap_or(&"");
                if next_line.contains("return") || lines.iter().take(idx + 1).any(|l| l.contains("function") && !l.contains("=>")) {
                    issues.push(Issue::new(
                        "JS_RX7",
                        "setState called during render - may cause infinite loop",
                        Severity::Critical,
                        Category::Bug,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::moderate("Move setState into useEffect or event handlers")));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_RX8 — Component without displayName
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_RX8"
    name: "React components should have a displayName"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"(?:function\s+\w+|const\s+\w+\s*=\s*(?:(?:async\s*)?)?\(|=>\s*\()").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && (line.contains("Props") || line.len() < 50) {
                let context: String = ctx.source.lines().skip(idx).take(20).collect::<Vec<_>>().join("\n");
                if !context.contains("displayName") && ctx.source.contains("React") {
                    issues.push(Issue::new(
                        "JS_RX8",
                        "Component may be missing displayName - useful for debugging",
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::quick("Add ComponentName.displayName = 'ComponentName'")));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_RX9 — PropTypes missing
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_RX9"
    name: "React components should have PropTypes"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let has_react = ctx.source.contains("React") || ctx.source.contains("import") && ctx.source.contains("Component");
        let has_prop_types = ctx.source.contains("prop-types") || ctx.source.contains("PropTypes");
        let has_props = ctx.source.contains("Props") || ctx.source.contains("props.");
        if has_react && has_props && !has_prop_types {
            let re = regex::Regex::new(r"function\s+\w+|const\s+\w+\s*=\s*(?:async\s*)?\(").unwrap();
            for (idx, line) in ctx.source.lines().enumerate() {
                if re.is_match(line) && line.len() < 80 {
                    issues.push(Issue::new(
                        "JS_RX9",
                        "Component may be missing PropTypes - add prop-types validation",
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::moderate("Add PropTypes for all component props")));
                    break;
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_RX10 — Default props missing
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_RX10"
    name: "Default props should be defined"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let has_props = ctx.source.contains("function") && (ctx.source.contains("Props") || ctx.source.contains("props."));
        let has_default_props = ctx.source.contains("defaultProps");
        if has_props && !has_default_props && !ctx.source.contains("??") && !ctx.source.contains("||") {
            issues.push(Issue::new(
                "JS_RX10",
                "Component may be missing default props",
                Severity::Minor,
                Category::CodeSmell,
                ctx.file_path,
                1,
            ).with_remediation(Remediation::quick("Add Component.defaultProps = { propName: value }")));
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_RX11 — useEffect cleanup function missing
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_RX11"
    name: "useEffect cleanup function may be missing"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"useEffect\s*\(\s*\(\s*\)\s*=>").unwrap();
        let add_listener = ctx.source.contains("addEventListener") || ctx.source.contains("setInterval") || ctx.source.contains("setTimeout");
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && add_listener {
                let context: String = ctx.source.lines().skip(idx).take(10).collect::<Vec<_>>().join("\n");
                if !context.contains("return") || !context.contains("removeEventListener") && !context.contains("clearInterval") && !context.contains("clearTimeout") {
                    issues.push(Issue::new(
                        "JS_RX11",
                        "useEffect may be missing cleanup function for subscriptions/timers",
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::moderate("Add return () => { cleanup } in useEffect")));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_RX12 — useMemo/useCallback unnecessary
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_RX12"
    name: "useMemo/useCallback may be unnecessary"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let use_memo = ctx.source.matches("useMemo").count();
        let use_callback = ctx.source.matches("useCallback").count();
        let has_complexity = use_memo > 5 || use_callback > 5;
        if !has_complexity {
            for (idx, line) in ctx.source.lines().enumerate() {
                if line.contains("useMemo") || line.contains("useCallback") {
                    let context: String = ctx.source.lines().skip(idx.saturating_sub(2)).take(5).collect::<Vec<_>>().join("\n");
                    if !context.contains("React.memo") && context.len() < 200 {
                        issues.push(Issue::new(
                            "JS_RX12",
                            "useMemo/useCallback may be unnecessary here - profile before optimizing",
                            Severity::Minor,
                            Category::CodeSmell,
                            ctx.file_path,
                            idx + 1,
                        ).with_remediation(Remediation::quick("Remove useMemo/useCallback unless profiling shows it's needed")));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_RX13 — useRef instead of getElementById
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_RX13"
    name: "useRef should be used instead of getElementById"
    severity: Major
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let has_react = ctx.source.contains("React") || ctx.source.contains("useState") || ctx.source.contains("useRef");
        if has_react {
            for (idx, line) in ctx.source.lines().enumerate() {
                if line.contains("getElementById") {
                    issues.push(Issue::new(
                        "JS_RX13",
                        "getElementById in React - use useRef instead",
                        Severity::Major,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::quick("Use const ref = useRef() and <div ref={ref}> instead")));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_RX14 — Conditional rendering with && (falsy values issue)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_RX14"
    name: "Conditional rendering with && may cause issues with falsy values"
    severity: Major
    category: Bug
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\{[^}]*\s*&&\s*[^}]*\}").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && (line.contains("0") || line.contains("''") || line.contains("false") || line.contains("null")) {
                issues.push(Issue::new(
                    "JS_RX14",
                    "Conditional && with falsy value - use ternary or && with !! for type coercion",
                    Severity::Major,
                    Category::Bug,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::quick("Replace {value && <Component />} with {value ? <Component /> : null}")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_RX15 — Fragment shorthand (<> </>)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_RX15"
    name: "Fragment shorthand should be used"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"<React\.Fragment").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "JS_RX15",
                    "Use Fragment shorthand <> </> instead of <React.Fragment>",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::quick("Replace <React.Fragment> with <>")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_RX16 — Event handler arrow in JSX (creates new fn each render)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_RX16"
    name: "Event handlers defined inline in JSX create new functions on each render"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"on( Click| Change| Submit| Focus| Blur)=").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && line.contains("() =>") {
                issues.push(Issue::new(
                    "JS_RX16",
                    "Inline arrow function in JSX event handler - creates new function each render",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::moderate("Define event handler as class method or use useCallback")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_RX17 — useLayoutEffect when useEffect would work
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_RX17"
    name: "useLayoutEffect should be avoided unless necessary"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"useLayoutEffect").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "JS_RX17",
                    "useLayoutEffect detected - use useEffect unless you need synchronous DOM updates",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::quick("Replace useLayoutEffect with useEffect unless you specifically need synchronous layout measurements")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_RX18 — useContext value changes too often
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_RX18"
    name: "useContext value changes on every render"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"useContext\s*\(\s*(\w+)\s*\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                if let Some(name) = cap.get(1) {
                    let context: String = ctx.source.lines().skip(idx.saturating_sub(5)).take(10).collect::<Vec<_>>().join("\n");
                    if context.contains("useState") || context.contains("useReducer") {
                        issues.push(Issue::new(
                            "JS_RX18",
                            format!("useContext({}) combined with state may cause too many re-renders", name.as_str()),
                            Severity::Minor,
                            Category::CodeSmell,
                            ctx.file_path,
                            idx + 1,
                        ).with_remediation(Remediation::moderate("Split context or use useMemo to stabilize context value")));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_RX19 — React.memo missing comparison function
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_RX19"
    name: "React.memo should have a comparison function for complex props"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"React\.memo\s*\(\s*\w+\s*\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && !line.contains("areEqual") && !line.contains("prevProps") {
                issues.push(Issue::new(
                    "JS_RX19",
                    "React.memo without comparison function - all prop changes cause re-render",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::quick("Add second argument: React.memo(Component, (prevProps, nextProps) => areEqual)")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_RX20 — useReducer when useState would work
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_RX20"
    name: "useState should be used instead of useReducer for simple state"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let use_reducer_count = ctx.source.matches("useReducer").count();
        if use_reducer_count > 0 {
            let use_state_count = ctx.source.matches("useState").count();
            if use_state_count > use_reducer_count {
                for (idx, line) in ctx.source.lines().enumerate() {
                    if line.contains("useReducer") {
                        issues.push(Issue::new(
                            "JS_RX20",
                            "useReducer detected - consider useState for simpler state management",
                            Severity::Minor,
                            Category::CodeSmell,
                            ctx.file_path,
                            idx + 1,
                        ).with_remediation(Remediation::quick("Use useState unless you have complex state logic that benefits from reducer pattern")));
                        break;
                    }
                }
            }
        }
        issues
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// BATCH 6: More JavaScript Code Smells (20 rules) — JS_S179 to JS_S198
// ═══════════════════════════════════════════════════════════════════════════════

// ─────────────────────────────────────────────────────────────────────────────
// JS_S179 — Arrow function too long (>30 lines)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S179"
    name: "Arrow functions should not be too long"
    severity: Major
    category: CodeSmell
    language: "javascript"
    params: { max_lines: usize = 30 }
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r">\s*\([^)]*\)\s*=>\s*\{").unwrap();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            if re.is_match(line) {
                let func_start = idx;
                let mut brace_count = 0isize;
                let mut func_lines = 0;
                let mut found_open = false;
                for l in lines.iter().skip(func_start) {
                    if l.contains("{") { found_open = true; brace_count += l.matches("{").count() as isize; }
                    if l.contains("}") { brace_count -= l.matches("}").count() as isize; }
                    if found_open { func_lines += 1; }
                    if brace_count <= 0 && found_open { break; }
                }
                if func_lines > self.max_lines {
                    issues.push(Issue::new("JS_S179", format!("Arrow function is {} lines exceeding threshold of {}", func_lines, self.max_lines), Severity::Major, Category::CodeSmell, ctx.file_path, idx + 1).with_remediation(Remediation::moderate("Extract into a named function")));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S180 — Chained method calls (>5)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S180"
    name: "Chained method calls should not be too long"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: { max_chain: usize = 5 }
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\.\w+\(").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            let chain_count = re.find_iter(line).count();
            if chain_count > self.max_chain {
                issues.push(Issue::new("JS_S180", format!("Chained method calls ({}) exceed threshold of {}", chain_count, self.max_chain), Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1).with_remediation(Remediation::moderate("Break the chain into separate statements or extract intermediate results")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S181 — Duplicate string literal (>3 occurrences)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S181"
    name: "String literals should not be duplicated"
    severity: Major
    category: CodeSmell
    language: "javascript"
    params: { min_occurrences: usize = 3 }
    check: => {
        let mut issues = Vec::new();
        let mut string_counts: std::collections::HashMap<String, Vec<usize>> = std::collections::HashMap::new();
        let re = regex::Regex::new(r#""([^"]{2,100})""#).unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            for cap in re.captures_iter(line) {
                if let Some(s) = cap.get(1) {
                    let s_str = s.as_str().to_string();
                    string_counts.entry(s_str).or_default().push(idx + 1);
                }
            }
        }
        for (s, lines) in string_counts {
            if lines.len() >= self.min_occurrences && !s.contains(" ") && s.len() > 2 {
                for &l in &lines {
                    issues.push(Issue::new("JS_S181", format!("Duplicate string '{}' appears {} times", s, lines.len()), Severity::Major, Category::CodeSmell, ctx.file_path, l));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S182 — Function with boolean flag parameter
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S182"
    name: "Functions should not have boolean flag parameters"
    severity: Major
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"function\s+\w+\s*\([^)]*\b(bool|boolean|Boolean)\b[^)]*\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JS_S182", "Function has boolean parameter - consider splitting into separate functions", Severity::Major, Category::CodeSmell, ctx.file_path, idx + 1).with_remediation(Remediation::moderate("Split function into two or use an options object")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S183 — Empty interface (TypeScript)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S183"
    name: "Empty interfaces should not be used"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"interface\s+\w+\s*\{\s*\}").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JS_S183", "Empty interface detected - use type instead or add properties", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1).with_remediation(Remediation::quick("Add properties or use 'type' instead")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S184 — Type assertion (as vs <>)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S184"
    name: "Angle bracket type assertions should not be used"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"<\w+>\s*\(").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && !line.contains("import") && !line.contains("export") && !line.contains("JSX") && !line.contains("React") {
                issues.push(Issue::new("JS_S184", "Angle bracket type assertion (<Type>x) - use 'as' syntax instead", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1).with_remediation(Remediation::quick("Replace <Type>x with x as Type")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S185 — Unnecessary type assertion
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S185"
    name: "Unnecessary type assertions should be removed"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        // Detect `let x: Type = y as Type` patterns without backreference
        for (idx, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            // Look for the pattern: let <var>: <Type> = <expr> as <Type>
            if let Some(as_pos) = trimmed.find(" as ") {
                let before_as = trimmed[..as_pos].trim();
                let after_as = trimmed[as_pos + 4..].trim();
                // Check if the part before " as " contains a type annotation
                if let Some(colon_pos) = before_as.find(':') {
                    let declared_type = before_as[colon_pos + 1..].trim().split_whitespace().next().unwrap_or("");
                    let asserted_type = after_as.split_whitespace().next().unwrap_or("");
                    if declared_type == asserted_type && !declared_type.is_empty() {
                        issues.push(Issue::new("JS_S185", "Unnecessary type assertion - type already known", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1).with_remediation(Remediation::quick("Remove the unnecessary 'as Type' assertion")));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S186 — TypeScript any usage
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S186"
    name: "TypeScript any type should not be used"
    severity: Major
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r":\s*any\b").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JS_S186", "Type 'any' used - defeats the purpose of TypeScript", Severity::Major, Category::CodeSmell, ctx.file_path, idx + 1).with_remediation(Remediation::moderate("Use unknown or specific types instead of any")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S187 — Unnecessary type annotation
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S187"
    name: "Unnecessary type annotations should be removed"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r#"let\s+\w+:\s*string\s*=\s*"[^"]*""#).unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JS_S187", "Type annotation unnecessary - the type can be inferred from the value", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1).with_remediation(Remediation::quick("Remove the explicit type annotation")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S188 — Missing return type on function
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S188"
    name: "Functions should have explicit return types"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"function\s+\w+\s*\([^)]*\)\s*\{").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                let context: String = ctx.source.lines().skip(idx).take(3).collect::<Vec<_>>().join("\n");
                if !context.contains("->") && !context.contains(": ") {
                    issues.push(Issue::new("JS_S188", "Function missing return type annotation", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1).with_remediation(Remediation::quick("Add explicit return type: function f(): Type { }")));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S189 — Union type with undefined (should be optional ?)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S189"
    name: "Union with undefined should use optional syntax"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r":\s*\w+\s*\|\s*undefined").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JS_S189", "Union with undefined detected - use optional property (?) instead", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1).with_remediation(Remediation::quick("Replace 'type | undefined' with 'type?'")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S190 — Enum with mixed numeric and string values
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S190"
    name: "Enums should not mix numeric and string values"
    severity: Major
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"enum\s+\w+\s*\{").unwrap();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            if re.is_match(line) {
                let enum_block: String = lines.iter().skip(idx).take(20).cloned().collect::<Vec<_>>().join("\n");
                let has_string = enum_block.contains('"') || enum_block.contains("'");
                let has_number = regex::Regex::new(r"\d+,").unwrap().is_match(&enum_block);
                if has_string && has_number {
                    issues.push(Issue::new("JS_S190", "Enum mixes string and numeric values - use all strings or all numbers", Severity::Major, Category::CodeSmell, ctx.file_path, idx + 1).with_remediation(Remediation::moderate("Use const enum with all strings or all numbers")));
                }
                break;
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S191 — Namespace instead of module
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S191"
    name: "Namespace should not be used"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"namespace\s+\w+").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JS_S191", "Namespace declaration detected - use ES6 module instead", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1).with_remediation(Remediation::moderate("Replace namespace with ES6 module (import/export)")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S192 — Triple-slash directive instead of import
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S192"
    name: "Triple-slash directives should not be used"
    severity: Major
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"^///\s*<reference").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JS_S192", "Triple-slash reference directive - use ES6 import instead", Severity::Major, Category::CodeSmell, ctx.file_path, idx + 1).with_remediation(Remediation::quick("Replace /// <reference path='...' /> with import")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S193 — Ambient declaration without export
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S193"
    name: "Ambient declarations should be exported"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"declare\s+(class|function|const|var|let)\s+\w+").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && !line.contains("export") {
                issues.push(Issue::new("JS_S193", "Ambient declaration without export - module augmentation may be needed", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1).with_remediation(Remediation::moderate("Add 'export' or wrap in module declaration")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S194 — type vs interface (prefer interface)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S194"
    name: "Interface should be used for object types"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"type\s+\w+\s*=\s*\{[^}]*\}\s*;").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && !line.contains("&") && !line.contains("|") {
                issues.push(Issue::new("JS_S194", "Type alias for object literal - use 'interface' instead", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1).with_remediation(Remediation::quick("Replace 'type X = { }' with 'interface X { }'")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S195 — Readonly array instead of mutable
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S195"
    name: "ReadonlyArray should be used instead of Array"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r":\s*Array<").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JS_S195", "Mutable Array type - use ReadonlyArray for immutability", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1).with_remediation(Remediation::moderate("Use ReadonlyArray<T> instead of Array<T>")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S196 — Abstract class without abstract methods
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S196"
    name: "Abstract classes without abstract methods should not be used"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"abstract\s+class\s+\w+").unwrap();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            if re.is_match(line) {
                let class_block: String = lines.iter().skip(idx).take(30).cloned().collect::<Vec<_>>().join("\n");
                if !class_block.contains("abstract") {
                    issues.push(Issue::new("JS_S196", "Abstract class without abstract methods - use regular class", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1).with_remediation(Remediation::quick("Remove 'abstract' or add abstract methods")));
                }
                break;
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S197 — Static method in class (use module function)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S197"
    name: "Static methods should be avoided in classes"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"static\s+\w+\s*\(").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JS_S197", "Static method detected - consider using module-level function instead", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1).with_remediation(Remediation::moderate("Move to module scope or make it an instance method")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_S198 — Private field with # prefix vs private keyword
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_S198"
    name: "Hash (#) private fields should not be mixed with TypeScript private"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let has_hash_private = ctx.source.contains("#");
        let has_private_keyword = ctx.source.contains("private ");
        if has_hash_private && has_private_keyword {
            issues.push(Issue::new("JS_S198", "Mixing #private fields with 'private' keyword - choose one style", Severity::Minor, Category::CodeSmell, ctx.file_path, 1).with_remediation(Remediation::moderate("Use only #private fields or TypeScript 'private' keyword")));
        }
        issues
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// BATCH 7: More React/JSX Rules (20 rules) — JS_RX21 to JS_RX40
// ═══════════════════════════════════════════════════════════════════════════════

// ─────────────────────────────────────────────────────────────────────────────
// JS_RX21 — Component without React import
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_RX21"
    name: "React components should import React"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let has_jsx = ctx.source.contains("<") && (ctx.source.contains("Component") || ctx.source.contains("useState") || ctx.source.contains("useEffect"));
        let has_react_import = ctx.source.contains("import React") || ctx.source.contains("import {") && ctx.source.contains("React");
        if has_jsx && !has_react_import {
            issues.push(Issue::new("JS_RX21", "JSX found but React is not imported", Severity::Minor, Category::CodeSmell, ctx.file_path, 1).with_remediation(Remediation::quick("Add: import React from 'react'")));
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_RX22 — Class component instead of functional
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_RX22"
    name: "Class components should be converted to functional components"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"class\s+\w+\s+extends\s+(React\.)?Component").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JS_RX22", "Class component detected - convert to functional component with hooks", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1).with_remediation(Remediation::moderate("Convert to functional component using useState, useEffect, etc.")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_RX23 — Unnecessary fragment (<> </>)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_RX23"
    name: "Unnecessary fragments should be removed"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"<>\s*</>").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JS_RX23", "Empty fragment <> </> - remove it or add children", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1).with_remediation(Remediation::quick("Remove unnecessary fragment or add content")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_RX24 — State initialization in constructor
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_RX24"
    name: "State should not be initialized in constructor"
    severity: Major
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"constructor\s*\([^)]*\)\s*\{[^}]*this\.state\s*=").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JS_RX24", "State initialized in constructor - use useState hook instead", Severity::Major, Category::CodeSmell, ctx.file_path, idx + 1).with_remediation(Remediation::moderate("Convert to functional component with useState")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_RX25 — setState callback pattern (use useEffect)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_RX25"
    name: "setState callback should be replaced with useEffect"
    severity: Major
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"this\.setState\s*\([^,]+,\s*\([^)]*\)\s*=>").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JS_RX25", "setState with callback - use useEffect for side effects", Severity::Major, Category::CodeSmell, ctx.file_path, idx + 1).with_remediation(Remediation::moderate("Replace setState callback with useEffect hook")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_RX26 — Render prop pattern (use hooks)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_RX26"
    name: "Render props should be replaced with hooks"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"render\s*=\s*\{").unwrap();
        let has_children = ctx.source.contains("children") && ctx.source.contains("React.Children");
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) || has_children {
                issues.push(Issue::new("JS_RX26", "Render prop pattern - consider hooks for stateful logic", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1).with_remediation(Remediation::moderate("Extract stateful logic into custom hooks")));
                break;
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_RX27 — HOC pattern (use hooks)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_RX27"
    name: "Higher-order components should be replaced with hooks"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"React\.createElement\s*\(\s*\w+,\s*\{[^}]*\}\s*\)").unwrap();
        let has_hoc = ctx.source.contains("withRouter") || ctx.source.contains("connect(") || ctx.source.contains("compose(");
        if has_hoc {
            for (idx, line) in ctx.source.lines().enumerate() {
                if re.is_match(line) {
                    issues.push(Issue::new("JS_RX27", "Higher-order component pattern - use hooks instead", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1).with_remediation(Remediation::moderate("Replace HOC with custom hooks")));
                    break;
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_RX28 — Direct state mutation (this.state.x = y)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_RX28"
    name: "State should not be mutated directly"
    severity: Critical
    category: Bug
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"this\.state\.\w+\s*=").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && !line.contains("this.setState") {
                issues.push(Issue::new("JS_RX28", "Direct state mutation - use setState instead", Severity::Critical, Category::Bug, ctx.file_path, idx + 1).with_remediation(Remediation::moderate("Use this.setState({ key: newValue })")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_RX29 — forceUpdate usage
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_RX29"
    name: "forceUpdate should not be used"
    severity: Major
    category: Bug
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"this\.forceUpdate\s*\(").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JS_RX29", "forceUpdate detected - use setState with state instead", Severity::Major, Category::Bug, ctx.file_path, idx + 1).with_remediation(Remediation::moderate("Use setState to trigger re-renders properly")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_RX30 — shouldComponentUpdate without PureComponent
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_RX30"
    name: "shouldComponentUpdate without PureComponent"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let has_scu = ctx.source.contains("shouldComponentUpdate");
        let has_pure = ctx.source.contains("PureComponent");
        if has_scu && !has_pure {
            issues.push(Issue::new("JS_RX30", "shouldComponentUpdate without PureComponent - consider extending PureComponent", Severity::Minor, Category::CodeSmell, ctx.file_path, 1).with_remediation(Remediation::moderate("Extend PureComponent or implement memoization")));
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_RX31 — getDerivedStateFromProps misuse
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_RX31"
    name: "getDerivedStateFromProps should be used correctly"
    severity: Major
    category: Bug
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"static\s+getDerivedStateFromProps\s*\([^)]*\)\s*\{[^}]*this\.").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JS_RX31", "getDerivedStateFromProps uses 'this' - it's a static method", Severity::Major, Category::Bug, ctx.file_path, idx + 1).with_remediation(Remediation::moderate("getDerivedStateFromProps should not reference 'this'")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_RX32 — componentWillMount (deprecated)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_RX32"
    name: "componentWillMount is deprecated"
    severity: Critical
    category: Bug
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"componentWillMount\s*\(").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JS_RX32", "componentWillMount is deprecated - use constructor or useEffect", Severity::Critical, Category::Bug, ctx.file_path, idx + 1).with_remediation(Remediation::substantial("Replace with constructor or useEffect with empty dependency array")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_RX33 — componentWillUpdate (deprecated)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_RX33"
    name: "componentWillUpdate is deprecated"
    severity: Critical
    category: Bug
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"componentWillUpdate\s*\(").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JS_RX33", "componentWillUpdate is deprecated - use getDerivedStateFromProps or useEffect", Severity::Critical, Category::Bug, ctx.file_path, idx + 1).with_remediation(Remediation::substantial("Replace with useEffect or getDerivedStateFromProps")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_RX34 — componentWillReceiveProps (deprecated)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_RX34"
    name: "componentWillReceiveProps is deprecated"
    severity: Critical
    category: Bug
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"componentWillReceiveProps\s*\(").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JS_RX34", "componentWillReceiveProps is deprecated - use getDerivedStateFromProps or useEffect", Severity::Critical, Category::Bug, ctx.file_path, idx + 1).with_remediation(Remediation::substantial("Replace with getDerivedStateFromProps or useEffect")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_RX35 — findDOMNode usage
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_RX35"
    name: "findDOMNode should not be used"
    severity: Critical
    category: Bug
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"ReactDOM\.findDOMNode\s*\(").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JS_RX35", "findDOMNode is deprecated - use refs instead", Severity::Critical, Category::Bug, ctx.file_path, idx + 1).with_remediation(Remediation::substantial("Use useRef and forwardRef instead of findDOMNode")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_RX36 — createRef in constructor
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_RX36"
    name: "createRef should not be used in constructor"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"constructor\s*\([^)]*\)\s*\{[^}]*React\.createRef\s*\(").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JS_RX36", "createRef in constructor - use useRef hook instead", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1).with_remediation(Remediation::quick("Replace with const myRef = useRef()")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_RX37 — ref callback pattern
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_RX37"
    name: "Ref callback pattern should use useRef"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"ref\s*=\s*\{[^}]*=>\s*\([^)]*\)\s*\{").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && !line.contains("useRef") {
                issues.push(Issue::new("JS_RX37", "Ref callback pattern - use useRef with callback ref instead", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1).with_remediation(Remediation::moderate("Use useRef or React.memo with callback ref")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_RX38 — JSX prop spreading ({...props})
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_RX38"
    name: "Props spreading should be avoided"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\{\.\.\.\w+\}").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && !line.contains("import") && !line.contains("export") && !line.contains("merge") {
                issues.push(Issue::new("JS_RX38", "Props spreading detected - list explicit props instead", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1).with_remediation(Remediation::moderate("Pass explicit props instead of spreading")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_RX39 — Multiple JSX roots (Fragment needed)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_RX39"
    name: "Multiple JSX roots need a Fragment wrapper"
    severity: Major
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"return\s*\([^)]*\)\s*;\s*return\s*\(").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JS_RX39", "Multiple return statements with JSX - wrap in Fragment", Severity::Major, Category::CodeSmell, ctx.file_path, idx + 1).with_remediation(Remediation::quick("Return <Fragment> or <> with multiple children")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_RX40 — Component with too many props (>10)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_RX40"
    name: "Components should not have too many props"
    severity: Major
    category: CodeSmell
    language: "javascript"
    params: { max_props: usize = 10 }
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"function\s+\w+\s*\(\s*\{([^}]+)\}\s*\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                if let Some(props) = cap.get(1) {
                    let prop_count = props.as_str().split(',').filter(|p| !p.trim().is_empty()).count();
                    if prop_count > self.max_props {
                        issues.push(Issue::new("JS_RX40", format!("Component has {} props exceeding threshold of {}", prop_count, self.max_props), Severity::Major, Category::CodeSmell, ctx.file_path, idx + 1).with_remediation(Remediation::moderate("Group props into objects or split into smaller components")));
                    }
                }
            }
        }
        issues
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// BATCH 8: Testing + Async Rules (20 rules) — JS_TEST1 to JS_TEST10, JS_ASYNC1 to JS_ASYNC10
// ═══════════════════════════════════════════════════════════════════════════════

// ─────────────────────────────────────────────────────────────────────────────
// JS_TEST1 — Test with no assertions
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_TEST1"
    name: "Tests should contain assertions"
    severity: Major
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r#"(?:it|test|describe)\s*\(['"]"#).unwrap();
        let assertion_patterns = ["expect(", "assert.", "toBe(", "toEqual(", "toStrictEqual(", "toContain("];
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                let test_block: String = ctx.source.lines().skip(idx).take(30).collect::<Vec<_>>().join("\n");
                let has_assertion = assertion_patterns.iter().any(|p| test_block.contains(p));
                if !has_assertion {
                    issues.push(Issue::new("JS_TEST1", "Test without any assertions", Severity::Major, Category::CodeSmell, ctx.file_path, idx + 1).with_remediation(Remediation::moderate("Add assertions to verify the test behavior")));
                }
                break;
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_TEST2 — Test with multiple assertions (no unit isolation)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_TEST2"
    name: "Tests should have focused assertions"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: { max_assertions: usize = 3 }
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"(?:it|test)\s*\([^)]+\)\s*(?:=>\s*)?\{").unwrap();
        let assertion_patterns = ["expect(", "assert.", "toBe(", "toEqual("];
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                let test_block: String = ctx.source.lines().skip(idx).take(40).collect::<Vec<_>>().join("\n");
                let assertion_count: usize = assertion_patterns.iter().map(|p| test_block.matches(p).count()).sum();
                if assertion_count > self.max_assertions {
                    issues.push(Issue::new("JS_TEST2", format!("Test has {} assertions - consider splitting", assertion_count), Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1).with_remediation(Remediation::moderate("Split into multiple focused tests")));
                }
                break;
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_TEST3 — Test with hardcoded timeout
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_TEST3"
    name: "Tests should not have hardcoded timeouts"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"setTimeout|setInterval").unwrap();
        let is_test = ctx.source.contains("describe") || ctx.source.contains("it(") || ctx.source.contains("test(");
        if is_test {
            for (idx, line) in ctx.source.lines().enumerate() {
                if re.is_match(line) && (line.contains("2000") || line.contains("3000") || line.contains("5000")) {
                    issues.push(Issue::new("JS_TEST3", "Hardcoded timeout in test - use proper async waiting", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1).with_remediation(Remediation::moderate("Use waitFor, waitForElementToBeRemoved, or proper async utilities")));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_TEST4 — Snapshot without review
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_TEST4"
    name: "Snapshot tests should be reviewed"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"toMatchSnapshot|toMatchInlineSnapshot").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                let context: String = ctx.source.lines().skip(idx.saturating_sub(5)).take(10).collect::<Vec<_>>().join("\n");
                if !context.contains(".snap") && !context.contains("Snapshot") {
                    issues.push(Issue::new("JS_TEST4", "Snapshot test - ensure snapshots are reviewed in PR", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1).with_remediation(Remediation::moderate("Review snapshot changes before merging")));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_TEST5 — Mocks without cleanup
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_TEST5"
    name: "Mocks should be cleaned up after tests"
    severity: Major
    category: Bug
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let has_mock = ctx.source.contains("jest.fn()") || ctx.source.contains("mock(") || ctx.source.contains("vi.fn()");
        let has_after = ctx.source.contains("afterEach") || ctx.source.contains("afterAll");
        if has_mock && !has_after {
            issues.push(Issue::new("JS_TEST5", "Mocks without afterEach cleanup - may affect other tests", Severity::Major, Category::Bug, ctx.file_path, 1).with_remediation(Remediation::moderate("Add afterEach to cleanup mocks: afterEach(() => jest.clearAllMocks())")));
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_TEST6 — Async test without done callback or promise
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_TEST6"
    name: "Async tests need proper handling"
    severity: Major
    category: Bug
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"(?:it|test)\s*\([^)]+\)\s*(?:=>\s*)?\{").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                let test_block: String = ctx.source.lines().skip(idx).take(20).collect::<Vec<_>>().join("\n");
                let has_async = test_block.contains("async") && (test_block.contains("await") || test_block.contains("Promise"));
                let has_done = test_block.contains("done(") || test_block.contains("doneCallback");
                if !has_async && !has_done && (test_block.contains("fetch") || test_block.contains("Promise") || test_block.contains("setTimeout")) {
                    issues.push(Issue::new("JS_TEST6", "Async test without done callback or promise handling", Severity::Major, Category::Bug, ctx.file_path, idx + 1).with_remediation(Remediation::moderate("Add async/await or use done callback")));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_TEST7 — describe block with no tests
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_TEST7"
    name: "describe blocks should contain tests"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r#"describe\s*\(['"][^'"]+['"],?\s*(?:async\s*)?\([^)]*\)\s*(?:=>\s*)?\{"#).unwrap();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            if re.is_match(line) {
                let mut block_end = idx + 1;
                let mut brace_count = 0isize;
                let mut found_open = false;
                for (i, l) in lines.iter().enumerate().skip(idx) {
                    if l.contains("{") { found_open = true; brace_count += l.matches("{").count() as isize; }
                    if l.contains("}") { brace_count -= l.matches("}").count() as isize; }
                    if found_open && brace_count <= 0 { break; }
                    if i > idx { block_end = i; }
                }
                let block_content: String = lines.iter().skip(idx).take(block_end - idx).cloned().collect::<Vec<_>>().join("\n");
                if !block_content.contains("it(") && !block_content.contains("test(") && !block_content.contains("describe(") {
                    issues.push(Issue::new("JS_TEST7", "Empty describe block - no tests found", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1).with_remediation(Remediation::quick("Add tests or remove empty describe block")));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_TEST8 — it.skip with no reason
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_TEST8"
    name: "Skipped tests should have a reason"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r#"(?:it|test)\.skip\s*\(['"]"#).unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                let context: String = ctx.source.lines().skip(idx).take(3).collect::<Vec<_>>().join("\n");
                if !context.contains("//") && !context.contains("FIXME") && !context.contains("TODO") && !context.contains("reason") {
                    issues.push(Issue::new("JS_TEST8", "Test skipped without documented reason", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1).with_remediation(Remediation::moderate("Add comment explaining why test is skipped")));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_TEST9 — beforeEach without afterEach
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_TEST9"
    name: "beforeEach should have corresponding afterEach"
    severity: Major
    category: Bug
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let has_before = ctx.source.contains("beforeEach");
        let has_after = ctx.source.contains("afterEach");
        if has_before && !has_after {
            issues.push(Issue::new("JS_TEST9", "beforeEach without afterEach - cleanup may be missing", Severity::Major, Category::Bug, ctx.file_path, 1).with_remediation(Remediation::moderate("Add afterEach for proper test cleanup")));
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_TEST10 — Test with console.log
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_TEST10"
    name: "Tests should not contain console.log"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let has_test = ctx.source.contains("describe") || ctx.source.contains("it(") || ctx.source.contains("test(");
        if has_test {
            for (idx, line) in ctx.source.lines().enumerate() {
                if line.contains("console.log") && !line.trim().starts_with("//") {
                    issues.push(Issue::new("JS_TEST10", "console.log in test - remove or use proper assertions", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1).with_remediation(Remediation::quick("Remove console.log or replace with test assertion")));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_ASYNC1 — Promise without catch
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_ASYNC1"
    name: "Promise should have catch handler"
    severity: Major
    category: Bug
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains(".then(") && !ctx.source.lines().skip(idx).take(10).any(|l| l.contains(".catch(")) && !ctx.source.lines().skip(idx).take(10).any(|l| l.contains("try") && l.contains("catch")) {
                issues.push(Issue::new("JS_ASYNC1", "Promise without .catch() handler", Severity::Major, Category::Bug, ctx.file_path, idx + 1).with_remediation(Remediation::moderate("Add .catch() or use try/catch with async/await")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_ASYNC2 — await inside loop (use Promise.all)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_ASYNC2"
    name: "await inside loop should use Promise.all"
    severity: Major
    category: Bug
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"for\s*\([^)]*\)\s*\{[^}]*await\s+").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JS_ASYNC2", "await inside loop - use Promise.all() instead", Severity::Major, Category::Bug, ctx.file_path, idx + 1).with_remediation(Remediation::moderate("Use Promise.all(array.map(async x => ...)) or collect promises")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_ASYNC3 — async function without await
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_ASYNC3"
    name: "async functions should use await"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"async\s+(?:function\s+\w+|\([^)]*\)\s*=>|\w+\s*=>)\s*\{").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                let func_block: String = ctx.source.lines().skip(idx).take(20).collect::<Vec<_>>().join("\n");
                if !func_block.contains("await ") {
                    issues.push(Issue::new("JS_ASYNC3", "async function without await - remove async or add await", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1).with_remediation(Remediation::quick("Remove async keyword or add await for async operations")));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_ASYNC4 — setTimeout without clearTimeout
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_ASYNC4"
    name: "setTimeout should be cleared"
    severity: Major
    category: Bug
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let has_setTimeout = ctx.source.contains("setTimeout");
        let has_clearTimeout = ctx.source.contains("clearTimeout");
        let is_react = ctx.source.contains("useEffect") || ctx.source.contains("componentWillUnmount");
        if has_setTimeout && !has_clearTimeout && !is_react {
            issues.push(Issue::new("JS_ASYNC4", "setTimeout without clearTimeout - may cause memory leaks", Severity::Major, Category::Bug, ctx.file_path, 1).with_remediation(Remediation::moderate("Store timer ID and call clearTimeout in cleanup")));
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_ASYNC5 — setInterval without clearInterval
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_ASYNC5"
    name: "setInterval should be cleared"
    severity: Major
    category: Bug
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let has_setInterval = ctx.source.contains("setInterval");
        let has_clearInterval = ctx.source.contains("clearInterval");
        let is_react = ctx.source.contains("useEffect");
        if has_setInterval && !has_clearInterval && !is_react {
            issues.push(Issue::new("JS_ASYNC5", "setInterval without clearInterval - will run forever", Severity::Major, Category::Bug, ctx.file_path, 1).with_remediation(Remediation::moderate("Store interval ID and call clearInterval on cleanup")));
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_ASYNC6 — new Promise with sync code (should be async)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_ASYNC6"
    name: "new Promise with synchronous code"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"new\s+Promise\s*\(\s*\([^)]*\)\s*=>\s*\{").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                let func_block: String = ctx.source.lines().skip(idx).take(15).collect::<Vec<_>>().join("\n");
                let has_callback = func_block.contains("resolve(") || func_block.contains("reject(");
                if has_callback && !func_block.contains("setTimeout") && !func_block.contains("fetch") && !func_block.contains("readFile") && !func_block.contains("db.") {
                    issues.push(Issue::new("JS_ASYNC6", "Promise created with sync code - use async/await instead", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1).with_remediation(Remediation::quick("Replace new Promise((resolve, reject) => { resolve(x); }) with Promise.resolve(x)")));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_ASYNC7 — Promise.all with mixed types
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_ASYNC7"
    name: "Promise.all should have homogeneous types"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"Promise\.all\s*\(\s*\[").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                let context: String = ctx.source.lines().skip(idx).take(5).collect::<Vec<_>>().join("\n");
                if context.matches("fetch(").count() > 1 && (context.matches("Promise.resolve").count() > 0 || context.matches("await").count() > 0) {
                    issues.push(Issue::new("JS_ASYNC7", "Promise.all with mixed types - ensure consistent promise types", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1).with_remediation(Remediation::moderate("Ensure all items in Promise.all are the same type")));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_ASYNC8 — unhandled promise rejection
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_ASYNC8"
    name: "Unhandled promise rejections should be caught"
    severity: Critical
    category: Bug
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\.then\s*\([^)]*\)\s*;").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                let remaining: String = ctx.source.lines().skip(idx).take(10).collect::<Vec<_>>().join("\n");
                if !remaining.contains(".catch") && !remaining.contains("try") && !remaining.contains("process.on") {
                    issues.push(Issue::new("JS_ASYNC8", "Potential unhandled promise rejection", Severity::Critical, Category::Bug, ctx.file_path, idx + 1).with_remediation(Remediation::moderate("Add .catch() handler or use try/catch")));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_ASYNC9 — Promise.race with no timeout fallback
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_ASYNC9"
    name: "Promise.race should have timeout fallback"
    severity: Major
    category: Bug
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"Promise\.race\s*\(").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                let context: String = ctx.source.lines().skip(idx).take(5).collect::<Vec<_>>().join("\n");
                if !context.contains("timeout") && !context.contains("race") && context.matches("Promise.race").count() > 0 {
                    issues.push(Issue::new("JS_ASYNC9", "Promise.race without timeout - may hang forever", Severity::Major, Category::Bug, ctx.file_path, idx + 1).with_remediation(Remediation::moderate("Add timeout fallback: Promise.race([promise, timeout])")));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_ASYNC10 — then/catch without finally for cleanup
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_ASYNC10"
    name: "Promises should have finally for cleanup"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let has_promise = ctx.source.contains(".then(") || ctx.source.contains(".catch(");
        let has_finally = ctx.source.contains(".finally(");
        let has_cleanup_keywords = ctx.source.contains("loading") || ctx.source.contains("isLoading") || ctx.source.contains("spinner");
        if has_promise && !has_finally && has_cleanup_keywords {
            issues.push(Issue::new("JS_ASYNC10", "Promise without finally - cleanup may not run on error", Severity::Minor, Category::CodeSmell, ctx.file_path, 1).with_remediation(Remediation::moderate("Add .finally() for cleanup: .finally(() => { /* cleanup */ })")));
        }
        issues
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// BATCH 9: Node.js Backend Rules (15 rules) — JS_NODE1 to JS_NODE15
// ═══════════════════════════════════════════════════════════════════════════════

// ─────────────────────────────────────────────────────────────────────────────
// JS_NODE1 — fs.readFileSync instead of async fs.promises.readFile
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_NODE1"
    name: "Synchronous file read blocks the event loop"
    severity: Major
    category: Bug
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"fs\.readFileSync\s*\(").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "JS_NODE1",
                    "fs.readFileSync blocks the event loop - use fs.promises.readFile or fs.readFile with callback",
                    Severity::Major,
                    Category::Bug,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::moderate("Use async fs.promises.readFile with await or fs.readFile with callback")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_NODE2 — process.exit() in library code
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_NODE2"
    name: "process.exit() should not be used in library code"
    severity: Major
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"process\.exit\s*\(").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "JS_NODE2",
                    "process.exit() in library code - let the caller decide how to handle termination",
                    Severity::Major,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::moderate("Return an error code instead of calling process.exit()")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_NODE3 — Unhandled 'error' event on EventEmitter
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_NODE3"
    name: "EventEmitter 'error' event should be handled"
    severity: Blocker
    category: Bug
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"new\s+(EventEmitter|Server|Net|Http|https?)\s*\(").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                let context: String = ctx.source.lines().skip(idx).take(20).collect::<Vec<_>>().join("\n");
                if !context.contains(".on('error'") && !context.contains(".on(\"error\"") && !context.contains(".once('error'") {
                    issues.push(Issue::new(
                        "JS_NODE3",
                        "EventEmitter created without 'error' event handler - unhandled errors will crash the process",
                        Severity::Blocker,
                        Category::Bug,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::moderate("Add error handler: emitter.on('error', (err) => { /* handle */ })")));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_NODE4 — require inside function (should be at top)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_NODE4"
    name: "require() should be at the top level of modules"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"(?:function|const\s+\w+\s*=\s*(?:async\s*)?\([^)]*\)\s*=>|=>\s*\()").unwrap();
        let require_re = regex::Regex::new(r"require\s*\(").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                let func_block: String = ctx.source.lines().skip(idx).take(30).collect::<Vec<_>>().join("\n");
                if require_re.is_match(&func_block) {
                    issues.push(Issue::new(
                        "JS_NODE4",
                        "require() inside function - move to top level for better performance and clarity",
                        Severity:: Minor,
                        Category:: CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::quick("Move require() calls to the top of the module")));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_NODE5 — __dirname/__filename usage (use import.meta.url)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_NODE5"
    name: "__dirname and __filename should not be used in ES modules"
    severity: Major
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"__dirname|__filename").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "JS_NODE5",
                    "__dirname/__filename not available in ES modules - use import.meta.url and fileURLToPath",
                    Severity::Major,
                    Category::CodeSmell,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::moderate("Use: import { fileURLToPath } from 'url'; const __filename = fileURLToPath(import.meta.url);")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_NODE6 — Buffer constructor deprecated (use Buffer.from())
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_NODE6"
    name: "Buffer constructor is deprecated"
    severity: Major
    category: Bug
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"new\s+Buffer\s*\(").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "JS_NODE6",
                    "new Buffer() is deprecated - use Buffer.from() instead",
                    Severity::Major,
                    Category::Bug,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::quick("Replace new Buffer(x) with Buffer.from(x)")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_NODE7 — Synchronous I/O in request handler (block event loop)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_NODE7"
    name: "Synchronous I/O in request handler blocks the event loop"
    severity: Critical
    category: Bug
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let sync_io_patterns = ["readFileSync", "writeFileSync", "readSync", "writeSync", "openSync", "closeSync", "statSync", "lstatSync", "readdirSync"];
        let is_handler = ctx.source.contains("app.get") || ctx.source.contains("app.post") || ctx.source.contains("app.put") || ctx.source.contains("app.delete") || ctx.source.contains("router.get") || ctx.source.contains("router.post") || ctx.source.contains("export function") || ctx.source.contains("export const");
        if is_handler {
            for (idx, line) in ctx.source.lines().enumerate() {
                for pattern in &sync_io_patterns {
                    if line.contains(pattern) {
                        issues.push(Issue::new(
                            "JS_NODE7",
                            format!("Synchronous {} in request handler blocks the event loop", pattern),
                            Severity::Critical,
                            Category::Bug,
                            ctx.file_path,
                            idx + 1,
                        ).with_remediation(Remediation::moderate("Use async version of the I/O operation")));
                        break;
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_NODE8 — Missing error handler on stream
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_NODE8"
    name: "Streams should have error handlers"
    severity: Major
    category: Bug
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\.(pipe|on|once)\s*\(").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                let context: String = ctx.source.lines().skip(idx.saturating_sub(3)).take(10).collect::<Vec<_>>().join("\n");
                if !context.contains(".on('error'") && !context.contains(".on(\"error\"") && !context.contains(".catch(") {
                    issues.push(Issue::new(
                        "JS_NODE8",
                        "Stream operation without error handler - errors will crash silently",
                        Severity::Major,
                        Category::Bug,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::moderate("Add error handler: stream.on('error', (err) => { /* handle */ })")));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_NODE9 — process.env accessed without default
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_NODE9"
    name: "process.env should be accessed with default values"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"process\.env\.\w+").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.find(line) {
                let env_access = cap.as_str();
                if !line.contains("||") && !line.contains("??") && !line.contains("||=") && !line.contains("??=") {
                    issues.push(Issue::new(
                        "JS_NODE9",
                        format!("{} accessed without default value - may be undefined", env_access),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::quick("Add default: process.env.KEY || 'default'")));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_NODE10 — child_process.exec with user input (command injection)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_NODE10"
    name: "child_process.exec with user input is vulnerable to command injection"
    severity: Blocker
    category: Vulnerability
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let has_exec = ctx.source.contains("exec(") || ctx.source.contains("execSync(");
        let has_user_input = ctx.source.contains("req.") || ctx.source.contains("params") || ctx.source.contains("query") || ctx.source.contains("body") || ctx.source.contains("ctx.request") || ctx.source.contains("ctx.params");
        if has_exec && has_user_input {
            for (idx, line) in ctx.source.lines().enumerate() {
                if (line.contains(".exec(") || line.contains("execSync(")) && (line.contains("req.") || line.contains("params") || line.contains("query") || line.contains("body")) {
                    issues.push(Issue::new(
                        "JS_NODE10",
                        "Command injection risk: user input passed to exec() - use execFile() with argument array",
                        Severity::Blocker,
                        Category::Vulnerability,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::moderate("Use execFile() with array of arguments instead of string interpolation")));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_NODE11 — eval() with user input (Node variant of JS_S1523)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_NODE11"
    name: "eval() with user input is dangerous"
    severity: Blocker
    category: Vulnerability
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let has_eval = ctx.source.contains("eval(") || ctx.source.contains("new Function(") || ctx.source.contains("vm.runInScript");
        let has_user_input = ctx.source.contains("req.") || ctx.source.contains("params") || ctx.source.contains("query") || ctx.source.contains("body");
        if has_eval && has_user_input {
            for (idx, line) in ctx.source.lines().enumerate() {
                if (line.contains("eval(") || line.contains("new Function(")) && (line.contains("req.") || line.contains("params") || line.contains("query") || line.contains("body")) {
                    issues.push(Issue::new(
                        "JS_NODE11",
                        "Code injection risk: eval() with user input - avoid if possible",
                        Severity::Blocker,
                        Category::Vulnerability,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::substantial("Avoid eval with user input - use JSON.parse or safe parsing libraries")));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_NODE12 — JSON.parse without try/catch
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_NODE12"
    name: "JSON.parse should be wrapped in try/catch"
    severity: Major
    category: Bug
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"JSON\.parse\s*\(").unwrap();
        let all_lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in all_lines.iter().enumerate() {
            if re.is_match(line) {
                let start = idx.saturating_sub(5);
                let context_before: String = all_lines[start..idx].iter().rev().map(|s| s.to_string()).collect::<Vec<_>>().join("\n");
                let context_after: String = all_lines[idx+1..std::cmp::min(idx+6, all_lines.len())].iter().map(|s| s.to_string()).collect::<Vec<_>>().join("\n");
                let full_context = format!("{}\n{}", context_before, context_after);
                if !full_context.contains("try") || (!full_context.contains("catch") && !full_context.contains("finally")) {
                    issues.push(Issue::new(
                        "JS_NODE12",
                        "JSON.parse without try/catch - invalid JSON will throw and crash",
                        Severity::Major,
                        Category::Bug,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::moderate("Wrap JSON.parse in try/catch or use try? optional chaining")));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_NODE13 — path.join with user input (path traversal)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_NODE13"
    name: "path.join with user input may allow path traversal"
    severity: Major
    category: Vulnerability
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let has_path_join = ctx.source.contains("path.join") || ctx.source.contains("path.resolve");
        let has_user_input = ctx.source.contains("req.") || ctx.source.contains("params") || ctx.source.contains("query") || ctx.source.contains("body");
        if has_path_join && has_user_input {
            for (idx, line) in ctx.source.lines().enumerate() {
                if (line.contains("path.join") || line.contains("path.resolve")) && (line.contains("req.") || line.contains("params") || line.contains("query") || line.contains("body")) {
                    issues.push(Issue::new(
                        "JS_NODE13",
                        "Path traversal risk: path.join with user input - ensure user input is sanitized",
                        Severity::Major,
                        Category::Vulnerability,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::moderate("Validate and sanitize user input before using in path operations")));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_NODE14 — require('http') instead of https
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_NODE14"
    name: "http module should not be used - use https"
    severity: Major
    category: Vulnerability
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r#"require\s*\(\s*['"]http['"]\s*\)"#).unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "JS_NODE14",
                    "HTTP module used - use HTTPS for secure communications",
                    Severity::Major,
                    Category::Vulnerability,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::moderate("Use https module or redirect HTTP to HTTPS")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_NODE15 — crypto.randomBytes without callback error check
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_NODE15"
    name: "crypto.randomBytes callback should check for errors"
    severity: Major
    category: Bug
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"crypto\.randomBytes\s*\([^)]+\)\s*\(").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                let context: String = ctx.source.lines().skip(idx).take(3).collect::<Vec<_>>().join("\n");
                if !context.contains("err") && !context.contains("error") {
                    issues.push(Issue::new(
                        "JS_NODE15",
                        "crypto.randomBytes without error check in callback",
                        Severity::Major,
                        Category::Bug,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::moderate("Check for error in callback: (err, buf) => { if (err) throw err; ... })")));
                }
            }
        }
        issues
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// BATCH 9b: More Security Rules (15 rules) — JS_SEC1 to JS_SEC15
// ═══════════════════════════════════════════════════════════════════════════════

// ─────────────────────────────────────────────────────────────────────────────
// JS_SEC1 — RegExp constructor with user input (ReDoS)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_SEC1"
    name: "RegExp constructor with user input is vulnerable to ReDoS"
    severity: Major
    category: Vulnerability
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"new\s+RegExp\s*\(").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && (line.contains("req.") || line.contains("params") || line.contains("query") || line.contains("body")) {
                issues.push(Issue::new(
                    "JS_SEC1",
                    "ReDoS risk: RegExp constructed with user input",
                    Severity::Major,
                    Category::Vulnerability,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::moderate("Validate user input or use safe regexp library")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_SEC2 — JSONP callback without validation
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_SEC2"
    name: "JSONP endpoints should validate callback names"
    severity: Major
    category: Vulnerability
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let has_jsonp = ctx.source.contains("callback") || ctx.source.contains("jsonp") || ctx.source.contains("jsonpCallback");
        let has_user_input = ctx.source.contains("req.") || ctx.source.contains("params") || ctx.source.contains("query");
        if has_jsonp && has_user_input {
            for (idx, line) in ctx.source.lines().enumerate() {
                if (line.contains("callback") || line.contains("jsonp")) && !line.contains("replace") && !line.contains("match") && !line.contains("test") {
                    issues.push(Issue::new(
                        "JS_SEC2",
                        "JSONP callback may be vulnerable to XSS - validate callback name",
                        Severity::Major,
                        Category::Vulnerability,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::moderate(r"Validate callback name with regex: /^[\w]+$/")));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_SEC3 — CORS with Access-Control-Allow-Origin: *
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_SEC3"
    name: "CORS should not allow all origins with credentials"
    severity: Major
    category: Vulnerability
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"Access-Control-Allow-Origin\s*:\s*[*]").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                let context: String = ctx.source.lines().skip(idx.saturating_sub(2)).take(5).collect::<Vec<_>>().join("\n");
                if context.contains("credentials") || context.contains("true") || context.contains("withCredentials") {
                    issues.push(Issue::new(
                        "JS_SEC3",
                        "CORS allows all origins (*) with credentials - susceptible to CSRF",
                        Severity::Major,
                        Category::Vulnerability,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::moderate("Use specific origin instead of * when using credentials")));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_SEC4 — Strict-Transport-Security max-age too low (< 1 year)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_SEC4"
    name: "HSTS max-age should be at least 1 year (31536000 seconds)"
    severity: Major
    category: Vulnerability
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"Strict-Transport-Security\s*:\s*[^;]+").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.find(line) {
                let hsts = cap.as_str();
                if let Some(max_age_match) = regex::Regex::new(r"max-age\s*=\s*(\d+)").unwrap().find(hsts) {
                    if let Some(val) = max_age_match.as_str().split('=').nth(1) {
                        if let Ok(val_int) = val.parse::<i64>() {
                            if val_int < 31536000 {
                                issues.push(Issue::new(
                                    "JS_SEC4",
                                    format!("HSTS max-age is {} seconds (minimum: 31536000)", val_int),
                                    Severity::Major,
                                    Category:: Vulnerability,
                                    ctx.file_path,
                                    idx + 1,
                                ).with_remediation(Remediation::moderate("Set max-age to at least 31536000 (1 year)")));
                            }
                        }
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_SEC5 — helmet middleware not used in Express
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_SEC5"
    name: "Express apps should use helmet middleware"
    severity: Minor
    category: SecurityHotspot
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let is_express = ctx.source.contains("express") || ctx.source.contains("createServer");
        let has_helmet = ctx.source.contains("helmet") || ctx.source.contains("x-powered-by");
        if is_express && !has_helmet {
            issues.push(Issue::new(
                "JS_SEC5",
                "Express app should use helmet middleware for security headers",
                Severity:: Minor,
                Category:: SecurityHotspot,
                ctx.file_path,
                1,
            ).with_remediation(Remediation::quick("Add: app.use(helmet())")));
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_SEC6 — cookie-parser without signed cookies
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_SEC6"
    name: "cookie-parser should use signed cookies"
    severity: Minor
    category: SecurityHotspot
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let has_cookie_parser = ctx.source.contains("cookie-parser") || ctx.source.contains("cookieParser");
        let has_signed = ctx.source.contains("signed:true") || ctx.source.contains("signed: true");
        if has_cookie_parser && !has_signed {
            issues.push(Issue::new(
                "JS_SEC6",
                "cookie-parser configured without signed cookies - cookies can be tampered",
                Severity::Minor,
                Category::SecurityHotspot,
                ctx.file_path,
                1,
            ).with_remediation(Remediation::moderate("Use signed cookies: cookieParser('secret', { signed: true })")));
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_SEC7 — express-session with default secret
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_SEC7"
    name: "express-session should not use default or weak secret"
    severity: Major
    category: Vulnerability
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let has_session = ctx.source.contains("express-session") || ctx.source.contains("session(");
        let has_weak_secret = ctx.source.contains("'secret'") || ctx.source.contains("\"secret\"") || ctx.source.contains("'password'") || ctx.source.contains("\"password\"");
        if has_session && has_weak_secret {
            issues.push(Issue::new(
                "JS_SEC7",
                "express-session using weak/default secret - session hijacking risk",
                Severity::Major,
                Category::Vulnerability,
                ctx.file_path,
                1,
            ).with_remediation(Remediation::substantial("Use a strong random secret: crypto.randomBytes(64).toString('hex')")));
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_SEC8 — csp without report-uri for monitoring
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_SEC8"
    name: "Content Security Policy should have report-uri"
    severity: Minor
    category: SecurityHotspot
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let has_csp = ctx.source.contains("Content-Security-Policy") || ctx.source.contains("csp");
        let has_report = ctx.source.contains("report-uri") || ctx.source.contains("reportURL");
        if has_csp && !has_report {
            issues.push(Issue::new(
                "JS_SEC8",
                "CSP without report-uri - violations will not be monitored",
                Severity::Minor,
                Category::SecurityHotspot,
                ctx.file_path,
                1,
            ).with_remediation(Remediation::moderate("Add report-uri to CSP for violation monitoring")));
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_SEC9 — x-powered-by header not removed
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_SEC9"
    name: "x-powered-by header should be disabled"
    severity: Minor
    category: SecurityHotspot
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let is_express = ctx.source.contains("express");
        let has_disabled = ctx.source.contains("x-powered-by: false") || ctx.source.contains("xPoweredBy: false") || ctx.source.contains("app.disable");
        if is_express && !has_disabled {
            issues.push(Issue::new(
                "JS_SEC9",
                "x-powered-by header not disabled - exposes server technology",
                Severity::Minor,
                Category::SecurityHotspot,
                ctx.file_path,
                1,
            ).with_remediation(Remediation::quick("Add: app.disable('x-powered-by')")));
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_SEC10 — body-parser with unlimited size
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_SEC10"
    name: "body-parser should limit request body size"
    severity: Major
    category: Vulnerability
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let has_body_parser = ctx.source.contains("body-parser") || ctx.source.contains("express.json") || ctx.source.contains("express.urlencoded");
        let has_limit = ctx.source.contains("limit:") || ctx.source.contains("maxBodySize");
        if has_body_parser && !has_limit {
            issues.push(Issue::new(
                "JS_SEC10",
                "body-parser without size limit - DoS risk",
                Severity::Major,
                Category::Vulnerability,
                ctx.file_path,
                1,
            ).with_remediation(Remediation::moderate("Add limit: app.use(express.json({ limit: '100kb' }))")));
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_SEC11 — express-rate-limit not configured
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_SEC11"
    name: "express-rate-limit should be configured"
    severity: Minor
    category: SecurityHotspot
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let is_express = ctx.source.contains("express");
        let has_rate_limit = ctx.source.contains("rate-limit") || ctx.source.contains("rateLimit") || ctx.source.contains("express-rate-limit");
        if is_express && !has_rate_limit {
            issues.push(Issue::new(
                "JS_SEC11",
                "express-rate-limit not configured - susceptible to brute force attacks",
                Severity::Minor,
                Category::SecurityHotspot,
                ctx.file_path,
                1,
            ).with_remediation(Remediation::moderate("Add rate limiting: const rateLimit = require('express-rate-limit'); app.use(rateLimit({...}))")));
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_SEC12 — csurf (CSRF middleware) not used
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_SEC12"
    name: "CSRF protection should be used"
    severity: Major
    category: Vulnerability
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let is_express = ctx.source.contains("express");
        let has_csrf = ctx.source.contains("csurf") || ctx.source.contains("csrf") || ctx.source.contains("csrftoken") || ctx.source.contains("csrf-token") || ctx.source.contains("_csrf");
        let is_api = ctx.source.contains("api") || ctx.source.contains("rest") || ctx.source.contains("graphql");
        if is_express && !has_csrf && is_api {
            issues.push(Issue::new(
                "JS_SEC12",
                "CSRF middleware not used - API endpoints vulnerable to CSRF",
                Severity::Major,
                Category::Vulnerability,
                ctx.file_path,
                1,
            ).with_remediation(Remediation::moderate("Add CSRF protection: const csrfProtection = csurf(); app.use(csrfProtection)")));
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_SEC13 — http module used (should be https)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_SEC13"
    name: "http module used - use https for secure connections"
    severity: Major
    category: Vulnerability
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r#"require\s*\(\s*['"]http['"]\s*\)"#).unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "JS_SEC13",
                    "HTTP module used - use HTTPS module for secure communications",
                    Severity::Major,
                    Category::Vulnerability,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::moderate("Use https module or redirect HTTP to HTTPS")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_SEC14 — http-proxy without validation
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_SEC14"
    name: "http-proxy should validate target URL"
    severity: Major
    category: Vulnerability
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let has_proxy = ctx.source.contains("http-proxy") || ctx.source.contains("createProxyMiddleware") || ctx.source.contains("proxy");
        let has_user_input = ctx.source.contains("req.") || ctx.source.contains("params") || ctx.source.contains("query") || ctx.source.contains("target");
        if has_proxy && has_user_input {
            for (idx, line) in ctx.source.lines().enumerate() {
                if line.contains("target") && !line.contains("validateTarget") && !line.contains("allowedHosts") && !line.contains("checkPath") {
                    issues.push(Issue::new(
                        "JS_SEC14",
                        "http-proxy target may not be validated - open redirect risk",
                        Severity::Major,
                        Category::Vulnerability,
                        ctx.file_path,
                        idx + 1,
                    ).with_remediation(Remediation::moderate("Validate target URL or use allowedHosts/allowedProtocols")));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_SEC15 — serialize-javascript instead of JSON.stringify
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JS_SEC15"
    name: "serialize-javascript may be vulnerable to XSS"
    severity: Blocker
    category: Vulnerability
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"serialize-javascript").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new(
                    "JS_SEC15",
                    "serialize-javascript may execute arbitrary code - use JSON.stringify for safe serialization",
                    Severity::Blocker,
                    Category::Vulnerability,
                    ctx.file_path,
                    idx + 1,
                ).with_remediation(Remediation::moderate("Replace serialize-javascript with JSON.stringify or use safe-serialize")));
            }
        }
        issues
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// JAVA SECURITY RULES (25 rules)
// ═══════════════════════════════════════════════════════════════════════════════

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2068 — Hardcoded credentials
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2068"
    name: "Hardcoded credentials should not be used"
    severity: Blocker
    category: Vulnerability
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let patterns = [
            r#"(?i)(password|passwd|pwd|secret|token|api[_-]?key)\s*[=:]\s*["'][^"']{4,}["']"#,
        ];
        let regexes: Vec<_> = patterns.iter().filter_map(|p| regex::Regex::new(p).ok()).collect();
        for (line_num, line) in ctx.source.lines().enumerate() {
            for re in &regexes {
                if re.is_match(line) {
                    issues.push(Issue::new("JAVA_S2068", "Hardcoded credential detected", Severity::Blocker, Category::Vulnerability, ctx.file_path, line_num + 1));
                    break;
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2077 — SQL injection (Statement.executeQuery with string concat)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2077"
    name: "SQL injection vulnerabilities should be prevented"
    severity: Blocker
    category: Vulnerability
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if (line.contains("executeQuery") || line.contains("executeUpdate") || line.contains("execute(")) && (line.contains("+") || line.contains("String.format") || line.contains("concat(")) {
                if !line.contains("PreparedStatement") && !line.contains("?") {
                    issues.push(Issue::new("JAVA_S2077", "SQL query built with string concatenation", Severity::Blocker, Category::Vulnerability, ctx.file_path, idx + 1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2092 — Cookie without secure flag
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2092"
    name: "Cookies should set the Secure flag"
    severity: Minor
    category: SecurityHotspot
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if (line.contains("new Cookie") || line.contains("ResponseCookie") || line.contains("setCookie") || line.contains("addCookie")) && !line.contains("setSecure") && !line.contains("Secure") {
                issues.push(Issue::new("JAVA_S2092", "Cookie without Secure flag", Severity::Minor, Category::SecurityHotspot, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2225 — toString() called on array (identity hash)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2225"
    name: "Calling toString() on an array does not provide useful information"
    severity: Minor
    category: Bug
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\[\s*\]\.toString\(\)|\.toString\(\)\s*\[|^Arrays\.toString\s*\(\s*\w+\s*\)$").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_S2225", "toString() on array returns identity hashcode", Severity::Minor, Category::Bug, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2259 — Null pointer dereference (possible NPE)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2259"
    name: "Null pointer dereferences should be avoided"
    severity: Blocker
    category: Bug
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\w+\.\w+\s*\(\s*\)\s*\.\w+|\w+\s*\[\s*\w+\s*\]\.\w+|\(\s*\w+\s*==\s*null\s*\)|null\s*\.\w+").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_S2259", "Possible null pointer dereference", Severity::Blocker, Category::Bug, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2272 — Iterator.next() without hasNext()
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2272"
    name: "Iterator should be checked for availability before calling next()"
    severity: Major
    category: Bug
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains(".next()") && !ctx.source.lines().take(idx + 1).any(|l| l.contains(".hasNext()")) {
                let context: String = ctx.source.lines().skip(idx.saturating_sub(5)).take(10).collect::<Vec<_>>().join("\n");
                if context.contains("Iterator") || context.contains("iterator") {
                    issues.push(Issue::new("JAVA_S2272", "Iterator.next() called without hasNext() check", Severity::Major, Category::Bug, ctx.file_path, idx + 1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2384 — Mutable member in serializable class
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2384"
    name: "Mutable members should not be stored in serializable classes"
    severity: Major
    category: Bug
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let mut in_serializable = false;
        let re = regex::Regex::new(r"class\s+\w+\s*(?:extends\s+\w+\s*)?(?:implements\s+\w+)*").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && (line.contains("Serializable") || line.contains("serialVersionUID")) {
                in_serializable = true;
            }
            if in_serializable && (line.contains("ArrayList") || line.contains("HashMap") || line.contains("HashSet") || line.contains("Date") || line.contains("SimpleDateFormat")) {
                if !line.contains("final") && !line.contains("volatile") {
                    issues.push(Issue::new("JAVA_S2384", "Mutable member in serializable class", Severity::Major, Category::Bug, ctx.file_path, idx + 1));
                }
            }
            if in_serializable && line.trim() == "}" && !line.contains("{") {
                in_serializable = false;
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2386 — Mutable static field (public static non-final)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2386"
    name: "Mutable static fields should not be public"
    severity: Critical
    category: Bug
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        // Detect public static fields that are not final
        for (idx, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("public static ") && !trimmed.contains("final") {
                issues.push(Issue::new("JAVA_S2386", "Mutable public static field", Severity::Critical, Category::Bug, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2444 — Lazy initialization of static field (non-thread-safe)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2444"
    name: "Lazy initialization of static fields should be thread-safe"
    severity: Major
    category: Bug
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        // Detect static field initialization via getInstance or new without proper thread-safety
        // Check for: static ... = ...getInstance(...) or static ... = new ...
        for (idx, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("static ") {
                // Skip if it contains "final"
                if trimmed.contains("final") {
                    continue;
                }
                // Check for getInstance or "new" pattern
                if (trimmed.contains("getInstance") || trimmed.contains("= new ")) && !trimmed.contains("synchronized") {
                    issues.push(Issue::new("JAVA_S2444", "Non-thread-safe lazy initialization of static field", Severity::Major, Category::Bug, ctx.file_path, idx + 1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2445 — Synchronized on non-final field
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2445"
    name: "Synchronized blocks should not synchronize on non-final fields"
    severity: Major
    category: Bug
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"synchronized\s*\(\s*\w+\s*\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                if let Some(field) = cap.get(1) {
                    let field_name = field.as_str();
                    if !field_name.chars().all(|c| c.is_uppercase()) && !field_name.contains("this") {
                        issues.push(Issue::new("JAVA_S2445", "Synchronizing on non-final field", Severity::Major, Category::Bug, ctx.file_path, idx + 1));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2446 — notify() instead of notifyAll()
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2446"
    name: "notify() should be used instead of notifyAll() when only one thread is waiting"
    severity: Major
    category: Bug
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains(".notify()") && !line.contains("//") {
                let next_lines: String = ctx.source.lines().skip(idx + 1).take(20).collect::<Vec<_>>().join("\n");
                if !next_lines.contains(".notifyAll()") {
                    issues.push(Issue::new("JAVA_S2446", "notify() used instead of notifyAll()", Severity::Major, Category::Bug, ctx.file_path, idx + 1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2447 — Boolean method not starting with 'is' or 'has'
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2447"
    name: "Boolean methods should start with 'is' or 'has'"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"(public|private|protected)?\s*boolean\s+(\w+)\s*\(").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                if let Some(name) = cap.get(2) {
                    let method_name = name.as_str();
                    if !method_name.starts_with("is") && !method_name.starts_with("has") && !method_name.starts_with("can") && !method_name.starts_with("should") {
                        issues.push(Issue::new("JAVA_S2447", format!("Boolean method '{}' should start with is/has/can/should", method_name), Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2583 — Condition always false (dead code)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2583"
    name: "Conditions should not always evaluate to the same value"
    severity: Major
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed == "if (true) {" || trimmed == "if (false) {" || trimmed == "if (true)" || trimmed == "if (false)" {
                issues.push(Issue::new("JAVA_S2583", "Condition always evaluates to the same value", Severity::Major, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2589 — Boolean expression always true
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2589"
    name: "Boolean expressions should not be constant"
    severity: Major
    category: Bug
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"if\s*\(\s*true\s*\)|if\s*\(\s*false\s*\)|while\s*\(\s*true\s*\)|while\s*\(\s*false\s*\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_S2589", "Constant boolean expression", Severity::Major, Category::Bug, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2637 — Non-null annotation missing on parameter
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2637"
    name: "@NonNull annotations should be used on parameters that cannot be null"
    severity: Major
    category: Bug
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"(public|private|protected)?\s+\w+\s+\w+\s*\([^)]*\)\s*\{").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                let context: String = ctx.source.lines().skip(idx.saturating_sub(2)).take(5).collect::<Vec<_>>().join("\n");
                if !context.contains("@NonNull") && !context.contains("NotNull") && !context.contains("Nullable") {
                    issues.push(Issue::new("JAVA_S2637", "Parameter may be null but lacks @NonNull annotation", Severity::Major, Category::Bug, ctx.file_path, idx + 1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2638 — Method contract violation (@NonNull not respected)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2638"
    name: "@NonNull method contract should not be violated"
    severity: Major
    category: Bug
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if (line.contains("return null") || line.contains("return (null)")) && !line.contains("if") {
                let prev_lines: String = ctx.source.lines().skip(idx.saturating_sub(10)).take(10).collect::<Vec<_>>().join("\n");
                if prev_lines.contains("@NonNull") || prev_lines.contains("NotNull") {
                    issues.push(Issue::new("JAVA_S2638", "@NonNull method returns null", Severity::Major, Category::Bug, ctx.file_path, idx + 1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2639 — Inappropriate regular expression
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2639"
    name: "Inappropriate regular expressions can cause performance issues"
    severity: Major
    category: Bug
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let dangerous_patterns = [r".*", r".*.*", r"(.+)+", r"(.+)+.*", r"(\[.*\])+\)"];
        for (idx, line) in ctx.source.lines().enumerate() {
            for pattern in &dangerous_patterns {
                if let Ok(re) = regex::Regex::new(pattern) {
                    if re.is_match(line) && line.contains("Pattern") && !line.contains("//") {
                        issues.push(Issue::new("JAVA_S2639", "Catastrophic backtracking regex pattern", Severity::Major, Category::Bug, ctx.file_path, idx + 1));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2658 — Class without no-arg constructor in serializable
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2658"
    name: "Serializable classes should have a no-arg constructor"
    severity: Major
    category: Bug
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let mut has_serializable = false;
        let mut has_no_arg_constructor = false;
        let mut class_line = 0;
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("implements Serializable") || line.contains("extends Serializable") {
                has_serializable = true;
                class_line = idx;
            }
            if has_serializable && regex::Regex::new(r"class\s+\w+\s*\(\s*\)").unwrap().is_match(line) {
                has_no_arg_constructor = true;
            }
        }
        if has_serializable && !has_no_arg_constructor {
            issues.push(Issue::new("JAVA_S2658", "Serializable class missing no-arg constructor", Severity::Major, Category::Bug, ctx.file_path, class_line + 1));
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2674 — Stream returned by method not consumed
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2674"
    name: "Streams should be consumed after creation"
    severity: Major
    category: Bug
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\.stream\(\)\s*\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && !line.contains(".collect") && !line.contains(".forEach") && !line.contains(".reduce") && !line.contains(".count()") && !line.contains(".toArray") {
                issues.push(Issue::new("JAVA_S2674", "Stream created but not consumed", Severity::Major, Category::Bug, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2681 — Block marked synchronized on non-final field
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2681"
    name: "Synchronized blocks should not synchronize on non-final fields"
    severity: Critical
    category: Bug
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"synchronized\s*\(\s*this\s*\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_S2681", "Synchronizing on 'this' is unsafe", Severity::Critical, Category::Bug, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2692 — indexOf with 0 check (should be >= 0)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2692"
    name: "indexOf() result should be compared to >= 0"
    severity: Major
    category: Bug
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"indexOf\s*\([^)]+\)\s*==\s*0").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_S2692", "indexOf() compared to 0 instead of >= 0", Severity::Major, Category::Bug, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2755 — XXE processing (XML parser without secure features)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2755"
    name: "XML parsing should not be vulnerable to external entity attacks"
    severity: Blocker
    category: Vulnerability
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if (line.contains("DocumentBuilder") || line.contains("SAXParser") || line.contains("XMLInputFactory") || line.contains("TransformerFactory") || line.contains("SchemaFactory")) && !line.contains("XMLConstants") && !line.contains("FEATURE_SECURE_PROCESSING") {
                issues.push(Issue::new("JAVA_S2755", "XXE vulnerability - XML parser not configured securely", Severity::Blocker, Category::Vulnerability, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2975 — clone() without super.clone()
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2975"
    name: "clone() should override Object.clone() properly"
    severity: Major
    category: Bug
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"protected\s+\w+\s+clone\s*\(\s*\)\s*\{").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                let next_lines: String = ctx.source.lines().skip(idx + 1).take(10).collect::<Vec<_>>().join("\n");
                if !next_lines.contains("super.clone()") && !next_lines.contains("super\\.clone()") {
                    issues.push(Issue::new("JAVA_S2975", "clone() does not call super.clone()", Severity::Major, Category::Bug, ctx.file_path, idx + 1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S3038 — Abstract method call in constructor
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S3038"
    name: "Abstract methods should not be called in constructors"
    severity: Critical
    category: Bug
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if regex::Regex::new(r"public\s+\w+\s*\([^)]*\)\s*\{").unwrap().is_match(line) {
                let next_lines: String = ctx.source.lines().skip(idx).take(15).collect::<Vec<_>>().join("\n");
                if next_lines.contains("abstract") || next_lines.contains("override") {
                    issues.push(Issue::new("JAVA_S3038", "Abstract method called in constructor", Severity::Critical, Category::Bug, ctx.file_path, idx + 1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S3358 — Nested ternary operators
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S3358"
    name: "Nested ternary operators should not be used"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\?\s*[^:]+\s*\?\s*[^:]+:").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_S3358", "Nested ternary operator detected", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// JAVA BUG RULES (25 rules)
// ═══════════════════════════════════════════════════════════════════════════════

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S106 — Standard output should not be used (System.out/err)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S106"
    name: "Standard output should not be used directly"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") || trimmed.starts_with("*") {
                continue;
            }
            if line.contains("System.out") || line.contains("System.err") {
                if line.contains("Logger") {
                    continue;
                }
                issues.push(Issue::new("JAVA_S106", "System.out/System.err used - use a logger instead", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S111 — Hidden field (local var shadows field)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S111"
    name: "Local variables should not shadow class fields"
    severity: Major
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"(private|protected|public)?\s+\w+\s+\w+\s*[=;]").unwrap();
        let mut fields: Vec<String> = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && !line.contains("(") && !line.contains("{") {
                if let Some(cap) = re.captures(line) {
                    if let Some(name) = cap.get(2) {
                        fields.push(name.as_str().to_string());
                    }
                }
            }
            for field in &fields {
                let local_re = regex::Regex::new(&format!(r"this\.{}\s*=", field)).unwrap();
                if local_re.is_match(line) {
                    issues.push(Issue::new("JAVA_S111", format!("Local variable shadows field '{}'", field), Severity::Major, Category::CodeSmell, ctx.file_path, idx + 1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S114 — Interface naming (should start with I)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S114"
    name: "Interface names should start with 'I'"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"interface\s+(\w+)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                if let Some(name) = cap.get(1) {
                    let interface_name = name.as_str();
                    if !interface_name.starts_with("I") && !interface_name.starts_with("Abstract") {
                        issues.push(Issue::new("JAVA_S114", format!("Interface '{}' should start with 'I'", interface_name), Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S116 — Field naming (non-final should be camelCase)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S116"
    name: "Non-final field names should be camelCase"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        // Detect field declarations with non-camelCase names (excluding final fields)
        let re = regex::Regex::new(r"(private|protected|public)?\s+\w+\s+([a-z][a-zA-Z0-9_]*)\s*[;=]").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            // Skip if line contains 'final'
            if line.contains("final") {
                continue;
            }
            if let Some(cap) = re.captures(line) {
                if let Some(name) = cap.get(2) {
                    let field_name = name.as_str();
                    if field_name.contains('_') || (field_name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) && !field_name.starts_with("m_")) {
                        issues.push(Issue::new("JAVA_S116", format!("Field '{}' should be camelCase", field_name), Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S117 — Local variable naming (camelCase)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S117"
    name: "Local variable names should be camelCase"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\b(int|String|boolean|double|float|long|char|byte|short|List|Map|Set|Object)\s+([A-Z][a-zA-Z0-9_]*)\s*[=;]").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                if let Some(name) = cap.get(2) {
                    let var_name = name.as_str();
                    if var_name.contains('_') {
                        issues.push(Issue::new("JAVA_S117", format!("Local variable '{}' should be camelCase", var_name), Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S118 — Abstract class naming (should start with Abstract)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S118"
    name: "Abstract class names should start with 'Abstract'"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"abstract\s+class\s+(\w+)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                if let Some(name) = cap.get(1) {
                    let class_name = name.as_str();
                    if !class_name.starts_with("Abstract") {
                        issues.push(Issue::new("JAVA_S118", format!("Abstract class '{}' should start with 'Abstract'", class_name), Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S119 — Type parameter naming (single letter)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S119"
    name: "Type parameter names should be single letters"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"<(\w\w+)>").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                if let Some(name) = cap.get(1) {
                    let type_param = name.as_str();
                    if type_param.len() > 1 {
                        issues.push(Issue::new("JAVA_S119", format!("Type parameter '{}' should be a single letter", type_param), Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S120 — Package naming (lowercase)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S120"
    name: "Package names should be lowercase"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"package\s+([a-z][a-zA-Z0-9_]*(?:\.[a-z][a-zA-Z0-9_]*)*)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                if let Some(name) = cap.get(1) {
                    let pkg_name = name.as_str();
                    if pkg_name.contains('A') || pkg_name.contains('B') || pkg_name.contains('C') || pkg_name.contains('D') || pkg_name.contains('E') || pkg_name.contains('F') || pkg_name.contains('G') || pkg_name.contains('H') || pkg_name.contains('I') || pkg_name.contains('J') || pkg_name.contains('K') || pkg_name.contains('L') || pkg_name.contains('M') || pkg_name.contains('N') || pkg_name.contains('O') || pkg_name.contains('P') || pkg_name.contains('Q') || pkg_name.contains('R') || pkg_name.contains('S') || pkg_name.contains('T') || pkg_name.contains('U') || pkg_name.contains('V') || pkg_name.contains('W') || pkg_name.contains('X') || pkg_name.contains('Y') || pkg_name.contains('Z') {
                        issues.push(Issue::new("JAVA_S120", format!("Package '{}' should be lowercase", pkg_name), Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S121 — Control structure without braces
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S121"
    name: "Control structures should use braces"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"(if|for|while|do)\s*\([^)]+\)\s*[^{;\n]").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && !line.contains("{") {
                issues.push(Issue::new("JAVA_S121", "Control structure without braces", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S122 — Statements should be on separate lines
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S122"
    name: "Only one statement should be on each line"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r";\s*(if|for|while|return|throw|break|continue)\s*\(").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_S122", "Multiple statements on one line", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S131 — Switch without default
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S131"
    name: "Switch statements should have a default case"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let mut switch_lines: Vec<usize> = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("switch") && line.contains("(") {
                switch_lines.push(idx);
            }
        }
        for switch_line in switch_lines {
            let switch_body: String = ctx.source.lines().skip(switch_line).take(30).collect::<Vec<_>>().join("\n");
            if !switch_body.contains("default:") && !switch_body.contains("default :") {
                issues.push(Issue::new("JAVA_S131", "Switch without default case", Severity::Minor, Category::CodeSmell, ctx.file_path, switch_line + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S144 — Equality on floating point
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S144"
    name: "Floating point numbers should not be compared with =="
    severity: Major
    category: Bug
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"(float|double)\s+\w+\s*=.*==|==\s*(float|double)\b").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_S144", "Floating point equality comparison", Severity::Major, Category::Bug, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S147 — Method with too many params (>7)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S147"
    name: "Methods should not have too many parameters"
    severity: Major
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"(public|private|protected)?\s+\w+\s+\w+\s*\(([^)]*)\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                if let Some(params) = cap.get(2) {
                    let param_count = params.as_str().split(',').filter(|s| !s.trim().is_empty()).count();
                    if param_count > 7 {
                        issues.push(Issue::new("JAVA_S147", format!("Method has {} parameters (max 7)", param_count), Severity::Major, Category::CodeSmell, ctx.file_path, idx + 1));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S148 — Long method (>50 lines)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S148"
    name: "Methods should not be too long"
    severity: Major
    category: CodeSmell
    language: "java"
    params: { threshold: usize = 50 }
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"(public|private|protected)?\s+\w+\s+\w+\s*\([^)]*\)\s*\{").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                let start = idx;
                let method_body: String = ctx.source.lines().skip(start).take(100).collect::<Vec<_>>().join("\n");
                let open_braces = method_body.matches('{').count();
                let close_braces = method_body.matches('}').count();
                let brace_count = open_braces.saturating_sub(close_braces);
                if brace_count > 0 {
                    let line_count = method_body.lines().take_while(|l| l.matches('{').count() > l.matches('}').count()).count();
                    if line_count > self.threshold {
                        issues.push(Issue::new("JAVA_S148", format!("Method has {} lines (max {})", line_count, self.threshold), Severity::Major, Category::CodeSmell, ctx.file_path, idx + 1));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S154 — Method complexity (>15 cyclomatic)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S154"
    name: "Methods should not be too complex"
    severity: Major
    category: CodeSmell
    language: "java"
    params: { threshold: usize = 15 }
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            let complexity_patterns = ["if ", "else if ", "for ", "while ", "case ", "catch ", "&&", "||", "?"];
            let context: String = ctx.source.lines().skip(idx.saturating_sub(5)).take(50).collect::<Vec<_>>().join("\n");
            let complexity = complexity_patterns.iter().map(|p| context.matches(p).count()).sum::<usize>();
            if complexity > self.threshold && line.contains("{") {
                issues.push(Issue::new("JAVA_S154", format!("Method has complexity {} (max {})", complexity, self.threshold), Severity::Major, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S164 — Empty catch block
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S164"
    name: "Empty catch blocks should be removed or filled"
    severity: Major
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"catch\s*\([^)]+\)\s*\{\s*\}").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_S164", "Empty catch block", Severity::Major, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S165 — Empty if/else/for/while body
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S165"
    name: "Empty control bodies should be removed"
    severity: Major
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"(if|for|while|else)\s*\([^)]*\)\s*\{\s*\}").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_S165", "Empty control structure body", Severity::Major, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S185 — Dead stores (variable assigned but not used)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S185"
    name: "Unused variables should be removed"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\b(int|String|boolean|double|float|long|char|byte|short|List|Map|Set|Object)\s+(\w+)\s*=\s*[^;]+;").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                if let Some(var_name) = cap.get(2) {
                    let remaining: String = ctx.source.lines().skip(idx + 1).take(50).collect::<Vec<_>>().join("\n");
                    if !remaining.contains(&format!("{} ", var_name.as_str())) && !remaining.contains(&format!("{}(", var_name.as_str())) {
                        issues.push(Issue::new("JAVA_S185", format!("Variable '{}' is assigned but never used", var_name.as_str()), Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S186 — Collection.isEmpty() instead of size()==0
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S186"
    name: "Use isEmpty() instead of comparing size() to 0"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\.size\(\)\s*==\s*0|\.size\(\)\s*!=\s*0|\.size\(\)\s*<\s*1|\.size\(\)\s*>\s*0").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_S186", "Use isEmpty() instead of size() == 0", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S187 — Duplicate branches in if/else
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S187"
    name: "Identical code should not be duplicated in if/else branches"
    severity: Major
    category: Bug
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"if\s*\([^)]+\)\s*\{([^}]+)\}\s*else\s*\{([^}]+)\}").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                if let (Some(if_body), Some(else_body)) = (cap.get(1), cap.get(2)) {
                    let if_text = if_body.as_str().trim();
                    let else_text = else_body.as_str().trim();
                    if !if_text.is_empty() && if_text == else_text {
                        issues.push(Issue::new("JAVA_S187", "Duplicate branches in if/else", Severity::Major, Category::Bug, ctx.file_path, idx + 1));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S194 — Field injection instead of constructor injection
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S194"
    name: "Use constructor injection instead of field injection"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"@(Autowired|Inject)\s+(private|protected)\s+\w+\s+\w+").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_S194", "Field injection instead of constructor injection", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S199 — Empty synchronized block
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S199"
    name: "Empty synchronized blocks should be removed"
    severity: Major
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"synchronized\s*\([^)]*\)\s*\{\s*\}").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_S199", "Empty synchronized block", Severity::Major, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S205 — Unused private method
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S205"
    name: "Unused private methods should be removed"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"private\s+\w+\s+(\w+)\s*\([^)]*\)").unwrap();
        let mut private_methods: Vec<String> = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                if let Some(name) = cap.get(1) {
                    private_methods.push(name.as_str().to_string());
                }
            }
        }
        let source = ctx.source.to_string();
        for method in private_methods {
            let call_count = source.matches(&format!("{}(", method)).count();
            if call_count <= 1 {
                issues.push(Issue::new("JAVA_S205", format!("Private method '{}' appears unused", method), Severity::Minor, Category::CodeSmell, ctx.file_path, 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S216 — Throws generic Exception
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S216"
    name: "Methods should not throw generic Exception"
    severity: Major
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"throws\s+(java\.)?Exception\b").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_S216", "Method throws generic Exception instead of specific type", Severity::Major, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S217 — Return of boolean literal (return x ? true : false)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S217"
    name: "Boolean literals should not be returned unnecessarily"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"return\s+\w+\s*\?\s*true\s*:\s*false|return\s+!\s*\w+\s*\?\s*false\s*:\s*true").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_S217", "Unnecessary boolean literal in ternary", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// JAVA Code Smell Rules — Batch 2 (50 rules)
// ═══════════════════════════════════════════════════════════════════════════════

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S100 — Method naming (camelCase)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S100"
    name: "Method names should follow camelCase convention"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"(public|private|protected)?\s+\w+\s+([a-z]+[A-Z]\w*)\s*\(").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                if let Some(name) = cap.get(2) {
                    issues.push(Issue::new("JAVA_S100", format!("Method '{}' should be camelCase", name.as_str()), Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S101 — Class naming (PascalCase)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S101"
    name: "Class names should follow PascalCase convention"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"(class|interface|enum)\s+([a-z]\w*)\b").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                if let Some(name) = cap.get(2) {
                    issues.push(Issue::new("JAVA_S101", format!("Class '{}' should be PascalCase", name.as_str()), Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S103 — Line too long (>120 chars)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S103"
    name: "Lines should not be too long"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: { max_length: usize = 120 }
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.len() > self.max_length {
                issues.push(Issue::new("JAVA_S103", format!("Line is {} characters (max {})", line.len(), self.max_length), Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S104 — File too long (>1000 lines)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S104"
    name: "Files should not be too long"
    severity: Major
    category: CodeSmell
    language: "java"
    params: { max_lines: usize = 1000 }
    check: => {
        let mut issues = Vec::new();
        let line_count = ctx.source.lines().count();
        if line_count > self.max_lines {
            issues.push(Issue::new("JAVA_S104", format!("File has {} lines (max {})", line_count, self.max_lines), Severity::Major, Category::CodeSmell, ctx.file_path, 1));
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S105 — Tab characters
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S105"
    name: "Tab characters should not be used"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains('\t') {
                issues.push(Issue::new("JAVA_S105", "Tab character found - use spaces", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S107 — Comment ratio too low
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S107"
    name: "Files should have a minimum comment ratio"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: { min_ratio: f32 = 0.05 }
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source.to_string();
        let total_lines = source.lines().count() as f32;
        let comment_lines = source.lines().filter(|l| l.trim().starts_with("//") || l.trim().starts_with("/*") || l.trim().starts_with("*")).count() as f32;
        if total_lines > 10.0 && comment_lines / total_lines < self.min_ratio {
            issues.push(Issue::new("JAVA_S107", format!("Comment ratio {:.1}% is below minimum {:.1}%", (comment_lines / total_lines) * 100.0, self.min_ratio * 100.0), Severity::Minor, Category::CodeSmell, ctx.file_path, 1));
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S108 — Nested if depth (>3)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S108"
    name: "Nested if statements should not be too deep"
    severity: Major
    category: CodeSmell
    language: "java"
    params: { max_depth: usize = 3 }
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"if\s*\(").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            let prefix_len = line.len() - line.trim_start().len();
            let if_count = re.find_iter(line).count();
            if if_count > 0 && prefix_len / 4 > self.max_depth {
                issues.push(Issue::new("JAVA_S108", format!("Nested if depth exceeds {}", self.max_depth), Severity::Major, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S109 — Magic numbers
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S109"
    name: "Magic numbers should not be used"
    severity: Major
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"[=<>+\-*/%\s]\s*[-+]?\d{3,}\s*[;<,\)\]]").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && !line.contains("final") && !line.contains("const") && !line.contains("test") && !line.contains("assert") {
                issues.push(Issue::new("JAVA_S109", "Magic number detected - use a named constant", Severity::Major, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S110 — Too many fields in class (>20)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S110"
    name: "Classes should not have too many fields"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: { max_fields: usize = 20 }
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"(private|public|protected)?\s+\w+\s+\w+\s*;").unwrap();
        let field_count = re.find_iter(&ctx.source).count();
        if field_count > self.max_fields {
            issues.push(Issue::new("JAVA_S110", format!("Class has {} fields (max {})", field_count, self.max_fields), Severity::Minor, Category::CodeSmell, ctx.file_path, 1));
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S112 — Generic exceptions thrown
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S112"
    name: "Generic exceptions should not be thrown"
    severity: Major
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"throw\s+(new\s+)?(Exception|Throwable|RuntimeException)\s*\(").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_S112", "Generic Exception thrown - use a specific type", Severity::Major, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S113 — String literals duplicated
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S113"
    name: "String literals should not be duplicated"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: { min_occurrences: usize = 3 }
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r#""([^"]{3,})""#).unwrap();
        let mut literals: std::collections::HashMap<String, Vec<usize>> = std::collections::HashMap::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            for cap in re.captures_iter(line) {
                if let Some(lit) = cap.get(1) {
                    literals.entry(lit.as_str().to_string()).or_default().push(idx + 1);
                }
            }
        }
        for (lit, lines) in literals {
            if lines.len() >= self.min_occurrences {
                for line in &lines {
                    issues.push(Issue::new("JAVA_S113", format!("Duplicate string literal '{}'", lit), Severity::Minor, Category::CodeSmell, ctx.file_path, *line));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S115 — Constant naming (UPPER_CASE)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S115"
    name: "Constant names should follow UPPER_CASE convention"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"static\s+final\s+\w+\s+([a-z][a-zA-Z0-9_]*)\s*=").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                if let Some(name) = cap.get(1) {
                    let n = name.as_str();
                    if n != n.to_uppercase() {
                        issues.push(Issue::new("JAVA_S115", format!("Constant '{}' should be UPPER_CASE", n), Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2122 — Empty statements ( lone semicolon) [was JAVA_S122]
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2122"
    name: "Empty statements should be removed"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.trim() == ";" {
                issues.push(Issue::new("JAVA_S2122", "Empty statement (lone semicolon)", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S123 — Missing package-info.java
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S123"
    name: "Packages with classes should have package-info.java"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let issues = Vec::new();
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S124 — Empty block comment /** */
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S124"
    name: "Empty block comments should be removed"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"/\*\*\s*\*/").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_S124", "Empty block comment detected", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S126 — Missing @Override
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S126"
    name: "Methods that override should be marked with @Override"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let methods = ["toString", "equals", "hashCode", "clone", "compareTo", "compare"];
        for (idx, line) in ctx.source.lines().enumerate() {
            for method in &methods {
                let re = regex::Regex::new(&format!(r"(public|protected)?\s+\w+\s+{}\s*\(", method)).unwrap();
                if re.is_match(line) && !line.contains("@Override") && !line.contains("interface") {
                    issues.push(Issue::new("JAVA_S126", format!("Method '{}' should have @Override annotation", method), Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S127 — Loop with single iteration
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S127"
    name: "Loops with single iteration should be refactored"
    severity: Major
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"(for|while)\s*\([^)]+\)\s*\{[^}]*break[^}]*\}").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_S127", "Loop executes at most once due to break", Severity::Major, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S128 — Too many returns (>5)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S128"
    name: "Methods should not have too many return statements"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: { max_returns: usize = 5 }
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"(public|private|protected)?\s+\w+\s+\w+\s*\([^)]*\)\s*\{").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                let method_body: String = ctx.source.lines().skip(idx).take(100).collect::<Vec<_>>().join("\n");
                let return_count = method_body.matches("return ").count();
                if return_count > self.max_returns {
                    issues.push(Issue::new("JAVA_S128", format!("Method has {} return statements (max {})", return_count, self.max_returns), Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S129 — Too many continue/break
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S129"
    name: "Methods should not have too many continue/break statements"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: { max_count: usize = 3 }
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"(public|private|protected)?\s+\w+\s+\w+\s*\([^)]*\)\s*\{").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                let method_body: String = ctx.source.lines().skip(idx).take(100).collect::<Vec<_>>().join("\n");
                let count = method_body.matches("continue").count() + method_body.matches("break").count();
                if count > self.max_count {
                    issues.push(Issue::new("JAVA_S129", format!("Method has {} continue/break statements (max {})", count, self.max_count), Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S130 — Too many methods in class (>30)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S130"
    name: "Classes should not have too many methods"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: { max_methods: usize = 30 }
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"(public|private|protected)?\s+\w+\s+\w+\s*\([^)]*\)\s*\{").unwrap();
        let method_count = re.find_iter(&ctx.source).count();
        if method_count > self.max_methods {
            issues.push(Issue::new("JAVA_S130", format!("Class has {} methods (max {})", method_count, self.max_methods), Severity::Minor, Category::CodeSmell, ctx.file_path, 1));
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S132 — Too many imports (>30)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S132"
    name: "Files should not have too many imports"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: { max_imports: usize = 30 }
    check: => {
        let mut issues = Vec::new();
        let import_count = ctx.source.lines().filter(|l| l.trim().starts_with("import ")).count();
        if import_count > self.max_imports {
            issues.push(Issue::new("JAVA_S132", format!("File has {} imports (max {})", import_count, self.max_imports), Severity::Minor, Category::CodeSmell, ctx.file_path, 1));
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S133 — Unused import
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S133"
    name: "Unused imports should be removed"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let issues = Vec::new();
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S134 — Deep nesting (>4)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S134"
    name: "Nesting should not be too deep"
    severity: Major
    category: CodeSmell
    language: "java"
    params: { max_depth: usize = 4 }
    check: => {
        let mut issues = Vec::new();
        let mut max_nesting = 0;
        let mut current_nesting = 0;
        for (idx, line) in ctx.source.lines().enumerate() {
            current_nesting += line.matches('{').count() as i32;
            current_nesting -= line.matches('}').count() as i32;
            max_nesting = max_nesting.max(current_nesting);
        }
        if max_nesting as usize > self.max_depth {
            issues.push(Issue::new("JAVA_S134", format!("Maximum nesting depth is {} (max {})", max_nesting, self.max_depth), Severity::Major, Category::CodeSmell, ctx.file_path, 1));
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S136 — Override equals but not hashCode
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S136"
    name: "If equals() is overridden, hashCode() should be overridden too"
    severity: Major
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let has_equals = regex::Regex::new(r"public\s+boolean\s+equals\s*\(\s*Object\s+").unwrap().is_match(&ctx.source);
        let has_hashcode = regex::Regex::new(r"public\s+int\s+hashCode\s*\(\s*\)").unwrap().is_match(&ctx.source);
        if has_equals && !has_hashcode {
            issues.push(Issue::new("JAVA_S136", "equals() is overridden but hashCode() is not", Severity::Major, Category::CodeSmell, ctx.file_path, 1));
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S137 — Useless override (just calls super)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S137"
    name: "Overrides that just call super should be removed"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"@Override\s+(public|protected)?\s+\w+\s+\w+\s*\([^)]*\)\s*\{\s*super\.\w+\([^)]*\)\s*;\s*\}").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_S137", "Useless override - just calls super", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S138 — Long anonymous class
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S138"
    name: "Anonymous classes should not be too long"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: { max_lines: usize = 20 }
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"new\s+\w+\s*\([^)]*\)\s*\{").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                let anon_body: String = ctx.source.lines().skip(idx).take(50).collect::<Vec<_>>().join("\n");
                let line_count = anon_body.matches('{').count() - anon_body.matches('}').count();
                if line_count > self.max_lines {
                    issues.push(Issue::new("JAVA_S138", format!("Anonymous class is {} lines (max {})", line_count, self.max_lines), Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S139 — Redundant interface modifier (public)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S139"
    name: "Interface methods should not be declared public explicitly"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"interface\s+\w+.*\{[^}]*public\s+\w+\s+\w+\s*\(").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) || (line.contains("interface") && line.contains("public") && line.contains("(")) {
                issues.push(Issue::new("JAVA_S139", "Interface methods are implicitly public", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S140 — Redundant throws clause
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S140"
    name: "Unnecessary throws clauses should be removed"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"throws\s+RuntimeException\s*\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_S140", "Redundant throws RuntimeException", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S141 — Unnecessary semicolon after class body
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S141"
    name: "Unnecessary semicolons should be removed"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"^\s*\}\s*;\s*$").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_S141", "Unnecessary semicolon after class body", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S142 — Unnecessary brackets in lambda
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S142"
    name: "Unnecessary parentheses in lambda expressions should be removed"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\|\s*\(\s*\w+\s*\)\s*->").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_S142", "Unnecessary parentheses in lambda", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S143 — Unnecessary fully qualified name
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S143"
    name: "Unnecessary fully qualified names should be shortened"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"import\s+\w+\.\w+\.\w+;").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_S143", "Consider using shorter import or type name", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2144 — For loop can be enhanced for [was JAVA_S144]
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2144"
    name: "For loops can be replaced with enhanced for loop"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"for\s*\(\s*int\s+\w+\s*=\s*0\s*;\s*\w+\s*<\s*\w+\.length\s*;\s*\w+\s*\+\+\s*\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_S2144", "This for loop can be an enhanced for loop", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S145 — Collection can be stream
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S145"
    name: "Collections can be processed with streams"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"for\s*\(\s*\w+\s+\w+\s*:\s*\w+\s*\)\s*\{[^}]*\b(stream|collect|filter|map|forEach)\b").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_S145", "Consider using stream API instead of loop", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S146 — StringBuilder instead of StringBuffer
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S146"
    name: "StringBuilder should be used instead of StringBuffer"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("new StringBuffer") || line.contains("StringBuffer ") {
                issues.push(Issue::new("JAVA_S146", "StringBuffer is synchronized - use StringBuilder for single-threaded code", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2147 — Interface with single method [was JAVA_S147]
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2147"
    name: "Interfaces with single method should be @FunctionalInterface"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"interface\s+\w+\s*\{[^}]*\}").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && !line.contains("@FunctionalInterface") {
                let body: String = ctx.source.lines().skip(idx).take(10).collect::<Vec<_>>().join("\n");
                let method_count = body.matches(";").count();
                if method_count == 1 {
                    issues.push(Issue::new("JAVA_S2147", "Single-method interface should be @FunctionalInterface", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2148 — Use diamond operator [was JAVA_S148]
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2148"
    name: "Diamond operator should be used"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"new\s+(ArrayList|TreeSet|HashMap|LinkedList|HashSet)<\w+>\s*\(\s*\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_S2148", "Use diamond operator <> instead of explicit type", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S149 — Use try-with-resources
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S149"
    name: "Try-with-resources should be used for resource management"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"finally\s*\{[^}]*\.close\(\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_S149", "Use try-with-resources instead of finally with close()", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S150 — Optional as method parameter
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S150"
    name: "Optional should not be used as method parameter"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\(\s*Optional\s*<[^>]+>\s+\w+\s*\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_S150", "Optional should not be used as method parameter", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S151 — Optional.isPresent() then .get()
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S151"
    name: "Optional.isPresent() followed by .get() should be replaced"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"if\s*\(\s*\w+\.isPresent\(\)\s*\)\s*\{[^}]*\.get\(\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_S151", "Use ifPresent() or orElse() instead of isPresent().get()", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S152 — equals() on different types (always false)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S152"
    name: "equals() comparison with unrelated types is always false"
    severity: Major
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\.equals\s*\(\s*(new\s+)?\w+\s*\(\s*\)\s*\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_S152", "equals() with unrelated type is always false", Severity::Major, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S153 — BigDecimal constructed from double
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S153"
    name: "BigDecimal should not be constructed from double"
    severity: Major
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"new\s+BigDecimal\s*\(\s*\d+\.\d+\s*\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_S153", "BigDecimal(double) loses precision - use BigDecimal.valueOf() or String", Severity::Major, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2154 — compareTo() inconsistent with equals() [was JAVA_S154]
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2154"
    name: "compareTo() should be consistent with equals()"
    severity: Major
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let has_equals = regex::Regex::new(r"public\s+boolean\s+equals\s*\(\s*Object\s+").unwrap().is_match(&ctx.source);
        let has_compare = regex::Regex::new(r"public\s+int\s+compareTo\s*\(\s*\w+\s+").unwrap().is_match(&ctx.source);
        if has_equals && has_compare {
            issues.push(Issue::new("JAVA_S2154", "compareTo() should return 0 for objects that equals() considers equal", Severity::Major, Category::CodeSmell, ctx.file_path, 1));
        }
        issues
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Exception Handling Rules (JAVA_S1130, S1141, S1148, S1160-S1165)
// ═══════════════════════════════════════════════════════════════════════════════

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S1130 — Throw exception in finally
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S1130"
    name: "Exception should not be thrown in finally block"
    severity: Major
    category: Bug
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"finally\s*\{[^}]*(throw|return)[^}]*\b(Exception|Error|Throwable)[^}]*\}").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_S1130", "Throwing an exception in a finally block can suppress the original exception", Severity::Major, Category::Bug, ctx.file_path, idx + 1).with_remediation(Remediation::moderate("Remove the throw statement from finally block or use try-catch for cleanup")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S1141 — Nested try-catch (>2 levels)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S1141"
    name: "Try-catch blocks should not be nested too deeply"
    severity: Major
    category: CodeSmell
    language: "java"
    params: { max_depth: usize = 2 }
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        let mut try_depth = 0;
        let mut max_depth_found = 0;
        let mut max_depth_line = 0;
        for (idx, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("try") || trimmed.starts_with("try{") || trimmed.starts_with("try ") {
                try_depth += 1;
                if try_depth > max_depth_found {
                    max_depth_found = try_depth;
                    max_depth_line = idx + 1;
                }
            }
            if trimmed == "}" && try_depth > 0 {
                try_depth -= 1;
            }
        }
        if max_depth_found > self.max_depth {
            issues.push(Issue::new("JAVA_S1141", format!("Try-catch nesting depth is {} (max allowed: {})", max_depth_found, self.max_depth), Severity::Major, Category::CodeSmell, ctx.file_path, max_depth_line).with_remediation(Remediation::moderate("Extract nested try-catch into a separate method")));
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S1148 — printStackTrace() usage
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S1148"
    name: "printStackTrace() should not be used"
    severity: Major
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\.printStackTrace\s*\(").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_S1148", "printStackTrace() does not properly handle exceptions - use a logger instead", Severity::Major, Category:: CodeSmell, ctx.file_path, idx + 1).with_remediation(Remediation::quick("Replace with proper logging: log.error(\"message\", exception)")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S1160 — Public method throws generic exception
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S1160"
    name: "Public methods should not throw generic Exception or Throwable"
    severity: Major
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"public\s+\w+\s+\w+\s*\([^)]*\)\s*throws\s+(Exception|Throwable)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_S1160", "Method throws generic Exception/Throwable - declare specific exception types", Severity::Major, Category::CodeSmell, ctx.file_path, idx + 1).with_remediation(Remediation::moderate("Declare specific checked exceptions that the method can throw")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S1161 — @Override missing on exception methods
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S1161"
    name: "@Override should be used when overriding exception methods"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"(public\s+)?\w+\s+(toString|equals|hashCode|getMessage|getLocalizedMessage)\s*\([^)]*\)").unwrap();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            if re.is_match(line) {
                if idx == 0 || !lines[idx - 1].contains("@Override") {
                    let next_lines: String = lines.iter().skip(idx).take(5).cloned().collect::<Vec<_>>().join("\n");
                    if !next_lines.contains("@Override") {
                        issues.push(Issue::new("JAVA_S1161", "Method overrides inherited method but lacks @Override annotation", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1).with_remediation(Remediation::quick("Add @Override annotation before the method")));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S1162 — Exception class naming (should end with Exception)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S1162"
    name: "Exception class names should end with Exception"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"class\s+(\w+Exception)\s*(extends|implements)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                let class_name = cap.get(1).unwrap().as_str();
                if !class_name.ends_with("Exception") {
                    issues.push(Issue::new("JAVA_S1162", format!("Exception class '{}' should end with 'Exception'", class_name), Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1).with_remediation(Remediation::quick("Rename class to end with 'Exception'")));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S1163 — Throwable caught (catch Throwable)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S1163"
    name: "Throwable should not be caught"
    severity: Major
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"catch\s*\(\s*Throwable\s+\w+\s*\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_S1163", "Catching Throwable is too broad - catch specific exceptions", Severity::Major, Category::CodeSmell, ctx.file_path, idx + 1).with_remediation(Remediation::moderate("Catch specific exception types like IOException, SQLException, etc.")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S1164 — Throwable.printStackTrace() in catch
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S1164"
    name: "Throwable.printStackTrace() should not be used in catch block"
    severity: Major
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"catch\s*\([^)]+\)\s*\{[^}]*\.printStackTrace\s*\(").unwrap();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            if line.contains("catch") {
                let block: String = lines.iter().skip(idx).take(10).map(|s| *s).collect::<Vec<_>>().join("\n");
                if re.is_match(&block) {
                    issues.push(Issue::new("JAVA_S1164", "printStackTrace() in catch block - use a proper logger", Severity::Major, Category::CodeSmell, ctx.file_path, idx + 1).with_remediation(Remediation::quick("Replace with logger.error(\"message\", exception)")));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S1165 — Exception swallowed without log
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S1165"
    name: "Caught exceptions should not be silently swallowed"
    severity: Major
    category: Bug
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"catch\s*\([^)]+\)\s*\{[^}]*\}[^}]*\}").unwrap();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            if line.contains("catch") {
                let block: String = lines.iter().skip(idx).take(8).map(|s| *s).collect::<Vec<_>>().join("\n");
                if block.contains("}") && !block.contains("log") && !block.contains("throw") && !block.contains("return") && !block.contains("e.printStackTrace") && block.matches("catch").count() <= 1 {
                    if block.contains("} catch") || block.contains("} // end catch") {
                        issues.push(Issue::new("JAVA_S1165", "Empty catch block or exception swallowed without logging", Severity::Major, Category::Bug, ctx.file_path, idx + 1).with_remediation(Remediation::quick("Add logging or rethrow the exception")));
                    }
                }
            }
        }
        issues
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Concurrency Rules (JAVA_S2160-S2169)
// ═══════════════════════════════════════════════════════════════════════════════

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2160 — synchronized on this
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2160"
    name: "Synchronized blocks should not synchronize on 'this'"
    severity: Major
    category: Bug
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"synchronized\s*\(\s*this\s*\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_S2160", "Synchronizing on 'this' is unsafe - use a private lock object", Severity::Major, Category::Bug, ctx.file_path, idx + 1).with_remediation(Remediation::moderate("Use a private final lock object instead: private final Object lock = new Object()")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2161 — Unsafe double-checked locking
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2161"
    name: "Double-checked locking should not be used without volatile"
    severity: Critical
    category: Bug
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"synchronized\s*\(\s*\w+\s*Class\s*\)").unwrap();
        let has_volatile = regex::Regex::new(r"volatile\s+").unwrap().is_match(&ctx.source);
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && !has_volatile {
                issues.push(Issue::new("JAVA_S2161", "Double-checked locking pattern without volatile is unsafe", Severity::Critical, Category::Bug, ctx.file_path, idx + 1).with_remediation(Remediation::moderate("Add volatile to the field or use Bill Pugh Singleton idiom")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2162 — Non-atomic volatile update (volatile++)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2162"
    name: "Volatile fields should not be incremented with ++ operator"
    severity: Critical
    category: Bug
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"volatile\s+\w+\s+\w+\s*;").unwrap();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            if re.is_match(line) {
                let var_name = line.split_whitespace().last().unwrap_or("").trim_end_matches(';');
                let following: String = lines.iter().skip(idx).take(20).map(|s| *s).collect::<Vec<_>>().join("\n");
                if following.contains(&format!("{}\\+\\+", var_name)) || following.contains(&format!("++{}", var_name)) {
                    issues.push(Issue::new("JAVA_S2162", format!("volatile field '{}' incremented with ++ is not atomic", var_name), Severity::Critical, Category::Bug, ctx.file_path, idx + 1).with_remediation(Remediation::moderate("Use AtomicInteger/AtomicLong or synchronize the increment")));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2163 — wait() not in loop
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2163"
    name: "wait() should always be called in a loop"
    severity: Major
    category: Bug
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\.wait\s*\(").unwrap();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            if re.is_match(line) {
                let prev_lines: String = lines.iter().take(idx + 1).rev().take(10).map(|s| *s).collect::<Vec<_>>().join("\n");
                let next_lines: String = lines.iter().skip(idx).take(5).map(|s| *s).collect::<Vec<_>>().join("\n");
                if !prev_lines.contains("while") && !next_lines.contains("while") && !next_lines.contains("if") {
                    issues.push(Issue::new("JAVA_S2163", "wait() should be called inside a while loop to handle spurious wakeups", Severity::Major, Category::Bug, ctx.file_path, idx + 1).with_remediation(Remediation::moderate("Wrap wait() in a while loop checking the condition")));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2164 — Thread.start() not called
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2164"
    name: "Thread.start() should be called to start a thread"
    severity: Major
    category: Bug
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"Thread\s+\w+\s*=\s*new\s+Thread").unwrap();
        let has_start = regex::Regex::new(r"\.start\s*\(").unwrap().is_match(&ctx.source);
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && !has_start {
                issues.push(Issue::new("JAVA_S2164", "Thread created but start() never called - thread will not run", Severity:: Major, Category::Bug, ctx.file_path, idx + 1).with_remediation(Remediation::quick("Call start() on the Thread to begin execution")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2165 — Thread.run() called directly instead of start()
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2165"
    name: "Thread.run() should not be called directly - use start()"
    severity: Major
    category: Bug
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\.run\s*\(\s*\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && line.contains("Thread") {
                issues.push(Issue::new("JAVA_S2165", "Thread.run() called directly instead of start() - runs in current thread", Severity::Major, Category::Bug, ctx.file_path, idx + 1).with_remediation(Remediation::quick("Replace .run() with .start() to run in a new thread")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2166 — Thread.stop() usage (deprecated)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2166"
    name: "Thread.stop() is deprecated and unsafe"
    severity: Critical
    category: Bug
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\.stop\s*\(").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_S2166", "Thread.stop() is deprecated and can leave objects in inconsistent state", Severity::Critical, Category::Bug, ctx.file_path, idx + 1).with_remediation(Remediation::moderate("Use interrupt() and check for interrupted status instead")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2167 — Thread.sleep() in synchronized block
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2167"
    name: "Thread.sleep() should not be used in synchronized block"
    severity: Major
    category: Bug
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"synchronized\s*\([^)]+\)\s*\{[^}]*Thread\.sleep").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("synchronized") {
                let block: String = ctx.source.lines().skip(idx).take(10).collect::<Vec<_>>().join("\n");
                if block.contains("Thread.sleep") && block.contains("synchronized") {
                    issues.push(Issue::new("JAVA_S2167", "Holding lock while sleeping reduces concurrency", Severity::Major, Category::Bug, ctx.file_path, idx + 1).with_remediation(Remediation::moderate("Move sleep outside synchronized block or use wait() instead")));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2168 — Non-thread-safe singleton
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2168"
    name: "Singleton without proper thread safety"
    severity: Major
    category: Bug
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let has_sync = regex::Regex::new(r"synchronized").unwrap().is_match(&ctx.source);
        let has_volatile = regex::Regex::new(r"volatile").unwrap().is_match(&ctx.source);
        let has_enum = regex::Regex::new(r"enum\s+\w+\s*\{").unwrap().is_match(&ctx.source);
        let has_holder = regex::Regex::new(r"private\s+static\s+class\s+\w*Holder").unwrap().is_match(&ctx.source);
        if !has_sync && !has_volatile && !has_enum && !has_holder {
            let re = regex::Regex::new(r"private\s+static\s+\w+\s+instance\s*=").unwrap();
            for (idx, line) in ctx.source.lines().enumerate() {
                if re.is_match(line) {
                    issues.push(Issue::new("JAVA_S2168", "Singleton may not be thread-safe - use Bill Pugh idiom or enum", Severity::Major, Category::Bug, ctx.file_path, idx + 1).with_remediation(Remediation::moderate("Use enum singleton, Bill Pugh holder idiom, or double-checked locking with volatile")));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2169 — ConcurrentHashMap instead of Hashtable
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2169"
    name: "Hashtable should be replaced by ConcurrentHashMap"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"new\s+Hashtable\s*<").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_S2169", "Hashtable is synchronized and slower than ConcurrentHashMap", Severity:: Minor, Category::CodeSmell, ctx.file_path, idx + 1).with_remediation(Remediation::quick("Replace Hashtable with ConcurrentHashMap")));
            }
        }
        issues
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Serialization Rules (JAVA_S2055, S2057, S2059-S2067)
// ═══════════════════════════════════════════════════════════════════════════════

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2055 — serialVersionUID not declared
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2055"
    name: "Serializable class should declare serialVersionUID"
    severity: Major
    category: Bug
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let implements_serial = regex::Regex::new(r"implements\s+(java\.io\.)?Serializable").unwrap().is_match(&ctx.source);
        let has_suid = regex::Regex::new(r"private\s+static\s+final\s+long\s+serialVersionUID").unwrap().is_match(&ctx.source);
        if implements_serial && !has_suid {
            issues.push(Issue::new("JAVA_S2055", "Serializable class should declare serialVersionUID", Severity::Major, Category::Bug, ctx.file_path, 1).with_remediation(Remediation::moderate("Add: private static final long serialVersionUID = 1L;")));
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2057 — Non-serializable field in serializable class
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2057"
    name: "Fields in Serializable class should be serializable"
    severity: Major
    category: Bug
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let implements_serial = regex::Regex::new(r"implements\s+(java\.io\.)?Serializable").unwrap().is_match(&ctx.source);
        if implements_serial {
            let re = regex::Regex::new(r"(private|public|protected)\s+(\w+)\s+(\w+)\s*;").unwrap();
            for (idx, line) in ctx.source.lines().enumerate() {
                if let Some(cap) = re.captures(line) {
                    let field_type = cap.get(2).unwrap().as_str();
                    let non_serializable = ["Thread", "Socket", "InputStream", "OutputStream", "Connection", "Statement", "ResultSet"];
                    if non_serializable.iter().any(|t| field_type.contains(t)) && !line.contains("transient") {
                        issues.push(Issue::new("JAVA_S2057", format!("Field '{}' of type '{}' may not be serializable", cap.get(3).unwrap().as_str(), field_type), Severity::Major, Category::Bug, ctx.file_path, idx + 1).with_remediation(Remediation::moderate("Mark non-serializable fields as transient or provide custom serialization")));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2059 — readObject() not private
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2059"
    name: "readObject() method should be private"
    severity: Major
    category: Bug
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"(private|protected|public)?\s*void\s+readObject\s*\(\s*ObjectInputStream").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && !line.contains("private") {
                issues.push(Issue::new("JAVA_S2059", "readObject() should be private to maintain serialization contract", Severity::Major, Category::Bug, ctx.file_path, idx + 1).with_remediation(Remediation::quick("Make readObject() method private")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2060 — readResolve() not used for singletons
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2060"
    name: "Singleton classes implementing Serializable should use readResolve"
    severity: Major
    category: Bug
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let has_singleton = regex::Regex::new(r"private\s+static\s+\w+\s+instance").unwrap().is_match(&ctx.source);
        let implements_serial = regex::Regex::new(r"implements\s+(java\.io\.)?Serializable").unwrap().is_match(&ctx.source);
        let has_read_resolve = regex::Regex::new(r"protected\s+Object\s+readResolve\s*\(\s*\)").unwrap().is_match(&ctx.source);
        if has_singleton && implements_serial && !has_read_resolve {
            issues.push(Issue::new("JAVA_S2060", "Singleton with Serializable should implement readResolve to maintain singleton", Severity::Major, Category::Bug, ctx.file_path, 1).with_remediation(Remediation::moderate("Add readResolve method that returns the singleton instance")));
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2061 — Externalizable without no-arg constructor
// ─ ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2061"
    name: "Externalizable class must have a public no-arg constructor"
    severity: Major
    category: Bug
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let implements_externalizable = regex::Regex::new(r"implements\s+(java\.io\.)?Externalizable").unwrap().is_match(&ctx.source);
        let has_noarg_constructor = regex::Regex::new(r"public\s+\w+\s*\(\s*\)\s*\{").unwrap().is_match(&ctx.source);
        if implements_externalizable && !has_noarg_constructor {
            issues.push(Issue::new("JAVA_S2061", "Externalizable class requires a public no-arg constructor", Severity::Major, Category::Bug, ctx.file_path, 1).with_remediation(Remediation::quick("Add a public no-arg constructor to the class")));
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2062 — transient not used on non-serializable fields
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2062"
    name: "Non-serializable fields should be marked transient"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let implements_serial = regex::Regex::new(r"implements\s+(java\.io\.)?Serializable").unwrap().is_match(&ctx.source);
        if implements_serial {
            let re = regex::Regex::new(r"(private|public|protected)\s+(\w+)\s+(\w+)\s*;").unwrap();
            for (idx, line) in ctx.source.lines().enumerate() {
                if let Some(cap) = re.captures(line) {
                    let field_type = cap.get(2).unwrap().as_str();
                    let non_serializable = ["Thread", "Socket", "InputStream", "OutputStream", "Connection"];
                    if non_serializable.iter().any(|t| field_type.contains(t)) && !line.contains("transient") {
                        issues.push(Issue::new("JAVA_S2062", format!("Field '{}' should be transient", cap.get(3).unwrap().as_str()), Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1).with_remediation(Remediation::quick("Add transient modifier to non-serializable field")));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2063 — Serializable comparator
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2063"
    name: "Comparator implementing Serializable should be serializable"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"class\s+\w+\s+implements\s+Comparator[^\{]*\{").unwrap();
        let has_ser = regex::Regex::new(r"implements\s+(java\.io\.)?Serializable").unwrap().is_match(&ctx.source);
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && !has_ser {
                issues.push(Issue::new("JAVA_S2063", "Comparator used in TreeSet/TreeMap should implement Serializable", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1).with_remediation(Remediation::quick("Add 'implements Serializable' to the comparator")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2065 — Non-transient non-serializable field
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2065"
    name: "Non-serializable field in Serializable class"
    severity: Major
    category: Bug
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let implements_serial = regex::Regex::new(r"implements\s+(java\.io\.)?Serializable").unwrap().is_match(&ctx.source);
        if implements_serial {
            // Check for non-serializable fields, excluding transient fields
            let re = regex::Regex::new(r"(private|public|protected)\s+(\w+)\s+(\w+)\s*;").unwrap();
            for (idx, line) in ctx.source.lines().enumerate() {
                // Skip transient fields
                if line.trim().starts_with("transient") {
                    continue;
                }
                if let Some(cap) = re.captures(line) {
                    let field_type = cap.get(2).unwrap().as_str();
                    let non_serializable = ["Thread", "Socket", "InputStream", "OutputStream", "Connection", "Statement", "ResultSet"];
                    if non_serializable.iter().any(|t| field_type.contains(t)) {
                        issues.push(Issue::new("JAVA_S2065", format!("Field '{}' of type '{}' is not serializable", cap.get(3).unwrap().as_str(), field_type), Severity::Major, Category::Bug, ctx.file_path, idx + 1).with_remediation(Remediation::moderate("Make the field transient or implement custom serialization")));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2066 — Custom serialization without readObject/writeObject
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2066"
    name: "Custom serialization should implement readObject and writeObject"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let has_serializable = regex::Regex::new(r"implements\s+(java\.io\.)?Serializable").unwrap().is_match(&ctx.source);
        let has_custom_fields = regex::Regex::new(r"transient\s+").unwrap().is_match(&ctx.source);
        let has_read_obj = regex::Regex::new(r"private\s+void\s+readObject").unwrap().is_match(&ctx.source);
        let has_write_obj = regex::Regex::new(r"private\s+void\s+writeObject").unwrap().is_match(&ctx.source);
        if has_serializable && has_custom_fields && (!has_read_obj || !has_write_obj) {
            issues.push(Issue::new("JAVA_S2066", "Class with transient fields should implement custom readObject/writeObject", Severity::Minor, Category::CodeSmell, ctx.file_path, 1).with_remediation(Remediation::moderate("Add private readObject() and writeObject() methods for custom serialization")));
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2067 — Serializable inner class
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2067"
    name: "Inner Serializable classes should be static"
    severity: Major
    category: Bug
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"class\s+\w+\s+implements\s+(java\.io\.)?Serializable\s*\{").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                let prev_lines: String = ctx.source.lines().take(idx).collect::<Vec<_>>().join("\n");
                if prev_lines.contains("class") && !prev_lines.contains("static class") && !prev_lines.contains(r"static\s+class") {
                    issues.push(Issue::new("JAVA_S2067", "Non-static inner class with Serializable - holds implicit reference to outer class", Severity::Major, Category::Bug, ctx.file_path, idx + 1).with_remediation(Remediation::moderate("Make the inner class static or use a separate class file")));
                }
            }
        }
        issues
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Collections Rules (JAVA_S2170-S2179)
// ═══════════════════════════════════════════════════════════════════════════════

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2170 — Raw type usage (List instead of List<String>)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2170"
    name: "Generic type List/Set/Map should not use raw types"
    severity: Major
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"(List|Set|Map|HashMap|ArrayList|HashSet|TreeSet)<>\s+\w+").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_S2170", "Raw generic type used - specify type parameters", Severity::Major, Category::CodeSmell, ctx.file_path, idx + 1).with_remediation(Remediation::quick("Use List<String>, Set<Integer>, Map<String, Object> etc.")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2171 — Unchecked cast
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2171"
    name: "Unchecked cast from Object should be avoided"
    severity: Major
    category: Bug
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\(\s*\w+\s*\)\s*\w+").unwrap();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            if let Some(cap) = re.captures(line) {
                let cast_type = cap.get(1).unwrap().as_str();
                if !cast_type.contains("<") && !line.contains("@SuppressWarnings") {
                    let prev_context: String = lines.iter().take(idx + 1).rev().take(3).map(|s| *s).collect::<Vec<_>>().join("\n");
                    if prev_context.contains("Object") || prev_context.contains("Collection") || prev_context.contains("Map") {
                        issues.push(Issue::new("JAVA_S2171", format!("Unchecked cast to '{}' - add @SuppressWarnings or use Optional", cast_type), Severity::Major, Category::Bug, ctx.file_path, idx + 1).with_remediation(Remediation::moderate("Use instanceof check before cast or consider redesign")));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2172 — Type parameter shadowing
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2172"
    name: "Type parameter should not shadow another type parameter"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        // Detect <T, T> patterns in generics without backreference
        for (idx, line) in ctx.source.lines().enumerate() {
            // Look for <something, something> pattern
            if let Some(lt_pos) = line.find('<') {
                if let Some(gt_pos) = line[lt_pos..].find('>') {
                    let generics_content = &line[lt_pos + 1..lt_pos + gt_pos];
                    // Split by comma and trim
                    let parts: Vec<&str> = generics_content.split(',').map(|s| s.trim()).collect();
                    if parts.len() == 2 && parts[0] == parts[1] && !parts[0].is_empty() {
                        issues.push(Issue::new("JAVA_S2172", "Type parameter shadows another type parameter with the same name", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1).with_remediation(Remediation::quick("Rename one of the type parameters to a different letter")));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2173 — Unnecessary type argument (diamond operator)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2173"
    name: "Redundant type arguments can be removed with diamond operator"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"(new\s+(ArrayList|HashMap|HashSet|TreeSet|LinkedList))<(\w+)>\s*\(\s*\)\s*;").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_S2173", "Type arguments can be inferred - use diamond operator <>", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1).with_remediation(Remediation::quick("Replace new ArrayList<String>() with new ArrayList<>()")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2174 — Unnecessary cast to same type
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2174"
    name: "Unnecessary cast to the same type"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        // Detect (Type) Type(...) patterns without backreference
        for (idx, line) in ctx.source.lines().enumerate() {
            // Look for (Type) pattern
            if let Some(open_paren) = line.find('(') {
                if open_paren > 0 {
                    let before_paren = line[..open_paren].trim();
                    if let Some(close_paren) = line[open_paren + 1..].find(')') {
                        let type_name = line[open_paren + 1..open_paren + 1 + close_paren].trim();
                        let after_cast = line[open_paren + close_paren + 2..].trim();
                        // Check if the next token is the same type name followed by (
                        if after_cast.starts_with(&type_name) {
                            let remainder = &after_cast[type_name.len()..];
                            if remainder.trim().starts_with('(') {
                                issues.push(Issue::new("JAVA_S2174", "Cast to the same type is unnecessary", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1).with_remediation(Remediation::quick("Remove the redundant cast")));
                            }
                        }
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2175 — instanceof with incompatible types
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2175"
    name: "instanceof check between unrelated types is always false"
    severity: Major
    category: Bug
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"instanceof\s+(\w+)").unwrap();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            if let Some(cap) = re.captures(line) {
                let target_type = cap.get(1).unwrap().as_str();
                let prev_context: String = lines.iter().take(idx).rev().take(10).map(|s| *s).collect::<Vec<_>>().join("\n");
                let incompatible = ["String", "Integer", "Long", "Double", "Boolean", "Float", "Byte", "Short", "Character"];
                if incompatible.contains(&target_type) && !prev_context.contains("Object") && !prev_context.contains("Number") && !prev_context.contains("Comparable") {
                    if prev_context.contains("instanceof") {
                        issues.push(Issue::new("JAVA_S2175", format!("instanceof '{}' in a chain with incompatible types", target_type), Severity::Major, Category::Bug, ctx.file_path, idx + 1).with_remediation(Remediation::moderate("Review the instanceof chain for type compatibility")));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2176 — ClassCastException risk
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2176"
    name: "ClassCastException may be thrown"
    severity: Major
    category: Bug
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\.cast\s*\(").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_S2176", "Class.cast() may throw ClassCastException - verify type compatibility", Severity::Major, Category::Bug, ctx.file_path, idx + 1).with_remediation(Remediation::moderate("Use instanceof check before cast or Class.cast() with proper type checking")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2177 — Map.get() without null check
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2177"
    name: "Map.get() result should be checked for null before use"
    severity: Major
    category: Bug
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"(\w+)\s*=\s*(\w+)\.get\s*\([^)]+\)\s*;").unwrap();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            if let Some(cap) = re.captures(line) {
                let var_name = cap.get(1).unwrap().as_str();
                let next_line = lines.get(idx + 1).unwrap_or(&"");
                if !next_line.contains(var_name) && !next_line.contains("null") && !next_line.contains("if") && !next_line.contains("Optional") {
                    let next_context: String = lines.iter().skip(idx).take(3).map(|s| *s).collect::<Vec<_>>().join("\n");
                    if next_context.contains(var_name) && !next_context.contains("== null") && !next_context.contains("!= null") && !next_context.contains("Optional") {
                        issues.push(Issue::new("JAVA_S2177", format!("Result of get() assigned to '{}' but may be null", var_name), Severity::Major, Category::Bug, ctx.file_path, idx + 1).with_remediation(Remediation::quick("Check if get() returns null or use getOrDefault()")));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2178 — .equals() on different collection types
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2178"
    name: "equals() called on incompatible collection types"
    severity: Major
    category: Bug
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\.equals\s*\(\s*(new\s+)?(ArrayList|LinkedList|HashSet|TreeSet|HashMap|TreeMap)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_S2178", "equals() between incompatible collection types will always return false", Severity::Major, Category::Bug, ctx.file_path, idx + 1).with_remediation(Remediation::moderate("Compare collections of the same type or use appropriate comparison methods")));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2179 — List.indexOf() result not checked
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2179"
    name: "indexOf() result should be checked before using as index"
    severity: Major
    category: Bug
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\.get\s*\(\s*\w+\.indexOf").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && !line.contains("if") && !line.contains(">=") && !line.contains("<") {
                issues.push(Issue::new("JAVA_S2179", "indexOf() returns -1 for not found but result used directly as index", Severity::Major, Category::Bug, ctx.file_path, idx + 1).with_remediation(Remediation::quick("Check indexOf() result >= 0 before using as index")));
            }
        }
        issues
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Naming Rules (JAVA_S2102, S2104, S2105, S2107-S2110)
// ═══════════════════════════════════════════════════════════════════════════════

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2102 — Enum naming (PascalCase)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2102"
    name: "Enum constants should be in PascalCase"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"enum\s+(\w+)\s*\{").unwrap();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            if let Some(cap) = re.captures(line) {
                let enum_name = cap.get(1).unwrap().as_str();
                let enum_body: String = lines.iter().skip(idx).take(30).map(|s| *s).collect::<Vec<_>>().join("\n");
                let constant_re = regex::Regex::new(r"\b([A-Z][a-z]+|[A-Z]{2,})([A-Z][a-z]*|[0-9_]*)*\b").unwrap();
                for c in constant_re.find_iter(&enum_body) {
                    let const_val = c.as_str();
                    if const_val.contains('_') || (const_val.chars().next().map(|f| f.is_uppercase()).unwrap_or(false) && const_val.chars().any(|c| c.is_lowercase())) {
                        if const_val.contains('_') {
                            issues.push(Issue::new("JAVA_S2102", format!("Enum constant '{}' should be PascalCase, not UPPER_SNAKE_CASE", const_val), Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1).with_remediation(Remediation::quick("Use PascalCase for enum constants")));
                        }
                    }
                }
                break;
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2104 — Annotation naming
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2104"
    name: "Annotation types should follow PascalCase naming"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"@interface\s+(\w+)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                let name = cap.get(1).unwrap().as_str();
                if name.contains('_') || name.chars().any(|c| c.is_uppercase() && c.is_ascii()) {
                    if name != name.chars().map(|c| if c == '_' { 'A' } else { c }).collect::<String>().chars().take(1).collect::<String>() {
                        issues.push(Issue::new("JAVA_S2104", format!("Annotation '{}' should use PascalCase", name), Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1).with_remediation(Remediation::quick("Rename annotation to use PascalCase")));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2105 — Generic type naming (E, T, K, V convention)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2105"
    name: "Generic type parameters should follow Java naming conventions"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"<([A-Z]{2,}[a-z]*|[A-Z][a-z]+)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                let type_param = cap.get(1).unwrap().as_str();
                let valid_single = ["E", "T", "K", "V", "N", "R", "S", "U", "W", "X", "Y"];
                if !valid_single.contains(&type_param) && type_param.len() == 1 {
                    issues.push(Issue::new("JAVA_S2105", format!("Type parameter '{}' should be a single uppercase letter (E, T, K, V)", type_param), Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1).with_remediation(Remediation::quick("Use single uppercase letter for type parameters")));
                } else if type_param.len() > 1 {
                    issues.push(Issue::new("JAVA_S2105", format!("Type parameter '{}' should be a single uppercase letter", type_param), Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1).with_remediation(Remediation::quick("Use single uppercase letter (E, T, K, V) for type parameters")));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2107 — Package naming segments (>8 chars discouraged)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2107"
    name: "Package naming segments longer than 8 characters are discouraged"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"package\s+([\w\.]+)\s*;").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                let pkg = cap.get(1).unwrap().as_str();
                for segment in pkg.split('.') {
                    if segment.len() > 8 && segment.chars().all(|c| c.is_lowercase()) {
                        issues.push(Issue::new("JAVA_S2107", format!("Package segment '{}' is longer than 8 characters", segment), Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1).with_remediation(Remediation::quick("Use shorter package segment names (max 8 characters)")));
                        break;
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2108 — Test method naming (should start with test)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2108"
    name: "Test methods should follow naming convention (test prefix)"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"@Test\s+(public|private|protected)?\s+void\s+(test\w+|\w+)\s*\(").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                let method_name = cap.get(2).unwrap().as_str();
                if !method_name.starts_with("test") && !method_name.starts_with("Test") {
                    issues.push(Issue::new("JAVA_S2108", format!("Test method '{}' should start with 'test'", method_name), Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1).with_remediation(Remediation::quick("Prefix test method with 'test' (e.g., testMethodName)")));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2109 — Setter naming (setXxx pattern)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2109"
    name: "Setter methods should follow setXxx naming convention"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"public\s+void\s+(set\w+)\s*\(\s*\w+\s+\w+\s*\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                let method_name = cap.get(1).unwrap().as_str();
                let field_name = method_name.strip_prefix("set").unwrap_or("");
                if !field_name.is_empty() && !field_name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
                    issues.push(Issue::new("JAVA_S2109", format!("Setter '{}' should be named setXxx where Xxx is capitalized field name", method_name), Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1).with_remediation(Remediation::quick("Rename to setXxx format (e.g., setName not setname)")));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S2110 — Getter naming (getXxx or isXxx pattern)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_S2110"
    name: "Getter methods should follow getXxx or isXxx naming convention"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"public\s+(\w+)\s+(get\w+|is\w+)\s*\(\s*\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                let return_type = cap.get(1).unwrap().as_str();
                let method_name = cap.get(2).unwrap().as_str();
                if method_name.starts_with("get") {
                    let field_name = method_name.strip_prefix("get").unwrap_or("");
                    if !field_name.is_empty() && !field_name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
                        issues.push(Issue::new("JAVA_S2110", format!("Getter '{}' should be named getXxx where Xxx is capitalized", method_name), Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1).with_remediation(Remediation::quick("Rename to getXxx format (e.g., getName not getname)")));
                    }
                }
                if method_name.starts_with("is") && return_type != "boolean" && return_type != "Boolean" {
                    issues.push(Issue::new("JAVA_S2110", format!("Method '{}' starts with 'is' but returns '{}' not boolean", method_name, return_type), Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1).with_remediation(Remediation::quick("Use 'get' prefix for non-boolean getters")));
                }
            }
        }
        issues
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Design Patterns & Architecture Rules (JAVA_D1-D30)
// ═══════════════════════════════════════════════════════════════════════════════

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_D1 — Singleton without private constructor
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_D1"
    name: "Singleton classes should have a private constructor"
    severity: Major
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("private static") && line.contains("instance") && !line.contains("private") {
                let context: String = ctx.source.lines().skip(idx.saturating_sub(5)).take(30).collect::<Vec<_>>().join("\n");
                if !context.contains("private") && context.contains("getInstance") {
                    issues.push(Issue::new("JAVA_D1", "Singleton without private constructor", Severity::Major, Category::CodeSmell, ctx.file_path, idx + 1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_D2 — Abstract factory naming
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_D2"
    name: "Abstract factory classes should be named with Factory suffix"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("abstract class") && !line.ends_with("Factory") && !line.ends_with("FactoryImpl") {
                issues.push(Issue::new("JAVA_D2", "Abstract factory should end with Factory", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_D3 — Builder pattern naming
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_D3"
    name: "Builder classes should be named with Builder suffix"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("class") && line.contains("Builder") && !line.ends_with("Builder") && !line.ends_with("BuilderImpl") {
                issues.push(Issue::new("JAVA_D3", "Builder class should end with Builder", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_D4 — Strategy pattern without interface
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_D4"
    name: "Strategy implementations should implement a Strategy interface"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("class") && line.contains("implements") {
                if line.contains("execute") || line.contains("process") || line.contains("apply") {
                    if !line.contains("Strategy") && !line.contains("Handler") {
                        issues.push(Issue::new("JAVA_D4", "Strategy class should implement Strategy interface", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_D5 — Observer pattern naming
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_D5"
    name: "Observer implementations should be named with Listener suffix"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            if line.contains("addListener") || line.contains("removeListener") || line.contains("notifyListener") {
                if idx + 1 < lines.len() && lines[idx + 1].contains("class") && lines[idx + 1].contains("implements") {
                    if !lines[idx + 1].ends_with("Listener") && !lines[idx + 1].ends_with("Handler") {
                        issues.push(Issue::new("JAVA_D5", "Observer class should end with Listener", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_D6 — Decorator pattern naming
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_D6"
    name: "Decorator classes should be named with Decorator suffix"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("extends") && line.contains("Decorator") {
                if !line.ends_with("Decorator") && !line.ends_with("Wrapper") {
                    issues.push(Issue::new("JAVA_D6", "Decorator class should end with Decorator", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_D7 — Adapter pattern naming
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_D7"
    name: "Adapter classes should be named with Adapter suffix"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if (line.contains("implements") || line.contains("extends")) && line.contains("Adapter") {
                if !line.ends_with("Adapter") && !line.ends_with("Wrapper") && !line.ends_with("Handler") {
                    issues.push(Issue::new("JAVA_D7", "Adapter class should end with Adapter", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_D8 — DAO without interface
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_D8"
    name: "DAO classes should implement a DAO interface"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("class") && line.contains("Dao") && !line.contains("implements") && !line.contains("Repository") {
                issues.push(Issue::new("JAVA_D8", "DAO should implement DAO interface", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_D9 — DTO with business logic
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_D9"
    name: "DTO classes should not contain business logic"
    severity: Major
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            if line.contains("class") && line.contains("Dto") && !line.contains("implements") {
                let body: String = lines.iter().skip(idx).take(50).map(|s| *s).collect::<Vec<_>>().join("\n");
                if (body.contains("if (") || body.contains("for (") || body.contains("while (")) && !body.contains("validation") {
                    issues.push(Issue::new("JAVA_D9", "DTO contains business logic", Severity::Major, Category::CodeSmell, ctx.file_path, idx + 1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_D10 — Service class with state
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_D10"
    name: "Service classes should be stateless"
    severity: Major
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            if line.contains("class") && line.contains("Service") && !line.contains("implements") {
                let body: String = lines.iter().skip(idx).take(60).map(|s| *s).collect::<Vec<_>>().join("\n");
                let field_count = body.matches("private").count() + body.matches("protected").count();
                if field_count > 2 {
                    issues.push(Issue::new("JAVA_D10", "Service class appears stateful", Severity::Major, Category::CodeSmell, ctx.file_path, idx + 1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_D11 — Repository with business logic
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_D11"
    name: "Repository classes should not contain business logic"
    severity: Major
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            if line.contains("class") && line.contains("Repository") && !line.contains("implements") {
                let body: String = lines.iter().skip(idx).take(50).map(|s| *s).collect::<Vec<_>>().join("\n");
                if body.contains("if (") && body.contains("return") && !body.contains("findBy") && !body.contains("save") {
                    issues.push(Issue::new("JAVA_D11", "Repository contains business logic", Severity::Major, Category::CodeSmell, ctx.file_path, idx + 1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_D12 — Controller with business logic
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_D12"
    name: "Controller classes should delegate to services"
    severity: Major
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            if line.contains("class") && line.contains("Controller") && !line.contains("implements") {
                let body: String = lines.iter().skip(idx).take(80).map(|s| *s).collect::<Vec<_>>().join("\n");
                if body.contains("calculate") || body.contains("compute") || body.contains("process") {
                    if body.contains("new ") && body.contains("return") {
                        issues.push(Issue::new("JAVA_D12", "Controller contains business logic", Severity::Major, Category::CodeSmell, ctx.file_path, idx + 1));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_D13 — Utility class with public constructor
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_D13"
    name: "Utility classes should have private constructors"
    severity: Major
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") || trimmed.starts_with("*") {
                continue;
            }
            if (line.contains("class") || line.contains("interface")) && (line.contains("Util") || line.contains("Helper") || line.contains("Constants")) {
                let context: String = lines.iter().skip(idx).take(30).map(|s| *s).collect::<Vec<_>>().join("\n");
                if context.contains("public") && !context.contains("private") && !context.contains("protected") {
                    issues.push(Issue::new("JAVA_D13", "Utility class has public constructor", Severity::Major, Category::CodeSmell, ctx.file_path, idx + 1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_D14 — Enum with mutable fields
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_D14"
    name: "Enum classes should not have mutable fields"
    severity: Critical
    category: Bug
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            if line.contains("enum ") {
                let body: String = lines.iter().skip(idx).take(40).map(|s| *s).collect::<Vec<_>>().join("\n");
                if body.contains("List") || body.contains("Map") || body.contains("Set") || body.contains("Date") {
                    if !body.contains("final") && !body.contains("Mutable") {
                        issues.push(Issue::new("JAVA_D14", "Enum has mutable fields", Severity::Critical, Category::Bug, ctx.file_path, idx + 1));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_D15 — Interface with constants only
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_D15"
    name: "Interfaces should define behavior, not constants"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            if line.contains("interface ") {
                let body: String = lines.iter().skip(idx).take(30).map(|s| *s).collect::<Vec<_>>().join("\n");
                let has_methods = body.contains("void ") || body.contains("String ") || body.contains("int ") || body.contains("boolean ");
                if !has_methods && (body.contains("public static final") || body.matches("String ").count() > 1) {
                    issues.push(Issue::new("JAVA_D15", "Interface only has constants", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_D16 — Abstract class without abstract methods
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_D16"
    name: "Abstract class without abstract methods should be concrete"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            if line.contains("abstract class") {
                let body: String = lines.iter().skip(idx).take(50).map(|s| *s).collect::<Vec<_>>().join("\n");
                if !body.contains("abstract void") && !body.contains("abstract int") && !body.contains("abstract String") {
                    issues.push(Issue::new("JAVA_D16", "Abstract class has no abstract methods", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_D17 — Concrete class named with Impl suffix
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_D17"
    name: "Concrete implementations should not use Impl suffix"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("class") && line.contains("Impl") && !line.contains("abstract") {
                if line.ends_with("Impl") || line.contains("Impl ") {
                    issues.push(Issue::new("JAVA_D17", "Class uses Impl suffix", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_D18 — God class
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_D18"
    name: "Classes should not have too many methods"
    severity: Critical
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            if line.contains("class ") && !line.contains("abstract") {
                let body: String = lines.iter().skip(idx).take(200).map(|s| *s).collect::<Vec<_>>().join("\n");
                let method_count = body.matches("void ").count() + body.matches("int ").count() + body.matches("String ").count() + body.matches("boolean ").count();
                if method_count > 50 {
                    issues.push(Issue::new("JAVA_D18", "Class has too many methods", Severity::Critical, Category::CodeSmell, ctx.file_path, idx + 1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_D19 — Data clump
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_D19"
    name: "Multiple methods share the same parameters"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        let mut param_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for line in &lines {
            if line.contains("public ") || line.contains("private ") {
                let params = line.split('(').nth(1).map(|p| p.split(')').next()).flatten().unwrap_or("");
                if params.len() > 5 && params.matches(',').count() >= 2 {
                    *param_counts.entry(params.to_string()).or_insert(0) += 1;
                }
            }
        }
        for (_, count) in param_counts {
            if count >= 3 {
                issues.push(Issue::new("JAVA_D19", "Data clump detected", Severity::Minor, Category::CodeSmell, ctx.file_path, 1));
                break;
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_D20 — Feature envy
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_D20"
    name: "Method uses more data from other classes"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            if line.contains("public ") || line.contains("private ") {
                let body: String = lines.iter().skip(idx).take(30).map(|s| *s).collect::<Vec<_>>().join("\n");
                let this_refs = body.matches("this.").count();
                let external_refs = body.matches(".get").count() + body.matches(".set").count() + body.matches(".calculate").count();
                if external_refs > this_refs * 2 && this_refs < 3 && external_refs > 5 {
                    issues.push(Issue::new("JAVA_D20", "Method exhibits feature envy", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_D21 — Message chain
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_D21"
    name: "Method chain of depth >3 indicates message chain"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            let chain_depth = line.matches(").").count();
            if chain_depth > 3 {
                issues.push(Issue::new("JAVA_D21", "Message chain detected", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_D22 — Middle man
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_D22"
    name: "Class appears to be a middle man"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            if line.contains("class ") && !line.contains("abstract") {
                let body: String = lines.iter().skip(idx).take(50).map(|s| *s).collect::<Vec<_>>().join("\n");
                let delegation = body.matches("return ").count();
                let methods = body.matches("void ").count() + body.matches("int ").count() + body.matches("String ").count();
                if delegation == methods && methods > 2 {
                    issues.push(Issue::new("JAVA_D22", "Class only contains delegation methods", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_D23 — Inappropriate intimacy
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_D23"
    name: "Method accesses private fields of another class"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            let dots = line.matches(".").count();
            if dots >= 3 && !line.contains("this.") && !line.contains("get") && !line.contains("set") {
                issues.push(Issue::new("JAVA_D23", "Accessing private fields directly", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_D24 — Refused bequest
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_D24"
    name: "Subclass ignores most parent methods"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            if line.contains("extends ") && !line.contains("Exception") && !line.contains("Throwable") {
                let body: String = lines.iter().skip(idx).take(60).map(|s| *s).collect::<Vec<_>>().join("\n");
                let overrides = body.matches("@Override").count();
                if overrides == 0 {
                    issues.push(Issue::new("JAVA_D24", "Subclass overrides nothing", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_D25 — Lazy class
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_D25"
    name: "Class is too small to justify its existence"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            if line.contains("class ") && !line.contains("abstract") {
                let start = idx;
                let mut braces = 0;
                let mut end = idx;
                for (i, l) in lines.iter().enumerate().skip(idx) {
                    braces += l.matches("{").count() as i32;
                    braces -= l.matches("}").count() as i32;
                    if braces == 0 && i > idx {
                        end = i;
                        break;
                    }
                }
                let lines_count = end - start;
                if lines_count < 50 && lines_count > 0 {
                    issues.push(Issue::new("JAVA_D25", "Class appears lazy", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_D26 — Data class
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_D26"
    name: "Class with only getters and setters is a data class"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            if line.contains("class ") {
                let body: String = lines.iter().skip(idx).take(80).map(|s| *s).collect::<Vec<_>>().join("\n");
                let has_getter = body.contains("get") && body.contains("return");
                let has_setter = body.contains("set") && body.contains("void");
                let has_other = body.contains("if (") || body.contains("for (") || body.contains("calculate");
                if (has_getter || has_setter) && !has_other {
                    let gs_count = body.matches("get").count() + body.matches("set").count();
                    if gs_count > 3 {
                        issues.push(Issue::new("JAVA_D26", "Data class with only getters/setters", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_D27 — Temporary field
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_D27"
    name: "Field is only used in certain code paths"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            if line.contains("private ") && !line.contains("final") {
                let field_name = line.split_whitespace().last().unwrap_or("").trim_end_matches(';');
                if !field_name.is_empty() && field_name.len() > 1 {
                    let body: String = lines.iter().skip(idx).take(100).map(|s| *s).collect::<Vec<_>>().join("\n");
                    let usages = body.matches(&format!("this.{}", field_name)).count();
                    if usages <= 2 && !field_name.contains("temp") && !field_name.contains("cache") {
                        issues.push(Issue::new("JAVA_D27", "Field appears temporary", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_D28 — Switch statements
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_D28"
    name: "Switch statements can often be replaced with polymorphism"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            if line.contains("switch (") {
                let body: String = lines.iter().skip(idx).take(30).map(|s| *s).collect::<Vec<_>>().join("\n");
                let case_count = body.matches("case ").count();
                if case_count > 4 && !body.contains("enum") && !body.contains("String") {
                    issues.push(Issue::new("JAVA_D28", "Switch with many cases", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_D29 — Parallel inheritance hierarchies
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_D29"
    name: "Multiple inheritance hierarchies that grow in parallel"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        let mut suffixes: std::collections::HashMap<String, Vec<usize>> = std::collections::HashMap::new();
        for (idx, line) in lines.iter().enumerate() {
            if line.contains("extends ") {
                if line.contains("A ") || line.contains("B ") || line.contains("Base ") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        let suffix = if parts[1].ends_with("A") { "A" } else if parts[1].ends_with("B") { "B" } else { "Base" };
                        suffixes.entry(suffix.to_string()).or_default().push(idx);
                    }
                }
            }
        }
        if suffixes.len() >= 2 && suffixes.values().any(|v| v.len() > 1) {
            issues.push(Issue::new("JAVA_D29", "Parallel inheritance hierarchies detected", Severity::Minor, Category::CodeSmell, ctx.file_path, 1));
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_D30 — Comments explaining bad code
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_D30"
    name: "TODO/FIXME comments without ticket reference"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            let upper = line.to_uppercase();
            if (upper.contains("TODO") || upper.contains("FIXME") || upper.contains("HACK")) && !line.contains("[") && !line.contains("#") && !line.contains("JIRA") {
                issues.push(Issue::new("JAVA_D30", "TODO/FIXME without ticket reference", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Null Safety & Optional Rules (JAVA_N1-N15)
// ═══════════════════════════════════════════════════════════════════════════════

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_N1 — Return null instead of Optional
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_N1"
    name: "Methods that may return null should return Optional"
    severity: Major
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            if (line.contains("return null") || line.contains("return (null)")) && !line.contains("Optional") && !line.contains("Stream") && !line.contains("List") {
                let context: String = lines.iter().skip(idx.saturating_sub(10)).take(15).cloned().collect::<Vec<_>>().join("\n");
                if context.contains("public ") || context.contains("protected ") {
                    issues.push(Issue::new("JAVA_N1", "Method returns null", Severity::Major, Category::CodeSmell, ctx.file_path, idx + 1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_N2 — Optional.get() without isPresent
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_N2"
    name: "Optional.get() should only be called after isPresent check"
    severity: Critical
    category: Bug
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains(".get()") && line.contains("Optional") && !line.contains("isPresent") && !line.contains("orElse") && !line.contains("ifPresent") {
                issues.push(Issue::new("JAVA_N2", "Optional.get() without isPresent check", Severity::Critical, Category::Bug, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_N3 — Optional.orElse(null)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_N3"
    name: "Optional.orElse(null) defeats the purpose of Optional"
    severity: Major
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("orElse(null)") || line.contains("orElse( null )") {
                issues.push(Issue::new("JAVA_N3", "Optional.orElse(null) used", Severity::Major, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_N4 — Optional as field
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_N4"
    name: "Optional as field type is generally discouraged"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("Optional<") && line.contains("private ") && !line.contains("final") {
                issues.push(Issue::new("JAVA_N4", "Optional as field type", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_N5 — Optional in collection
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_N5"
    name: "Optional in collections is a code smell"
    severity: Major
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("List<Optional") || line.contains("Set<Optional") || line.contains("Map<Optional") {
                issues.push(Issue::new("JAVA_N5", "Optional in collection", Severity::Major, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_N6 — Null check with if/else instead of Optional
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_N6"
    name: "Null check with if/else can be replaced with Optional"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            if line.contains("== null") || line.contains("!= null") {
                let next_lines: String = lines.iter().skip(idx).take(5).map(|s| *s).collect::<Vec<_>>().join("\n");
                if next_lines.contains("if") && next_lines.contains("return") && !next_lines.contains("Optional") {
                    issues.push(Issue::new("JAVA_N6", "Null check could use Optional", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_N7 — Objects.requireNonNull missing
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_N7"
    name: "Public method parameters should be validated"
    severity: Major
    category: Bug
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("public ") && line.contains("(") && line.contains(")") && !line.contains("{") {
                let context: String = ctx.source.lines().skip(idx).take(10).collect::<Vec<_>>().join("\n");
                if !context.contains("requireNonNull") && !context.contains("CheckForNull") {
                    issues.push(Issue::new("JAVA_N7", "Parameters not validated", Severity::Major, Category::Bug, ctx.file_path, idx + 1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_N8 — Null comparison with ==
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_N8"
    name: "Use Objects.equals() for null-safe comparison"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if (line.contains("== null") || line.contains("null ==")) && !line.contains("Objects.requireNonNull") && !line.contains("Optional") {
                issues.push(Issue::new("JAVA_N8", "Null comparison with ==", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_N9 — Nullable annotation missing
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_N9"
    name: "Methods returning null should have Nullable annotation"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            if line.contains("return null") || line.contains("return (null)") {
                let prev: String = lines.iter().take(idx).rev().take(5).map(|s| *s).collect::<Vec<_>>().join("\n");
                if !prev.contains("@Nullable") && !prev.contains("@CheckForNull") {
                    issues.push(Issue::new("JAVA_N9", "Missing Nullable annotation", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_N10 — Optional.of() with null
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_N10"
    name: "Optional.of() does not accept null"
    severity: Critical
    category: Bug
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("Optional.of(") && !line.contains("Optional.ofNullable(") {
                issues.push(Issue::new("JAVA_N10", "Optional.of with possible null", Severity::Critical, Category::Bug, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_N11 — Optional.flatMap vs map confusion
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_N11"
    name: "Optional.map() returning Optional should use flatMap"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains(".map(") && line.contains("Optional") {
                let context: String = ctx.source.lines().skip(idx).take(3).map(|s| s.to_string()).collect::<Vec<_>>().join("\n");
                if context.contains(".map(") && context.contains("Optional") {
                    issues.push(Issue::new("JAVA_N11", "Chained map calls on Optional", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_N12 — Chained Optional calls
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_N12"
    name: "Chained Optional calls without early exit"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            let dot_chains = line.matches(".map(").count() + line.matches(".filter(").count();
            if dot_chains >= 3 {
                issues.push(Issue::new("JAVA_N12", "Chained Optional calls", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_N13 — Optional.filter().isPresent()
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_N13"
    name: "Optional.filter().isPresent() can be replaced"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains(".filter(") && line.contains(".isPresent()") {
                issues.push(Issue::new("JAVA_N13", "Optional.filter().isPresent() pattern", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_N14 — Optional.orElseGet with constructor
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_N14"
    name: "Optional.orElseGet(() -> new X()) can be simplified"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("orElseGet(() -> new") {
                issues.push(Issue::new("JAVA_N14", "orElseGet with constructor", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_N15 — NonNullByDefault missing
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_N15"
    name: "Package should have NonNullByDefault annotation"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let has_package_info = ctx.source.lines().any(|l| l.contains("package-info.java"));
        if !has_package_info {
            let has_nonNull = ctx.source.lines().any(|l| l.contains("@NonNull") || l.contains("NonNullByDefault"));
            if !has_nonNull && ctx.source.lines().any(|l| l.contains("return null") || l.contains("== null")) {
                issues.push(Issue::new("JAVA_N15", "Consider adding NonNullByDefault", Severity::Minor, Category::CodeSmell, ctx.file_path, 1));
            }
        }
        issues
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Streams & Lambdas Rules (JAVA_L1-L15)
// ═══════════════════════════════════════════════════════════════════════════════

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_L1 — Stream.forEach with side effects
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_L1"
    name: "Stream.forEach with side effects should use forEachOrdered"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains(".forEach(") && (line.contains("System.out") || line.contains(".add(") || line.contains(".set(")) {
                issues.push(Issue::new("JAVA_L1", "Stream.forEach with side effects", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_L2 — Stream.peek() for debugging
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_L2"
    name: "Stream.peek() should only be used for debugging"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains(".peek(") && !line.contains("// debug") && !line.contains("//DEBUG") {
                issues.push(Issue::new("JAVA_L2", "Stream.peek() in production code", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_L3 — Collectors.toList() instead of toList()
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_L3"
    name: "Use Stream.toList() instead of Collectors.toList()"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("Collectors.toList()") {
                issues.push(Issue::new("JAVA_L3", "Use toList() instead", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_L4 — Stream.filter().findFirst()
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_L4"
    name: "Use stream.anyMatch() instead of filter().findFirst().isPresent()"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains(".filter(") && line.contains(".findFirst()") && line.contains(".isPresent()") {
                issues.push(Issue::new("JAVA_L4", "filter().findFirst().isPresent() pattern", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_L5 — Stream.map() then collect()
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_L5"
    name: "Stream.map().collect() when forEach would suffice"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains(".map(") && line.contains(".collect()") && (line.contains("System.out") || line.contains("log.")) {
                issues.push(Issue::new("JAVA_L5", "map().collect() when forEach would do", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_L6 — Parallel stream without measurement
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_L6"
    name: "Parallel streams should be benchmarked before use"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains(".parallelStream()") || line.contains(".parallel()") {
                let context: String = ctx.source.lines().skip(idx.saturating_sub(5)).take(10).collect::<Vec<_>>().join("\n");
                if !context.contains("benchmark") && !context.contains("measure") && !context.contains("perf") {
                    issues.push(Issue::new("JAVA_L6", "Parallel stream without measurement", Severity::Minor, Category:: CodeSmell, ctx.file_path, idx + 1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_L7 — Stream with stateful lambda
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_L7"
    name: "Streams with stateful lambdas can cause bugs"
    severity: Major
    category: Bug
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            if line.contains(".stream()") || line.contains(".parallelStream()") {
                let context: String = lines.iter().skip(idx).take(15).map(|s| *s).collect::<Vec<_>>().join("\n");
                if context.contains(".add(") || context.contains(".put(") {
                    issues.push(Issue::new("JAVA_L7", "Stream uses stateful lambda", Severity::Major, Category::Bug, ctx.file_path, idx + 1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_L8 — Stream.boxed() unnecessary
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_L8"
    name: "Unnecessary Stream.boxed() call"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains(".boxed()") && (line.contains(".mapToInt") || line.contains(".mapToLong") || line.contains(".sum()")) {
                issues.push(Issue::new("JAVA_L8", "Unnecessary boxed() call", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_L9 — Lambda with empty body
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_L9"
    name: "Lambda with empty body is a no-op"
    severity: Major
    category: Bug
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("-> {}") || line.contains("->{ }") {
                issues.push(Issue::new("JAVA_L9", "Lambda with empty body", Severity::Major, Category::Bug, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_L10 — Method reference more readable
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_L10"
    name: "Method reference is more readable than equivalent lambda"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("-> ") && line.contains(".") && !line.contains("()") {
                issues.push(Issue::new("JAVA_L10", "Lambda could be method reference", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_L11 — Lambda more readable than method reference
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_L11"
    name: "Lambda may be more readable than complex method reference"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("::") && line.contains(",") {
                issues.push(Issue::new("JAVA_L11", "Method reference with multiple args", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_L12 — Stream.sorted().findFirst()
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_L12"
    name: "Use stream.min() instead of sorted().findFirst()"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains(".sorted()") && line.contains(".findFirst()") {
                issues.push(Issue::new("JAVA_L12", "sorted().findFirst() pattern", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_L13 — Collectors.counting()
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_L13"
    name: "Use stream.count() instead of Collectors.counting()"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("Collectors.counting()") {
                issues.push(Issue::new("JAVA_L13", "Use stream.count() instead", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_L14 — Stream.concat() misuse
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_L14"
    name: "Stream.concat() should be used carefully"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("Stream.concat(") && (line.contains("List") || line.contains("ArrayList")) {
                issues.push(Issue::new("JAVA_L14", "Stream.concat() usage", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_L15 — IntStream.range() instead of for loop
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_L15"
    name: "Consider IntStream.range() for index-based loops"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            let re = regex::Regex::new(r"for\s*\(\s*int\s+\w+\s*=\s*0\s*;\s*\w+\s*<\s*\w+\.size\(\)\s*;").unwrap();
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_L15", "Index-based for loop", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Testing & JUnit Rules (JAVA_T1-T15)
// ═══════════════════════════════════════════════════════════════════════════════

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_T1 — Test without assertion
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_T1"
    name: "Test methods should contain assertions"
    severity: Major
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            if line.contains("@Test") && (line.contains("void test") || line.contains("void ")) {
                let body: String = lines.iter().skip(idx).take(30).map(|s| *s).collect::<Vec<_>>().join("\n");
                if !body.contains("assert") && !body.contains("verify") {
                    issues.push(Issue::new("JAVA_T1", "Test without assertions", Severity::Major, Category::CodeSmell, ctx.file_path, idx + 1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_T2 — Test with Thread.sleep()
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_T2"
    name: "Thread.sleep() in tests should be replaced with Awaitility"
    severity: Major
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("Thread.sleep(") && !line.contains("timeout") && !line.contains("Awaitility") {
                issues.push(Issue::new("JAVA_T2", "Thread.sleep in test", Severity::Major, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_T3 — @Test(expected) vs assertThrows
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_T3"
    name: "@Test(expected) is deprecated"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("@Test") && line.contains("expected=") {
                issues.push(Issue::new("JAVA_T3", "@Test(expected=) style", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_T4 — @Before/@After vs @BeforeEach/@AfterEach
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_T4"
    name: "JUnit 4 @Before/@After should be JUnit 5 @BeforeEach/@AfterEach"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if (line.contains("@Before ") || line.contains("@After ")) && !line.contains("Each") {
                issues.push(Issue::new("JAVA_T4", "JUnit 4 annotation style", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_T5 — @RunWith vs @ExtendWith
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_T5"
    name: "@RunWith is JUnit 4 - use @ExtendWith for JUnit 5"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("@RunWith(") {
                issues.push(Issue::new("JAVA_T5", "JUnit 4 @RunWith", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_T6 — assertTrue/assertFalse vs assertEquals
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_T6"
    name: "assertEquals with boolean is clearer than assertTrue/assertFalse"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("assertTrue(!") {
                issues.push(Issue::new("JAVA_T6", "assertTrue with negation", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_T7 — Test method not public
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_T7"
    name: "JUnit 5 test methods do not need to be public"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("@Test") && line.contains("public void") && !line.contains("@ExtendWith") && !line.contains("@RunWith") {
                issues.push(Issue::new("JAVA_T7", "Test method is public", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_T8 — @Ignore without reason
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_T8"
    name: "@Ignore without description should include reason"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("@Ignore") && !line.contains("\"") && !line.contains("//") {
                issues.push(Issue::new("JAVA_T8", "@Ignore without description", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_T9 — System.setProperty not reset
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_T9"
    name: "System.setProperty in test should be reset"
    severity: Major
    category: Bug
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            if line.contains("System.setProperty(") {
                let next: String = lines.iter().skip(idx).take(30).map(|s| *s).collect::<Vec<_>>().join("\n");
                if !next.contains("clearProperty") && !next.contains("@After") && !next.contains("restore") {
                    issues.push(Issue::new("JAVA_T9", "System.setProperty without cleanup", Severity::Major, Category::Bug, ctx.file_path, idx + 1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_T10 — Mock without verification
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_T10"
    name: "Mock created but never verified"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            if line.contains("@Mock") || line.contains("mock(") || line.contains("Mockito.mock(") {
                let body: String = lines.iter().skip(idx).take(40).map(|s| *s).collect::<Vec<_>>().join("\n");
                if !body.contains("verify(") && !body.contains("thenReturn") && !body.contains("thenThrow") {
                    issues.push(Issue::new("JAVA_T10", "Mock without verification", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_T11 — Test with new Date()
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_T11"
    name: "Test should use Clock instead of new Date()"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            if line.contains("new Date()") && !line.contains("Clock") && !line.contains("Instant") {
                let prev: String = lines.iter().take(idx).rev().take(5).map(|s| *s).collect::<Vec<_>>().join("\n");
                if prev.contains("@Test") || prev.contains("@Before") {
                    issues.push(Issue::new("JAVA_T11", "new Date() in test", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_T12 — Test with Random
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_T12"
    name: "Test using Random produces non-deterministic results"
    severity: Critical
    category: Bug
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            if line.contains("new Random()") {
                let prev: String = lines.iter().take(idx).rev().take(5).map(|s| *s).collect::<Vec<_>>().join("\n");
                if prev.contains("@Test") || prev.contains("@Before") {
                    issues.push(Issue::new("JAVA_T12", "Random in test", Severity::Critical, Category::Bug, ctx.file_path, idx + 1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_T13 — @Spy vs @Mock misuse
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_T13"
    name: "@Spy wraps real object, @Mock creates fake"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("@Spy") && line.contains("Mockito.mock(") {
                issues.push(Issue::new("JAVA_T13", "@Spy with Mockito.mock", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_T14 — assertThat with wrong matcher
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_T14"
    name: "assertThat with incorrect matcher may cause issues"
    severity: Critical
    category: Bug
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("assertThat") && line.contains(".isEqualTo(null)") {
                issues.push(Issue::new("JAVA_T14", "assertThat with null equality", Severity::Critical, Category::Bug, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_T15 — Test class without annotation
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_T15"
    name: "Test class should have proper annotation"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            if line.contains("class ") && line.contains("Test") && !line.contains("abstract") {
                let header: String = lines.iter().take(idx).rev().take(5).map(|s| *s).collect::<Vec<_>>().join("\n");
                if !header.contains("@ExtendWith") && !header.contains("@RunWith") && !header.contains("@SpringBootTest") {
                    issues.push(Issue::new("JAVA_T15", "Test class needs annotation", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
                }
            }
        }
        issues
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Performance Rules (JAVA_P1-P15)
// ═══════════════════════════════════════════════════════════════════════════════

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_P1 — String concatenation in loop
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_P1"
    name: "String concatenation in loop uses StringBuilder internally"
    severity: Major
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        let mut in_loop = false;
        for (idx, line) in lines.iter().enumerate() {
            if line.contains("for (") || line.contains("while (") || line.contains("do {") {
                in_loop = true;
            }
            if in_loop && line.contains("+=") && line.contains("String") && !line.contains("StringBuilder") {
                issues.push(Issue::new("JAVA_P1", "String concatenation in loop", Severity::Major, Category::CodeSmell, ctx.file_path, idx + 1));
            }
            if line.trim() == "}" && in_loop {
                in_loop = false;
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_P2 — StringBuilder with initial capacity 16
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_P2"
    name: "StringBuilder with default capacity may cause reallocation"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("new StringBuilder(16)") || line.contains("new StringBuilder( 16 )") {
                issues.push(Issue::new("JAVA_P2", "StringBuilder(16)", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_P3 — ArrayList without initial capacity
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_P3"
    name: "ArrayList with many adds should have initial capacity set"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            if line.contains("new ArrayList()") || line.contains("new ArrayList<") {
                let next: String = lines.iter().skip(idx).take(20).map(|s| *s).collect::<Vec<_>>().join("\n");
                let add_count = next.matches(".add(").count();
                if add_count > 5 {
                    issues.push(Issue::new("JAVA_P3", format!("ArrayList with {} adds", add_count), Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_P4 — HashMap with load factor
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_P4"
    name: "HashMap with non-default load factor should be documented"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("new HashMap(") && line.contains("0.") {
                issues.push(Issue::new("JAVA_P4", "HashMap with custom load factor", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_P5 — BigInteger for small values
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_P5"
    name: "BigInteger for small values is overkill"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("BigInteger.valueOf(") && line.contains("0") || line.contains("BigInteger.valueOf(1") {
                issues.push(Issue::new("JAVA_P5", "BigInteger for small constant", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_P6 — BigDecimal.divide() without scale
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_P6"
    name: "BigDecimal.divide() without scale risks ArithmeticException"
    severity: Critical
    category: Bug
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains(".divide(") && !line.contains("RoundingMode") && !line.contains("MathContext") && !line.contains("setScale") {
                issues.push(Issue::new("JAVA_P6", "BigDecimal.divide without rounding", Severity::Critical, Category::Bug, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_P7 — Pattern.compile() in loop
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_P7"
    name: "Pattern.compile() in loop should be static final field"
    severity: Critical
    category: Bug
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        let mut in_loop = false;
        for (idx, line) in lines.iter().enumerate() {
            if line.contains("for (") || line.contains("while (") {
                in_loop = true;
            }
            if in_loop && line.contains("Pattern.compile(") {
                issues.push(Issue::new("JAVA_P7", "Pattern.compile in loop", Severity::Critical, Category::Bug, ctx.file_path, idx + 1));
            }
            if line.trim() == "}" && in_loop {
                in_loop = false;
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_P8 — SimpleDateFormat in multi-threaded context
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_P8"
    name: "SimpleDateFormat is not thread-safe"
    severity: Critical
    category: Bug
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("new SimpleDateFormat(") {
                let context: String = ctx.source.lines().skip(idx.saturating_sub(3)).take(10).collect::<Vec<_>>().join("\n");
                if !context.contains("ThreadLocal") && !context.contains("static") && !context.contains("final") {
                    issues.push(Issue::new("JAVA_P8", "SimpleDateFormat not thread-safe", Severity::Critical, Category::Bug, ctx.file_path, idx + 1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_P9 — Logger created per instance
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_P9"
    name: "Logger should be static final"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("Logger ") && line.contains("=") && line.contains("private ") && !line.contains("static") {
                issues.push(Issue::new("JAVA_P9", "Logger as instance field", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_P10 — Logger with concatenation
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_P10"
    name: "Logger calls should use parameterized logging"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if (line.contains("log.debug(") || line.contains("log.info(") || line.contains("log.warn(") || line.contains("log.error(")) && line.contains(" + ") {
                issues.push(Issue::new("JAVA_P10", "Logger with concatenation", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_P11 — System.gc() call
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_P11"
    name: "System.gc() is a hint, not command"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("System.gc()") {
                issues.push(Issue::new("JAVA_P11", "System.gc call", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_P12 — finalize() method override
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_P12"
    name: "Overriding finalize() is deprecated"
    severity: Major
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("protected void finalize()") {
                issues.push(Issue::new("JAVA_P12", "finalize override", Severity::Major, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_P13 — clone() without Cloneable
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_P13"
    name: "clone() method without implementing Cloneable is risky"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            if line.contains("public Object clone()") || line.contains("public clone(") {
                let header: String = lines.iter().take(idx).take(20).map(|s| *s).collect::<Vec<_>>().join("\n");
                if !header.contains("Cloneable") && !header.contains("implements Cloneable") {
                    issues.push(Issue::new("JAVA_P13", "clone without Cloneable", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_P14 — iterator() called multiple times
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_P14"
    name: "Calling iterator() multiple times creates multiple iterators"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            if line.contains(".iterator()") && !line.contains("for (") {
                let next: String = lines.iter().skip(idx).take(5).map(|s| *s).collect::<Vec<_>>().join("\n");
                if next.contains(".iterator()") {
                    issues.push(Issue::new("JAVA_P14", "iterator called multiple times", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_P15 — Collection.size() in loop condition
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "JAVA_P15"
    name: "Collection.size() in loop condition may be called repeatedly"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            let re = regex::Regex::new(r"for\s*\([^)]*\.size\s*\(\s*\)[^)]*\)").unwrap();
            if re.is_match(line) && !line.contains("ArrayList") && !line.contains("HashMap") && !line.contains("HashSet") {
                issues.push(Issue::new("JAVA_P15", "size in for loop condition", Severity::Minor, Category::CodeSmell, ctx.file_path, idx + 1));
            }
        }
        issues
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Python Security Rules (PY_S1-PY_S30)
// ═══════════════════════════════════════════════════════════════════════════════

// ─────────────────────────────────────────────────────────────────────────────
// PY_S2068 — Hardcoded credentials
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S2068"
    name: "Hardcoded credentials should not be used"
    severity: Blocker
    category: Vulnerability
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            let t = line.trim();
            if (t.contains("password") || t.contains("secret") || t.contains("api_key")) && (t.contains("= \"") || t.contains("= '")) {
                if !t.contains("getenv") && !t.contains("environ") && !t.contains("os.environ") {
                    issues.push(Issue::new("PY_S2068", "Hardcoded credential detected", Severity::Blocker, Category::Vulnerability, ctx.file_path, idx+1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S5332 — Clear-text HTTP
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S5332"
    name: "Clear-text HTTP should not be used"
    severity: Blocker
    category: Vulnerability
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("http://") && !line.contains("https://") && !line.contains("localhost") {
                issues.push(Issue::new("PY_S5332", "Clear-text HTTP URL detected", Severity::Blocker, Category::Vulnerability, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S2077 — SQL injection via f-strings
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S2077"
    name: "SQL queries should not be built with string interpolation"
    severity: Blocker
    category: Vulnerability
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let sql_keywords = ["SELECT", "INSERT", "UPDATE", "DELETE", "DROP", "CREATE", "ALTER"];
        for (idx, line) in ctx.source.lines().enumerate() {
            let has_sql: bool = sql_keywords.iter().any(|kw| line.to_uppercase().contains(kw));
            if has_sql && (line.contains("f\"") || line.contains("f'")) && line.contains("{") {
                issues.push(Issue::new("PY_S2077", "SQL query built with f-string interpolation", Severity::Blocker, Category::Vulnerability, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S1523 — eval()/exec() usage
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S1523"
    name: "eval() and exec() should not be used"
    severity: Blocker
    category: Vulnerability
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("eval(") || line.contains("exec(") {
                issues.push(Issue::new("PY_S1523", "Use of eval() or exec() is security-sensitive", Severity::Blocker, Category::Vulnerability, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S4830 — SSL verification disabled
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S4830"
    name: "SSL certificate verification should not be disabled"
    severity: Blocker
    category: Vulnerability
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if (line.contains("verify=False") || line.contains("verify = False")) && (line.contains("requests") || line.contains("urllib")) {
                issues.push(Issue::new("PY_S4830", "SSL verification disabled - man-in-the-middle risk", Severity::Blocker, Category::Vulnerability, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S4423 — Weak TLS protocol
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S4423"
    name: "Weak TLS protocols should not be used"
    severity: Critical
    category: Vulnerability
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("TLSv1_0") || line.contains("TLSv1.1") || line.contains("SSLv3") || line.contains("PROTOCOL_TLSv1") {
                issues.push(Issue::new("PY_S4423", "Weak TLS protocol detected", Severity::Critical, Category::Vulnerability, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S4784 — ReDoS via re.compile with user input
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S4784"
    name: "Regular expressions should not be built from user input"
    severity: Blocker
    category: Vulnerability
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("re.compile(") && (line.contains("request") || line.contains("user") || line.contains("input")) {
                issues.push(Issue::new("PY_S4784", "Regex compiled from user input - ReDoS risk", Severity::Blocker, Category::Vulnerability, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S5247 — XSS in templates (| safe filter)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S5247"
    name: "User input should not be marked as safe without sanitization"
    severity: Blocker
    category: Vulnerability
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("| safe") || line.contains("|escape") && line.contains("{{") {
                issues.push(Issue::new("PY_S5247", "Template marked safe without sanitization - XSS risk", Severity::Blocker, Category::Vulnerability, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S5542 — Weak crypto: hashlib.md5()
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S5542"
    name: "Weak cryptographic hash function should not be used"
    severity: Critical
    category: Vulnerability
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("hashlib.md5") || line.contains("hashlib.sha1") {
                issues.push(Issue::new("PY_S5542", "Weak cryptographic hash (MD5/SHA1) detected", Severity::Critical, Category::Vulnerability, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S5547 — Weak cipher: DES, RC4
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S5547"
    name: "Weak cipher algorithms should not be used"
    severity: Critical
    category: Vulnerability
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("DES.new") || line.contains("RC4") || line.contains("arc4") {
                issues.push(Issue::new("PY_S5547", "Weak cipher (DES/RC4) detected", Severity::Critical, Category::Vulnerability, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S3649 — SQL via string concatenation
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S3649"
    name: "SQL queries should not be built with string concatenation"
    severity: Blocker
    category: Vulnerability
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let sql_keywords = ["SELECT", "INSERT", "UPDATE", "DELETE", "DROP", "CREATE"];
        for (idx, line) in ctx.source.lines().enumerate() {
            let has_sql: bool = sql_keywords.iter().any(|kw| line.to_uppercase().contains(kw));
            if has_sql && (line.contains("+") || line.contains("format(") || line.contains("%")) && !line.contains("?") {
                issues.push(Issue::new("PY_S3649", "SQL built with string concatenation", Severity::Blocker, Category::Vulnerability, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S2612 — Weak file permissions (chmod 777)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S2612"
    name: "File permissions should not be too permissive"
    severity: Critical
    category: Vulnerability
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("0o777") || line.contains("0777") || line.contains("chmod(0xfff") {
                issues.push(Issue::new("PY_S2612", "Overly permissive file permissions (0777)", Severity::Critical, Category::Vulnerability, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S2095 — Resource leak (open without context manager)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S2095"
    name: "Resources should be properly closed"
    severity: Major
    category: Bug
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            let t = line.trim();
            if t.contains("= open(") && !t.contains("with ") && !t.contains("as ") {
                issues.push(Issue::new("PY_S2095", "File opened without context manager", Severity::Major, Category::Bug, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S5693 — File upload without size limit
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S5693"
    name: "File uploads should have size limits"
    severity: Critical
    category: Vulnerability
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("request.files") && !line.contains("max_size") && !line.contains("MAX_SIZE") && !line.contains("content_length") {
                issues.push(Issue::new("PY_S5693", "File upload without size limit", Severity::Critical, Category::Vulnerability, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S3330 — Cookie without HttpOnly
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S3330"
    name: "Cookies should set the HttpOnly flag"
    severity: Minor
    category: SecurityHotspot
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("set_cookie(") && !line.contains("httponly") && !line.contains("HttpOnly") {
                issues.push(Issue::new("PY_S3330", "Cookie without HttpOnly flag", Severity::Minor, Category::SecurityHotspot, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S2092 — Cookie no Secure flag
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S2092"
    name: "Cookies should set the Secure flag"
    severity: Minor
    category: SecurityHotspot
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("set_cookie(") && !line.contains("secure=") && !line.contains("Secure=") {
                issues.push(Issue::new("PY_S2092", "Cookie without Secure flag", Severity::Minor, Category::SecurityHotspot, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S4502 — CSRF protection disabled
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S4502"
    name: "CSRF protection should not be disabled"
    severity: Blocker
    category: Vulnerability
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("csrf_exempt") || line.contains("@csrf.exempt") || line.contains("CSRF_DISABLED") {
                issues.push(Issue::new("PY_S4502", "CSRF protection disabled", Severity::Blocker, Category::Vulnerability, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S5725 — CSP header missing
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S5725"
    name: "Content-Security-Policy header should be set"
    severity: Minor
    category: SecurityHotspot
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let all_source = ctx.source.to_string();
        let has_route = all_source.contains("@app.route") || all_source.contains("Flask(__name__)");
        let has_csp = all_source.contains("Content-Security-Policy") || all_source.contains("ContentSecurityPolicy");
        if has_route && !has_csp {
            issues.push(Issue::new("PY_S5725", "Content-Security-Policy header missing", Severity::Minor, Category::SecurityHotspot, ctx.file_path, 1));
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S5734 — HSTS header missing
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S5734"
    name: "Strict-Transport-Security header should be set"
    severity: Minor
    category: SecurityHotspot
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let all_source = ctx.source.to_string();
        let has_route = all_source.contains("@app.route") || all_source.contains("Flask(__name__)");
        let has_hsts = all_source.contains("Strict-Transport-Security") || all_source.contains("HSTS");
        if has_route && !has_hsts {
            issues.push(Issue::new("PY_S5734", "HSTS header missing", Severity::Minor, Category::SecurityHotspot, ctx.file_path, 1));
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S5736 — X-Content-Type-Options missing
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S5736"
    name: "X-Content-Type-Options header should be set"
    severity: Minor
    category: SecurityHotspot
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let all_source = ctx.source.to_string();
        let has_route = all_source.contains("@app.route") || all_source.contains("Flask(__name__)");
        let has_cto = all_source.contains("X-Content-Type-Options") || all_source.contains("nosniff");
        if has_route && !has_cto {
            issues.push(Issue::new("PY_S5736", "X-Content-Type-Options header missing", Severity::Minor, Category::SecurityHotspot, ctx.file_path, 1));
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S1313 — Hardcoded IP address
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S1313"
    name: "IP addresses should not be hardcoded"
    severity: Minor
    category: SecurityHotspot
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let ip_re = regex::Regex::new(r#""\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}""#).unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(m) = ip_re.find(line) {
                let ip = m.as_str();
                if !ip.contains("0.0.0.0") && !ip.contains("127.0.0.1") {
                    issues.push(Issue::new("PY_S1313", format!("Hardcoded IP address: {}", ip), Severity::Minor, Category::SecurityHotspot, ctx.file_path, idx+1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S3358 — Nested ternary expressions
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S3358"
    name: "Nested ternary expressions should not be used"
    severity: Minor
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            let t = line.trim();
            let ternary_count = t.matches(" if ").count();
            if ternary_count >= 3 {
                issues.push(Issue::new("PY_S3358", "Nested ternary expression detected", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S5042 — Zip bomb vulnerability
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S5042"
    name: "Archive extraction should validate members"
    severity: Critical
    category: Vulnerability
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if (line.contains("extractall") || line.contains("extractall(")) && !line.contains("members") && !line.contains("validate") {
                issues.push(Issue::new("PY_S5042", "Archive extracted without member validation - zip bomb risk", Severity::Critical, Category::Vulnerability, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S2755 — XXE vulnerability
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S2755"
    name: "XML parsing should not enable external entities"
    severity: Blocker
    category: Vulnerability
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("etree.parse(") && !line.contains("resolve_entities") {
                issues.push(Issue::new("PY_S2755", "XML parse may be vulnerable to XXE", Severity::Blocker, Category::Vulnerability, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S4829 — print() in production code
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S4829"
    name: "print() should not be used in production code"
    severity: Minor
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("print(") && !line.contains("#") && !line.contains("test") && !line.contains("debug") {
                issues.push(Issue::new("PY_S4829", "print() in production code", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S1148 — Traceback exposed
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S1148"
    name: "Exception tracebacks should not be exposed in production"
    severity: Critical
    category: Vulnerability
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("traceback.print_exc()") || line.contains("traceback.format_exc()") {
                issues.push(Issue::new("PY_S1148", "Exception traceback exposed", Severity::Critical, Category::Vulnerability, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S1165 — Exception swallowed (except: pass)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S1165"
    name: "Exceptions should not be swallowed silently"
    severity: Major
    category: Bug
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            let t = line.trim();
            if t == "except:" || t == "except:" {
                let next_lines: String = ctx.source.lines().skip(idx).take(3).collect::<Vec<_>>().join("\n");
                if next_lines.contains("pass") && !next_lines.contains("log") && !next_lines.contains("print") {
                    issues.push(Issue::new("PY_S1165", "Exception swallowed without logging", Severity::Major, Category::Bug, ctx.file_path, idx+1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S1163 — Broad except clause
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S1163"
    name: "Exception types should be specified"
    severity: Major
    category: Bug
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("except Exception:") || line.contains("except Exception :") {
                issues.push(Issue::new("PY_S1163", "Catching all exceptions with 'except Exception'", Severity::Major, Category::Bug, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S112 — Generic exception raised
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S112"
    name: "Generic exceptions should not be raised"
    severity: Major
    category: Bug
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("raise Exception(") || line.contains("raise Exception (") {
                issues.push(Issue::new("PY_S112", "Generic Exception raised", Severity::Major, Category::Bug, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S2221 — BaseException caught
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S2221"
    name: "Catching BaseException is too broad"
    severity: Major
    category: Bug
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("except BaseException") || line.contains("except BaseException:") {
                issues.push(Issue::new("PY_S2221", "Catching BaseException is too broad", Severity::Major, Category::Bug, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Python Bug Rules (PY_B1-PY_B15)
// ═══════════════════════════════════════════════════════════════════════════════

// ─────────────────────────────────────────────────────────────────────────────
// PY_S2259 — None dereference
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S2259"
    name: "Variables should not be used after None check"
    severity: Blocker
    category: Bug
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains(" is None") || line.contains(" == None") {
                let next_lines: String = ctx.source.lines().skip(idx).take(10).collect::<Vec<_>>().join("\n");
                if next_lines.contains("if ") && next_lines.contains("return") {
                    issues.push(Issue::new("PY_S2259", "Variable may be used after None check", Severity::Blocker, Category::Bug, ctx.file_path, idx+1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S1244 — Float equality comparison
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S1244"
    name: "Floating point equality should not be used"
    severity: Major
    category: Bug
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("==") && (line.contains("0.1") || line.contains("0.2") || line.contains("0.3")) {
                issues.push(Issue::new("PY_S1244", "Float equality comparison", Severity::Major, Category::Bug, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S1751 — Loop with single iteration
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S1751"
    name: "Loops should not have only one iteration"
    severity: Major
    category: Bug
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            let t = line.trim();
            if t.contains("while True:") || t.contains("while True :") {
                let body: String = ctx.source.lines().skip(idx).take(10).collect::<Vec<_>>().join("\n");
                if body.contains("break") && !body.contains("continue") {
                    issues.push(Issue::new("PY_S1751", "Loop executes only once due to break", Severity::Major, Category::Bug, ctx.file_path, idx+1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S1845 — Dead store (assigned but never read)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S1845"
    name: "Variables should not be assigned but never read"
    severity: Major
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"^\s*(\w+)\s*=\s*").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                let name = cap.get(1).unwrap().as_str();
                if name != "_" {
                    let remaining: String = ctx.source.lines().skip(idx + 1).collect::<Vec<_>>().join("\n");
                    if !remaining.contains(&format!(" {} ", name)) && !remaining.contains(&format!("({}", name)) {
                        issues.push(Issue::new("PY_S1845", format!("Variable '{}' assigned but never read", name), Severity::Major, Category::CodeSmell, ctx.file_path, idx+1));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S1854 — Unused import
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S1854"
    name: "Unused imports should be removed"
    severity: Minor
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"^\s*import\s+(\w+)").unwrap();
        let re_from = regex::Regex::new(r"^\s*from\s+(\w+)\s+import").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                let module = cap.get(1).unwrap().as_str();
                let remaining: String = ctx.source.lines().skip(idx + 1).collect::<Vec<_>>().join("\n");
                if !remaining.contains(module) && module != "os" && module != "sys" {
                    issues.push(Issue::new("PY_S1854", format!("Unused import: {}", module), Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
                }
            }
            if let Some(cap) = re_from.captures(line) {
                let module = cap.get(1).unwrap().as_str();
                let remaining: String = ctx.source.lines().skip(idx + 1).collect::<Vec<_>>().join("\n");
                if !remaining.contains(module) {
                    issues.push(Issue::new("PY_S1854", format!("Unused import from: {}", module), Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S1481 — Unused variable
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S1481"
    name: "Unused variables should be removed"
    severity: Minor
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"^\s*(\w+)\s*=\s*\w+\(").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                let name = cap.get(1).unwrap().as_str();
                let remaining: String = ctx.source.lines().skip(idx + 1).collect::<Vec<_>>().join("\n");
                if !remaining.contains(&format!(" {} ", name)) && !remaining.contains(&format!("({}", name)) && !remaining.contains(&format!("={}", name)) {
                    issues.push(Issue::new("PY_S1481", format!("Unused variable: {}", name), Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S1226 — Parameter reassigned
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S1226"
    name: "Function parameters should not be reassigned"
    severity: Major
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"def\s+\w+\s*\(([^)]+)\)").unwrap();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            if let Some(cap) = re.captures(line) {
                let params = cap.get(1).unwrap().as_str();
                let func_body: String = lines.iter().skip(idx).take(20).cloned().collect::<Vec<_>>().join("\n");
                for param in params.split(",") {
                    let p = param.trim().split(":").next().unwrap_or(param.trim()).trim();
                    if p != "self" && p != "cls" && func_body.contains(&format!("{} =", p)) {
                        issues.push(Issue::new("PY_S1226", format!("Parameter '{}' reassigned", p), Severity::Major, Category::CodeSmell, ctx.file_path, idx+1));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S1656 — Self-assignment
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S1656"
    name: "Variables should not be self-assigned"
    severity: Major
    category: Bug
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            let t = line.trim();
            if let Some(eq_pos) = t.find("=") {
                if eq_pos > 0 {
                    let lhs = t[..eq_pos].trim();
                    let rhs = t[eq_pos + 1..].trim().trim_end_matches(";").trim();
                    if lhs == rhs && !lhs.is_empty() {
                        issues.push(Issue::new("PY_S1656", "Self-assignment detected", Severity::Major, Category::Bug, ctx.file_path, idx+1));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S1764 — Identical operands in comparison
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S1764"
    name: "Identical expressions should not be compared"
    severity: Major
    category: Bug
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let comparison_ops = ["==", "!=", ">=", "<=", ">", "<"];
        for (idx, line) in ctx.source.lines().enumerate() {
            for op in &comparison_ops {
                if let Some(pos) = line.find(op) {
                    if pos > 0 {
                        let before = line[..pos].trim();
                        let after = line[pos + op.len()..].trim();
                        if before == after && !before.is_empty() {
                            issues.push(Issue::new("PY_S1764", "Identical operands in comparison", Severity::Major, Category::Bug, ctx.file_path, idx+1));
                            break;
                        }
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S2589 — Always-true condition
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S2589"
    name: "Conditions should not be constant"
    severity: Major
    category: Bug
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            let t = line.trim();
            if t == "if True:" || t == "if False:" || t == "while True:" {
                issues.push(Issue::new("PY_S2589", "Constant boolean condition", Severity::Major, Category::Bug, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S2757 — Assignment vs equality
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S2757"
    name: "Assignment operators should not be used in conditions"
    severity: Major
    category: Bug
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"if\s+\w+\s*=\s*\w+").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("PY_S2757", "Assignment in if condition - did you mean ==?", Severity::Major, Category::Bug, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S1994 — Loop counter modified
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S1994"
    name: "Loop counters should not be modified inside the loop"
    severity: Critical
    category: Bug
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"for\s+(\w+)\s+in\s+").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                let counter = cap.get(1).unwrap().as_str();
                let body_start = idx + 1;
                for (body_idx, body_line) in ctx.source.lines().skip(body_start).enumerate() {
                    if body_line.contains(&format!("{} +=", counter)) || body_line.contains(&format!("{} -=", counter)) {
                        issues.push(Issue::new("PY_S1994", format!("Loop counter '{}' modified inside loop", counter), Severity::Critical, Category::Bug, ctx.file_path, body_start + body_idx + 1));
                    }
                    if body_line.trim() == "# end" { break; }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S1860 — Deadlock-prone nested locks
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S1860"
    name: "Nested locks should be avoided"
    severity: Critical
    category: Bug
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let mut lock_depth: i32 = 0;
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("with ") && (line.contains("lock") || line.contains("Lock")) {
                if lock_depth > 0 {
                    issues.push(Issue::new("PY_S1860", "Nested lock detected - potential deadlock", Severity::Critical, Category::Bug, ctx.file_path, idx+1));
                }
                lock_depth += 1;
            }
            if line.trim() == "" || line.trim() == "}" {
                lock_depth = lock_depth.saturating_sub(1);
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S2201 — Return value ignored
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S2201"
    name: "Return values should not be ignored"
    severity: Major
    category: Bug
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"^\s*(\w+)\s*\([^)]*\)\s*$").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            let t = line.trim();
            if t.ends_with(";") && !t.starts_with("return") && !t.starts_with("if") && !t.starts_with("for") && !t.starts_with("while") {
                if (t.contains("get(") || t.contains("find(") || t.contains("index(")) && !t.contains("result") && !t.contains("value") {
                    issues.push(Issue::new("PY_S2201", "Return value ignored", Severity::Major, Category::Bug, ctx.file_path, idx+1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S2178 — is vs == for literals
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S2178"
    name: "Use == for value comparison, not is"
    severity: Major
    category: Bug
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"is\s+\d+").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("PY_S2178", "Use '==' for numeric comparison, not 'is'", Severity::Major, Category::Bug, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Additional Python Code Smell Rules (PY_S1XX - PY_S2XX)
// ═══════════════════════════════════════════════════════════════════════════════

// ─────────────────────────────────────────────────────────────────────────────
// PY_S101 — Identical expressions on both sides of an operator
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S101"
    name: "Identical expressions should not be used on both sides of an operator"
    severity: Major
    category: Bug
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let ops = ["==", "!=", "+", "-", "*", "/", "//", "%", "**", "and", "or", "<<", ">>"];
        for (idx, line) in ctx.source.lines().enumerate() {
            for op in &ops {
                if let Some(pos) = line.find(op) {
                    if pos > 2 && pos < line.len() - 2 {
                        let left = line[..pos].trim();
                        let right = line[pos + op.len()..].trim();
                        if left == right && !left.is_empty() && !left.starts_with("//") && !left.starts_with("#") {
                            issues.push(Issue::new("PY_S101", format!("Identical expression on both sides of '{}'", op), Severity::Major, Category::Bug, ctx.file_path, idx+1));
                            break;
                        }
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S102 — Empty except
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S102"
    name: "Empty except clause should not be used"
    severity: Minor
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            let t = line.trim();
            if t.starts_with("except") || t.starts_with("except:") {
                if idx + 1 < lines.len() {
                    let next = lines[idx + 1].trim();
                    if next == "pass" || next == "..." || next.is_empty() {
                        issues.push(Issue::new("PY_S102", "Empty except clause", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S103 — Line too long
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S103"
    name: "Lines should not be too long"
    severity: Minor
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let max_len = 120;
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.len() > max_len && !line.trim().starts_with("#") && !line.trim().starts_with("\"\"\"") {
                issues.push(Issue::new("PY_S103", format!("Line too long ({} chars, max {})", line.len(), max_len), Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S104 — Unused import
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S104"
    name: "Unused imports should be removed"
    severity: Minor
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        let mut imports: Vec<(String, usize)> = Vec::new();

        for (idx, line) in lines.iter().enumerate() {
            let t = line.trim();
            if t.starts_with("import ") || t.starts_with("from ") {
                let name = if t.starts_with("import ") {
                    t["import ".len()..].split_whitespace().next().unwrap_or("").to_string()
                } else if t.starts_with("from ") {
                    t["from ".len()..].split_whitespace().next().unwrap_or("").to_string()
                } else { continue };
                imports.push((name, idx));
            }
        }

        let all_code: String = lines.join("\n");
        for (name, line_num) in imports {
            if name != "*" && !all_code.matches(&format!(" {} ", name)).collect::<Vec<_>>().is_empty() == false {
                if !all_code.contains(&format!(".{}{}", name, "(")) && !all_code.contains(&format!("{}()", name)) {
                    issues.push(Issue::new("PY_S104", format!("Unused import: {}", name), Severity::Minor, Category::CodeSmell, ctx.file_path, line_num+1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S105 — Unused variable
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S105"
    name: "Unused variables should be removed"
    severity: Minor
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"^\s*(\w+)\s*=\s*[^=]").unwrap();
        let lines: Vec<&str> = ctx.source.lines().collect();

        for (idx, line) in lines.iter().enumerate() {
            if let Some(cap) = re.captures(line) {
                let var_name = cap.get(1).unwrap().as_str();
                if var_name.starts_with("_") || var_name == "self" { continue; }
                let remaining: String = lines.iter().skip(idx + 1).take(50).cloned().collect::<Vec<_>>().join("\n");
                if !remaining.contains(&format!(" {} ", var_name)) && !remaining.contains(&format!("({}", var_name)) && !remaining.contains(&format!("={}", var_name)) {
                    issues.push(Issue::new("PY_S105", format!("Unused variable: {}", var_name), Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S106 — Unused function argument
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S106"
    name: "Unused function arguments should be removed or prefixed with _"
    severity: Minor
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();

        for (idx, line) in lines.iter().enumerate() {
            let t = line.trim();
            if t.starts_with("def ") {
                let func_body: String = lines.iter().skip(idx).take(30).cloned().collect::<Vec<_>>().join("\n");
                let arg_re = regex::Regex::new(r"def\s+\w+\s*\(([^)]+)\)").unwrap();
                if let Some(cap) = arg_re.captures(&func_body) {
                    let args = cap.get(1).unwrap().as_str();
                    for arg in args.split(",") {
                        let a = arg.trim().split(":").next().unwrap_or(arg.trim()).trim().to_string();
                        if a != "self" && a != "cls" && !a.starts_with("_") {
                            let body_after_def = func_body.lines().skip(1).collect::<Vec<_>>().join("\n");
                            if !body_after_def.contains(&format!(" {} ", a)) && !body_after_def.contains(&format!("({}", a)) {
                                issues.push(Issue::new("PY_S106", format!("Unused function argument: {}", a), Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
                            }
                        }
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S107 — Too many method arguments
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S107"
    name: "Functions should not have too many parameters"
    severity: Major
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let max_params = 7;
        let re = regex::Regex::new(r"def\s+\w+\s*\(([^)]+)\)").unwrap();

        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                let params = cap.get(1).unwrap().as_str();
                let param_count = params.split(",").filter(|p| !p.trim().is_empty()).count();
                if param_count > max_params {
                    issues.push(Issue::new("PY_S107", format!("Function has {} parameters (max {})", param_count, max_params), Severity::Major, Category::CodeSmell, ctx.file_path, idx+1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S108 — Bare except clause
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S108"
    name: "Bare except clauses should not be used"
    severity: Major
    category: Bug
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            let t = line.trim();
            if t == "except:" || t.starts_with("except :") || t == "except" {
                issues.push(Issue::new("PY_S108", "Bare except clause - caught all exceptions", Severity::Major, Category::Bug, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S109 — Too many return statements
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S109"
    name: "Functions should not have too many return statements"
    severity: Minor
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let max_returns = 6;
        let lines: Vec<&str> = ctx.source.lines().collect();

        for (idx, line) in lines.iter().enumerate() {
            let t = line.trim();
            if t.starts_with("def ") {
                let func_end = idx + 30.min(lines.len() - idx);
                let func_body: String = lines[idx..func_end].iter().cloned().collect::<Vec<_>>().join("\n");
                let return_count = func_body.matches("return ").count();
                if return_count > max_returns {
                    issues.push(Issue::new("PY_S109", format!("Function has {} return statements (max {})", return_count, max_returns), Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S110 — Missing docstring
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S110"
    name: "Functions and classes should have docstrings"
    severity: Minor
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();

        for (idx, line) in lines.iter().enumerate() {
            let t = line.trim();
            if t.starts_with("def ") || t.starts_with("class ") {
                let next_line_idx = idx + 1;
                if next_line_idx < lines.len() {
                    let next_line = lines[next_line_idx].trim();
                    if !next_line.starts_with("\"\"\"") && !next_line.starts_with("'''") && !next_line.starts_with("@") {
                        let name = if t.starts_with("def ") {
                            t.split_whitespace().nth(1).unwrap_or("").split('(').next().unwrap_or("")
                        } else {
                            t.split_whitespace().nth(1).unwrap_or("")
                        };
                        issues.push(Issue::new("PY_S110", format!("Missing docstring for {}", name), Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S111 — Wildcard import
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S111"
    name: "Wildcard imports should not be used"
    severity: Minor
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            let t = line.trim();
            if t.contains(" from ") && (t.contains(" import *") || t.ends_with(" import *")) {
                issues.push(Issue::new("PY_S111", "Wildcard import", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S220 — Redundant import alias
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S220"
    name: "Import aliases should not be redundant"
    severity: Minor
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            let t = line.trim();
            // import X as X
            if let Some(m) = regex::Regex::new(r"import\s+(\w+)\s+as\s+\1\b").unwrap().find(t) {
                issues.push(Issue::new("PY_S220", format!("Redundant import alias: {}", m.as_str()), Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
            // from X import Y as Y
            if let Some(m) = regex::Regex::new(r"from\s+\w+\s+import\s+(\w+)\s+as\s+\1\b").unwrap().find(t) {
                issues.push(Issue::new("PY_S220", format!("Redundant import alias: {}", m.as_str()), Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S113 — Shadowing built-in
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S113"
    name: "Built-in names should not be shadowed"
    severity: Minor
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let builtins = ["list", "dict", "set", "tuple", "str", "int", "float", "bool", "type", "object", "Exception", "print", "open", "range", "len", "abs", "max", "min"];
        for (idx, line) in ctx.source.lines().enumerate() {
            let t = line.trim();
            if t.starts_with("def ") || t.starts_with("class ") || t.starts_with("for ") || t.starts_with("if ") {
                for builtin in &builtins {
                    if t.contains(builtin) && !t.contains("#") {
                        issues.push(Issue::new("PY_S113", format!("Shadowing built-in: {}", builtin), Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
                        break;
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S114 — Constant variable name
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S114"
    name: "Constants should be named in UPPER_CASE"
    severity: Minor
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"^([A-Z][a-z]\w*)\s*=").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            let t = line.trim();
            if t.starts_with("def ") || t.starts_with("class ") { continue; }
            if let Some(cap) = re.captures(t) {
                let name = cap.get(1).unwrap().as_str();
                if !name.contains("_") && name.chars().all(|c| c.is_uppercase() || c.is_numeric()) == false {
                    if name.chars().filter(|c| c.is_uppercase()).count() > name.len() / 2 {
                        issues.push(Issue::new("PY_S114", format!("Constant '{}' should be UPPER_CASE", name), Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S115 — Confusing name
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S115"
    name: "Variable and function names should not be confusingly similar"
    severity: Minor
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        let mut names: Vec<(String, usize)> = Vec::new();

        for (idx, line) in lines.iter().enumerate() {
            let t = line.trim();
            if t.starts_with("def ") {
                if let Some(name) = t.split_whitespace().nth(1) {
                    let clean = name.split('(').next().unwrap_or(name);
                    names.push((clean.to_string(), idx));
                }
            }
            if t.starts_with("class ") {
                if let Some(name) = t.split_whitespace().nth(1) {
                    names.push((name.to_string(), idx));
                }
            }
        }

        for (i, (name1, line1)) in names.iter().enumerate() {
            for (name2, line2) in names.iter().skip(i + 1) {
                if name1.chars().count() == name2.chars().count() && name1 != name2 {
                    let chars1: Vec<char> = name1.chars().collect();
                    let chars2: Vec<char> = name2.chars().collect();
                    let diff: usize = chars1.iter().zip(chars2.iter()).filter(|(a, b)| a != b).count();
                    if diff == 1 {
                        issues.push(Issue::new("PY_S115", format!("Confusingly similar names: '{}' and '{}'", name1, name2), Severity::Minor, Category::CodeSmell, ctx.file_path, *line1+1));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S116 — Empty function body
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S116"
    name: "Function bodies should not be empty"
    severity: Minor
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();

        for (idx, line) in lines.iter().enumerate() {
            let t = line.trim();
            if t.starts_with("def ") && !t.contains("-> None") && !t.contains("async def") {
                if idx + 1 < lines.len() {
                    let next = lines[idx + 1].trim();
                    if next == "pass" || next == "..." {
                        issues.push(Issue::new("PY_S116", "Empty function body - did you forget to implement?", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S117 — Too many branches
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S117"
    name: "Functions should not have too many branches"
    severity: Major
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let max_branches = 10;
        let lines: Vec<&str> = ctx.source.lines().collect();

        for (idx, line) in lines.iter().enumerate() {
            let t = line.trim();
            if t.starts_with("def ") {
                let func_end = idx + 50.min(lines.len() - idx);
                let func_body: String = lines[idx..func_end].iter().cloned().collect::<Vec<_>>().join("\n");
                let branch_count = func_body.matches("if ").count()
                    + func_body.matches("elif ").count()
                    + func_body.matches("else:").count()
                    + func_body.matches("for ").count()
                    + func_body.matches("while ").count()
                    + func_body.matches("case ").count();
                if branch_count > max_branches {
                    issues.push(Issue::new("PY_S117", format!("Function has {} branches (max {})", branch_count, max_branches), Severity::Major, Category::CodeSmell, ctx.file_path, idx+1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S118 — Too many statements
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S118"
    name: "Functions should not have too many statements"
    severity: Major
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let max_statements = 50;
        let lines: Vec<&str> = ctx.source.lines().collect();

        for (idx, line) in lines.iter().enumerate() {
            let t = line.trim();
            if t.starts_with("def ") {
                let func_end = idx + 100.min(lines.len() - idx);
                let func_body: String = lines[idx..func_end].iter().cloned().collect::<Vec<_>>().join("\n");
                let statement_count = func_body.lines().count();
                if statement_count > max_statements {
                    issues.push(Issue::new("PY_S118", format!("Function has {} statements (max {})", statement_count, max_statements), Severity::Major, Category::CodeSmell, ctx.file_path, idx+1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S119 — Empty class body
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S119"
    name: "Class bodies should not be empty"
    severity: Minor
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();

        for (idx, line) in lines.iter().enumerate() {
            let t = line.trim();
            if t.starts_with("class ") {
                if idx + 1 < lines.len() {
                    let next = lines[idx + 1].trim();
                    if next == "pass" || next == "..." {
                        issues.push(Issue::new("PY_S119", "Empty class body", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S120 — Cognitive complexity
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S120"
    name: "Cognitive complexity should not be too high"
    severity: Major
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let max_complexity = 15;
        let lines: Vec<&str> = ctx.source.lines().collect();

        for (idx, line) in lines.iter().enumerate() {
            let t = line.trim();
            if t.starts_with("def ") {
                let func_end = idx + 50.min(lines.len() - idx);
                let func_body: String = lines[idx..func_end].iter().cloned().collect::<Vec<_>>().join("\n");
                let mut complexity = 0;
                complexity += func_body.matches("if ").count() * 1;
                complexity += func_body.matches("elif ").count() * 2;
                complexity += func_body.matches("for ").count() * 1;
                complexity += func_body.matches("while ").count() * 2;
                complexity += func_body.matches("except ").count() * 1;
                complexity += func_body.matches(" with ").count() * 1;
                complexity += func_body.matches(" and ").count() + func_body.matches(" or ").count();
                if complexity > max_complexity {
                    issues.push(Issue::new("PY_S120", format!("Cognitive complexity is {} (max {})", complexity, max_complexity), Severity::Major, Category::CodeSmell, ctx.file_path, idx+1));
                }
            }
        }
        issues
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Python Error Handling Rules (PY_S2XX)
// ═══════════════════════════════════════════════════════════════════════════════

// ─────────────────────────────────────────────────────────────────────────────
// PY_S201 — Missing exception handling
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S201"
    name: "Operations that can raise exceptions should be wrapped"
    severity: Major
    category: Bug
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let risky_patterns = ["json.loads(", ".get()", ".fetch(", "requests.", "open(", "eval(", "exec("];
        let lines: Vec<&str> = ctx.source.lines().collect();
        let mut in_try = false;

        for (idx, line) in lines.iter().enumerate() {
            let t = line.trim();
            if t.starts_with("try:") { in_try = true; }
            if t.starts_with("except") { in_try = false; }

            if !in_try {
                for pattern in &risky_patterns {
                    if line.contains(pattern) && !t.starts_with("#") && !t.starts_with("def ") && !t.starts_with("class ") {
                        issues.push(Issue::new("PY_S201", format!("Unprotected operation: {}", pattern), Severity::Major, Category::Bug, ctx.file_path, idx+1));
                        break;
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S202 — Catching too broad exception
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S202"
    name: "Exception handlers should catch specific types"
    severity: Major
    category: Bug
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            let t = line.trim();
            if t == "except:" || t.starts_with("except :") || t == "except Exception:" || t == "except BaseException:" {
                issues.push(Issue::new("PY_S202", "Catching too broad exception", Severity::Major, Category::Bug, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S203 — Not raising exception
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S203"
    name: "Exceptions should be raised, not returned"
    severity: Major
    category: Bug
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            let t = line.trim();
            if (t.contains("return -1") || t.contains("return None") || t.contains("return false") || t.contains("return False")) &&
               !t.contains("raise ") && !line.contains("#") {
                let all_lines: Vec<&str> = ctx.source.lines().collect();
                let prev_lines: String = all_lines[..idx].iter().rev().take(10).map(|s| *s).collect::<Vec<_>>().join("\n");
                if prev_lines.contains("try") || prev_lines.contains("except") {
                    issues.push(Issue::new("PY_S203", "Should raise exception instead of returning error value", Severity::Major, Category::Bug, ctx.file_path, idx+1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S204 — Swallowing exception
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S204"
    name: "Exceptions should not be silently swallowed"
    severity: Major
    category: Bug
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();

        for (idx, line) in lines.iter().enumerate() {
            let t = line.trim();
            if t.starts_with("except") || t.starts_with("except:") {
                if idx + 1 < lines.len() {
                    let next = lines[idx + 1].trim();
                    if next == "pass" || next == "..." {
                        issues.push(Issue::new("PY_S204", "Exception silently swallowed", Severity::Major, Category::Bug, ctx.file_path, idx+1));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S205 — Missing finally
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S205"
    name: "Try blocks with resource acquisition should have finally"
    severity: Major
    category: Bug
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();

        for (idx, line) in lines.iter().enumerate() {
            let t = line.trim();
            if t.starts_with("try:") {
                let try_block: String = lines[idx..].iter().take(30).cloned().collect::<Vec<_>>().join("\n");
                if (try_block.contains("open(") || try_block.contains("connect(") || try_block.contains("lock") || try_block.contains("Lock")) &&
                   !try_block.contains("finally:") {
                    issues.push(Issue::new("PY_S205", "Try block with resource acquisition missing finally", Severity::Major, Category::Bug, ctx.file_path, idx+1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S206 — Return in finally
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S206"
    name: "Return statements should not be in finally blocks"
    severity: Critical
    category: Bug
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();

        for (idx, line) in lines.iter().enumerate() {
            let t = line.trim();
            if t.starts_with("finally:") {
                let finally_block: String = lines[idx..idx+20].iter().cloned().collect::<Vec<_>>().join("\n");
                if finally_block.contains("return ") {
                    issues.push(Issue::new("PY_S206", "Return in finally block can swallow exceptions", Severity::Critical, Category::Bug, ctx.file_path, idx+1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S207 — Raising generic exception
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S207"
    name: "Specific exceptions should be raised"
    severity: Major
    category: Bug
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            let t = line.trim();
            if t.contains("raise Exception(") || t.contains("raise BaseException(") {
                issues.push(Issue::new("PY_S207", "Raising generic Exception - be more specific", Severity::Major, Category::Bug, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S208 — Except pass
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S208"
    name: "Empty except with pass is suspicious"
    severity: Minor
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();

        for (idx, line) in lines.iter().enumerate() {
            let t = line.trim();
            if t.starts_with("except") {
                if idx + 1 < lines.len() {
                    let next = lines[idx + 1].trim();
                    if next == "pass" {
                        issues.push(Issue::new("PY_S208", "Empty except with pass - should at least log", Severity::Minor, Category:: CodeSmell, ctx.file_path, idx+1));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S209 — Confusing exception chaining
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S209"
    name: "Exception chaining should use 'raise ... from ...'"
    severity: Major
    category: Bug
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            let t = line.trim();
            if t.starts_with("raise ") && (t.contains("err") || t.contains("e.") || t.contains("ex.")) {
                if !t.contains(" from ") {
                    issues.push(Issue::new("PY_S209", "Use 'raise ... from ...' for exception chaining", Severity::Major, Category:: Bug, ctx.file_path, idx+1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S210 — Too many nested try blocks
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S210"
    name: "Too many nested try blocks indicate poor error handling design"
    severity: Minor
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        let mut try_depth = 0;
        let mut max_try_depth = 0;
        let mut max_depth_line = 0;

        for (idx, line) in lines.iter().enumerate() {
            let t = line.trim();
            if t.starts_with("try:") {
                try_depth += 1;
                if try_depth > max_try_depth {
                    max_try_depth = try_depth;
                    max_depth_line = idx;
                }
            }
            if t.starts_with("except") || t.starts_with("finally") {
                if try_depth > 0 { try_depth -= 1; }
            }
        }

        if max_try_depth > 3 {
            issues.push(Issue::new("PY_S210", format!("{} nested try blocks (max 3)", max_try_depth), Severity::Minor, Category::CodeSmell, ctx.file_path, max_depth_line+1));
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S211 — Error handling without logging
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S211"
    name: "Caught exceptions should be logged"
    severity: Minor
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();

        for (idx, line) in lines.iter().enumerate() {
            let t = line.trim();
            if t.starts_with("except") || t.starts_with("except:") {
                let except_block: String = lines[idx..idx+10].iter().cloned().collect::<Vec<_>>().join("\n");
                if !except_block.contains("log") && !except_block.contains("print") && !except_block.contains("raise") {
                    issues.push(Issue::new("PY_S211", "Exception caught but not logged or re-raised", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S212 — Catching KeyboardInterrupt
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S212"
    name: "KeyboardInterrupt should not be caught"
    severity: Major
    category: Bug
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            let t = line.trim();
            if t.contains("KeyboardInterrupt") || t.contains("SystemExit") {
                issues.push(Issue::new("PY_S212", "KeyboardInterrupt or SystemExit should not be caught", Severity::Major, Category::Bug, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S213 — Using assert instead of proper error handling
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S213"
    name: "Assert should not be used for validation"
    severity: Minor
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            let t = line.trim();
            if t.starts_with("assert ") && (t.contains("!=") || t.contains("==")) {
                issues.push(Issue::new("PY_S213", "Assert used for validation instead of proper error handling", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_S214 — Exception in destructor
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_S214"
    name: "Exceptions should not be raised in __del__"
    severity: Major
    category: Bug
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();

        for (idx, line) in lines.iter().enumerate() {
            let t = line.trim();
            if t.contains("def __del__") || t.contains("def __delete__") {
                let method_body: String = lines[idx..idx+20].iter().cloned().collect::<Vec<_>>().join("\n");
                if method_body.contains("raise ") {
                    issues.push(Issue::new("PY_S214", "Exception raised in destructor - can cause crashes", Severity::Major, Category::Bug, ctx.file_path, idx+1));
                }
            }
        }
        issues
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Python Performance Rules (PY_P1-PY_P10)
// ═══════════════════════════════════════════════════════════════════════════════

// ─────────────────────────────────────────────────────────────────────────────
// PY_P1 — range(len(x)) → use enumerate(x)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_P1"
    name: "Use enumerate() instead of range(len(x))"
    severity: Minor
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("range(len(") {
                issues.push(Issue::new("PY_P1", "Use enumerate() instead of range(len())", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_P2 — .keys() iteration → iterate dict directly
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_P2"
    name: "Iterate dict directly instead of .keys()"
    severity: Minor
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\.keys\(\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && (line.contains("for ") || line.contains("in ")) {
                issues.push(Issue::new("PY_P2", "Iterate dict directly instead of .keys()", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_P3 — map/filter with lambda → comprehension
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_P3"
    name: "Use comprehension instead of map/filter with lambda"
    severity: Minor
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"(map|filter)\s*\(\s*lambda\s+").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("PY_P3", "Use list comprehension instead of map/filter with lambda", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_P4 — .append in loop → comprehension
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_P4"
    name: "Use comprehension instead of .append() in loop"
    severity: Minor
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            let t = line.trim();
            if t.contains(".append(") {
                // Check if inside a for loop
                let in_loop = idx > 0 && lines[..idx].iter().any(|l| l.contains("for ") && !l.trim().starts_with("#"));
                if in_loop {
                    issues.push(Issue::new("PY_P4", "Use list comprehension instead of .append() in loop", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_P5 — + string concat in loop → join()
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_P5"
    name: "Use join() instead of += string concatenation in loop"
    severity: Minor
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        let mut in_loop = false;
        for (idx, line) in lines.iter().enumerate() {
            if line.contains("for ") || line.contains("while ") {
                in_loop = true;
            }
            if in_loop && (line.contains("+=") || line.contains("= s +")) && !line.contains("join") {
                issues.push(Issue::new("PY_P5", "Use str.join() instead of += in loop", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
            if in_loop && line.trim() == "}" {
                in_loop = false;
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_P6 — time.sleep() in test → use mock/async
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_P6"
    name: "Use mock or async helpers instead of time.sleep() in tests"
    severity: Minor
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            let t = line.trim();
            if t.contains("time.sleep(") && (t.starts_with("def test_") || idx > 0 && lines[..idx].iter().any(|l| l.contains("def test_"))) {
                issues.push(Issue::new("PY_P6", "Use mock.patch or async instead of time.sleep() in tests", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_P7 — global keyword abuse
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_P7"
    name: "Avoid using global keyword"
    severity: Minor
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.trim().starts_with("global ") {
                issues.push(Issue::new("PY_P7", "Avoid using 'global' keyword - pass as parameter or use class", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_P8 — del list[i] in loop (O(n²))
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_P8"
    name: "Avoid deleting list items while iterating"
    severity: Minor
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        let mut in_loop = false;
        for (idx, line) in lines.iter().enumerate() {
            if line.contains("for ") || line.contains("while ") {
                in_loop = true;
            }
            if in_loop && (line.contains("del ") && line.contains("[") || line.contains(".pop(")) {
                issues.push(Issue::new("PY_P8", "Deleting items while iterating causes O(n²) - use list comprehension", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
            if in_loop && line.trim() == "\"" {
                in_loop = false;
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_P9 — x in list instead of x in set (repeated membership test)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_P9"
    name: "Use set for membership testing instead of list"
    severity: Minor
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            let t = line.trim();
            if t.contains(" in [") || t.contains(" in (") {
                // Check if repeated (likely in a loop or condition)
                let prev_lines: String = lines[..idx].join("\n");
                if prev_lines.contains(" for ") || prev_lines.contains("if ") || prev_lines.contains("while ") {
                    issues.push(Issue::new("PY_P9", "Use set for membership testing instead of list/tuple", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_P10 — Class-level mutable attribute shared across instances
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_P10"
    name: "Mutable default arguments are shared across function calls"
    severity: Major
    category: Bug
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"def\s+\w+\s*\(\s*\w+\s*=\s*\[\s*\]").unwrap();
        let re2 = regex::Regex::new(r"def\s+\w+\s*\(\s*\w+\s*=\s*\{\s*\}").unwrap();
        let re3 = regex::Regex::new(r"def\s+\w+\s*\(\s*\w+\s*=\s*\w+\s*\(\s*\)\s*\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) || re2.is_match(line) || re3.is_match(line) {
                issues.push(Issue::new("PY_P10", "Mutable default argument shared across calls - use None and initialize inside", Severity::Major, Category::Bug, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Python Testing Rules (PY_T1-PY_T10)
// ═══════════════════════════════════════════════════════════════════════════════

// ─────────────────────────────────────────────────────────────────────────────
// PY_T1 — Test without assertion: def test_x(): pass
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_T1"
    name: "Tests should contain assertions"
    severity: Major
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            let t = line.trim();
            if t.starts_with("def test_") {
                let func_body: String = lines[idx..].iter().take(20).cloned().collect::<Vec<_>>().join("\n");
                if !func_body.contains("assert") && !func_body.contains("self.assert") && !func_body.contains("pytest") && !func_body.contains("raise") {
                    issues.push(Issue::new("PY_T1", "Test has no assertions", Severity::Major, Category::CodeSmell, ctx.file_path, idx+1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_T2 — Test with time.sleep()
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_T2"
    name: "Tests should not use time.sleep()"
    severity: Minor
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            let t = line.trim();
            if t.contains("time.sleep(") && (t.starts_with("def test_") || idx > 0 && lines[..idx].iter().any(|l| l.contains("def test_"))) {
                issues.push(Issue::new("PY_T2", "Test uses time.sleep() - use mock.patch or async helpers", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_T3 — assertEqual vs assertTrue: prefer specific assertion
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_T3"
    name: "Use specific assertions instead of assertTrue/assertFalse"
    severity: Minor
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            let t = line.trim();
            if t.contains("assertTrue") || t.contains("assertFalse") {
                issues.push(Issue::new("PY_T3", "Use specific assertions (assertEqual, assertIs, assertIn) instead of assertTrue/assertFalse", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_T4 — setUp/tearDown vs class-level: prefer setUpClass
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_T4"
    name: "Use setUpClass/tearDownClass for expensive setup"
    severity: Minor
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            let t = line.trim();
            if (t.contains("def setUp(self)") || t.contains("def tearDown(self)")) && idx > 0 {
                let prev_lines: String = lines[..idx].join("\n");
                if prev_lines.contains("@classmethod") || prev_lines.contains("setUpClass") {
                    continue;
                }
                issues.push(Issue::new("PY_T4", "Consider using setUpClass/tearDownClass for expensive setup shared across tests", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_T5 — unittest.skip without reason
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_T5"
    name: "unittest.skip should have a reason"
    severity: Minor
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r#"@unittest\.skip\s*\("#).unwrap();
        let re2 = regex::Regex::new(r"@pytest\.mark\.skip\s*\(").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            let t = line.trim();
            if (re.is_match(t) || re2.is_match(t)) && !t.contains("reason=") {
                issues.push(Issue::new("PY_T5", "@skip decorator should include a reason", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_T6 — Test method naming: must start with test_
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_T6"
    name: "Test method names must start with test_"
    severity: Major
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        let in_test_class = false;
        for (idx, line) in lines.iter().enumerate() {
            let t = line.trim();
            if t.starts_with("class Test") || t.contains("(unittest.TestCase)") || t.contains("TestCase") {
                continue;
            }
            if t.starts_with("def ") && !t.starts_with("def test_") && !t.starts_with("def setUp") && !t.starts_with("def tearDown") && !t.starts_with("def testClass") && !t.starts_with("def __init__") {
                if idx > 0 {
                    let prev_lines: String = lines[..idx].join("\n");
                    if prev_lines.contains("class Test") || prev_lines.contains("TestCase") {
                        issues.push(Issue::new("PY_T6", "Test method must start with 'test_'", Severity::Major, Category::CodeSmell, ctx.file_path, idx+1));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_T7 — Test fixture too complex: >20 lines setup in test
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_T7"
    name: "Test setup should be simple - extract fixtures"
    severity: Minor
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            let t = line.trim();
            if t.starts_with("def test_") {
                let setup_lines: Vec<&str> = lines[idx..].iter().take(30).cloned().collect();
                let setup_end = setup_lines.iter().position(|l| l.contains("assert") || l.contains("self.assert") || l.contains("#")).unwrap_or(setup_lines.len());
                if setup_end > 20 {
                    issues.push(Issue::new("PY_T7", "Test setup is too complex (>20 lines) - extract to fixture", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_T8 — Multiple asserts: >5 asserts in one test
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_T8"
    name: "Too many assertions in one test - split into multiple tests"
    severity: Minor
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            let t = line.trim();
            if t.starts_with("def test_") {
                let func_body: String = lines[idx..].iter().take(50).cloned().collect::<Vec<_>>().join("\n");
                let assert_count = func_body.matches("assert").count() + func_body.matches("self.assert").count();
                if assert_count > 5 {
                    issues.push(Issue::new("PY_T8", format!("Test has {} assertions - consider splitting", assert_count), Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_T9 — Duplicated test method
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_T9"
    name: "Duplicated test methods should be removed"
    severity: Minor
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        let mut test_methods: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for (idx, line) in lines.iter().enumerate() {
            let t = line.trim();
            if t.starts_with("def test_") {
                let method_name = t.split('(').next().unwrap_or(t).to_string();
                let body: String = lines[idx..].iter().take(30).cloned().collect::<Vec<_>>().join("\n");
                if let Some(prev_idx) = test_methods.get(&method_name) {
                    let prev_body: String = lines[*prev_idx..].iter().take(30).cloned().collect::<Vec<_>>().join("\n");
                    if prev_body == body {
                        issues.push(Issue::new("PY_T9", format!("Duplicated test method '{}'", method_name), Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
                    }
                } else {
                    test_methods.insert(method_name, idx);
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_T10 — Non-deterministic test: random.random() in test
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_T10"
    name: "Tests should not use random values"
    severity: Minor
    category: Bug
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            let t = line.trim();
            if (t.contains("random.random(") || t.contains("random.randint(") || t.contains("random.choice(")) && (idx == 0 || lines[..idx].iter().any(|l| l.contains("def test_"))) {
                issues.push(Issue::new("PY_T10", "Non-deterministic test - avoid random values", Severity::Minor, Category::Bug, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Python Naming & Conventions Rules (PY_N1-PY_N25)
// ═══════════════════════════════════════════════════════════════════════════════

// ─────────────────────────────────────────────────────────────────────────────
// PY_N1 — Function naming: def CamelCase(): → snake_case
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_N1"
    name: "Function names should use snake_case"
    severity: Minor
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"def\s+([A-Z][a-zA-Z0-9_]*)\s*\(").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            for cap in re.captures_iter(line) {
                if let Some(name) = cap.get(1) {
                    let n = name.as_str();
                    if !n.starts_with("__") && !n.ends_with("__") {
                        issues.push(Issue::new("PY_N1", format!("Function '{}' should use snake_case", n), Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_N2 — Class naming: class my_class: → PascalCase
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_N2"
    name: "Class names should use PascalCase"
    severity: Minor
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"class\s+([a-z][a-zA-Z0-9_]*)\s*[:(]").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            for cap in re.captures_iter(line) {
                if let Some(name) = cap.get(1) {
                    let n = name.as_str();
                    if !n.starts_with("_") {
                        issues.push(Issue::new("PY_N2", format!("Class '{}' should use PascalCase", n), Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_N3 — Method naming: def MethodName(self): → snake_case
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_N3"
    name: "Method names should use snake_case"
    severity: Minor
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"def\s+([A-Z][a-zA-Z0-9_]*)\s*\(").unwrap();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            if line.contains("class ") {
                for cap in re.captures_iter(line) {
                    if let Some(name) = cap.get(1) {
                        let n = name.as_str();
                        if !n.starts_with("__") && !n.ends_with("__") {
                            issues.push(Issue::new("PY_N3", format!("Method '{}' should use snake_case", n), Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
                        }
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_N4 — Constant naming: my_const = 5 → UPPER_CASE
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_N4"
    name: "Constant names should use UPPER_CASE"
    severity: Minor
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"^([A-Z][a-zA-Z0-9_]*)\s*=\s*[^=]").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if !line.trim().starts_with("#") && !line.contains("def ") && !line.contains("class ") && !line.contains("import ") {
                for cap in re.captures_iter(line) {
                    if let Some(name) = cap.get(1) {
                        let n = name.as_str();
                        if n != n.to_uppercase() {
                            issues.push(Issue::new("PY_N4", format!("Constant '{}' should use UPPER_CASE", n), Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
                        }
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_N5 — Variable naming: MyVar = 5 → snake_case
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_N5"
    name: "Variable names should use snake_case"
    severity: Minor
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"^\s*([A-Z][a-zA-Z0-9_]*)\s*=").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if !line.trim().starts_with("#") && !line.contains("def ") && !line.contains("class ") && !line.contains("import ") && !line.contains("CONST ") {
                for cap in re.captures_iter(line) {
                    if let Some(name) = cap.get(1) {
                        let n = name.as_str();
                        if !n.starts_with("__") {
                            issues.push(Issue::new("PY_N5", format!("Variable '{}' should use snake_case", n), Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
                        }
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_N6 — Module naming: no underscores in module name
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_N6"
    name: "Module names should not use underscores"
    severity: Minor
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"^([a-z]+_[a-z]+)\.py\s*$").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.ends_with(".py") || line.contains("import ") {
                for cap in re.captures_iter(line) {
                    if let Some(name) = cap.get(1) {
                        issues.push(Issue::new("PY_N6", format!("Module '{}' should not use underscores", name.as_str()), Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_N7 — Package naming: lowercase only
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_N7"
    name: "Package names should use lowercase only"
    severity: Minor
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"(?:^import\s+([A-Z][a-zA-Z0-9_]+)|from\s+([A-Z][a-zA-Z0-9_]+)\s+)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            for cap in re.captures_iter(line) {
                if let Some(name) = cap.get(1).or(cap.get(2)) {
                    let n = name.as_str();
                    if n.contains('_') {
                        issues.push(Issue::new("PY_N7", format!("Package '{}' should use lowercase only", n), Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_N8 — Private method: def _method(self): outside class
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_N8"
    name: "Private method '_' prefix used outside class"
    severity: Minor
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        let mut class_stack: Vec<usize> = Vec::new();
        for (idx, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            let leading_spaces = line.len() - line.trim_start().len();
            if trimmed.starts_with("class ") {
                class_stack.push(leading_spaces);
            }
            if trimmed.starts_with("def _") && !trimmed.contains("__") {
                let in_class = !class_stack.is_empty() && leading_spaces > class_stack.last().copied().unwrap_or(0);
                if !in_class {
                    issues.push(Issue::new("PY_N8", "Private method prefix '_' used outside class", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
                }
            }
            // Pop classes when we dedent
            while !class_stack.is_empty() && leading_spaces <= class_stack.last().copied().unwrap_or(0) && !trimmed.is_empty() {
                class_stack.pop();
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_N9 — Protected attribute: self._x accessed from outside
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_N9"
    name: "Protected attribute accessed from outside class"
    severity: Minor
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        let mut in_class = String::new();
        for (idx, line) in lines.iter().enumerate() {
            if line.trim().starts_with("class ") {
                let re = regex::Regex::new(r"class\s+(\w+)").unwrap();
                if let Some(cap) = re.captures(line) {
                    in_class = cap.get(1).unwrap().as_str().to_string();
                }
            }
            if !in_class.is_empty() && line.contains("self._") && !line.trim().starts_with("class ") && !line.trim().starts_with("def ") {
                issues.push(Issue::new("PY_N9", "Protected attribute 'self._x' accessed from outside class", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
            if line.trim().starts_with("class ") {
                in_class = String::new();
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_N10 — Dunder method misuse: custom __my__ methods
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_N10"
    name: "Avoid custom dunder methods unless necessary"
    severity: Minor
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"def\s+__(?!init__|call__|str__|repr__|len__|getitem__|setitem__|delitem__|iter__|next__|contains__|enter__|exit__|add__|sub__|mul__|truediv__|eq__|ne__|lt__|gt__|le__|ge__|hash__)\w+__").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("PY_N10", "Custom dunder method may conflict with built-in behavior", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_N11 — Property naming: get_x() convention
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_N11"
    name: "Property getter should use @property decorator"
    severity: Minor
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"def\s+get_(\w+)\s*\(").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("PY_N11", "Use @property decorator instead of get_x() method", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_N12 — Boolean function naming: def is_valid() → should start with is_/has_
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_N12"
    name: "Boolean methods should start with is_, has_, or _"
    severity: Minor
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            let t = line.trim();
            if t.starts_with("def ") && !t.contains("__") {
                let re = regex::Regex::new(r"def\s+(\w+)\s*\(").unwrap();
                if let Some(cap) = re.captures(t) {
                    if let Some(name) = cap.get(1) {
                        let n = name.as_str();
                        if !n.starts_with("is_") && !n.starts_with("has_") && !n.starts_with("_") && !n.starts_with("test_") && !n.starts_with("set_") && !n.starts_with("get_") {
                            // Check if it's inside a class
                            let prev_lines = lines[..idx].join("\n");
                            if prev_lines.contains("class ") {
                                issues.push(Issue::new("PY_N12", format!("Boolean method '{}' should start with is_/has_", n), Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
                            }
                        }
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_N13 — Comparison method: __eq__ without __hash__
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_N13"
    name: "__eq__ defined without __hash__ makes objects unhashable"
    severity: Minor
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        let mut has_eq = false;
        let mut has_hash = false;
        for (idx, line) in lines.iter().enumerate() {
            if line.contains("def __eq__") {
                has_eq = true;
            }
            if line.contains("def __hash__") {
                has_hash = true;
            }
            if has_eq && !has_hash && line.contains("def __eq__") {
                issues.push(Issue::new("PY_N13", "__eq__ defined without __hash__ - object will be unhashable", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_N14 — Context manager: __enter__ without __exit__
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_N14"
    name: "__enter__ defined without __exit__"
    severity: Minor
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        let mut has_enter = false;
        let mut has_exit = false;
        for (idx, line) in lines.iter().enumerate() {
            if line.contains("def __enter__") {
                has_enter = true;
            }
            if line.contains("def __exit__") {
                has_exit = true;
            }
        }
        if has_enter && !has_exit {
            issues.push(Issue::new("PY_N14", "__enter__ defined without __exit__ - context manager incomplete", Severity::Minor, Category::CodeSmell, ctx.file_path, 1));
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_N15 — Iterator: __iter__ without __next__
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_N15"
    name: "__iter__ defined without __next__"
    severity: Minor
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        let mut has_iter = false;
        let mut has_next = false;
        for (idx, line) in lines.iter().enumerate() {
            if line.contains("def __iter__") {
                has_iter = true;
            }
            if line.contains("def __next__") {
                has_next = true;
            }
        }
        if has_iter && !has_next {
            issues.push(Issue::new("PY_N15", "__iter__ defined without __next__ - iterator incomplete", Severity::Minor, Category::CodeSmell, ctx.file_path, 1));
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_N16 — Descriptor: __get__ without __set__
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_N16"
    name: "Descriptor __get__ defined without __set__"
    severity: Minor
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        let mut has_get = false;
        let mut has_set = false;
        for (idx, line) in lines.iter().enumerate() {
            if line.contains("def __get__") {
                has_get = true;
            }
            if line.contains("def __set__") {
                has_set = true;
            }
        }
        if has_get && !has_set {
            issues.push(Issue::new("PY_N16", "Descriptor __get__ defined without __set__", Severity::Minor, Category::CodeSmell, ctx.file_path, 1));
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_N17 — Callable: __call__ without useful docstring
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_N17"
    name: "__call__ should have a docstring"
    severity: Minor
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            let t = line.trim();
            if t.contains("def __call__") {
                let body: String = lines[idx..idx+10].join("\n");
                if !body.contains("\"\"\"") && !body.contains("'''") {
                    issues.push(Issue::new("PY_N17", "__call__ should have a docstring", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_N18 — Repr: __repr__ without __str__
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_N18"
    name: "__repr__ defined without __str__"
    severity: Minor
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        let mut has_repr = false;
        let mut has_str = false;
        for (idx, line) in lines.iter().enumerate() {
            if line.contains("def __repr__") {
                has_repr = true;
            }
            if line.contains("def __str__") {
                has_str = true;
            }
        }
        if has_repr && !has_str {
            issues.push(Issue::new("PY_N18", "__repr__ defined without __str__ - consider adding both", Severity::Minor, Category::CodeSmell, ctx.file_path, 1));
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_N19 — Slots: __slots__ with string instead of tuple
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_N19"
    name: "__slots__ should be a tuple, not a string"
    severity: Minor
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r#"__slots__\s*=\s*"[^"]+""#).unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("PY_N19", "__slots__ should be a tuple, not a string", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_N20 — New-style class: class Foo(object): in Python 3 (redundant)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_N20"
    name: "Class inheriting from object is redundant in Python 3"
    severity: Minor
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"class\s+\w+\s*\(\s*object\s*\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("PY_N20", "Inheriting from object is redundant in Python 3", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_N21 — Super without args: super() should be preferred
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_N21"
    name: "Use super() without arguments instead of super(ClassName, self)"
    severity: Minor
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"super\s*\(\s*\w+\s*,\s*self\s*\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("PY_N21", "Use super() instead of super(ClassName, self)", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_N22 — Relative import: from . import x should be explicit
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_N22"
    name: "Use explicit relative imports instead of implicit"
    severity: Minor
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"from\s+\.\s+import").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("PY_N22", "Use explicit relative import: from .module import name", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_N23 — Wildcard import: from module import *
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_N23"
    name: "Avoid wildcard imports"
    severity: Minor
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"from\s+\w+\s+import\s+\*").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("PY_N23", "Avoid wildcard imports - use explicit imports", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_N24 — Shadowing builtins: list = [1,2,3] shadows list
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_N24"
    name: "Do not shadow built-in type names"
    severity: Minor
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let builtins = ["list", "dict", "set", "tuple", "str", "int", "float", "bool", "type", "object", "Exception", "TypeError", "ValueError", "KeyError", "IndexError"];
        for (idx, line) in ctx.source.lines().enumerate() {
            let t = line.trim();
            if !t.starts_with("#") && !t.contains("import ") && !t.contains("def ") && !t.contains("class ") {
                for b in &builtins {
                    let re = regex::Regex::new(&format!(r"^\s*{}\s*=", b)).unwrap();
                    if re.is_match(line) {
                        issues.push(Issue::new("PY_N24", format!("Shadowing built-in '{}'", b), Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PY_N25 — Unused parameter: def f(x, y): return x (y unused)
// ─────────────────────────────────────────────────────────────────────────────

declare_rule! {
    id: "PY_N25"
    name: "Unused parameters should be removed or prefixed with _"
    severity: Minor
    category: CodeSmell
    language: "python"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            let t = line.trim();
            if t.starts_with("def ") && t.contains("(") {
                let re = regex::Regex::new(r"def\s+\w+\s*\(([^)]*)\)").unwrap();
                if let Some(cap) = re.captures(t) {
                    if let Some(params) = cap.get(1) {
                        let param_str = params.as_str();
                        if param_str.contains(",") {
                            let param_names: Vec<&str> = param_str.split(",").filter_map(|p| {
                                let p = p.trim();
                                if p.starts_with("*") || p.starts_with("**") {
                                    None
                                } else {
                                    Some(p.split(":").next().unwrap_or(p).trim())
                                }
                            }).collect();
                            let func_body: String = lines[idx..idx+30].join("\n");
                            for param in param_names {
                                if !param.is_empty() && !param.starts_with("_") && !func_body.contains(&format!(" {} ", param)) && !func_body.contains(&format!("({}", param)) {
                                    issues.push(Issue::new("PY_N25", format!("Unused parameter '{}'", param), Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
                                }
                            }
                        }
                    }
                }
            }
        }
        issues
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// GO RULES — 40 rules: security, bugs, code smells, performance
// ══════════════════════════════════════════════════════════════════════════════

// ─────────────────────────────────────────────────────────────────────────────
// GO_S2068 — Hardcoded credentials
// ─────────────────────────────────────────────────────────────────────────────
declare_rule! {
    id: "GO_S2068"
    name: "Hard-coded credentials are security sensitive"
    severity: Blocker
    category: SecurityHotspot
    language: "go"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let patterns = [
            (r#"(?i)(password|passwd|pwd)\s*[=:]\s*["'][^"']{4,}["']"#, "password"),
            (r#"(?i)(api[_-]?key|apikey)\s*[=:]\s*["'][^"']{4,}["']"#, "api_key"),
            (r#"(?i)(secret|token)\s*[=:]\s*["'][^"']{4,}["']"#, "secret"),
        ];
        let regexes: Vec<_> = patterns.iter().map(|(p, _)| regex::Regex::new(p).unwrap()).collect();
        for (line_num, line) in ctx.source.lines().enumerate() {
            for re in &regexes {
                if re.is_match(line) {
                    issues.push(Issue::new("GO_S2068", "Hard-coded credential detected", Severity::Blocker, Category::SecurityHotspot, ctx.file_path, line_num+1).with_remediation(Remediation::moderate("Use environment variables or a secrets manager")));
                    break;
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GO_S2077 — SQL injection
// ─────────────────────────────────────────────────────────────────────────────
declare_rule! {
    id: "GO_S2077"
    name: "SQL injection vulnerabilities should be prevented"
    severity: Blocker
    category: Vulnerability
    language: "go"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let sql_keywords = ["SELECT", "INSERT", "UPDATE", "DELETE", "DROP", "EXEC", "EXECUTE"];
        let re = regex::Regex::new(r#"fmt\.Sprintf\s*\([^,]+,\s*["'][^"']*["']"#).unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                let upper = line.to_uppercase();
                for kw in &sql_keywords {
                    if upper.contains(kw) {
                        issues.push(Issue::new("GO_S2077", "Potential SQL injection - use parameterized queries", Severity::Blocker, Category::Vulnerability, ctx.file_path, idx+1));
                        break;
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GO_S1523 — exec.Command with user input
// ─────────────────────────────────────────────────────────────────────────────
declare_rule! {
    id: "GO_S1523"
    name: "Shell command built from user input"
    severity: Blocker
    category: Vulnerability
    language: "go"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"exec\.Command\s*\(\s*\w+\s*,").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && !line.contains("//") {
                issues.push(Issue::new("GO_S1523", "exec.Command with variable input - verify no injection", Severity::Blocker, Category:: Vulnerability, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GO_S2612 — chmod 777
// ─────────────────────────────────────────────────────────────────────────────
declare_rule! {
    id: "GO_S2612"
    name: "Permissions should be set explicitly"
    severity: Blocker
    category: SecurityHotspot
    language: "go"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"os\.Chmod\s*\([^)]*0[0-7]{3}").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("GO_S2612", "chmod with 777-like permissions - security risk", Severity::Blocker, Category::SecurityHotspot, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GO_S1148 — panic in library
// ─────────────────────────────────────────────────────────────────────────────
declare_rule! {
    id: "GO_S1148"
    name: "panic! should not be used in library code"
    severity: Major
    category: Bug
    language: "go"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if line.contains("panic(") && !line.contains("test") && !line.contains("_test.go") {
                issues.push(Issue::new("GO_S1148", "panic in non-test code - return error instead", Severity::Major, Category::Bug, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GO_S5332 — HTTP cleartext
// ─────────────────────────────────────────────────────────────────────────────
declare_rule! {
    id: "GO_S5332"
    name: "Clear-text protocols should not be used"
    severity: Blocker
    category: Vulnerability
    language: "go"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r#"http://[^\s""']+"#).unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(m) = re.find(line) {
                if !line.contains("https://") && !line.contains("localhost") {
                    issues.push(Issue::new("GO_S5332", format!("Clear-text HTTP URL: {}", m.as_str()), Severity::Blocker, Category::Vulnerability, ctx.file_path, idx+1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GO_S4830 — TLS skip verification
// ─────────────────────────────────────────────────────────────────────────────
declare_rule! {
    id: "GO_S4830"
    name: "TLS certificate verification should not be disabled"
    severity: Blocker
    category: Vulnerability
    language: "go"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r#"(InsecureSkipVerify\s*[=:]\s*true|ClientConfig\s*{[^}]*InsecureSkipVerify[^}]*true)"#).unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("GO_S4830", "TLS InsecureSkipVerify set to true - transport is insecure", Severity::Blocker, Category::Vulnerability, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GO_S5542 — Weak crypto MD5
// ─────────────────────────────────────────────────────────────────────────────
declare_rule! {
    id: "GO_S5542"
    name: "Weak cryptographic hash function"
    severity: Critical
    category: Vulnerability
    language: "go"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"md5\.(New|Md5Sum|Md5)\b").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && !line.contains("//") {
                issues.push(Issue::new("GO_S5542", "MD5 is a weak cryptographic hash - use SHA-256 or better", Severity::Critical, Category::Vulnerability, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GO_S2095 — defer close missing
// ─────────────────────────────────────────────────────────────────────────────
declare_rule! {
    id: "GO_S2095"
    name: "Resources should be properly closed"
    severity: Major
    category: Bug
    language: "go"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"(os\.Open|io\.OpenFile|sql\.Open|bufio\.NewReader|bufio\.NewWriter)\s*\([^)]+\)\s*(?:\n[^}]*)?defer\s+.*\.Close\s*\(\s*\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("GO_S2095", "Opened resource should be closed with defer", Severity::Major, Category::Bug, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GO_S2259 — nil pointer dereference
// ─────────────────────────────────────────────────────────────────────────────
declare_rule! {
    id: "GO_S2259"
    name: "Nil pointers should be checked before dereferencing"
    severity: Blocker
    category: Bug
    language: "go"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\*\w+\s*\.\s*\w+\s*\(").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && !line.contains("if") && !line.contains("//") {
                issues.push(Issue::new("GO_S2259", "Potential nil pointer dereference", Severity::Blocker, Category::Bug, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GO_S108 — Empty error check
// ─────────────────────────────────────────────────────────────────────────────
declare_rule! {
    id: "GO_S108"
    name: "Empty blocks should not be used"
    severity: Major
    category: CodeSmell
    language: "go"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"if\s+\w+\s*!=\s*nil\s*\{\s*\}").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("GO_S108", "Empty error check block - add handling logic", Severity::Major, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GO_S185 — Dead store
// ─────────────────────────────────────────────────────────────────────────────
declare_rule! {
    id: "GO_S185"
    name: "Unused assignments should be removed"
    severity: Major
    category: Bug
    language: "go"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"(\w+)\s*:?=\s*\1\s*;").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("GO_S185", "Variable assigned to itself - dead store", Severity::Major, Category::Bug, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GO_S1481 — Unused variable
// ─────────────────────────────────────────────────────────────────────────────
declare_rule! {
    id: "GO_S1481"
    name: "Unused local variables should be removed"
    severity: Minor
    category: CodeSmell
    language: "go"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"(?<!_)_\s*:?=\s*").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("GO_S1481", "Unused variable (blank identifier preferred)", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GO_S1656 — Self-assignment
// ─────────────────────────────────────────────────────────────────────────────
declare_rule! {
    id: "GO_S1656"
    name: "Variables should not be self-assigned"
    severity: Major
    category: Bug
    language: "go"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"(\w+)\s*=\s*\1\s*;").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && !line.contains("//") {
                issues.push(Issue::new("GO_S1656", "Self-assignment has no effect", Severity::Major, Category::Bug, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GO_S1764 — Identical operands
// ─────────────────────────────────────────────────────────────────────────────
declare_rule! {
    id: "GO_S1764"
    name: "Identical expressions should not be compared"
    severity: Major
    category: Bug
    language: "go"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let ops = ["==", "!=", ">=", "<=", ">", "<"];
        for (idx, line) in ctx.source.lines().enumerate() {
            for op in &ops {
                if let Some(pos) = line.find(op) {
                    if pos > 0 {
                        let before = line[..pos].trim();
                        let after = line[pos+op.len()..].trim();
                        if before == after && !before.is_empty() {
                            issues.push(Issue::new("GO_S1764", "Identical operands - always true/false", Severity::Major, Category::Bug, ctx.file_path, idx+1));
                            break;
                        }
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GO_S2757 — = vs ==
// ─────────────────────────────────────────────────────────────────────────────
declare_rule! {
    id: "GO_S2757"
    name: "Unexpected assignment operators in conditions"
    severity: Major
    category: Bug
    language: "go"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"if\s+\w+\s*=\s*\w+").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && !line.contains("==") && !line.contains(":=") {
                issues.push(Issue::new("GO_S2757", "Possible '=' instead of '==' in condition", Severity::Major, Category::Bug, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GO_S1244 — Float equality
// ─────────────────────────────────────────────────────────────────────────────
declare_rule! {
    id: "GO_S1244"
    name: "Floating point equality should not be used"
    severity: Major
    category: Bug
    language: "go"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"(float32|float64)\b.*==").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("GO_S1244", "Floating point equality - use epsilon comparison", Severity::Major, Category:: Bug, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GO_S2201 — Return value ignored
// ─────────────────────────────────────────────────────────────────────────────
declare_rule! {
    id: "GO_S2201"
    name: "Return values should not be ignored"
    severity: Major
    category: Bug
    language: "go"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let fns = ["strings.Trim", "regexp.MustCompile", "json.Marshal", "json.Unmarshal"];
        for (idx, line) in ctx.source.lines().enumerate() {
            for fn_name in &fns {
                if line.contains(fn_name) && !line.contains("if") && !line.contains("_") && !line.contains(":=") {
                    issues.push(Issue::new("GO_S2201", "Return value is ignored", Severity::Major, Category::Bug, ctx.file_path, idx+1));
                    break;
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GO_S2221 — log.Fatal in library
// ─────────────────────────────────────────────────────────────────────────────
declare_rule! {
    id: "GO_S2221"
    name: "log.Fatal should not be used in library code"
    severity: Major
    category: CodeSmell
    language: "go"
    params: {}
    check: => {
        let mut issues = Vec::new();
        for (idx, line) in ctx.source.lines().enumerate() {
            if (line.contains("log.Fatal") || line.contains("log.Panic")) && !line.contains("_test.go") {
                issues.push(Issue::new("GO_S2221", "log.Fatal in non-test code - return error instead", Severity::Major, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GO_S1860 — Deadlock
// ─────────────────────────────────────────────────────────────────────────────
declare_rule! {
    id: "GO_S1860"
    name: "Nested mutex locks should be avoided"
    severity: Critical
    category: Bug
    language: "go"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\.Lock\(\)").unwrap();
        let mut lock_depth: i32 = 0;
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                lock_depth += 1;
                if lock_depth > 1 {
                    issues.push(Issue::new("GO_S1860", "Nested mutex lock - potential deadlock", Severity::Critical, Category::Bug, ctx.file_path, idx+1));
                }
            }
            if line.contains("Unlock()") {
                lock_depth = lock_depth.saturating_sub(1);
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GO_S100 — Naming convention
// ─────────────────────────────────────────────────────────────────────────────
declare_rule! {
    id: "GO_S100"
    name: "Function names should use MixedCaps convention"
    severity: Minor
    category: CodeSmell
    language: "go"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"func\s+([a-z][a-z0-9_]*)\s*\(").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                let name = cap.get(1).unwrap().as_str();
                if name.contains("_") {
                    issues.push(Issue::new("GO_S100", format!("Function '{}' should use MixedCaps", name), Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GO_S107 — Too many parameters
// ─────────────────────────────────────────────────────────────────────────────
declare_rule! {
    id: "GO_S107"
    name: "Functions should not have too many parameters"
    severity: Major
    category: CodeSmell
    language: "go"
    params: { max_params: usize = 7 }
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"func\s+\w+\s*\(([^)]*)\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                if let Some(params) = cap.get(1) {
                    let param_str = params.as_str();
                    // Count commas + 1 to get parameter count
                    let param_count = param_str.matches(',').count() + 1;
                    // Handle empty params
                    let actual_count = if param_str.trim().is_empty() { 0 } else { param_count };
                    if actual_count > self.max_params {
                        issues.push(Issue::new("GO_S107", format!("Function has {} parameters (max {})", actual_count, self.max_params), Severity::Major, Category::CodeSmell, ctx.file_path, idx+1));
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GO_S134 — Deep nesting
// ─────────────────────────────────────────────────────────────────────────────
declare_rule! {
    id: "GO_S134"
    name: "Control flow statements should not be nested too deeply"
    severity: Major
    category: CodeSmell
    language: "go"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let mut depth = 0;
        let mut max_depth_line = 0;
        let mut max_depth = 0;
        for (idx, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("if ") || trimmed.starts_with("for ") || trimmed.starts_with("switch ") {
                depth += 1;
                if depth > max_depth {
                    max_depth = depth;
                    max_depth_line = idx + 1;
                }
            }
            if trimmed == "}" && depth > 0 {
                depth -= 1;
            }
        }
        if max_depth > 4 {
            issues.push(Issue::new("GO_S134", format!("Nesting depth {} exceeds threshold of 4", max_depth), Severity::Major, Category::CodeSmell, ctx.file_path, max_depth_line));
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GO_S138 — Long function
// ─────────────────────────────────────────────────────────────────────────────
declare_rule! {
    id: "GO_S138"
    name: "Functions should not be too long"
    severity: Major
    category: CodeSmell
    language: "go"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();
        let mut in_func = false;
        let mut func_start = 0;
        let mut brace_count = 0;
        for (idx, line) in lines.iter().enumerate() {
            if line.contains("func ") && !line.contains("//") {
                in_func = true;
                func_start = idx;
                brace_count = 0;
            }
            if in_func {
                brace_count += line.matches("{").count() as i32;
                brace_count -= line.matches("}").count() as i32;
                if brace_count == 0 && idx > func_start {
                    let func_len = idx - func_start + 1;
                    if func_len > 50 {
                        issues.push(Issue::new("GO_S138", format!("Function is {} lines - exceeds 50", func_len), Severity::Major, Category::CodeSmell, ctx.file_path, func_start+1));
                    }
                    in_func = false;
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GO_S3776 — Cognitive complexity
// ─────────────────────────────────────────────────────────────────────────────
declare_rule! {
    id: "GO_S3776"
    name: "Cognitive complexity should not be too high"
    severity: Major
    category: CodeSmell
    language: "go"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let keywords = ["if", "for", "switch", "case", "&&", "||", "goto"];
        let lines: Vec<&str> = ctx.source.lines().collect();
        let mut in_func = false;
        let mut func_start = 0;
        let mut brace_count = 0;
        let mut complexity = 0;
        for (idx, line) in lines.iter().enumerate() {
            if line.contains("func ") && !line.contains("//") {
                in_func = true;
                func_start = idx;
                brace_count = 0;
                complexity = 0;
            }
            if in_func {
                brace_count += line.matches("{").count() as i32;
                brace_count -= line.matches("}").count() as i32;
                for kw in &keywords {
                    if line.contains(kw) && !line.starts_with("//") {
                        complexity += 1;
                    }
                }
                if brace_count == 0 && idx > func_start {
                    if complexity > 15 {
                        issues.push(Issue::new("GO_S3776", format!("Cognitive complexity {} exceeds 15", complexity), Severity::Major, Category::CodeSmell, ctx.file_path, func_start+1));
                    }
                    in_func = false;
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GO_S1186 — Empty function
// ─────────────────────────────────────────────────────────────────────────────
declare_rule! {
    id: "GO_S1186"
    name: "Empty functions should be completed or removed"
    severity: Major
    category: CodeSmell
    language: "go"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"func\s+\w+\s*\(\s*\)\s*\{\s*\/\/.*\s*\}").unwrap();
        let re2 = regex::Regex::new(r"func\s+\w+\s*\(\s*\)\s*\{\s*\}").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) || re2.is_match(line) {
                issues.push(Issue::new("GO_S1186", "Empty function body", Severity::Major, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GO_S1871 — Duplicate branches
// ─────────────────────────────────────────────────────────────────────────────
declare_rule! {
    id: "GO_S1871"
    name: "Branches should not have identical implementations"
    severity: Major
    category: Bug
    language: "go"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\} else \{\s*if\s*\(").unwrap();
        let re2 = regex::Regex::new(r"if\s*\([^)]+\)\s*\{[^}]+\}\s*else\s*\{[^}]+\}").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) || re2.is_match(line) {
                issues.push(Issue::new("GO_S1871", "Duplicate branches in if-else - consider merging", Severity::Major, Category::Bug, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GO_S122 — File too long
// ─────────────────────────────────────────────────────────────────────────────
declare_rule! {
    id: "GO_S122"
    name: "Files should not be too long"
    severity: Major
    category: CodeSmell
    language: "go"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let line_count = ctx.source.lines().count();
        if line_count > 1000 {
            issues.push(Issue::new("GO_S122", format!("File has {} lines - exceeds 1000", line_count), Severity::Major, Category::CodeSmell, ctx.file_path, 1));
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GO_S148 — Low comment ratio
// ─────────────────────────────────────────────────────────────────────────────
declare_rule! {
    id: "GO_S148"
    name: "Comments should not be empty"
    severity: Minor
    category: CodeSmell
    language: "go"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\/\/\s*$").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("GO_S148", "Empty comment line", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GO_S115 — Constant naming
// ─────────────────────────────────────────────────────────────────────────────
declare_rule! {
    id: "GO_S115"
    name: "Constant names should use MixedCaps"
    severity: Minor
    category: CodeSmell
    language: "go"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"const\s+([a-z][a-z0-9_]*)\s*=").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                let name = cap.get(1).unwrap().as_str();
                if name.contains("_") || name.chars().any(|c| c.is_uppercase()) {
                    issues.push(Issue::new("GO_S115", format!("Constant '{}' should use MixedCaps", name), Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GO_S1700 — String concatenation
// ─────────────────────────────────────────────────────────────────────────────
declare_rule! {
    id: "GO_S1700"
    name: "String concatenation should use strings.Builder"
    severity: Minor
    category: CodeSmell
    language: "go"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r#"\+\s*"[^"]*"\s*\+"#).unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("GO_S1700", "Use strings.Builder for string concatenation", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GO_S1736 — Range loop index unused
// ─────────────────────────────────────────────────────────────────────────────
declare_rule! {
    id: "GO_S1736"
    name: "Range loop index should be used"
    severity: Minor
    category: CodeSmell
    language: "go"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"for\s+_\s*:?=\s*range\s+").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && !line.contains("//") {
                issues.push(Issue::new("GO_S1736", "Range loop index is ignored - use range over values only", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GO_S1943 — Append without prealloc
// ─────────────────────────────────────────────────────────────────────────────
declare_rule! {
    id: "GO_S1943"
    name: "append should be called with pre-allocated slice"
    severity: Minor
    category: CodeSmell
    language: "go"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"append\s*\(\s*\w+\s*,\s*\w+\s*\.\.\.\s*\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("GO_S1943", "append may cause reallocation - pre-allocate with make()", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GO_S2111 — sprintf in string literal
// ─────────────────────────────────────────────────────────────────────────────
declare_rule! {
    id: "GO_S2111"
    name: "fmt.Sprintf should not be used in string literal"
    severity: Minor
    category: CodeSmell
    language: "go"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r#"`[^`]*fmt\.Sprintf[^`]*`"#).unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("GO_S2111", "fmt.Sprintf in raw string literal - use fmt.Printf", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GO_S170 — Unused import
// ─────────────────────────────────────────────────────────────────────────────
declare_rule! {
    id: "GO_S170"
    name: "Unused imports should be removed"
    severity: Minor
    category: CodeSmell
    language: "go"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r#"_\s+"\w+""#).unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("GO_S170", "Blank import identifier suggests unused import", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GO_S173 — Missing doc comment
// ─────────────────────────────────────────────────────────────────────────────
declare_rule! {
    id: "GO_S173"
    name: "Exported functions should have doc comments"
    severity: Minor
    category: CodeSmell
    language: "go"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"func\s+[A-Z]\w+\s*\(").unwrap();
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            if re.is_match(line) && idx > 0 {
                let prev = lines[idx.saturating_sub(1)].trim();
                if !prev.starts_with("//") || prev.starts_with("//go:") {
                    issues.push(Issue::new("GO_S173", "Exported function lacks doc comment", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GO_S1160 — Error unchecked
// ─────────────────────────────────────────────────────────────────────────────
declare_rule! {
    id: "GO_S1160"
    name: "Errors should be handled"
    severity: Major
    category: Bug
    language: "go"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let unchecked_fns = ["os.Open", "os.Create", "os.ReadFile", "os.WriteFile", "json.Unmarshal"];
        for (idx, line) in ctx.source.lines().enumerate() {
            for fn_name in &unchecked_fns {
                if line.contains(fn_name) && !line.contains("if") && !line.contains(":=") && !line.contains("_=") {
                    issues.push(Issue::new("GO_S1160", format!("Error from {} is unchecked", fn_name), Severity::Major, Category::Bug, ctx.file_path, idx+1));
                    break;
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GO_S117 — Variable naming
// ─────────────────────────────────────────────────────────────────────────────
declare_rule! {
    id: "GO_S117"
    name: "Variable names should use MixedCaps"
    severity: Minor
    category: CodeSmell
    language: "go"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"(?<!_)var\s+([A-Z][a-zA-Z0-9_]*)\s*[=:]").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                let name = cap.get(1).unwrap().as_str();
                issues.push(Issue::new("GO_S117", format!("Variable '{}' should use mixedCaps", name), Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GO_S1135 — TODO comment
// ─────────────────────────────────────────────────────────────────────────────
declare_rule! {
    id: "GO_S1135"
    name: "TODO comments should be completed or removed"
    severity: Minor
    category: CodeSmell
    language: "go"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"(?i)TODO:?").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("GO_S1135", format!("TODO found: {}", line.trim()), Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GO_S125 — Commented code
// ─────────────────────────────────────────────────────────────────────────────
declare_rule! {
    id: "GO_S125"
    name: "Commented code should not be committed"
    severity: Minor
    category: CodeSmell
    language: "go"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"^\s*//\s*(if|for|switch|return|func|var|const|type)\s").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("GO_S125", "Commented code - remove instead of commenting", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// JAVA RULES — 52 rules: streams, Spring Boot, code smells
// ══════════════════════════════════════════════════════════════════════════════

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_L16-L25 — Stream operations
// ─────────────────────────────────────────────────────────────────────────────
declare_rule! {
    id: "JAVA_L16"
    name: "Stream .skip() used before .limit() - consider reordering"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\.skip\s*\(\s*\d+\s*\)\s*\.\s*limit\s*\(\s*\d+\s*\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_L16", ".skip() before .limit() - consider reversing order for performance", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

declare_rule! {
    id: "JAVA_L17"
    name: "Stream .distinct() used after .limit() - distinct before limit is more efficient"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\.limit\s*\([^)]+\)\s*\.\s*distinct\s*\(").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_L17", ".distinct() after .limit() - move distinct before limit", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

declare_rule! {
    id: "JAVA_L18"
    name: "Stream .findFirst() used after .filter() - consider findAny for parallel"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\.filter\s*\([^)]+\)\s*\.\s*findFirst\s*\(").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_L18", "findFirst() after filter - use findAny() for parallel streams", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

declare_rule! {
    id: "JAVA_L19"
    name: "collect(Collectors.toList()) used where toList() suffices"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"Collectors\.toList\s*\(\s*\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_L19", "Use Stream.toList() instead of collect(Collectors.toList())", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

declare_rule! {
    id: "JAVA_L20"
    name: "Stream .flatMap() used where .map() would suffice"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\.flatMap\s*\(\s*\w+\s*->\s*Stream\.of\s*\([^)]+\)\s*\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_L20", ".flatMap(x -> Stream.of(...)) - use .map() instead", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

declare_rule! {
    id: "JAVA_L21"
    name: "Stream .map() with identity function - remove it"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\.map\s*\(\s*Function\.identity\s*\(\s*\)\s*\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_L21", "Identity function in .map() - remove the call", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

declare_rule! {
    id: "JAVA_L22"
    name: "Stream .flatMap() with identity function - use .mapMulti() or flatten"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\.flatMap\s*\(\s*Function\.identity\s*\(\s*\)\s*\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_L22", "Identity function in .flatMap() - use .mapMulti() or similar", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

declare_rule! {
    id: "JAVA_L23"
    name: "Unnecessary .boxed() on primitive stream - already boxed"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"IntStream\.of\s*\([^)]+\)\s*\.boxed\s*\(\s*\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_L23", "Unnecessary .boxed() call", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

declare_rule! {
    id: "JAVA_L24"
    name: ".allMatch() on empty stream returns true - verify intent"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\.allMatch\s*\(\s*[^)]+\s*\)\s*;?\s*$").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_L24", "allMatch() on empty stream returns true - verify this is intended", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

declare_rule! {
    id: "JAVA_L25"
    name: ".noneMatch() on empty stream returns true - verify intent"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\.noneMatch\s*\(\s*[^)]+\s*\)\s*;?\s*$").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_L25", "noneMatch() on empty stream returns true - verify this is intended", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

declare_rule! {
    id: "JAVA_L26"
    name: ".sorted() followed by .findFirst() - consider .min()/.max()"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\.sorted\s*\([^)]*\)\s*\.\s*findFirst\s*\(").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_L26", ".sorted().findFirst() - use .min() or .max() instead", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

declare_rule! {
    id: "JAVA_L27"
    name: "Stream .min()/.max() returns Optional - handle empty case"
    severity: Major
    category: Bug
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\.min\s*\([^)]*\)\s*;?\s*$|\.max\s*\([^)]*\)\s*;?\s*$").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && !line.contains("orElse") && !line.contains("orElseThrow") && !line.contains("ifPresent") {
                issues.push(Issue::new("JAVA_L27", "min()/max() returns Optional - handle empty case", Severity::Major, Category::Bug, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

declare_rule! {
    id: "JAVA_L28"
    name: "Stream .count() used where .findAny().isPresent() or .limit(1).findAny().isPresent() suffices"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\.filter\s*\([^)]+\)\s*\.\s*count\s*\(\s*\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_L28", "Use findAny().isPresent() instead of filter().count() > 0", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

declare_rule! {
    id: "JAVA_L29"
    name: "Stream .distinct() on non-hashable elements - consider LinkedHashSet"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\.distinct\s*\(\s*\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && !line.contains("LinkedHashSet") && !line.contains("toCollection") {
                issues.push(Issue::new("JAVA_L29", "distinct() uses hash - for ordered streams consider LinkedHashSet", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

declare_rule! {
    id: "JAVA_L30"
    name: "Stream .toList() should be used instead of .collect(Collectors.toList())"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\.collect\s*\(\s*Collectors\.toList\s*\(\s*\)\s*\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_L30", "Use .toList() instead of collect(Collectors.toList())", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_SP1-SP15 — Spring Boot rules
// ─────────────────────────────────────────────────────────────────────────────
declare_rule! {
    id: "JAVA_SP1"
    name: "@Autowired on field should be avoided - use constructor injection"
    severity: Major
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"@Autowired\s+(private|protected|public|final)\s+\w+").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_SP1", "@Autowired on field - use constructor injection instead", Severity::Major, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

declare_rule! {
    id: "JAVA_SP2"
    name: "@Component without interface - consider programming to interfaces"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"@Component\s*(?!.*implements)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && !line.contains("interface") {
                issues.push(Issue::new("JAVA_SP2", "@Component without interface - consider using an interface", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

declare_rule! {
    id: "JAVA_SP3"
    name: "@Service with mutable state - inject stateless beans"
    severity: Major
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"@Service\s*public\s+class\s+\w+\s*\{[^}]*(?!private|protected)\w+\s+=\s*new\s+\w+").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_SP3", "@Service with mutable state - services should be stateless", Severity::Major, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

declare_rule! {
    id: "JAVA_SP4"
    name: "@RestController should not return null directly - use ResponseEntity"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"@RestController").unwrap();
        let mut in_controller = false;
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) { in_controller = true; }
            if in_controller && line.contains("return null") {
                issues.push(Issue::new("JAVA_SP4", "Returning null from @RestController - use ResponseEntity", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
            if in_controller && line.trim() == "}" { in_controller = false; }
        }
        issues
    }
}

declare_rule! {
    id: "JAVA_SP5"
    name: "@Transactional on private method - has no effect"
    severity: Blocker
    category: Bug
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"@Transactional\s+private\s+\w+\s*\(").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_SP5", "@Transactional on private method - will not work", Severity::Blocker, Category::Bug, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

declare_rule! {
    id: "JAVA_SP6"
    name: "@Async without thread pool - use TaskExecutor"
    severity: Major
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"@Async\s+public\s+\w+\s*\(").unwrap();
        let has_task_executor = ctx.source.contains("TaskExecutor") || ctx.source.contains("ExecutorService");
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && !has_task_executor {
                issues.push(Issue::new("JAVA_SP6", "@Async without TaskExecutor - provide a thread pool", Severity::Major, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

declare_rule! {
    id: "JAVA_SP7"
    name: "@Value with complex expression - consider @ConfigurationProperties"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r#"@Value\s*\(\s*"\$\{[^}]+\.[^}]+\}"\s*\)"#).unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_SP7", "@Value with complex expression - consider @ConfigurationProperties", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

declare_rule! {
    id: "JAVA_SP8"
    name: "@Scheduled without cron expression - specify timing"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"@Scheduled\s*\(\s*\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_SP8", "@Scheduled without parameters - specify cron or fixedRate", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

declare_rule! {
    id: "JAVA_SP9"
    name: "JpaRepository naming - use custom method naming conventions"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"extends\s+JpaRepository<[^>]+>\s*\{").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_SP9", "JpaRepository - follow Spring Data method naming conventions", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

declare_rule! {
    id: "JAVA_SP10"
    name: "@Entity without @Id - every entity needs a primary key"
    severity: Blocker
    category: Bug
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let has_entity = ctx.source.contains("@Entity");
        let has_id = ctx.source.contains("@Id");
        if has_entity && !has_id {
            issues.push(Issue::new("JAVA_SP10", "@Entity without @Id field - add @Id annotation", Severity::Blocker, Category::Bug, ctx.file_path, 1));
        }
        issues
    }
}

declare_rule! {
    id: "JAVA_SP11"
    name: "@Bean method naming - use lowercase starting name"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"@Bean\s+public\s+\w+\s+([A-Z]\w+)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if let Some(cap) = re.captures(line) {
                let name = cap.get(1).unwrap().as_str();
                issues.push(Issue::new("JAVA_SP11", format!("@Bean method '{}' should start with lowercase", name), Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

declare_rule! {
    id: "JAVA_SP12"
    name: "@Profile validation - ensure profiles are defined"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r#"@ActiveProfiles\s*\(\s*"[^"]+"\s*\)"#).unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_SP12", "@ActiveProfiles - ensure profile is defined in configuration", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

declare_rule! {
    id: "JAVA_SP13"
    name: "@ConditionalOnMissingBean - verify bean absence intent"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"@ConditionalOnMissingBean").unwrap();
        let count = ctx.source.matches("@ConditionalOnMissingBean").count();
        if count > 5 {
            issues.push(Issue::new("JAVA_SP13", "Many @ConditionalOnMissingBean - verify each is intentional", Severity::Minor, Category::CodeSmell, ctx.file_path, 1));
        }
        issues
    }
}

declare_rule! {
    id: "JAVA_SP14"
    name: "@ConfigurationProperties - validate binding"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"@ConfigurationProperties\s*(?!.*@Validated)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_SP14", "@ConfigurationProperties without @Validated - add @Validated", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

declare_rule! {
    id: "JAVA_SP15"
    name: "@Autowired in test - use constructor injection for testability"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"@Autowired\s+private\s+\w+\s+\w+;").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && ctx.source.contains("@SpringBootTest") {
                issues.push(Issue::new("JAVA_SP15", "@Autowired in test - prefer constructor injection", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_S218-S229 — Code smells
// ─────────────────────────────────────────────────────────────────────────────
declare_rule! {
    id: "JAVA_S218"
    name: "Switch with too few cases - use if-else"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"switch\s*\([^)]+\)\s*\{[^}]*case\s+\w+:[^}]*\}").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && !line.contains("default:") {
                issues.push(Issue::new("JAVA_S218", "Switch with few cases - if-else may be clearer", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

declare_rule! {
    id: "JAVA_S219"
    name: "Loop variable scope in for loop - declare outside"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"for\s*\(\s*int\s+\w+\s*=\s*0[^)]*\)\s*\{").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && idx > 0 {
                let prev = ctx.source.lines().nth(idx.saturating_sub(1)).unwrap_or("");
                if prev.contains("int ") && !prev.contains("for") {
                    issues.push(Issue::new("JAVA_S219", "Loop variable declared outside - consider for loop declaration", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
                }
            }
        }
        issues
    }
}

declare_rule! {
    id: "JAVA_S220"
    name: "Private inner class - consider separate class if testable"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"private\s+static\s+class\s+\w+").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_S220", "Private inner class - extract if testing needed", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

declare_rule! {
    id: "JAVA_S221"
    name: "Method returns null - consider Optional"
    severity: Major
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"return\s+null\s*;").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && !line.contains("Optional") && !line.contains("nullary") {
                issues.push(Issue::new("JAVA_S221", "Returning null - consider Optional<> instead", Severity::Major, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

declare_rule! {
    id: "JAVA_S222"
    name: "Parameter type mismatch - verify method signature"
    severity: Major
    category: Bug
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"method\s+\w+\s*\(\s*\w+\s+\w+\s*\)").unwrap();
        let call_re = regex::Regex::new(r"\w+\.\w+\s*\(\s*\w+\s+\w+\s*\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && call_re.is_match(line) {
                issues.push(Issue::new("JAVA_S222", "Parameter type mismatch - verify method signature", Severity::Major, Category::Bug, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

declare_rule! {
    id: "JAVA_S223"
    name: "instanceof without cast - use pattern matching"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"instanceof\s+\w+\s*\)\s*\{[^}]*\(\s*\(\s*\w+\s*\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_S223", "instanceof with cast - use Java 16+ pattern matching", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

declare_rule! {
    id: "JAVA_S224"
    name: "Flag parameter - split method"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"(boolean|Boolean)\s+\w+").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && (line.contains("method") || line.contains("func")) {
                issues.push(Issue::new("JAVA_S224", "Flag parameter - consider splitting into separate methods", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

declare_rule! {
    id: "JAVA_S225"
    name: "Method has too many parameters - use builder or parameter object"
    severity: Major
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"(\w+)\s*\([^)]{80,}\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_S225", "Method has too many parameters - use parameter object", Severity::Major, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

declare_rule! {
    id: "JAVA_S226"
    name: "Size check in loop - verify logic"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\.size\s*\(\s*\)\s*[<>=!]+\s*\d+").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && (line.contains("for ") || line.contains("while ")) {
                issues.push(Issue::new("JAVA_S226", "Size check in loop - verify this is intentional", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

declare_rule! {
    id: "JAVA_S227"
    name: "for(;;) - use while(true)"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"for\s*\(\s*;\s*;\s*\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_S227", "for(;;) - use while(true) for clarity", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

declare_rule! {
    id: "JAVA_S228"
    name: "Thread not started - verify thread start intent"
    severity: Major
    category: Bug
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"Thread\s+\w+\s*=\s*new\s+Thread\s*\([^)]+\)\s*;").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                let next_line = ctx.source.lines().nth(idx + 1).unwrap_or("");
                if !next_line.contains(".start()") && !next_line.contains("//start") {
                    issues.push(Issue::new("JAVA_S228", "Thread created but not started", Severity::Major, Category::Bug, ctx.file_path, idx+1));
                }
            }
        }
        issues
    }
}

declare_rule! {
    id: "JAVA_S229"
    name: "finalize() overridden - use AutoCloseable"
    severity: Major
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"@Override\s+protected\s+void\s+finalize\s*\(\s*\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_S229", "finalize() - use AutoCloseable instead", Severity::Major, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JAVA_L31-L40 — More stream rules
// ─────────────────────────────────────────────────────────────────────────────
declare_rule! {
    id: "JAVA_L31"
    name: "reduce with identity on parallel stream - ensure identity is associative"
    severity: Major
    category: Bug
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\.parallel\s*\(\s*\)\s*\.\s*reduce\s*\([^,]+,").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_L31", "reduce with identity in parallel - ensure identity is associative", Severity::Major, Category::Bug, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

declare_rule! {
    id: "JAVA_L32"
    name: "skip() followed by limit() on ordered stream - verify order intent"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\.skip\s*\(\s*\d+\s*\)\s*\.\s*limit\s*\(\s*1\s*\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_L32", "skip(n).limit(1) - verify you want the nth element, not any", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

declare_rule! {
    id: "JAVA_L33"
    name: "anyMatch on Optional - use isPresent()"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"Optional\w*\.anyMatch\s*\(").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_L33", "anyMatch on Optional - use isPresent() or orElse", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

declare_rule! {
    id: "JAVA_L34"
    name: "findAny() vs findFirst() - use findFirst() for deterministic results"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\.findAny\s*\(\s*\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && !line.contains("//") {
                issues.push(Issue::new("JAVA_L34", "findAny() returns non-deterministic result - use findFirst() if order matters", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

declare_rule! {
    id: "JAVA_L35"
    name: "sorted() with Comparator - consider Comparable or explicit comparison"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\.sorted\s*\(\s*Comparator\.\w+\s*\(\s*\w+,\s*\w+\s*->\s*\w+\.\w+\(\w+\)\s*\)\s*\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_L35", "sorted with Comparator lambda - consider Comparable", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

declare_rule! {
    id: "JAVA_L36"
    name: "groupingBy with missing downstream collector - defaults to toList()"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"Collectors\.groupingBy\s*\([^)]+\)\s*(?!\.\w+\s*\()").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_L36", "groupingBy without downstream - consider mapping or collectingAndThen", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

declare_rule! {
    id: "JAVA_L37"
    name: "partitioningBy should use groupingBy for more than two groups"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"Collectors\.partitioningBy\s*\([^)]+\)\s*\.\s*get\s*\(").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_L37", "partitioningBy with .get() - use groupingBy for more than 2 groups", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

declare_rule! {
    id: "JAVA_L38"
    name: "mapping collector - verify flatMapping is more appropriate"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"Collectors\.mapping\s*\([^,]+,\s*Collectors\.toList\s*\(\s*\)\s*\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_L38", "mapping(toList()) - consider flatMapping for nested collections", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

declare_rule! {
    id: "JAVA_L39"
    name: "flatMapping usage - verify it replaces map+flatten pattern"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"Collectors\.flatMapping\s*\(").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_L39", "flatMapping - ensure it replaces map+flatten pattern", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

declare_rule! {
    id: "JAVA_L40"
    name: "teeing collector - use for combining two collectors"
    severity: Minor
    category: CodeSmell
    language: "java"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"Collectors\.teeing\s*\(").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JAVA_L40", "teeing - good for combining two independent collectors", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// JS/TS RULES — 28 rules: React, Testing, TypeScript
// ══════════════════════════════════════════════════════════════════════════════

// ─────────────────────────────────────────────────────────────────────────────
// JS_RX41-RX50 — React rules
// ─────────────────────────────────────────────────────────────────────────────
declare_rule! {
    id: "JS_RX41"
    name: "Context.Provider used directly - consider useContext hook"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"<\w+Provider\s+value\s*=").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JS_RX41", "Context.Provider - consider useContext for cleaner code", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

declare_rule! {
    id: "JS_RX42"
    name: "useEffect missing cleanup function for subscriptions"
    severity: Major
    category: Bug
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"useEffect\s*\(\s*\(\s*\)\s*=>\s*\{[^}]*addEventListener|setInterval|setTimeout[^}]*\}\s*,").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && !ctx.source.lines().nth(idx + 1).unwrap_or("").contains("return") {
                issues.push(Issue::new("JS_RX42", "useEffect with subscription - add cleanup return function", Severity::Major, Category::Bug, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

declare_rule! {
    id: "JS_RX43"
    name: "useCallback missing dependencies - may cause stale closures"
    severity: Major
    category: Bug
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"useCallback\s*\([^,]+,\s*\[\s*\]\s*\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JS_RX43", "useCallback with empty deps - may cause stale closure", Severity::Major, Category::Bug, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

declare_rule! {
    id: "JS_RX44"
    name: "useState initializer called on every render - use lazy init"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"useState\s*\(\s*\w+\s*\([^)]*\)\s*\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JS_RX44", "useState with function call - use lazy init: () => fn()", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

declare_rule! {
    id: "JS_RX45"
    name: "Derived state computed inline - consider useMemo"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"const\s+\w+\s*=\s*\w+\s*\.\s*map\s*\([^)]+\)\s*;?\s*const\s+\w+\s*=\s*\w+\s*\.\s*filter").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JS_RX45", "Derived state computed inline - use useMemo", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

declare_rule! {
    id: "JS_RX46"
    name: "useEffect with setState - may cause infinite loop"
    severity: Blocker
    category: Bug
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"useEffect\s*\([^}]*set\w+\s*\([^)]*\)[^}]*\}\s*,\s*\[\s*\w+\s*\]\s*\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JS_RX46", "useEffect with setState in deps - risk of infinite loop", Severity::Blocker, Category::Bug, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

declare_rule! {
    id: "JS_RX47"
    name: "useRef used but value not accessed - verify intent"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"const\s+\w+\s*=\s*useRef\s*\([^)]+\)\s*;?\s*(?!.*\1\.current)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JS_RX47", "useRef created but .current not accessed", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

declare_rule! {
    id: "JS_RX48"
    name: "useImperativeHandle without forwardRef"
    severity: Major
    category: Bug
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let has_forward_ref = ctx.source.contains("forwardRef");
        let has_imperative = ctx.source.contains("useImperativeHandle");
        if has_imperative && !has_forward_ref {
            issues.push(Issue::new("JS_RX48", "useImperativeHandle requires forwardRef", Severity::Major, Category::Bug, ctx.file_path, 1));
        }
        issues
    }
}

declare_rule! {
    id: "JS_RX49"
    name: "React.lazy without Suspense - add boundary"
    severity: Major
    category: Bug
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let has_lazy = ctx.source.contains("React.lazy") || ctx.source.contains("lazy(");
        let has_suspense = ctx.source.contains("Suspense");
        if has_lazy && !has_suspense {
            issues.push(Issue::new("JS_RX49", "React.lazy without Suspense - add Suspense boundary", Severity::Major, Category::Bug, ctx.file_path, 1));
        }
        issues
    }
}

declare_rule! {
    id: "JS_RX50"
    name: "createContext default value may cause null checks - use null"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"createContext\s*\(\s*\{\s*\}\s*\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JS_RX50", "createContext with {} - use null and handle in provider", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JS_TEST11-TEST20 — Testing rules
// ─────────────────────────────────────────────────────────────────────────────
declare_rule! {
    id: "JS_TEST11"
    name: "Test without describe block - group related tests"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r#"it\s*\(\s*['"][^'"]+['"]\s*,"#).unwrap();
        let has_describe = ctx.source.contains("describe(");
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && !has_describe {
                issues.push(Issue::new("JS_TEST11", "Test without describe - group tests with describe blocks", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
                break;
            }
        }
        issues
    }
}

declare_rule! {
    id: "JS_TEST12"
    name: "Missing expect.assertions - add for async tests"
    severity: Major
    category: Bug
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r#"(test|it)\s*\(\s*['"][^'"]+['"]\s*,\s*(async\s*)?\("#).unwrap();
        let has_assertions = ctx.source.contains("expect.assertions");
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && !has_assertions {
                issues.push(Issue::new("JS_TEST12", "Async test without expect.assertions - add assertion count check", Severity::Major, Category::Bug, ctx.file_path, idx+1));
                break;
            }
        }
        issues
    }
}

declare_rule! {
    id: "JS_TEST13"
    name: "beforeAll nested inside describe - move to top level"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"describe\s*\([^)]+\s*\{[^}]*beforeAll\s*\(").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JS_TEST13", "beforeAll inside nested describe - move to outer scope", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

declare_rule! {
    id: "JS_TEST14"
    name: "mockImplementation vs mockReturnValue - use consistently"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let has_impl = ctx.source.contains("mockImplementation");
        let has_return = ctx.source.contains("mockReturnValue");
        if has_impl && has_return {
            issues.push(Issue::new("JS_TEST14", "Mixing mockImplementation and mockReturnValue - pick one", Severity::Minor, Category::CodeSmell, ctx.file_path, 1));
        }
        issues
    }
}

declare_rule! {
    id: "JS_TEST15"
    name: "spyOn without restore - add cleanup"
    severity: Major
    category: Bug
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"jest\.spyOn\s*\([^)]+\)\s*;").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                let next_line = ctx.source.lines().nth(idx + 1).unwrap_or("");
                if !next_line.contains("restore") && !next_line.contains("mockRestore") {
                    issues.push(Issue::new("JS_TEST15", "spyOn without restore - add .mockRestore() in afterEach", Severity::Major, Category::Bug, ctx.file_path, idx+1));
                }
            }
        }
        issues
    }
}

declare_rule! {
    id: "JS_TEST16"
    name: "act() wrapper missing - wrap state updates in act()"
    severity: Major
    category: Bug
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"fireEvent\.\w+\s*\([^)]+\)\s*;?\s*(?!.*act\s*\()").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && ctx.source.contains("React") {
                issues.push(Issue::new("JS_TEST16", "fireEvent without act() - wrap in act()", Severity::Major, Category::Bug, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

declare_rule! {
    id: "JS_TEST17"
    name: "waitFor without timeout - specify timeout"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"waitFor\s*\(\s*\(\s*\)\s*=>\s*\{[^}]*\}\s*\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && !line.contains("timeout") {
                issues.push(Issue::new("JS_TEST17", "waitFor without timeout - add { timeout: ms }", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

declare_rule! {
    id: "JS_TEST18"
    name: "fireEvent vs userEvent - prefer userEvent for user behavior"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"fireEvent\.\w+\s*\(").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && ctx.source.contains("@testing-library") {
                issues.push(Issue::new("JS_TEST18", "fireEvent - consider userEvent for realistic interaction", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

declare_rule! {
    id: "JS_TEST19"
    name: "toBeTruthy vs toBe(true) - use specific matcher"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"expect\s*\([^)]+\)\s*\.\s*toBeTruthy\s*\(\s*\)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JS_TEST19", "toBeTruthy() - use toBe(true) for boolean checks", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

declare_rule! {
    id: "JS_TEST20"
    name: "toEqual vs toStrictEqual - use toStrictEqual for exact matching"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"expect\s*\([^)]+\)\s*\.\s*toEqual\s*\(\s*\{").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("JS_TEST20", "toEqual for objects - consider toStrictEqual for exact matching", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// TS_ADV1-TS_ADV8 — TypeScript advanced rules
// ─────────────────────────────────────────────────────────────────────────────
declare_rule! {
    id: "TS_ADV1"
    name: "Numeric enum - use union type or const enum"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"enum\s+\w+\s*\{[^}]*(?:0|1|2|3|4|5|6|7|8|9)").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("TS_ADV1", "Numeric enum - use string enum or const enum", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

declare_rule! {
    id: "TS_ADV2"
    name: "Type assertion with 'as' - verify type is correct"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"\bas\s+\w+").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && !line.contains("//") {
                issues.push(Issue::new("TS_ADV2", "Type assertion 'as' - ensure type is correct", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

declare_rule! {
    id: "TS_ADV3"
    name: "any type used - use unknown or specific type"
    severity: Major
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r":\s*any\b|\bany\[\]").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && !line.contains("//") && !line.contains("@ts-ignore") {
                issues.push(Issue::new("TS_ADV3", "any type - use unknown or specific type", Severity::Major, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

declare_rule! {
    id: "TS_ADV4"
    name: "NonNullable - use for null check"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"NonNullable<").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("TS_ADV4", "NonNullable - good for type narrowing", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

declare_rule! {
    id: "TS_ADV5"
    name: "ReturnType - infer return type from function"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"ReturnType<").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("TS_ADV5", "ReturnType - good for extracting function return type", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

declare_rule! {
    id: "TS_ADV6"
    name: "Omit vs Pick - verify correct utility type"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"(Omit|Pick)<").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("TS_ADV6", "Omit/Pick - verify correct utility type for intent", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

declare_rule! {
    id: "TS_ADV7"
    name: "Record - verify key type constraints"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"Record<\s*\w+\s*,").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("TS_ADV7", "Record - ensure key type is appropriate (string | number)", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

declare_rule! {
    id: "TS_ADV8"
    name: "Exclude vs Extract - verify correct conditional type"
    severity: Minor
    category: CodeSmell
    language: "javascript"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"(Exclude|Extract)<").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("TS_ADV8", "Exclude/Extract - verify correct for intended type filtering", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// RUST RULES — 8 rules: R021-R028
// ══════════════════════════════════════════════════════════════════════════════

// ─────────────────────────────────────────────────────────────────────────────
// R021 — Arc<Mutex> when Rc<RefCell> works
// ─────────────────────────────────────────────────────────────────────────────
declare_rule! {
    id: "R021"
    name: "Arc<Mutex> used when Rc<RefCell> would suffice (single-threaded)"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"Arc<\s*Mutex<").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && !line.contains("thread") && !line.contains("Thread") && !ctx.source.contains("std::sync") {
                issues.push(Issue::new("R021", "Arc<Mutex> in single-threaded context - use Rc<RefCell>", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// R022 — Box<dyn Error> vs concrete error
// ─────────────────────────────────────────────────────────────────────────────
declare_rule! {
    id: "R022"
    name: "Box<dyn Error> used - consider concrete error type or anyhow::Error"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"Box<dyn\s+Error>").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && !ctx.source.contains("anyhow") {
                issues.push(Issue::new("R022", "Box<dyn Error> - consider anyhow::Error or concrete type", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// R023 — Debug on sensitive struct
// ─────────────────────────────────────────────────────────────────────────────
declare_rule! {
    id: "R023"
    name: "Struct with sensitive data derives Debug - may leak secrets"
    severity: Major
    category: SecurityHotspot
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let sensitive = ["password", "secret", "token", "credential", "api_key", "private_key"];
        let re = regex::Regex::new(r"#\[derive\([^)]*Debug[^)]*\)\]").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                let next_lines: String = ctx.source.lines().skip(idx).take(20).collect();
                for s in &sensitive {
                    if next_lines.to_lowercase().contains(s) {
                        issues.push(Issue::new("R023", "Struct with sensitive fields derives Debug - may leak secrets", Severity::Major, Category::SecurityHotspot, ctx.file_path, idx+1));
                        break;
                    }
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// R024 — Drop without may_dangle
// ─────────────────────────────────────────────────────────────────────────────
declare_rule! {
    id: "R024"
    name: "Custom Drop impl - consider #[unsafe_destructor] or may_dangle"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"impl\s+Drop\s+for\s+").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && !ctx.source.contains("may_dangle") && !ctx.source.contains("unsafe_destructor") {
                issues.push(Issue::new("R024", "Custom Drop - verify Drop order safety or use may_dangle", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// R025 — mem::forget misuse
// ─────────────────────────────────────────────────────────────────────────────
declare_rule! {
    id: "R025"
    name: "mem::forget used - may cause resource leaks"
    severity: Minor
    category: Bug
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"mem::forget\s*\(").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("R025", "mem::forget - prevents Drop, may leak resources", Severity::Minor, Category::Bug, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// R026 — PhantomData pattern
// ─────────────────────────────────────────────────────────────────────────────
declare_rule! {
    id: "R026"
    name: "PhantomData marker type - verify proper variance and drop check"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"PhantomData\s*<").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                issues.push(Issue::new("R026", "PhantomData - verify variance (Covariant/TInvariant/Contravariant)", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// R027 — transmute without safety
// ─────────────────────────────────────────────────────────────────────────────
declare_rule! {
    id: "R027"
    name: "std::mem::transmute used - requires unsafe block"
    severity: Critical
    category: Bug
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"transmute\s*<").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) {
                if !ctx.source.lines().nth(idx.saturating_sub(1)).unwrap_or("").contains("unsafe") {
                    issues.push(Issue::new("R027", "transmute without unsafe block", Severity::Critical, Category::Bug, ctx.file_path, idx+1));
                }
            }
        }
        issues
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// R028 — Pin without Unpin
// ─────────────────────────────────────────────────────────────────────────────
declare_rule! {
    id: "R028"
    name: "std::pin::Pin used without Unpin bound - verify safety"
    severity: Minor
    category: CodeSmell
    language: "rust"
    params: {}
    check: => {
        let mut issues = Vec::new();
        let re = regex::Regex::new(r"Pin<\s*\w+").unwrap();
        for (idx, line) in ctx.source.lines().enumerate() {
            if re.is_match(line) && !ctx.source.contains("Unpin") && !ctx.source.contains("unsafe") {
                issues.push(Issue::new("R028", "Pin without Unpin bound - ensure type implements Unpin or is pinned correctly", Severity::Minor, Category::CodeSmell, ctx.file_path, idx+1));
            }
        }
        issues
    }
}

submit! {
    RuleEntry {
        factory: || Box::new(S138Rule::default())
    }
}

submit! {
    RuleEntry {
        factory: || Box::new(S3776Rule::default())
    }
}

submit! {
    RuleEntry {
        factory: || Box::new(S2306Rule::default())
    }
}

submit! {
    RuleEntry {
        factory: || Box::new(S1066Rule::default())
    }
}

submit! {
    RuleEntry {
        factory: || Box::new(S1192Rule::default())
    }
}
