//! Reads a Terraform file into the IR without evaluating anything.
//!
//! The reader keeps every attribute as an expression ([`TfExpr`]) and only
//! writes down what the text says. A construct it does not model is recorded
//! as [`TfExpr::Opaque`] with the traversals it contains, so the dependency
//! survives even when the value cannot be known. Nothing is executed: the
//! input is a tree the generic pass already built from the file's text.

use tree_sitter::{Node, Tree};

use super::terraform_handler::{
    block_header, hcl_header, is_bound_by_an_enclosing_for, line_of, node_text, top_level_blocks,
};
use crate::domain::iac::terraform::{
    Reference, ResourceMode, Step, TfAttribute, TfBlock, TfBody, TfExpr, TfLocal, TfModule,
    TfModuleCall, TfOutput, TfProvider, TfResource, TfValue, TfVariable, UnknownCause,
};

/// Module-call arguments that configure the call and are not inputs.
const MODULE_META_ARGUMENTS: &[&str] = &["source", "version", "providers", "depends_on"];

/// The deepest node nesting the reader follows. The reader recurses over the
/// tree, so a hostile file nested far beyond anything written by hand would
/// exhaust the stack; such a file is refused instead.
const MAX_NODE_DEPTH: usize = 512;

/// Refuses a tree nested deeper than [`MAX_NODE_DEPTH`]. The walk is iterative
/// so the check itself cannot overflow.
fn check_depth(file: &str, tree: &Tree) -> Result<(), String> {
    let mut cursor = tree.walk();
    let mut depth = 0usize;
    loop {
        if depth > MAX_NODE_DEPTH {
            return Err(format!(
                "{file} is nested more than {MAX_NODE_DEPTH} levels deep"
            ));
        }
        if cursor.goto_first_child() {
            depth += 1;
            continue;
        }
        while !cursor.goto_next_sibling() {
            if !cursor.goto_parent() {
                return Ok(());
            }
            depth -= 1;
        }
    }
}

/// Reads the declarations of one file. `file` is the path the nodes will
/// report. A file with syntax errors is refused whole, like the generic pass
/// refuses it: half a file is not a configuration anyone can rely on.
pub fn read_terraform_file(file: &str, source: &str, tree: &Tree) -> Result<TfModule, String> {
    if tree.root_node().has_error() {
        return Err(format!("Syntax errors in {file}"));
    }
    check_depth(file, tree)?;
    let src = source.as_bytes();
    let mut module = TfModule::default();
    for block in top_level_blocks(tree.root_node()) {
        let header = block_header(block, src);
        let Some((keyword, labels)) = hcl_header(&header) else {
            continue;
        };
        let body = read_body(block, src);
        let line = line_of(block);
        let file = file.to_string();
        match (keyword, labels.as_slice()) {
            ("resource" | "data", [kind, name, ..]) => {
                let mode = if keyword == "data" {
                    ResourceMode::Data
                } else {
                    ResourceMode::Managed
                };
                let resource = TfResource {
                    mode,
                    kind: kind.clone(),
                    name: name.clone(),
                    body,
                    file,
                    line,
                };
                module.declare_resource(resource);
            }
            ("variable", [name, ..]) => {
                let default = body.attribute("default").map(|a| a.expr.clone());
                let sensitive = is_true(body.attribute("sensitive"));
                module.declare_variable(TfVariable {
                    name: name.clone(),
                    default,
                    sensitive,
                    file,
                    line,
                });
            }
            ("output", [name, ..]) => {
                let value = body.attribute("value").map(|a| a.expr.clone());
                let sensitive = is_true(body.attribute("sensitive"));
                module.declare_output(TfOutput {
                    name: name.clone(),
                    value,
                    sensitive,
                    file,
                    line,
                });
            }
            ("locals", _) => {
                for attribute in body.attributes {
                    module.declare_local(TfLocal {
                        name: attribute.name,
                        expr: attribute.expr,
                        file: file.clone(),
                        line: attribute.line,
                    });
                }
            }
            ("module", [name, ..]) => {
                let source_text = match body.attribute("source").map(|a| &a.expr) {
                    Some(TfExpr::Literal(TfValue::Str(text))) => text.clone(),
                    _ => String::new(),
                };
                let args = body
                    .attributes
                    .into_iter()
                    .filter(|a| !MODULE_META_ARGUMENTS.contains(&a.name.as_str()))
                    .collect();
                module.declare_call(TfModuleCall {
                    name: name.clone(),
                    source: source_text,
                    args,
                    file,
                    line,
                });
            }
            ("provider", [name, ..]) => module.providers.push(TfProvider {
                name: name.clone(),
                body,
                file,
                line,
            }),
            _ => {}
        }
    }
    Ok(module)
}

/// The `name = value` assignments of a `.tfvars` file, as expressions.
pub fn read_tfvars(file: &str, source: &str, tree: &Tree) -> Result<Vec<TfAttribute>, String> {
    if tree.root_node().has_error() {
        return Err(format!("Syntax errors in {file}"));
    }
    check_depth(file, tree)?;
    let mut out = Vec::new();
    for body in children_of(tree.root_node()) {
        for item in children_of(body) {
            if item.kind() == "attribute"
                && let Some(attribute) = read_attribute(item, source.as_bytes())
            {
                out.push(attribute);
            }
        }
    }
    Ok(out)
}

fn is_true(attribute: Option<&TfAttribute>) -> bool {
    matches!(
        attribute.map(|a| &a.expr),
        Some(TfExpr::Literal(TfValue::Bool(true)))
    )
}

/// The attributes and nested blocks of a block, in source order.
fn read_body(block: Node, src: &[u8]) -> TfBody {
    let mut body = TfBody::default();
    let mut cursor = block.walk();
    let Some(inner) = block
        .named_children(&mut cursor)
        .find(|c| c.kind() == "body")
    else {
        return body;
    };
    let mut items = inner.walk();
    for item in inner.named_children(&mut items) {
        match item.kind() {
            "attribute" => {
                if let Some(attribute) = read_attribute(item, src) {
                    body.attributes.push(attribute);
                }
            }
            "block" => body.blocks.push(read_block(item, src)),
            _ => {}
        }
    }
    body
}

fn read_attribute(node: Node, src: &[u8]) -> Option<TfAttribute> {
    let mut cursor = node.walk();
    let children: Vec<Node> = node.named_children(&mut cursor).collect();
    let name = children.iter().find(|c| c.kind() == "identifier")?;
    let expression = children.iter().find(|c| c.kind() == "expression")?;
    Some(TfAttribute {
        name: node_text(*name, src),
        expr: read_expression(*expression, src),
        line: line_of(node),
    })
}

/// A nested block. `dynamic "name" { for_each = .. content { .. } }` becomes
/// a block named `name` that keeps the `for_each` and the `content` body, so
/// the rules can tell that its instances are computed.
fn read_block(node: Node, src: &[u8]) -> TfBlock {
    let header = block_header(node, src);
    let (kind, labels) = hcl_header(&header)
        .map(|(k, l)| (k.to_string(), l))
        .unwrap_or_default();
    let body = read_body(node, src);
    if kind == "dynamic" && !labels.is_empty() {
        let for_each = body.attribute("for_each").map(|a| Box::new(a.expr.clone()));
        let content = body
            .blocks
            .into_iter()
            .find(|b| b.kind == "content")
            .map(|b| b.body)
            .unwrap_or_default();
        return TfBlock {
            kind: labels[0].clone(),
            labels: labels[1..].to_vec(),
            dynamic: Some(for_each.unwrap_or_else(|| {
                Box::new(opaque(UnknownCause::UnsupportedExpression, Vec::new()))
            })),
            body: content,
            line: line_of(node),
        };
    }
    TfBlock {
        kind,
        labels,
        dynamic: None,
        body,
        line: line_of(node),
    }
}

fn opaque(cause: UnknownCause, refs: Vec<Reference>) -> TfExpr {
    TfExpr::Opaque { cause, refs }
}

fn children_of(node: Node) -> Vec<Node> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .filter(|c| c.kind() != "comment")
        .collect()
}

/// Reads an `expression` node.
fn read_expression(node: Node, src: &[u8]) -> TfExpr {
    let children = children_of(node);
    let Some(first) = children.first() else {
        return opaque(UnknownCause::UnsupportedExpression, Vec::new());
    };
    match first.kind() {
        "variable_expr" => read_traversal(&children, src),
        // Anything that continues after its head (`f(x).a`, `[1][0]`) is a
        // traversal over a computed value: it is not read as a literal.
        _ if children.len() > 1 => {
            let cause = match first.kind() {
                "function_call" => UnknownCause::FunctionCall {
                    name: function_name(*first, src),
                },
                "operation" => UnknownCause::Operation,
                _ => UnknownCause::UnsupportedExpression,
            };
            opaque(cause, refs_in(node, src))
        }
        "literal_value" => read_literal(*first, src),
        "template_expr" => read_template(*first, src),
        "collection_value" => read_collection(*first, src),
        "conditional" => {
            let parts: Vec<Node> = children_of(*first)
                .into_iter()
                .filter(|c| c.kind() == "expression")
                .collect();
            if let [condition, then, otherwise] = parts.as_slice() {
                TfExpr::Conditional {
                    condition: Box::new(read_expression(*condition, src)),
                    then: Box::new(read_expression(*then, src)),
                    otherwise: Box::new(read_expression(*otherwise, src)),
                }
            } else {
                opaque(UnknownCause::UnsupportedExpression, refs_in(node, src))
            }
        }
        "function_call" => read_call(*first, src),
        "for_expr" => opaque(UnknownCause::ForExpression, refs_in(node, src)),
        "operation" => opaque(UnknownCause::Operation, refs_in(node, src)),
        // A parenthesised expression is the expression inside.
        "expression" => read_expression(*first, src),
        _ => opaque(UnknownCause::UnsupportedExpression, refs_in(node, src)),
    }
}

fn function_name(call: Node, src: &[u8]) -> String {
    children_of(call)
        .into_iter()
        .find(|c| c.kind() == "identifier")
        .map(|n| node_text(n, src))
        .unwrap_or_default()
}

fn read_call(call: Node, src: &[u8]) -> TfExpr {
    let name = function_name(call, src);
    let arguments = children_of(call)
        .into_iter()
        .find(|c| c.kind() == "function_arguments");
    let mut args = Vec::new();
    if let Some(arguments) = arguments {
        for argument in children_of(arguments) {
            if argument.kind() == "expression" {
                args.push(read_expression(argument, src));
            } else {
                // `f(list...)` expands an argument list: not modeled.
                return opaque(UnknownCause::FunctionCall { name }, refs_in(call, src));
            }
        }
    }
    TfExpr::Call {
        function: name,
        args,
    }
}

/// `root.attr[expr].attr` and its first break: a splat or anything else
/// that is not an attribute access or an index makes the whole traversal
/// opaque, with the references it still reads.
fn read_traversal(children: &[Node], src: &[u8]) -> TfExpr {
    let root = &children[0];
    let (steps, rest) = read_steps(&children[1..], src);
    let head = Reference {
        root: node_text(*root, src),
        steps,
        line: line_of(*root),
    };
    if rest.is_empty() {
        return TfExpr::Ref(head);
    }
    let cause = if rest[0].kind() == "splat" {
        UnknownCause::Splat
    } else {
        UnknownCause::UnsupportedExpression
    };
    let mut refs = vec![head];
    for node in rest {
        collect_refs_in(*node, src, &mut refs);
    }
    opaque(cause, refs)
}

/// The leading `.attr` and `[index]` nodes of `nodes` as steps, and the nodes
/// left over.
fn read_steps<'a, 'b>(nodes: &'b [Node<'a>], src: &[u8]) -> (Vec<Step>, &'b [Node<'a>]) {
    let mut steps = Vec::new();
    let mut taken = 0;
    for node in nodes {
        match node.kind() {
            "get_attr" => {
                let Some(identifier) = children_of(*node)
                    .into_iter()
                    .find(|c| c.kind() == "identifier")
                else {
                    break;
                };
                steps.push(Step::Attr(node_text(identifier, src)));
            }
            "index" => {
                let inner = children_of(*node).into_iter().next();
                match inner.map(|n| (n.kind(), n)) {
                    Some(("new_index", n)) => {
                        let expression = children_of(n)
                            .into_iter()
                            .find(|c| c.kind() == "expression");
                        match expression {
                            Some(e) => steps.push(Step::Index(Box::new(read_expression(e, src)))),
                            None => break,
                        }
                    }
                    Some(("legacy_index", n)) => {
                        let text = node_text(n, src);
                        let text = text.trim_start_matches('.').to_string();
                        steps.push(Step::Index(Box::new(TfExpr::Literal(TfValue::Number(
                            text,
                        )))));
                    }
                    _ => break,
                }
            }
            _ => break,
        }
        taken += 1;
    }
    (steps, &nodes[taken..])
}

// -- literals -------------------------------------------------------------

fn read_literal(node: Node, src: &[u8]) -> TfExpr {
    let Some(inner) = children_of(node).into_iter().next() else {
        return opaque(UnknownCause::UnsupportedExpression, Vec::new());
    };
    match inner.kind() {
        "numeric_lit" => TfExpr::Literal(TfValue::Number(node_text(inner, src))),
        "bool_lit" => TfExpr::Literal(TfValue::Bool(node_text(inner, src) == "true")),
        "null_lit" => TfExpr::Literal(TfValue::Null),
        "string_lit" => {
            let text = children_of(inner)
                .into_iter()
                .find(|c| c.kind() == "template_literal")
                .map(|c| node_text(c, src))
                .unwrap_or_default();
            if has_escape(&text) {
                opaque(UnknownCause::StringEscape, Vec::new())
            } else {
                TfExpr::Literal(TfValue::Str(text))
            }
        }
        _ => opaque(UnknownCause::UnsupportedExpression, Vec::new()),
    }
}

/// Whether quoted text carries an escape the analysis does not decode.
fn has_escape(text: &str) -> bool {
    text.contains('\\') || has_literal_marker(text)
}

/// `$${` and `%%{` write a literal `${` and `%{`.
fn has_literal_marker(text: &str) -> bool {
    text.contains("$${") || text.contains("%%{")
}

fn read_template(node: Node, src: &[u8]) -> TfExpr {
    let Some(template) = children_of(node).into_iter().next() else {
        return opaque(UnknownCause::UnsupportedExpression, Vec::new());
    };
    match template.kind() {
        "quoted_template" => read_quoted_template(template, src),
        "heredoc_template" => read_heredoc(template, src),
        _ => opaque(UnknownCause::UnsupportedExpression, refs_in(node, src)),
    }
}

fn read_quoted_template(template: Node, src: &[u8]) -> TfExpr {
    let mut parts = Vec::new();
    let mut cursor = template.walk();
    for child in template.named_children(&mut cursor) {
        match child.kind() {
            "template_literal" => {
                let text = node_text(child, src);
                if has_escape(&text) {
                    return opaque(UnknownCause::StringEscape, refs_in(template, src));
                }
                parts.push(TfExpr::Literal(TfValue::Str(text)));
            }
            "template_interpolation" => match interpolation(child, src) {
                Some(expr) => parts.push(expr),
                None => {
                    return opaque(UnknownCause::UnsupportedExpression, refs_in(template, src));
                }
            },
            "template_directive" => {
                return opaque(UnknownCause::TemplateDirective, refs_in(template, src));
            }
            _ => {}
        }
    }
    finish_template(parts)
}

/// `"${x}"` is `x` itself and a text with no interpolation is a literal;
/// anything else is a template.
fn finish_template(mut parts: Vec<TfExpr>) -> TfExpr {
    match parts.len() {
        0 => TfExpr::Literal(TfValue::Str(String::new())),
        1 => parts.remove(0),
        _ => TfExpr::Template(parts),
    }
}

/// The expression of `${ expr }`; `None` when whitespace-trimming markers
/// (`${~`) change what the surrounding text means.
fn interpolation(node: Node, src: &[u8]) -> Option<TfExpr> {
    let children = children_of(node);
    let start = children
        .iter()
        .find(|c| c.kind() == "template_interpolation_start")?;
    let end = children
        .iter()
        .find(|c| c.kind() == "template_interpolation_end")?;
    if node_text(*start, src) != "${" || node_text(*end, src) != "}" {
        return None;
    }
    let expression = children.iter().find(|c| c.kind() == "expression")?;
    Some(read_expression(*expression, src))
}

/// A heredoc is read from the source text between its markers, because the
/// grammar splits its literal text in ways that lose the newlines.
fn read_heredoc(template: Node, src: &[u8]) -> TfExpr {
    let children = children_of(template);
    let indented = children
        .iter()
        .find(|c| c.kind() == "heredoc_start")
        .is_some_and(|c| node_text(*c, src).ends_with('-'));
    let identifiers: Vec<&Node> = children
        .iter()
        .filter(|c| c.kind() == "heredoc_identifier")
        .collect();
    let (Some(open), Some(close)) = (identifiers.first(), identifiers.last()) else {
        return opaque(UnknownCause::UnsupportedExpression, refs_in(template, src));
    };
    if identifiers.len() < 2 {
        return opaque(UnknownCause::UnsupportedExpression, refs_in(template, src));
    }
    let mut cursor = open.end_byte();
    if src.get(cursor) == Some(&b'\r') {
        cursor += 1;
    }
    if src.get(cursor) == Some(&b'\n') {
        cursor += 1;
    }
    let slice = |from: usize, to: usize| String::from_utf8_lossy(&src[from..to]).into_owned();

    let mut parts = Vec::new();
    let mut has_dynamic = false;
    for child in &children {
        match child.kind() {
            "template_interpolation" => {
                let Some(expr) = interpolation(*child, src) else {
                    return opaque(UnknownCause::UnsupportedExpression, refs_in(template, src));
                };
                parts.push(TfExpr::Literal(TfValue::Str(slice(
                    cursor,
                    child.start_byte(),
                ))));
                parts.push(expr);
                cursor = child.end_byte();
                has_dynamic = true;
            }
            "template_directive" => {
                return opaque(UnknownCause::TemplateDirective, refs_in(template, src));
            }
            _ => {}
        }
    }
    parts.push(TfExpr::Literal(TfValue::Str(slice(
        cursor,
        close.start_byte(),
    ))));
    let literal_text: String = parts
        .iter()
        .filter_map(|p| match p {
            TfExpr::Literal(TfValue::Str(s)) => Some(s.as_str()),
            _ => None,
        })
        .collect();
    if has_literal_marker(&literal_text) {
        return opaque(UnknownCause::StringEscape, refs_in(template, src));
    }
    if indented {
        if has_dynamic {
            // The indentation to strip depends on the whole text, which an
            // interpolation changes.
            return opaque(UnknownCause::UnsupportedExpression, refs_in(template, src));
        }
        return TfExpr::Literal(TfValue::Str(strip_indent(&literal_text)));
    }
    parts.retain(|p| !matches!(p, TfExpr::Literal(TfValue::Str(s)) if s.is_empty()));
    finish_template(parts)
}

/// `<<-` removes the smallest common leading whitespace of the non-blank
/// lines, and the whitespace that precedes the closing marker is not text.
fn strip_indent(text: &str) -> String {
    let mut lines: Vec<&str> = text.split('\n').collect();
    if lines.last().is_some_and(|l| l.trim().is_empty()) {
        lines.pop();
        lines.push("");
    }
    let indent = lines
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start().len())
        .min()
        .unwrap_or(0);
    lines
        .iter()
        .map(|l| {
            if l.len() >= indent {
                &l[indent..]
            } else {
                l.trim_start()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// -- collections ----------------------------------------------------------

fn read_collection(node: Node, src: &[u8]) -> TfExpr {
    let Some(inner) = children_of(node).into_iter().next() else {
        return opaque(UnknownCause::UnsupportedExpression, Vec::new());
    };
    match inner.kind() {
        "tuple" => TfExpr::List(
            children_of(inner)
                .into_iter()
                .filter(|c| c.kind() == "expression")
                .map(|c| read_expression(c, src))
                .collect(),
        ),
        "object" => {
            let mut entries = Vec::new();
            for element in children_of(inner)
                .into_iter()
                .filter(|c| c.kind() == "object_elem")
            {
                let pair: Vec<Node> = children_of(element)
                    .into_iter()
                    .filter(|c| c.kind() == "expression")
                    .collect();
                let [key, value] = pair.as_slice() else {
                    return opaque(UnknownCause::UnsupportedExpression, refs_in(node, src));
                };
                entries.push((read_key(*key, src), read_expression(*value, src)));
            }
            TfExpr::Object(entries)
        }
        _ => opaque(UnknownCause::UnsupportedExpression, refs_in(node, src)),
    }
}

/// A bare identifier key (`{ a = 1 }`) is the string `a`, not a reference.
fn read_key(key: Node, src: &[u8]) -> TfExpr {
    let children = children_of(key);
    if let [only] = children.as_slice()
        && only.kind() == "variable_expr"
    {
        return TfExpr::Literal(TfValue::Str(node_text(*only, src)));
    }
    read_expression(key, src)
}

// -- references inside constructs that are not evaluated -------------------

/// Every traversal read inside `node`, outside the names a `for` expression
/// binds for itself.
fn refs_in(node: Node, src: &[u8]) -> Vec<Reference> {
    let mut out = Vec::new();
    collect_refs_in(node, src, &mut out);
    out
}

fn collect_refs_in(node: Node, src: &[u8], out: &mut Vec<Reference>) {
    let children = children_of(node);
    let mut i = 0;
    while i < children.len() {
        let child = children[i];
        if child.kind() == "variable_expr" {
            let (steps, rest) = read_steps(&children[i + 1..], src);
            let consumed = children.len() - i - 1 - rest.len();
            let name = node_text(child, src);
            if !is_bound_by_an_enclosing_for(child, &name, src) {
                out.push(Reference {
                    root: name,
                    steps,
                    line: line_of(child),
                });
            }
            i += 1 + consumed;
            continue;
        }
        // A unary or binary operation ends in an operand whose accesses
        // follow the operation, as siblings (`!var.a` reads `var.a`).
        if child.kind() == "operation" {
            let tail: Vec<Node> = children[i + 1..]
                .iter()
                .take_while(|n| matches!(n.kind(), "get_attr" | "index"))
                .copied()
                .collect();
            collect_operation_refs(child, &tail, src, out);
            i += 1 + tail.len();
            continue;
        }
        collect_refs_in(child, src, out);
        i += 1;
    }
}

/// An operation whose accesses were split off by the grammar: they belong to
/// the last operand when that operand is a bare name.
fn collect_operation_refs(operation: Node, tail: &[Node], src: &[u8], out: &mut Vec<Reference>) {
    let last_operand = last_operand(operation);
    match last_operand {
        Some(name) if !tail.is_empty() && name.next_named_sibling().is_none() => {
            // Everything before the last operand reads normally.
            collect_refs_except(operation, name, src, out);
            let (steps, rest) = read_steps(tail, src);
            let text = node_text(name, src);
            if !is_bound_by_an_enclosing_for(name, &text, src) {
                out.push(Reference {
                    root: text,
                    steps,
                    line: line_of(name),
                });
            }
            for node in rest {
                collect_refs_in(*node, src, out);
            }
        }
        _ => {
            collect_refs_in(operation, src, out);
            for node in tail {
                collect_refs_in(*node, src, out);
            }
        }
    }
}

fn last_operand(operation: Node) -> Option<Node> {
    let mut node = operation;
    loop {
        let last = children_of(node).into_iter().next_back()?;
        match last.kind() {
            "variable_expr" => return Some(last),
            "binary_operation" | "unary_operation" | "operation" => node = last,
            _ => return None,
        }
    }
}

fn collect_refs_except(node: Node, skip: Node, src: &[u8], out: &mut Vec<Reference>) {
    let children = children_of(node);
    let mut i = 0;
    while i < children.len() {
        let child = children[i];
        if child.id() == skip.id() {
            i += 1;
            continue;
        }
        if child.kind() == "variable_expr" {
            let (steps, rest) = read_steps(&children[i + 1..], src);
            let consumed = children.len() - i - 1 - rest.len();
            let name = node_text(child, src);
            if !is_bound_by_an_enclosing_for(child, &name, src) {
                out.push(Reference {
                    root: name,
                    steps,
                    line: line_of(child),
                });
            }
            i += 1 + consumed;
            continue;
        }
        if child.kind() == "binary_operation" || child.kind() == "unary_operation" {
            collect_refs_except(child, skip, src, out);
        } else {
            collect_refs_in(child, src, out);
        }
        i += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn module(source: &str) -> TfModule {
        let tree = super::super::tree::build_tree(&tree_sitter_hcl::LANGUAGE.into(), source)
            .expect("parses");
        read_terraform_file("main.tf", source, &tree).expect("reads")
    }

    /// The expression of `attr` in a resource `r.x`.
    fn expr(attribute: &str, value: &str) -> TfExpr {
        let source = format!("resource \"r\" \"x\" {{\n  {attribute} = {value}\n}}\n");
        let module = module(&source);
        module.resources["r.x"]
            .body
            .attribute(attribute)
            .expect("attribute")
            .expr
            .clone()
    }

    fn text(s: &str) -> TfExpr {
        TfExpr::Literal(TfValue::Str(s.into()))
    }

    fn refs_of(expr: &TfExpr) -> Vec<String> {
        expr.references().iter().map(|r| r.text()).collect()
    }

    #[test]
    fn literals_keep_their_type() {
        assert_eq!(expr("a", "\"plain\""), text("plain"));
        assert_eq!(expr("a", "3"), TfExpr::Literal(TfValue::Number("3".into())));
        assert_eq!(expr("a", "true"), TfExpr::Literal(TfValue::Bool(true)));
        assert_eq!(expr("a", "false"), TfExpr::Literal(TfValue::Bool(false)));
        assert_eq!(expr("a", "null"), TfExpr::Literal(TfValue::Null));
        assert_eq!(expr("a", "\"\""), text(""));
    }

    #[test]
    fn a_traversal_keeps_attributes_and_indexes_as_steps() {
        let TfExpr::Ref(reference) = expr("a", "var.rules[0].port") else {
            panic!("a traversal");
        };
        assert_eq!(reference.root, "var");
        assert_eq!(reference.steps.len(), 3);
        assert_eq!(reference.steps[0], Step::Attr("rules".into()));
        assert_eq!(
            reference.steps[1],
            Step::Index(Box::new(TfExpr::Literal(TfValue::Number("0".into()))))
        );
        assert_eq!(reference.steps[2], Step::Attr("port".into()));
    }

    #[test]
    fn a_lone_interpolation_is_the_value_itself_and_text_around_it_is_a_template() {
        assert!(matches!(expr("a", "\"${var.a}\""), TfExpr::Ref(_)));
        let TfExpr::Template(parts) = expr("a", "\"x-${var.a}-y\"") else {
            panic!("a template");
        };
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0], text("x-"));
        assert_eq!(parts[2], text("-y"));
    }

    #[test]
    fn escapes_and_directives_are_not_decoded_but_keep_their_references() {
        let escaped = expr("a", "\"a\\nb\"");
        assert!(matches!(
            escaped,
            TfExpr::Opaque {
                cause: UnknownCause::StringEscape,
                ..
            }
        ));
        let directive = expr("a", "\"%{ if var.a }x%{ endif }\"");
        assert!(matches!(
            directive,
            TfExpr::Opaque {
                cause: UnknownCause::TemplateDirective,
                ..
            }
        ));
        assert_eq!(refs_of(&directive), ["var.a"]);
        assert!(matches!(
            expr("a", "\"$${literal}\""),
            TfExpr::Opaque {
                cause: UnknownCause::StringEscape,
                ..
            }
        ));
    }

    #[test]
    fn heredocs_keep_their_text_and_their_interpolations() {
        let plain = expr("a", "<<EOT\nline one\nline two\nEOT");
        assert_eq!(plain, text("line one\nline two\n"));
        let indented = expr("a", "<<-EOT\n    {\n      \"a\": 1\n    }\n    EOT");
        assert_eq!(indented, text("{\n  \"a\": 1\n}\n"));
        let with_ref = expr("a", "<<EOT\nhello ${var.who}!\nEOT");
        let TfExpr::Template(parts) = with_ref else {
            panic!("a template");
        };
        assert_eq!(parts[0], text("hello "));
        assert!(matches!(parts[1], TfExpr::Ref(_)));
        assert_eq!(parts[2], text("!\n"));
    }

    #[test]
    fn collections_read_bare_keys_as_strings_not_references() {
        let TfExpr::Object(entries) = expr("a", "{ k = 1, \"q\" = var.x, (var.key) = 2 }") else {
            panic!("an object");
        };
        assert_eq!(entries[0].0, text("k"));
        assert_eq!(entries[1].0, text("q"));
        assert!(matches!(entries[2].0, TfExpr::Ref(_)));
        let TfExpr::List(items) = expr("a", "[1, \"two\", var.z]") else {
            panic!("a list");
        };
        assert_eq!(items.len(), 3);
    }

    #[test]
    fn conditionals_and_calls_are_structured_and_parentheses_vanish() {
        assert!(matches!(
            expr("a", "var.on ? \"p\" : \"q\""),
            TfExpr::Conditional { .. }
        ));
        let TfExpr::Call { function, args } = expr("a", "lookup(var.m, \"k\", \"d\")") else {
            panic!("a call");
        };
        assert_eq!(function, "lookup");
        assert_eq!(args.len(), 3);
        assert!(matches!(expr("a", "(var.on)"), TfExpr::Ref(_)));
    }

    #[test]
    fn what_is_not_evaluated_stays_opaque_with_the_references_it_reads() {
        let splat = expr("a", "aws_instance.web[*].id");
        assert!(matches!(
            splat,
            TfExpr::Opaque {
                cause: UnknownCause::Splat,
                ..
            }
        ));
        assert_eq!(refs_of(&splat), ["aws_instance.web"]);

        let for_expr = expr("a", "[for s in var.list : s.id if s != var.skip]");
        assert!(matches!(
            for_expr,
            TfExpr::Opaque {
                cause: UnknownCause::ForExpression,
                ..
            }
        ));
        // `s` is bound by the expression and names nothing in the project.
        assert_eq!(refs_of(&for_expr), ["var.list", "var.skip"]);

        let binary = expr("a", "var.a + var.b.c");
        assert!(matches!(
            binary,
            TfExpr::Opaque {
                cause: UnknownCause::Operation,
                ..
            }
        ));
        assert_eq!(refs_of(&binary), ["var.a", "var.b.c"]);
    }

    #[test]
    fn the_accesses_the_grammar_splits_off_a_unary_operation_stay_with_their_operand() {
        let negated = expr("a", "!var.settings.enabled");
        assert_eq!(refs_of(&negated), ["var.settings.enabled"]);
        let computed = expr("a", "upper(var.a).x");
        assert!(matches!(
            computed,
            TfExpr::Opaque {
                cause: UnknownCause::FunctionCall { .. },
                ..
            }
        ));
        assert_eq!(refs_of(&computed), ["var.a"]);
    }

    #[test]
    fn dynamic_blocks_keep_their_for_each_and_content() {
        let module = module(
            r#"
resource "aws_security_group" "s" {
  dynamic "ingress" {
    for_each = var.rules
    content {
      from_port = ingress.value
    }
  }
  egress {
    from_port = 0
  }
}
"#,
        );
        let body = &module.resources["aws_security_group.s"].body;
        let dynamic = body.blocks.iter().find(|b| b.dynamic.is_some()).unwrap();
        assert_eq!(dynamic.kind, "ingress");
        assert!(dynamic.body.attribute("from_port").is_some());
        assert!(matches!(dynamic.dynamic.as_deref(), Some(TfExpr::Ref(_))));
        let plain = body.blocks.iter().find(|b| b.kind == "egress").unwrap();
        assert!(plain.dynamic.is_none());
    }

    #[test]
    fn every_declaration_kind_is_read_with_its_file_and_line() {
        let module = module(
            r#"variable "region" {
  default   = "eu-west-1"
  sensitive = true
}
locals {
  name = "app-${var.region}"
}
output "arn" {
  value = aws_s3_bucket.b.arn
}
data "aws_caller_identity" "me" {}
provider "aws" {
  region = var.region
}
module "net" {
  source  = "./net"
  version = "1.0"
  cidr    = "10.0.0.0/16"
  count   = 2
}
resource "aws_s3_bucket" "b" {}
"#,
        );
        let variable = &module.variables["region"];
        assert!(variable.sensitive);
        assert_eq!(variable.default, Some(text("eu-west-1")));
        assert_eq!((variable.file.as_str(), variable.line), ("main.tf", 1));
        assert_eq!(module.locals["name"].line, 6);
        assert!(module.outputs["arn"].value.is_some());
        assert!(module.resources.contains_key("data.aws_caller_identity.me"));
        assert_eq!(module.providers[0].name, "aws");
        let call = &module.calls["net"];
        assert_eq!(call.source, "./net");
        let args: Vec<&str> = call.args.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(args, ["cidr", "count"]);
        assert!(module.resources.contains_key("aws_s3_bucket.b"));
    }

    #[test]
    fn a_file_with_syntax_errors_is_refused_whole() {
        let source = "resource \"a\" \"b\" {\n  x = \n";
        let tree = super::super::tree::build_tree(&tree_sitter_hcl::LANGUAGE.into(), source)
            .expect("parses with errors");
        assert!(read_terraform_file("bad.tf", source, &tree).is_err());
    }

    #[test]
    fn a_tfvars_file_yields_its_assignments() {
        let source = "region = \"us-east-1\"\nports = [80, 443]\n";
        let tree = super::super::tree::build_tree(&tree_sitter_hcl::LANGUAGE.into(), source)
            .expect("parses");
        let attributes = read_tfvars("terraform.tfvars", source, &tree).unwrap();
        let names: Vec<&str> = attributes.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, ["region", "ports"]);
    }

    fn read_text(source: &str) -> Result<TfModule, String> {
        let tree = super::super::tree::build_tree(&tree_sitter_hcl::LANGUAGE.into(), source)
            .expect("parses");
        read_terraform_file("deep.tf", source, &tree)
    }

    #[test]
    fn nesting_beyond_the_cap_is_a_read_error_and_ordinary_nesting_is_read() {
        let nested = |depth: usize| {
            format!(
                "resource \"r\" \"x\" {{ a = {}1{} }}\n",
                "[".repeat(depth),
                "]".repeat(depth)
            )
        };
        let refused = read_text(&nested(100_000)).expect_err("hostile nesting is refused");
        assert!(refused.contains("nest"), "{refused}");
        let calls = format!(
            "resource \"r\" \"x\" {{ a = {}1{} }}\n",
            "f(".repeat(100_000),
            ")".repeat(100_000)
        );
        assert!(read_text(&calls).is_err());
        let blocks = format!("{}{}", "b {\n".repeat(100_000), "}\n".repeat(100_000));
        assert!(read_text(&blocks).is_err());
        assert!(
            read_text(&nested(40)).is_ok(),
            "human-written nesting is not affected"
        );
    }
}
