//! Reading the name a node carries.
//!
//! A tree-sitter node holds a name in different places depending on the
//! grammar: a call names what it calls in a field the grammar declares, and a
//! definition names itself in a child whose kind says so. Two questions, one
//! answer each, and both answers belong to one place.
//!
//! The reader therefore asks the node, not the language: it tries the fields
//! the language declares, falls back to the first child that is not the
//! arguments, and reads the identifier inside. A boolean that said "this
//! language has a function field" was the same question asked with the wrong
//! tool — it could only answer yes or no to a question whose answer is a name.
//!
//! One implementation, used by every reader that has to name something: the
//! generic walker, the parser that predates it, the refactor strategies, the
//! complexity calculator and the tool handlers all answer the same question
//! about the same node the same way.

use tree_sitter::Node;

/// The node that holds the name of what a call calls.
///
/// The name is read from the field the grammar declares for it, because that
/// is where the grammar puts it. A grammar that declares no such field keeps
/// the name in its first named child, and the arguments are not it.
pub fn callee_node<'tree>(node: &Node<'tree>, callee_fields: &[&str]) -> Option<Node<'tree>> {
    for field in callee_fields {
        if let Some(child) = node.child_by_field_name(field) {
            return Some(child);
        }
    }
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find(|child| !is_arguments(child.kind()))
}

/// The name of the thing a call calls.
///
/// The last segment of the name is what is published: a call names the thing
/// it calls, not the path that reaches it. `crate::nested::callee()` calls
/// `callee`, and `obj.method()` calls `method`.
pub fn callee_name(node: &Node, source: &[u8], callee_fields: &[&str]) -> Option<String> {
    let callee = callee_node(node, callee_fields)?;
    let cleaned = clean_callee_name(&node_text(&callee, source));
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned)
    }
}

/// The last segment of a callee's name.
pub fn clean_callee_name(raw: &str) -> String {
    let cleaned = raw
        .split("::")
        .last()
        .unwrap_or(raw)
        .split('.')
        .next_back()
        .unwrap_or(raw);
    cleaned.trim().to_string()
}

/// The name a node carries.
///
/// A node that is an identifier names itself. Otherwise the name is the
/// first identifier inside it, and a type identifier only counts when the
/// node carries no plain identifier: a path like `Type::method` names the
/// method, not the type.
pub fn identifier_text(node: &Node, source: &[u8]) -> Option<String> {
    if node.kind() == "identifier" {
        return Some(node_text(node, source));
    }
    first_identifier(node, source, "identifier")
        .or_else(|| first_identifier(node, source, "type_identifier"))
}

/// The first identifier of the given kind inside a node.
fn first_identifier(node: &Node, source: &[u8], kind: &str) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() == kind {
            return Some(node_text(&child, source));
        }
        if let Some(found) = first_identifier(&child, source, kind) {
            return Some(found);
        }
    }
    None
}

/// Whether a child holds the arguments of a call rather than its callee.
fn is_arguments(kind: &str) -> bool {
    kind.ends_with("arguments") || kind.ends_with("argument_list")
}

/// The text of a node.
fn node_text(node: &Node, source: &[u8]) -> String {
    String::from_utf8_lossy(&source[node.start_byte()..node.end_byte()]).into_owned()
}
