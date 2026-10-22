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
                function_types: &["function_declaration", "method_definition", "arrow_function"],
                class_types: &["class_declaration"],
                variable_types: &["variable_declaration", "lexical_declaration"],
                call_types: &["call_expression", "new_expression"],
                import_types: &["import_statement", "export_statement"],
                callee_fields: &["function", "constructor"],
                import_callees: &[],
            },
            Language::TypeScript => LanguageSymbols {
                function_types: &["function_declaration", "method_definition", "arrow_function"],
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
                call_types: &["call_expression", "constructor_invocation", "new_expression"],
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
        }
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
        ]
    }
}
