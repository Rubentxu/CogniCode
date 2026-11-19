//! The calls written inside the body of a macro.
//!
//! A grammar does not parse the body of a macro as expressions: it hands it
//! over as a token tree, and the reader has to read it as it finds it. A
//! call written in it is a name whose next sibling is the list of arguments;
//! a macro invoked inside the body puts a `!` between its name and its own
//! body, so it is not mistaken for a call; and the body of a macro
//! definition is expanded wherever the macro is used, so it is not a call
//! site at all.
//!
//! The rule lives here, and not in the reader that applies it: two readers
//! answer "what does this file call", and a rule each of them wrote for
//! itself is a rule that answers two questions.
//!
//! The rule says which calls are written in a body. How each reader
//! publishes the edge it found is the reader's own business: the reader
//! that publishes facts says that a call recognised by shape is weaker
//! than a call read out of code, and the reader that publishes a call
//! site has no field to say it with.

use tree_sitter::Node;

use crate::domain::value_objects::MacroCallShape;

/// A call written in the body of a macro.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroCall {
    /// The text the call is written with, as the body spells it: `reload`
    /// in `assert_eq!(reload("a=1"), 1)`, `foo::bar` in
    /// `assert_eq!(foo::bar(), 1)`.
    pub text: String,
    /// The line the call is written on, counting from one.
    pub line: u32,
}

/// The calls written in the body of the macro invocation `macro_node`.
///
/// The body of the invocation is the node that holds it: a grammar that
/// hands a macro body over as a token tree names it as one.
pub fn calls_in_body(macro_node: &Node, source: &[u8], shape: &MacroCallShape) -> Vec<MacroCall> {
    let mut calls = Vec::new();
    let mut cursor = macro_node.walk();
    let children: Vec<Node> = macro_node.children(&mut cursor).collect();
    for child in children {
        if shape.argument_types.contains(&child.kind()) {
            collect_calls(&child, source, shape, &mut calls);
        }
    }
    calls
}

/// The calls written under `node`, and under every node below it.
fn collect_calls(node: &Node, source: &[u8], shape: &MacroCallShape, calls: &mut Vec<MacroCall>) {
    let mut cursor = node.walk();
    let children: Vec<Node> = node.children(&mut cursor).collect();
    for index in 0..children.len() {
        let child = &children[index];
        if shape.callee_types.contains(&child.kind())
            && let Some(arguments) = children.get(index + 1)
            && shape.argument_types.contains(&arguments.kind())
            && opens_with_paren(arguments)
        {
            calls.push(MacroCall {
                text: callee_text(&children, index, source, shape),
                line: child.start_position().row as u32 + 1,
            });
        }
        collect_calls(child, source, shape, calls);
    }
}

/// The text a callee is written with, as the body spells it.
///
/// The name of the callee is the last segment of the path it is written
/// through: `foo::bar` names `bar` and qualifies it with `foo`. The text is
/// read whole, because the reader that resolves a call reads the same text
/// of a call written in code, and a text read twice is a text read two
/// ways.
fn callee_text(children: &[Node], index: usize, source: &[u8], shape: &MacroCallShape) -> String {
    let mut parts = vec![node_text(&children[index], source)];
    let mut separators: Vec<&str> = Vec::new();
    let mut cursor = index;
    while cursor >= 2 {
        let separator = children[cursor - 1].kind();
        if separator != "." && separator != "::" {
            break;
        }
        // Only a name is a segment of the path: anything else before the
        // separator is not part of what the call names.
        let segment = &children[cursor - 2];
        if !shape.callee_types.contains(&segment.kind()) {
            break;
        }
        separators.push(separator);
        parts.push(node_text(segment, source));
        cursor -= 2;
    }
    let mut text = parts[0].clone();
    for (position, separator) in separators.iter().enumerate().rev() {
        text = format!("{}{separator}{text}", parts[position + 1]);
    }
    text
}

/// Whether the list of arguments opens with a parenthesis. A list that
/// opens with a brace is a block, not a call.
fn opens_with_paren(node: &Node) -> bool {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .next()
        .is_some_and(|first| first.kind() == "(")
}

/// The text of a node, as it is written.
fn node_text(node: &Node, source: &[u8]) -> String {
    let bytes = node.byte_range();
    String::from_utf8_lossy(&source[bytes]).to_string()
}
