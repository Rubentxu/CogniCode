//! Native quality metrics over tree-sitter syntax trees.
//!
//! This adapter walks the tree of a Rust, Java, Kotlin or Go file once and
//! produces the [`FileMetrics`] evidence defined by
//! [`crate::domain::metrics`], which owns the meaning of every number. Any
//! other language is answered with [`MetricsOutcome::Unsupported`]: no function
//! list and no zero.
//!
//! One walk serves all the metrics. A language only contributes the
//! classification of its node kinds (`classify`); the counting rules are the
//! same for all of them, so a semantic exception is defined once.

use std::collections::BTreeSet;

use tree_sitter::{Node, Tree};

use crate::domain::findings::digest::sha256_hex;
use crate::domain::metrics::{
    FileMetrics, FileSize, FunctionMetrics, METRICS_RULES_VERSION, MetricsCoverage, MetricsOutcome,
    SizeMetrics, capability_matrix, is_measurable,
};
use crate::domain::value_objects::Language;
use crate::infrastructure::complexity::count_parameters;
use crate::infrastructure::parser::TreeSitterParser;

/// How deep the walk goes before it stops and says so. Real code is far
/// shallower; the limit protects the stack from machine-generated sources.
const MAX_WALK_DEPTH: u32 = 1500;

/// Measures `source` as the language its `path` extension names.
///
/// An extension no language claims is `Unsupported`, like a language without
/// rules.
pub fn measure_by_extension(path: &str, source: &str) -> MetricsOutcome {
    match Language::from_path(std::path::Path::new(path)) {
        Some(language) => measure_source(language, path, source),
        None => MetricsOutcome::Unsupported {
            language: "unknown".to_string(),
            reason: format!("no language is known for {path}; nothing was measured"),
        },
    }
}

/// Measures `source` as `language`.
///
/// Deterministic: the same bytes and rules give an identical report. `path` is
/// only a label that enters the function identities.
pub fn measure_source(language: Language, path: &str, source: &str) -> MetricsOutcome {
    if !is_measurable(language) {
        return MetricsOutcome::Unsupported {
            language: language.name().to_string(),
            reason: format!(
                "no native metric rules for {}; nothing was measured",
                language.name()
            ),
        };
    }
    let tree = match TreeSitterParser::new(language).and_then(|p| p.parse_tree(source)) {
        Ok(tree) => tree,
        Err(e) => {
            return MetricsOutcome::Failed {
                reason: format!("could not parse {path}: {e}"),
            };
        }
    };
    MetricsOutcome::Measured {
        metrics: Box::new(measure_tree(language, path, &tree, source)),
    }
}

/// Measures an already parsed tree. `language` must be measurable.
pub(crate) fn measure_tree(
    language: Language,
    path: &str,
    tree: &Tree,
    source: &str,
) -> FileMetrics {
    let root = tree.root_node();
    let mut functions = Vec::new();
    let mut reasons = Vec::new();
    collect_functions(language, root, source, path, &mut functions, &mut reasons);

    if root.has_error() {
        reasons.push("the file contains syntax the grammar could not read".to_string());
    }
    let coverage = if reasons.is_empty() {
        MetricsCoverage::Complete
    } else {
        MetricsCoverage::Partial { reasons }
    };
    FileMetrics {
        path: path.to_string(),
        language: language.name().to_string(),
        rules_version: METRICS_RULES_VERSION.to_string(),
        source_digest: sha256_hex(source.as_bytes()),
        size: FileSize {
            lines: u32::try_from(source.lines().count()).unwrap_or(u32::MAX),
            code_lines: code_lines(root, 0, root.end_position().row),
            functions: u32::try_from(functions.len()).unwrap_or(u32::MAX),
        },
        functions,
        coverage,
        capabilities: capability_matrix(language),
    }
}

/// Visits the tree in source order and measures every function that has a
/// body. Nested named functions are visited too, as units of their own.
fn collect_functions(
    language: Language,
    node: Node,
    source: &str,
    path: &str,
    out: &mut Vec<FunctionMetrics>,
    reasons: &mut Vec<String>,
) {
    if is_function(language, node) && has_body(language, node) {
        let (metrics, truncated) = measure_function(language, node, source, path);
        if truncated {
            reasons.push(format!(
                "{}: walk depth limit reached; its numbers are lower bounds",
                metrics.id
            ));
        }
        out.push(metrics);
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_functions(language, child, source, path, out, reasons);
    }
}

fn is_function(language: Language, node: Node) -> bool {
    matches!(
        (language, node.kind()),
        (Language::Rust, "function_item")
            | (
                Language::Java,
                "method_declaration" | "constructor_declaration"
            )
            | (
                Language::Kotlin,
                "function_declaration" | "secondary_constructor"
            )
            | (Language::Go, "function_declaration" | "method_declaration")
    )
}

/// A declaration without a body (an interface method, an abstract function)
/// has nothing to measure.
fn has_body(language: Language, node: Node) -> bool {
    match language {
        Language::Kotlin => {
            node.kind() == "secondary_constructor" || child_of_kind(node, "function_body").is_some()
        }
        _ => node.child_by_field_name("body").is_some(),
    }
}

fn child_of_kind<'t>(node: Node<'t>, kind: &str) -> Option<Node<'t>> {
    let mut cursor = node.walk();
    node.children(&mut cursor).find(|c| c.kind() == kind)
}

fn text<'s>(node: Node, source: &'s str) -> &'s str {
    node.utf8_text(source.as_bytes()).unwrap_or("")
}

fn measure_function(
    language: Language,
    node: Node,
    source: &str,
    path: &str,
) -> (FunctionMetrics, bool) {
    let name = function_name(language, node, source);
    let container = container_of(language, node, source);
    let qualified = match &container {
        Some(c) => format!("{c}.{name}"),
        None => name.clone(),
    };
    let start_line = u32::try_from(node.start_position().row + 1).unwrap_or(u32::MAX);
    let end_line = u32::try_from(node.end_position().row + 1).unwrap_or(u32::MAX);

    let mut walker = Walker {
        language,
        source,
        root: node,
        acc: Counts::default(),
    };
    walker.visit(node, Scope::default(), 0);
    let Counts {
        cyclomatic,
        cognitive,
        max_depth,
        callees,
        truncated,
    } = walker.acc;
    let callees: Vec<String> = callees.into_iter().collect();

    let metrics = FunctionMetrics {
        id: format!("{path}:{qualified}:{start_line}"),
        name,
        container,
        start_line,
        end_line,
        cyclomatic: 1 + cyclomatic,
        cognitive,
        nesting_depth: max_depth,
        size: SizeMetrics {
            lines: end_line - start_line + 1,
            code_lines: code_lines(node, node.start_position().row, node.end_position().row),
            parameters: count_parameters(node, source),
        },
        fan_out: u32::try_from(callees.len()).unwrap_or(u32::MAX),
        callees,
        syntax_errors: node.has_error(),
    };
    (metrics, truncated)
}

/// The declared name. Kotlin spells it as a plain child, a secondary
/// constructor has none and takes the class it belongs to.
fn function_name(language: Language, node: Node, source: &str) -> String {
    if let Some(name) = node.child_by_field_name("name") {
        return text(name, source).to_string();
    }
    if language == Language::Kotlin {
        if node.kind() == "secondary_constructor" {
            return enclosing(node, "class_declaration")
                .and_then(|c| child_of_kind(c, "type_identifier"))
                .map_or_else(
                    || "constructor".to_string(),
                    |n| text(n, source).to_string(),
                );
        }
        if let Some(id) = child_of_kind(node, "simple_identifier") {
            return text(id, source).to_string();
        }
    }
    "<anonymous>".to_string()
}

fn enclosing<'t>(node: Node<'t>, kind: &str) -> Option<Node<'t>> {
    let mut cur = node.parent();
    while let Some(n) = cur {
        if n.kind() == kind {
            return Some(n);
        }
        cur = n.parent();
    }
    None
}

/// The chain of enclosing types and functions, outermost first, joined by `.`.
fn container_of(language: Language, node: Node, source: &str) -> Option<String> {
    let mut parts = Vec::new();
    let mut cur = node.parent();
    while let Some(n) = cur {
        if is_function(language, n) {
            parts.push(function_name(language, n, source));
        } else if let Some(name) = type_name(language, n, source) {
            parts.push(name);
        }
        cur = n.parent();
    }
    if language == Language::Go
        && node.kind() == "method_declaration"
        && let Some(receiver) = go_receiver_type(node, source)
    {
        parts.push(receiver);
    }
    parts.reverse();
    (!parts.is_empty()).then(|| parts.join("."))
}

fn go_receiver_type(node: Node, source: &str) -> Option<String> {
    let receiver = node.child_by_field_name("receiver")?;
    let param = receiver.named_child(0)?;
    let ty = param.child_by_field_name("type")?;
    Some(
        text(ty, source)
            .trim_start_matches('*')
            .split('[')
            .next()
            .unwrap_or("")
            .to_string(),
    )
}

fn type_name(language: Language, node: Node, source: &str) -> Option<String> {
    match (language, node.kind()) {
        (Language::Rust, "impl_item") => node.child_by_field_name("type").map(|t| {
            text(t, source)
                .split('<')
                .next()
                .unwrap_or("")
                .trim()
                .to_string()
        }),
        (Language::Rust, "trait_item")
        | (
            Language::Java,
            "class_declaration"
            | "interface_declaration"
            | "enum_declaration"
            | "record_declaration",
        ) => node
            .child_by_field_name("name")
            .map(|n| text(n, source).to_string()),
        (Language::Kotlin, "class_declaration" | "object_declaration") => {
            child_of_kind(node, "type_identifier").map(|n| text(n, source).to_string())
        }
        (Language::Kotlin, "companion_object") => Some("Companion".to_string()),
        _ => None,
    }
}

/// Counts lines in `first..=last` that carry something other than whitespace
/// and comments.
fn code_lines(node: Node, first: usize, last: usize) -> u32 {
    let mut rows = vec![false; last - first + 1];
    mark_code_rows(node, first, &mut rows);
    u32::try_from(rows.iter().filter(|r| **r).count()).unwrap_or(u32::MAX)
}

fn mark_code_rows(node: Node, first: usize, rows: &mut [bool]) {
    if node.kind().contains("comment") || node.is_missing() {
        return;
    }
    if node.child_count() == 0 {
        let from = node.start_position().row.max(first) - first;
        let to = (node.end_position().row - first).min(rows.len() - 1);
        for row in rows.iter_mut().take(to + 1).skip(from) {
            *row = true;
        }
        return;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        mark_code_rows(child, first, rows);
    }
}

// ---------------------------------------------------------------------------
// Classification
// ---------------------------------------------------------------------------

/// What a syntax node means for the metrics.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Construct {
    /// `if`, `else if` included.
    If,
    /// Any repeating statement.
    Loop,
    /// A `match`, `when`, `switch` or `select` as a whole.
    Switch,
    /// One arm, entry or case; `default` ones add nothing to cyclomatic.
    Case { default: bool },
    /// `catch` clause.
    Catch,
    /// `try` block (nesting only).
    Try,
    /// Conditional expression.
    Ternary,
    /// Lambda or closure: part of the enclosing function.
    Closure,
    /// `&&` or `||`.
    Bool(&'static str),
    /// Elvis `?:`.
    Elvis,
    /// Rust `?`.
    TryOperator,
    /// Rust `let ... else`.
    LetElse,
    /// `break`/`continue`/`goto` to a label.
    LabeledJump,
    /// A call; carries the callee key when it can be named.
    Call(Option<String>),
    /// Parentheses: transparent for boolean operator runs.
    Parenthesized,
    /// Anything else.
    Other,
}

fn operator<'s>(node: Node, source: &'s str) -> &'s str {
    node.child_by_field_name("operator")
        .map_or("", |o| text(o, source))
}

fn bool_op(node: Node, source: &str) -> Construct {
    match operator(node, source) {
        "&&" => Construct::Bool("&&"),
        "||" => Construct::Bool("||"),
        _ => Construct::Other,
    }
}

fn has_token(node: Node, kind: &str) -> bool {
    let mut cursor = node.walk();
    node.children(&mut cursor).any(|c| c.kind() == kind)
}

fn classify(language: Language, node: Node, source: &str) -> Construct {
    match language {
        Language::Rust => classify_rust(node, source),
        Language::Java => classify_java(node, source),
        Language::Kotlin => classify_kotlin(node, source),
        Language::Go => classify_go(node, source),
        _ => Construct::Other,
    }
}

/// A `match` arm is the default one when its pattern is `_` or a lowercase
/// binding and it has no guard: nothing is left to decide.
fn rust_arm_is_default(arm: Node, source: &str) -> bool {
    let Some(pattern) = child_of_kind(arm, "match_pattern") else {
        return false;
    };
    if has_token(pattern, "if") || pattern.child_count() != 1 {
        return false;
    }
    let only = pattern.child(0).expect("one child");
    match only.kind() {
        "_" => true,
        "identifier" => text(only, source)
            .chars()
            .next()
            .is_some_and(|c| c == '_' || c.is_lowercase()),
        _ => false,
    }
}

fn classify_rust(node: Node, source: &str) -> Construct {
    match node.kind() {
        "if_expression" => Construct::If,
        "while_expression" | "for_expression" | "loop_expression" => Construct::Loop,
        "match_expression" => Construct::Switch,
        "match_arm" => Construct::Case {
            default: rust_arm_is_default(node, source),
        },
        "closure_expression" => Construct::Closure,
        "binary_expression" => bool_op(node, source),
        "try_expression" => Construct::TryOperator,
        "let_declaration" if node.child_by_field_name("alternative").is_some() => {
            Construct::LetElse
        }
        "break_expression" | "continue_expression" if child_of_kind(node, "label").is_some() => {
            Construct::LabeledJump
        }
        "parenthesized_expression" => Construct::Parenthesized,
        "call_expression" => Construct::Call(rust_callee(node, source)),
        _ => Construct::Other,
    }
}

fn rust_callee(call: Node, source: &str) -> Option<String> {
    let mut function = call.child_by_field_name("function")?;
    while function.kind() == "generic_function" {
        function = function.child_by_field_name("function")?;
    }
    match function.kind() {
        "identifier" | "scoped_identifier" => Some(compact(text(function, source))),
        "field_expression" => function
            .child_by_field_name("field")
            .map(|f| format!(".{}", text(f, source))),
        _ => None,
    }
}

fn compact(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect()
}

fn classify_java(node: Node, source: &str) -> Construct {
    match node.kind() {
        "if_statement" => Construct::If,
        "for_statement" | "enhanced_for_statement" | "while_statement" | "do_statement" => {
            Construct::Loop
        }
        "switch_expression" => Construct::Switch,
        "switch_label" => Construct::Case {
            default: has_token(node, "default"),
        },
        "try_statement" | "try_with_resources_statement" => Construct::Try,
        "catch_clause" => Construct::Catch,
        "ternary_expression" => Construct::Ternary,
        "lambda_expression" => Construct::Closure,
        "binary_expression" => bool_op(node, source),
        "break_statement" | "continue_statement" if child_of_kind(node, "identifier").is_some() => {
            Construct::LabeledJump
        }
        "parenthesized_expression" => Construct::Parenthesized,
        "method_invocation" => Construct::Call(node.child_by_field_name("name").map(|n| {
            if node.child_by_field_name("object").is_some() {
                format!(".{}", text(n, source))
            } else {
                text(n, source).to_string()
            }
        })),
        "object_creation_expression" => Construct::Call(
            node.child_by_field_name("type")
                .map(|t| format!("new {}", compact(text(t, source)))),
        ),
        "explicit_constructor_invocation" => Construct::Call(
            node.child_by_field_name("constructor")
                .map(|c| text(c, source).to_string()),
        ),
        _ => Construct::Other,
    }
}

fn classify_kotlin(node: Node, source: &str) -> Construct {
    match node.kind() {
        "if_expression" => Construct::If,
        "for_statement" | "while_statement" | "do_while_statement" => Construct::Loop,
        "when_expression" => Construct::Switch,
        "when_entry" => Construct::Case {
            default: has_token(node, "else"),
        },
        "try_expression" => Construct::Try,
        "catch_block" => Construct::Catch,
        "lambda_literal" | "anonymous_function" => Construct::Closure,
        "conjunction_expression" => Construct::Bool("&&"),
        "disjunction_expression" => Construct::Bool("||"),
        "elvis_expression" => Construct::Elvis,
        "jump_expression"
            if child_of_kind(node, "label").is_some()
                && node.child(0).is_some_and(|t| {
                    t.kind().starts_with("break") || t.kind().starts_with("continue")
                }) =>
        {
            Construct::LabeledJump
        }
        "parenthesized_expression" => Construct::Parenthesized,
        "call_expression" => Construct::Call(kotlin_callee(node, source)),
        _ => Construct::Other,
    }
}

fn kotlin_callee(call: Node, source: &str) -> Option<String> {
    let callee = call.named_child(0)?;
    match callee.kind() {
        "simple_identifier" => Some(text(callee, source).to_string()),
        "navigation_expression" => {
            let mut cursor = callee.walk();
            let suffix = callee
                .children(&mut cursor)
                .filter(|c| c.kind() == "navigation_suffix")
                .last()?;
            child_of_kind(suffix, "simple_identifier").map(|id| format!(".{}", text(id, source)))
        }
        _ => None,
    }
}

fn classify_go(node: Node, source: &str) -> Construct {
    match node.kind() {
        "if_statement" => Construct::If,
        "for_statement" => Construct::Loop,
        "expression_switch_statement" | "type_switch_statement" | "select_statement" => {
            Construct::Switch
        }
        "expression_case" | "type_case" | "communication_case" => {
            Construct::Case { default: false }
        }
        "default_case" => Construct::Case { default: true },
        "func_literal" => Construct::Closure,
        "binary_expression" => bool_op(node, source),
        "goto_statement" => Construct::LabeledJump,
        "break_statement" | "continue_statement" if child_of_kind(node, "label_name").is_some() => {
            Construct::LabeledJump
        }
        "parenthesized_expression" => Construct::Parenthesized,
        "call_expression" => Construct::Call(node.child_by_field_name("function").and_then(|f| {
            match f.kind() {
                "identifier" => Some(text(f, source).to_string()),
                "selector_expression" => f
                    .child_by_field_name("field")
                    .map(|n| format!(".{}", text(n, source))),
                _ => None,
            }
        })),
        _ => Construct::Other,
    }
}

/// Whether `node` is an `else if`: an `if` that is the alternative of another.
fn is_else_if(language: Language, node: Node) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };
    match language {
        Language::Rust => parent.kind() == "else_clause",
        Language::Kotlin => {
            parent.kind() == "control_structure_body"
                && parent.named_child_count() == 1
                && parent.prev_sibling().is_some_and(|s| s.kind() == "else")
        }
        _ => node.prev_sibling().is_some_and(|s| s.kind() == "else"),
    }
}

/// Whether `child` of an `if` is its `else` part, plain or chained.
fn is_alternative(child: Node) -> bool {
    child.kind() == "else_clause" || child.prev_sibling().is_some_and(|s| s.kind() == "else")
}

/// Whether the else part of an `if` is another `if`.
fn alternative_is_if(language: Language, alternative: Node) -> bool {
    let inner = match alternative.kind() {
        "else_clause" | "control_structure_body" if alternative.named_child_count() == 1 => {
            alternative.named_child(0)
        }
        _ => Some(alternative),
    };
    inner.is_some_and(|n| classify(language, n, "") == Construct::If)
}

// ---------------------------------------------------------------------------
// Walk
// ---------------------------------------------------------------------------

/// Position of a node inside the function being measured.
#[derive(Debug, Clone, Copy, Default)]
struct Scope {
    /// Nesting that weighs cognitive increments.
    nesting: u32,
    /// Enclosing control-flow structures, for the reported depth.
    depth: u32,
    /// Operator of the boolean run this node continues, if any.
    run: Option<&'static str>,
}

impl Scope {
    fn nested(self) -> Self {
        Self {
            nesting: self.nesting + 1,
            run: None,
            ..self
        }
    }

    fn deeper(self) -> Self {
        Self {
            nesting: self.nesting + 1,
            depth: self.depth + 1,
            run: None,
        }
    }

    fn flat(self) -> Self {
        Self { run: None, ..self }
    }
}

#[derive(Debug, Default)]
struct Counts {
    cyclomatic: u32,
    cognitive: u32,
    max_depth: u32,
    callees: BTreeSet<String>,
    truncated: bool,
}

struct Walker<'a> {
    language: Language,
    source: &'a str,
    root: Node<'a>,
    acc: Counts,
}

impl<'a> Walker<'a> {
    fn visit(&mut self, node: Node<'a>, scope: Scope, level: u32) {
        if node != self.root && is_function(self.language, node) {
            return;
        }
        if level > MAX_WALK_DEPTH {
            self.acc.truncated = true;
            return;
        }
        match classify(self.language, node, self.source) {
            Construct::If => self.visit_if(node, scope, level),
            construct @ (Construct::Loop | Construct::Switch) => {
                self.acc.cyclomatic += u32::from(construct == Construct::Loop);
                self.acc.cognitive += 1 + scope.nesting;
                self.enter_level(scope);
                self.children(node, scope.deeper(), level);
            }
            Construct::Case { default } => {
                self.acc.cyclomatic += u32::from(!default);
                self.children(node, scope.flat(), level);
            }
            Construct::Try => {
                self.enter_level(scope);
                self.children(
                    node,
                    Scope {
                        depth: scope.depth + 1,
                        run: None,
                        ..scope
                    },
                    level,
                );
            }
            Construct::Catch => {
                self.acc.cyclomatic += 1;
                self.acc.cognitive += 1 + scope.nesting;
                self.children(node, scope.nested(), level);
            }
            Construct::Ternary => {
                self.acc.cyclomatic += 1;
                self.acc.cognitive += 1 + scope.nesting;
                self.children(node, scope.nested(), level);
            }
            Construct::Closure => self.children(node, scope.nested(), level),
            Construct::Bool(op) => {
                self.acc.cyclomatic += 1;
                if scope.run != Some(op) {
                    self.acc.cognitive += 1;
                }
                self.children(
                    node,
                    Scope {
                        run: Some(op),
                        ..scope
                    },
                    level,
                );
            }
            Construct::Elvis | Construct::TryOperator => {
                self.acc.cyclomatic += 1;
                self.children(node, scope.flat(), level);
            }
            Construct::LetElse => {
                self.acc.cyclomatic += 1;
                self.acc.cognitive += 1 + scope.nesting;
                self.children(node, scope.flat(), level);
            }
            Construct::LabeledJump => {
                self.acc.cognitive += 1;
                self.children(node, scope.flat(), level);
            }
            Construct::Call(callee) => {
                if let Some(callee) = callee {
                    self.acc.callees.insert(callee);
                }
                self.children(node, scope.flat(), level);
            }
            Construct::Parenthesized => self.children(node, scope, level),
            Construct::Other => self.children(node, scope.flat(), level),
        }
    }

    fn enter_level(&mut self, scope: Scope) {
        self.acc.max_depth = self.acc.max_depth.max(scope.depth + 1);
    }

    fn children(&mut self, node: Node<'a>, scope: Scope, level: u32) {
        let mut cursor = node.walk();
        let children: Vec<Node<'a>> = node.children(&mut cursor).collect();
        for child in children {
            self.visit(child, scope, level + 1);
        }
    }

    /// An `if` costs one decision. A first `if` weighs the nesting it sits
    /// in; an `else if` continues the chain at the same level and is flat; a
    /// plain `else` is a flat increment whose body is one level deeper.
    fn visit_if(&mut self, node: Node<'a>, scope: Scope, level: u32) {
        self.acc.cyclomatic += 1;
        let chained = is_else_if(self.language, node);
        if chained {
            self.acc.cognitive += 1;
        } else {
            self.acc.cognitive += 1 + scope.nesting;
        }
        self.enter_level(scope);

        let mut cursor = node.walk();
        let children: Vec<Node<'a>> = node.children(&mut cursor).collect();
        for child in children {
            if !is_alternative(child) {
                self.visit(child, scope.deeper(), level + 1);
            } else if alternative_is_if(self.language, child) {
                self.visit(child, scope.flat(), level + 1);
            } else {
                self.acc.cognitive += 1;
                self.visit(child, scope.deeper(), level + 1);
            }
        }
    }
}
