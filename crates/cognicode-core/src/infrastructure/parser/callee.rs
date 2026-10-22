//! Reading the callee of a call, the way the grammar declares it.
//!
//! A call node holds the name of what it calls, but where it holds it is
//! the grammar's business and not the reader's: one grammar calls the field
//! `function`, another `method`, another `name`, another `target`, another
//! `expr`, and one grammar has no field at all and keeps the name in its
//! first named child.
//!
//! The reader therefore asks the node, not the language: it tries the fields
//! the language declares and falls back to the first child that is not the
//! arguments. A boolean that said "this language has a function field" was
//! the same question asked with the wrong tool — it could only answer yes
//! or no to a question whose answer is a name.
//!
//! One implementation, used by every reader that has to name a callee: the
//! generic walker, the parser that predates it and the refactor strategies
//! all answer the same question about the same node the same way.

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
    if cleaned.is_empty() { None } else { Some(cleaned) }
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

/// Whether a child holds the arguments of a call rather than its callee.
fn is_arguments(kind: &str) -> bool {
    kind.ends_with("arguments") || kind.ends_with("argument_list")
}

/// The text of a node.
fn node_text(node: &Node, source: &[u8]) -> String {
    String::from_utf8_lossy(&source[node.start_byte()..node.end_byte()]).into_owned()
}
