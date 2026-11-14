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

use std::collections::{HashMap, HashSet};

use tree_sitter::Node;

use crate::domain::value_objects::{NameRule, Receiver};

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

/// The node that carries the name of a declaration.
///
/// The rule is the one [`own_name`] applies, and it answers with the node
/// instead of with its text: a reader that has to know WHICH node writes
/// the name —the occurrence search, to tell a declaration from a use— asks
/// here, so the two readers cannot disagree about where a name is written.
///
/// A header that names more than one thing (HCL) has one node per name.
/// The first of them is the one this returns, and it is the one a reader
/// can match an occurrence against.
pub fn declaration_name_node<'tree>(node: &Node<'tree>, rule: &NameRule) -> Option<Node<'tree>> {
    if let Some(name) = swallowed_declaration_name(node) {
        return Some(name);
    }
    if let Some(name) = node.child_by_field_name("name") {
        return Some(name);
    }

    for i in 0..node.child_count() {
        let Some(child) = node.child(i) else { break };
        if !child.is_named() {
            continue;
        }
        let kind = child.kind();
        if rule.kinds.contains(&kind) {
            return Some(child);
        } else if rule.prefixes.contains(&kind) {
            continue;
        } else {
            break;
        }
    }

    None
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
    let cleaned = clean_callee_name(&callee_text(node, source, callee_fields)?);
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
    type_qualifier_of(&callee_text(node, source, callee_fields)?)
}

/// The type-like names a Rust file binds by itself.
///
/// A type qualifier is judged foreign when the project declares no type of
/// that name. A file can bind a name the project never declares and still
/// mean one of the project's types by it:
///
/// - `use event::EventTime as Et;` — `Et` is another spelling of `EventTime`;
/// - `type Moment = EventTime;` — the same, by an alias;
/// - `fn make<Time: Clock>() { Time::now() }` — `Time` is whatever the caller
///   chose, the project's own type included;
/// - `trait Source { type Item; }` — `Item` is the implementor's type.
///
/// Judging those would lose a true edge to avoid a false one, so the parser
/// does not publish them as qualifiers: the call keeps the bare-name answer
/// it had before qualifiers were read. The names are collected from the whole
/// file, not by scope: a name bound anywhere in the file is not judged
/// anywhere in it, which can only keep an edge, never invent one.
pub fn type_names_bound_in(root: Node, source: &[u8]) -> HashSet<String> {
    let mut bound = HashSet::new();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        let name_field = match node.kind() {
            "use_as_clause" => Some("alias"),
            "type_item" | "associated_type" | "type_parameter" => Some("name"),
            _ => None,
        };
        if let Some(name) = name_field
            .and_then(|field| node.child_by_field_name(field))
            .map(|name| node_text(&name, source))
        {
            bound.insert(name);
        }
        let mut cursor = node.walk();
        stack.extend(node.children(&mut cursor));
    }
    bound
}

/// The names a file binds to something of its own.
///
/// A module the file declares (`pub mod log;`) and a name it imports
/// (`use super::log;`) are the file's own: a path that starts at one of them
/// names the project's own thing, even when a crate from outside carries the
/// same name. In Rust 2018 an item declared in the current module shadows
/// the crate of the same name, so `log::info()` written in a module that
/// declares `pub mod log;` is the project's own call.
///
/// The names are collected from the whole file, not by scope: a name bound
/// anywhere in the file is not judged anywhere in it, which can only keep an
/// edge, never invent one.
pub fn own_names_bound_in(root: Node, source: &[u8]) -> HashSet<String> {
    let mut bound = HashSet::new();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        match node.kind() {
            "mod_item" => {
                if let Some(name) = node.child_by_field_name("name") {
                    bound.insert(node_text(&name, source));
                }
            }
            "use_declaration" => {
                if let Some(argument) = node.child_by_field_name("argument") {
                    use_bound_names(&argument, source, &mut bound);
                }
            }
            _ => {}
        }
        let mut cursor = node.walk();
        stack.extend(node.children(&mut cursor));
    }
    bound
}

/// The names a `use` declaration binds.
///
/// A `use` binds the name it is written with: the last segment of the path
/// (`use super::log;` binds `log`) or the alias it renames it with
/// (`use super::log as trace;` binds `trace`).
fn use_bound_names(node: &Node, source: &[u8], bound: &mut HashSet<String>) {
    match node.kind() {
        "scoped_identifier" => {
            if let Some(name) = node.child_by_field_name("name") {
                bound.insert(node_text(&name, source));
            }
        }
        "use_as_clause" => {
            if let Some(alias) = node.child_by_field_name("alias") {
                bound.insert(node_text(&alias, source));
            }
        }
        "use_list" => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                use_bound_names(&child, source, bound);
            }
        }
        "scoped_use_list" => {
            if let Some(list) = node.child_by_field_name("list") {
                use_bound_names(&list, source, bound);
            }
        }
        _ => {}
    }
}

/// The crate the names a file imports come from.
///
/// A file that writes `use serde_json::to_value;` and then calls
/// `to_value(..)` calls the crate's function, not a symbol of the project
/// that happens to have the same name: the import shadows the name in the
/// module it is written in. The head of the path the import is written
/// through is the crate, and it is the only place the file says where the
/// name comes from.
///
/// The imports are read per module, because a `use` binds its name in the
/// module it is written in, and a module does not inherit the imports of
/// the module that holds it. The walk reads the modules of the file and the
/// imports of each; it does not go inside a function, because a `use`
/// written in a body binds its name in that body, and a name the reader
/// cannot place keeps the answer it had — which can only keep an edge.
///
/// The shape read here is the shape of a Rust import. A language that
/// writes its imports differently keeps the answer it had.
pub struct ImportOrigins {
    /// The modules of the file, with the crate each name comes from.
    modules: Vec<ModuleOrigins>,
}

/// The names one module binds, and the crate each comes from.
struct ModuleOrigins {
    /// The first line the module covers.
    start: usize,
    /// The last line the module covers.
    end: usize,
    /// The crate each imported name comes from.
    origins: HashMap<String, String>,
}

impl ImportOrigins {
    /// The crate the name comes from, in the module that holds the line.
    ///
    /// `None` when the module does not import the name, and `None` too when
    /// the name comes from the project itself: a path that starts at a root
    /// of the crate graph (`crate`, `self`, `super`) names the project's
    /// own thing, and there is no crate to judge.
    pub fn crate_of(&self, name: &str, line: usize) -> Option<&str> {
        let module = self.module_of(line)?;
        module.origins.get(name).map(String::as_str)
    }

    /// The module that holds the line.
    ///
    /// The innermost one: a module inside a module is the one that holds
    /// the line, and the module that holds it does not bind the names the
    /// inner module binds.
    fn module_of(&self, line: usize) -> Option<&ModuleOrigins> {
        self.modules
            .iter()
            .filter(|module| module.start <= line && line <= module.end)
            .min_by_key(|module| module.end - module.start)
    }
}

/// The crates the names a file imports come from.
///
/// See [`ImportOrigins`] for what the answer is for and how the imports are
/// read.
pub fn import_origins_in(root: Node, source: &[u8], import_types: &[&str]) -> ImportOrigins {
    let mut modules = vec![ModuleOrigins {
        start: root.start_position().row + 1,
        end: root.end_position().row + 1,
        origins: HashMap::new(),
    }];
    // The modules of the file and the imports of each. The stack holds the
    // module an item belongs to and the node whose children are that
    // module's items.
    let mut stack = vec![(0usize, root)];
    while let Some((module, node)) = stack.pop() {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "mod_item" => {
                    // The body of a module is another module.
                    let Some(body) = child.child_by_field_name("body") else {
                        continue;
                    };
                    let index = modules.len();
                    modules.push(ModuleOrigins {
                        start: body.start_position().row + 1,
                        end: body.end_position().row + 1,
                        origins: HashMap::new(),
                    });
                    stack.push((index, body));
                }
                kind if import_types.contains(&kind) => {
                    for (name, origin) in import_bindings(&child, source) {
                        // A path that starts at a root of the crate graph
                        // names the project's own thing, and there is no
                        // crate to judge.
                        if !is_own_root(&origin) {
                            modules[module].origins.insert(name, origin);
                        }
                    }
                }
                _ => {}
            }
        }
    }
    ImportOrigins { modules }
}

/// The names an import binds, and the crate each comes from.
///
/// An import binds the name it is written with: the last segment of the
/// path (`use serde_json::to_value;` binds `to_value`) or the alias it
/// renames it with (`use serde_json::to_value as tv;` binds `tv`). The
/// crate is the head of the path the import is written through.
fn import_bindings(node: &Node, source: &[u8]) -> Vec<(String, String)> {
    let Some(argument) = node.child_by_field_name("argument") else {
        return Vec::new();
    };
    let mut bound = Vec::new();
    collect_import(&argument, source, None, &mut bound);
    bound
}

/// The names an import path binds, and the crate they come from.
///
/// The head of the path is inherited by the names a list binds: in
/// `use serde_json::{to_value, from_value};` both names come from
/// `serde_json`.
fn collect_import(
    node: &Node,
    source: &[u8],
    head: Option<&str>,
    bound: &mut Vec<(String, String)>,
) {
    match node.kind() {
        "scoped_identifier" => {
            let name = node
                .child_by_field_name("name")
                .map(|name| node_text(&name, source));
            let path = node
                .child_by_field_name("path")
                .and_then(|path| path_head(&path, source));
            match (head, path) {
                (Some(head), _) => {
                    if let Some(name) = name {
                        bound.push((name, head.to_string()));
                    }
                }
                (None, Some(path)) => {
                    if let Some(name) = name {
                        bound.push((name, path));
                    }
                }
                (None, None) => {}
            }
        }
        "use_as_clause" => {
            let alias = node
                .child_by_field_name("alias")
                .map(|alias| node_text(&alias, source));
            let path = node
                .child_by_field_name("path")
                .and_then(|path| path_head(&path, source));
            match (head, path) {
                (Some(head), _) => {
                    if let Some(alias) = alias {
                        bound.push((alias, head.to_string()));
                    }
                }
                (None, Some(path)) => {
                    if let Some(alias) = alias {
                        bound.push((alias, path));
                    }
                }
                (None, None) => {}
            }
        }
        "identifier" => {
            // The name of a list: `use serde_json::{to_value, from_value};`
            // binds `to_value` and `from_value`, and both come from the
            // head the list is written through.
            if let Some(head) = head {
                bound.push((node_text(node, source), head.to_string()));
            }
        }
        "use_list" => {
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                collect_import(&child, source, head, bound);
            }
        }
        "scoped_use_list" => {
            let path = node
                .child_by_field_name("path")
                .and_then(|path| path_head(&path, source));
            match (head, path) {
                (Some(head), _) => {
                    if let Some(list) = node.child_by_field_name("list") {
                        collect_import(&list, source, Some(head), bound);
                    }
                }
                (None, Some(path)) => {
                    if let Some(list) = node.child_by_field_name("list") {
                        collect_import(&list, source, Some(&path), bound);
                    }
                }
                (None, None) => {}
            }
        }
        _ => {}
    }
}

/// The head of a path an import is written through.
///
/// The head is the crate: `serde_json` in `use serde_json::to_value;`. A
/// path that starts at a root of the crate graph names the project's own
/// thing, and the root is its head.
fn path_head(node: &Node, source: &[u8]) -> Option<String> {
    match node.kind() {
        "identifier" => Some(node_text(node, source)),
        "crate" | "self" | "super" => Some(node.kind().to_string()),
        "scoped_identifier" => node
            .child_by_field_name("path")
            .and_then(|path| path_head(&path, source)),
        _ => None,
    }
}

/// The roots of the crate graph.
///
/// A path that starts at one of them names the project's own thing: `crate`
/// is the crate itself, `self` the module the path is written in, and
/// `super` the module that holds it.
fn is_own_root(head: &str) -> bool {
    matches!(head, "crate" | "self" | "super")
}

/// The type qualifier of a callee as written. See [`callee_type_qualifier`].
pub fn type_qualifier_of(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.starts_with('<') {
        return None;
    }
    let path = without_generic_arguments(raw);
    let segments = path_segments(&path)?;
    let qualifier = segments[segments.len() - 2];
    is_type_name(qualifier).then(|| qualifier.to_string())
}

/// The text of what a call calls.
///
/// The name, the type qualifier and the head of the path are three readings
/// of the same text. Reading it once and reading it three times cost the
/// same in a file with few calls and three times more in a file with many,
/// and the text of a callee is read for every call of every file of the
/// project.
pub fn callee_text(node: &Node, source: &[u8], callee_fields: &[&str]) -> Option<String> {
    let callee = callee_node(node, callee_fields)?;
    Some(node_text(&callee, source))
}

/// The head of a path, as written.
///
/// A path says who the callee belongs to before it says its name:
/// `serde_json::to_value` and `event::build` both call something called
/// `to_value` or `build`, and only the head tells the project's own module
/// from a crate the project pulls from outside itself.
///
/// A path that starts at a type is not one of those: `Duration::from_millis`
/// starts with a type, and the type qualifier carries that call. `Self` and
/// a one-letter parameter are types the project may not declare at all, and
/// they are not crates either.
///
/// The text of a callee is read once and read three ways — the name, the
/// qualifier and the head — and the helper that read it again is not
/// published:
///
/// ```compile_fail
/// use cognicode_core::infrastructure::parser::naming::callee_path_head;
/// ```
pub fn path_head_of(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.starts_with('<') {
        return None;
    }
    // Generic arguments are not a segment: `Vec::<u8>::new` is `Vec::new`.
    // Only a path that carries them pays for the copy.
    let without_arguments;
    let raw = if raw.contains('<') {
        without_arguments = without_generic_arguments(raw);
        without_arguments.as_str()
    } else {
        raw
    };
    // A path may start at the root of the crate graph: `::std::time::Duration`.
    let raw = raw.strip_prefix("::").unwrap_or(raw);
    let mut segments = raw.split("::");
    let head = segments.next()?;
    let mut rest = 0;
    for segment in segments {
        if !is_identifier(segment) {
            return None;
        }
        rest += 1;
    }
    if rest == 0 || !is_identifier(head) {
        return None;
    }
    is_crate_or_module(head).then(|| head.to_string())
}

/// The segments of a path, when the path is written in segments.
///
/// A path may start at the root of the crate graph (`::std::time::Duration`)
/// and may carry generic arguments (`Vec::<u8>::new`); neither is a segment
/// the caller names.
fn path_segments(path: &str) -> Option<Vec<&str>> {
    let mut segments: Vec<&str> = path.split("::").collect();
    if segments.first() == Some(&"") {
        segments.remove(0);
    }
    if segments.len() < 2 || !segments.iter().all(|segment| is_identifier(segment)) {
        return None;
    }
    Some(segments)
}

/// Whether a path segment is written as an identifier.
fn is_identifier(segment: &str) -> bool {
    !segment.is_empty() && segment.chars().all(|c| c.is_alphanumeric() || c == '_')
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

/// Whether a path segment names a crate or a module.
///
/// A crate or a module is written in lower case, and it is never `Self` and
/// never a single letter: those are a type and a type parameter, and the
/// type qualifier is what carries them.
fn is_crate_or_module(segment: &str) -> bool {
    segment != "Self"
        && segment.chars().count() > 1
        && segment.chars().next().is_some_and(|c| !c.is_uppercase())
}

/// The last segment of a callee's name.
///
/// A turbofish is not a name: `parse::<u32>` calls `parse`, and the
/// generic arguments are not the callee. Without this the name of
/// `x.parse::<u32>()` is published as `<u32>`, which no declaration can
/// ever carry, and the call is counted as unresolved even when the
/// project declares the method.
pub fn clean_callee_name(raw: &str) -> String {
    // Only a path that carries them pays for the copy.
    let without_arguments;
    let raw = if raw.contains('<') {
        without_arguments = without_generic_arguments(raw);
        without_arguments.as_str()
    } else {
        raw
    };
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

/// The receiver of a call, when the call is written on a value.
///
/// The text of a call written on a value is the receiver, a dot, and the
/// name: `bytes.as_bytes` names `as_bytes` on `bytes`. The receiver is
/// what is written before the last dot, and a call written through a path
/// has no receiver at all: `serde_json::to_value` names what the path
/// names.
pub fn receiver_of(raw: &str) -> Option<&str> {
    let dot = raw.rfind('.')?;
    let receiver = raw[..dot].trim();
    if receiver.is_empty() {
        return None;
    }
    Some(receiver)
}

/// What a file says about the receivers of its calls.
///
/// A call written on a value names a method of the receiver's type, and the
/// type is the file's to say. The file says it in four ways:
///
/// - `self` is the type of the `impl` that holds it;
/// - a parameter or a local that declares a type is that type;
/// - a constructor is the type it constructs (`Workspace::new()`);
/// - a type the file names in a path is named with the crate it comes from.
///
/// What the file cannot say, this does not say either: a receiver that is a
/// field, or the result of a call, has no type here, and the call keeps the
/// answer it had.
pub struct Receivers {
    /// The `impl` blocks of the file, the type each one holds, and the
    /// lines it holds them on.
    impls: Vec<ImplType>,
    /// The names the file binds to a type, and the scope each binding is
    /// written in.
    ///
    /// The bindings are indexed by the name they bind: a call asks about one
    /// name, and a file that binds the same name in many scopes would
    /// otherwise be walked once per call.
    bindings: HashMap<String, Vec<Binding>>,
    /// The generic parameters the file declares.
    generics: HashSet<String>,
    /// The types the file declares.
    declared: HashSet<String>,
    /// The names the file imports, and the crate each comes from.
    imported: HashMap<String, String>,
}

/// An `impl` block, the type it holds, and the lines it holds them on.
struct ImplType {
    /// The first line of the block.
    start: usize,
    /// The last line of the block.
    end: usize,
    /// The type the block holds.
    name: String,
}

/// A name the file binds to a type.
struct Binding {
    /// The type the binding holds.
    type_name: String,
    /// The lines of the function the binding is written in.
    scope: (usize, usize),
    /// The line the binding is written on.
    at: usize,
}

impl Receivers {
    /// What the file says about the receiver written on this line.
    ///
    /// The answer is the file's, and it is the same for every route that
    /// asks about the same call: the judgment of a call is not the
    /// judgment of a scope.
    pub fn of(&self, receiver: &str, line: usize) -> Receiver {
        let Some((name, origin)) = self.type_of(receiver, line) else {
            return Receiver::Uncertain;
        };
        self.classify(&name, origin.as_deref())
    }

    /// The type the receiver has, and the crate it comes from.
    fn type_of(&self, receiver: &str, line: usize) -> Option<(String, Option<String>)> {
        if receiver == "self" {
            let block = self
                .impls
                .iter()
                .filter(|block| block.start <= line && line <= block.end)
                .max_by_key(|block| block.end - block.start)?;
            return Some((block.name.clone(), None));
        }
        if is_identifier(receiver) {
            let binding = self
                .bindings
                .get(receiver)?
                .iter()
                .filter(|binding| binding.scope.0 <= line && line <= binding.scope.1)
                .max_by_key(|binding| binding.at)?;
            return type_and_origin(&binding.type_name);
        }
        // A constructor: `Workspace::new()` is a value of the type it
        // constructs.
        let text = receiver.strip_suffix("()").unwrap_or(receiver);
        let segments: Vec<&str> = text
            .split("::")
            .map(str::trim)
            .filter(|segment| !segment.is_empty())
            .collect();
        if segments.len() < 2 {
            return None;
        }
        let name = segments[segments.len() - 2];
        if !is_type_name(name) {
            return None;
        }
        let origin = match segments.len() {
            2 => None,
            _ => Some(segments[0].to_string()),
        };
        Some((name.to_string(), origin))
    }

    /// What the file says about a type, and the crate it comes from.
    fn classify(&self, name: &str, origin: Option<&str>) -> Receiver {
        if let Some(origin) = origin {
            return if is_own_root(origin) {
                Receiver::Project
            } else {
                Receiver::Crate(origin.to_string())
            };
        }
        if self.declared.contains(name) {
            return Receiver::Project;
        }
        if self.generics.contains(name) {
            // The type is whatever the caller chose, and the file does not
            // say whose: the call keeps the answer it had.
            return Receiver::Uncertain;
        }
        match self.imported.get(name) {
            Some(origin) if is_own_root(origin) => Receiver::Project,
            Some(origin) => Receiver::Crate(origin.clone()),
            None => Receiver::Unbound,
        }
    }
}

/// The receivers of the calls of a file, and what the file says about them.
pub fn receivers_in(root: Node, source: &[u8], import_types: &[&str]) -> Receivers {
    let mut receivers = Receivers {
        impls: Vec::new(),
        bindings: HashMap::new(),
        generics: HashSet::new(),
        declared: HashSet::new(),
        imported: HashMap::new(),
    };
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        match node.kind() {
            "impl_item" => {
                if let Some(type_node) = node.child_by_field_name("type")
                    && let Some((name, _)) = type_and_origin(&node_text(&type_node, source))
                {
                    receivers.impls.push(ImplType {
                        start: node.start_position().row + 1,
                        end: node.end_position().row + 1,
                        name,
                    });
                }
            }
            "struct_item" | "enum_item" | "trait_item" | "union_item" | "type_item" => {
                if let Some(name) = node.child_by_field_name("name") {
                    receivers.declared.insert(node_text(&name, source));
                }
            }
            "type_parameter" => {
                if let Some(name) = node.child_by_field_name("name") {
                    receivers.generics.insert(node_text(&name, source));
                }
            }
            "parameter" => {
                let name = node
                    .child_by_field_name("pattern")
                    .map(|pattern| node_text(&pattern, source))
                    .unwrap_or_default();
                let type_name = node
                    .child_by_field_name("type")
                    .map(|type_node| node_text(&type_node, source))
                    .unwrap_or_default();
                if let Some(scope) = enclosing_function(&node) {
                    receivers.bindings.entry(name).or_default().push(Binding {
                        type_name,
                        scope,
                        at: node.start_position().row + 1,
                    });
                }
            }
            "let_declaration" => {
                let name = node
                    .child_by_field_name("pattern")
                    .map(|pattern| node_text(&pattern, source))
                    .unwrap_or_default();
                let name = name.trim_start_matches("mut ").trim().to_string();
                let type_name = node
                    .child_by_field_name("type")
                    .map(|type_node| node_text(&type_node, source))
                    .unwrap_or_default();
                if let Some(scope) = enclosing_function(&node) {
                    receivers.bindings.entry(name).or_default().push(Binding {
                        type_name,
                        scope,
                        at: node.start_position().row + 1,
                    });
                }
            }
            kind if import_types.contains(&kind) => {
                for (name, origin) in import_bindings(&node, source) {
                    receivers.imported.insert(name, origin);
                }
            }
            _ => {}
        }
        let mut cursor = node.walk();
        stack.extend(node.children(&mut cursor));
    }
    receivers
}

/// The lines of the function a node is written in.
fn enclosing_function(node: &Node) -> Option<(usize, usize)> {
    let mut current = node.parent();
    while let Some(parent) = current {
        if matches!(parent.kind(), "function_item" | "closure_expression") {
            return Some((
                parent.start_position().row + 1,
                parent.end_position().row + 1,
            ));
        }
        current = parent.parent();
    }
    None
}

/// The type a text names, and the crate it comes from.
///
/// `&crate::event::Event` names `Event` and comes from the crate graph's
/// root; `Vec<u8>` names `Vec` and comes from nowhere the file names.
fn type_and_origin(raw: &str) -> Option<(String, Option<String>)> {
    let raw = raw
        .trim()
        .trim_start_matches('&')
        .trim_start_matches("mut ")
        .trim();
    let head = raw.split('<').next().unwrap_or(raw).trim();
    let segments: Vec<&str> = head
        .split("::")
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .collect();
    match segments.len() {
        0 => None,
        1 => {
            let name = segments[0];
            is_identifier(name).then(|| (name.to_string(), None))
        }
        _ => {
            let name = segments[segments.len() - 1];
            if !is_identifier(name) {
                return None;
            }
            Some((name.to_string(), Some(segments[0].to_string())))
        }
    }
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
    use super::{
        clean_callee_name, import_origins_in, path_head_of, receiver_of, receivers_in,
        type_names_bound_in, type_qualifier_of,
    };
    use crate::domain::value_objects::{Language, Receiver};
    use std::collections::HashSet;

    /// The crate a name of a Rust file comes from, in the module that holds
    /// the line.
    fn origin_in(source: &str, name: &str, line: usize) -> Option<String> {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .expect("the Rust grammar loads");
        let tree = parser.parse(source, None).expect("the fixture parses");
        let origins = import_origins_in(
            tree.root_node(),
            source.as_bytes(),
            Language::Rust.symbols().import_types,
        );
        origins.crate_of(name, line).map(str::to_string)
    }

    /// What a Rust file says about the receiver written on a line.
    fn receiver_in(source: &str, receiver: &str, line: usize) -> Receiver {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .expect("the Rust grammar loads");
        let tree = parser.parse(source, None).expect("the fixture parses");
        let receivers = receivers_in(
            tree.root_node(),
            source.as_bytes(),
            Language::Rust.symbols().import_types,
        );
        receivers.of(receiver, line)
    }

    fn qualifier(raw: &str) -> Option<String> {
        type_qualifier_of(raw)
    }

    fn head(raw: &str) -> Option<String> {
        path_head_of(raw)
    }

    /// The names a Rust file binds, as a sorted list.
    fn bound_in(source: &str) -> Vec<String> {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .expect("the Rust grammar loads");
        let tree = parser.parse(source, None).expect("the fixture parses");
        let names: HashSet<String> = type_names_bound_in(tree.root_node(), source.as_bytes());
        let mut names: Vec<String> = names.into_iter().collect();
        names.sort();
        names
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
    fn a_path_names_the_crate_it_starts_at() {
        assert_eq!(head("serde_json::to_value").as_deref(), Some("serde_json"));
        assert_eq!(
            head("tokio::time::sleep").as_deref(),
            Some("tokio"),
            "the head is the crate, not the module the path goes through"
        );
        assert_eq!(
            head("::std::time::Duration::new").as_deref(),
            Some("std"),
            "a path that starts at the root of the crate graph names that root"
        );
    }

    #[test]
    fn a_path_that_starts_at_a_type_names_no_crate() {
        assert_eq!(head("Duration::from_millis"), None);
        assert_eq!(head("Self::from_millis"), None);
        assert_eq!(head("T::default"), None);
    }

    #[test]
    fn a_path_that_names_no_crate_is_not_one() {
        assert_eq!(head("build"), None);
        assert_eq!(head("x.method"), None);
        assert_eq!(
            head("make().to_value"),
            None,
            "the path of a value is not a crate path"
        );
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

    #[test]
    fn a_two_letter_name_is_a_type_and_a_one_letter_name_is_not() {
        assert_eq!(qualifier("Et::from_millis").as_deref(), Some("Et"));
        assert_eq!(qualifier("T::from_millis"), None);
    }

    #[test]
    fn a_lowercase_name_is_not_a_type() {
        assert_eq!(qualifier("duration::from_millis"), None);
        assert_eq!(qualifier("_Hidden::from_millis"), None);
    }

    #[test]
    fn a_qualified_trait_path_with_an_associated_type_is_not_judged() {
        assert_eq!(qualifier("<Foo as Trait>::Assoc::method"), None);
    }

    #[test]
    fn a_renamed_import_binds_its_alias() {
        assert_eq!(
            bound_in("use event::EventTime as Et;\nuse other::{Thing as Other, Plain};\n"),
            vec!["Et".to_string(), "Other".to_string()]
        );
    }

    #[test]
    fn a_type_alias_binds_its_name() {
        assert_eq!(
            bound_in("type Moment = EventTime;\nstruct Real;\n"),
            vec!["Moment".to_string()]
        );
    }

    #[test]
    fn a_generic_parameter_binds_its_name_wherever_it_is_declared() {
        let source = "\
fn make<Time: Clock>() -> Time { Time::now() }
struct Holder<Item>(Item);
impl<Unit> Holder<Unit> {}
trait Source { type Output; }
";
        assert_eq!(
            bound_in(source),
            vec![
                "Item".to_string(),
                "Output".to_string(),
                "Time".to_string(),
                "Unit".to_string()
            ]
        );
    }

    #[test]
    fn a_file_that_binds_nothing_binds_no_name() {
        assert!(
            bound_in("use std::time::Duration;\nfn run() { Duration::from_millis(1); }\n")
                .is_empty()
        );
    }

    #[test]
    fn a_turbofish_is_not_the_name_of_the_callee() {
        assert_eq!(clean_callee_name("parse::<u32>"), "parse");
        assert_eq!(clean_callee_name("Vec::<u8>::new"), "new");
        assert_eq!(clean_callee_name("collect::<Vec<_>>"), "collect");
        assert_eq!(
            clean_callee_name("x.parse::<u32>"),
            "parse",
            "the receiver is not the callee either"
        );
    }

    #[test]
    fn a_call_on_a_value_names_the_receiver_it_is_written_on() {
        assert_eq!(receiver_of("bytes.as_bytes"), Some("bytes"));
        assert_eq!(
            receiver_of("Workspace::new().build"),
            Some("Workspace::new()"),
            "the receiver of a chained call is what the chain returns"
        );
        assert_eq!(receiver_of("serde_json::to_value"), None);
        assert_eq!(receiver_of("to_value"), None);
    }

    #[test]
    fn a_receiver_of_the_projects_own_type_is_the_projects() {
        let source = "\
struct Holder { value: u32 }
impl Holder {
    pub fn len(&self) -> usize {
        self.value as usize
    }
}
";
        assert_eq!(
            receiver_in(source, "self", 5),
            Receiver::Project,
            "`self` is the type of the `impl` that holds it"
        );
    }

    #[test]
    fn a_receiver_of_a_type_the_file_declares_is_the_projects() {
        let source = "\
struct Holder { value: u32 }
fn measure(holder: &Holder) -> usize {
    holder.len()
}
";
        assert_eq!(
            receiver_in(source, "holder", 4),
            Receiver::Project,
            "the type of a parameter the file declares is the project's"
        );
    }

    #[test]
    fn a_receiver_of_a_type_the_file_does_not_bind_is_nobody_s() {
        let source = "\
fn measure(values: &Vec<u8>) -> usize {
    values.len()
}
";
        assert_eq!(
            receiver_in(source, "values", 2),
            Receiver::Unbound,
            "a type the file does not bind is the standard library's"
        );
    }

    #[test]
    fn a_receiver_of_a_type_from_another_crate_is_that_crate_s() {
        let source = "\
use serde::Value;
fn measure(value: &Value) -> usize {
    value.len()
}
";
        assert_eq!(
            receiver_in(source, "value", 3),
            Receiver::Crate("serde".to_string()),
            "the import says whose type it is, and the manifest says whether that crate is \
             the project's"
        );
    }

    #[test]
    fn a_receiver_of_a_type_the_project_imports_is_the_projects() {
        let source = "\
use crate::holder::Holder;
fn measure(holder: &Holder) -> usize {
    holder.len()
}
";
        assert_eq!(
            receiver_in(source, "holder", 3),
            Receiver::Project,
            "a type the file imports from the project's own root is the project's"
        );
    }

    #[test]
    fn a_receiver_whose_type_the_file_does_not_state_is_uncertain() {
        let source = "\
struct Holder { value: u32 }
fn measure(holder: &Holder) -> usize {
    holder.value as usize
}
";
        assert_eq!(
            receiver_in(source, "holder.value", 4),
            Receiver::Uncertain,
            "the type of a field is not the file's to say"
        );
    }

    #[test]
    fn a_generic_parameter_is_not_a_type_the_project_declares() {
        let source = "\
fn measure<T>(value: &T) -> usize {
    value.len()
}
";
        assert_eq!(
            receiver_in(source, "value", 2),
            Receiver::Uncertain,
            "the type is whatever the caller chose, and the file does not say whose"
        );
    }

    #[test]
    fn an_import_says_where_the_name_comes_from() {
        let source = "use serde_json::to_value;\nfn run() { to_value(&1); }\n";
        assert_eq!(
            origin_in(source, "to_value", 2).as_deref(),
            Some("serde_json"),
            "the import says the name is the crate's"
        );
    }

    #[test]
    fn an_import_of_the_project_names_no_crate() {
        let source = "use crate::event::build;\nfn run() { build(); }\n";
        assert_eq!(
            origin_in(source, "build", 2),
            None,
            "a path that starts at a root of the crate graph names the project's own thing"
        );
    }

    #[test]
    fn an_alias_says_where_the_name_comes_from() {
        let source = "use serde_json::to_value as tv;\nfn run() { tv(&1); }\n";
        assert_eq!(
            origin_in(source, "tv", 2).as_deref(),
            Some("serde_json"),
            "the alias names the crate's function too"
        );
    }

    #[test]
    fn a_list_import_says_where_every_name_comes_from() {
        let source = "use serde_json::{to_value, from_value};\nfn run() { to_value(&1); }\n";
        assert_eq!(
            origin_in(source, "to_value", 2).as_deref(),
            Some("serde_json")
        );
        assert_eq!(
            origin_in(source, "from_value", 2).as_deref(),
            Some("serde_json")
        );
    }

    #[test]
    fn a_module_does_not_inherit_the_imports_of_the_module_that_holds_it() {
        let source = "\
mod inner {
    use serde_json::to_value;
}
fn run() { to_value(&1); }
";
        assert_eq!(
            origin_in(source, "to_value", 2).as_deref(),
            Some("serde_json"),
            "the module that writes the import binds the name"
        );
        assert_eq!(
            origin_in(source, "to_value", 5),
            None,
            "the module that holds the module does not bind the name"
        );
    }

    #[test]
    fn an_import_of_a_module_of_the_project_names_the_module() {
        let source = "use events::build;\nfn run() { build(); }\n";
        assert_eq!(
            origin_in(source, "build", 2).as_deref(),
            Some("events"),
            "the module the name comes from is named, and the rule asks the manifest whether it \
             is a crate from outside"
        );
    }
}
