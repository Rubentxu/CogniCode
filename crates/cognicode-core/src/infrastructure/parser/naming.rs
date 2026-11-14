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

use crate::domain::value_objects::NameRule;

/// The name a declaration carries, read by the language's rule.
///
/// The question is the same for every language — "what is this
/// declaration called?" — and the answer is not: some grammars put the
/// name in a `name` field, some leave it as a child whose kind says so,
/// and some have a declaration with no name of its own at all.
///
/// The rule is the language's answer, and it lives in the domain next to
/// the rest of what a language declares. This is the reader that applies
/// it, and every reader that has to name a declaration comes through here,
/// so the generic extractor and the older parser cannot disagree about
/// what a symbol is called. Two readers answering differently is how a
/// language ends up with two names for one thing.
///
/// The `class_types` are needed only by the languages that have a
/// declaration without a name of its own: it takes the name of the class
/// that contains it, which is how Kotlin calls its secondary constructors
/// and its companion objects.
pub fn declaration_name(
    node: &Node,
    source: &[u8],
    rule: &NameRule,
    class_types: &[&str],
) -> Option<String> {
    if let Some(name) = own_name(node, source, rule) {
        return Some(name);
    }
    rule.inherited
        .iter()
        .find(|(kind, _)| *kind == node.kind())
        .and_then(|(_, suffix)| enclosing_class_name(node, source, rule, class_types, suffix))
}

/// The name a declaration carries by itself.
///
/// A field the grammar declares is where the grammar puts the name. When
/// there is no such field, the name is a child whose kind says so, and the
/// kinds that may sit in front of it —modifiers, type parameters, a
/// receiver— do not end the search for it.
///
/// The header is a LEADING RUN of name-bearing children, and it stops at
/// the first child that is not one, because that is where the body
/// starts. This is not a rule for HCL: it is the rule that keeps a YAML
/// `block_mapping` (whose children are `pair`) unnamed, as it was before.
///
/// It is a rule because the first alternative was wrong. "The first child
/// that is an identifier" names a header grammar by its keyword: HCL's
/// `block` is `identifier(string_lit|identifier)* block_start body?`, has
/// no `name` field, and its first identifier is the word "resource". Over
/// the real fixture `sandbox/fixtures/terraform-iac/main.tf` that
/// produced 144 labels all equal to "resource", which destroyed the type
/// and the name before the Terraform handler could see them and made every
/// block in the file share one identity.
fn own_name(node: &Node, source: &[u8], rule: &NameRule) -> Option<String> {
    if let Some(name) = swallowed_declaration_name(node) {
        return Some(node_text(&name, source));
    }
    if let Some(name_node) = node.child_by_field_name("name") {
        return Some(node_text(&name_node, source));
    }

    let mut header: Vec<String> = Vec::new();
    for i in 0..node.child_count() {
        let Some(child) = node.child(i) else { break };
        if !child.is_named() {
            continue;
        }
        let kind = child.kind();
        if rule.kinds.contains(&kind) {
            header.push(node_text(&child, source));
        } else if rule.prefixes.contains(&kind) {
            continue;
        } else {
            break;
        }
    }

    if header.is_empty() {
        None
    } else {
        Some(header.join(" "))
    }
}

/// The name of a callable the grammar swallowed into the declaration before
/// it.
///
/// The Groovy grammar does not end a statement at a line break, so an untyped
/// field followed by a constructor reads as one declaration:
///
/// ```groovy
/// def steps          // type `def`, name `steps`
/// Kube(steps) { }    // an ERROR holding `Kube`, then the parameters
/// ```
///
/// The shape is narrow on purpose: the child right after the `name` field is
/// an `ERROR` holding exactly one identifier, and the next sibling is the
/// parameter list. Only then is the identifier the callable's name, and what
/// came before it a separate field. Anything else keeps the grammar's name.
pub fn swallowed_declaration_name<'tree>(node: &Node<'tree>) -> Option<Node<'tree>> {
    let name = node.child_by_field_name("name")?;
    let stray = name.next_sibling()?;
    if stray.kind() != "ERROR" || !stray.next_sibling()?.kind().ends_with("parameters") {
        return None;
    }
    let mut cursor = stray.walk();
    let mut inside = stray.named_children(&mut cursor);
    let only = inside.next()?;
    (only.kind() == "identifier" && inside.next().is_none()).then_some(only)
}

/// The name of the class that contains this node, with the suffix its
/// language appends to it.
fn enclosing_class_name(
    node: &Node,
    source: &[u8],
    rule: &NameRule,
    class_types: &[&str],
    suffix: &str,
) -> Option<String> {
    let mut current = node.parent();
    while let Some(parent) = current {
        if class_types.contains(&parent.kind()) {
            let name = own_name(&parent, source, rule)?;
            return Some(format!("{name}{suffix}"));
        }
        current = parent.parent();
    }
    None
}

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

/// The type a call is qualified by, when the path says one.
///
/// [`callee_name`] keeps the last segment of the path, so `Duration::from_millis`
/// and `EventTime::from_millis` are both `from_millis`. What the call names is
/// then no longer enough to say WHOSE `from_millis` it is, and a resolver that
/// finds one declaration of that name in the project joins every call to it,
/// the standard library's included. The segment before the name is what tells
/// the two apart, so it is read here, once, from the same node.
///
/// Only a path that spells a type is a qualifier: `Type::name`,
/// `a::b::Type::name`, `Type::<T>::name`. A value (`x.name`, `make().name`),
/// a qualified-trait path (`<T as Trait>::name`) and a path with no
/// qualifier at all have none, and neither does a module path (`module::name`),
/// `Self` or a one-letter generic parameter: those name something the project
/// declares or something that cannot be judged from the text.
///
/// The capital letter is the convention of the language that asks (see
/// `Language::spells_types_with_capitals`); a caller of another language does
/// not ask.
pub fn callee_type_qualifier(node: &Node, source: &[u8], callee_fields: &[&str]) -> Option<String> {
    let callee = callee_node(node, callee_fields)?;
    type_qualifier_of(&node_text(&callee, source))
}

/// The type qualifier of a callee as written. See [`callee_type_qualifier`].
pub fn type_qualifier_of(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.starts_with('<') {
        return None;
    }
    let path = without_generic_arguments(raw);
    let mut segments: Vec<&str> = path.split("::").collect();
    // A path may start at the root of the crate graph: `::std::time::Duration`.
    if segments.first() == Some(&"") {
        segments.remove(0);
    }
    let is_identifier = |segment: &&str| {
        !segment.is_empty() && segment.chars().all(|c| c.is_alphanumeric() || c == '_')
    };
    if segments.len() < 2 || !segments.iter().all(is_identifier) {
        return None;
    }
    let qualifier = segments[segments.len() - 2];
    is_type_name(qualifier).then(|| qualifier.to_string())
}

/// A path with its generic arguments removed: `Vec::<u8>::new` is `Vec::new`.
///
/// The `::` that introduces an argument list goes with it, so what is left
/// is the path as a reader would write it without turbofish.
fn without_generic_arguments(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut depth = 0usize;
    for c in raw.chars() {
        match c {
            '<' => {
                if depth == 0 && out.ends_with("::") {
                    out.truncate(out.len() - 2);
                }
                depth += 1;
            }
            '>' if depth > 0 => depth -= 1,
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out
}

/// Whether a path segment is spelled as the name of a type: it starts with a
/// capital letter, it is not `Self` and it is longer than one letter.
///
/// One letter is how a generic parameter is written (`T::default()`), and a
/// parameter is a type the project does not declare because it is not a
/// declaration at all. `Self` is the enclosing type, which the project does
/// declare.
fn is_type_name(segment: &str) -> bool {
    segment != "Self"
        && segment.chars().count() > 1
        && segment.chars().next().is_some_and(char::is_uppercase)
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

#[cfg(test)]
mod tests {
    use super::type_qualifier_of;

    fn qualifier(raw: &str) -> Option<String> {
        type_qualifier_of(raw)
    }

    #[test]
    fn a_type_path_names_its_type() {
        assert_eq!(
            qualifier("Duration::from_millis").as_deref(),
            Some("Duration")
        );
        assert_eq!(
            qualifier("std::time::Duration::from_millis").as_deref(),
            Some("Duration")
        );
        assert_eq!(
            qualifier("::std::time::Duration::from_millis").as_deref(),
            Some("Duration")
        );
    }

    #[test]
    fn the_generic_arguments_of_a_path_are_not_a_segment() {
        assert_eq!(qualifier("Vec::<u8>::new").as_deref(), Some("Vec"));
        assert_eq!(
            qualifier("HashMap::<String, Vec<u8>>::with_capacity").as_deref(),
            Some("HashMap")
        );
    }

    #[test]
    fn a_module_path_has_no_type_qualifier() {
        assert_eq!(qualifier("event::build"), None);
        assert_eq!(qualifier("crate::nested::callee"), None);
        assert_eq!(qualifier("super::helper"), None);
    }

    #[test]
    fn self_and_a_generic_parameter_are_not_judged() {
        assert_eq!(qualifier("Self::from_millis"), None);
        assert_eq!(qualifier("T::default"), None);
    }

    #[test]
    fn a_value_has_no_type_qualifier() {
        assert_eq!(qualifier("x.method"), None);
        assert_eq!(qualifier("self.items.push"), None);
        assert_eq!(qualifier("Foo::new().bar"), None);
        assert_eq!(qualifier("make().Foo::bar"), None);
    }

    #[test]
    fn a_qualified_trait_path_is_not_judged() {
        assert_eq!(qualifier("<Foo as Trait>::method"), None);
        assert_eq!(qualifier("<Foo>::method"), None);
    }

    #[test]
    fn a_name_without_a_path_has_no_qualifier() {
        assert_eq!(qualifier("helper"), None);
        assert_eq!(qualifier(""), None);
        assert_eq!(qualifier("::helper"), None);
    }

    #[test]
    fn a_path_that_is_not_made_of_identifiers_is_not_judged() {
        assert_eq!(qualifier("Foo::"), None);
        assert_eq!(qualifier("Foo::::bar"), None);
        assert_eq!(qualifier("Foo :: bar"), None);
    }
}
