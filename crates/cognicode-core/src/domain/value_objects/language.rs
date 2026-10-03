//! The languages CogniCode can parse, and everything it knows about them
//! that is not an implementation detail.
//!
//! `Language` lived in `infrastructure/parser` and was imported from there by
//! `application`, `interface` and about twenty infrastructure modules alike.
//! That is the CR-06 drift `infrastructure::parser::` in the allowlist named,
//! and it was the wrong layer for the type: a closed enumeration of the
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

impl Language {
    /// Detect language from file extension
    pub fn from_extension(ext: Option<&std::ffi::OsStr>) -> Option<Self> {
        ext.and_then(|e| e.to_str())
            .and_then(|s| match s.to_lowercase().as_str() {
                "py" => Some(Language::Python),
                "rs" => Some(Language::Rust),
                "js" => Some(Language::JavaScript),
                "ts" => Some(Language::TypeScript),
                "jsx" => Some(Language::JavaScript),
                "tsx" => Some(Language::TypeScript),
                "go" => Some(Language::Go),
                "java" => Some(Language::Java),
                "c" | "h" => Some(Language::C),
                "cpp" | "cc" | "cxx" | "hpp" | "hxx" => Some(Language::Cpp),
                "cs" => Some(Language::CSharp),
                "tf" | "tfvars" | "hcl" => Some(Language::Hcl),
                "yml" | "yaml" => Some(Language::Yaml),
                "rb" => Some(Language::Ruby),
                "php" => Some(Language::Php),
                "swift" => Some(Language::Swift),
                "scala" => Some(Language::Scala),
                "lua" | "luau" => Some(Language::Lua),
                "zig" => Some(Language::Zig),
                "dart" => Some(Language::Dart),
                "groovy" | "gradle" => Some(Language::Groovy),
                "ex" | "exs" => Some(Language::Elixir),
                "erl" | "hrl" => Some(Language::Erlang),
                "hs" => Some(Language::Haskell),
                "jl" => Some(Language::Julia),
                "sh" | "bash" => Some(Language::Bash),
                "r" | "R" => Some(Language::R),
                "ps1" | "psm1" => Some(Language::PowerShell),
                "json" => Some(Language::Json),
                "f" | "f90" | "f95" | "f03" | "f08" => Some(Language::Fortran),
                "v" => Some(Language::Verilog),
                "sv" => Some(Language::SystemVerilog),
                _ => None,
            })
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

    /// Returns the node type for function definitions in this language
    pub fn function_node_type(self) -> &'static str {
        match self {
            Language::Python => "function_definition",
            Language::Rust => "function_item",
            Language::JavaScript | Language::TypeScript => "function_declaration",
            Language::Go => "function_declaration",
            Language::Java => "method_declaration",
            Language::C => "function_definition",
            Language::Cpp => "function_definition",
            Language::CSharp => "method_declaration",
            Language::Hcl => "block",
            Language::Yaml => "block_mapping",
            Language::Ruby => "method",
            // tree-sitter-php 0.24.2 emits `function_definition` for free
            // functions (was previously mapped to `method_declaration`,
            // which only appears for class methods). `method_declaration`
            // is still extracted via the iterative DFS in
            // `find_all_symbols_with_path`, so we are not losing any
            // coverage; we are restoring it for free functions.
            Language::Php => "function_definition",
            // tree-sitter-swift 0.7.3 emits `function_declaration` for
            // free functions. `method_declaration` only appears inside
            // type bodies (class/struct/protocol/enum).
            Language::Swift => "function_declaration",
            Language::Scala => "function_declaration",
            Language::Lua => "function_declaration",
            Language::Zig => "function_declaration",
            Language::Dart => "method_declaration",
            Language::Groovy => "method_declaration",
            Language::Elixir => "function",
            Language::Erlang => "function_clause",
            Language::Haskell => "function",
            Language::Julia => "function_definition",
            Language::Bash => "function_definition",
            Language::R => "function_definition",
            Language::PowerShell => "function_definition",
            Language::Json => "object",
            Language::Fortran => "function_definition",
            Language::Verilog => "module_declaration",
            Language::SystemVerilog => "module_declaration",
        }
    }

    /// Returns the node type for class definitions in this language
    pub fn class_node_type(self) -> &'static str {
        match self {
            Language::Python => "class_definition",
            Language::Rust => "struct_item",
            Language::JavaScript | Language::TypeScript => "class_declaration",
            Language::Go => "type_declaration",
            Language::Java => "class_declaration",
            Language::C => "struct_specifier",
            Language::Cpp => "class_specifier",
            Language::CSharp => "class_declaration",
            Language::Hcl => "block",
            Language::Yaml => "block_mapping",
            Language::Ruby => "class",
            Language::Php => "class_declaration",
            Language::Swift => "class_declaration",
            Language::Scala => "class_declaration",
            Language::Lua => "function_declaration",
            Language::Zig => "struct_declaration",
            Language::Dart => "class_declaration",
            Language::Groovy => "class_declaration",
            Language::Elixir => "module",
            Language::Erlang => "module",
            Language::Haskell => "module",
            Language::Julia => "module_definition",
            Language::Bash => "function_definition",
            Language::R => "function_definition",
            Language::PowerShell => "function_definition",
            Language::Json => "object",
            Language::Fortran => "module",
            Language::Verilog => "module_declaration",
            Language::SystemVerilog => "module_declaration",
        }
    }

    /// Returns the node type for variable declarations in this language
    pub fn variable_node_type(self) -> &'static str {
        match self {
            Language::Python => "variable_declaration",
            Language::Rust => "let_declaration",
            Language::JavaScript | Language::TypeScript => "variable_declaration",
            Language::Go => "short_var_declaration",
            Language::Java => "local_variable_declaration",
            Language::C => "declaration",
            Language::Cpp => "declaration",
            Language::CSharp => "local_declaration_statement",
            Language::Hcl => "attribute",
            Language::Yaml => "block_mapping_pair",
            Language::Ruby => "assignment",
            Language::Php => "expression_statement",
            Language::Swift => "variable_declaration",
            Language::Scala => "val_declaration",
            Language::Lua => "variable_declaration",
            Language::Zig => "variable_declaration",
            Language::Dart => "variable_declaration",
            Language::Groovy => "variable_declaration",
            Language::Elixir => "variable_declaration",
            Language::Erlang => "variable_declaration",
            Language::Haskell => "declaration",
            Language::Julia => "assignment",
            Language::Bash => "variable_assignment",
            Language::R => "assignment",
            Language::PowerShell => "assignment",
            Language::Json => "pair",
            Language::Fortran => "variable_declaration",
            Language::Verilog => "variable_declaration",
            Language::SystemVerilog => "variable_declaration",
        }
    }

    /// Returns the node type for call expressions in this language
    pub fn call_node_type(self) -> &'static str {
        match self {
            Language::Python => "call",
            Language::Rust => "call_expression",
            Language::JavaScript | Language::TypeScript => "call_expression",
            Language::Go => "call_expression",
            Language::Java => "method_invocation",
            Language::C => "call_expression",
            Language::Cpp => "call_expression",
            Language::CSharp => "invocation_expression",
            Language::Hcl => "expression",
            Language::Yaml => "flow_node",
            Language::Ruby => "call",
            Language::Php => "function_call_expression",
            Language::Swift => "call_expression",
            Language::Scala => "call_expression",
            Language::Lua => "function_call",
            Language::Zig => "call_expression",
            Language::Dart => "function_expression_invocation",
            Language::Groovy => "method_call_expression",
            Language::Elixir => "call",
            Language::Erlang => "function_call",
            Language::Haskell => "application",
            Language::Julia => "call_expression",
            Language::Bash => "command",
            Language::R => "call",
            Language::PowerShell => "command",
            Language::Json => "string",
            Language::Fortran => "call_expression",
            Language::Verilog => "module_instantiation",
            Language::SystemVerilog => "module_instantiation",
        }
    }

    /// Returns whether this language uses 'function' field in call nodes
    pub fn call_has_function_field(self) -> bool {
        match self {
            Language::Python => true,
            Language::Rust => false,
            Language::JavaScript | Language::TypeScript => true,
            Language::Go => true,
            Language::Java => false,
            Language::C => true,
            Language::Cpp => true,
            Language::CSharp => true,
            Language::Hcl => false,
            Language::Yaml => false,
            Language::Ruby => true,
            Language::Php => true,
            Language::Swift => true,
            Language::Scala => true,
            Language::Lua => true,
            Language::Zig => true,
            Language::Dart => true,
            Language::Groovy => true,
            Language::Elixir => true,
            Language::Erlang => true,
            Language::Haskell => true,
            Language::Julia => true,
            Language::Bash => true,
            Language::R => true,
            Language::PowerShell => true,
            Language::Json => false,
            Language::Fortran => true,
            Language::Verilog => false,
            Language::SystemVerilog => false,
        }
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
