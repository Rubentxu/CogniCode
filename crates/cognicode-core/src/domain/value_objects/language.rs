//! The languages CogniCode can parse, and everything it knows about them
//! that is not an implementation detail.
//!
//! `Language` lived in `infrastructure/parser` and was imported from there by
//! `application`, `interface` and about twenty infrastructure modules alike.
//! It was in the wrong layer for the type: a closed enumeration of the
//! languages the product supports, the file extensions that select them, the
//! AST node kinds each one uses for functions and classes, and the language
//! server that speaks to it. None of that is tree-sitter. It is the vocabulary
//! of the problem.
//!
//! What genuinely belongs to the adapter is the single method that maps a
//! variant onto a compiled `tree_sitter_*` grammar, because those are Rust
//! crates that only the infrastructure layer links. It stayed behind as an
//! inherent impl on this very type, which Rust allows in any module of the
//! defining crate, so every call site kept the method syntax it already had.
//! This file is deliberately free of any reference to tree-sitter — which is
//! what lets the application ask "is this a Rust file?" without naming a
//! parser.
//!
//! One definition, re-exported by `infrastructure::parser` for the adapters
//! that already speak this name. There is no second enum to drift.

#![allow(clippy::match_same_arms)]

/// Supported programming languages for parsing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Language {
    Python,
    Rust,
    JavaScript,
    TypeScript,
    Go,
    Java,
    C,
    Cpp,
    CSharp,
    Hcl,
    Yaml,
    Ruby,
    Php,
    Swift,
    Scala,
    Lua,
    Zig,
    Dart,
    Groovy,
    Elixir,
    Erlang,
    Haskell,
    Julia,
    Bash,
    R,
    PowerShell,
    Json,
    Fortran,
    Verilog,
    SystemVerilog,
    /// Kotlin, and the `.kts` scripts that build with it.
    ///
    /// MEDIDO: la gramática es `tree-sitter-kotlin-sg`
    /// 0.4.1, de la familia ast-grep. No expone campos `name`: el nombre
    /// de una declaración es un hijo cuyo tipo lo dice, y eso es lo que
    /// `NameRule` describe.
    Kotlin,
}
/// The AST node kinds a language uses, and how a call names what it calls.
///
/// These names are the vocabulary of the language, not of tree-sitter: this
/// file names node kinds as strings and never loads a grammar. The adapter
/// that binds them to a compiled grammar lives in the infrastructure layer,
/// and a contract checks that every name here is a kind the grammar really
/// produces — a registry that names a node the grammar never emits is a
/// registry that promises symbols nobody will ever find.
///
/// A language that declares nothing in a category says so with an empty
/// list. An empty list is not a gap to be filled with a plausible name: it
/// is the answer "this language has no such thing", and the graph stays
/// empty rather than inventing a symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LanguageSymbols {
    /// Node kinds that declare a function.
    pub function_types: &'static [&'static str],
    /// Node kinds that declare a class, a struct, a trait or an interface.
    pub class_types: &'static [&'static str],
    /// Node kinds that declare a variable or a binding.
    pub variable_types: &'static [&'static str],
    /// Node kinds that call something.
    pub call_types: &'static [&'static str],
    /// Node kinds that declare a dependency.
    pub import_types: &'static [&'static str],
    /// Fields that can name the callee of a call, in the order they are
    /// tried.
    ///
    /// The callee is read from the field the grammar declares, because the
    /// field is where the grammar puts the name. Some grammars call it
    /// `function`, some `method`, some `name`, some `target`; a grammar
    /// that has no such field leaves the callee as the first named child.
    pub callee_fields: &'static [&'static str],
    /// Callees that load a dependency instead of calling something.
    ///
    /// Some languages have no import statement: what they have is a call to
    /// a loader. A call to `require` declares a dependency, and publishing
    /// it as a call would be claiming that the file calls a function named
    /// "require".
    pub import_callees: &'static [&'static str],
}

/// How a language spells the name of a declaration.
///
/// Most grammars put the name in a `name` field, and then the reader has
/// nothing to ask. Some do not: the name is a child whose kind says so,
/// and it may sit behind other children — modifiers, type parameters, a
/// receiver. A reader that only knows the global vocabulary would either
/// miss those names or, worse, take the first identifier it finds and
/// publish a symbol whose name belongs to something else.
///
/// This is that knowledge, as data: which kinds carry a name, which kinds
/// may sit in front of it without ending the search, and which kinds have
/// no name of their own and take the name of the class that contains
/// them. The reader that applies it is the same for every language; only
/// the answers differ.
///
/// MEDIDO: `tree-sitter-kotlin-sg` 0.4.1 no declara
/// ningún campo `name`. El nombre de una función es su `simple_identifier`
/// directo; el de una clase, su `type_identifier`; el de una propiedad, el
/// `simple_identifier` de su `variable_declaration`; y el de un `import`,
/// el `identifier` con la ruta con puntos. Un constructor secundario y un
/// `companion object` no tienen nombre propio: lo toman de la clase que
/// los contiene, que es como Kotlin los llama.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NameRule {
    /// The kinds that carry a name.
    pub kinds: &'static [&'static str],
    /// The kinds that may sit in front of the name without ending the
    /// search for it.
    pub prefixes: &'static [&'static str],
    /// The kinds that have no name of their own, with the suffix their
    /// language appends to the name of the class that contains them.
    ///
    /// An empty suffix means the name is the class's name as it stands.
    pub inherited: &'static [(&'static str, &'static str)],
}

impl NameRule {
    /// The rule the languages that were already supported read by.
    ///
    /// It is the vocabulary that was in force before any language declared
    /// its own: the kinds that carry a name, no prefixes, and nothing
    /// inherited. Every language that does not declare a rule reads exactly
    /// as it did before, because this is the rule it was reading by.
    pub const DEFAULT: NameRule = NameRule {
        kinds: &["identifier", "string_lit", "function_name", "simple_name"],
        prefixes: &[],
        inherited: &[],
    };
}

impl Language {
    /// What this language declares, and how a call names what it calls.
    ///
    /// One table, one truth: the adapter that binds a language to a
    /// compiled grammar reads it from here, and so does every reader that
    /// asks what a language looks like. Two copies of this knowledge would
    /// drift, and a drifted pair is how a language ends up claiming node
    /// kinds its own grammar never emits.
    pub const fn symbols(self) -> LanguageSymbols {
        match self {
            Language::Rust => LanguageSymbols {
                function_types: &["function_item"],
                class_types: &[
                    "struct_item",
                    "enum_item",
                    "trait_item",
                    "impl_item",
                    "union_item",
                ],
                variable_types: &["let_declaration", "const_item", "static_item"],
                call_types: &["call_expression", "macro_invocation"],
                import_types: &["use_declaration"],
                callee_fields: &["function", "macro"],
                import_callees: &[],
            },
            Language::Python => LanguageSymbols {
                function_types: &["function_definition"],
                class_types: &["class_definition"],
                variable_types: &["assignment"],
                call_types: &["call"],
                import_types: &["import_statement", "import_from_statement"],
                callee_fields: &["function"],
                import_callees: &[],
            },
            Language::JavaScript => LanguageSymbols {
                function_types: &[
                    "function_declaration",
                    "method_definition",
                    "arrow_function",
                ],
                class_types: &["class_declaration"],
                variable_types: &["variable_declaration", "lexical_declaration"],
                call_types: &["call_expression", "new_expression"],
                import_types: &["import_statement", "export_statement"],
                callee_fields: &["function", "constructor"],
                import_callees: &[],
            },
            Language::TypeScript => LanguageSymbols {
                function_types: &[
                    "function_declaration",
                    "method_definition",
                    "arrow_function",
                ],
                class_types: &["class_declaration", "interface_declaration"],
                variable_types: &["variable_declaration", "lexical_declaration"],
                call_types: &["call_expression", "new_expression"],
                import_types: &["import_statement", "export_statement"],
                callee_fields: &["function", "constructor"],
                import_callees: &[],
            },
            Language::Go => LanguageSymbols {
                function_types: &["function_declaration", "method_declaration"],
                class_types: &["type_declaration"],
                variable_types: &["short_var_declaration", "var_declaration"],
                call_types: &["call_expression"],
                import_types: &["import_spec"],
                callee_fields: &["function"],
                import_callees: &[],
            },
            Language::Java => LanguageSymbols {
                function_types: &["method_declaration", "constructor_declaration"],
                class_types: &[
                    "class_declaration",
                    "interface_declaration",
                    "enum_declaration",
                ],
                variable_types: &["local_variable_declaration", "field_declaration"],
                call_types: &["method_invocation"],
                import_types: &["import_declaration"],
                callee_fields: &["name"],
                import_callees: &[],
            },
            Language::C => LanguageSymbols {
                function_types: &["function_definition"],
                class_types: &["struct_specifier", "union_specifier", "enum_specifier"],
                variable_types: &["declaration"],
                call_types: &["call_expression"],
                import_types: &["preproc_include"],
                callee_fields: &["function"],
                import_callees: &[],
            },
            Language::Cpp => LanguageSymbols {
                function_types: &["function_definition"],
                class_types: &["class_specifier", "struct_specifier", "union_specifier"],
                variable_types: &["declaration"],
                call_types: &["call_expression"],
                import_types: &["preproc_include", "using_declaration"],
                callee_fields: &["function"],
                import_callees: &[],
            },
            Language::CSharp => LanguageSymbols {
                function_types: &[
                    "method_declaration",
                    "constructor_declaration",
                    "local_function_statement",
                ],
                class_types: &[
                    "class_declaration",
                    "struct_declaration",
                    "interface_declaration",
                    "enum_declaration",
                    "record_declaration",
                ],
                variable_types: &[
                    "local_declaration_statement",
                    "field_declaration",
                    "property_declaration",
                ],
                call_types: &["invocation_expression"],
                import_types: &["using_directive"],
                callee_fields: &["function"],
                import_callees: &[],
            },
            Language::Hcl => LanguageSymbols {
                function_types: &[],
                class_types: &[],
                variable_types: &[],
                call_types: &[],
                import_types: &[],
                callee_fields: &[],
                import_callees: &[],
            },
            Language::Yaml => LanguageSymbols {
                function_types: &[],
                class_types: &[],
                variable_types: &[],
                call_types: &[],
                import_types: &[],
                callee_fields: &[],
                import_callees: &[],
            },
            Language::Ruby => LanguageSymbols {
                function_types: &["method", "singleton_method"],
                class_types: &["class", "module", "singleton_class"],
                variable_types: &["assignment", "operator_assignment"],
                call_types: &["call"],
                import_types: &[],
                callee_fields: &["method"],
                import_callees: &["require", "require_relative", "load", "autoload"],
            },
            Language::Php => LanguageSymbols {
                function_types: &["function_definition", "method_declaration"],
                class_types: &[
                    "class_declaration",
                    "interface_declaration",
                    "trait_declaration",
                    "enum_declaration",
                ],
                variable_types: &["property_element", "static_variable_declaration"],
                call_types: &[
                    "function_call_expression",
                    "member_call_expression",
                    "nullsafe_member_call_expression",
                    "scoped_call_expression",
                ],
                import_types: &["namespace_use_declaration"],
                callee_fields: &["function", "name"],
                import_callees: &[],
            },
            Language::Swift => LanguageSymbols {
                function_types: &[
                    "function_declaration",
                    "init_declaration",
                    "subscript_declaration",
                    "protocol_function_declaration",
                ],
                class_types: &["class_declaration", "protocol_declaration"],
                variable_types: &["property_declaration"],
                call_types: &["call_expression"],
                import_types: &["import_declaration"],
                callee_fields: &[],
                import_callees: &[],
            },
            Language::Scala => LanguageSymbols {
                function_types: &["function_definition", "function_declaration"],
                class_types: &[
                    "class_definition",
                    "object_definition",
                    "trait_definition",
                    "enum_definition",
                ],
                variable_types: &["val_declaration", "var_declaration"],
                call_types: &["call_expression"],
                import_types: &["import_declaration"],
                callee_fields: &["function"],
                import_callees: &[],
            },
            Language::Lua => LanguageSymbols {
                function_types: &["function_declaration"],
                class_types: &[],
                variable_types: &["variable_declaration"],
                call_types: &["function_call"],
                import_types: &[],
                callee_fields: &["name"],
                import_callees: &["require"],
            },
            Language::Zig => LanguageSymbols {
                function_types: &["function_declaration"],
                class_types: &[
                    "struct_declaration",
                    "union_declaration",
                    "enum_declaration",
                    "opaque_declaration",
                ],
                variable_types: &["variable_declaration"],
                call_types: &["call_expression", "builtin_function"],
                import_types: &[],
                callee_fields: &["function"],
                import_callees: &["@import", "@cImport"],
            },
            Language::Dart => LanguageSymbols {
                function_types: &[
                    "function_declaration",
                    "method_declaration",
                    "local_function_declaration",
                    "getter_declaration",
                    "setter_declaration",
                ],
                class_types: &[
                    "class_declaration",
                    "mixin_declaration",
                    "enum_declaration",
                    "extension_declaration",
                ],
                variable_types: &[
                    "initialized_variable_definition",
                    "static_final_declaration",
                    "local_variable_declaration",
                ],
                call_types: &[
                    "call_expression",
                    "constructor_invocation",
                    "new_expression",
                ],
                import_types: &["import_specification"],
                callee_fields: &["function", "constructor"],
                import_callees: &[],
            },
            Language::Groovy => LanguageSymbols {
                function_types: &[
                    "method_declaration",
                    "function_definition",
                    "constructor_declaration",
                ],
                class_types: &[
                    "class_declaration",
                    "interface_declaration",
                    "enum_declaration",
                    "record_declaration",
                ],
                variable_types: &["variable_declarator"],
                call_types: &["method_invocation", "juxt_function_call"],
                import_types: &["import_declaration"],
                callee_fields: &["name"],
                import_callees: &[],
            },
            Language::Elixir => LanguageSymbols {
                function_types: &[],
                class_types: &[],
                variable_types: &[],
                call_types: &["call"],
                import_types: &[],
                callee_fields: &["target"],
                import_callees: &[],
            },
            Language::Erlang => LanguageSymbols {
                function_types: &["function_clause", "fun_clause"],
                class_types: &["module", "spec", "record_decl", "type_alias"],
                variable_types: &["match_expr"],
                call_types: &["call"],
                import_types: &["import_attribute", "import_record_attribute"],
                callee_fields: &["expr"],
                import_callees: &[],
            },
            Language::Haskell => LanguageSymbols {
                function_types: &["function", "signature"],
                class_types: &["data_type", "newtype", "class", "module"],
                variable_types: &["bind"],
                call_types: &["apply"],
                import_types: &["import", "imports"],
                callee_fields: &["function"],
                import_callees: &[],
            },
            Language::Julia => LanguageSymbols {
                function_types: &["function_definition"],
                class_types: &["module_definition"],
                variable_types: &["assignment"],
                call_types: &["call_expression"],
                import_types: &["import_statement"],
                callee_fields: &[],
                import_callees: &[],
            },
            Language::Bash => LanguageSymbols {
                function_types: &["function_definition"],
                class_types: &[],
                variable_types: &["variable_assignment"],
                call_types: &["command"],
                import_types: &[],
                callee_fields: &["name"],
                import_callees: &[],
            },
            Language::R => LanguageSymbols {
                function_types: &["function_definition"],
                class_types: &[],
                variable_types: &["binary_operator"],
                call_types: &["call"],
                import_types: &[],
                callee_fields: &["function"],
                import_callees: &["library", "require", "source"],
            },
            Language::PowerShell => LanguageSymbols {
                function_types: &["function_statement"],
                class_types: &["class_statement"],
                variable_types: &["assignment_expression"],
                call_types: &["command"],
                import_types: &[],
                callee_fields: &["command_name"],
                import_callees: &[],
            },
            Language::Json => LanguageSymbols {
                function_types: &[],
                class_types: &[],
                variable_types: &[],
                call_types: &[],
                import_types: &[],
                callee_fields: &[],
                import_callees: &[],
            },
            Language::Fortran => LanguageSymbols {
                function_types: &["function_statement", "subroutine_statement"],
                class_types: &["module_statement", "derived_type_statement"],
                variable_types: &["variable_declaration"],
                call_types: &["call_expression", "subroutine_call"],
                import_types: &["use_statement", "import_statement", "include_statement"],
                callee_fields: &["function", "subroutine"],
                import_callees: &[],
            },
            Language::Verilog => LanguageSymbols {
                function_types: &["function_declaration", "task_declaration"],
                class_types: &[
                    "module_declaration",
                    "interface_declaration",
                    "package_declaration",
                ],
                variable_types: &["data_declaration", "net_declaration"],
                call_types: &["module_instantiation"],
                import_types: &["include_compiler_directive", "package_import_declaration"],
                callee_fields: &[],
                import_callees: &[],
            },
            Language::SystemVerilog => LanguageSymbols {
                function_types: &["function_declaration", "task_declaration"],
                class_types: &[
                    "module_declaration",
                    "class_declaration",
                    "interface_declaration",
                ],
                variable_types: &["data_declaration", "net_declaration", "class_property"],
                call_types: &["module_instantiation"],
                import_types: &[
                    "include_statement",
                    "include_compiler_directive",
                    "package_import_declaration",
                ],
                callee_fields: &["instance_type"],
                import_callees: &[],
            },
            // MEDIDO sobre `tree-sitter-kotlin-sg` 0.4.1:
            // el nombre de una declaración es un hijo, nunca un campo, y
            // el binding de una propiedad cuelga de `variable_declaration`
            // (su padre `property_declaration` no lo nombra).
            //
            // `class_parameter` es el parámetro del constructor
            // primario. En Kotlin, un `val`/`var` ahí DECLARA UNA
            // PROPIEDAD, y no hay otra sintaxis para declarar un campo
            // de clase: el conjunto es el paralelo de
            // `local_variable_declaration`/`field_declaration` en Java,
            // y por eso comparte con ellos el mismo `SymbolKind`.
            //
            // El constructor PRIMARIO no se publica aparte: su
            // declaración es la de la clase, y publicarlo con el mismo
            // nombre en la misma línea produciría dos símbolos con la
            // MISMA identidad (`fichero:nombre:línea`). El secundario sí
            // se publica, con el nombre de la clase que lo envuelve.
            Language::Kotlin => LanguageSymbols {
                function_types: &["function_declaration", "secondary_constructor"],
                class_types: &[
                    "class_declaration",
                    "object_declaration",
                    "companion_object",
                ],
                variable_types: &["variable_declaration", "class_parameter"],
                call_types: &["call_expression"],
                import_types: &["import_header"],
                callee_fields: &[],
                import_callees: &[],
            },
        }
    }

    /// How this language spells the name of a declaration.
    ///
    /// The reader asks this instead of guessing. A language that says
    /// nothing reads by `NameRule::DEFAULT`, which is what it read by
    /// before it was asked.
    pub const fn name_rule(self) -> NameRule {
        match self {
            // MEDIDO sobre `tree-sitter-kotlin-sg` 0.4.1.
            Language::Kotlin => NameRule {
                kinds: &["identifier", "simple_identifier", "type_identifier"],
                prefixes: &[
                    "modifiers",
                    "type_parameters",
                    "receiver_type",
                    // `val`/`var` live here in a constructor parameter:
                    // `private val radius: Double` carries the binding
                    // keyword BEFORE the name it binds. Without this in
                    // the prefixes the walk stops at the keyword and the
                    // parameter has no name at all.
                    "binding_pattern_kind",
                ],
                inherited: &[
                    ("secondary_constructor", ""),
                    ("companion_object", ".Companion"),
                ],
            },
            _ => NameRule::DEFAULT,
        }
    }

    /// The node kinds that declare the package a file belongs to.
    ///
    /// The taxonomy has always known what a package IS —`SymbolKind::
    /// Package`— but no language said WHICH of its nodes says it, so no
    /// reader had anything to publish. Kotlin is the first that says
    /// something, and it says it here, in the language's own vocabulary.
    ///
    /// This is a seam and not a table: a language that says nothing keeps
    /// saying nothing, so no other language changes what it publishes.
    pub const fn package_types(self) -> &'static [&'static str] {
        match self {
            // MEDIDO sobre `tree-sitter-kotlin-sg` 0.4.1:
            // `package_header` tiene un único hijo nombrado, un
            // `identifier` que lleva la ruta con puntos —la misma forma
            // que el `identifier` de un `import_header`.
            Language::Kotlin => &["package_header"],
            _ => &[],
        }
    }

    /// Whether the reader that predates the generic extractor reads the
    /// declared sets instead of one kind per category.
    ///
    /// The generic extractor has always read `Language::symbols()`. The
    /// older reader — the one the endpoints use — asks for a single kind
    /// per category and matches on it, which loses every kind the language
    /// declares beyond the first. Kotlin declares several kinds in three
    /// categories, so it needs the sets.
    ///
    /// This is the seam, and it is per language on purpose: switching every
    /// language at once would change what fifteen of them publish, and
    /// that is not a change this unit is entitled to make. A language
    /// migrates here when its own unit says so.
    ///
    /// A language that answers `true` also has its symbols named by
    /// `name_rule()`, so that the two readers agree about what a symbol is
    /// called. Two readers answering differently is the drift this
    /// project already paid for once.
    pub const fn reads_declared_symbol_sets(self) -> bool {
        matches!(self, Language::Kotlin)
    }
}

impl Language {
    /// The file extensions that select this language.
    ///
    /// One list per language, and it is the only one: a file that selects a
    /// language here and a different language somewhere else is read two
    /// different ways, and the reader that loses the race is the one that
    /// answers the user.
    pub const fn extensions(self) -> &'static [&'static str] {
        match self {
            Language::Rust => &["rs"],
            Language::Python => &["py", "pyw"],
            Language::JavaScript => &["js", "jsx", "mjs", "cjs"],
            Language::TypeScript => &["ts", "tsx"],
            Language::Go => &["go"],
            Language::Java => &["java"],
            Language::C => &["c", "h"],
            Language::Cpp => &["cpp", "cc", "cxx", "hpp", "hxx"],
            Language::CSharp => &["cs"],
            Language::Hcl => &["tf", "tfvars", "hcl"],
            Language::Yaml => &["yml", "yaml"],
            Language::Ruby => &["rb"],
            Language::Php => &["php"],
            Language::Swift => &["swift"],
            Language::Scala => &["scala"],
            Language::Lua => &["lua", "luau"],
            Language::Zig => &["zig"],
            Language::Dart => &["dart"],
            Language::Groovy => &["groovy", "gradle"],
            Language::Elixir => &["ex", "exs"],
            Language::Erlang => &["erl", "hrl"],
            Language::Haskell => &["hs"],
            Language::Julia => &["jl"],
            Language::Bash => &["sh", "bash"],
            Language::R => &["r"],
            Language::PowerShell => &["ps1", "psm1"],
            Language::Json => &["json"],
            Language::Fortran => &["f", "f90", "f95", "f03", "f08"],
            Language::Verilog => &["v"],
            Language::SystemVerilog => &["sv"],
            Language::Kotlin => &["kt", "kts"],
        }
    }

    /// Detect language from file extension.
    ///
    /// The extension lists above are the declaration; this only asks which
    /// one claims the extension. A hard-coded table here would be a second
    /// declaration, and the two would drift.
    pub fn from_extension(ext: Option<&std::ffi::OsStr>) -> Option<Self> {
        let extension = ext?.to_str()?.to_lowercase();
        Self::all_languages()
            .iter()
            .copied()
            .find(|language| language.extensions().iter().any(|e| *e == extension))
    }

    /// The public identifier of this language.
    ///
    /// The spelling the product publishes: the key of a language in the
    /// support matrix, the name a tool announces when it says what it
    /// reads. It lives here because a language's identity is part of what
    /// a language is, and a second spelling somewhere else is a second
    /// language.
    pub const fn id(self) -> &'static str {
        match self {
            Language::Rust => "rust",
            Language::Python => "python",
            Language::JavaScript => "javascript",
            Language::TypeScript => "typescript",
            Language::Go => "go",
            Language::Java => "java",
            Language::C => "c",
            Language::Cpp => "cpp",
            Language::CSharp => "csharp",
            Language::Hcl => "hcl",
            Language::Yaml => "yaml",
            Language::Ruby => "ruby",
            Language::Php => "php",
            Language::Swift => "swift",
            Language::Scala => "scala",
            Language::Lua => "lua",
            Language::Zig => "zig",
            Language::Dart => "dart",
            Language::Groovy => "groovy",
            Language::Elixir => "elixir",
            Language::Erlang => "erlang",
            Language::Haskell => "haskell",
            Language::Julia => "julia",
            Language::Bash => "bash",
            Language::R => "r",
            Language::PowerShell => "powershell",
            Language::Json => "json",
            Language::Fortran => "fortran",
            Language::Verilog => "verilog",
            Language::SystemVerilog => "systemverilog",
            Language::Kotlin => "kotlin",
        }
    }

    /// Returns the name of the language
    pub fn name(self) -> &'static str {
        match self {
            Language::Python => "Python",
            Language::Rust => "Rust",
            Language::JavaScript => "JavaScript",
            Language::TypeScript => "TypeScript",
            Language::Go => "Go",
            Language::Java => "Java",
            Language::C => "C",
            Language::Cpp => "C++",
            Language::CSharp => "C#",
            Language::Hcl => "HCL",
            Language::Yaml => "YAML",
            Language::Ruby => "Ruby",
            Language::Php => "PHP",
            Language::Swift => "Swift",
            Language::Scala => "Scala",
            Language::Lua => "Lua",
            Language::Zig => "Zig",
            Language::Dart => "Dart",
            Language::Groovy => "Groovy",
            Language::Elixir => "Elixir",
            Language::Erlang => "Erlang",
            Language::Haskell => "Haskell",
            Language::Julia => "Julia",
            Language::Bash => "Bash",
            Language::R => "R",
            Language::PowerShell => "PowerShell",
            Language::Json => "JSON",
            Language::Fortran => "Fortran",
            Language::Verilog => "Verilog",
            Language::SystemVerilog => "SystemVerilog",
            Language::Kotlin => "Kotlin",
        }
    }

    /// Returns the node type for function definitions in this language.
    ///
    /// An empty string means the language declares no such thing: no
    /// grammar produces a node kind named "", so the comparison never
    /// matches and the graph stays empty instead of inventing a symbol.
    pub fn function_node_type(self) -> &'static str {
        self.symbols().function_types.first().copied().unwrap_or("")
    }

    /// Returns the node type for class definitions in this language.
    ///
    /// An empty string means the language declares no such thing, with the
    /// same reasoning as [`Self::function_node_type`].
    pub fn class_node_type(self) -> &'static str {
        self.symbols().class_types.first().copied().unwrap_or("")
    }

    /// Returns the node type for variable declarations in this language.
    ///
    /// An empty string means the language declares no such thing, with the
    /// same reasoning as [`Self::function_node_type`].
    pub fn variable_node_type(self) -> &'static str {
        self.symbols().variable_types.first().copied().unwrap_or("")
    }

    /// Returns the node type for call expressions in this language.
    ///
    /// An empty string means the language declares no such thing, with the
    /// same reasoning as [`Self::function_node_type`].
    pub fn call_node_type(self) -> &'static str {
        self.symbols().call_types.first().copied().unwrap_or("")
    }

    /// The fields that can name the callee of a call, in the order they are
    /// tried.
    ///
    /// The callee is read from the field the grammar declares, because that
    /// is where the grammar puts the name. A grammar that names it `method`,
    /// `name`, `target` or `expr` would otherwise be read as if it had no
    /// name at all, and the call would go unpublished.
    pub fn callee_fields(self) -> &'static [&'static str] {
        self.symbols().callee_fields
    }

    /// Returns the LSP server binary name for this language
    pub fn lsp_server_binary(self) -> &'static str {
        match self {
            Language::Rust => "rust-analyzer",
            Language::Python => "pyright-langserver",
            Language::TypeScript | Language::JavaScript => "typescript-language-server",
            Language::Go => "gopls",
            Language::Java => "jdtls",
            Language::C | Language::Cpp => "clangd",
            Language::CSharp => "omnisharp",
            _ => "",
        }
    }

    /// Returns the install command
    pub fn lsp_install_command(self) -> &'static str {
        match self {
            Language::Rust => "rustup component add rust-analyzer",
            Language::Python => "npm install -g pyright",
            Language::TypeScript | Language::JavaScript => {
                "npm install -g typescript-language-server typescript"
            }
            Language::Go => "go install golang.org/x/tools/gopls@latest",
            Language::Java => "brew install jdtls",
            Language::C | Language::Cpp => "apt install clangd",
            Language::CSharp => "dotnet tool install -g omnisharp",
            _ => "",
        }
    }

    pub fn lsp_args(self) -> &'static [&'static str] {
        match self {
            Language::Rust => &[],
            Language::Go => &["serve"],
            Language::Java => &[],
            _ => &["--stdio"],
        }
    }

    pub fn lsp_server_name(self) -> &'static str {
        match self {
            Language::Rust => "rust-analyzer",
            Language::Python => "pyright",
            Language::TypeScript | Language::JavaScript => "typescript-language-server",
            Language::Go => "gopls",
            Language::Java => "eclipse-jdtls",
            Language::C | Language::Cpp => "clangd",
            Language::CSharp => "omnisharp",
            _ => "",
        }
    }

    /// Returns all supported languages
    pub fn all_languages() -> &'static [Self] {
        &[
            Language::Rust,
            Language::Python,
            Language::JavaScript,
            Language::TypeScript,
            Language::Go,
            Language::Java,
            Language::C,
            Language::Cpp,
            Language::CSharp,
            Language::Hcl,
            Language::Yaml,
            Language::Ruby,
            Language::Php,
            Language::Swift,
            Language::Scala,
            Language::Lua,
            Language::Zig,
            Language::Dart,
            Language::Groovy,
            Language::Elixir,
            Language::Erlang,
            Language::Haskell,
            Language::Julia,
            Language::Bash,
            Language::R,
            Language::PowerShell,
            Language::Json,
            Language::Fortran,
            Language::Verilog,
            Language::SystemVerilog,
            Language::Kotlin,
        ]
    }
}
