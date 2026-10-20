//! Terraform semantic handler: HCL blocks become infrastructure identities,
//! and the traversals inside their bodies become reference edges.
//!
//! ## Why this handler parses the file itself
//!
//! A generic extraction pass reduces every HCL block to a *label*, and a
//! label is only its header: `resource "aws_s3_bucket" "app_bucket"`. The
//! body —where the interesting part lives, `bucket = aws_s3_bucket.app_bucket.id`—
//! is gone before the domain layer sees it, so a handler that reads labels
//! can never find the relations the file actually states.
//!
//! Reading the file here keeps the domain layer the authority for its own
//! language: it decides what a block is and what refers to what, and the
//! generic pass stays what it is —the path taken by the languages that have
//! no domain layer of their own.

use crate::application::ingest::types::{ExtractionEdge, ExtractionResult, TargetRef};
use crate::domain::aggregates::{GraphNode, NodeId};
use crate::domain::value_objects::pending_reference::readings;
use crate::domain::value_objects::{Declaration, DependencyType, NodeKind, Provenance, SymbolKind};
use tree_sitter::Node;

/// A recognised HCL block, with everything needed to emit it and to resolve
/// the traversals of the other blocks against it.
struct Block<'a> {
    /// Domain identity: `aws_s3_bucket.app_bucket`, `var.region`,
    /// `module.vpc`. It is both the label of the node and the suffix of its id.
    identity: String,
    kind: SymbolKind,
    iac_type: Option<String>,
    iac_kind: &'static str,
    node: Node<'a>,
}

/// Interprets an HCL file as Terraform.
///
/// When the file declares no Terraform block at all, the result comes back
/// untouched: a `.tfvars` file, an HCL file of another dialect or a JSON
/// file with an HCL extension are not a Terraform configuration.
pub fn interpret_terraform(
    source_path: &str,
    source_hash: &str,
    source: &str,
    result: &ExtractionResult,
) -> ExtractionResult {
    // The tree lives here and not in a helper that returns it: the nodes read
    // while resolving traversals point into it, and a `Node` that outlives its
    // `Tree` is a pointer to nothing.
    let Some(tree) = parse(source) else {
        return result.clone();
    };
    let blocks = terraform_blocks(tree.root_node(), source.as_bytes());
    if blocks.is_empty() {
        return result.clone();
    }

    let mut nodes = Vec::with_capacity(blocks.len() + 1);
    let mut edges = Vec::new();
    let file_node_id = NodeId::new(source_path);

    // The file node comes from the generic path and is not reinterpreted:
    // it is not a declaration.
    if let Some(file_node) = result.nodes.first() {
        nodes.push(file_node.clone());
    }

    // The identities declared in this file. This is the set the traversals are
    // resolved against, and it exists here because this is the only place where
    // both facts are true at once: the declarations have just been read, and
    // the uses are read right here too.
    let declared: Vec<&str> = blocks.iter().map(|b| b.identity.as_str()).collect();

    for block in &blocks {
        let id = tf_id(source_path, &block.identity);
        let mut builder = GraphNode::builder(NodeId::new(&id), NodeKind::Symbol(block.kind))
            .label(&block.identity)
            .source_path(std::path::PathBuf::from(source_path))
            .property(
                Declaration::KIND_PROPERTY.to_string(),
                block.iac_kind.to_string(),
            );
        if let Some(t) = &block.iac_type {
            builder = builder.property(Declaration::TYPE_PROPERTY.to_string(), t.clone());
        }
        nodes.push(builder.build());

        let line = line_of(block.node);
        edges.push(ExtractionEdge {
            source: file_node_id.as_str().to_string(),
            target_ref: TargetRef::Resolved(id.clone()),
            kind: format!("dependency.{}", DependencyType::Contains),
            provenance: Provenance::Extracted,
            confidence: 1.0,
            line: Some(line),
        });

        for (target, ref_line) in
            references_in(block.node, source.as_bytes(), &declared, source_path)
        {
            edges.push(ExtractionEdge {
                source: id.clone(),
                target_ref: target,
                kind: format!("dependency.{}", DependencyType::References),
                provenance: Provenance::Extracted,
                confidence: 1.0,
                line: Some(ref_line),
            });
        }
    }

    ExtractionResult {
        source_path: std::path::PathBuf::from(source_path),
        nodes,
        edges,
        content_hash: source_hash.into(),
        statements_by_function: std::collections::BTreeMap::new(),
        error: result.error.clone(),
    }
}

/// The top-level Terraform blocks declared by `source`.
///
/// Top-level and not nested: `versioning_configuration { ... }` or
/// `rule { ... }` are internal structure of their own block, not resources.
/// Their traversals are still attributed to the block that contains them,
/// because the dependency belongs to that block.
fn terraform_blocks<'a>(root: Node<'a>, source: &[u8]) -> Vec<Block<'a>> {
    top_level_blocks(root)
        .into_iter()
        .filter_map(|node| classify(node, source))
        .collect()
}

fn parse(source: &str) -> Option<tree_sitter::Tree> {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_hcl::LANGUAGE.into())
        .ok()?;
    parser.parse(source, None)
}

/// `config_file > body > block`. A complete walk would also yield the nested
/// ones, and `aws_lb_listener` is not a resource apart from its
/// `default_action`.
fn top_level_blocks(root: Node) -> Vec<Node> {
    let mut cursor = root.walk();
    root.named_children(&mut cursor)
        .flat_map(|body| {
            let mut inner = body.walk();
            body.named_children(&mut inner)
                .collect::<Vec<_>>()
                .into_iter()
        })
        .filter(|n| n.kind() == "block")
        .collect()
}

/// The block header: everything that precedes the opening brace.
///
/// The text of the whole block would also contain the body, and splitting
/// it on whitespace would then yield `bucket`, `=` and the rest of the
/// attributes. The header is what names the block; the body is what uses it.
fn block_header(node: Node, source: &[u8]) -> String {
    let mut header = String::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "block_start" {
            break;
        }
        header.push_str(&node_text(child, source));
        header.push(' ');
    }
    header
}

/// `expression > variable_expr + get_attr*` is a traversal: the root
/// expression names the thing, and every attribute access adds a segment.
///
/// The attribute accesses are *siblings* of the root expression inside the
/// enclosing expression, not children of it. Asking for them as children
/// leaves every traversal with a single segment, and a single segment names
/// nothing.
///
/// `None` when the expression does not start from a root: a function call,
/// a concatenation or a literal is not a traversal, and its dots do not
/// name blocks.
fn traversal_segments(expression: Node, source: &[u8]) -> Option<Vec<String>> {
    let mut segments: Vec<String> = Vec::new();
    let mut cursor = expression.walk();
    for child in expression.named_children(&mut cursor) {
        match child.kind() {
            "variable_expr" if segments.is_empty() => segments.push(node_text(child, source)),
            // The chain ends where the expression ends.
            // `module.vpc.subnets[0]` is still a traversal of `module.vpc`:
            // an index does not name another block.
            "get_attr" if !segments.is_empty() => {
                let mut inner = child.walk();
                let identifier = child
                    .named_children(&mut inner)
                    .find(|c| c.kind() == "identifier")?;
                segments.push(node_text(identifier, source));
            }
            _ => break,
        }
    }
    // A lone root (`var`) names nothing: there is nothing to point at.
    (segments.len() > 1).then_some(segments)
}

/// The traversals in the body of a block, already resolved against what the
/// file declares.
///
/// A traversal has two possible readings and the file says which one it means:
/// `aws_s3_bucket.app_bucket.id` can denote the bucket
/// (`aws_s3_bucket.app_bucket`, a declared block) or something called
/// `aws_s3_bucket.app_bucket.id` (which nobody declares). The longest reading
/// that names something declared wins; when none does, the whole traversal
/// stays unresolved.
///
/// An edge whose far end was not resolved is never invented.
fn references_in(
    block: Node,
    source: &[u8],
    declared: &[&str],
    source_path: &str,
) -> Vec<(TargetRef, u32)> {
    let mut cursor = block.walk();
    let mut found: Vec<(TargetRef, u32)> = Vec::new();
    let mut queue: Vec<Node> = block.named_children(&mut cursor).collect();

    while let Some(node) = queue.pop() {
        if let Some((target_ref, line)) = reference_in(node, source, declared, source_path) {
            // A relation is a pair. Reading `var.environment` twice inside the
            // same block is one relation, not two, and repeating it only
            // inflates the count the graph shows.
            if !found.iter().any(|(t, _)| same_target(t, &target_ref)) {
                found.push((target_ref, line));
            }
        }
        let mut inner = node.walk();
        queue.extend(node.named_children(&mut inner));
    }
    found
}

/// `Some` when the node is an expression that names another block, already
/// resolved. `None` for everything else —literals, calls, indexes—: the walk
/// keeps going down, because an expression can contain another one.
fn reference_in(
    node: Node,
    source: &[u8],
    declared: &[&str],
    source_path: &str,
) -> Option<(TargetRef, u32)> {
    if node.kind() != "expression" {
        return None;
    }
    let segments = traversal_segments(node, source)?;
    // A name the language binds itself names no declaration of the project:
    // `count.index` is the index of the instance, `each.value` the key of
    // the map, and the name of a `for` expression is bound by that
    // expression. The graph answers about declarations, so these are not
    // references at all.
    let root = segments.first().map(String::as_str)?;
    if is_language_built_in(root) || is_bound_by_an_enclosing_for(node, root, source) {
        return None;
    }
    Some(resolve_traversal(
        &segments,
        declared,
        source_path,
        line_of(node),
    ))
}

/// The names Terraform binds itself: they are values of the language, not
/// declarations of the project.
fn is_language_built_in(name: &str) -> bool {
    matches!(name, "count" | "each" | "self" | "path")
}

/// Whether the name is bound by a `for` expression that encloses the node.
///
/// `[for s in aws_subnet.public : s.id]` binds `s` for that expression, and
/// `s` names no declaration of the project.
fn is_bound_by_an_enclosing_for(node: Node, name: &str, source: &[u8]) -> bool {
    let mut current = node.parent();
    while let Some(ancestor) = current {
        if matches!(ancestor.kind(), "for_tuple_expr" | "for_object_expr") {
            let mut cursor = ancestor.walk();
            let intro = ancestor
                .named_children(&mut cursor)
                .find(|child| child.kind() == "for_intro");
            if let Some(intro) = intro {
                let mut inner = intro.walk();
                let bound = intro
                    .named_children(&mut inner)
                    .find(|child| child.kind() == "identifier");
                if let Some(bound) = bound {
                    return node_text(bound, source) == name;
                }
            }
        }
        current = ancestor.parent();
    }
    false
}

fn same_target(a: &TargetRef, b: &TargetRef) -> bool {
    match (a, b) {
        (TargetRef::Resolved(x), TargetRef::Resolved(y))
        | (TargetRef::Unresolved(x), TargetRef::Unresolved(y)) => x == y,
        _ => false,
    }
}

fn resolve_traversal(
    segments: &[String],
    declared: &[&str],
    source_path: &str,
    line: u32,
) -> (TargetRef, u32) {
    let full = segments.join(".");
    // Longest reading first: `a.b.c.d`, `a.b.c`, `a.b`, `a`. The rule is
    // the one the project reads names with, so the file that writes a
    // reference and the project that reads it agree on what a name denotes.
    for candidate in readings(&full) {
        if declared.contains(&candidate.as_str()) {
            // Resolved means "this node of this file": the id the graph
            // matches on, not the bare identity. Without the prefix the edge
            // would point at an id that does not exist.
            return (TargetRef::Resolved(tf_id(source_path, &candidate)), line);
        }
    }
    // Unresolved keeps the text of the traversal: it is what a reader needs to
    // know what is missing, and what the resolution stage can still match
    // against another file if there is one.
    (TargetRef::Unresolved(full), line)
}

/// The id of a Terraform block in the graph.
///
/// A single definition: the emitted node and the far end of the edge that
/// points at it have to come from here, or a change of format separates them
/// and the relation is lost in silence.
fn tf_id(source_path: &str, identity: &str) -> String {
    format!("tf:{source_path}:{identity}")
}

/// The text of a tree node.
fn node_text(node: Node, source: &[u8]) -> String {
    String::from_utf8_lossy(&source[node.start_byte()..node.end_byte()]).into_owned()
}

fn line_of(node: Node) -> u32 {
    (node.start_position().row + 1) as u32
}

// ── Clasificación de cabeceras ────────────────────────────────────────────

/// Splits an HCL block header into its keyword and its arguments:
/// `resource "aws_s3_bucket" "app_bucket"` →
/// `("resource", ["aws_s3_bucket", "app_bucket"])`.
///
/// An HCL header has no fixed arity: the only real pattern is "a keyword
/// followed by its arguments", and every consumer decides how many of them
/// it cares about. A function, not five.
///
/// The quotes around HCL literals are removed here, once. Keeping them would
/// contaminate both the label and the identifier of the node.
fn hcl_header(label: &str) -> Option<(&str, Vec<String>)> {
    let mut parts = label.split_whitespace();
    let keyword = parts.next()?;
    Some((
        keyword,
        parts.map(|arg| arg.trim_matches('"').to_string()).collect(),
    ))
}

/// Recognises a Terraform block header and gives it its domain identity.
fn classify<'a>(node: Node<'a>, source: &[u8]) -> Option<Block<'a>> {
    let header = block_header(node, source);
    let (keyword, args) = hcl_header(&header)?;
    let need = |n: usize| args.len() >= n;
    let at = |i: usize| args[i].clone();

    let (identity, kind, iac_type, iac_kind) = match keyword {
        "resource" if need(2) => (
            format!("{}.{}", at(0), at(1)),
            SymbolKind::Function,
            Some(at(0)),
            "resource",
        ),
        "data" if need(2) => (
            format!("data.{}.{}", at(0), at(1)),
            SymbolKind::Variable,
            Some(at(0)),
            "data",
        ),
        "variable" if need(1) => (
            format!("var.{}", at(0)),
            SymbolKind::Variable,
            None,
            "variable",
        ),
        "module" if need(1) => (
            format!("module.{}", at(0)),
            SymbolKind::Module,
            None,
            "module",
        ),
        "output" if need(1) => (
            format!("output.{}", at(0)),
            SymbolKind::Property,
            None,
            "output",
        ),
        // A `terraform { ... }` or `provider "aws" { ... }` block is structure
        // of the configuration, not a resource. No node is invented for them,
        // and their traversals have nobody to be attributed to.
        _ => return None,
    };

    Some(Block {
        identity,
        kind,
        iac_type,
        iac_kind,
        node,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn refs(source: &str) -> Vec<(String, String)> {
        let result = extract_file_for_test(source);
        let mut out: Vec<(String, String)> = result
            .edges
            .iter()
            .filter(|e| e.kind == "dependency.references")
            .map(|e| {
                let target = match &e.target_ref {
                    TargetRef::Resolved(r) | TargetRef::Unresolved(r) => r.clone(),
                };
                let source = result
                    .nodes
                    .iter()
                    .find(|n| n.id.as_str() == e.source)
                    .map(|n| n.label.clone())
                    .unwrap_or_default();
                (source, target)
            })
            .collect();
        out.sort();
        out
    }

    fn extract_file_for_test(source: &str) -> ExtractionResult {
        let bare = ExtractionResult::ok(
            std::path::PathBuf::from("main.tf"),
            "hash".to_string(),
            vec![
                GraphNode::builder(NodeId::new("main.tf"), NodeKind::Symbol(SymbolKind::File))
                    .label("main.tf".to_string())
                    .build(),
            ],
            vec![],
        );
        interpret_terraform("main.tf", "hash", source, &bare)
    }

    #[test]
    fn test_parse_terraform_resource() {
        assert_eq!(
            hcl_header("resource aws_instance web"),
            Some(("resource", vec!["aws_instance".into(), "web".into()]))
        );
        assert_eq!(
            hcl_header("resource \"aws_s3_bucket\" \"app_bucket\""),
            Some((
                "resource",
                vec!["aws_s3_bucket".into(), "app_bucket".into()]
            ))
        );
    }

    /// A traversal that sits in the body of a block becomes a reference.
    ///
    /// Both blocks are declared, and that is what separates the two laws:
    /// here the relation resolves, while the test next to it —where the
    /// reference is not declared— comes out unresolved.
    #[test]
    fn a_traversal_in_a_block_body_becomes_a_reference() {
        assert_eq!(
            refs(concat!(
                "data \"aws_ami\" \"ubuntu\" {\n  most_recent = true\n}\n",
                "resource \"aws_instance\" \"web\" {\n",
                "  ami = data.aws_ami.ubuntu.id\n}\n",
            )),
            vec![(
                "aws_instance.web".to_string(),
                "tf:main.tf:data.aws_ami.ubuntu".to_string()
            )]
        );
    }

    /// The other side of the law: `ami = var.ami_id` names nothing declared
    /// in this file, and comes out unresolved instead of inventing a node.
    #[test]
    fn an_undeclared_traversal_is_unresolved() {
        let result =
            extract_file_for_test("resource \"aws_instance\" \"web\" {\n  ami = var.ami_id\n}\n");
        let edge = result
            .edges
            .iter()
            .find(|e| e.kind == "dependency.references")
            .expect("la traversal existe aunque su destino no: el código la lee");
        assert!(matches!(&edge.target_ref, TargetRef::Unresolved(r) if r == "var.ami_id"));
    }

    /// An HCL file that is not Terraform comes back untouched. The domain
    /// layer only speaks when it has something to say.
    #[test]
    fn a_non_terraform_hcl_file_is_left_alone() {
        let bare = ExtractionResult::ok(
            std::path::PathBuf::from("main.hcl"),
            "hash".to_string(),
            vec![
                GraphNode::builder(NodeId::new("main.hcl"), NodeKind::Symbol(SymbolKind::File))
                    .label("main.hcl".to_string())
                    .build(),
            ],
            vec![],
        );
        let result = interpret_terraform(
            "main.hcl",
            "hash",
            "job \"x\" {\n  group \"y\" {}\n}\n",
            &bare,
        );
        assert_eq!(result.nodes.len(), 1, "solo el nodo de fichero, como entró");
        assert!(result.edges.is_empty());
    }
}
