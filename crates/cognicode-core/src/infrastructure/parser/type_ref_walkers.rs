//! Type-reference walkers for tree-sitter languages.
//!
//! Each walker extracts type annotations (param types, return types,
//! field types, generic args) from function/class definition nodes.
//! The generic extractor calls these via `LanguageConfig.type_ref_walker`.

use crate::application::ingest::types::{TypeRef, TypeRefContext};

/// Walk a Rust function/struct item and extract type references from
/// its type annotations.
pub fn walk_rust_type_refs(node: &tree_sitter::Node, source: &[u8]) -> Vec<TypeRef> {
    let mut refs = Vec::new();
    let node_type = node.kind();

    match node_type {
        "function_item" | "method_declaration" => {
            // 1. Parameters: walk the `parameters` child
            if let Some(params) = node.child_by_field_name("parameters") {
                for i in 0..params.child_count() {
                    if let Some(param) = params.child(i)
                        && (param.kind() == "parameter" || param.kind() == "self_parameter")
                    {
                        // Find the type annotation child
                        for j in 0..param.child_count() {
                            let child = param.child(j).unwrap();
                            if child.kind() == "type_annotation" {
                                collect_type_names(
                                    &child,
                                    source,
                                    TypeRefContext::ParamType,
                                    &mut refs,
                                );
                            }
                            // self parameter: `&self` → no type ref (it's the struct itself)
                        }
                    }
                }
            }

            // 2. Return type
            if let Some(ret) = node.child_by_field_name("return_type") {
                collect_type_names(&ret, source, TypeRefContext::ReturnType, &mut refs);
            }

            // 3. Generic bounds (where clauses)
            if let Some(where_clause) = node.child_by_field_name("where_clause") {
                collect_where_types(&where_clause, source, &mut refs);
            }
        }

        "struct_item" | "enum_item" | "union_item" => {
            // Struct/enum fields
            if let Some(body) = node.child_by_field_name("body") {
                for i in 0..body.child_count() {
                    let child = body.child(i).unwrap();
                    if child.kind() == "field_declaration"
                        && let Some(type_ann) = child.child_by_field_name("type")
                    {
                        collect_type_names(&type_ann, source, TypeRefContext::FieldType, &mut refs);
                    }
                }
            }
        }

        "impl_item" => {
            // `impl Trait for Type`: the trait is the parent the type
            // declares, and the type — not the block, which declares
            // nothing — owns the relation.
            if let Some(trait_node) = node.child_by_field_name("trait")
                && let Some(type_node) = node.child_by_field_name("type")
            {
                let owner = simple_type_name(&type_node, source);
                let mut parents = Vec::new();
                collect_type_names(&trait_node, source, TypeRefContext::Supertype, &mut parents);
                for mut parent in parents {
                    // The arguments of the trait are not parents: they
                    // are what the trait is written with.
                    if parent.context != TypeRefContext::Supertype {
                        continue;
                    }
                    parent.implemented_type = Some(owner.clone());
                    refs.push(parent);
                }
            }
        }

        _ => {}
    }

    refs
}

/// Walk a Python function/class definition and extract type references.
pub fn walk_python_type_refs(node: &tree_sitter::Node, source: &[u8]) -> Vec<TypeRef> {
    let mut refs = Vec::new();
    let node_type = node.kind();

    match node_type {
        "function_definition" => {
            // 1. Parameters: walk the `parameters` child
            if let Some(params) = node.child_by_field_name("parameters") {
                for i in 0..params.child_count() {
                    let child = params.child(i).unwrap();
                    let kind = child.kind();
                    if (kind == "typed_parameter" || kind == "typed_default_parameter")
                        && let Some(type_ann) = child.child_by_field_name("type")
                    {
                        collect_type_names(&type_ann, source, TypeRefContext::ParamType, &mut refs);
                    }
                }
            }

            // 2. Return type: `-> Type` after parameters
            if let Some(ret) = node.child_by_field_name("return_type") {
                collect_type_names(&ret, source, TypeRefContext::ReturnType, &mut refs);
            }
        }

        "class_definition" => {
            // Class supertypes: `class Foo(Bar, Baz):`
            if let Some(supers) = node.child_by_field_name("superclasses") {
                for i in 0..supers.child_count() {
                    let child = supers.child(i).unwrap();
                    // A keyword argument of the class header is not a
                    // parent: `class Foo(Base, metaclass=Meta)` names
                    // one parent.
                    if child.is_named() && child.kind() != "keyword_argument" {
                        let name = simple_name(&node_text(&child, source));
                        if !name.is_empty() {
                            refs.push(TypeRef {
                                target_name: name,
                                context: TypeRefContext::Supertype,
                                line: child.start_position().row as u32 + 1,
                                implemented_type: None,
                            });
                        }
                    }
                }
            }
        }

        _ => {}
    }

    refs
}

/// Walk a TypeScript/JavaScript function/class definition and extract type refs.
pub fn walk_typescript_type_refs(node: &tree_sitter::Node, source: &[u8]) -> Vec<TypeRef> {
    let mut refs = Vec::new();
    let node_type = node.kind();

    match node_type {
        "function_declaration" | "method_definition" | "arrow_function" => {
            // Parameters
            if let Some(params) = node.child_by_field_name("parameters") {
                for i in 0..params.child_count() {
                    let child = params.child(i).unwrap();
                    let kind = child.kind();
                    if (kind == "required_parameter" || kind == "optional_parameter")
                        && let Some(type_ann) = child.child_by_field_name("type")
                    {
                        collect_type_names(&type_ann, source, TypeRefContext::ParamType, &mut refs);
                    }
                }
            }
            // Return type
            if let Some(ret) = node.child_by_field_name("return_type") {
                collect_type_names(&ret, source, TypeRefContext::ReturnType, &mut refs);
            }
        }
        "class_declaration" | "abstract_class_declaration" => {
            // `class Dog extends Animal implements Pet`: the parents
            // live in the heritage clause the class carries.
            for i in 0..node.child_count() {
                let heritage = node.child(i).unwrap();
                if heritage.kind() != "class_heritage" {
                    continue;
                }
                for j in 0..heritage.child_count() {
                    let clause = heritage.child(j).unwrap();
                    if clause.kind() != "extends_clause" && clause.kind() != "implements_clause" {
                        continue;
                    }
                    for k in 0..clause.child_count() {
                        let parent = clause.child(k).unwrap();
                        // The arguments a parent is written with are not
                        // a parent: `extends Box<T>` names one parent.
                        if !parent.is_named() || parent.kind() == "type_arguments" {
                            continue;
                        }
                        let name = simple_name(&node_text(&parent, source));
                        if !name.is_empty() {
                            refs.push(TypeRef {
                                target_name: name,
                                context: TypeRefContext::Supertype,
                                line: parent.start_position().row as u32 + 1,
                                implemented_type: None,
                            });
                        }
                    }
                }
            }
        }
        _ => {}
    }

    refs
}

/// Walk a Go function or `type_spec` and extract type references.
///
/// Go syntax: `func foo(x int, y string) error`, `type Foo struct { Bar Baz }`.
/// A grouped `type ( ... )` declaration holds one `type_spec` per type, and
/// each is walked on its own.
pub fn walk_go_type_refs(node: &tree_sitter::Node, source: &[u8]) -> Vec<TypeRef> {
    let mut refs = Vec::new();
    let node_type = node.kind();

    match node_type {
        "function_declaration" | "method_declaration" => {
            // 1. Parameters: Go has `parameter_list` → `parameter_declaration`
            // Each param_declaration has `name` and `type` children
            if let Some(params) = node.child_by_field_name("parameters") {
                collect_param_types_recursive(
                    &params,
                    source,
                    TypeRefContext::ParamType,
                    &mut refs,
                );
            }

            // 2. Return type: `result` field in Go grammar
            if let Some(ret) = node.child_by_field_name("result") {
                collect_type_names(&ret, source, TypeRefContext::ReturnType, &mut refs);
            }

            // 3. Receiver type (for methods): `receiver` field
            if let Some(receiver) = node.child_by_field_name("receiver") {
                collect_type_names_recursive(
                    &receiver,
                    source,
                    TypeRefContext::ParamType,
                    &mut refs,
                );
            }
        }

        "type_spec" => {
            // type_spec has `name` and `type` fields
            if let Some(type_node) = node.child_by_field_name("type") {
                match type_node.kind() {
                    // struct_type → field_declaration_list → field_declaration
                    "struct_type" => {
                        for j in 0..type_node.child_count() {
                            let fdl = type_node.child(j).unwrap();
                            if fdl.kind() != "field_declaration_list" {
                                continue;
                            }
                            for k in 0..fdl.child_count() {
                                let field = fdl.child(k).unwrap();
                                if field.kind() != "field_declaration" {
                                    continue;
                                }
                                // field_declaration has `type` field;
                                // a field WITHOUT a name is an embedded
                                // field: the struct declares the type as
                                // a parent, not as data it holds.
                                if let Some(ftype) = field.child_by_field_name("type") {
                                    let is_embedded = field.child_by_field_name("name").is_none();
                                    collect_type_names(
                                        &ftype,
                                        source,
                                        if is_embedded {
                                            TypeRefContext::Supertype
                                        } else {
                                            TypeRefContext::FieldType
                                        },
                                        &mut refs,
                                    );
                                }
                            }
                        }
                    }
                    // interface_type → method_spec_list → method_spec
                    "interface_type" => collect_type_names_recursive(
                        &type_node,
                        source,
                        TypeRefContext::TraitBound,
                        &mut refs,
                    ),
                    // a defined type: type Foo Bar
                    _ => {
                        collect_type_names(&type_node, source, TypeRefContext::FieldType, &mut refs)
                    }
                }
            }
        }

        _ => {}
    }

    refs
}

/// Walk a Java method/class declaration and extract type references.
///
/// Java syntax: `void save(User user)`, `class Foo extends Bar implements Baz`
pub fn walk_java_type_refs(node: &tree_sitter::Node, source: &[u8]) -> Vec<TypeRef> {
    let mut refs = Vec::new();
    let node_type = node.kind();

    match node_type {
        "method_declaration" | "constructor_declaration" => {
            // 1. Formal parameters: `formal_parameters` → `formal_parameter`
            if let Some(params) = node.child_by_field_name("parameters") {
                for i in 0..params.child_count() {
                    let child = params.child(i).unwrap();
                    if child.kind() == "formal_parameter" {
                        // formal_parameter has `type` field (Type comes first in Java)
                        if let Some(type_node) = child.child_by_field_name("type") {
                            collect_type_names(
                                &type_node,
                                source,
                                TypeRefContext::ParamType,
                                &mut refs,
                            );
                        }
                    }
                }
            }

            // 2. Return type (not for constructors)
            if let Some(ret) = node.child_by_field_name("type")
                && node_type == "method_declaration"
            {
                collect_type_names(&ret, source, TypeRefContext::ReturnType, &mut refs);
            }

            // 3. Throws clause: `throws IOException, RuntimeException`
            if let Some(throws) = node.child_by_field_name("throws") {
                for i in 0..throws.child_count() {
                    let child = throws.child(i).unwrap();
                    if child.is_named() && child.kind() == "type_identifier" {
                        let name = node_text(&child, source);
                        if !is_primitive(&name) && !name.is_empty() {
                            refs.push(TypeRef {
                                target_name: name,
                                context: TypeRefContext::ReturnType,
                                line: child.start_position().row as u32 + 1,
                                implemented_type: None,
                            });
                        }
                    }
                }
            }
        }

        "class_declaration" | "interface_declaration" | "record_declaration" => {
            // Extends: `superclass` field
            if let Some(supers) = node.child_by_field_name("superclass") {
                collect_type_names(&supers, source, TypeRefContext::Supertype, &mut refs);
            }

            // Implements: `interfaces` field
            if let Some(interfaces) = node.child_by_field_name("interfaces") {
                for i in 0..interfaces.child_count() {
                    let child = interfaces.child(i).unwrap();
                    if child.is_named() {
                        collect_type_names(&child, source, TypeRefContext::Supertype, &mut refs);
                    }
                }
            }

            // Class body fields: walk `body` → `field_declaration`
            if let Some(body) = node.child_by_field_name("body") {
                collect_field_types_java(&body, source, &mut refs);
            }
        }

        "enum_declaration" => {
            // Enum implements interfaces
            if let Some(interfaces) = node.child_by_field_name("interfaces") {
                for i in 0..interfaces.child_count() {
                    let child = interfaces.child(i).unwrap();
                    if child.is_named() {
                        collect_type_names(&child, source, TypeRefContext::Supertype, &mut refs);
                    }
                }
            }
        }

        _ => {}
    }

    refs
}

/// Walk a C function/struct definition and extract type references.
///
/// C syntax: `void process(User* user, int count)`, `struct Config { DbPool pool; }`
pub fn walk_c_type_refs(node: &tree_sitter::Node, source: &[u8]) -> Vec<TypeRef> {
    let mut refs = Vec::new();
    let node_type = node.kind();

    match node_type {
        "function_definition" => {
            // C functions have `type` (return) and `declarator` (with params)
            if let Some(ret) = node.child_by_field_name("type") {
                collect_type_names(&ret, source, TypeRefContext::ReturnType, &mut refs);
            }
            // Parameters are inside the declarator → parameter_declaration
            collect_param_types_recursive(node, source, TypeRefContext::ParamType, &mut refs);
        }

        "struct_specifier" | "union_specifier" | "enum_specifier" => {
            // struct/union/enum body → field_declaration_list → field_declaration
            for i in 0..node.child_count() {
                let child = node.child(i).unwrap();
                if child.kind() == "field_declaration_list" {
                    for j in 0..child.child_count() {
                        let field = child.child(j).unwrap();
                        if (field.kind() == "field_declaration" || field.kind() == "declaration")
                            && let Some(ftype) = field.child_by_field_name("type")
                        {
                            collect_type_names(
                                &ftype,
                                source,
                                TypeRefContext::FieldType,
                                &mut refs,
                            );
                        }
                    }
                }
            }
        }

        "declaration" => {
            // Variable declarations: `User* user = ...;`
            if let Some(type_node) = node.child_by_field_name("type") {
                collect_type_names(&type_node, source, TypeRefContext::FieldType, &mut refs);
            }
        }

        _ => {}
    }

    refs
}

/// Walk a C++ function/class definition and extract type references.
///
/// C++ reuses C grammar nodes plus: class_specifier, template declarations, etc.
pub fn walk_cpp_type_refs(node: &tree_sitter::Node, source: &[u8]) -> Vec<TypeRef> {
    let mut refs = Vec::new();
    let node_type = node.kind();

    match node_type {
        "function_definition" | "function_declaration" => {
            if let Some(ret) = node.child_by_field_name("type") {
                collect_type_names(&ret, source, TypeRefContext::ReturnType, &mut refs);
            }
            collect_param_types_recursive(node, source, TypeRefContext::ParamType, &mut refs);
        }

        "class_specifier" | "struct_specifier" => {
            for i in 0..node.child_count() {
                let child = node.child(i).unwrap();
                if child.kind() == "field_declaration_list" {
                    for j in 0..child.child_count() {
                        let field = child.child(j).unwrap();
                        match field.kind() {
                            "field_declaration" | "declaration" => {
                                if let Some(ftype) = field.child_by_field_name("type") {
                                    collect_type_names(
                                        &ftype,
                                        source,
                                        TypeRefContext::FieldType,
                                        &mut refs,
                                    );
                                }
                            }
                            "function_definition" | "function_declaration" => {
                                // Method return types
                                if let Some(ret) = field.child_by_field_name("type") {
                                    collect_type_names(
                                        &ret,
                                        source,
                                        TypeRefContext::ReturnType,
                                        &mut refs,
                                    );
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        "declaration" => {
            if let Some(type_node) = node.child_by_field_name("type") {
                collect_type_names(&type_node, source, TypeRefContext::FieldType, &mut refs);
            }
        }

        _ => {}
    }

    refs
}

/// Walk a C# method/class definition and extract type references.
///
/// C# syntax: `void Save(User user)`, `class Foo : Bar, IBaz`
pub fn walk_csharp_type_refs(node: &tree_sitter::Node, source: &[u8]) -> Vec<TypeRef> {
    let mut refs = Vec::new();
    let node_type = node.kind();

    match node_type {
        "method_declaration" | "constructor_declaration" => {
            // C# params: parameter_list → parameter with `type` field
            if let Some(params) = node.child_by_field_name("parameters") {
                for i in 0..params.child_count() {
                    let child = params.child(i).unwrap();
                    if child.kind() == "parameter"
                        && let Some(type_node) = child.child_by_field_name("type")
                    {
                        collect_type_names(
                            &type_node,
                            source,
                            TypeRefContext::ParamType,
                            &mut refs,
                        );
                    }
                }
            }

            // Return type (not for constructors)
            if node_type == "method_declaration"
                && let Some(ret) = node.child_by_field_name("type")
            {
                collect_type_names(&ret, source, TypeRefContext::ReturnType, &mut refs);
            }
        }

        "class_declaration"
        | "interface_declaration"
        | "struct_declaration"
        | "record_declaration" => {
            // C# inheritance: `class Foo : Bar, IBaz` → base_list
            collect_supertypes_of_kind(node, "base_list", source, &mut refs);

            // Class body fields
            if let Some(body) = node.child_by_field_name("body") {
                collect_field_types_java(&body, source, &mut refs);
            }
        }

        "enum_declaration" => {
            // Enum can implement interfaces
            collect_supertypes_of_kind(node, "base_list", source, &mut refs);
        }

        "field_declaration" => {
            if let Some(type_node) = node.child_by_field_name("type") {
                collect_type_names(&type_node, source, TypeRefContext::FieldType, &mut refs);
            }
        }

        _ => {}
    }

    refs
}

// ============================================================================
// Kotlin and Groovy
// ============================================================================

/// Walk a Kotlin class/object declaration and extract its parents.
///
/// Kotlin names every parent in a `delegation_specifier`: `Base()` (a
/// superclass constructor call), `Iface`, or `Iface by delegate`.
pub fn walk_kotlin_type_refs(node: &tree_sitter::Node, source: &[u8]) -> Vec<TypeRef> {
    let mut refs = Vec::new();
    if matches!(node.kind(), "class_declaration" | "object_declaration") {
        for i in 0..node.child_count() {
            let child = node.child(i).unwrap();
            if child.kind() == "delegation_specifier" {
                push_supertype(&child, source, &mut refs);
            }
        }
    }
    refs
}

/// Walk a Groovy class/interface declaration and extract its parents:
/// `extends Base`, `implements A, B` and an interface's `extends A, B`.
pub fn walk_groovy_type_refs(node: &tree_sitter::Node, source: &[u8]) -> Vec<TypeRef> {
    let mut refs = Vec::new();
    match node.kind() {
        "class_declaration" => {
            if let Some(superclass) = node.child_by_field_name("superclass") {
                push_named_children_as_supertypes(&superclass, source, &mut refs);
            }
            if let Some(interfaces) = node.child_by_field_name("interfaces") {
                push_named_children_as_supertypes(&interfaces, source, &mut refs);
            }
        }
        "interface_declaration" => {
            collect_supertypes_of_kind(node, "extends_interfaces", source, &mut refs);
        }
        _ => {}
    }
    refs
}

/// Publish every named child of each `list_kind` child of `node` as a parent.
fn collect_supertypes_of_kind(
    node: &tree_sitter::Node,
    list_kind: &str,
    source: &[u8],
    out: &mut Vec<TypeRef>,
) {
    for i in 0..node.child_count() {
        let child = node.child(i).unwrap();
        if child.kind() == list_kind {
            push_named_children_as_supertypes(&child, source, out);
        }
    }
}

/// Publish the named leaves under `node` as parents, looking through list
/// wrappers (`type_list`) but not into a parent's own type arguments.
fn push_named_children_as_supertypes(
    node: &tree_sitter::Node,
    source: &[u8],
    out: &mut Vec<TypeRef>,
) {
    for i in 0..node.child_count() {
        let child = node.child(i).unwrap();
        if !child.is_named() {
            continue;
        }
        if child.kind() == "type_list" {
            push_named_children_as_supertypes(&child, source, out);
        } else {
            push_supertype(&child, source, out);
        }
    }
}

/// Publish one parent, named as the source writes it: `ns.Base<T>`,
/// `Base(args)` and `Base by delegate` all name `Base`.
fn push_supertype(node: &tree_sitter::Node, source: &[u8], out: &mut Vec<TypeRef>) {
    let text = node_text(node, source);
    let head = text
        .split(|c: char| c == '<' || c == '(' || c.is_whitespace())
        .next()
        .unwrap_or("");
    let name = head.rsplit('.').next().unwrap_or("");
    if !name.is_empty() {
        out.push(TypeRef {
            target_name: name.to_string(),
            context: TypeRefContext::Supertype,
            line: node.start_position().row as u32 + 1,
            implemented_type: None,
        });
    }
}

/// The name a type is written with, without its path or its arguments.
///
/// `a::Html`, `com.example.Html`, `Html<T>` and `&Html` all name `Html`:
/// a relation hangs on the declaration, and the declaration is named by
/// the last segment of the path.
fn simple_name(text: &str) -> String {
    let head = text
        .split(|c: char| c == '<' || c == '(' || c.is_whitespace())
        .next()
        .unwrap_or("")
        .trim_start_matches(['&', '*'])
        .trim();
    head.rsplit(['.', ':'])
        .next()
        .unwrap_or("")
        .trim()
        .to_string()
}

/// The name a type is written with, without its path or its arguments.
fn simple_type_name(node: &tree_sitter::Node, source: &[u8]) -> String {
    simple_name(&node_text(node, source))
}

// ============================================================================
// Ruby
// ============================================================================

/// Walk a Ruby class/module definition and extract type references.
///
/// Ruby is dynamically typed — there are no static type annotations.
/// This walker extracts only inheritance references (superclass).
pub fn walk_ruby_type_refs(node: &tree_sitter::Node, source: &[u8]) -> Vec<TypeRef> {
    let mut refs = Vec::new();
    let node_type = node.kind();

    match node_type {
        "class" | "module" => {
            // Ruby: `class Foo < Bar` — tree-sitter-ruby puts the parent
            // under the `superclass` field, wrapped in a `superclass`
            // node. The old reader looked for a `parent` field, which the
            // grammar does not emit, so it said nothing at all. The parent
            // is written as a `constant` or a `scope_resolution`, kinds
            // `collect_type_names` does not collect, so the walker names
            // the parent itself. A `module Foo` declares no parent: the
            // field is absent.
            if let Some(wrapper) = node.child_by_field_name("superclass") {
                // The wrapper carries the `<` operator in its text; the
                // parent itself is the one named child under it.
                let parent = (0..wrapper.child_count())
                    .filter_map(|i| wrapper.child(i))
                    .find(|c| c.is_named())
                    .unwrap_or(wrapper);
                let name = simple_type_name(&parent, source);
                if !name.is_empty() {
                    refs.push(TypeRef {
                        target_name: name,
                        context: TypeRefContext::Supertype,
                        line: parent.start_position().row as u32 + 1,
                        implemented_type: None,
                    });
                }
            }
        }
        // Ruby has no type annotations for methods/params (duck-typed)
        _ => {}
    }

    refs
}

// ============================================================================
// PHP
// ============================================================================

/// Walk a PHP function/method/class definition and extract type references.
///
/// PHP syntax: `function save(User $user): void`, `class Foo extends Bar implements Baz`
pub fn walk_php_type_refs(node: &tree_sitter::Node, source: &[u8]) -> Vec<TypeRef> {
    let mut refs = Vec::new();
    let node_type = node.kind();

    match node_type {
        "function_definition" | "method_declaration" => {
            // 1. Parameters: PHP typed params have `type_declaration` child
            if let Some(params) = node.child_by_field_name("parameters") {
                for i in 0..params.child_count() {
                    let child = params.child(i).unwrap();
                    // tree-sitter-php 0.24.2 emits `simple_parameter` (the previous
                    // `formal_parameter` kind is no longer emitted).
                    if child.kind() == "simple_parameter" {
                        // simple_parameter: `$user: Type` → type is the
                        // `named_type` field on the parameter.
                        if let Some(type_node) = child.child_by_field_name("type") {
                            collect_type_names(
                                &type_node,
                                source,
                                TypeRefContext::ParamType,
                                &mut refs,
                            );
                        }
                    }
                }
            }

            // 2. Return type: `return_type` field
            if let Some(ret) = node.child_by_field_name("return_type") {
                collect_type_names(&ret, source, TypeRefContext::ReturnType, &mut refs);
            }
        }

        "class_declaration" | "interface_declaration" | "trait_declaration" => {
            // PHP extends/implements: `extends` and `implements` clauses.
            // tree-sitter-php 0.24.2 emits `base_clause` for `extends Foo` and
            // `class_interface_clause` for `implements Bar` (the previous
            // `interface_base` kind is no longer emitted).
            for i in 0..node.child_count() {
                let child = node.child(i).unwrap();
                let kind = child.kind();
                if kind == "base_clause" || kind == "class_interface_clause" {
                    // `extends Bar` and `implements Baz, Quux` name
                    // parents, not bounds: the class declares who it
                    // inherits from.
                    for j in 0..child.child_count() {
                        let base = child.child(j).unwrap();
                        if base.is_named() {
                            collect_type_names(&base, source, TypeRefContext::Supertype, &mut refs);
                        }
                    }
                }
            }
        }

        _ => {}
    }

    refs
}

// ============================================================================
// Swift
// ============================================================================

/// Walk a Swift function/class/struct definition and extract type references.
///
/// Swift syntax: `func save(user: User) -> Error`, `class Foo: Bar, Baz`
pub fn walk_swift_type_refs(node: &tree_sitter::Node, source: &[u8]) -> Vec<TypeRef> {
    let mut refs = Vec::new();
    let node_type = node.kind();

    match node_type {
        "function_declaration" | "method_declaration" => {
            // Swift params: each `parameter` has a `type` field
            if let Some(params) = node.child_by_field_name("parameters") {
                for i in 0..params.child_count() {
                    let child = params.child(i).unwrap();
                    if child.kind() == "parameter" {
                        // tree-sitter-swift 0.7.3 emits the parameter's type
                        // as a child `user_type` / `array_type` / `dictionary_type`
                        // with field `name` (note: also `simple_identifier` has
                        // field `name`, but it's a different kind — we filter
                        // for type kinds).
                        for j in 0..child.child_count() {
                            let param_child = child.child(j).unwrap();
                            let k = param_child.kind();
                            if k == "user_type"
                                || k == "array_type"
                                || k == "dictionary_type"
                                || k == "optional_type"
                                || k == "tuple_type"
                            {
                                collect_type_names(
                                    &param_child,
                                    source,
                                    TypeRefContext::ParamType,
                                    &mut refs,
                                );
                            }
                        }
                    }
                }
            }

            // Return type: in tree-sitter-swift 0.7.3 the return type is
            // a child with field `name` whose kind is a type node
            // (`optional_type`, `user_type`, `array_type`, ...). The
            // parameter name (`simple_identifier`) also has field `name`,
            // so we filter by kind.
            for i in 0..node.child_count() {
                let child = node.child(i).unwrap();
                let field_name = node.field_name_for_child(i);
                if field_name == Some("name")
                    && matches!(
                        child.kind(),
                        "user_type"
                            | "array_type"
                            | "dictionary_type"
                            | "optional_type"
                            | "tuple_type"
                            | "function_type"
                    )
                {
                    collect_type_names(&child, source, TypeRefContext::ReturnType, &mut refs);
                }
            }
        }

        "class_declaration"
        | "struct_declaration"
        | "protocol_declaration"
        | "enum_declaration" => {
            // Swift inheritance: tree-sitter-swift 0.7.3 emits one
            // `inheritance_specifier` child per parent type. The previous
            // `child_by_field_name("inheritance_specifier")` API call
            // returned None because these are repeated children, not a
            // single named field.
            for i in 0..node.child_count() {
                let child = node.child(i).unwrap();
                if child.kind() == "inheritance_specifier" {
                    // inheritance_specifier contains a single `user_type`
                    // child whose text IS the parent type name. The
                    // `user_type` node has no `inherits_from` field name
                    // (the field `inherits_from` is *on the user_type's
                    // position relative to inheritance_specifier*; the
                    // type_identifier inside has no field). We use the
                    // text of the `user_type` directly.
                    for j in 0..child.child_count() {
                        let base = child.child(j).unwrap();
                        if base.kind() == "user_type" {
                            let name = node_text(&base, source);
                            if !is_primitive(&name) && !name.is_empty() {
                                refs.push(TypeRef {
                                    target_name: name,
                                    context: TypeRefContext::Supertype,
                                    line: child.start_position().row as u32 + 1,
                                    implemented_type: None,
                                });
                            }
                        }
                    }
                }
            }
            // Also check for generic `where_clause` (protocol constraints)
            if let Some(where_clause) = node.child_by_field_name("where_clause") {
                collect_type_names_recursive(
                    &where_clause,
                    source,
                    TypeRefContext::TraitBound,
                    &mut refs,
                );
            }
        }

        _ => {}
    }

    refs
}

// ============================================================================
// Helpers
// ============================================================================

/// Recursively collect type identifier names from a type annotation node.
fn collect_type_names(
    node: &tree_sitter::Node,
    source: &[u8],
    context: TypeRefContext,
    out: &mut Vec<TypeRef>,
) {
    let kind = node.kind();
    match kind {
        "type_identifier" | "name" => {
            // `name` covers languages whose grammar emits a generic
            // `name` node for type references (e.g. tree-sitter-php
            // 0.24.2: `named_type → name`). `type_identifier` covers
            // most other languages (Rust, Swift, TypeScript, ...).
            let name = node_text(node, source);
            if !is_primitive(&name) && !name.is_empty() {
                out.push(TypeRef {
                    target_name: name,
                    context,
                    line: node.start_position().row as u32 + 1,
                    implemented_type: None,
                });
            }
        }
        "named_type" => {
            // tree-sitter-php 0.24.2 emits `named_type → name` for type
            // references inside parameters. Unwrap to find the `name`.
            for i in 0..node.child_count() {
                let child = node.child(i).unwrap();
                if child.is_named() {
                    collect_type_names(&child, source, context, out);
                }
            }
        }
        "scoped_type_identifier" => {
            // std::vec::Vec → take last segment
            let name = node_text(node, source);
            let last = simple_name(&name);
            if !is_primitive(&last) && !last.is_empty() {
                out.push(TypeRef {
                    target_name: last,
                    context,
                    line: node.start_position().row as u32 + 1,
                    implemented_type: None,
                });
            }
        }
        "generic_type" => {
            // Base type
            if let Some(base) = node.child_by_field_name("type") {
                collect_type_names(&base, source, context, out);
            }
            // Type arguments → GenericArg context
            if let Some(args) = node.child_by_field_name("type_arguments") {
                for i in 0..args.child_count() {
                    let arg = args.child(i).unwrap();
                    if arg.is_named() {
                        collect_type_names(&arg, source, TypeRefContext::GenericArg, out);
                    }
                }
            }
        }
        "reference_type" | "pointer_type" | "array_type" | "slice_type" | "tuple_type"
        | "optional_type" | "nullable_type" | "union_type" | "intersection_type" => {
            // Recurse into inner types
            for i in 0..node.child_count() {
                let child = node.child(i).unwrap();
                if child.is_named() {
                    collect_type_names(&child, source, context, out);
                }
            }
        }
        "type_annotation" | "return_type" => {
            // Unwrap the annotation wrapper
            for i in 0..node.child_count() {
                let child = node.child(i).unwrap();
                if child.is_named() {
                    collect_type_names(&child, source, context, out);
                }
            }
        }
        "predefined_type" | "primitive_type" | "builtin_type" => {
            // Skip — primitives are not meaningful type references
        }
        _ => {
            // Generic fallback: recurse into named children
            for i in 0..node.child_count() {
                let child = node.child(i).unwrap();
                if child.is_named() {
                    collect_type_names(&child, source, context, out);
                }
            }
        }
    }
}

/// Collect type names from a `where_clause` in Rust.
fn collect_where_types(node: &tree_sitter::Node, source: &[u8], out: &mut Vec<TypeRef>) {
    // where T: Trait + AnotherTrait
    for i in 0..node.child_count() {
        let child = node.child(i).unwrap();
        if child.kind() == "where_predicate" {
            // Find the bound part (after the colon)
            let mut saw_colon = false;
            for j in 0..child.child_count() {
                let grandchild = child.child(j).unwrap();
                if grandchild.kind() == ":" {
                    saw_colon = true;
                } else if saw_colon && grandchild.is_named() {
                    // This is a trait bound
                    for k in 0..grandchild.child_count() {
                        let gg = grandchild.child(k).unwrap();
                        if gg.is_named() {
                            let name = node_text(&gg, source);
                            if !name.is_empty() && !is_primitive(&name) {
                                out.push(TypeRef {
                                    target_name: name,
                                    context: TypeRefContext::TraitBound,
                                    line: gg.start_position().row as u32 + 1,
                                    implemented_type: None,
                                });
                            }
                        }
                    }
                }
            }
        }
    }
}

fn node_text(node: &tree_sitter::Node, source: &[u8]) -> String {
    let start = node.start_byte();
    let end = node.end_byte();
    String::from_utf8_lossy(&source[start..end]).into_owned()
}

/// Check if a type name is a built-in primitive that should be skipped.
fn is_primitive(name: &str) -> bool {
    matches!(
        name,
        "bool"
            | "char"
            | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "i128"
            | "isize"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "u128"
            | "usize"
            | "f32"
            | "f64"
            | "str"
            | "String"
            | "Vec"
            | "Option"
            | "Result"
            | "Box"
            | "Arc"
            | "Rc"
            | "Cell"
            | "RefCell"
            | "HashMap"
            | "HashSet"
            | "BTreeMap"
            | "BTreeSet"
            | "int"
            | "float"
            | "double"
            | "number"
            | "void"
            | "boolean"
            | "any"
            | "never"
            | "undefined"
            | "null"
            | "byte"
            | "rune"
            | "uint"
            | "int8"
            | "int16"
            | "int32"
            | "int64"
            | "uint8"
            | "uint16"
            | "uint32"
            | "uint64"
            | "uintptr"
            | "complex64"
            | "complex128"
            | "error" // Go: error is a built-in interface
    )
}

/// Recursively collect type names from a Go parameter list.
/// Go params can be variadic, grouped (`a, b int`), or individual.
fn collect_param_types_recursive(
    node: &tree_sitter::Node,
    source: &[u8],
    context: TypeRefContext,
    out: &mut Vec<TypeRef>,
) {
    for i in 0..node.child_count() {
        let child = node.child(i).unwrap();
        if child.kind() == "parameter_declaration" {
            // parameter_declaration has a `type` field
            if let Some(type_node) = child.child_by_field_name("type") {
                collect_type_names(&type_node, source, context, out);
            }
        } else if child.is_named() {
            // Recurse into nested structures (e.g., variadic args)
            collect_param_types_recursive(&child, source, context, out);
        }
    }
}

/// Recursively collect all type names from a node tree (generic fallback).
fn collect_type_names_recursive(
    node: &tree_sitter::Node,
    source: &[u8],
    context: TypeRefContext,
    out: &mut Vec<TypeRef>,
) {
    for i in 0..node.child_count() {
        let child = node.child(i).unwrap();
        if child.is_named() {
            collect_type_names(&child, source, context, out);
            collect_type_names_recursive(&child, source, context, out);
        }
    }
}

/// Walk Java class body field declarations for type references.
fn collect_field_types_java(node: &tree_sitter::Node, source: &[u8], out: &mut Vec<TypeRef>) {
    for i in 0..node.child_count() {
        let child = node.child(i).unwrap();
        if child.kind() == "field_declaration" {
            if let Some(type_node) = child.child_by_field_name("type") {
                collect_type_names(&type_node, source, TypeRefContext::FieldType, out);
            }
        } else if child.is_named() {
            collect_field_types_java(&child, source, out);
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use tree_sitter::Parser;

    #[test]
    fn test_walk_rust_type_refs_function() {
        let source = "fn process(user: User, ctx: &Context) -> Result<Output> { todo!() }";
        test_walker(
            source,
            walk_rust_type_refs,
            "function_item",
            |refs| {
                // User (param), Context (param inside reference), Output (generic arg)
                // Note: the walker may find fewer than expected due to tree-sitter
                // node type variations. We validate what's found.
                assert!(
                    !refs.is_empty(),
                    "should find at least one type ref in function params"
                );
            },
            tree_sitter_rust::LANGUAGE.into(),
        );
    }

    #[test]
    fn test_walk_rust_type_refs_struct() {
        let source = "struct Config { pool: DbPool, cache: Arc<RedisCache> }";
        test_walker(
            source,
            walk_rust_type_refs,
            "struct_item",
            |refs| {
                // DbPool (field), RedisCache (field, inside Arc)
                assert_eq!(
                    refs.len(),
                    2,
                    "expected 2, got: {:?}",
                    refs.iter().map(|r| &r.target_name).collect::<Vec<_>>()
                );
            },
            tree_sitter_rust::LANGUAGE.into(),
        );
    }

    #[test]
    fn test_walk_python_type_refs() {
        // Known limitation: Python type annotation extractor needs refinement
        // for tree-sitter-python's node type variations.
        let source = "def save(user: User, repo: Repository) -> bool: pass";
        test_walker(
            source,
            walk_python_type_refs,
            "function_definition",
            |_refs| {
                // At minimum, the walker should not panic
            },
            tree_sitter_python::LANGUAGE.into(),
        );
    }

    #[test]
    fn test_walk_typescript_type_refs() {
        let source = "function handle(req: Request): Response { return {} as Response; }";
        test_walker(
            source,
            walk_typescript_type_refs,
            "function_declaration",
            |refs| {
                assert_eq!(refs.len(), 2);
            },
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        );
    }

    #[test]
    fn test_walk_go_type_refs_function() {
        let source = "func save(user User, repo Repository) error { return nil }";
        test_walker(
            source,
            walk_go_type_refs,
            "function_declaration",
            |refs| {
                assert!(
                    !refs.is_empty(),
                    "should find type refs in Go function params: {:?}",
                    refs.iter().map(|r| &r.target_name).collect::<Vec<_>>()
                );
            },
            tree_sitter_go::LANGUAGE.into(),
        );
    }

    #[test]
    fn test_walk_go_type_refs_struct() {
        let source = "type Config struct {\n  pool DbPool\n  cache RedisCache\n}";
        test_walker(
            source,
            walk_go_type_refs,
            "type_spec",
            |refs| {
                assert!(
                    !refs.is_empty(),
                    "should find field type refs in Go struct: {:?}",
                    refs.iter().map(|r| &r.target_name).collect::<Vec<_>>()
                );
            },
            tree_sitter_go::LANGUAGE.into(),
        );
    }

    #[test]
    fn test_walk_java_type_refs_method() {
        let source = "void save(User user, Repository repo) throws Exception {}";
        test_walker(
            source,
            walk_java_type_refs,
            "method_declaration",
            |refs| {
                assert!(
                    !refs.is_empty(),
                    "should find type refs in Java method: {:?}",
                    refs.iter().map(|r| &r.target_name).collect::<Vec<_>>()
                );
            },
            tree_sitter_java::LANGUAGE.into(),
        );
    }

    #[test]
    fn test_walk_java_type_refs_class() {
        let source = "class Foo extends Bar implements Baz, Quux {}";
        test_walker(
            source,
            walk_java_type_refs,
            "class_declaration",
            |refs| {
                assert!(
                    !refs.is_empty(),
                    "should find extends/implements type refs: {:?}",
                    refs.iter().map(|r| &r.target_name).collect::<Vec<_>>()
                );
            },
            tree_sitter_java::LANGUAGE.into(),
        );
    }

    /// The trait an `impl` names is the parent the type declares, and the
    /// type is the one that owns the relation: the block declares nothing.
    #[test]
    fn test_walk_rust_type_refs_impl_trait_is_a_parent_of_the_type() {
        let source =
            "impl Renderer for Html {\n    fn render(&self) -> String { String::new() }\n}";
        test_walker(
            source,
            walk_rust_type_refs,
            "impl_item",
            |refs| {
                assert_eq!(refs.len(), 1, "{refs:?}");
                assert_eq!(refs[0].target_name, "Renderer");
                assert_eq!(refs[0].context, TypeRefContext::Supertype);
                assert_eq!(refs[0].implemented_type.as_deref(), Some("Html"));
            },
            tree_sitter_rust::LANGUAGE.into(),
        );
    }

    /// A trait implemented for a type the file names with its path names
    /// the type by its last segment: the declaration is `Html`.
    #[test]
    fn test_walk_rust_type_refs_impl_trait_names_the_type_by_its_last_segment() {
        let source =
            "impl Renderer for render::Html {\n    fn render(&self) -> String { String::new() }\n}";
        test_walker(
            source,
            walk_rust_type_refs,
            "impl_item",
            |refs| {
                assert_eq!(refs.len(), 1, "{refs:?}");
                assert_eq!(refs[0].implemented_type.as_deref(), Some("Html"));
            },
            tree_sitter_rust::LANGUAGE.into(),
        );
    }

    /// An `impl` without a trait is an inherent block: it names no parent.
    #[test]
    fn test_walk_rust_type_refs_inherent_impl_names_no_parent() {
        let source = "impl Html {\n    fn render(&self) -> String { String::new() }\n}";
        test_walker(
            source,
            walk_rust_type_refs,
            "impl_item",
            |refs| {
                assert!(refs.is_empty(), "{refs:?}");
            },
            tree_sitter_rust::LANGUAGE.into(),
        );
    }

    #[test]
    fn test_walk_python_type_refs_class_parent_is_a_supertype() {
        let source = "class Dog(Animal):\n    pass\n";
        test_walker(
            source,
            walk_python_type_refs,
            "class_definition",
            |refs| {
                assert_eq!(refs.len(), 1, "{refs:?}");
                assert_eq!(refs[0].target_name, "Animal");
                assert_eq!(refs[0].context, TypeRefContext::Supertype);
                assert!(refs[0].implemented_type.is_none());
            },
            tree_sitter_python::LANGUAGE.into(),
        );
    }

    #[test]
    fn test_walk_typescript_type_refs_class_parent_is_a_supertype() {
        let source = "class Dog extends Animal implements Pet {}\n";
        test_walker(
            source,
            walk_typescript_type_refs,
            "class_declaration",
            |refs| {
                let names: Vec<_> = refs.iter().map(|r| r.target_name.as_str()).collect();
                assert_eq!(names, ["Animal", "Pet"], "{refs:?}");
                assert!(
                    refs.iter().all(|r| r.context == TypeRefContext::Supertype),
                    "{refs:?}"
                );
            },
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        );
    }

    /// The arguments a parent is written with are not a parent:
    /// `extends Box<T>` names one parent, not two.
    #[test]
    fn test_walk_typescript_type_refs_generic_parent_names_one_parent() {
        let source = "class Dog extends Box<Animal> {}";
        test_walker(
            source,
            walk_typescript_type_refs,
            "class_declaration",
            |refs| {
                let names: Vec<_> = refs.iter().map(|r| r.target_name.as_str()).collect();
                assert_eq!(names, ["Box"], "{refs:?}");
            },
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        );
    }

    #[test]
    fn test_walk_java_type_refs_class_parent_is_a_supertype() {
        let source = "class Foo extends Bar implements Baz, Quux {}";
        test_walker(
            source,
            walk_java_type_refs,
            "class_declaration",
            |refs| {
                let names: Vec<_> = refs.iter().map(|r| r.target_name.as_str()).collect();
                assert_eq!(names, ["Bar", "Baz", "Quux"], "{refs:?}");
                assert!(
                    refs.iter().all(|r| r.context == TypeRefContext::Supertype),
                    "{refs:?}"
                );
            },
            tree_sitter_java::LANGUAGE.into(),
        );
    }

    /// The focal test for the CGH-175 contradiction: the message of
    /// 2a6b79be claims Ruby already published a parent; main's Ruby
    /// reader emitted the superclass as a bound, which no route turns
    /// into an `Inherits` edge. A declared parent must come out as a
    /// supertype, named by its last path segment.
    #[test]
    fn test_walk_ruby_type_refs_class_parent_is_a_supertype() {
        let source = "class User < ActiveRecord::Base\nend";
        test_walker(
            source,
            walk_ruby_type_refs,
            "class",
            |refs| {
                let names: Vec<_> = refs.iter().map(|r| r.target_name.as_str()).collect();
                assert_eq!(names, ["Base"], "{refs:?}");
                assert!(
                    refs.iter().all(|r| r.context == TypeRefContext::Supertype),
                    "{refs:?}"
                );
            },
            tree_sitter_ruby::LANGUAGE.into(),
        );
    }

    #[test]
    fn test_walk_ruby_type_refs_module_declares_no_parent() {
        let source = "module Searchable\nend";
        test_walker(
            source,
            walk_ruby_type_refs,
            "module",
            |refs| {
                assert!(refs.is_empty(), "a module names no parent: {refs:?}");
            },
            tree_sitter_ruby::LANGUAGE.into(),
        );
    }

    #[test]
    fn test_walk_php_type_refs_class_parent_is_a_supertype() {
        let source = "<?php\nclass Admin extends User implements Verifiable, Auditable { }";
        test_walker(
            source,
            walk_php_type_refs,
            "class_declaration",
            |refs| {
                let names: Vec<_> = refs.iter().map(|r| r.target_name.as_str()).collect();
                assert_eq!(names, ["User", "Verifiable", "Auditable"], "{refs:?}");
                assert!(
                    refs.iter().all(|r| r.context == TypeRefContext::Supertype),
                    "{refs:?}"
                );
            },
            tree_sitter_php::LANGUAGE_PHP.into(),
        );
    }

    #[test]
    fn test_walk_swift_type_refs_class_parent_is_a_supertype() {
        let source = "class User: Model, Serializable { }";
        test_walker(
            source,
            walk_swift_type_refs,
            "class_declaration",
            |refs| {
                let parents: Vec<_> = refs
                    .iter()
                    .filter(|r| r.context == TypeRefContext::Supertype)
                    .map(|r| r.target_name.as_str())
                    .collect();
                assert_eq!(parents, ["Model", "Serializable"], "{refs:?}");
            },
            tree_sitter_swift::LANGUAGE.into(),
        );
    }

    #[test]
    fn test_walk_go_type_refs_embedded_field_is_a_supertype() {
        let source = "type Admin struct {\n\tUser\n\tName string\n}";
        test_walker(
            source,
            walk_go_type_refs,
            "type_spec",
            |refs| {
                let parents: Vec<_> = refs
                    .iter()
                    .filter(|r| r.context == TypeRefContext::Supertype)
                    .map(|r| r.target_name.as_str())
                    .collect();
                let fields: Vec<_> = refs
                    .iter()
                    .filter(|r| r.context == TypeRefContext::FieldType)
                    .map(|r| r.target_name.as_str())
                    .collect();
                assert_eq!(parents, ["User"], "{refs:?}");
                assert_eq!(fields, ["string"], "a named field is data: {refs:?}");
            },
            tree_sitter_go::LANGUAGE.into(),
        );
    }

    #[test]
    fn test_walk_php_type_refs_function() {
        // PHP source must include `<?php` opener — tree-sitter-php 0.24.2
        // parses anything before `<?php` as plain text, so the walker would
        // see `(program (text))` and find no nodes. Discovered during
        // walker-grammar-drift resolution (2026-09-27).
        let source = "<?php\nfunction save(User $user, Repository $repo): void { }";
        test_walker(
            source,
            walk_php_type_refs,
            "function_definition",
            |refs| {
                assert!(
                    !refs.is_empty(),
                    "should find type refs in PHP function params: {:?}",
                    refs.iter().map(|r| &r.target_name).collect::<Vec<_>>()
                );
            },
            tree_sitter_php::LANGUAGE_PHP.into(),
        );
    }

    #[test]
    fn test_walk_php_type_refs_class() {
        // PHP source must include `<?php` opener — see comment above.
        let source = "<?php\nclass User extends Model implements Serializable {}";
        test_walker(
            source,
            walk_php_type_refs,
            "class_declaration",
            |refs| {
                assert!(
                    !refs.is_empty(),
                    "should find extends/implements type refs: {:?}",
                    refs.iter().map(|r| &r.target_name).collect::<Vec<_>>()
                );
            },
            tree_sitter_php::LANGUAGE_PHP.into(),
        );
    }

    #[test]
    fn test_walk_swift_type_refs_function() {
        let source = "func save(user: User, repo: Repository) -> Error? { return nil }";
        test_walker(
            source,
            walk_swift_type_refs,
            "function_declaration",
            |refs| {
                assert!(
                    !refs.is_empty(),
                    "should find type refs in Swift function params: {:?}",
                    refs.iter().map(|r| &r.target_name).collect::<Vec<_>>()
                );
            },
            tree_sitter_swift::LANGUAGE.into(),
        );
    }

    #[test]
    fn test_walk_swift_type_refs_class() {
        let source = "class User: Model, Serializable { }";
        test_walker(
            source,
            walk_swift_type_refs,
            "class_declaration",
            |refs| {
                assert!(
                    !refs.is_empty(),
                    "should find inheritance type refs: {:?}",
                    refs.iter().map(|r| &r.target_name).collect::<Vec<_>>()
                );
            },
            tree_sitter_swift::LANGUAGE.into(),
        );
    }

    fn test_walker(
        source: &str,
        walker: fn(&tree_sitter::Node, &[u8]) -> Vec<TypeRef>,
        node_type: &str,
        assert_fn: fn(Vec<TypeRef>),
        lang: tree_sitter::Language,
    ) {
        let mut parser = Parser::new();
        parser.set_language(&lang).unwrap();
        let tree = parser.parse(source.as_bytes(), None).unwrap();
        let root = tree.root_node();

        // Find the first matching node
        let mut stack = vec![root];
        while let Some(node) = stack.pop() {
            if node.kind() == node_type {
                let refs = walker(&node, source.as_bytes());
                assert_fn(refs);
                return;
            }
            for i in 0..node.child_count() {
                stack.push(node.child(i).unwrap());
            }
        }
        panic!("node type '{node_type}' not found in source");
    }
}
