//! The types a file constructs.
//!
//! Constructing a value of a type is calling the type's constructor, and
//! Rust writes that constructor as the name of the type: `let page = Html;`
//! constructs an `Html`, and so does `let page = Html { escape: true };`. A
//! reader that reads only calls sees the construction of a tuple struct —
//! `Html(..)` is a call expression— and misses the other two, and a type
//! whose value is built is a type whose change reaches the code that builds
//! it.
//!
//! The rule is not "the unit struct": the value of a unit struct and the
//! literal of a struct are two shapes of the same fact, and the file does
//! not have to say which one it is writing. The rule is that a value
//! written where a value is expected, and written with the name of a type
//! the file binds, constructs that type.
//!
//! The rule lives here, and not in the reader that applies it: two readers
//! answer "what does this file use", and a rule each of them wrote for
//! itself is a rule that answers two questions.
//!
//! The rule says which constructions are written in a body. How each reader
//! publishes the edge it found is the reader's own business.

use std::collections::HashSet;

use tree_sitter::Node;

/// A construction written in a body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Construction {
    /// The name of the type that is constructed, as it is written.
    pub name: String,
    /// The line the construction is written on, counted from one.
    pub line: u32,
}

/// The constructions written under `node`, and under every node below it.
///
/// A construction is a value of a type written where a value is expected:
/// the value of a `let`. The name it writes has to be one the file binds to
/// a type —a type the file declares, or a name the file imports— because a
/// name the file does not bind is a local, and a local is not a type.
pub fn constructions_in(node: &Node, source: &[u8], types: &HashSet<String>) -> Vec<Construction> {
    // The names the body binds to a local, and the values it builds. The
    // two are read in one pass: the body is walked once, and a walk per
    // question is a walk the reader pays on every file.
    let mut locals = HashSet::new();
    let mut values = Vec::new();
    let mut stack = vec![*node];
    while let Some(current) = stack.pop() {
        match current.kind() {
            "let_declaration" => {
                if let Some(pattern) = current.child_by_field_name("pattern") {
                    locals.insert(local_name(&node_text(&pattern, source)));
                }
                if let Some(value) = current.child_by_field_name("value") {
                    values.push(value);
                }
            }
            "parameter" | "self_parameter" => {
                if let Some(pattern) = current.child_by_field_name("pattern") {
                    locals.insert(local_name(&node_text(&pattern, source)));
                }
            }
            _ => {}
        }
        let mut cursor = current.walk();
        stack.extend(current.children(&mut cursor));
    }

    values
        .into_iter()
        .filter_map(|value| {
            let name = constructed_type(&value, source, types)?;
            // A local shadows the name it carries: a `let` written with the
            // name of an imported type binds a value of that name, and the
            // value is not the type.
            if locals.contains(&name) {
                return None;
            }
            Some(Construction {
                name,
                line: value.start_position().row as u32 + 1,
            })
        })
        .collect()
}

/// The type a value constructs, when the value constructs one.
///
/// The three shapes that construct a value are the name of a unit struct,
/// the literal of a struct, and a reference to either of them.
fn constructed_type(value: &Node, source: &[u8], types: &HashSet<String>) -> Option<String> {
    match value.kind() {
        // `Html` — the value of a unit struct.
        "identifier" => {
            let name = node_text(value, source);
            types.contains(&name).then_some(name)
        }
        // `Html { escape: true }` — the value of a struct.
        "struct_expression" => {
            let name = value
                .child_by_field_name("name")
                .map(|name| type_name_of(&node_text(&name, source)))?;
            types.contains(&name).then_some(name)
        }
        // `&Html` — a reference to the value of a unit struct.
        "reference_expression" => value
            .child_by_field_name("value")
            .and_then(|inner| constructed_type(&inner, source, types)),
        _ => None,
    }
}

/// The name a pattern binds, without the words that qualify it.
///
/// `mut page` binds `page`, and `&page` binds `page`: what a pattern binds
/// is the name, and the rest of the text says how it is held.
fn local_name(pattern: &str) -> String {
    pattern
        .trim()
        .trim_start_matches("mut ")
        .trim_start_matches("ref ")
        .trim_start_matches('&')
        .trim_start_matches('*')
        .trim()
        .to_string()
}

/// The name of a type, without its path or its arguments.
///
/// `a::Html`, `Html<T>` and `Html::<T>` all name `Html`: a relation hangs
/// on the declaration, and the declaration is named by the last segment of
/// the path.
fn type_name_of(text: &str) -> String {
    let head = text
        .split(|c: char| c == '<' || c == '(' || c.is_whitespace() || c == ',')
        .next()
        .unwrap_or("")
        .trim()
        .trim_end_matches(':');
    head.rsplit("::").next().unwrap_or("").trim().to_string()
}

/// The text of a node, as it is written.
fn node_text(node: &Node, source: &[u8]) -> String {
    let bytes = node.byte_range();
    String::from_utf8_lossy(&source[bytes]).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tree_sitter::Parser;

    /// The constructions of a Rust body, read with the types it binds.
    fn constructions_of(source: &str, types: &[&str]) -> Vec<Construction> {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .expect("the grammar loads");
        let tree = parser
            .parse(source.as_bytes(), None)
            .expect("the body parses");
        let types: HashSet<String> = types.iter().map(|name| name.to_string()).collect();
        constructions_in(&tree.root_node(), source.as_bytes(), &types)
    }

    /// The name of the value of a unit struct is a construction of that
    /// struct.
    #[test]
    fn the_value_of_a_unit_struct_is_a_construction() {
        let found = constructions_of("fn f() { let page = Html; }", &["Html"]);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].name, "Html");
        assert_eq!(found[0].line, 1);
    }

    /// The literal of a struct is a construction of that struct.
    #[test]
    fn the_literal_of_a_struct_is_a_construction() {
        let found = constructions_of("fn f() { let page = Html { escape: true }; }", &["Html"]);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].name, "Html");
    }

    /// A name the file does not bind to a type is a local, and a local is
    /// not a type.
    #[test]
    fn a_name_the_file_does_not_bind_is_not_a_construction() {
        let found = constructions_of("fn f() { let page = other; }", &["Html"]);
        assert!(found.is_empty(), "{found:?}");
    }

    /// A local shadows the name of an imported type.
    #[test]
    fn a_local_shadows_the_name_of_a_type() {
        let found = constructions_of("fn f() { let Html = 1; let page = Html; }", &["Html"]);
        assert!(found.is_empty(), "{found:?}");
    }

    /// A value built by a call is a call, and the reader that reads calls
    /// already sees it.
    #[test]
    fn a_value_built_by_a_call_is_not_a_construction() {
        let found = constructions_of("fn f() { let page = Html::new(); }", &["Html"]);
        assert!(found.is_empty(), "{found:?}");
    }

    /// A reference to the value of a unit struct constructs it too.
    #[test]
    fn a_reference_to_the_value_of_a_unit_struct_is_a_construction() {
        let found = constructions_of("fn f() { let page = &Html; }", &["Html"]);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].name, "Html");
    }
}
